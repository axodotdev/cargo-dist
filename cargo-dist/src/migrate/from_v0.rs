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
    let V0DistConfig {
        dist,
        workspace,
        package,
    } = old_config;


    let current_dist_version: semver::Version = std::env!("CARGO_PKG_VERSION").parse().unwrap();
    let version_one = semver::Version::new(1, 0, 0);

    let dist = dist
        // Take Some(v0::DistMetadata) and turn it into Some(v1::TomlLayer).
        .map(|dist| dist.to_toml_layer(true))
        .map(|mut dist| {
            // If dist_version is pinned to <1.0.0, set it to the current version
            if dist.dist_version < Some(version_one) {
                dist.dist_version = Some(current_dist_version);
            }

            // Change config_version from V0 to V1, since we're migrating to it.
            dist.config_version = config::ConfigVersion::V1;
            dist
        });

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
