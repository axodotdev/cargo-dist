use crate::config::v1::installers::InstallerLayer;
use crate::config::v1::layer::BoolOrOptExt;
use crate::config::v1::TomlLayer;
use crate::config::InstallPathStrategy;
use crate::METADATA_DIST;
use axoasset::toml_edit;

mod artifacts;
mod builds;
mod ci;
mod helpers;
mod hosts;
mod installers;
mod publishers;

use helpers::*;

/// Update a workspace toml-edit document with the current DistMetadata value
pub fn apply_dist_to_workspace_toml(workspace_toml: &mut toml_edit::DocumentMut, meta: &TomlLayer) {
    let metadata = workspace_toml.as_item_mut();
    apply_dist_to_metadata(metadata, meta);
}

/// Ensure [dist] has the given values
pub fn apply_dist_to_metadata(metadata: &mut toml_edit::Item, meta: &TomlLayer) {
    let dist_metadata = &mut metadata[METADATA_DIST];

    // If there's no table, make one
    if !dist_metadata.is_table() {
        *dist_metadata = toml_edit::table();
    }

    // Apply formatted/commented values
    let table = dist_metadata.as_table_mut().unwrap();

    // This is intentionally written awkwardly to make you update this
    let TomlLayer {
        config_version,
        dist_version,
        dist_url_override,
        dist,
        allow_dirty,
        targets,
        artifacts,
        builds,
        ci,
        hosts,
        installers,
        publishers,
    } = &meta;

    let installers = &Some(apply_default_install_path(installers));

    apply_optional_value(
        table,
        "config-version",
        "# The configuration version to use (valid options: 1)\n",
        Some(config_version.to_string()),
    );

    apply_optional_value(
        table,
        "dist-version",
        "# The preferred dist version to use in CI (Cargo.toml SemVer syntax)\n",
        dist_version.as_ref().map(|v| v.to_string()),
    );

    apply_optional_value(
        table,
        "dist-url-override",
        "# A URL to use to install `cargo-dist` (with the installer script)\n",
        dist_url_override.as_ref().map(|v| v.to_string()),
    );

    apply_optional_value(
        table,
        "dist",
        "# Whether the package should be distributed/built by dist (defaults to true)\n",
        *dist,
    );

    apply_string_list(
        table,
        "allow-dirty",
        "# Skip checking whether the specified configuration files are up to date\n",
        allow_dirty.as_ref(),
    );

    apply_string_list(
        table,
        "targets",
        "# Target platforms to build apps for (Rust target-triple syntax)\n",
        targets.as_ref(),
    );

    artifacts::apply(table, artifacts);
    builds::apply(table, builds);
    ci::apply(table, ci);
    hosts::apply(table, hosts);
    installers::apply(table, installers);
    publishers::apply(table, publishers);

    // Finalize the table
    table.decor_mut().set_prefix("\n# Config for 'dist'\n");
}

fn apply_default_install_path(installers: &Option<InstallerLayer>) -> InstallerLayer {
    let mut installers = installers.clone().unwrap_or_default();

    // Forcibly inline the default install_path if not specified,
    // and if we've specified a shell or powershell installer
    let install_path = if installers.common.install_path.is_none()
        && !(installers.shell.is_none_or_false() || installers.powershell.is_none_or_false())
    {
        Some(InstallPathStrategy::default_list())
    } else {
        installers.common.install_path.clone()
    };

    installers.common.install_path = install_path;
    installers
}
