//! Errors!
//!
//! This module is kind of pointless and stubbed out right now,
//! because the crate is currently opting for a "typeless" approach
//! (where everything gets folded into miette::Report right away).
//!
//! If we ever change this decision, this will be a lot more important!

use axoproject::errors::AxoprojectError;
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
}
