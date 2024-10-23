//! CI script generation
//!
//! In the future this may get split up into submodules.

use std::collections::BTreeMap;

use axoasset::{LocalAsset, SourceFile};
use axoprocess::Cmd;
use camino::{Utf8Path, Utf8PathBuf};
use cargo_dist_schema::{GithubMatrix, GithubMatrixEntry, TargetTriple, TargetTripleRef};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::{
    backend::{diff_files, templates::TEMPLATE_CI_GITHUB},
    config::{
        v1::{ci::github::GithubCiConfig, publishers::PublisherConfig},
        DependencyKind, GithubPermission, GithubPermissionMap, GithubReleasePhase, HostingStyle,
        JinjaGithubRepoPair, JobStyle, ProductionMode, PublishStyle, SystemDependencies,
    },
    errors::DistResult,
    DistError, DistGraph, SortedMap, SortedSet,
};

#[cfg(not(windows))]
const GITHUB_CI_DIR: &str = ".github/workflows/";
#[cfg(windows)]
const GITHUB_CI_DIR: &str = r".github\workflows\";
const GITHUB_CI_FILE: &str = "release.yml";

/// Info about running cargo-dist in Github CI
#[derive(Debug, Serialize)]
pub struct GithubCiInfo {
    /// Cached path to github CI workflows dir
    #[serde(skip_serializing)]
    pub github_ci_workflow_dir: Utf8PathBuf,
    /// Version of rust toolchain to install (deprecated)
    pub rust_version: Option<String>,
    /// expression to use for installing cargo-dist via shell script
    pub install_dist_sh: String,
    /// expression to use for installing cargo-dist via powershell script
    pub install_dist_ps1: String,
    /// Whether to fail-fast
    pub fail_fast: bool,
    /// Whether to cache builds
    pub cache_builds: bool,
    /// Whether to include builtin local artifacts tasks
    pub build_local_artifacts: bool,
    /// Whether to make CI get dispatched manually instead of by tag
    pub dispatch_releases: bool,
    /// Trigger releases on pushes to this branch instead of ci
    pub release_branch: Option<String>,
    /// Matrix for upload-local-artifacts
    pub artifacts_matrix: cargo_dist_schema::GithubMatrix,
    /// What kind of job to run on pull request
    pub pr_run_mode: cargo_dist_schema::PrRunMode,
    /// global task
    pub global_task: GithubMatrixEntry,
    /// homebrew tap
    pub tap: Option<String>,
    /// plan jobs
    pub plan_jobs: Vec<GithubCiJob>,
    /// local artifacts jobs
    pub local_artifacts_jobs: Vec<GithubCiJob>,
    /// global artifacts jobs
    pub global_artifacts_jobs: Vec<GithubCiJob>,
    /// host jobs
    pub host_jobs: Vec<GithubCiJob>,
    /// publish jobs
    pub publish_jobs: Vec<String>,
    /// user-specified publish jobs
    pub user_publish_jobs: Vec<GithubCiJob>,
    /// post-announce jobs
    pub post_announce_jobs: Vec<GithubCiJob>,
    /// \[unstable\] whether to add ssl.com windows binary signing
    pub ssldotcom_windows_sign: Option<ProductionMode>,
    /// Whether to enable macOS codesigning
    pub macos_sign: bool,
    /// what hosting provider we're using
    pub hosting_providers: Vec<HostingStyle>,
    /// whether to prefix release.yml and the tag pattern
    pub tag_namespace: Option<String>,
    /// Extra permissions the workflow file should have
    pub root_permissions: Option<GithubPermissionMap>,
    /// Extra build steps
    pub github_build_setup: Vec<GithubJobStep>,
    /// Info about making a GitHub Release (if we're making one)
    #[serde(flatten)]
    pub github_release: Option<GithubReleaseInfo>,
}

/// Details for github releases
#[derive(Debug, Serialize)]
pub struct GithubReleaseInfo {
    /// whether to create the release or assume an existing one
    pub create_release: bool,
    /// external repo to release to
    pub github_releases_repo: Option<JinjaGithubRepoPair>,
    /// commit to use for github_release_repo
    pub external_repo_commit: Option<String>,
    /// Whether to enable GitHub Attestations
    pub github_attestations: bool,
    /// `gh` command to run to create the release
    pub release_command: String,
    /// Which phase to create the release at
    pub release_phase: GithubReleasePhase,
}

