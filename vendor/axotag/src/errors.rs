//! This module contains axotag's custom errors.

use miette::Diagnostic;
use thiserror::Error;

/// An alias for the Result type for this crate
pub type TagResult<T> = std::result::Result<T, TagError>;

/// Errors axotag can have
#[derive(Debug, Error, Diagnostic)]
pub enum TagError {
    /// parse_tag concluded that versions didn't line up
    #[error("The provided announcement tag ({tag}) claims we're releasing {package_name} {tag_version}, but that package is version {real_version}")]
    ContradictoryTagVersion {
        /// The full tag
        tag: String,
        /// The package name
        package_name: String,
        /// The version the tag claimed
        tag_version: semver::Version,
        /// The version the package actually has
        real_version: semver::Version,
    },

    /// parse_tag couldn't parse the version component at all
    #[error("Couldn't parse the version from the provided announcement tag ({tag})")]
    TagVersionParse {
        /// the full tag
        tag: String,
        /// parse error
        #[source]
        details: semver::Error,
    },

    /// parse_tag couldn't make sense of the --tag provided
    #[error("The provided announcement tag ({tag}) didn't match any Package or Version")]
    NoTagMatch {
        /// The --tag
        tag: String,
    },
}
