use crate::config::v0::V0DistConfig;
use crate::config::v1::DistConfig;
use crate::{config, errors::DistResult};
use axoasset::toml;

// A purely-functional subset of `do_migrate_from_v0()`:
// takes a DistMetadata (v0) and returns an equivalent DistConfig (v1)
// without touching files on disk.
//
// This could theoretically have unit tests written for it, but we would
// need to make every piece of DistConfig derive/impl PartialEq, which is
// a lot. -duckinator
fn migrate_from_v0_dist_config(old_config: V0DistConfig) -> DistConfig {
    let dist = old_config.dist.map(|dist| dist.to_toml_layer(true));
    let workspace = old_config.workspace;
    let package = old_config.package;

    config::v1::DistConfig {
        dist,
        workspace,
        package,
    }
}

pub fn do_migrate_from_v0() -> DistResult<()> {
    let workspaces = config::get_project()?;
    let root_workspace = workspaces.root_workspace();
    let manifest_path = &root_workspace.manifest_path;

    if config::v1::load(manifest_path).is_ok() {
        // We're already on a V1 config, no need to migrate!
        return Ok(());
    }

    // Load in the root workspace toml to edit and write back
    let Ok(old_config) = config::v0::load(manifest_path) else {
        // We don't have a valid v0 _or_ v1 config. No migration can be done.
        // It feels weird to return Ok(()) here, but I think it's right?
        return Ok(());
    };

    let config = migrate_from_v0_dist_config(old_config);

    let workspace_toml_text = toml::to_string(&config)?;

    // Write new config file.
    axoasset::LocalAsset::write_new(&workspace_toml_text, manifest_path)?;

    Ok(())
}
