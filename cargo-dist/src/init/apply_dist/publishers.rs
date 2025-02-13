use super::helpers::*;
use crate::config::v1::layer::BoolOr;
use crate::config::v1::publishers::{CommonPublisherLayer, PublisherLayer};
use axoasset::toml_edit::{self, Item, Table};

pub fn apply(table: &mut toml_edit::Table, publishers: &Option<PublisherLayer>) {
    let Some(publishers) = publishers else {
        // Nothing to do.
        return;
    };

    let publishers_table = table
        .entry("publishers")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .expect("[dist.publishers] should be a table");


    apply_common(publishers_table, &publishers.common);
    apply_homebrew(publishers_table, publishers);
    apply_npm(publishers_table, publishers);

    // Finalize the table
    publishers_table
        .decor_mut()
        .set_prefix("\n# Publisher configuration for dist\n");
}

fn apply_common(table: &mut toml_edit::Table, common: &CommonPublisherLayer) {
    apply_optional_value(
        table,
        "prereleases",
        "# Whether to publish prereleases (defaults to false)\n",
        common.prereleases,
    );
}

fn apply_homebrew(publishers_table: &mut toml_edit::Table, publishers: &PublisherLayer) {
    if let Some(BoolOr::Bool(b)) = publishers.homebrew {
        // If it was set as a boolean, simply set it as a boolean and return.
        apply_optional_value(publishers_table,
            "homebrew",
            "# Whether to publish to Homebrew\n",
            Some(b),
        );
        return;
    }

    let Some(BoolOr::Val(ref homebrew)) = publishers.homebrew else {
        // dist.publishers.homebrew isn't specified; nothing to do.
        return;
    };

    let hb_table = publishers_table
        .entry("homebrew")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .expect("[dist.publishers.homebrew] should be a bool or a table");

    apply_common(hb_table, &homebrew.common);

    // Finalize the table
    publishers_table
        .decor_mut()
        .set_prefix("\n# Configuration for publishing to Homebrew\n");
}

fn apply_npm(publishers_table: &mut toml_edit::Table, publishers: &PublisherLayer) {
    if let Some(BoolOr::Bool(b)) = publishers.npm {
        // If it was set as a boolean, simply set it as a boolean and return.
        apply_optional_value(publishers_table,
            "npm",
            "# Whether to publish to NPM\n",
            Some(b),
        );
        return;
    }

    let Some(BoolOr::Val(ref npm)) = publishers.npm else {
        // dist.publishers.npm isn't specified; nothing to do.
        return;
    };

    let hb_table = publishers_table
        .entry("npm")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .expect("[dist.publishers.npm] should be a bool or a table");

    apply_common(hb_table, &npm.common);

    // Finalize the table
    publishers_table
        .decor_mut()
        .set_prefix("\n# Configuration for publishing to NPM\n");
}



#[cfg(test)]
mod test {
    use super::*;
    use crate::config::v1::publishers::homebrew::HomebrewPublisherLayer;
    use crate::config::v1::publishers::npm::NpmPublisherLayer;
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

        let layer = Some(PublisherLayer {
            common: CommonPublisherLayer {
                prereleases: None,
            },
            homebrew: None,
            npm: None,
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
# Publisher configuration for dist
[dist.publishers]
# Whether to publish prereleases (defaults to false)
prereleases = true
# Whether to publish to Homebrew
homebrew = true
# Whether to publish to NPM
npm = true
"#;

        let layer = Some(PublisherLayer {
            common: CommonPublisherLayer {
                prereleases: Some(true),
            },
            homebrew: Some(BoolOr::Bool(true)),
            npm: Some(BoolOr::Bool(true)),
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
# Publisher configuration for dist
[dist.publishers]
# Whether to publish prereleases (defaults to false)
prereleases = true

[dist.publishers.homebrew]
# Whether to publish prereleases (defaults to false)
prereleases = true

[dist.publishers.npm]
# Whether to publish prereleases (defaults to false)
prereleases = true
"#;

        let layer = Some(PublisherLayer {
            common: CommonPublisherLayer {
                prereleases: Some(true),
            },
            homebrew: Some(BoolOr::Val(HomebrewPublisherLayer {
                common: CommonPublisherLayer {
                    prereleases: Some(true),
                },
            })),
            npm: Some(BoolOr::Val(NpmPublisherLayer {
                common: CommonPublisherLayer {
                    prereleases: Some(true),
                },
            })),
        });

        let mut doc = source();
        let table = dist_table(&mut doc);

        apply(table, &layer);

        let toml_text = doc.to_string();
        assert_eq!(expected, toml_text);
    }
}
