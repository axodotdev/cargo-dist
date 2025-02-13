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

    // TODO(migration): make sure all of these are handled
    /*

    apply_optional_value(
        table,
        "create-release",
        "# Whether dist should create a Github Release or use an existing draft\n",
        *create_release,
    );

    apply_optional_value(
        table,
        "github-release",
        "# Which phase dist should use to create the GitHub release\n",
        github_release.as_ref().map(|a| a.to_string()),
    );

    apply_optional_value(
        table,
        "github-releases-repo",
        "# Publish GitHub Releases to this repo instead\n",
        github_releases_repo.as_ref().map(|a| a.to_string()),
    );

    apply_optional_value(
        table,
        "github-releases-submodule-path",
        "# Read the commit to be tagged from the submodule at this path\n",
        github_releases_submodule_path
            .as_ref()
            .map(|a| a.to_string()),
    );

    apply_string_list(
        table,
        "local-artifacts-jobs",
        "# Local artifacts jobs to run in CI\n",
        local_artifacts_jobs.as_ref(),
    );

    apply_string_list(
        table,
        "global-artifacts-jobs",
        "# Global artifacts jobs to run in CI\n",
        global_artifacts_jobs.as_ref(),
    );

    apply_string_list(
        table,
        "host-jobs",
        "# Host jobs to run in CI\n",
        host_jobs.as_ref(),
    );

    apply_string_list(
        table,
        "publish-jobs",
        "# Publish jobs to run in CI\n",
        publish_jobs.as_ref(),
    );

    apply_string_list(
        table,
        "post-announce-jobs",
        "# Post-announce jobs to run in CI\n",
        post_announce_jobs.as_ref(),
    );

    apply_optional_value(
        table,
        "publish-prereleases",
        "# Whether to publish prereleases to package managers\n",
        *publish_prereleases,
    );

    apply_optional_value(
        table,
        "force-latest",
        "# Always mark releases as latest, ignoring semver semantics\n",
        *force_latest,
    );

    apply_optional_value(
        table,
        "github-attestations",
        "# Whether to enable GitHub Attestations\n",
        *github_attestations,
    );

    apply_string_or_list(
        table,
        "hosting",
        "# Where to host releases\n",
        hosting.as_ref(),
    );

    apply_optional_value(
        table,
        "install-updater",
        "# Whether to install an updater program\n",
        *install_updater,
    );

    apply_optional_value(
        table,
        "always-use-latest-updater",
        "# Whether to always use the latest updater instead of a specific known-good version\n",
        *always_use_latest_updater,
    );

    apply_optional_value(
        table,
        "display",
        "# Whether to display this app's installers/artifacts in release bodies\n",
        *display,
    );

    apply_optional_value(
        table,
        "display-name",
        "# Custom display name to use for this app in release bodies\n",
        display_name.as_ref(),
    );



    */

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
