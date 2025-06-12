//! Errors

use miette::Diagnostic;
use thiserror::Error;

/// An alias for Result<T, AxoupdateError>
pub type AxoupdateResult<T> = std::result::Result<T, AxoupdateError>;

/// An enum representing all of this crate's errors
#[derive(Debug, Error, Diagnostic)]
pub enum AxoupdateError {
    /// Passed through from Reqwest
    #[error(transparent)]
    Reqwest(#[from] axoasset::reqwest::Error),

    /// Passed through from std::io::Error
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Passed through from Camino
    #[error(transparent)]
    CaminoPathBuf(#[from] camino::FromPathBufError),

    /// Passed through from homedir
    #[error(transparent)]
    Homedir(#[from] homedir::GetHomeError),

    /// Passed through from axoasset
    #[error(transparent)]
    Axoasset(#[from] axoasset::AxoassetError),

    /// Passed through from axoprocess
    #[error(transparent)]
    Axoprocess(#[from] axoprocess::AxoprocessError),

    /// Passed through from axotag
    #[error(transparent)]
    Axotag(#[from] axotag::errors::TagError),

    /// Passed through from gazenot
    #[cfg(feature = "axo_releases")]
    #[error(transparent)]
    Gazenot(#[from] gazenot::error::GazenotError),

    /// Failed to parse a version
    #[error(transparent)]
    Version(#[from] axotag::semver::Error),

    /// Failed to parse a URL
    #[error(transparent)]
    UrlParseError(#[from] url::ParseError),

    /// Failure when converting a PathBuf to a Utf8PathBuf
    #[error("An internal error occurred when decoding path `{:?}' to utf8", path)]
    #[diagnostic(help("This probably isn't your fault; please open an issue!"))]
    CaminoConversionFailed {
        /// The path which Camino failed to convert
        path: std::path::PathBuf,
    },

    /// Indicates that the only updates available are located at a source
    /// this crate isn't configured to support. This is returned if the
    /// appropriate source is disabled via features.
    #[error("Release is located on backend {backend}, but it's not enabled")]
    #[diagnostic(help("This probably isn't your fault; please open an issue!"))]
    BackendDisabled {
        /// The name of the backend
        backend: String,
    },

    /// Indicates that axoupdater wasn't able to determine the config file path
    /// for this app. This path is where install receipts are located.
    #[error("Unable to determine config file path for app {app_name}!")]
    #[diagnostic(help("This probably isn't your fault; please open an issue!"))]
    ConfigFetchFailed {
        /// This app's name
        app_name: String,
    },

    /// Indicates that the install receipt for this app couldn't be read.
    #[error("Unable to read installation information for app {app_name}.")]
    #[diagnostic(help("This probably isn't your fault; please open an issue!"))]
    ReceiptLoadFailed {
        /// This app's name
        app_name: String,
    },

    /// Not a generic receipt load failure, but the receipt itself doesn't exist.
    #[error("Unable to load receipt for app {app_name}")]
    #[diagnostic(help(
        "This may indicate that this installation of {app_name} was installed via a method that's not eligible for upgrades."
    ))]
    NoReceipt {
        /// This app's name
        app_name: String,
    },

    /// Indicates that this app's name couldn't be determined when trying
    /// to autodetect it.
    #[error("Unable to determine the name of the app to update")]
    #[diagnostic(help("This probably isn't your fault; please open an issue!"))]
    NoAppName {},

    /// Indicates that no app name was specified before the updater process began.
    #[error("No app name was configured for this updater")]
    #[diagnostic(help("This isn't your fault; please open an issue!"))]
    NoAppNamePassed {},

    /// Indicates that the home directory couldn't be determined.
    #[error("Unable to fetch your home directory")]
    #[diagnostic(help("This may not be your fault; please open an issue!"))]
    NoHome {},

    /// Indicates that no installer is available for this OS when looking up
    /// the latest release.
    #[error("Unable to find an installer for your OS")]
    NoInstallerForPackage {},

    /// Indicates that no stable releases exist for the app being updated.
    #[error("There are no stable releases available for {app_name}")]
    NoStableReleases {
        /// This app's name
        app_name: String,
    },

    /// Indicates that no releases exist for this app at all.
    #[error("No releases were found for the app {app_name} in workspace {name}")]
    ReleaseNotFound {
        /// The workspace's name
        name: String,
        /// This app's name
        app_name: String,
    },

    /// Indicates that no releases exist for this app at all.
    #[error("The version {version} was not found for the app {app_name} in workspace {name}")]
    VersionNotFound {
        /// The workspace's name
        name: String,
        /// This app's name
        app_name: String,
        /// The version we failed to find
        version: String,
    },

    /// This error catches an edge case where the axoupdater executable was run
    /// under its default filename, "axoupdater", instead of being installed
    /// under an app-specific name.
    #[error("App name calculated as `axoupdater'")]
    #[diagnostic(help(
        "This probably isn't what you meant to update; was the updater installed correctly?"
    ))]
    UpdateSelf {},

    /// Indicates that a mandatory config field wasn't specified before the
    /// update process ran.
    #[error("The updater isn't properly configured")]
    #[diagnostic(help("Missing configuration value for {}", missing_field))]
    NotConfigured {
        /// The name of the missing field
        missing_field: String,
    },

    /// Indicates the installation failed for some reason we're not sure of
    #[error("The installation failed. Output from the installer: {}\n{}", stdout.clone().unwrap_or_default(), stderr.clone().unwrap_or_default())]
    InstallFailed {
        /// The status code from the underlying process, if any
        status: Option<i32>,
        /// The stdout, decoded to UTF-8. This will be None if it was piped
        /// to the terminal when running the installer.
        stdout: Option<String>,
        /// The stderr, decoded to UTF-8. This will be None if it was piped
        /// to the terminal when running the installer.
        stderr: Option<String>,
    },

    /// self_replace/self_delete failed
    #[error(
        "Cleaning up the previous version failed; a copy of the old version has been left behind."
    )]
    #[diagnostic(help("This probably isn't your fault; please open an issue at https://github.com/axodotdev/axoupdater!"))]
    CleanupFailed {},

    /// User passed conflicting GitHub API environment variables
    #[error("Both {ghe_env_var} and {github_env_var} have been set in the environment")]
    #[diagnostic(help("These variables are mutually exclusive; please pick one."))]
    MultipleGitHubAPIs {
        /// The GitHub Enterprise env var
        ghe_env_var: String,
        /// The GitHub env var
        github_env_var: String,
    },

    /// Couldn't parse the text domain (could be an IP, etc.)
    #[error("Unable to parse the domain from the passed url: {url}")]
    #[diagnostic(help("The {env_var} variable only takes domains. If you're using an IP, we recommend the GitHub Enterprise-style variable: {ghe_env_var}"))]
    GitHubDomainParseError {
        /// The GitHub env var
        env_var: String,
        /// The GitHub Enterprise env var
        ghe_env_var: String,
        /// The supplied URL
        url: String,
    },
}