/// A github action workflow step
#[derive(Debug, Clone, Serialize, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct GithubJobStep {
    /// A step's ID for looking up any outputs in a later step
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// If this step should run
    #[serde(default)]
    #[serde(rename = "if")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub if_expr: Option<serde_json::Value>,

    /// The name of this step
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// The identifier for a marketplace action or relative path for a repo hosted action
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uses: Option<String>,

    /// A script to run
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,

    /// The working directory this action sould run
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,

    /// The shell name to run the `run` property in e.g. bash or powershell
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,

    /// A map of action arguments
    #[serde(default)]
    pub with: BTreeMap<String, serde_json::Value>,

    /// Environment variables for this step
    #[serde(default)]
    pub env: BTreeMap<String, serde_json::Value>,

    /// If this job should continue if this step errors
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continue_on_error: Option<serde_json::Value>,

    /// Maximum number of minutes this step should take
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_minutes: Option<serde_json::Value>,
}

/// A custom ci job
#[derive(Debug, Serialize)]
pub struct GithubCiJob {
    /// Name of the job
    pub name: String,
    /// Permisions to give the job
    pub permissions: Option<GithubPermissionMap>,
}

impl GithubCiInfo {
    /// Compute the Github CI stuff
    pub fn new(dist: &DistGraph, ci_config: &GithubCiConfig) -> DistResult<GithubCiInfo> {
        // Legacy deprecated support
        let rust_version = dist.config.builds.cargo.rust_toolchain_version.clone();

        // If they don't specify a cargo-dist version, use this one
        let self_dist_version = super::SELF_DIST_VERSION.parse().unwrap();
        let dist_version = dist
            .config
            .dist_version
            .as_ref()
            .unwrap_or(&self_dist_version);
        let fail_fast = ci_config.fail_fast;
        let cache_builds = ci_config.cache_builds;
        let build_local_artifacts = ci_config.build_local_artifacts;
        let dispatch_releases = ci_config.dispatch_releases;
        let release_branch = ci_config.release_branch.clone();
        let ssldotcom_windows_sign = dist.config.builds.ssldotcom_windows_sign.clone();
        let macos_sign = dist.config.builds.macos_sign;
        let tag_namespace = ci_config.tag_namespace.clone();
        let pr_run_mode = ci_config.pr_run_mode;

        let github_release = GithubReleaseInfo::new(dist)?;
        let mut dependencies = SystemDependencies::default();

        let caching_could_be_profitable =
            release_branch.is_some() || pr_run_mode == cargo_dist_schema::PrRunMode::Upload;
        let cache_builds = cache_builds.unwrap_or(caching_could_be_profitable);

        // Figure out what builds we need to do
        let mut local_targets: SortedSet<&TargetTripleRef> = SortedSet::new();
        for release in &dist.releases {
            for target in &release.targets {
                local_targets.insert(target);
            }
            dependencies.append(&mut release.config.builds.system_dependencies.clone());
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
        let global_runner = ci_config
            .runners
            .get("global")
            .map(|s| s.as_str())
            .unwrap_or(GITHUB_LINUX_RUNNER);
        let global_task = GithubMatrixEntry {
            targets: None,
            cache_provider: cache_provider_for_runner(global_runner),
            runner: Some(global_runner.into()),
            dist_args: Some("--artifacts=global".into()),
            install_dist: Some(install_dist_sh.clone()),
            packages_install: None,
        };

        let tap = dist.global_homebrew_tap.clone();

        let mut job_permissions = ci_config.permissions.clone();
        // user publish jobs default to elevated priviledges
        for JobStyle::User(name) in &ci_config.publish_jobs {
            job_permissions.entry(name.clone()).or_insert_with(|| {
                GithubPermissionMap::from_iter([
                    ("id-token".to_owned(), GithubPermission::Write),
                    ("packages".to_owned(), GithubPermission::Write),
                ])
            });
        }

        let mut root_permissions = GithubPermissionMap::new();
        root_permissions.insert("contents".to_owned(), GithubPermission::Write);
        let has_attestations = github_release
            .as_ref()
            .map(|g| g.github_attestations)
            .unwrap_or(false);
        if has_attestations {
            root_permissions.insert("id-token".to_owned(), GithubPermission::Write);
            root_permissions.insert("attestations".to_owned(), GithubPermission::Write);
        }

        let mut publish_jobs = vec![];
        if let Some(PublisherConfig { homebrew, npm }) = &dist.global_publishers {
            if homebrew.is_some() {
                publish_jobs.push(PublishStyle::Homebrew.to_string());
            }
            if npm.is_some() {
                publish_jobs.push(PublishStyle::Npm.to_string());
            }
        }

        let plan_jobs = build_jobs(&ci_config.plan_jobs, &job_permissions)?;
        let local_artifacts_jobs = build_jobs(&ci_config.build_local_jobs, &job_permissions)?;
        let global_artifacts_jobs = build_jobs(&ci_config.build_global_jobs, &job_permissions)?;
        let host_jobs = build_jobs(&ci_config.host_jobs, &job_permissions)?;
        let user_publish_jobs = build_jobs(&ci_config.publish_jobs, &job_permissions)?;
        let post_announce_jobs = build_jobs(&ci_config.post_announce_jobs, &job_permissions)?;

        let root_permissions = (!root_permissions.is_empty()).then_some(root_permissions);

        // Figure out what Local Artifact tasks we need
        let local_runs = if ci_config.merge_tasks {
            distribute_targets_to_runners_merged(local_targets, &ci_config.runners)
        } else {
            distribute_targets_to_runners_split(local_targets, &ci_config.runners)
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
                cache_provider: cache_provider_for_runner(&runner),
                runner: Some(runner),
                dist_args: Some(dist_args),
                install_dist: Some(install_dist.to_owned()),
                packages_install: package_install_for_targets(&targets, &dependencies),
            });
        }

