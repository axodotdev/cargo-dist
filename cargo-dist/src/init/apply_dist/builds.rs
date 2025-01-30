use super::helpers::*;
use super::system_dependencies;
use crate::config::v1::builds::BuildLayer;
use crate::config::v1::layer::{BoolOr, BoolOrOptExt};
use axoasset::toml_edit;

pub fn apply(table: &mut toml_edit::Table, builds: &Option<BuildLayer>) {
    let Some(builds) = builds else {
        // Nothing to do.
        return;
    };
    let Some(builds_table) = table.get_mut("builds") else {
        // Nothing to do.
        return;
    };
    let toml_edit::Item::Table(builds_table) = builds_table else {
        panic!("Expected [dist.builds] to be a table");
    };

    apply_optional_value(
        builds_table,
        "ssldotcom-windows-sign",
        "# Whether we should sign Windows binaries using ssl.com",
        builds
            .ssldotcom_windows_sign
            .as_ref()
            .map(|p| p.to_string()),
    );

    apply_optional_value(
        builds_table,
        "macos-sign",
        "# Whether to sign macOS executables\n",
        builds.macos_sign,
    );

    apply_cargo_builds(builds_table, builds);
    system_dependencies::apply(builds_table, builds.system_dependencies.as_ref());

    apply_optional_min_glibc_version(
        builds_table,
        "min-glibc-version",
        "# The minimum glibc version supported by the package (overrides auto-detection)\n",
        builds.min_glibc_version.as_ref(),
    );

    apply_optional_value(
        builds_table,
        "omnibor",
        "# Whether to use omnibor-cli to generate OmniBOR Artifact IDs\n",
        builds.omnibor,
    );

    // Finalize the table
    builds_table
        .decor_mut()
        .set_prefix("\n# Build configuration for dist\n");
}

fn apply_cargo_builds(builds_table: &mut toml_edit::Table, builds: &BuildLayer) {
    if let Some(BoolOr::Bool(b)) = builds.cargo {
        // If it was set as a boolean, simply set it as a boolean and return.
        apply_optional_value(builds_table,
            "cargo",
            "# Whether dist should build cargo projects\n# (Use the table format of [dist.builds.cargo] for more nuanced config!)\n",
            Some(b),
        );
        return;
    }

    let Some(BoolOr::Val(ref cargo_builds)) = builds.cargo else {
        return;
    };

    let mut possible_table = toml_edit::table();
    let cargo_builds_table = builds_table.get_mut("cargo").unwrap_or(&mut possible_table);

    let toml_edit::Item::Table(cargo_builds_table) = cargo_builds_table else {
        panic!("Expected [dist.builds.cargo] to be a table")
    };

    apply_optional_value(
        cargo_builds_table,
        "rust-toolchain-version",
        "# The preferred Rust toolchain to use in CI (rustup toolchain syntax)\n",
        cargo_builds.rust_toolchain_version.as_deref(),
    );

    apply_optional_value(
        cargo_builds_table,
        "msvc-crt-static",
        "# Whether +crt-static should be used on msvc\n",
        cargo_builds.msvc_crt_static,
    );

    apply_optional_value(
        cargo_builds_table,
        "precise-builds",
        "# Build only the required packages, and individually\n",
        cargo_builds.precise_builds,
    );

    apply_string_list(
        cargo_builds_table,
        "features",
        "# Features to pass to cargo build\n",
        cargo_builds.features.as_ref(),
    );

    apply_optional_value(
        cargo_builds_table,
        "default-features",
        "# Whether default-features should be enabled with cargo build\n",
        cargo_builds.default_features,
    );

    apply_optional_value(
        cargo_builds_table,
        "all-features",
        "# Whether to pass --all-features to cargo build\n",
        cargo_builds.all_features,
    );

    apply_optional_value(
        cargo_builds_table,
        "cargo-auditable",
        "# Whether to embed dependency information using cargo-auditable\n",
        cargo_builds.cargo_auditable,
    );

    apply_optional_value(
        cargo_builds_table,
        "cargo-cyclonedx",
        "# Whether to use cargo-cyclonedx to generate an SBOM\n",
        cargo_builds.cargo_cyclonedx,
    );

    // Finalize the table
    cargo_builds_table
        .decor_mut()
        .set_prefix("\n# How dist should build Cargo projects\n");
}
