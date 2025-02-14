use super::helpers::*;
use crate::config::v1::ci::github::GithubCiLayer;
use crate::config::v1::ci::{CiLayer, CommonCiLayer};
use crate::config::v1::layer::BoolOr;
use axoasset::toml_edit::{self, Item, Table};

pub fn apply(table: &mut toml_edit::Table, ci: &Option<CiLayer>) {
    let Some(ci) = ci else {
        // Nothing to do.
        return;
    };
    let ci_table = table
        .entry("ci")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .expect("[dist.ci] should be a table");

    apply_ci_common(ci_table, &ci.common);

    if let Some(github) = &ci.github {
        match github {
            BoolOr::Bool(b) => {
                apply_optional_value(
                    ci_table,
                    "github",
                    "# Whether dist should generate workflows for GitHub CI\n",
                    Some(*b),
                );
            }
            BoolOr::Val(v) => {
                apply_ci_github(ci_table, v);
            }
        }
    }

    // Finalize the table
    ci_table
        .decor_mut()
        .set_prefix("\n# CI configuration for dist\n");
}

fn apply_ci_github(ci_table: &mut toml_edit::Table, github: &GithubCiLayer) {
    let gh_table = ci_table
        .entry("github")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .expect("[dist.ci.github] should be a table");

    apply_ci_common(gh_table, &github.common);

    // [dist.ci.github.runners] is not reformatted due to complexity.
    skip_optional_value(
        gh_table,
        "runners",
        "# Custom GitHub runners, specified as target triples\n",
        github.runners.as_ref(),
    );

    // [dist.ci.github.permissions] is not reformatted due to complexity.
    skip_optional_value(
        gh_table,
        "permissions",
        "# Custom permissions for jobs\n",
        github.permissions.as_ref(),
    );

    apply_optional_value(
        gh_table,
        "build-setup",
        "# Path to a file containing a YAML array of steps, to be performed before 'dist build'\n\
        # KNOWN BUG: https://github.com/axodotdev/cargo-dist/issues/1750\n",
        github.build_setup.clone(),
    );

    // Finalize the table
    gh_table
        .decor_mut()
        .set_prefix("\n# Configure generated workflows for GitHub CI\n");
}

fn apply_ci_common(table: &mut toml_edit::Table, common: &CommonCiLayer) {
    apply_optional_value(
        table,
        "merge-tasks",
        "# Whether to run otherwise-parallelizable tasks on the same machine\n",
        common.merge_tasks,
    );

    apply_optional_value(
        table,
        "fail-fast",
        "# Whether failing tasks should make us give up on all other tasks\n",
        common.fail_fast,
    );

    apply_optional_value(
        table,
        "cache-builds",
        "# Whether builds should try to be cached in CI\n",
        common.cache_builds,
    );

    apply_optional_value(
        table,
        "build-local-artifacts",
        "# Whether CI should include auto-generated code to build local artifacts\n",
        common.build_local_artifacts,
    );

    apply_optional_value(
        table,
        "dispatch-releases",
        "# Whether CI should trigger releases with dispatches instead of tag pushes\n",
        common.dispatch_releases,
    );

    apply_optional_value(
        table,
        "release-branch",
        "# Trigger releases on pushes to this branch instead of tag pushes\n",
        common.release_branch.as_ref(),
    );

    apply_optional_value(
        table,
        "pr-run-mode",
        "# Which actions to run on pull requests\n",
        common.pr_run_mode.as_ref().map(|m| m.to_string()),
    );

    apply_optional_value(
        table,
        "tag-namespace",
        "# A prefix git tags must include for dist to care about them\n",
        common.tag_namespace.as_ref(),
    );

    apply_string_list(
        table,
        "plan-jobs",
        "# Additional plan jobs to run in CI\n",
        common.plan_jobs.as_ref(),
    );

    apply_string_list(
        table,
        "build-local-jobs",
        "# Additional local artifacts jobs to run in CI\n",
        common.build_local_jobs.as_ref(),
    );

    apply_string_list(
        table,
        "build-global-jobs",
        "# Additional global artifacts jobs to run in CI\n",
        common.build_global_jobs.as_ref(),
    );

    apply_string_list(
        table,
        "host-jobs",
        "# Additional hosts jobs to run in CI\n",
        common.host_jobs.as_ref(),
    );

    apply_string_list(
        table,
        "publish-jobs",
        "# Additional publish jobs to run in CI\n",
        common.publish_jobs.as_ref(),
    );

    apply_string_list(
        table,
        "post-announce-jobs",
        "# Additional jobs to run in CI, after the announce job finishes\n",
        common.post_announce_jobs.as_ref(),
    );
}

