use super::helpers::*;
use crate::config::v1::hosts::HostLayer;
use crate::config::v1::layer::BoolOr;
use axoasset::toml_edit::{self, Item, Table};

pub fn apply(table: &mut toml_edit::Table, hosts: &Option<HostLayer>) {
    let Some(hosts) = hosts else {
        // Nothing to do.
        return;
    };

    let hosts_table = table
        .entry("hosts")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .expect("[dist.hosts] should be a table");

    // hosts.common is just `CommonHostLayer {}`, so there's nothing to do.

    apply_optional_value(
        hosts_table,
        "force-latest",
        "# Always regard releases as stable (defaults to false)\n",
        hosts.force_latest,
    );

    apply_optional_value(
        hosts_table,
        "display",
        "# Whether artifacts/installers for this app should be displayed in release bodies\n",
        hosts.display,
    );

    apply_optional_value(
        hosts_table,
        "display-name",
        "# How to refer to the app in release bodies\n",
        hosts.display_name.as_ref(),
    );

    apply_github(hosts_table, hosts);
    apply_axodotdev(hosts_table, hosts);

    // Finalize the table
    hosts_table
        .decor_mut()
        .set_prefix("\n# Hosting configuration for dist\n");
}

fn apply_github(hosts_table: &mut toml_edit::Table, hosts: &HostLayer) {
    if let Some(BoolOr::Bool(b)) = hosts.github {
        // If it was set as a boolean, simply set it as a boolean and return.
        apply_optional_value(hosts_table,
            "github",
            "# Configuration for GitHub hosting\n# (Use the table format of [dist.hosts.github] for more nuanced config!)\n",
            Some(b),
        );
        return;
    }

    let Some(BoolOr::Val(ref github)) = hosts.github else {
        return;
    };

    let gh_table = hosts_table
        .entry("github")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .expect("[dist.hosts.github] should be a bool or a table");

    apply_optional_value(
        gh_table,
        "create",
        "# Whether dist should create the GitHub release (default: true)\n",
        github.create,
    );

    apply_optional_value(
        gh_table,
        "repo",
        "# Publish GitHub Releases to this repo instead\n",
        github.repo.as_ref().map(|a| a.to_string()),
    );

    apply_optional_value(
        gh_table,
        "during",
        "# Which phase dist should use to create the GitHub release\n",
        github.during.as_ref().map(|a| a.to_string()),
    );

    apply_optional_value(
        gh_table,
        "submodule-path",
        "# Read the commit to be tagged from the submodule at this path\n",
        github.submodule_path
            .as_ref()
            .map(|a| a.to_string()),
    );

    apply_optional_value(
        gh_table,
        "attestations",
        "# Whether to enable GitHub Attestations\n",
        github.attestations,
    );

    // Finalize the table
    gh_table
        .decor_mut()
        .set_prefix("\n# Configuration for GitHub hosting\n");
}

fn apply_axodotdev(hosts_table: &mut toml_edit::Table, hosts: &HostLayer) {
    if let Some(BoolOr::Bool(b)) = hosts.axodotdev {
        // If it was set as a boolean, simply set it as a boolean and return.
        apply_optional_value(hosts_table,
            "axodotdev",
            "# Whether to use axo.dev hosting\n",
            Some(b),
        );
        return;
    }

    let Some(BoolOr::Val(ref _axo)) = hosts.axodotdev else {
        return;
    };

    // There is no reason for this to ever involve a struct,
    // but it does and I don't have time to untangle it.
    //
    // So: If the table exists, we turn it into `axodotdev=true`.
    //
    // Theoretically, there's no valid representation of AxodotdevHostLayer
    // which isn't empty, so this should never run.
    // -@duckinator
    apply_optional_value(hosts_table,
        "axodotdev",
        "# Whether to use axo.dev hosting\n",
        Some(true),
    );
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::config::v1::hosts::CommonHostLayer;
    use crate::config::v1::hosts::github::GithubHostLayer;
    use crate::config::{GithubReleasePhase, GithubRepoPair};
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

        let layer = Some(HostLayer {
            common: CommonHostLayer {},
            force_latest: None,
            display: None,
            display_name: None,
            github: None,
            axodotdev: None,
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
# Hosting configuration for dist
[dist.hosts]
# Always regard releases as stable (defaults to false)
force-latest = true
# Whether artifacts/installers for this app should be displayed in release bodies
display = true
# How to refer to the app in release bodies
display-name = "some-name"
# Configuration for GitHub hosting
# (Use the table format of [dist.hosts.github] for more nuanced config!)
github = true
# Whether to use axo.dev hosting
axodotdev = true
"#;

        let layer = Some(HostLayer {
            common: CommonHostLayer {},
            force_latest: Some(true),
            display: Some(true),
            display_name: Some("some-name".to_string()),
            github: Some(BoolOr::Bool(true)),
            axodotdev: Some(BoolOr::Bool(true)),
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
# Hosting configuration for dist
[dist.hosts]
# Always regard releases as stable (defaults to false)
force-latest = true
# Whether artifacts/installers for this app should be displayed in release bodies
display = true
# How to refer to the app in release bodies
display-name = "some-name"
# Whether to use axo.dev hosting
axodotdev = true

# Configuration for GitHub hosting
[dist.hosts.github]
# Whether dist should create the GitHub release (default: true)
create = true
# Publish GitHub Releases to this repo instead
repo = "example-user/example-repo"
# Which phase dist should use to create the GitHub release
during = "auto"
# Read the commit to be tagged from the submodule at this path
submodule-path = "./foo"
# Whether to enable GitHub Attestations
attestations = true
"#;

        let github = GithubHostLayer {
            common: CommonHostLayer {},
            create: Some(true),
            repo: Some(GithubRepoPair {
                owner: "example-user".to_string(),
                repo: "example-repo".to_string(),
            }),
            submodule_path: Some("./foo".into()),
            during: Some(GithubReleasePhase::Auto),
            attestations: Some(true),
        };

        let layer = Some(HostLayer {
            common: CommonHostLayer {},
            force_latest: Some(true),
            display: Some(true),
            display_name: Some("some-name".to_string()),
            github: Some(BoolOr::Val(github)),
            axodotdev: Some(BoolOr::Bool(true)),
        });

        let mut doc = source();
        let table = dist_table(&mut doc);

        apply(table, &layer);

        let toml_text = doc.to_string();
        assert_eq!(expected, toml_text);
    }
}
