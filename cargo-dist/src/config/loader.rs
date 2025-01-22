use crate::{
    config::{self, v1::DistConfig, v1::TomlLayer},
    errors::{DistError, DistResult},
};
use axoasset::SourceFile;
use camino::Utf8Path;

/// Load the dist(-workspace).toml for the root workspace.
pub fn load_root() -> DistResult<DistConfig> {
    let workspaces = config::get_project()?;
    let root_workspace = workspaces.root_workspace();

    let Some(path) = root_workspace.dist_manifest_path.as_deref() else {
        return Err(DistError::NoConfigFile {});
    };

    load(path)
}

/// Loads a dist(-workspace).toml from disk.
pub fn load(dist_manifest_path: &Utf8Path) -> DistResult<DistConfig> {
    let src = SourceFile::load_local(dist_manifest_path)?;
    parse(src)
}

/// Load a dist(-workspace).toml from disk and return its `[dist]` table.
pub fn load_dist(dist_manifest_path: &Utf8Path) -> DistResult<TomlLayer> {
    Ok(load(dist_manifest_path)?.dist.unwrap_or_default())
}

/// Given a SourceFile of a dist(-workspace).toml, deserializes it.
pub fn parse(src: SourceFile) -> DistResult<DistConfig> {
    // parse() can probably be consolidated into load() eventually.
    Ok(src.deserialize_toml()?)
}

/// Given a SourceFile of a dist(-workspace).toml, deserialize its `[dist]` table.
pub fn parse_dist(src: SourceFile) -> DistResult<TomlLayer> {
    Ok(parse(src)?.dist.unwrap_or_default())
}