        let github_ci_workflow_dir = dist.repo_dir.join(GITHUB_CI_DIR);
        let github_build_setup = ci_config
            .build_setup
            .as_ref()
            .map(|local| {
                crate::backend::ci::github::GithubJobStepsBuilder::new(
                    &github_ci_workflow_dir,
                    local,
                )?
                .validate()
            })
            .transpose()?
            .unwrap_or_default();

        Ok(GithubCiInfo {
            github_ci_workflow_dir,
            tag_namespace,
            rust_version,
            install_dist_sh,
            install_dist_ps1,
            fail_fast,
            cache_builds,
            build_local_artifacts,
            dispatch_releases,
            release_branch,
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
            ssldotcom_windows_sign,
            macos_sign,
            hosting_providers,
            root_permissions,
            github_build_setup,
            github_release,
        })
    }

    fn github_ci_release_yml_path(&self) -> Utf8PathBuf {
        // If tag-namespace is set, apply the prefix to the filename to emphasize it's
        // just one of many workflows in this project
        let prefix = self
            .tag_namespace
            .as_deref()
            .map(|p| format!("{p}-"))
            .unwrap_or_default();
        self.github_ci_workflow_dir
            .join(format!("{prefix}{GITHUB_CI_FILE}"))
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
        let ci_file = self.github_ci_release_yml_path();
        let rendered = self.generate_github_ci(dist)?;

        LocalAsset::write_new_all(&rendered, &ci_file)?;
        eprintln!("generated Github CI to {}", ci_file);

        Ok(())
    }

    /// Check whether the new configuration differs from the config on disk
    /// writhout actually writing the result.
    pub fn check(&self, dist: &DistGraph) -> DistResult<()> {
        let ci_file = self.github_ci_release_yml_path();

        let rendered = self.generate_github_ci(dist)?;
        diff_files(&ci_file, &rendered)
    }
}

