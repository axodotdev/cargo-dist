#![deny(missing_docs)]
#![allow(clippy::result_large_err)]

//! axoupdater crate

pub mod errors;
mod receipt;
mod release;
pub mod test;

pub use errors::*;
pub use release::*;

use std::{
    env::{self, args},
    ffi::OsStr,
    process::Stdio,
};

#[cfg(unix)]
use std::{fs::File, os::unix::fs::PermissionsExt};

#[cfg(windows)]
use self_replace;

use axoasset::LocalAsset;
use axoprocess::Cmd;
pub use axotag::Version;
use camino::Utf8PathBuf;

use tempfile::TempDir;

/// Version number for this release of axoupdater.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Provides information about the result of the upgrade operation
pub struct UpdateResult {
    /// The old version (pre-upgrade)
    pub old_version: Option<Version>,
    /// The new version (post-upgrade)
    pub new_version: Version,
    /// The tag the new version was created from
    pub new_version_tag: String,
    /// The root that the new version was installed to
    /// NOTE: This is a prediction, and the underlying installer may ignore it
    /// if it's out of date. Installers built with cargo-dist 0.12.0 or later
    /// will definitively use this value.
    pub install_prefix: Utf8PathBuf,
}

/// Used to specify what version to upgrade to
#[derive(Clone)]
pub enum UpdateRequest {
    /// Always update to the latest
    Latest,
    /// Always update to the latest, allow prereleases
    LatestMaybePrerelease,
    /// Upgrade (or downgrade) to this specific version
    SpecificVersion(String),
    /// Upgrade (or downgrade) to this specific tag
    SpecificTag(String),
}

#[derive(Default)]
pub(crate) struct AuthorizationTokens {
    github: Option<String>,
    axodotdev: Option<String>,
}

/// Tool used to produce this install receipt
pub struct Provider {
    /// The name of the tool used to create this receipt
    pub source: String,
    /// The version of the above tool
    pub version: Version,
}

/// Struct representing an updater process
pub struct AxoUpdater {
    /// The name of the program to update, if specified
    pub name: Option<String>,
    /// Information about where updates should be fetched from
    pub source: Option<ReleaseSource>,
    /// What version should be updated to
    version_specifier: UpdateRequest,
    /// Information about the latest release; used to determine if an update is needed
    requested_release: Option<Release>,
    /// The current version number
    current_version: Option<Version>,
    /// Version of cargo-dist current version is installed by
    current_version_installed_by: Option<Provider>,
    /// Information about the install prefix of the previous version
    install_prefix: Option<Utf8PathBuf>,
    /// Whether to display the underlying installer's stdout
    print_installer_stdout: bool,
    /// Whether to display the underlying installer's stderr
    print_installer_stderr: bool,
    /// The path to the installer to use for the new version.
    /// If not specified, downloads the installer from the release source.
    installer_path: Option<Utf8PathBuf>,
    /// A token to use to query releases from GitHub. If not supplied,
    /// AxoUpdater will perform unauthorized requests.
    tokens: AuthorizationTokens,
    /// When set to true, skips performing version checks and always assumes
    /// the software is out of date.
    always_update: bool,
    /// Whether to modify the system path when installing
    modify_path: bool,
}

impl Default for AxoUpdater {
    fn default() -> Self {
        Self::new()
    }
}

impl AxoUpdater {
    /// Creates a new, empty AxoUpdater struct. This struct lacks information
    /// necessary to perform the update, so at least the name and source fields
    /// will need to be filled in before the update can run.
    pub fn new() -> AxoUpdater {
        AxoUpdater {
            name: None,
            source: None,
            version_specifier: UpdateRequest::Latest,
            requested_release: None,
            current_version: None,
            current_version_installed_by: None,
            install_prefix: None,
            print_installer_stdout: true,
            print_installer_stderr: true,
            installer_path: None,
            tokens: AuthorizationTokens::default(),
            always_update: false,
            modify_path: true,
        }
    }

    /// Creates a new AxoUpdater struct with an explicitly-specified name.
    pub fn new_for(app_name: &str) -> AxoUpdater {
        AxoUpdater {
            name: Some(app_name.to_owned()),
            source: None,
            version_specifier: UpdateRequest::Latest,
            requested_release: None,
            current_version: None,
            current_version_installed_by: None,
            install_prefix: None,
            print_installer_stdout: true,
            print_installer_stderr: true,
            installer_path: None,
            tokens: AuthorizationTokens::default(),
            always_update: false,
            modify_path: true,
        }
    }

