//! Errors!

use miette::Diagnostic;
use thiserror::Error;

/// Gotta love a newtyped Result
pub type Result<T> = std::result::Result<T, AxoprocessError>;

/// An error from executing a Command
#[derive(Debug, Error, Diagnostic)]
pub enum AxoprocessError {
    /// The command fundamentally failed to execute (usually means it didn't exist)
    #[error("failed to {summary}")]
    Exec {
        /// Summary of what the Command was trying to do
        summary: String,
        /// What failed
        #[source]
        cause: std::io::Error,
    },
    /// The command ran but signaled some kind of error condition
    /// (assuming the exit code is used for that)
    #[error("failed to {summary} (status: {status})")]
    Status {
        /// Summary of what the Command was trying to do
        summary: String,
        /// What status the Command returned
        status: std::process::ExitStatus,
    },
}