impl GithubReleaseInfo {
    fn new(dist: &DistGraph) -> DistResult<Option<Self>> {
        let Some(host_config) = &dist.config.hosts.github else {
            return Ok(None);
        };

        let create_release = host_config.create;
        let github_releases_repo = host_config.repo.clone().map(|r| r.into_jinja());
        let github_attestations = host_config.attestations;

        let github_releases_submodule_path = host_config.submodule_path.clone();
        let external_repo_commit = github_releases_submodule_path
            .as_ref()
            .map(submodule_head)
            .transpose()?
            .flatten();

        let release_phase = if host_config.during == GithubReleasePhase::Auto {
            // We typically prefer to release in announce.
            // If Axo is in use, we also want the release to come late
            // because the release body will contain links to Axo URLs
            // that won't become live until the announce phase.
            if dist.config.hosts.axodotdev.is_some() {
                GithubReleasePhase::Announce
            // Otherwise, if Axo isn't present, we lean on host for
            // safety reasons - because npm/Homebrew contain links to
            // URLs that won't exist until the GitHub release happens.
            } else {
                GithubReleasePhase::Host
            }
        // If the user chose a non-auto option, respect that.
        } else {
            host_config.during
        };

        let mut release_args = vec![];
        let action;
        // Always need to use the tag flag
        release_args.push("\"${{ needs.plan.outputs.tag }}\"");

        // If using remote repos, specify the repo
        if github_releases_repo.is_some() {
            release_args.push("--repo");
            release_args.push("\"$REPO\"")
        }
        release_args.push("--target");
        release_args.push("\"$RELEASE_COMMIT\"");
        release_args.push("$PRERELEASE_FLAG");
        if host_config.create {
            action = "create";
            release_args.push("--title");
            release_args.push("\"$ANNOUNCEMENT_TITLE\"");
            release_args.push("--notes-file");
            release_args.push("\"$RUNNER_TEMP/notes.txt\"");
            // When creating release, upload artifacts transactionally
            release_args.push("artifacts/*");
        } else {
            action = "edit";
            release_args.push("--draft=false");
        }
        let release_command = format!("gh release {action} {}", release_args.join(" "));

        Ok(Some(Self {
            create_release,
            github_releases_repo,
            external_repo_commit,
            github_attestations,
            release_command,
            release_phase,
        }))
    }
}

// Determines the *cached* HEAD for a submodule within the workspace.
// Note that any unstaged commits, and any local changes to commit
// history that aren't reflected by the submodule commit history,
// won't be reflected here.
fn submodule_head(submodule_path: &Utf8PathBuf) -> DistResult<Option<String>> {
    let output = Cmd::new("git", "fetch cached commit for a submodule")
        .arg("submodule")
        .arg("status")
        .arg("--cached")
        .arg(submodule_path)
        .output()
        .map_err(|_| DistError::GitSubmoduleCommitError {
            path: submodule_path.to_string(),
        })?;

    let line = String::from_utf8_lossy(&output.stdout);
    // Format: one status character, commit, a space, repo name
    let line = line.trim_start_matches([' ', '-', '+']);
    let Some((commit, _)) = line.split_once(' ') else {
        return Err(DistError::GitSubmoduleCommitError {
            path: submodule_path.to_string(),
        });
    };

    if commit.is_empty() {
        Ok(None)
    } else {
        Ok(Some(commit.to_owned()))
    }
}

fn build_jobs(
    jobs: &[JobStyle],
    perms: &SortedMap<String, GithubPermissionMap>,
) -> DistResult<Vec<GithubCiJob>> {
    let mut output = vec![];
    for JobStyle::User(name) in jobs {
        let perms_for_job = perms.get(name);

        // Create the job
        output.push(GithubCiJob {
            name: name.clone(),
            permissions: perms_for_job.cloned(),
        });
    }
    Ok(output)
}