#[cfg(test)]
mod test {
    use super::*;
    use miette::IntoDiagnostic;

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
    fn apply_ci_empty() {
        let expected = "";

        let ci = Some(CiLayer {
            common: CommonCiLayer {
                merge_tasks: None,
                fail_fast: None,
                cache_builds: None,
                build_local_artifacts: None,
                dispatch_releases: None,
                release_branch: None,
                pr_run_mode: None,
                tag_namespace: None,
                plan_jobs: None,
                build_local_jobs: None,
                build_global_jobs: None,
                host_jobs: None,
                publish_jobs: None,
                post_announce_jobs: None,
            },
            github: None,
        });

        let mut doc = source();
        let table = dist_table(&mut doc);

        apply(table, &ci);

        let toml_text = table.to_string();
        assert_eq!(toml_text, expected);
    }

    #[test]
    fn apply_ci_everything() {
        let expected = r#"
# CI configuration for dist
[dist.ci]
# Whether to run otherwise-parallelizable tasks on the same machine
merge-tasks = true
# Whether failing tasks should make us give up on all other tasks
fail-fast = true
# Whether builds should try to be cached in CI
cache-builds = true
# Whether CI should include auto-generated code to build local artifacts
build-local-artifacts = true
# Whether CI should trigger releases with dispatches instead of tag pushes
dispatch-releases = true
# Trigger releases on pushes to this branch instead of tag pushes
release-branch = "main"
# Which actions to run on pull requests
pr-run-mode = "skip"
# A prefix git tags must include for dist to care about them
tag-namespace = "some-namespace"
# Additional plan jobs to run in CI
plan-jobs = ["./plan-job"]
# Additional local artifacts jobs to run in CI
build-local-jobs = ["./build-local-job-1", "./build-local-job-2"]
# Additional global artifacts jobs to run in CI
build-global-jobs = ["./build-global-job"]
# Additional hosts jobs to run in CI
host-jobs = ["./host-job"]
# Additional publish jobs to run in CI
publish-jobs = ["./publish-job"]
# Additional jobs to run in CI, after the announce job finishes
post-announce-jobs = ["./post-announce-job"]
# Whether dist should generate workflows for GitHub CI
github = true
"#;

        let ci = Some(CiLayer {
            common: CommonCiLayer {
                merge_tasks: Some(true),
                fail_fast: Some(true),
                cache_builds: Some(true),
                build_local_artifacts: Some(true),
                dispatch_releases: Some(true),
                release_branch: Some("main".to_string()),
                pr_run_mode: Some(dist_schema::PrRunMode::Skip),
                tag_namespace: Some("some-namespace".to_string()),
                plan_jobs: Some(vec!["./plan-job".parse().unwrap()]),
                build_local_jobs: Some(vec![
                    "./build-local-job-1".parse().unwrap(),
                    "./build-local-job-2".parse().unwrap(),
                ]),
                build_global_jobs: Some(vec!["./build-global-job".parse().unwrap()]),
                host_jobs: Some(vec!["./host-job".parse().unwrap()]),
                publish_jobs: Some(vec!["./publish-job".parse().unwrap()]),
                post_announce_jobs: Some(vec!["./post-announce-job".parse().unwrap()]),
            },
            github: Some(BoolOr::Bool(true)),
        });

        let mut doc = source();
        let table = dist_table(&mut doc);

        apply(table, &ci);

        let toml_text = doc.to_string();
        assert_eq!(expected, toml_text);
    }

    #[test]
    fn apply_ci_gh_complex() {
        let expected = r#"
# CI configuration for dist
[dist.ci]

# Configure generated workflows for GitHub CI
[dist.ci.github]
# Path to a file containing a YAML array of steps, to be performed before 'dist build'
# KNOWN BUG: https://github.com/axodotdev/cargo-dist/issues/1750
build-setup = "some-build-setup"
"#;

        let ci = Some(CiLayer {
            common: CommonCiLayer::default(),
            github: Some(BoolOr::Val(GithubCiLayer {
                common: CommonCiLayer::default(),
                build_setup: Some("some-build-setup".to_string()),
                permissions: None,
                runners: None,
            })),
        });

        let mut doc = source();
        let table = dist_table(&mut doc);

        apply(table, &ci);

        let toml_text = doc.to_string();
        assert_eq!(expected, toml_text);
    }
}