    /// Creates a new AxoUpdater struct by attempting to autodetect the name
    /// of the current executable. This is only meant to be used by standalone
    /// updaters, not when this crate is used as a library in another program.
    pub fn new_for_updater_executable() -> AxoupdateResult<AxoUpdater> {
        let Some(app_name) = get_app_name() else {
            return Err(AxoupdateError::NoAppName {});
        };

        // Happens if the binary didn't get renamed properly
        if app_name == "axoupdater" {
            return Err(AxoupdateError::UpdateSelf {});
        };

        Ok(AxoUpdater {
            name: Some(app_name.to_owned()),
            source: None,
            version_specifier: UpdateRequest::Latest,
            requested_release: None,
            current_version: None,
            current_version_installed_by: None,
            install_prefix: None,
            print_installer_stdout: true,
            print_installer_stderr: true,
            installer_path: None,
            tokens: AuthorizationTokens::default(),
            always_update: false,
            modify_path: true,
        })
    }

    /// Explicitly configures the release source as an alternative to
    /// reading it from the install receipt. This can be useful for tasks
    /// which want to query the new version without actually performing an
    /// upgrade.
    pub fn set_release_source(&mut self, source: ReleaseSource) -> &mut AxoUpdater {
        self.source = Some(source);

        self
    }

    /// Explicitly specifies the current version.
    pub fn set_current_version(&mut self, version: Version) -> AxoupdateResult<&mut AxoUpdater> {
        self.current_version = Some(version);

        Ok(self)
    }

    /// Changes this updater's name to `app_name`, regardless of what it was
    /// initialized as and regardless of what was read from the receipt.
    pub fn set_name(&mut self, app_name: &str) -> &mut AxoUpdater {
        self.name = Some(app_name.to_owned());
        if let Some(source) = &self.source {
            let mut our_source = source.clone();
            our_source.app_name = app_name.to_owned();
            self.source = Some(our_source);
        }

        self
    }

    /// Enables printing the underlying installer's stdout.
    pub fn enable_installer_stdout(&mut self) -> &mut AxoUpdater {
        self.print_installer_stdout = true;

        self
    }

    /// Disables printing the underlying installer's stdout.
    pub fn disable_installer_stdout(&mut self) -> &mut AxoUpdater {
        self.print_installer_stdout = false;

        self
    }

    /// Enables printing the underlying installer's stderr.
    pub fn enable_installer_stderr(&mut self) -> &mut AxoUpdater {
        self.print_installer_stderr = true;

        self
    }

    /// Disables printing the underlying installer's stderr.
    pub fn disable_installer_stderr(&mut self) -> &mut AxoUpdater {
        self.print_installer_stderr = false;

        self
    }

    /// Enables all output for the underlying installer.
    pub fn enable_installer_output(&mut self) -> &mut AxoUpdater {
        self.print_installer_stdout = true;
        self.print_installer_stderr = true;

        self
    }

    /// Disables all output for the underlying installer.
    pub fn disable_installer_output(&mut self) -> &mut AxoUpdater {
        self.print_installer_stdout = false;
        self.print_installer_stderr = false;

        self
    }

    /// Configures AxoUpdater to use a specific installer for the new release
    /// instead of downloading it from the release source.
    pub fn configure_installer_path(&mut self, path: impl Into<Utf8PathBuf>) -> &mut AxoUpdater {
        self.installer_path = Some(path.into().to_owned());

        self
    }

    /// Configures AxoUpdater to use the installer from the new release.
    /// This is the default setting.
    pub fn use_release_installer(&mut self) -> &mut AxoUpdater {
        self.installer_path = None;

        self
    }

    /// Configures AxoUpdater with the install path to use. This is only needed
    /// if installing without an explicit install prefix.
    pub fn set_install_dir(&mut self, path: impl Into<Utf8PathBuf>) -> &mut AxoUpdater {
        self.install_prefix = Some(path.into());

        self
    }

    /// Configures axoupdater's update strategy, replacing whatever was
    /// previously configured with the strategy in `version_specifier`.
    pub fn configure_version_specifier(
        &mut self,
        version_specifier: UpdateRequest,
    ) -> &mut AxoUpdater {
        self.version_specifier = version_specifier;

        self
    }

