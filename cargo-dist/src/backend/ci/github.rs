//! CI script generation
//!
//! In the future this may get split up into submodules.

use std::collections::HashMap;

use axoasset::LocalAsset;
use cargo_dist_schema::{GithubMatrix, GithubMatrixEntry};
use serde::Serialize;
use tracing::warn;

use crate::{
    backend::{diff_files, templates::TEMPLATE_CI_GITHUB},
    config::{
        DependencyKind, HostingStyle, JinjaGithubRepoPair, ProductionMode, SystemDependencies,
    },
    errors::DistResult,
    DistGraph, SortedMap, SortedSet, TargetTriple,
};

const GITHUB_CI_DIR: &str = ".github/workflows/";
const GITHUB_CI_FILE: &str = "release.yml";

/// Info about running cargo-dist in Github CI
#[derive(Debug, Serialize)]
pub struct GithubCiInfo {
    /// Version of rust toolchain to install (deprecated)
    pub rust_version: Option<String>,
    /// expression to use for installing cargo-dist via shell script
    pub install_dist_sh: String,
    /// expression to use for installing cargo-dist via powershell script
    pub install_dist_ps1: String,
    /// Whether to fail-fast
    pub fail_fast: bool,
    /// Whether to include builtin local artifacts tasks
    pub build_local_artifacts: bool,
    /// Whether to make CI get dispatched manually instead of by tag
    pub dispatch_releases: bool,
    /// Matrix for upload-local-artifacts
    pub artifacts_matrix: cargo_dist_schema::GithubMatrix,
    /// What kind of job to run on pull request
    pub pr_run_mode: cargo_dist_schema::PrRunMode,
    /// global task
    pub global_task: GithubMatrixEntry,
    /// homebrew tap
    pub tap: Option<String>,
    /// plan jobs
    pub plan_jobs: Vec<String>,
    /// local artifacts jobs
    pub local_artifacts_jobs: Vec<String>,
    /// global artifacts jobs
    pub global_artifacts_jobs: Vec<String>,
    /// host jobs
    pub host_jobs: Vec<String>,
    /// publish jobs
    pub publish_jobs: Vec<String>,
    /// user-specified publish jobs
    pub user_publish_jobs: Vec<String>,
    /// post-announce jobs
    pub post_announce_jobs: Vec<String>,
    /// whether to create the release or assume an existing one
    pub create_release: bool,
    /// external repo to release to
    pub github_releases_repo: Option<JinjaGithubRepoPair>,
    /// \[unstable\] whether to add ssl.com windows binary signing
    pub ssldotcom_windows_sign: Option<ProductionMode>,
    /// what hosting provider we're using
    pub hosting_providers: Vec<HostingStyle>,
    /// whether to prefix release.yml and the tag pattern
    pub tag_namespace: Option<String>,
}

