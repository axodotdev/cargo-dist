//! Errors!

use camino::Utf8PathBuf;
use miette::Diagnostic;
use thiserror::Error;

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
        details: toml_edit::TomlError,
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
}
