//! Codesigning using Apple's builtin `codesign` tool.
//! Because Apple's tools are tightly integrated into their
//! ecosystem, there's a couple of considerations here:
//! 1) This can only be run on a Mac, and
//! 2) Apple expects certificates to be located in the Keychain,
//!    a Mac-specific certificate store, which interacts a bit
//!    weirdly with our ephemeral runner setup in CI.
//! Most of this module is actually concerned with ephemeral
//! keychain setup, with the signing section of the code relatively
//! short in comparison. The keychain code will be reused elsewhere
//! in the future.
//!
//! The workflow we follow here is:
//! 1) Create an ephemeral keychain in a temporary directory;
//! 2) Configure it to be usable for signing;
//! 3) Import the certificate specified in the environment;
//! 4) Actually perform the signing;
//! 5) Let the keychain be deleted when the temporary directory is dropped.
//!
//! In the future, this module will also support notarization.
use axoasset::LocalAsset;
use axoprocess::Cmd;
use base64::Engine;
use camino::{Utf8Path, Utf8PathBuf};
use temp_dir::TempDir;
use tracing::warn;

use crate::{create_tmp, DistError, DistResult, TargetTriple};

/// Represents a temporary macOS keychain database. The database object will be created within `_root`, and deleted once this struct is dropped.
pub struct Keychain {
    _root: TempDir,
    root_path: Utf8PathBuf,
    password: String,
    /// The path to the keychain database.
    pub path: Utf8PathBuf,
}

impl Keychain {
    /// Creates a keychain in a temporary directory, secured
    /// with the provided password.
    pub fn create(password: String) -> DistResult<Self> {
        let (root, root_path) = create_tmp()?;
        let path = root_path.join("signing.keychain-db");

        let mut cmd = Cmd::new("/usr/bin/security", "create keychain");
        cmd.arg("create-keychain");
        cmd.arg("-p").arg(&password);
        cmd.arg(&path);
        cmd.stdout_to_stderr();
        cmd.status()?;

        // This sets a longer timeout so that it remains
        // unlocked by the time we perform the signature;
        // the keychain will be deleted before this
        // lock period expires.
        let mut cmd = Cmd::new("/usr/bin/security", "set timeout");
        cmd.arg("set-keychain-settings");
        cmd.arg("-lut").arg("21600");
        cmd.arg(&path);
        cmd.stdout_to_stderr();
        cmd.status()?;

        // Unlock for use in later commands
        let mut cmd = Cmd::new("/usr/bin/security", "unlock keychain");
        cmd.arg("unlock-keychain");
        cmd.arg("-p").arg(&password);
        cmd.arg(&path);
        cmd.stdout_to_stderr();
        cmd.status()?;

        // Set as the default keychain for subsequent commands
        let mut cmd = Cmd::new("/usr/bin/security", "set keychain as default");
        cmd.arg("default-keychain");
        cmd.arg("-s");
        cmd.arg(&path);
        cmd.stdout_to_stderr();
        cmd.status()?;

        Ok(Self {
            _root: root,
            root_path,
            password,
            path,
        })
    }

    /// Imports certificate `certificate` with passphrase `passphrase`
    /// into the keychain at `self`.
    pub fn import_certificate(&self, certificate: &[u8], passphrase: &str) -> DistResult<()> {
        // Temporarily write `certificate` into `path` for `security`
        let cert_path = self.root_path.join("cert.p12");
        LocalAsset::new(&cert_path, certificate.to_owned())?.write_to_dir(&self.root_path)?;

        let mut cmd = Cmd::new("/usr/bin/security", "import certificate");
        cmd.arg("import");
        cmd.arg(&cert_path);
        cmd.arg("-k").arg(&self.path);
        cmd.arg("-P").arg(passphrase);
        cmd.arg("-t").arg("cert");
        cmd.arg("-f").arg("pkcs12");
        cmd.arg("-A");
        cmd.arg("-T")
            .arg("/usr/bin/codesign")
            .arg("-T")
            .arg("/usr/bin/security")
            .arg("-T")
            .arg("/usr/bin/productsign");
        cmd.stdout_to_stderr();
        cmd.status()?;

        let mut cmd = Cmd::new("/usr/bin/security", "configure certificate for signing");
        cmd.arg("set-key-partition-list");
        cmd.arg("-S").arg("apple-tool:,apple:,codesign:");
        cmd.arg("-k").arg(&self.password);
        cmd.arg(&self.path);
        cmd.stdout_to_stderr();
        cmd.status()?;

        Ok(())
    }
}

/// Configuration for the system macOS codesign(1)
#[derive(Debug)]
pub struct Codesign {
    env: CodesignEnv,
}

struct CodesignEnv {
    pub identity: String,
    pub password: String,
    pub certificate: Vec<u8>,
}

impl CodesignEnv {
    pub fn from(identity: &str, password: &str, raw_certificate: &str) -> DistResult<Self> {
        let certificate = base64::prelude::BASE64_STANDARD
            .decode(raw_certificate)
            .map_err(|_| DistError::CertificateDecodeError {})?;

        Ok(Self {
            identity: identity.to_owned(),
            password: password.to_owned(),
            certificate,
        })
    }
}

impl std::fmt::Debug for CodesignEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodesignEnv")
            .field("identity", &"<hidden>")
            .field("password", &"<hidden>")
            .field("certificate", &"<hidden>")
            .finish()
    }
}

impl Codesign {
    /// Creates a new Codesign instance, if the host is darwin and the required information is in the environment
    pub fn new(host_target: &TargetTriple) -> DistResult<Option<Self>> {
        if !host_target.contains("darwin") {
            return Ok(None);
        }

        if let (Some(identity), Some(password), Some(certificate)) = (
            Self::var("CODESIGN_IDENTITY"),
            Self::var("CODESIGN_CERTIFICATE_PASSWORD"),
            Self::var("CODESIGN_CERTIFICATE"),
        ) {
            let env = CodesignEnv::from(&identity, &password, &certificate)?;

            Ok(Some(Self { env }))
        } else {
            Ok(None)
        }
    }

    fn var(var: &str) -> Option<String> {
        let val = std::env::var(var).ok();
        if val.is_none() {
            warn!("{var} is missing");
        }
        val
    }

    /// Creates a Keychain with this signer's certificate imported,
    /// then returns it.
    pub fn create_keychain(&self) -> DistResult<Keychain> {
        let password = uuid::Uuid::new_v4().as_hyphenated().to_string();
        let keychain = Keychain::create(password)?;
        keychain.import_certificate(&self.env.certificate, &self.env.password)?;

        Ok(keychain)
    }

    /// Signs a binary using `codesign`.
    pub fn sign(&self, file: &Utf8Path) -> DistResult<()> {
        let keychain = self.create_keychain()?;

        let mut cmd = Cmd::new("/usr/bin/codesign", "sign macOS artifacts");
        cmd.arg("--sign").arg(&self.env.identity);
        cmd.arg("--keychain").arg(&keychain.path);
        cmd.arg(file);
        cmd.stdout_to_stderr();
        cmd.output()?;

        Ok(())
    }

    /// Returns the signing identity represented by this signer.
    pub fn identity(&self) -> &str {
        &self.env.identity
    }
}