impl GithubCiInfo {
    /// Compute the Github CI stuff
    pub fn new(dist: &DistGraph) -> GithubCiInfo {
        // Legacy deprecated support
        let rust_version = dist.desired_rust_toolchain.clone();

        // If they don't specify a cargo-dist version, use this one
        let self_dist_version = super::SELF_DIST_VERSION.parse().unwrap();
        let dist_version = dist
            .desired_cargo_dist_version
            .as_ref()
            .unwrap_or(&self_dist_version);
        let fail_fast = dist.fail_fast;
        let build_local_artifacts = dist.build_local_artifacts;
        let dispatch_releases = dist.dispatch_releases;
        let create_release = dist.create_release;
        let github_releases_repo = dist.github_releases_repo.clone().map(|r| r.into_jinja());
        let ssldotcom_windows_sign = dist.ssldotcom_windows_sign.clone();
        let tag_namespace = dist.tag_namespace.clone();
        let mut dependencies = SystemDependencies::default();

        // Figure out what builds we need to do
        let mut local_targets = SortedSet::new();
        for release in &dist.releases {
            local_targets.extend(release.targets.iter());
            dependencies.append(&mut release.system_dependencies.clone());
        }

        // Get the platform-specific installation methods
        let install_dist_sh = super::install_dist_sh_for_version(dist_version);
        let install_dist_ps1 = super::install_dist_ps1_for_version(dist_version);
        let hosting_providers = dist
            .hosting
            .as_ref()
            .expect("should not be possible to have the Github CI backend without hosting!?")
            .hosts
            .clone();

        // Build up the task matrix for building Artifacts
        let mut tasks = vec![];

        // The global task is responsible for:
        //
        // 1. building "global artifacts" like platform-agnostic installers
        // 2. stitching together dist-manifests from all builds to produce a final one
        //
        // If we've done a Good Job, then these artifacts should be possible to build on *any*
        // platform. Linux is usually fast/cheap, so that's a reasonable choice.
        let global_task = GithubMatrixEntry {
            targets: None,
            runner: Some(GITHUB_LINUX_RUNNER.into()),
            dist_args: Some("--artifacts=global".into()),
            install_dist: Some(install_dist_sh.clone()),
            packages_install: None,
        };

        let pr_run_mode = dist.pr_run_mode;

        let tap = dist.tap.clone();
        let plan_jobs = dist.plan_jobs.clone();
        let local_artifacts_jobs = dist.local_artifacts_jobs.clone();
        let global_artifacts_jobs = dist.global_artifacts_jobs.clone();
        let host_jobs = dist.host_jobs.clone();
        let publish_jobs = dist.publish_jobs.iter().map(|j| j.to_string()).collect();
        let user_publish_jobs = dist.user_publish_jobs.clone();
        let post_announce_jobs = dist.post_announce_jobs.clone();

        // Figure out what Local Artifact tasks we need
        let local_runs = if dist.merge_tasks {
            distribute_targets_to_runners_merged(local_targets, &dist.github_custom_runners)
        } else {
            distribute_targets_to_runners_split(local_targets, &dist.github_custom_runners)
        };
        for (runner, targets) in local_runs {
            use std::fmt::Write;
            let install_dist =
                install_dist_for_targets(&targets, &install_dist_sh, &install_dist_ps1);
            let mut dist_args = String::from("--artifacts=local");
            for target in &targets {
                write!(dist_args, " --target={target}").unwrap();
            }
            tasks.push(GithubMatrixEntry {
                targets: Some(targets.iter().map(|s| s.to_string()).collect()),
                runner: Some(runner.to_owned()),
                dist_args: Some(dist_args),
                install_dist: Some(install_dist.to_owned()),
                packages_install: package_install_for_targets(&targets, &dependencies),
            });
        }

        GithubCiInfo {
            tag_namespace,
            rust_version,
            install_dist_sh,
            install_dist_ps1,
            fail_fast,
            build_local_artifacts,
            dispatch_releases,
            tap,
            plan_jobs,
            local_artifacts_jobs,
            global_artifacts_jobs,
            host_jobs,
            publish_jobs,
            user_publish_jobs,
            post_announce_jobs,
            artifacts_matrix: GithubMatrix { include: tasks },
            pr_run_mode,
            global_task,
            create_release,
            github_releases_repo,
            ssldotcom_windows_sign,
            hosting_providers,
        }
    }

    fn github_ci_path(&self, dist: &DistGraph) -> camino::Utf8PathBuf {
        let ci_dir = dist.workspace_dir.join(GITHUB_CI_DIR);
        // If tag-namespace is set, apply the prefix to the filename to emphasize it's
        // just one of many workflows in this project
        let prefix = self
            .tag_namespace
            .as_deref()
            .map(|p| format!("{p}-"))
            .unwrap_or_default();
        ci_dir.join(format!("{prefix}{GITHUB_CI_FILE}"))
    }

    /// Generate the requested configuration and returns it as a string.
    pub fn generate_github_ci(&self, dist: &DistGraph) -> DistResult<String> {
        let rendered = dist
            .templates
            .render_file_to_clean_string(TEMPLATE_CI_GITHUB, self)?;

        Ok(rendered)
    }

