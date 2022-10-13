use errors::*;
use serde::{Deserialize, Serialize};

pub mod errors;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub cats_are_cute: bool,
}

pub fn some_op() -> Result<Report> {
    let report = Report {
        cats_are_cute: true,
    };

    Ok(report)
}