    /// Always upgrade, including when already running the latest version or when the current version isn't known
    pub fn always_update(&mut self, setting: bool) -> &mut AxoUpdater {
        self.always_update = setting;

        self
    }

    /// Determines if an update is needed by querying the newest version from
    /// the location specified in `source`.
    /// This includes a blocking network call, so it may be slow.
    /// This can only be performed if the `current_version` field has been
    /// set, either by loading the install receipt or by specifying it using
    /// `set_current_version`.
    /// Note that this also checks to see if the current executable is
    /// *eligible* for updates, by checking to see if it's the executable
    /// that the install receipt is for. In the case that the executable comes
    /// from a different source, it will return before the network call for a
    /// new version.
    pub async fn is_update_needed(&mut self) -> AxoupdateResult<bool> {
        if self.always_update {
            return Ok(true);
        }

        if !self.check_receipt_is_for_this_executable()? {
            return Ok(false);
        }

        let Some(current_version) = self.current_version.to_owned() else {
            return Err(AxoupdateError::NotConfigured {
                missing_field: "current_version".to_owned(),
            });
        };

        let release = match &self.requested_release {
            Some(r) => r,
            None => {
                self.fetch_release().await?;
                self.requested_release.as_ref().unwrap()
            }
        };

        // If we're doing "latest" semantics we need to check cur < new
        // If we're doing "specific" semantics we need to check cur != new
        let conclusion = match self.version_specifier {
            UpdateRequest::Latest | UpdateRequest::LatestMaybePrerelease => {
                current_version < release.version
            }
            UpdateRequest::SpecificVersion(_) | UpdateRequest::SpecificTag(_) => {
                current_version != release.version
            }
        };
        Ok(conclusion)
    }

    #[cfg(feature = "blocking")]
    /// Identical to Axoupdater::is_update_needed(), but performed synchronously.
    pub fn is_update_needed_sync(&mut self) -> AxoupdateResult<bool> {
        tokio::runtime::Builder::new_current_thread()
            .worker_threads(1)
            .max_blocking_threads(128)
            .enable_all()
            .build()
            .expect("Initializing tokio runtime failed")
            .block_on(self.is_update_needed())
    }

    /// Returns the root of the install prefix, stripping the final `/bin`
    /// component if necessary. Works around a bug introduced in cargo-dist
    /// where this field was returned inconsistently in receipts for a few
    /// versions.
    pub fn install_prefix_root(&self) -> AxoupdateResult<Utf8PathBuf> {
        let Some(install_prefix) = &self.install_prefix else {
            return Err(AxoupdateError::NotConfigured {
                missing_field: "install_prefix".to_owned(),
            });
        };

        let mut install_root = install_prefix.to_owned();
        // Works around a bug in cargo-dist between 0.10.0 and 0.15.0, in which
        // prefix-style workspaces like CARGO_HOME had the prefix incorrectly
        // set to include the `bin` directory.
        if let Some(provider) = &self.current_version_installed_by {
            let min = Version::parse("0.10.0-prerelease.1").expect("failed to parse min version?!");
            let max = Version::parse("0.15.0-prerelease.8").expect("failed to parse max version?!");
            if provider.source == "cargo-dist" && provider.version >= min && provider.version < max
            {
                install_root = root_without_bin(&install_root);
            }
        }

        Ok(install_root)
    }

    /// Returns a normalized version of install_prefix_root, for comparison
    fn install_prefix_root_normalized(&self) -> AxoupdateResult<Utf8PathBuf> {
        let raw_root = self.install_prefix_root()?;
        // The canonicalize path could fail if the path doesn't exist anymore;
        // catch that specific error here and return the original path.
        // (We want to leave the UTF8 conversion to the next step so we handle
        // those errors separately.)
        let canonicalized = if let Ok(path) = raw_root.canonicalize() {
            path
        } else {
            raw_root.into_std_path_buf()
        };
        let normalized = Utf8PathBuf::from_path_buf(canonicalized)
            .map_err(|path| AxoupdateError::CaminoConversionFailed { path })?;
        Ok(normalized)
    }