/// Get the best `cache-provider` key to use for <https://github.com/Swatinem/rust-cache>.
///
/// In the future we might make "None" here be a way to say "disable the cache".
fn cache_provider_for_runner(runner: &str) -> Option<String> {
    if runner.contains("buildjet") {
        Some("buildjet".into())
    } else {
        Some("github".into())
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
/// In principle it does remove some duplicated setup work, so this is ostensibly "cheaper".
fn distribute_targets_to_runners_merged<'a>(
    targets: SortedSet<&'a TargetTripleRef>,
    custom_runners: &BTreeMap<TargetTriple, String>,
) -> std::vec::IntoIter<(GithubRunner, Vec<&'a TargetTripleRef>)> {
    let mut groups = SortedMap::<GithubRunner, Vec<&TargetTripleRef>>::new();
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
    targets: SortedSet<&'a TargetTripleRef>,
    custom_runners: &BTreeMap<TargetTriple, String>,
) -> std::vec::IntoIter<(GithubRunner, Vec<&'a TargetTripleRef>)> {
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
const GITHUB_MACOS_INTEL_RUNNER: &str = "macos-13";
/// The Github Runner to use for Apple Silicon macos
const GITHUB_MACOS_ARM64_RUNNER: &str = "macos-13";
/// The Github Runner to use for windows
const GITHUB_WINDOWS_RUNNER: &str = "windows-2019";

/// Get the appropriate Github Runner for building a target
fn github_runner_for_target(
    target: &TargetTripleRef,
    custom_runners: &BTreeMap<TargetTriple, String>,
) -> Option<GithubRunner> {
    if let Some(runner) = custom_runners.get(target) {
        return Some(runner.to_owned());
    }

    // We want to default to older runners to minimize the places
    // where random system dependencies can creep in and be very
    // recent. This helps with portability!
    if target.is_linux() {
        Some(GITHUB_LINUX_RUNNER.to_owned())
    } else if target.is_apple() && target.is_x86_64() {
        Some(GITHUB_MACOS_INTEL_RUNNER.to_owned())
    } else if target.is_apple() && target.is_aarch64() {
        Some(GITHUB_MACOS_ARM64_RUNNER.to_owned())
    } else if target.is_windows() {
        Some(GITHUB_WINDOWS_RUNNER.to_owned())
    } else {
        None
    }
}

/// Select the cargo-dist installer approach for a given Github Runner
fn install_dist_for_targets<'a>(
    targets: &'a [&'a TargetTripleRef],
    install_sh: &'a str,
    install_ps1: &'a str,
) -> &'a str {
    for target in targets {
        if target.is_linux() || target.is_apple() {
            return install_sh;
        } else if target.is_windows() {
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
    targets: &[&TargetTripleRef],
    packages: &SystemDependencies,
) -> Option<String> {
    // FIXME?: handle mixed-OS targets
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
                if target.is_linux_musl() {
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

/// Builder for looking up and reporting errors in the steps provided by the
/// `github-build-setup` configuration
pub struct GithubJobStepsBuilder {
    steps: Vec<GithubJobStep>,
    path: Utf8PathBuf,
}

impl GithubJobStepsBuilder {
    #[cfg(test)]
    /// Test only ctor for skipping the fs lookup
    pub fn from_values(
        steps: impl IntoIterator<Item = GithubJobStep>,
        path: impl Into<Utf8PathBuf>,
    ) -> Self {
        Self {
            steps: Vec::from_iter(steps),
            path: path.into(),
        }
    }

    /// Create a new validator
    pub fn new(
        base_path: impl AsRef<Utf8Path>,
        cfg_value: impl AsRef<Utf8Path>,
    ) -> Result<Self, DistError> {
        let path = base_path.as_ref().join(cfg_value.as_ref());
        let src = SourceFile::load_local(&path)
            .map_err(|e| DistError::GithubBuildSetupNotFound { details: e })?;
        let steps =
            deserialize_yaml(&src).map_err(|e| DistError::GithubBuildSetupParse { details: e })?;
        Ok(Self { steps, path })
    }

    /// Validate the whole list of build setup steps
    pub fn validate(self) -> Result<Vec<GithubJobStep>, DistError> {
        for (i, step) in self.steps.iter().enumerate() {
            if let Some(message) = Self::validate_step(i, step) {
                return Err(DistError::GithubBuildSetupNotValid {
                    file_path: self.path.to_path_buf(),
                    message,
                });
            }
        }
        Ok(self.steps)
    }

    /// validate a single step in the list of steps, returns `Some` if an error is detected
    fn validate_step(idx: usize, step: &GithubJobStep) -> Option<String> {
        //github-build-step {step_name} is invalid, cannot have both `uses` and `{conflict_step_name}` defined
        let key_mismatch = |lhs: &str, rhs: &str| {
            let step_name = Self::get_name_id_or_idx(idx, step);
            format!("github-build-step {step_name} is invalid, cannot have both `{lhs}` and `{rhs}` defined")
        };
        let invalid_object = |prop: &str, msg: &str| {
            let step_name = Self::get_name_id_or_idx(idx, step);
            format!("github-build-step {step_name} has an invalid `{prop}` entry: {msg}")
        };
        if let Some(key) = Self::validate_step_uses_keys(step) {
            return Some(key_mismatch("uses", key));
        }
        if let Some(key) = Self::validate_step_run_keys(step) {
            return Some(key_mismatch("run", key));
        }
        if let Some(message) = Self::validate_with_shape(step) {
            return Some(invalid_object("with", &message));
        }
        None
    }

    /// Validate there are no conflicting keys in this workflow that defines the `uses` keyword
    ///
    /// returns `Some` if an error is detected
    fn validate_step_uses_keys(step: &GithubJobStep) -> Option<&'static str> {
        // if uses is None, return early
        step.uses.as_ref()?;
        if step.run.is_some() {
            return Some("run");
        }
        if step.shell.is_some() {
            return Some("shell");
        }
        if step.working_directory.is_some() {
            return Some("working-directory");
        }
        None
    }

    /// Validate there are no conflicting keys in this workflow that defines the `run` keyword
    /// this is called _after_ `Self::validate_step_uses_keys`
    ///
    /// returns `Some` if an error is detected
    fn validate_step_run_keys(step: &GithubJobStep) -> Option<&'static str> {
        // if run is None, return early
        step.run.as_ref()?;
        if !step.with.is_empty() {
            return Some("with");
        }
        None
    }

    /// Validate the with mapping only contains key/value pairs with the values being either
    /// strings, booleans, or numbers
    ///
    /// returns `Some` if an error is detected
    fn validate_with_shape(step: &GithubJobStep) -> Option<String> {
        for (k, v) in &step.with {
            let invalid_type = match v {
                serde_json::Value::Null => "null",
                serde_json::Value::Array(_) => "array",
                serde_json::Value::Object(_) => "object",
                _ => continue,
            };
            return Some(format!("key `{k}` has the type of `{invalid_type}` only `string`, `number` or `boolean` are supported"));
        }
        None
    }

    fn get_name_id_or_idx(idx: usize, step: &GithubJobStep) -> String {
        step.name
            .clone()
            .or_else(|| step.id.clone())
            .unwrap_or_else(|| idx.to_string())
    }
}

/// Try to deserialize the contents of the SourceFile as yaml
///
/// FIXME: upstream to axoasset
fn deserialize_yaml<'a, T: for<'de> serde::Deserialize<'de>>(
    src: &'a SourceFile,
) -> Result<T, crate::errors::AxoassetYamlError> {
    let yaml = serde_yml::from_str(src.contents()).map_err(|details| {
        let span = details
            .location()
            .and_then(|location| src.span_for_line_col(location.line(), location.column()));
        crate::errors::AxoassetYamlError {
            source: src.clone(),
            span,
            details,
        }
    })?;
    Ok(yaml)
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    #[test]
    fn validator_works() {
        let steps = [GithubJobStep {
            uses: Some("".to_string()),
            with: BTreeMap::from_iter([
                ("key".to_string(), Value::from("value")),
                ("key2".to_string(), Value::from(2)),
                ("key2".to_string(), Value::from(false)),
            ]),
            timeout_minutes: Some("8".into()),
            continue_on_error: Some("true".into()),
            ..Default::default()
        }];
        let path = Utf8PathBuf::from(std::thread::current().name().unwrap_or(""));
        GithubJobStepsBuilder::from_values(steps, path)
            .validate()
            .expect("validation to pass");
    }

    #[test]
    #[should_panic = "cannot have both `uses` and `run` defined"]
    fn validator_catches_run_and_uses() {
        let steps = [GithubJobStep {
            uses: Some("".to_string()),
            run: Some("".to_string()),
            ..Default::default()
        }];
        let path = Utf8PathBuf::from(std::thread::current().name().unwrap_or(""));
        GithubJobStepsBuilder::from_values(steps, path)
            .validate()
            .unwrap();
    }

    #[test]
    #[should_panic = "cannot have both `uses` and `shell` defined"]
    fn validator_catches_run_and_shell() {
        let steps = [GithubJobStep {
            uses: Some("".to_string()),
            shell: Some("".to_string()),
            ..Default::default()
        }];
        let path = Utf8PathBuf::from(std::thread::current().name().unwrap_or(""));
        GithubJobStepsBuilder::from_values(steps, path)
            .validate()
            .unwrap();
    }

    #[test]
    #[should_panic = "cannot have both `uses` and `working-directory` defined"]
    fn validator_catches_run_and_cwd() {
        let steps = [GithubJobStep {
            uses: Some("".to_string()),
            working_directory: Some("".to_string()),
            ..Default::default()
        }];
        let path = Utf8PathBuf::from(std::thread::current().name().unwrap_or(""));
        GithubJobStepsBuilder::from_values(steps, path)
            .validate()
            .unwrap();
    }

    #[test]
    #[should_panic = "cannot have both `run` and `with` defined"]
    fn validator_catches_run_and_with() {
        let steps = [GithubJobStep {
            run: Some("".to_string()),
            with: BTreeMap::from_iter([("key".to_string(), Value::from("value"))]),
            ..Default::default()
        }];
        let path = Utf8PathBuf::from(std::thread::current().name().unwrap_or(""));
        GithubJobStepsBuilder::from_values(steps, path)
            .validate()
            .unwrap();
    }

    #[test]
    #[should_panic = "has an invalid `with` entry: key `key` has the type of `object` only `string`, `number` or `boolean` are supported"]
    fn validator_catches_invalid_with() {
        let steps = [GithubJobStep {
            uses: Some("".to_string()),
            with: BTreeMap::from_iter([(
                "key".to_string(),
                serde_json::json!({
                    "obj-key": "obj-value"
                }),
            )]),
            ..Default::default()
        }];
        let path = Utf8PathBuf::from(std::thread::current().name().unwrap_or(""));
        GithubJobStepsBuilder::from_values(steps, path)
            .validate()
            .unwrap();
    }

    #[test]
    #[should_panic = "step-name"]
    fn validator_errors_with_name() {
        let steps = [GithubJobStep {
            name: Some("step-name".to_string()),
            uses: Some(String::new()),
            run: Some(String::new()),
            ..Default::default()
        }];
        let path = Utf8PathBuf::from(std::thread::current().name().unwrap_or(""));
        GithubJobStepsBuilder::from_values(steps, path)
            .validate()
            .unwrap();
    }

    #[test]
    #[should_panic = "step-name"]
    fn validator_errors_with_name_over_id() {
        let steps = [GithubJobStep {
            name: Some("step-name".to_string()),
            id: Some("step-id".to_string()),
            uses: Some(String::new()),
            run: Some(String::new()),
            ..Default::default()
        }];
        let path = Utf8PathBuf::from(std::thread::current().name().unwrap_or(""));
        GithubJobStepsBuilder::from_values(steps, path)
            .validate()
            .unwrap();
    }

    #[test]
    #[should_panic = "step-id"]
    fn validator_errors_with_id() {
        let steps = [GithubJobStep {
            id: Some("step-id".to_string()),
            uses: Some(String::new()),
            run: Some(String::new()),
            ..Default::default()
        }];
        let path = Utf8PathBuf::from(std::thread::current().name().unwrap_or(""));
        GithubJobStepsBuilder::from_values(steps, path)
            .validate()
            .unwrap();
    }

    #[test]
    fn build_setup_can_read() {
        let tmp = temp_dir::TempDir::new().unwrap();
        let base = Utf8PathBuf::from_path_buf(tmp.path().to_owned())
            .expect("temp_dir made non-utf8 path!?");
        let cfg = "build-setup.yml".to_string();
        std::fs::write(
            base.join(&cfg),
            r#"
- uses: some-action-user/some-action
  continue-on-error: ${{ some.expression }}
  timeout-minutes: ${{ matrix.timeout }}
"#,
        )
        .unwrap();
        GithubJobStepsBuilder::new(&base, &cfg).unwrap();
    }

    #[test]
    fn build_setup_with_if() {
        let tmp = temp_dir::TempDir::new().unwrap();
        let base = Utf8PathBuf::from_path_buf(tmp.path().to_owned())
            .expect("temp_dir made non-utf8 path!?");
        let cfg = "build-setup.yml".to_string();
        std::fs::write(
            base.join(&cfg),
            r#"
- uses: some-action-user/some-action
  if: false
"#,
        )
        .unwrap();
        let out = GithubJobStepsBuilder::new(&base, &cfg)
            .unwrap()
            .validate()
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(out.if_expr, Some(false.into()));
    }
}
