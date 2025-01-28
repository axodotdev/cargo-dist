use axoasset::toml_edit;
use crate::config::v1::layer::{BoolOr, BoolOrOptExt};
use crate::config::SystemDependencies;
use super::helpers::*;

pub fn apply(
    builds_table: &mut toml_edit::Table,
    system_dependencies: Option<&SystemDependencies>,
) {
    let Some(system_dependencies) = system_dependencies else {
        // Nothing to do.
        return;
    };

    // TODO(migration): implement this
}