    /// Attempts to perform an update. The return value specifies whether an
    /// update was actually performed or not; false indicates "no update was
    /// needed", while an error indicates that an update couldn't be performed
    /// due to an error.
    pub async fn run(&mut self) -> AxoupdateResult<Option<UpdateResult>> {
        if !self.is_update_needed().await? {
            return Ok(None);
        }

        let release = match &self.requested_release {
            Some(r) => r,
            None => {
                self.fetch_release().await?;
                self.requested_release.as_ref().unwrap()
            }
        };
        let tempdir = TempDir::new()?;

        // If we've been given an installer path to use, skip downloading and
        // install from that.
        let installer_path = if let Some(path) = &self.installer_path {
            path.to_owned()
        // Otherwise, proceed with downloading the installer from the release
        // we just looked up.
        } else {
            let app_name = self.name.clone().unwrap_or_default();
            let installer_url = match env::consts::OS {
                "macos" | "linux" => release
                    .assets
                    .iter()
                    .find(|asset| asset.name == format!("{app_name}-installer.sh")),
                "windows" => release
                    .assets
                    .iter()
                    .find(|asset| asset.name == format!("{app_name}-installer.ps1")),
                _ => unreachable!(),
            };

            let installer_url = if let Some(installer_url) = installer_url {
                installer_url
            } else {
                return Err(AxoupdateError::NoInstallerForPackage {});
            };

            let extension = if cfg!(windows) { ".ps1" } else { ".sh" };

            let installer_path =
                Utf8PathBuf::try_from(tempdir.path().join(format!("installer{extension}")))?;

            #[cfg(unix)]
            {
                let installer_file = File::create(&installer_path)?;
                let mut perms = installer_file.metadata()?.permissions();
                perms.set_mode(0o744);
                installer_file.set_permissions(perms)?;
            }

            let client = axoasset::reqwest::Client::new();
            let download = client
                .get(&installer_url.browser_download_url)
                .header(
                    axoasset::reqwest::header::ACCEPT,
                    "application/octet-stream",
                )
                .send()
                .await?
                .text()
                .await?;

            LocalAsset::write_new_all(&download, &installer_path)?;

            installer_path
        };

        // Before we update, rename ourselves to a temporary name.
        // This is necessary because Windows won't let an actively-running
        // executable be overwritten.
        // If the update fails, we'll move it back to where it was before
        // we began the update process.
        let to_restore = if cfg!(target_family = "windows") {
            let old_filename = std::env::current_exe()?;

            let mut new_filename = old_filename.as_os_str().to_os_string();
            // Filename follows the pattern set here: https://docs.rs/self-replace/1.5.0/self_replace/#implementation
            new_filename.push(OsStr::new(".previous.exe"));
            std::fs::rename(&old_filename, &new_filename)?;

            Some((new_filename, old_filename))
        } else {
            None
        };

        let path = if cfg!(windows) {
            "powershell"
        } else {
            installer_path.as_str()
        };
        let mut command = Cmd::new(path, "execute installer");
        if cfg!(windows) {
            // don't fall over on default security-policy windows machines
            // which require opt-in to execing powershell scripts.
            // This doesn't bypass proper organization-set policies.
            command.arg("-ExecutionPolicy").arg("ByPass");
            command.arg(&installer_path);
        }
        if self.print_installer_stdout {
            command.stdout(Stdio::inherit());
        }
        if self.print_installer_stderr {
            command.stderr(Stdio::inherit());
        }
        command.check(false);
        // On Windows, fixes a bug that occurs if the parent process is
        // PowerShell Core.
        // https://github.com/PowerShell/PowerShell/issues/18530
        command.env_remove("PSModulePath");
        let install_prefix = self.install_prefix_root()?;
        // Forces the generated installer to install to exactly this path,
        // regardless of how it's configured to install.
        command.env("CARGO_DIST_FORCE_INSTALL_DIR", &install_prefix);

        // Also set the app-specific name for this; in the future, the
        // CARGO_DIST_ version may be removed.
        let app_name = self.name.clone().unwrap_or_default();
        let app_name_env_var = app_name_to_env_var(&app_name);
        let app_specific_env_var = format!("{app_name_env_var}_INSTALL_DIR");
        command.env(app_specific_env_var, &install_prefix);

        // If the previous installation didn't modify the path, we shouldn't either
        if !self.modify_path {
            let app_specific_modify_path = format!("{app_name_env_var}_NO_MODIFY_PATH");
            command.env(app_specific_modify_path, "1");
        }

        let result = command.output();

        let failed;
        let stdout;
        let stderr;
        let statuscode;
        if let Ok(output) = &result {
            failed = !output.status.success();
            stdout = if output.stdout.is_empty() {
                None
            } else {
                Some(String::from_utf8_lossy(&output.stdout).to_string())
            };
            stderr = if output.stderr.is_empty() {
                None
            } else {
                Some(String::from_utf8_lossy(&output.stderr).to_string())
            };
            statuscode = output.status.code();
        } else {
            failed = true;
            stdout = None;
            stderr = None;
            statuscode = None;
        }

        if let Some((ourselves, old_path)) = to_restore {
            if failed {
                std::fs::rename(ourselves, old_path)?;
            } else {
                #[cfg(windows)]
                self_replace::self_delete_at(&ourselves)
                    .map_err(|_| AxoupdateError::CleanupFailed {})?;
            }
        }

        // Return the original AxoprocessError if we failed to launch
        // the command at all
        result?;

        // Otherwise return a more specific error with status code and
        // stdout/err. Note that this stdout/stderr will be None if the
        // caller requested us to print stdout/stderr to the terminal.
        if failed {
            return Err(AxoupdateError::InstallFailed {
                status: statuscode,
                stdout,
                stderr,
            });
        }

        let result = UpdateResult {
            old_version: self.current_version.clone(),
            new_version: release.version.clone(),
            new_version_tag: release.tag_name.to_owned(),
            install_prefix,
        };

        Ok(Some(result))
    }

