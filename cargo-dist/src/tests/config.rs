use crate::{
    backend::diff_source,
    config::{parse_generic_config, parse_metadata_table, DistMetadata},
    init::apply_dist_to_workspace_toml,
    DistResult,
};
use axoasset::SourceFile;
use axoproject::WorkspaceKind;
use camino::Utf8PathBuf;

fn parse_rust_config(src: SourceFile) -> DistResult<DistMetadata> {
    // yes this is deserializing a toml document into a json value
    // this is literally what `cargo metadata` does. serde is magic.
    let json_val: serde_json::Value = src.deserialize_toml()?;
    let path = Utf8PathBuf::from(src.origin_path());
    let metadata = json_val.get("workspace").and_then(|v| v.get("metadata"));
    parse_metadata_table(&path, metadata)
}

fn parse_config(src: &SourceFile, input_kind: WorkspaceKind) -> DistResult<DistMetadata> {
    match input_kind {
        WorkspaceKind::Javascript => {
            unimplemented!("npm packages don't have [package.metadata.dist]")
        }
        WorkspaceKind::Rust => parse_rust_config(src.clone()),
        WorkspaceKind::Generic => parse_generic_config(src.clone()),
    }
}

fn format_config(
    src: &SourceFile,
    input_kind: WorkspaceKind,
    config: &DistMetadata,
) -> DistResult<SourceFile> {
    let mut workspace_toml = src.deserialize_toml_edit()?;
    apply_dist_to_workspace_toml(&mut workspace_toml, input_kind, config);
    Ok(SourceFile::new(
        src.origin_path(),
        workspace_toml.to_string(),
    ))
}

fn source(input: &str, input_kind: WorkspaceKind) -> SourceFile {
    let src_name = match input_kind {
        WorkspaceKind::Javascript => "package.json",
        WorkspaceKind::Rust => "Cargo.toml",
        WorkspaceKind::Generic => "dist.toml",
    };
    SourceFile::new(src_name, input.to_owned())
}

#[test]
fn basic_cargo_toml_no_change() {
    // check that formatting roundtrips stabley
    let input_kind = WorkspaceKind::Rust;
    let input = r##"
[package]
name = "whatever"
version = "1.0.0"

# Config for 'cargo dist'
[workspace.metadata.dist]
# The preferred cargo-dist version to use in CI (Cargo.toml SemVer syntax)
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
# Publish jobs to run in CI
pr-run-mode = "plan"
# Where to host releases
hosting = ["axodotdev", "github"]
# Whether to install an updater program
install-updater = false

[[workspace.metadata.dist.extra-artifacts]]
artifacts = ["dist-manifest-schema.json"]
build = ["cargo", "run", "--", "dist", "manifest-schema", "--output=dist-manifest-schema.json"]

[workspace.metadata.dist.github-custom-runners]
aarch64-unknown-linux-gnu = "buildjet-8vcpu-ubuntu-2204-arm"
aarch64-unknown-linux-musl = "buildjet-8vcpu-ubuntu-2204-arm"

# The profile that 'cargo dist' will build with
[profile.dist]
inherits = "release"
lto = "thin"

"##;

    let src = source(input, input_kind);
    let config = parse_config(&src, input_kind).unwrap();
    let result = format_config(&src, input_kind, &config).unwrap();
    // Require no diff
    diff_source(src, result.contents())
        .map_err(miette::Report::new)
        .unwrap();
}

#[test]
fn basic_cargo_toml_one_item_arrays() {
    // Check that one item arrays get reformatted to just strings (when applicable)
    let input_kind = WorkspaceKind::Rust;
    let input = r##"
[package]
name = "whatever"
version = "1.0.0"

# Config for 'cargo dist'
[workspace.metadata.dist]
# The preferred cargo-dist version to use in CI (Cargo.toml SemVer syntax)
cargo-dist-version = "0.13.1-prerelease.1"
# CI backends to support
ci = ["github"]
# Where to host releases
hosting = ["axodotdev"]
# Path that installers should place binaries in
install-path = ["$MY_COMPANY/bin"]
# Target platforms to build apps for (Rust target-triple syntax)
targets = ["aarch64-apple-darwin"]
"##;

    let expected = r##"
[package]
name = "whatever"
version = "1.0.0"

# Config for 'cargo dist'
[workspace.metadata.dist]
# The preferred cargo-dist version to use in CI (Cargo.toml SemVer syntax)
cargo-dist-version = "0.13.1-prerelease.1"
# CI backends to support
ci = "github"
# Where to host releases
hosting = "axodotdev"
# Path that installers should place binaries in
install-path = "$MY_COMPANY/bin"
# Target platforms to build apps for (Rust target-triple syntax)
targets = ["aarch64-apple-darwin"]
"##;

    let src = source(input, input_kind);
    let config = parse_config(&src, input_kind).unwrap();
    let result = format_config(&src, input_kind, &config).unwrap();
    let expect = source(expected, input_kind);
    // Require no diff
    diff_source(expect, result.contents())
        .map_err(miette::Report::new)
        .unwrap();
}

#[test]
fn basic_cargo_toml_multi_item_arrays() {
    // Check that multi-items arrays get read properly (especially when "string or array")
    let input_kind = WorkspaceKind::Rust;
    let input = r##"
[package]
name = "whatever"
version = "1.0.0"

# Config for 'cargo dist'
[workspace.metadata.dist]
# The preferred cargo-dist version to use in CI (Cargo.toml SemVer syntax)
cargo-dist-version = "0.13.1-prerelease.1"
# CI backends to support
ci = ["github", "github"]
# Where to host releases
hosting = ["axodotdev", "github"]
# Path that installers should place binaries in
install-path = ["$MY_COMPANY/bin", "~/.mycompany/bin"]
# Target platforms to build apps for (Rust target-triple syntax)
targets = ["aarch64-apple-darwin", "x86_64-apple-darwin"]
"##;

    let expected = r##"
[package]
name = "whatever"
version = "1.0.0"

# Config for 'cargo dist'
[workspace.metadata.dist]
# The preferred cargo-dist version to use in CI (Cargo.toml SemVer syntax)
cargo-dist-version = "0.13.1-prerelease.1"
# CI backends to support
ci = ["github", "github"]
# Where to host releases
hosting = ["axodotdev", "github"]
# Path that installers should place binaries in
install-path = ["$MY_COMPANY/bin", "~/.mycompany/bin"]
# Target platforms to build apps for (Rust target-triple syntax)
targets = ["aarch64-apple-darwin", "x86_64-apple-darwin"]
"##;

    let src = source(input, input_kind);
    let config = parse_config(&src, input_kind).unwrap();
    let result = format_config(&src, input_kind, &config).unwrap();
    let expect = source(expected, input_kind);
    // Require no diff
    diff_source(expect, result.contents())
        .map_err(miette::Report::new)
        .unwrap();
}
