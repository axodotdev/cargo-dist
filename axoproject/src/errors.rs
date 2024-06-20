//! Errors!

use camino::Utf8PathBuf;
use miette::Diagnostic;
use thiserror::Error;

use crate::Version;

/// A Result returned by Axoproject
pub type Result<T> = std::result::Result<T, AxoprojectError>;

/// An Error/Diagnostic returned by Axoproject
#[derive(Debug, Error, Diagnostic)]
#[non_exhaustive]
pub enum AxoprojectError {
    /// Axoasset returned an error (I/O error)
    #[error(transparent)]
    #[diagnostic(transparent)]
    Axoasset(#[from] axoasset::AxoassetError),

    /// An error occured in guppy/cargo-metadata when trying to find a cargo project
    #[cfg(feature = "cargo-projects")]
    #[error(transparent)]
    CargoMetadata(#[from] guppy::Error),

    /// An error occured in parse_changelog
    #[error(transparent)]
    ParseChangelog(#[from] parse_changelog::Error),

    /// An error parsing a Cargo.toml
    #[cfg(feature = "cargo-projects")]
    #[error("couldn't read Cargo.toml")]
    ParseCargoToml {
        /// The toml file
        #[source_code]
        source: axoasset::SourceFile,
        /// Where we found an issue
        #[label]
        span: Option<miette::SourceSpan>,
        /// The underlying issue
        #[source]
        details: axoasset::toml_edit::TomlError,
    },

    /// We found a package.json but it didn't have "name" set
    #[cfg(feature = "npm-projects")]
    #[error("your package doesn't have a name: {manifest}")]
    #[diagnostic(help("is it a workspace? We don't support that yet."))]
    NamelessNpmPackage {
        /// path to the package.json
        manifest: Utf8PathBuf,
    },

    /// We tried to get the bins from a package.json but something went wrong
    #[cfg(feature = "npm-projects")]
    #[error("Failed to read the binaries from your package.json: {manifest_path}")]
    BuildInfoParse {
        /// Path to the package.json
        manifest_path: Utf8PathBuf,
        /// underlying error
        #[source]
        details: std::io::Error,
    },

    /// Your workspace gave several different values for "repository"
    #[error("your workspace has inconsistent values for 'repository', refusing to select one:\n  {file1}: {url1}\n  {file2}: {url2}")]
    #[diagnostic(severity("warning"))]
    InconsistentRepositoryKey {
        /// Path to the first manifest
        file1: Utf8PathBuf,
        /// value the first manifest had set
        url1: String,
        /// Path to the second manifest
        file2: Utf8PathBuf,
        /// value the second manifest had set
        url2: String,
    },

    /// An error that occured while trying to find READMEs and whatnot in your project dir
    #[error("couldn't search for files in {dir}")]
    AutoIncludeSearch {
        /// path to the dir we were searching
        dir: Utf8PathBuf,
        /// underlying error
        #[source]
        details: std::io::Error,
    },

    /// An error that occurred while trying to parse a repository string
    #[error("Your repository URL {url} couldn't be parsed.")]
    #[diagnostic(help("only git-compatible URLs are supported."))]
    UnknownRepoStyle {
        /// URL to the repository
        url: String,
    },

    /// An error that occurred because a repository string could not be parsed for a specific reason
    #[error("failed to parse your repo, current config has repo as: {repo}")]
    #[diagnostic(help("We found a repo url but we had trouble parsing it. Please make sure it's entered correctly. This may be an error, and if so you should file an issue."))]
    RepoParseError {
        /// URL to the repository
        repo: String,
    },

    /// An error that occurred when parsing a repository string
    #[error(transparent)]
    UrlParseError(#[from] url::ParseError),

    /// An error returned when a non-GitHub URL is parsed
    #[error("Your repository URL {url} couldn't be parsed.")]
    #[diagnostic(help("Only GitHub URLs are supported at the moment."))]
    NotGitHubError {
        /// URL to the repository
        url: String,
    },

    /// We searched a changelog file but found no result
    #[error("couldn't find a suitable changelog entry for {version} in {path}")]
    ChangelogVersionNotFound {
        /// Path of the file
        path: Utf8PathBuf,
        /// Version we were looking for
        version: Version,
    },
}

/// Errors related to finding the project
#[derive(Debug, Error, Diagnostic)]
pub enum ProjectError {
    /// No workspace found from axoproject
    #[error("No workspace found; either your project doesn't have a Cargo.toml/dist.toml, or we couldn't read it")]
    ProjectMissing {
        /// axoproject's error for the unidentified project
        #[related]
        sources: Vec<AxoprojectError>,
    },

    /// Found a workspace but it was malformed
    #[error("We encountered an issue trying to read your workspace")]
    ProjectBroken {
        /// The cause
        #[source]
        cause: AxoprojectError,
    },
}
