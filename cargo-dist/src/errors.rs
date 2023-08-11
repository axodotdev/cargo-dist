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
    #[help("these packages customized either features, no-default-features, or all-features: {packages:?}")]
    PreciseImpossible {
        /// names of problem packages
        packages: Vec<String>,
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
