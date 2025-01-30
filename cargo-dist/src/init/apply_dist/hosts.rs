use super::helpers::*;
use crate::config::v1::hosts::HostLayer;
use crate::config::v1::layer::{BoolOr, BoolOrOptExt};
use axoasset::toml_edit;

pub fn apply(table: &mut toml_edit::Table, hosts: &Option<HostLayer>) {
    let Some(hosts_table) = table.get_mut("hosts") else {
        // Nothing to do.
        return;
    };
    let toml_edit::Item::Table(hosts_table) = hosts_table else {
        panic!("Expected [dist.hosts] to be a table");
    };

    // TODO(migration): implement this

    // Finalize the table
    hosts_table
        .decor_mut()
        .set_prefix("\n# Hosting configuration for dist\n");
}
