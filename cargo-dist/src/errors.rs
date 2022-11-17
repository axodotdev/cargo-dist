use miette::Diagnostic;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, miette::Report>;

#[derive(Debug, Error, Diagnostic)]
pub enum CargoDistError {}
