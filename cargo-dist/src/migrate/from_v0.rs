use crate::{config, errors::DistResult};
use axoasset::toml;

pub fn do_migrate_from_v0() -> DistResult<()> {
    let workspaces = config::get_project()?;
    let root_workspace = workspaces.root_workspace();
    let manifest_path = &root_workspace.manifest_path;

    if config::load(manifest_path).is_ok() {
        // We're already on a V1 config, no need to migrate!
        return Ok(());
    }

    // Load in the root workspace toml to edit and write back
    let Ok(old_config) = config::v0::load(manifest_path) else {
        // We don't have a valid v0 _or_ v1 config. No migration can be done.
        // It feels weird to return Ok(()) here, but I think it's right?
        return Ok(());
    };

    let Some(dist_metadata) = &old_config.dist else {
        // We don't have a valid v0 config. No migration can be done.
        return Ok(());
    };

    let dist = Some(dist_metadata.to_toml_layer(true));

    let workspace = old_config.workspace;
    let package = None;

    let config = config::v1::DistConfig {
        dist,
        workspace,
        package,
    };

    let workspace_toml_text = toml::to_string(&config)?;

    // Write new config file.
    axoasset::LocalAsset::write_new(&workspace_toml_text, manifest_path)?;

    Ok(())
}
