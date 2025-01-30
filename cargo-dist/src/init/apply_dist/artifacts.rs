use super::helpers::*;
use crate::config::v1::artifacts::archives::ArchiveLayer;
use crate::config::v1::artifacts::ArtifactLayer;
use crate::config::v1::layer::{BoolOr, BoolOrOptExt};
use axoasset::toml_edit;

pub fn apply(table: &mut toml_edit::Table, artifacts: &Option<ArtifactLayer>) {
    let Some(artifacts) = artifacts else {
        return;
    };
    let Some(artifacts_table) = table.get_mut("artifacts") else {
        return;
    };
    let toml_edit::Item::Table(artifacts_table) = artifacts_table else {
        panic!("Expected [dist.artifacts] to be a table");
    };

    // TODO(migration): implement this

    apply_artifacts_archives(artifacts_table, &artifacts.archives);

    apply_optional_value(
        artifacts_table,
        "source-tarball",
        "# Generate and dist a source tarball\n",
        artifacts.source_tarball,
    );

    // TODO(migration): implement dist.artifacts.extra.
    /*
    apply_optional_value(
        artifacts_table,
        "extra",
        "# Any extra artifacts, and their build scripts\n",
        artifacts.extra,
    );
    */

    apply_optional_value(
        artifacts_table,
        "checksum",
        "# The checksum format to generate\n",
        artifacts.checksum.map(|cs| cs.to_string()),
    );

    // Finalize the table
    artifacts_table
        .decor_mut()
        .set_prefix("\n# Artifact configuration for dist\n");
}

fn apply_artifacts_archives(
    artifacts_table: &mut toml_edit::Table,
    archives: &Option<ArchiveLayer>,
) {
    let Some(archives) = archives else {
        return;
    };
    let Some(archives_table) = artifacts_table.get_mut("archives") else {
        return;
    };
    let toml_edit::Item::Table(archives_table) = archives_table else {
        panic!("Expected [dist.artifacts.archives] to be a table");
    };

    apply_string_list(
        archives_table,
        "include",
        "# Extra static files to include in each App (path relative to this Cargo.toml's dir)\n",
        archives.include.as_ref(),
    );

    apply_optional_value(
        archives_table,
        "auto-includes",
        "# Whether to auto-include files like READMEs, LICENSEs, and CHANGELOGs (default true)\n",
        archives.auto_includes,
    );

    apply_optional_value(
        archives_table,
        "windows-archive",
        "# The archive format to use for windows builds (defaults .zip)\n",
        archives.windows_archive.map(|a| a.ext()),
    );

    apply_optional_value(
        archives_table,
        "unix-archive",
        "# The archive format to use for non-windows builds (defaults .tar.xz)\n",
        archives.unix_archive.map(|a| a.ext()),
    );

    apply_string_or_list(
        archives_table,
        "package-libraries",
        "# Which kinds of built libraries to include in the final archives\n",
        archives.package_libraries.as_ref(),
    );
}
