//! CI script generation
//!
//! In the future this may get split up into submodules.

use std::fs::File;

use miette::{IntoDiagnostic, WrapErr};
use newline_converter::dos2unix;
use semver::Version;
use tracing::{info, warn};

use crate::{DistGraph, SortedMap, SortedSet, TargetTriple};

/// Generate CI for Github
///
/// This actually creates a file and writes to disk!
pub fn generate_github_ci(dist: &DistGraph) -> Result<(), miette::Report> {
    const GITHUB_CI_DIR: &str = ".github/workflows/";
    const GITHUB_CI_FILE: &str = "release.yml";

    // FIXME: should we try to avoid clobbering old files..?
    let ci_dir = dist.workspace_dir.join(GITHUB_CI_DIR);
    let ci_file = ci_dir.join(GITHUB_CI_FILE);

    info!("generating Github CI at {ci_file}");
    std::fs::create_dir_all(&ci_dir)
        .into_diagnostic()
        .wrap_err("Failed to create ci dir")?;
    let mut file = File::create(&ci_file)
        .into_diagnostic()
        .wrap_err("Failed to create ci file")?;
    write_github_ci(&mut file, dist)
        .into_diagnostic()
        .wrap_err("Failed to write to CI file")?;
    eprintln!("generated Github CI to {}", ci_file);
    Ok(())
}

/// Write the Github CI to something
fn write_github_ci<W: std::io::Write>(f: &mut W, dist: &DistGraph) -> Result<(), std::io::Error> {
    // If they don't specify a Rust version, just go for "stable"
    let rust_version = dist.desired_rust_toolchain.as_deref();

    // If they don't specify a cargo-dist version, use this one
    let self_dist_version = SELF_DIST_VERSION.parse().unwrap();
    let dist_version = dist
        .desired_cargo_dist_version
        .as_ref()
        .unwrap_or(&self_dist_version);

    // Figue out what builds we need to do
    let mut needs_global_build = false;
    let mut local_targets = SortedSet::new();
    for release in &dist.releases {
        if !release.global_artifacts.is_empty() {
            needs_global_build = true;
        }
        local_targets.extend(release.targets.iter());
    }

    // Install Rust with rustup (deprecated, use rust-toolchain.toml)
    //
    // We pass --no-self-update to work around https://github.com/rust-lang/rustup/issues/2441
    //
    // If not specified we just let default toolchains on the system be used
    // (rust-toolchain.toml will override things automagically if the system uses rustup,
    // because rustup intercepts all commands like `cargo` and `rustc` to reselect the toolchain)
    let install_rust = rust_version
        .map(|rust_version| {
            format!(
                r#"
      - name: Install Rust
        run: rustup update {rust_version} --no-self-update && rustup default {rust_version}"#
            )
        })
        .unwrap_or(String::new());

    // Get the platform-specific installation methods
    let install_dist_sh = install_dist_sh_for_version(dist_version);
    let install_dist_ps1 = install_dist_ps1_for_version(dist_version);

    // Build up the task matrix for building Artifacts
    let mut artifacts_matrix = String::from("include:");

    // If we have Global Artifacts, we need one task for that. If we've done a Good Job
    // then these artifacts should be possible to build on *any* platform. Linux is usually
    // fast/cheap, so that's a reasonable choice.s
    if needs_global_build {
        push_github_artifacts_matrix_entry(
            &mut artifacts_matrix,
            GITHUB_LINUX_RUNNER,
            "--artifacts=global",
            &install_dist_sh,
        );
    }

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
        push_github_artifacts_matrix_entry(&mut artifacts_matrix, runner, &dist_args, install_dist);
    }

    // Finally write the final CI script to the Writer
    let ci_yml = include_str!("../templates/ci.yml");
    let ci_yml = ci_yml
        .replace("{{{{INSTALL_RUST}}}}", &install_rust)
        .replace("{{{{INSTALL_DIST_SH}}}}", &install_dist_sh)
        .replace("{{{{ARTIFACTS_MATRIX}}}}", &artifacts_matrix);

    f.write_all(dos2unix(&ci_yml).as_bytes())?;

    Ok(())
}

/// Add an entry to a Github Matrix (for the Artifacts tasks)
fn push_github_artifacts_matrix_entry(
    matrix: &mut String,
    runner: &str,
    dist_args: &str,
    install_dist: &str,
) {
    use std::fmt::Write;

    const MATRIX_ENTRY_TEMPLATE: &str = r###"
        - os: {{{{GITHUB_RUNNER}}}}
          dist-args: {{{{DIST_ARGS}}}}
          install-dist: {{{{INSTALL_DIST}}}}"###;

    let entry = MATRIX_ENTRY_TEMPLATE
        .replace("{{{{GITHUB_RUNNER}}}}", runner)
        .replace("{{{{DIST_ARGS}}}}", dist_args)
        .replace("{{{{INSTALL_DIST}}}}", install_dist);

    write!(matrix, "{}", entry).unwrap();
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

/// The current version of cargo-dist
const SELF_DIST_VERSION: &str = env!("CARGO_PKG_VERSION");
const BASE_DIST_FETCH_URL: &str = "https://github.com/axodotdev/cargo-dist/releases/download";

/// Get the command to invoke to install cargo-dist via sh script
fn install_dist_sh_for_version(version: &Version) -> String {
    if let Some(git) = install_dist_git(version) {
        return git;
    }
    let format = cargo_dist_schema::format_of_version(version);
    let installer_name = if format.unsupported() {
        // FIXME: we should probably do this check way higher up and produce a proper err...
        panic!("requested cargo-dist v{version}, which is not supported by the this copy of cargo-dist ({SELF_DIST_VERSION})");
    } else if format.artifact_names_contain_versions() {
        format!("cargo-dist-v{version}-installer.sh")
    } else {
        "cargo-dist-installer.sh".to_owned()
    };

    // FIXME: it would be nice if these values were somehow using all the machinery
    // to compute these values for packages we build *BUT* it's messy and not that important
    let installer_url = format!("{BASE_DIST_FETCH_URL}/v{version}/{installer_name}");
    format!("curl --proto '=https' --tlsv1.2 -LsSf {installer_url} | sh")
}

/// Get the command to invoke to install cargo-dist via ps1 script
fn install_dist_ps1_for_version(version: &Version) -> String {
    if let Some(git) = install_dist_git(version) {
        return git;
    }
    let format = cargo_dist_schema::format_of_version(version);
    let installer_name = if format.unsupported() {
        // FIXME: we should probably do this check way higher up and produce a proper err...
        panic!("requested cargo-dist v{version}, which is not supported by the this copy of cargo-dist ({SELF_DIST_VERSION})");
    } else if format.artifact_names_contain_versions() {
        format!("cargo-dist-v{version}-installer.ps1")
    } else {
        "cargo-dist-installer.ps1".to_owned()
    };

    // FIXME: it would be nice if these values were somehow using all the machinery
    // to compute these values for packages we build *BUT* it's messy and not that important
    let installer_url = format!("{BASE_DIST_FETCH_URL}/v{version}/{installer_name}");
    format!("irm  {installer_url} | iex")
}

/// Cute little hack for developing dist itself: if we see a version like "0.0.3-github-config"
/// then install from the main github repo with branch=config!
fn install_dist_git(version: &Version) -> Option<String> {
    version.pre.strip_prefix("github-").map(|branch| {
        format!("cargo install --git https://github.com/axodotdev/cargo-dist/ --branch={branch} cargo-dist")
    })
}