    #[cfg(feature = "blocking")]
    /// Identical to Axoupdater::run(), but performed synchronously.
    pub fn run_sync(&mut self) -> AxoupdateResult<Option<UpdateResult>> {
        tokio::runtime::Builder::new_current_thread()
            .worker_threads(1)
            .max_blocking_threads(128)
            .enable_all()
            .build()
            .expect("Initializing tokio runtime failed")
            .block_on(self.run())
    }

    /// Queries for new releases and then returns the detected version.
    pub async fn query_new_version(&mut self) -> AxoupdateResult<Option<&Version>> {
        self.fetch_release().await?;

        if let Some(release) = &self.requested_release {
            Ok(Some(&release.version))
        } else {
            Ok(None)
        }
    }
}

fn get_app_name() -> Option<String> {
    if let Ok(name) = env::var("AXOUPDATER_APP_NAME") {
        Some(name)
    } else if let Some(path) = args().next() {
        Utf8PathBuf::from(&path)
            .file_name()
            .map(|s| s.strip_suffix(".exe").unwrap_or(s))
            .map(|s| s.strip_suffix("-update").unwrap_or(s))
            .map(|s| s.to_owned())
    } else {
        None
    }
}

/// Returns an environment variable-compatible version of the app name.
pub fn app_name_to_env_var(app_name: &str) -> String {
    app_name.to_ascii_uppercase().replace('-', "_")
}

fn root_without_bin(path: &Utf8PathBuf) -> Utf8PathBuf {
    if path.file_name() == Some("bin") {
        if let Some(parent) = path.parent() {
            return parent.to_path_buf();
        }
    }

    path.to_owned()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::AxoUpdater;

    #[test]
    fn test_installer_path_str() {
        let mut updater = AxoUpdater::new();
        updater.configure_installer_path("/tmp");
    }

    #[test]
    fn test_installer_path_string() {
        let mut updater = AxoUpdater::new();
        updater.configure_installer_path("/tmp".to_string());
    }

    #[test]
    fn test_installer_path() {
        let mut updater = AxoUpdater::new();
        let path = Path::new("/tmp");
        updater.configure_installer_path(&path.to_string_lossy());
    }

    #[test]
    fn test_installer_pathbuf() {
        let mut updater = AxoUpdater::new();
        let mut path = PathBuf::new();
        path.push("/tmp");
        updater.configure_installer_path(&path.to_string_lossy());
    }

    #[test]
    fn test_install_dir_path_str() {
        let mut updater = AxoUpdater::new();
        updater.set_install_dir("/tmp");
    }

    #[test]
    fn test_install_dir_path_string() {
        let mut updater = AxoUpdater::new();
        updater.set_install_dir("/tmp".to_string());
    }

    #[test]
    fn test_install_dir_path() {
        let mut updater = AxoUpdater::new();
        let path = Path::new("/tmp");
        updater.set_install_dir(&path.to_string_lossy());
    }

    #[test]
    fn test_install_dir_pathbuf() {
        let mut updater = AxoUpdater::new();
        let mut path = PathBuf::new();
        path.push("/tmp");
        updater.set_install_dir(&path.to_string_lossy());
    }
}
