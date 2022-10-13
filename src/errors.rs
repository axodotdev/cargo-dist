use miette::Diagnostic;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, CargoDistError>;

#[derive(Debug, Error, Diagnostic)]
#[error("cargo-dist couldn't do that")]
pub struct CargoDistError {
    // TODO: at some context fields / miette stuff
}
