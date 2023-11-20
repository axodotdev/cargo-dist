//! Errors!
//!
//! This module is kind of pointless and stubbed out right now,
//! because the crate is currently opting for a "typeless" approach
//! (where everything gets folded into miette::Report right away).
//!
//! If we ever change this decision, this will be a lot more important!

use axoproject::errors::AxoprojectError;
use camino::Utf8PathBuf;
use miette::Diagnostic;
use thiserror::Error;

/// An alias for the common Result type of this crate
pub type Result<T> = std::result::Result<T, miette::Report>;
/// An alias for the NEW Result type for this crate (undergoing migration)
pub type DistResult<T> = std::result::Result<T, DistError>;

/// Errors cargo-dist can have
#[derive(Debug, Error, Diagnostic)]
pub enum DistError {
    /// random i/o error
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// random axoasset error
    #[error(transparent)]
    #[diagnostic(transparent)]
    Asset(#[from] axoasset::AxoassetError),

    /// random gazenot error
    #[error(transparent)]
    #[diagnostic(transparent)]
    Gazenot(#[from] gazenot::error::GazenotError),

    /// random string error
    #[error(transparent)]
    FromUtf8Error(#[from] std::string::FromUtf8Error),

    /// A problem with a jinja template, which is always a cargo-dist bug
    #[error("Failed to render template")]
    #[diagnostic(help("this is a bug in cargo-dist, let us know and we'll fix it: https://github.com/axodotdev/cargo-dist/issues/new"))]
    Jinja {
        /// The SourceFile we were try to parse
        #[source_code]
        source: String,
        /// The range the error was found on
        #[label]
        span: Option<miette::SourceSpan>,
        /// Details of the error
        #[source]
        details: minijinja::Error,
    },

    /// Error from (cargo-)wix
    #[error("WiX returned an error while building {msi}")]
    Wix {
        /// The msi we were trying to build
        msi: String,
        /// The underyling wix error
        #[source]
        details: wix::Error,
    },

    /// Error from (cargo-)wix init
    #[error("Couldn't generate main.wxs for {package}'s msi installer")]
    WixInit {
        /// The package
        package: String,
        /// The underlying wix error
        #[source]
        details: wix::Error,
    },

    /// Error parsing metadata in Cargo.toml (json because it's from cargo-metadata)
    #[error("Malformed metadata.dist in {manifest_path}")]
    CargoTomlParse {
        /// path to file
        manifest_path: Utf8PathBuf,
        /// Inner error
        #[source]
        cause: serde_json::Error,
    },

    /// User declined to update cargo-dist, refuse to make progress
    #[error(
        "to update your cargo-dist config you must use the version your project is configured for"
    )]
    #[diagnostic(help(
        "you're running {running_version} but the project is configured for {project_version}"
    ))]
    NoUpdateVersion {
        /// Version the config had
        project_version: semver::Version,
        /// Version they're running
        running_version: semver::Version,
    },

    /// User tried to enable Github CI support but had inconsistent urls for the repo
    #[error("Github CI support requires your crates to agree on the URL of your repository")]
    CantEnableGithubUrlInconsistent {
        /// inner error that caught this
        #[diagnostic_source]
        inner: AxoprojectError,
    },
    /// User tried to enable Github CI support but no url for the repo
    #[error("Github CI support requires you to specify the URL of your repository")]
    #[diagnostic(help(r#"Set the repository = "https://github.com/..." key in your Cargo.toml"#))]
    CantEnableGithubNoUrl,
    /// User declined to force tar.gz with npm
    #[error("Cannot enable npm support without forcing artifacts to be .tar.gz")]
    MustEnableTarGz,

    /// Completely unknown format to install-path
    ///
    /// NOTE: we can't use `diagnostic(help)` here because this will get crammed into
    /// a serde_json error, reducing it to a String. So we inline the help!
    #[error(r#"install-path = "{path}" has an unknown format (it can either be "CARGO_HOME", "~/subdir/", or "$ENV_VAR/subdir/")"#)]
    InstallPathInvalid {
        /// The full value passed to install-path
        path: String,
    },

    /// Being pedantic about the env-var mode of install-path to be consistent
    ///
    /// NOTE: we can't use `diagnostic(help)` here because this will get crammed into
    /// a serde_json error, reducing it to a String. So we inline the help!
    #[error(r#"install-path = "{path}" is missing a subdirectory (add a trailing slash if you want no subdirectory)"#)]
    InstallPathEnvSlash {
        /// The full value passed to install-path
        path: String,
    },

    /// Being pedantic about the home mode of install-path to be consistent
    ///
    /// NOTE: we can't use `diagnostic(help)` here because this will get crammed into
    /// a serde_json error, reducing it to a String. So we inline the help!
    #[error(r#"install-path = "{path}" is missing a subdirectory (installing directly to home isn't allowed)"#)]
    InstallPathHomeSubdir {
        /// The full value passed to install-path
        path: String,
    },

    /// Use explicitly requested workspace builds, but had packages with custom feature settings
    #[error("precise-builds = false was set, but some packages have custom build features, making it impossible")]
    #[diagnostic(help("these packages customized either features, no-default-features, or all-features: {packages:?}"))]
    PreciseImpossible {
        /// names of problem packages
        packages: Vec<String>,
    },

    /// parse_tag concluded there was nothing to release
    #[error("This workspace doesn't have anything for cargo-dist to Release!")]
    NothingToRelease {
        /// full help printout (very dynamic)
        #[help]
        help: String,
    },

    /// parse_tag concluded there are too many unrelated things for a single tag
    #[error("There are too many unrelated apps in your workspace to coherently Announce!")]
    TooManyUnrelatedApps {
        /// full help printout (very dynamic)
        #[help]
        help: String,
    },
    /// Not an error; indicates that a file's contents differ via --check
    #[error("{} has out of date contents and needs to be regenerated:\n{diff}", file.origin_path())]
    #[diagnostic(help("run 'cargo dist init' to update the file or set 'allow-dirty' in Cargo.toml to ignore out of date contents"))]
    CheckFileMismatch {
        /// The file whose contents differ
        file: axoasset::SourceFile,
        /// The diff
        diff: String,
    },

    /// `cargo dist generate` was passed an explicit GenerateMode but the config in their Cargo.toml
    /// has that mode set to allow-dirty, a contradiction!
    #[error(
        "'{generate_mode}' is marked as allow-dirty in your cargo-dist config, refusing to run"
    )]
    ContradictoryGenerateModes {
        /// The problematic mode
        generate_mode: crate::config::GenerateMode,
    },
    /// msi with too many packages
    #[error("{artifact_name} depends on multiple packages, which isn't yet supported")]
    #[diagnostic(help("depends on {spec1} and {spec2}"))]
    MultiPackageMsi {
        /// Name of the msi
        artifact_name: String,
        /// One of the pacakges
        spec1: String,
        /// A different package
        spec2: String,
    },
    /// msi with too few packages
    #[error("{artifact_name} has no binaries")]
    #[diagnostic(help("This should be impossible, you did nothing wrong, please file an issue!"))]
    NoPackageMsi {
        /// Name of the msi
        artifact_name: String,
    },
    /// These GUIDs for msi's are required and enforced by `cargo dist generate --check`
    #[error("missing WiX GUIDs in {manifest_path}: {keys:?}")]
    #[diagnostic(help("run 'cargo dist init' to generate them"))]
    MissingWixGuids {
        /// The Cargo.toml missing them
        manifest_path: Utf8PathBuf,
        /// The missing keys
        keys: &'static [&'static str],
    },
    /// unrecognized style
    #[error("{style} is not a recognized value")]
    #[diagnostic(help("Jobs that do not come with cargo-dist should be prefixed with ./"))]
    UnrecognizedStyle {
        /// Name of the msi
        style: String,
    },
    /// Linkage report can't be run for this combination of OS and target
    #[error("unable to run linkage report for {target} on {host}")]
    LinkageCheckInvalidOS {
        /// The OS the check was run on
        host: String,
        /// The OS being checked
        target: String,
    },
    /// Linkage report can't be run for this target
    #[error("unable to run linkage report for this type of binary")]
    LinkageCheckUnsupportedBinary {},

    /// random i/o error
    #[error(transparent)]
    Goblin(#[from] goblin::error::Error),

    /// random camino conversion error
    #[error(transparent)]
    FromPathBufError(#[from] camino::FromPathBufError),

    /// Error parsing a string containing an environment variable
    /// in VAR=value syntax
    #[error("Unable to parse environment variable as a key/value pair: {line}")]
    #[diagnostic(help("This should be impossible, you did nothing wrong, please file an issue!"))]
    EnvParseError {
        /// The line of text that couldn't be parsed
        line: String,
    },

    /// random dialoguer error
    #[error(transparent)]
    DialoguerError(#[from] dialoguer::Error),

    /// random axotag error
    #[error(transparent)]
    AxotagError(#[from] axotag::errors::TagError),

    /// No workspace found from axoproject
    #[error("No workspace found; either your project doesn't have a Cargo.toml/dist.toml, or we couldn't read it")]
    ProjectMissing {
        /// axoproject's error for the unidentified project
        #[related]
        sources: Vec<AxoprojectError>,
    },
}

impl From<minijinja::Error> for DistError {
    fn from(details: minijinja::Error) -> Self {
        let source: String = details.template_source().unwrap_or_default().to_owned();
        let span = details.range().map(|r| r.into());
        DistError::Jinja {
            source,
            span,
            details,
        }
    }
}
