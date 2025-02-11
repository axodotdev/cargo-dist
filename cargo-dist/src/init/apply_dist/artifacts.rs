use super::helpers::*;
use crate::config::v1::artifacts::archives::ArchiveLayer;
use crate::config::v1::artifacts::ArtifactLayer;
use crate::config::v1::layer::{BoolOr, BoolOrOptExt};
use axoasset::toml_edit::{self, DocumentMut, Item, Table};

pub fn apply(table: &mut toml_edit::Table, artifacts: &Option<ArtifactLayer>) {
    let Some(artifacts) = artifacts else {
        return;
    };
    let artifacts_table = table
        .entry("artifacts")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .expect("[dist.artifacts] should be a table");

    apply_artifacts_archives(artifacts_table, &artifacts.archives);

    apply_optional_value(
        artifacts_table,
        "source-tarball",
        "# Generate a source tarball\n",
        artifacts.source_tarball,
    );

    // [dist.artifacts.extra] is not reformatted due to complexity.
    skip_optional_value(
        artifacts_table,
        "extra",
        "# Any extra artifacts, and their build scripts\n",
        artifacts.extra.as_ref(),
    );

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
    let archives_table = artifacts_table
        .entry("archives")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .expect("[dist.artifacts.archives] should be a table");

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

#[cfg(test)]
mod test {
    use super::*;
    use crate::config::LibraryStyle;
    use crate::{ChecksumStyle, CompressionImpl, ZipStyle};
    use axoasset::toml_edit::{self, DocumentMut, Item, Table};
    use miette::IntoDiagnostic;
    use pretty_assertions::{assert_eq, assert_ne};

    fn source() -> toml_edit::DocumentMut {
        let src = axoasset::SourceFile::new("fake-dist-workspace.toml", String::new());
        let doc = src.deserialize_toml_edit().into_diagnostic().unwrap();
        doc
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
    fn apply_artifacts_empty() {
        let expected = "";

        let artifacts = Some(ArtifactLayer {
            archives: None,
            checksum: None,
            extra: None,
            source_tarball: None,
        });

        let mut doc = source();
        let table = dist_table(&mut doc);

        apply(table, &artifacts);

        let toml_text = table.to_string();
        assert_eq!(toml_text, expected);
    }

    #[test]
    fn apply_artifacts_everything() {
        let expected = r#"
# Artifact configuration for dist
[dist.artifacts]
# Generate a source tarball
source-tarball = false
# The checksum format to generate
checksum = "blake2b"

[dist.artifacts.archives]
# Extra static files to include in each App (path relative to this Cargo.toml's dir)
include = ["some-include"]
# Whether to auto-include files like READMEs, LICENSEs, and CHANGELOGs (default true)
auto-includes = false
# The archive format to use for windows builds (defaults .zip)
windows-archive = ".tar.gz"
# The archive format to use for non-windows builds (defaults .tar.xz)
unix-archive = ".zip"
# Which kinds of built libraries to include in the final archives
package-libraries = ["cdylib", "cstaticlib"]
"#;

        let artifacts = Some(ArtifactLayer {
            archives: Some(ArchiveLayer {
                include: Some(vec!["some-include".into()]),
                auto_includes: Some(false),
                windows_archive: Some(ZipStyle::Tar(CompressionImpl::Gzip)),
                unix_archive: Some(ZipStyle::Zip),
                package_libraries: Some(vec![LibraryStyle::CDynamic, LibraryStyle::CStatic]),
            }),
            checksum: Some(ChecksumStyle::Blake2b),
            extra: None,
            source_tarball: Some(false),
        });

        let mut doc = source();
        let table = dist_table(&mut doc);

        apply(table, &artifacts);

        let toml_text = doc.to_string();
        assert_eq!(expected, toml_text);
    }
}