    /// Write release.yml to disk
    pub fn write_to_disk(&self, dist: &DistGraph) -> DistResult<()> {
        let ci_file = self.github_ci_path(dist);
        let rendered = self.generate_github_ci(dist)?;

        LocalAsset::write_new_all(&rendered, &ci_file)?;
        eprintln!("generated Github CI to {}", ci_file);

        Ok(())
    }

    /// Check whether the new configuration differs from the config on disk
    /// writhout actually writing the result.
    pub fn check(&self, dist: &DistGraph) -> DistResult<()> {
        let ci_file = self.github_ci_path(dist);

        let rendered = self.generate_github_ci(dist)?;
        diff_files(&ci_file, &rendered)
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
fn distribute_targets_to_runners_merged<'a>(
    targets: SortedSet<&'a TargetTriple>,
    custom_runners: &HashMap<String, String>,
) -> std::vec::IntoIter<(GithubRunner, Vec<&'a TargetTriple>)> {
    let mut groups = SortedMap::<GithubRunner, Vec<&TargetTriple>>::new();
    for target in targets {
        let runner = github_runner_for_target(target, custom_runners);
        let runner = runner.unwrap_or_else(|| {
            let default = GITHUB_LINUX_RUNNER;
            warn!("not sure which github runner should be used for {target}, assuming {default}");
            default.to_owned()
        });
        groups.entry(runner).or_default().push(target);
    }
    // This extra into_iter+collect is needed to make this have the same
    // return type as distribute_targets_to_runners_split
    groups.into_iter().collect::<Vec<_>>().into_iter()
}

/// Given a set of targets we want to build local artifacts for, map them to Github Runners
/// while preferring each target gets its own runner for latency and fault-isolation.
fn distribute_targets_to_runners_split<'a>(
    targets: SortedSet<&'a TargetTriple>,
    custom_runners: &HashMap<String, String>,
) -> std::vec::IntoIter<(GithubRunner, Vec<&'a TargetTriple>)> {
    let mut groups = vec![];
    for target in targets {
        let runner = github_runner_for_target(target, custom_runners);
        let runner = runner.unwrap_or_else(|| {
            let default = GITHUB_LINUX_RUNNER;
            warn!("not sure which github runner should be used for {target}, assuming {default}");
            default.to_owned()
        });
        groups.push((runner, vec![target]));
    }
    groups.into_iter()
}

/// A string representing a Github Runner
type GithubRunner = String;
/// The Github Runner to use for Linux
const GITHUB_LINUX_RUNNER: &str = "ubuntu-20.04";
/// The Github Runner to use for Intel macos
const GITHUB_MACOS_INTEL_RUNNER: &str = "macos-12";
/// The Github Runner to use for Apple Silicon macos
const GITHUB_MACOS_ARM64_RUNNER: &str = "macos-12";
/// The Github Runner to use for windows
const GITHUB_WINDOWS_RUNNER: &str = "windows-2019";

/// Get the appropriate Github Runner for building a target
fn github_runner_for_target(
    target: &TargetTriple,
    custom_runners: &HashMap<String, String>,
) -> Option<GithubRunner> {
    if let Some(runner) = custom_runners.get(target) {
        return Some(runner.to_owned());
    }

    // We want to default to older runners to minimize the places
    // where random system dependencies can creep in and be very
    // recent. This helps with portability!
    if target.contains("linux") {
        Some(GITHUB_LINUX_RUNNER.to_owned())
    } else if target.contains("x86_64-apple") {
        Some(GITHUB_MACOS_INTEL_RUNNER.to_owned())
    } else if target.contains("aarch64-apple") {
        Some(GITHUB_MACOS_ARM64_RUNNER.to_owned())
    } else if target.contains("windows") {
        Some(GITHUB_WINDOWS_RUNNER.to_owned())
    } else {
        None
    }
}

