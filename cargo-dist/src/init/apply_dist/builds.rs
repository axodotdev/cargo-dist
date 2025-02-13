use super::helpers::*;
use super::system_dependencies;
use crate::config::v1::builds::BuildLayer;
use crate::config::v1::layer::BoolOr;
use axoasset::toml_edit::{self, Item, Table};

pub fn apply(table: &mut toml_edit::Table, builds: &Option<BuildLayer>) {
    let Some(builds) = builds else {
        // Nothing to do.
        return;
    };

    let builds_table = table
        .entry("builds")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .expect("[dist.builds] should be a table");

    apply_optional_value(
        builds_table,
        "ssldotcom-windows-sign",
        "# Whether we should sign Windows binaries using ssl.com\n",
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

    apply_optional_value(
        builds_table,
        "omnibor",
        "# Whether to use omnibor-cli to generate OmniBOR Artifact IDs\n",
        builds.omnibor,
    );

    apply_optional_min_glibc_version(
        builds_table,
        "min-glibc-version",
        "\n# The minimum glibc version supported by the package (overrides auto-detection)\n",
        builds.min_glibc_version.as_ref(),
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

    let cargo_builds_table = builds_table
        .entry("cargo")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .expect("[dist.builds.cargo] should be a bool or a table");

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

#[cfg(test)]
mod test {
    use super::*;
    use crate::config::v1::builds::cargo::CargoBuildLayer;
    use crate::config::v1::builds::generic::GenericBuildLayer;
    use crate::config::v1::builds::CommonBuildLayer;
    use crate::config::{ProductionMode, SystemDependencies};
    use miette::IntoDiagnostic;
    use pretty_assertions::assert_eq;

    fn source() -> toml_edit::DocumentMut {
        let src = axoasset::SourceFile::new("fake-dist-workspace.toml", String::new());
        src.deserialize_toml_edit().into_diagnostic().unwrap()
    }

    // Given a DocumentMut, make sure it has a [dist] table, and return
    // a reference to that dist table.
    fn dist_table(doc: &mut toml_edit::DocumentMut) -> &mut toml_edit::Table {
        let dist = doc
            .entry("dist")
            .or_insert(Item::Table(Table::new()))
            .as_table_mut()
            .unwrap();
        // Don't show the empty top-level [dist].
        dist.set_implicit(true);
        // Return the table we just created.
        dist
    }

    #[test]
    fn apply_empty() {
        let expected = "";

        let layer = Some(BuildLayer {
            common: CommonBuildLayer {},
            ssldotcom_windows_sign: None,
            macos_sign: None,
            cargo: None,
            generic: None,
            system_dependencies: None,
            min_glibc_version: None,
            omnibor: None,
        });

        let mut doc = source();
        let table = dist_table(&mut doc);

        apply(table, &layer);

        let toml_text = table.to_string();
        assert_eq!(toml_text, expected);
    }

    #[test]
    fn apply_everything() {
        let expected = r#"
# Build configuration for dist
[dist.builds]
# Whether we should sign Windows binaries using ssl.com
ssldotcom-windows-sign = "test"
# Whether to sign macOS executables
macos-sign = true
# Whether dist should build cargo projects
# (Use the table format of [dist.builds.cargo] for more nuanced config!)
cargo = true
# Whether to use omnibor-cli to generate OmniBOR Artifact IDs
omnibor = true

# The minimum glibc version supported by the package (overrides auto-detection)
[dist.builds.min-glibc-version]
some-target = "1.2"
"#;

        let mut min_glibc = crate::platform::MinGlibcVersion::new();
        min_glibc.insert(
            "some-target".to_string(),
            crate::platform::LibcVersion {
                major: 1,
                series: 2,
            },
        );

        let layer = Some(BuildLayer {
            common: CommonBuildLayer {},
            ssldotcom_windows_sign: Some(ProductionMode::Test),
            macos_sign: Some(true),
            cargo: Some(BoolOr::Bool(true)),
            generic: Some(BoolOr::Bool(true)),
            system_dependencies: None,
            min_glibc_version: Some(min_glibc),
            omnibor: Some(true),
        });

        let mut doc = source();
        let table = dist_table(&mut doc);

        apply(table, &layer);

        let toml_text = doc.to_string();
        assert_eq!(expected, toml_text);
    }

    #[test]
    fn apply_complex() {
        let expected = r#"
# Build configuration for dist
[dist.builds]
# Whether we should sign Windows binaries using ssl.com
ssldotcom-windows-sign = "test"
# Whether to sign macOS executables
macos-sign = true
# Whether to use omnibor-cli to generate OmniBOR Artifact IDs
omnibor = true

# How dist should build Cargo projects
[dist.builds.cargo]
# Whether +crt-static should be used on msvc
msvc-crt-static = true
# Build only the required packages, and individually
precise-builds = true
# Features to pass to cargo build
features = ["some-feature"]
# Whether default-features should be enabled with cargo build
default-features = true
# Whether to pass --all-features to cargo build
all-features = true
# Whether to embed dependency information using cargo-auditable
cargo-auditable = true
# Whether to use cargo-cyclonedx to generate an SBOM
cargo-cyclonedx = true

# The minimum glibc version supported by the package (overrides auto-detection)
[dist.builds.min-glibc-version]
some-target = "1.2"
"#;

        let mut min_glibc = crate::platform::MinGlibcVersion::new();
        min_glibc.insert(
            "some-target".to_string(),
            crate::platform::LibcVersion {
                major: 1,
                series: 2,
            },
        );

        let cargo_bl = CargoBuildLayer {
            common: CommonBuildLayer {},
            // Deprecated/v0-specific.
            rust_toolchain_version: None,
            msvc_crt_static: Some(true),
            precise_builds: Some(true),
            features: Some(vec!["some-feature".to_string()]),
            default_features: Some(true),
            all_features: Some(true),
            cargo_auditable: Some(true),
            cargo_cyclonedx: Some(true),
        };

        let generic_bl = GenericBuildLayer {
            common: CommonBuildLayer {},
        };

        let layer = Some(BuildLayer {
            common: CommonBuildLayer {},
            ssldotcom_windows_sign: Some(ProductionMode::Test),
            macos_sign: Some(true),
            cargo: Some(BoolOr::Val(cargo_bl)),
            generic: Some(BoolOr::Val(generic_bl)),
            system_dependencies: None,
            min_glibc_version: Some(min_glibc),
            omnibor: Some(true),
        });

        let mut doc = source();
        let table = dist_table(&mut doc);

        apply(table, &layer);

        let toml_text = doc.to_string();
        assert_eq!(expected, toml_text);
    }
}
