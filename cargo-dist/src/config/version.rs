//! For determining which configuration version we're using.

use axoasset::SourceFile;
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::DistResult;

/// Represents all known configuration versions.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum ConfigVersion {
    /// The original legacy configuration formats are all lumped in as V0.
    #[serde(rename = "0")]
    V0 = 0,
    /// The current configuration format.
    #[serde(rename = "1")]
    #[default]
    V1 = 1,
}

impl std::fmt::Display for ConfigVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.clone() as i64)
    }
}

// Extremely minimal struct designed to differentiate between config versions.
// V0 does not have the `config-version` field, so will fail to parse.
// V1+ should have it, so will parse, and contain a `config_version` field.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct FauxDistTable {
    #[allow(dead_code)]
    config_version: ConfigVersion,
}

#[derive(Deserialize)]
struct FauxConfig {
    #[allow(dead_code)]
    dist: FauxDistTable,
}

/// Return the config version used for the root workspace.
pub fn get_version() -> DistResult<ConfigVersion> {
    let workspaces = super::get_project()?;
    let root_workspace = workspaces.root_workspace();

    get_version_for_manifest(root_workspace.manifest_path.to_owned())
}

/// Given a path to a dist manifest (e.g. `dist-workspace.toml`), returns
/// the config version being used.
pub fn get_version_for_manifest(dist_manifest_path: Utf8PathBuf) -> DistResult<ConfigVersion> {
    if dist_manifest_path.file_name() != Some("dist-workspace.toml") {
        // If the manifest is in Cargo.toml or dist.toml, we're
        // definitely using a v0 config.
        return Ok(ConfigVersion::V0);
    }

    let src = SourceFile::load_local(&dist_manifest_path)?;

    let Ok(config) = src.deserialize_toml::<FauxConfig>() else {
        // If we could load it, but can't parse it, it's likely v0.
        return Ok(ConfigVersion::V0);
    };

    let version = config.dist.config_version;

    Ok(version)
}

/// Returns true if the project is using a v1 config _or_ if the `DIST_V1`
/// environment variable is set to any value except `false`.
pub fn want_v1() -> DistResult<bool> {
    let want_v1 = std::env::var("DIST_V1")
        .map(|s| s != "false")
        .unwrap_or(false);

    Ok(want_v1 || (get_version()? == ConfigVersion::V1))
}
