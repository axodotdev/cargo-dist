//! Codesigning using Azure Artifact Signing.

use axoprocess::Cmd;
use camino::Utf8Path;
use cargo_dist_schema::TripleNameRef;
use tracing::info;
use tracing::warn;

use crate::errors::*;
use crate::platform::targets::TARGET_X64_WINDOWS;

/// An instance of Azure Artifact Signing.
#[derive(Debug)]
pub struct AzureArtifactSigning {
    env: AzureArtifactSigningEnv,
}

/// Required env vars for Azure Artifact Signing.
struct AzureArtifactSigningEnv {
    endpoint: String,
    account_name: String,
    certificate_profile_name: String,
}

// manual debug impl to prevent anyone adding derive(Debug) and leaking new auth fields later
impl std::fmt::Debug for AzureArtifactSigningEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzureArtifactSigningEnv")
            .field("endpoint", &self.endpoint)
            .field("account_name", &self.account_name)
            .field("certificate_profile_name", &self.certificate_profile_name)
            .finish()
    }
}

impl AzureArtifactSigningEnv {
    fn new() -> Option<Self> {
        if let (Some(endpoint), Some(account_name), Some(certificate_profile_name)) = (
            Self::var("AZURE_CODESIGNING_ENDPOINT"),
            Self::var("AZURE_CODESIGNING_ACCOUNT_NAME"),
            Self::var("AZURE_CODESIGNING_CERT_PROFILE_NAME"),
        ) {
            Some(Self {
                endpoint,
                account_name,
                certificate_profile_name,
            })
        } else {
            None
        }
    }

    fn var(var: &str) -> Option<String> {
        let val = std::env::var(var).ok();
        if val.is_none() {
            warn!("{var} is missing");
        }
        val
    }
}

impl AzureArtifactSigning {
    pub fn new(host_target: &TripleNameRef, azure_windows_sign: bool) -> DistResult<Option<Self>> {
        // Feature must be enabled
        if !azure_windows_sign {
            return Ok(None);
        }
        // Azure Artifact Signing's supported GitHub-hosted runner path is x64 Windows.
        if host_target != TARGET_X64_WINDOWS {
            return Ok(None);
        }

        if let Some(env) = AzureArtifactSigningEnv::new() {
            Ok(Some(Self { env }))
        } else {
            warn!("skipping codesigning, required AZURE_CODESIGNING env-vars aren't set");
            Ok(None)
        }
    }

    pub fn sign(&self, file: &Utf8Path) -> DistResult<()> {
        info!("Azure Artifact Signing {file}");

        let AzureArtifactSigningEnv {
            endpoint,
            account_name,
            certificate_profile_name,
        } = &self.env;

        // Match Azure's official GitHub Action integration, which wraps this module.
        let script = r#"
$ErrorActionPreference = 'Stop'
Import-Module ArtifactSigning
Invoke-ArtifactSigning `
    -Endpoint $env:AZURE_CODESIGNING_ENDPOINT `
    -CodeSigningAccountName $env:AZURE_CODESIGNING_ACCOUNT_NAME `
    -CertificateProfileName $env:AZURE_CODESIGNING_CERT_PROFILE_NAME `
    -Files $env:CARGO_DIST_SIGN_FILE `
    -FileDigest SHA256 `
    -TimestampRfc3161 'http://timestamp.acs.microsoft.com' `
    -TimestampDigest SHA256
"#;

        Cmd::new("pwsh", "sign windows artifacts")
            .arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-Command")
            .arg(script)
            .env("AZURE_CODESIGNING_ENDPOINT", endpoint)
            .env("AZURE_CODESIGNING_ACCOUNT_NAME", account_name)
            .env(
                "AZURE_CODESIGNING_CERT_PROFILE_NAME",
                certificate_profile_name,
            )
            .env("CARGO_DIST_SIGN_FILE", file.as_str())
            .stdout_to_stderr()
            .status()?;
        Ok(())
    }
}
