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

#[cfg(test)]
mod tests {
    use super::*;
    use axoasset::SourceFile;

    #[test]
    fn parse_v1_succeeds() {
        let file = SourceFile::new("fake-v1-dist-workspace.toml", r##"
[workspace]
members = ["cargo:*"]

[package]
name = "whatever"
version = "1.0.0"

# Config for 'dist'
[dist]
dist-version = "1.0.0"
targets = ["aarch64-apple-darwin", "aarch64-unknown-linux-gnu", "aarch64-unknown-linux-musl", "x86_64-apple-darwin", "x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl", "x86_64-pc-windows-msvc"]

[dist.build]
min-glibc-version."*" = "2.18"

[dist.ci]
github = true
pr-run-mode = "plan"
#publish-jobs = ["homebrew", "./publish-crates"]

[dist.hosts]
github = true

[dist.installers]
install-path = "CARGO_HOME"
shell = true
updater = false

[dist.installers.homebrew]
tap = "axodotdev/homebrew-tap"
        "##.to_string());

        assert!(parse(file).is_ok());
    }

    #[test]
    fn parse_v0_fails() {
        let file = SourceFile::new("fake-v0-dist-workspace.toml", r##"
[workspace]
members = ["cargo:*"]

[package]
name = "whatever"
version = "1.0.0"

# Config for 'dist'
[dist]
# The preferred dist version to use in CI (Cargo.toml SemVer syntax)
cargo-dist-version = "0.13.1-prerelease.1"
# CI backends to support
ci = "github"
# The installers to generate for each app
installers = ["shell", "powershell", "homebrew"]
# A GitHub repo to push Homebrew formulas to
tap = "axodotdev/homebrew-tap"
# Publish jobs to run in CI
publish-jobs = ["homebrew", "./publish-crates"]
# Target platforms to build apps for (Rust target-triple syntax)
targets = ["aarch64-apple-darwin", "aarch64-unknown-linux-gnu", "aarch64-unknown-linux-musl", "x86_64-apple-darwin", "x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl", "x86_64-pc-windows-msvc"]
# Which actions to run on pull requests
pr-run-mode = "plan"
# Where to host releases
hosting = ["github"]
# Whether to install an updater program
install-updater = false
# Path that installers should place binaries in
install-path = "CARGO_HOME"
# The minimum glibc version supported by the package (overrides auto-detection)
min-glibc-version."*" = "2.18"
        "##.to_string());

        assert!(parse(file).is_err());
    }
}
