use miette::Diagnostic;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, CargoDistError>;

#[derive(Debug, Error, Diagnostic)]
pub enum CargoDistError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}
