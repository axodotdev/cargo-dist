//! Errors!
//!
//! This module is kind of pointless and stubbed out right now,
//! because the crate is currently opting for a "typeless" approach
//! (where everything gets folded into miette::Report right away).
//!
//! If we ever change this decision, this will be a lot more important!

/// An alias for the common Result type of this crate
pub type Result<T> = std::result::Result<T, miette::Report>;
