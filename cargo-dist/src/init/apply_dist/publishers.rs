use super::helpers::*;
use crate::config::v1::layer::{BoolOr, BoolOrOptExt};
use crate::config::v1::publishers::PublisherLayer;
use axoasset::toml_edit;

pub fn apply(table: &mut toml_edit::Table, publishers: &Option<PublisherLayer>) {
    let Some(publishers_table) = table.get_mut("publishers") else {
        return;
    };
    let toml_edit::Item::Table(publishers_table) = publishers_table else {
        panic!("Expected [dist.publishers] to be a table");
    };

    // TODO(migration): implement this

    // Finalize the table
    publishers_table
        .decor_mut()
        .set_prefix("\n# Publisher configuration for dist\n");
}
