//! CI script generation
//!
//! In the future this may get split up into submodules.

// FIXME(#283): migrate this to minijinja (steal logic from oranda to load a whole dir)

use axoasset::LocalAsset;
use serde::Serialize;
use tracing::warn;

use crate::{
    backend::templates::TEMPLATE_CI_GITHUB, DistGraph, SortedMap, SortedSet, TargetTriple,
};

#[derive(Debug, Serialize)]
struct CiInfo {
    rust_version: Option<String>,
    install_dist_sh: String,
    install_dist_ps1: String,
    fail_fast: bool,
    local_tasks: Vec<CiTask>,
    global_task: Option<CiTask>,
}

#[derive(Debug, Serialize)]
struct CiTask {
    runner: String,
    dist_args: String,
    install_dist: String,
}

/// Generate CI for Github
///
/// This actually creates a file and writes to disk!
pub fn generate_github_ci(dist: &DistGraph) -> Result<(), miette::Report> {
    const GITHUB_CI_DIR: &str = ".github/workflows/";
    const GITHUB_CI_FILE: &str = "release.yml";

    // FIXME: should we try to avoid clobbering old files..?
    let ci_dir = dist.workspace_dir.join(GITHUB_CI_DIR);
    let ci_file = ci_dir.join(GITHUB_CI_FILE);
    let ci_info = compute_ci_info(dist);

    let rendered = dist
        .templates
        .render_file_to_clean_string(TEMPLATE_CI_GITHUB, &ci_info)?;
    LocalAsset::write_new_all(&rendered, &ci_file)?;
    eprintln!("generated Github CI to {}", ci_file);

    Ok(())
}

/// Write the Github CI to something
fn compute_ci_info(dist: &DistGraph) -> CiInfo {
    // Legacy deprecated support for
    let rust_version = dist.desired_rust_toolchain.clone();

    // If they don't specify a cargo-dist version, use this one
    let self_dist_version = super::SELF_DIST_VERSION.parse().unwrap();
    let dist_version = dist
        .desired_cargo_dist_version
        .as_ref()
        .unwrap_or(&self_dist_version);
    let fail_fast = dist.fail_fast;

    // Figure out what builds we need to do
    let mut needs_global_build = false;
    let mut local_targets = SortedSet::new();
    for release in &dist.releases {
        if !release.global_artifacts.is_empty() {
            needs_global_build = true;
        }
        local_targets.extend(release.targets.iter());
    }

    // Get the platform-specific installation methods
    let install_dist_sh = super::install_dist_sh_for_version(dist_version);
    let install_dist_ps1 = super::install_dist_ps1_for_version(dist_version);

    // Build up the task matrix for building Artifacts
    let mut local_tasks = vec![];

    // If we have Global Artifacts, we need one task for that. If we've done a Good Job
    // then these artifacts should be possible to build on *any* platform. Linux is usually
    // fast/cheap, so that's a reasonable choice.s
    let global_task = if needs_global_build {
        Some(CiTask {
            runner: GITHUB_LINUX_RUNNER.into(),
            dist_args: "--artifacts=global".into(),
            install_dist: install_dist_sh.clone(),
        })
    } else {
        None
    };

    // Figure out what Local Artifact tasks we need
    let local_runs = if dist.merge_tasks {
        distribute_targets_to_runners_merged(local_targets)
    } else {
        distribute_targets_to_runners_split(local_targets)
    };
    for (runner, targets) in local_runs {
        use std::fmt::Write;
        let install_dist =
            install_dist_for_github_runner(runner, &install_dist_sh, &install_dist_ps1);
        let mut dist_args = String::from("--artifacts=local");
        for target in targets {
            write!(dist_args, " --target={target}").unwrap();
        }
        local_tasks.push(CiTask {
            runner: runner.to_owned(),
            dist_args,
            install_dist: install_dist.to_owned(),
        });
    }

    CiInfo {
        rust_version,
        install_dist_sh,
        install_dist_ps1,
        fail_fast,
        local_tasks,
        global_task,
    }
}

/// Given a set of targets we want to build local artifacts for, map them to Github Runners
/// while preferring to merge builds that can happen on the same machine.
///
/// This optimizes for machine-hours, at the cost of latency and fault-isolation.
///
/// Typically this will result in both x64 macos and arm64 macos getting shoved onto
/// the same runner, making the entire release process get bottlenecked on the twice-as-long
/// macos builds. It also makes it impossible to have one macos build fail and the other
/// succeed (uploading itself to the draft release).
///
/// In priniciple it does remove some duplicated setup work, so this is ostensibly "cheaper".
fn distribute_targets_to_runners_merged(
    targets: SortedSet<&TargetTriple>,
) -> std::vec::IntoIter<(GithubRunner, Vec<&TargetTriple>)> {
    let mut groups = SortedMap::<GithubRunner, Vec<&TargetTriple>>::new();
    for target in targets {
        let runner = github_runner_for_target(target);
        let runner = runner.unwrap_or_else(|| {
            let default = GITHUB_LINUX_RUNNER;
            warn!("not sure which github runner should be used for {target}, assuming {default}");
            default
        });
        groups.entry(runner).or_default().push(target);
    }
    // This extra into_iter+collect is needed to make this have the same
    // return type as distribute_targets_to_runners_split
    groups.into_iter().collect::<Vec<_>>().into_iter()
}

/// Given a set of targets we want to build local artifacts for, map them to Github Runners
/// while preferring each target gets its own runner for latency and fault-isolation.
fn distribute_targets_to_runners_split(
    targets: SortedSet<&TargetTriple>,
) -> std::vec::IntoIter<(GithubRunner, Vec<&TargetTriple>)> {
    let mut groups = vec![];
    for target in targets {
        let runner = github_runner_for_target(target);
        let runner = runner.unwrap_or_else(|| {
            let default = GITHUB_LINUX_RUNNER;
            warn!("not sure which github runner should be used for {target}, assuming {default}");
            default
        });
        groups.push((runner, vec![target]));
    }
    groups.into_iter()
}

/// A string representing a Github Runner
type GithubRunner = &'static str;
/// The Github Runner to use for Linux
const GITHUB_LINUX_RUNNER: &str = "ubuntu-20.04";
/// The Github Runner to use for macos
const GITHUB_MACOS_RUNNER: &str = "macos-11";
/// The Github Runner to use for windows
const GITHUB_WINDOWS_RUNNER: &str = "windows-2019";

/// Get the appropriate Github Runner for building a target
fn github_runner_for_target(target: &TargetTriple) -> Option<GithubRunner> {
    // We want to default to older runners to minimize the places
    // where random system dependencies can creep in and be very
    // recent. This helps with portability!
    if target.contains("linux") {
        Some(GITHUB_LINUX_RUNNER)
    } else if target.contains("apple") {
        Some(GITHUB_MACOS_RUNNER)
    } else if target.contains("windows") {
        Some(GITHUB_WINDOWS_RUNNER)
    } else {
        None
    }
}

/// Select the cargo-dist installer approach for a given Github Runner
fn install_dist_for_github_runner<'a>(
    runner: GithubRunner,
    install_sh: &'a str,
    install_ps1: &'a str,
) -> &'a str {
    if runner == GITHUB_LINUX_RUNNER || runner == GITHUB_MACOS_RUNNER {
        install_sh
    } else if runner == GITHUB_WINDOWS_RUNNER {
        install_ps1
    } else {
        unreachable!("internal error: unknown github runner!?")
    }
}
