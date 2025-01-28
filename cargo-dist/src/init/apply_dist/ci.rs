use axoasset::toml_edit;
use crate::config::v1::ci::{CiLayer, CommonCiLayer};
use crate::config::v1::ci::github::GithubCiLayer;
use crate::config::v1::layer::{BoolOr, BoolOrOptExt};
use super::helpers::*;


pub fn apply(table: &mut toml_edit::Table, ci: &Option<CiLayer>) {
    let Some(ci) = ci else {
        // Nothing to do.
        return;
    };
    let Some(ci_table) = table.get_mut("ci") else {
        // Nothing to do.
        return;
    };
    let toml_edit::Item::Table(ci_table) = ci_table else {
        panic!("Expected [dist.ci] to be a table");
    };

    apply_ci_common(ci_table, &ci.common);

    if let Some(github) = &ci.github {
        match github {
            BoolOr::Bool(b) => {
                apply_optional_value(
                    ci_table,
                    "github",
                    "# Whether to use GitHub CI\n",
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
    let Some(gh_table) = ci_table.get_mut("github") else {
        return;
    };
    let toml_edit::Item::Table(gh_table) = gh_table else {
        panic!("Expected [dist.ci.github] to be a table");
    };

    apply_ci_common(gh_table, &github.common);

    // FIXME(migration): make these actually compile.
    /*
    apply_optional_value(
        gh_table,
        "runners",
        "# Custom GitHub runners, specified as target triples\n",
        github.runners,
    );

    apply_optional_value(
        gh_table,
        "permissions",
        "# Custom permissions for jobs\n",
        github.permissions,
    );

    apply_optional_value(
        gh_table,
        "build-setup",
        "# Custom permissions for jobs\n",
        github.build_setup,
    );
    */

    // Finalize the table
    gh_table
        .decor_mut()
        .set_prefix("\n# Configure GitHub CI\n");
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