/// Select the cargo-dist installer approach for a given Github Runner
fn install_dist_for_targets<'a>(
    targets: &'a [&'a TargetTriple],
    install_sh: &'a str,
    install_ps1: &'a str,
) -> &'a str {
    for target in targets {
        if target.contains("linux") || target.contains("apple") {
            return install_sh;
        } else if target.contains("windows") {
            return install_ps1;
        }
    }

    unreachable!("internal error: unknown target triple!?")
}

fn brewfile_from(packages: &[String]) -> String {
    let brewfile_lines: Vec<String> = packages
        .iter()
        .map(|p| {
            let lower = p.to_ascii_lowercase();
            // Although `brew install` can take either a formula or a cask,
            // Brewfiles require you to use the `cask` verb for casks and `brew`
            // for formulas.
            if lower.starts_with("homebrew/cask") || lower.starts_with("homebrew/homebrew-cask") {
                format!(r#"cask "{p}""#).to_owned()
            } else {
                format!(r#"brew "{p}""#).to_owned()
            }
        })
        .collect();

    brewfile_lines.join("\n")
}

fn brew_bundle_command(packages: &[String]) -> String {
    format!(
        r#"cat << EOF >Brewfile
{}
EOF

brew bundle install"#,
        brewfile_from(packages)
    )
}

fn package_install_for_targets(
    targets: &Vec<&TargetTriple>,
    packages: &SystemDependencies,
) -> Option<String> {
    // TODO handle mixed-OS targets
    for target in targets {
        match target.as_str() {
            "i686-apple-darwin" | "x86_64-apple-darwin" | "aarch64-apple-darwin" => {
                let packages: Vec<String> = packages
                    .homebrew
                    .clone()
                    .into_iter()
                    .filter(|(_, package)| package.0.wanted_for_target(target))
                    .filter(|(_, package)| package.0.stage_wanted(&DependencyKind::Build))
                    .map(|(name, _)| name)
                    .collect();

                if packages.is_empty() {
                    return None;
                }

                return Some(brew_bundle_command(&packages));
            }
            "i686-unknown-linux-gnu"
            | "x86_64-unknown-linux-gnu"
            | "aarch64-unknown-linux-gnu"
            | "i686-unknown-linux-musl"
            | "x86_64-unknown-linux-musl"
            | "aarch64-unknown-linux-musl" => {
                let mut packages: Vec<String> = packages
                    .apt
                    .clone()
                    .into_iter()
                    .filter(|(_, package)| package.0.wanted_for_target(target))
                    .filter(|(_, package)| package.0.stage_wanted(&DependencyKind::Build))
                    .map(|(name, spec)| {
                        if let Some(version) = spec.0.version {
                            format!("{name}={version}")
                        } else {
                            name
                        }
                    })
                    .collect();

                // musl builds may require musl-tools to build;
                // necessary for more complex software
                if target.ends_with("linux-musl") {
                    packages.push("musl-tools".to_owned());
                }

                if packages.is_empty() {
                    return None;
                }

                let apts = packages.join(" ");
                return Some(
                    format!("sudo apt-get update && sudo apt-get install {apts}").to_owned(),
                );
            }
            "i686-pc-windows-msvc" | "x86_64-pc-windows-msvc" | "aarch64-pc-windows-msvc" => {
                let commands: Vec<String> = packages
                    .chocolatey
                    .clone()
                    .into_iter()
                    .filter(|(_, package)| package.0.wanted_for_target(target))
                    .filter(|(_, package)| package.0.stage_wanted(&DependencyKind::Build))
                    .map(|(name, package)| {
                        if let Some(version) = package.0.version {
                            format!("choco install {name} --version={version}")
                        } else {
                            format!("choco install {name}")
                        }
                    })
                    .collect();

                if commands.is_empty() {
                    return None;
                }

                return Some(commands.join("\n"));
            }
            _ => {}
        }
    }

    None
}
