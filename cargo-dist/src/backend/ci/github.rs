//! CI script generation
//!
//! In the future this may get split up into submodules.

use std::collections::BTreeMap;

use axoasset::{LocalAsset, SourceFile};
use axoprocess::Cmd;
use camino::{Utf8Path, Utf8PathBuf};
use cargo_dist_schema::{
    target_lexicon::{self, Architecture, OperatingSystem, Triple},
    AptPackageName, ChocolateyPackageName, ContainerImageRef, GhaRunStep, GithubGlobalJobConfig,
    GithubLocalJobConfig, GithubMatrix, GithubRunnerConfig, GithubRunnerRef, GithubRunners,
    HomebrewPackageName, PackageInstallScript, PackageVersion, PipPackageName, TripleNameRef,
};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::{
    backend::{diff_files, templates::TEMPLATE_CI_GITHUB},
    build_wrapper_for_cross,
    config::{
        v1::{ci::github::GithubCiConfig, publishers::PublisherConfig},
        DependencyKind, GithubPermission, GithubPermissionMap, GithubReleasePhase, HostingStyle,
        JinjaGithubRepoPair, JobStyle, ProductionMode, PublishStyle, SystemDependencies,
    },
    errors::DistResult,
    platform::{github_runners::target_for_github_runner_or_default, targets},
    CargoBuildWrapper, DistError, DistGraph, SortedMap, SortedSet,
};

use super::{
    CargoAuditableInstallStrategy, CargoCyclonedxInstallStrategy, DistInstallSettings,
    DistInstallStrategy, InstallStrategy, OmniborInstallStrategy,
};

#[cfg(not(windows))]
const GITHUB_CI_DIR: &str = ".github/workflows/";
#[cfg(windows)]
const GITHUB_CI_DIR: &str = r".github\workflows\";
const GITHUB_CI_FILE: &str = "release.yml";

/// Info about running dist in Github CI
///
/// THESE FIELDS ARE LOAD-BEARING because they're used in the templates.
#[derive(Debug, Serialize)]
pub struct GithubCiInfo {
    /// Cached path to github CI workflows dir
    #[serde(skip_serializing)]
    pub github_ci_workflow_dir: Utf8PathBuf,
    /// Version of rust toolchain to install (deprecated)
    pub rust_version: Option<String>,
    /// How to install dist when "coordinating" (plan, global build, etc.)
    pub dist_install_for_coordinator: GhaRunStep,
    /// Our install strategy for dist itself
    pub dist_install_strategy: DistInstallStrategy,
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
    pub global_task: GithubGlobalJobConfig,
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
    /// Action versions to use
    pub actions: SortedMap<String, String>,
    /// Whether to install cargo-auditable
    pub need_cargo_auditable: bool,
    /// Whether to run cargo-cyclonedx
    pub need_cargo_cyclonedx: bool,
    /// Whether to install and run omnibor-cli
    pub need_omnibor: bool,
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

    /// The working directory this action should run
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
    /// Permissions to give the job
    pub permissions: Option<GithubPermissionMap>,
}

impl GithubCiInfo {
    /// Compute the Github CI stuff
    pub fn new(dist: &DistGraph, ci_config: &GithubCiConfig) -> DistResult<GithubCiInfo> {
        // Legacy deprecated support
        let rust_version = dist.config.builds.cargo.rust_toolchain_version.clone();

        // If they don't specify a dist version, use this one
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

        let need_cargo_auditable = dist.config.builds.cargo.cargo_auditable;
        let need_cargo_cyclonedx = dist.config.builds.cargo.cargo_cyclonedx;
        let need_omnibor = dist.config.builds.omnibor;

        // Figure out what builds we need to do
        let mut local_targets: SortedSet<&TripleNameRef> = SortedSet::new();
        for release in &dist.releases {
            for target in &release.targets {
                local_targets.insert(target);
            }
            dependencies.append(&mut release.config.builds.system_dependencies.clone());
        }

        let dist_install_strategy = (DistInstallSettings {
            version: dist_version,
            url_override: dist.config.dist_url_override.as_deref(),
        })
        .install_strategy();
        let cargo_auditable_install_strategy = CargoAuditableInstallStrategy;
        let cargo_cyclonedx_install_strategy = CargoCyclonedxInstallStrategy;
        let omnibor_install_strategy = OmniborInstallStrategy;

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
            .cloned()
            .unwrap_or_else(default_global_runner_config);
        let global_task = GithubGlobalJobConfig {
            runner: global_runner.to_owned(),
            dist_args: "--artifacts=global".into(),
            install_dist: dist_install_strategy.dash(),
            install_cargo_cyclonedx: Some(cargo_cyclonedx_install_strategy.dash()),
            install_omnibor: need_omnibor.then_some(omnibor_install_strategy.dash()),
        };

        let tap = dist.global_homebrew_tap.clone();

        let mut job_permissions = ci_config.permissions.clone();
        // user publish jobs default to elevated privileges
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
            distribute_targets_to_runners_merged(local_targets, &ci_config.runners)?
        } else {
            distribute_targets_to_runners_split(local_targets, &ci_config.runners)?
        };
        for (runner, targets) in local_runs {
            use std::fmt::Write;
            let real_triple = runner.real_triple();
            let install_dist = dist_install_strategy.for_triple(&real_triple);
            let install_cargo_auditable =
                cargo_auditable_install_strategy.for_triple(&runner.real_triple());
            let install_omnibor = omnibor_install_strategy.for_triple(&real_triple);

            let mut dist_args = String::from("--artifacts=local");
            for target in &targets {
                write!(dist_args, " --target={target}").unwrap();
            }
            let packages_install = system_deps_install_script(&runner, &targets, &dependencies)?;
            tasks.push(GithubLocalJobConfig {
                targets: Some(targets.iter().copied().map(|s| s.to_owned()).collect()),
                cache_provider: cache_provider_for_runner(&runner),
                runner,
                dist_args,
                install_dist: install_dist.to_owned(),
                install_cargo_auditable: need_cargo_auditable
                    .then_some(install_cargo_auditable.to_owned()),
                install_omnibor: need_omnibor.then_some(install_omnibor.to_owned()),
                packages_install,
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

        let default_action_versions = [
            ("actions/checkout", "v4"),
            ("actions/upload-artifact", "v4"),
            ("actions/download-artifact", "v4"),
            ("actions/attest-build-provenance", "v2"),
            ("swatinem/rust-cache", "v2"),
            ("actions/setup-node", "v4"),
        ];
        let actions = default_action_versions
            .iter()
            .map(|(name, version)| {
                let version = ci_config
                    .action_commits
                    .get(*name)
                    .map(|c| &**c)
                    .unwrap_or(*version);
                (name.to_string(), format!("{name}@{version}"))
            })
            .collect::<SortedMap<_, _>>();

        Ok(GithubCiInfo {
            github_ci_workflow_dir,
            tag_namespace,
            rust_version,
            dist_install_for_coordinator: dist_install_strategy.dash(),
            dist_install_strategy,
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
            actions,
            need_cargo_auditable,
            need_cargo_cyclonedx,
            need_omnibor,
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
fn cache_provider_for_runner(rc: &GithubRunnerConfig) -> Option<String> {
    if rc.runner.is_buildjet() {
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
    targets: SortedSet<&'a TripleNameRef>,
    custom_runners: &GithubRunners,
) -> DistResult<std::vec::IntoIter<(GithubRunnerConfig, Vec<&'a TripleNameRef>)>> {
    let mut groups = SortedMap::<GithubRunnerConfig, Vec<&TripleNameRef>>::new();
    for target in targets {
        let runner_conf = github_runner_for_target(target, custom_runners)?;
        let runner_conf = runner_conf.unwrap_or_else(|| {
            let fallback = default_global_runner_config();
            warn!(
                "not sure which github runner should be used for {target}, assuming {}",
                fallback.runner
            );
            fallback.to_owned()
        });
        groups.entry(runner_conf).or_default().push(target);
    }
    // This extra into_iter+collect is needed to make this have the same
    // return type as distribute_targets_to_runners_split
    Ok(groups.into_iter().collect::<Vec<_>>().into_iter())
}

/// Given a set of targets we want to build local artifacts for, map them to Github Runners
/// while preferring each target gets its own runner for latency and fault-isolation.
fn distribute_targets_to_runners_split<'a>(
    targets: SortedSet<&'a TripleNameRef>,
    custom_runners: &GithubRunners,
) -> DistResult<std::vec::IntoIter<(GithubRunnerConfig, Vec<&'a TripleNameRef>)>> {
    let mut groups = vec![];
    for target in targets {
        let runner = github_runner_for_target(target, custom_runners)?;
        let runner = runner.unwrap_or_else(|| {
            let fallback = default_global_runner_config();
            warn!(
                "not sure which github runner should be used for {target}, assuming {}",
                fallback.runner
            );
            fallback.to_owned()
        });
        groups.push((runner, vec![target]));
    }
    Ok(groups.into_iter())
}

/// Generates a [`GithubRunnerConfig`] from a given github runner name
pub fn runner_to_config(runner: &GithubRunnerRef) -> GithubRunnerConfig {
    GithubRunnerConfig {
        runner: runner.to_owned(),
        host: target_for_github_runner_or_default(runner).to_owned(),
        container: None,
    }
}

const DEFAULT_LINUX_RUNNER: &GithubRunnerRef = GithubRunnerRef::from_str("ubuntu-22.04");

fn default_global_runner_config() -> GithubRunnerConfig {
    runner_to_config(DEFAULT_LINUX_RUNNER)
}

/// Get the appropriate Github Runner for building a target
fn github_runner_for_target(
    target: &TripleNameRef,
    custom_runners: &GithubRunners,
) -> DistResult<Option<GithubRunnerConfig>> {
    if let Some(runner) = custom_runners.get(target) {
        return Ok(Some(runner.clone()));
    }

    let target_triple: Triple = target.parse()?;

    // We want to default to older runners to minimize the places
    // where random system dependencies can creep in and be very
    // recent. This helps with portability!
    let result = Some(match target_triple.operating_system {
        OperatingSystem::Linux => runner_to_config(GithubRunnerRef::from_str("ubuntu-22.04")),
        OperatingSystem::Darwin => runner_to_config(GithubRunnerRef::from_str("macos-13")),
        OperatingSystem::Windows => {
            // Default to cargo-xwin for Windows cross-compiles
            if target_triple.architecture != Architecture::X86_64 {
                cargo_xwin()
            } else {
                runner_to_config(GithubRunnerRef::from_str("windows-2022"))
            }
        }
        _ => return Ok(None),
    });

    Ok(result)
}

fn cargo_xwin() -> GithubRunnerConfig {
    GithubRunnerConfig {
        runner: GithubRunnerRef::from_str("ubuntu-22.04").to_owned(),
        host: targets::TARGET_X64_LINUX_GNU.to_owned(),
        container: Some(cargo_dist_schema::ContainerConfig {
            image: ContainerImageRef::from_str("messense/cargo-xwin").to_owned(),
            host: targets::TARGET_X64_LINUX_MUSL.to_owned(),
            package_manager: Some(cargo_dist_schema::PackageManager::Apt),
        }),
    }
}

fn brewfile_from<'a>(packages: impl Iterator<Item = &'a HomebrewPackageName>) -> String {
    packages
        .map(|p| {
            let lower = p.as_str().to_ascii_lowercase();
            // Although `brew install` can take either a formula or a cask,
            // Brewfiles require you to use the `cask` verb for casks and `brew`
            // for formulas.
            if lower.starts_with("homebrew/cask") || lower.starts_with("homebrew/homebrew-cask") {
                format!(r#"cask "{p}""#).to_owned()
            } else {
                format!(r#"brew "{p}""#).to_owned()
            }
        })
        .join("\n")
}

fn brew_bundle_command<'a>(packages: impl Iterator<Item = &'a HomebrewPackageName>) -> String {
    format!(
        r#"cat << EOF >Brewfile
{}
EOF

brew bundle install"#,
        brewfile_from(packages)
    )
}

fn system_deps_install_script(
    rc: &GithubRunnerConfig,
    targets: &[&TripleNameRef],
    packages: &SystemDependencies,
) -> DistResult<Option<PackageInstallScript>> {
    let mut brew_packages: SortedSet<HomebrewPackageName> = Default::default();
    let mut apt_packages: SortedSet<(AptPackageName, Option<PackageVersion>)> = Default::default();
    let mut chocolatey_packages: SortedSet<(ChocolateyPackageName, Option<PackageVersion>)> =
        Default::default();

    let host = rc.real_triple();
    match host.operating_system {
        OperatingSystem::Darwin => {
            for (name, pkg) in &packages.homebrew {
                if !pkg.0.stage_wanted(&DependencyKind::Build) {
                    continue;
                }
                if !targets.iter().any(|target| pkg.0.wanted_for_target(target)) {
                    continue;
                }
                brew_packages.insert(name.clone());
            }
        }
        OperatingSystem::Linux => {
            // We currently don't support non-apt package managers on Linux
            // is_none() means a native build, probably on GitHub's
            // apt-using runners.
            if rc.container.is_none()
                || rc.container.as_ref().and_then(|c| c.package_manager)
                    == Some(cargo_dist_schema::PackageManager::Apt)
            {
                for (name, pkg) in &packages.apt {
                    if !pkg.0.stage_wanted(&DependencyKind::Build) {
                        continue;
                    }
                    if !targets.iter().any(|target| pkg.0.wanted_for_target(target)) {
                        continue;
                    }
                    apt_packages.insert((name.clone(), pkg.0.version.clone()));
                }

                let has_musl_target = targets.iter().any(|target| {
                    target.parse().unwrap().environment == target_lexicon::Environment::Musl
                });
                if has_musl_target {
                    // musl builds may require musl-tools to build;
                    // necessary for more complex software
                    apt_packages.insert((AptPackageName::new("musl-tools".to_owned()), None));
                }
            }
        }
        OperatingSystem::Windows => {
            for (name, pkg) in &packages.chocolatey {
                if !pkg.0.stage_wanted(&DependencyKind::Build) {
                    continue;
                }
                if !targets.iter().any(|target| pkg.0.wanted_for_target(target)) {
                    continue;
                }
                chocolatey_packages.insert((name.clone(), pkg.0.version.clone()));
            }
        }
        _ => {
            panic!(
                "unsupported host operating system: {:?}",
                host.operating_system
            )
        }
    }

    let mut lines = vec![];
    if !brew_packages.is_empty() {
        lines.push(brew_bundle_command(brew_packages.iter()))
    }

    // If we're crossing, we'll most likely be running from a container with
    // no sudo. We should avoid calling sudo in that case.
    let sudo = if rc.container.is_some() { "" } else { "sudo " };
    if !apt_packages.is_empty() {
        lines.push(format!("{sudo}apt-get update"));
        let args = apt_packages
            .iter()
            .map(|(pkg, version)| {
                if let Some(v) = version {
                    format!("{pkg}={v}")
                } else {
                    pkg.to_string()
                }
            })
            .join(" ");
        lines.push(format!("{sudo}apt-get install {args}"));
    }

    for (pkg, version) in &chocolatey_packages {
        lines.push(if let Some(v) = version {
            format!("choco install {pkg} --version={v} --yes")
        } else {
            format!("choco install {pkg} --yes")
        });
    }

    // Regardless of what we're doing, we might need build wrappers!
    let mut required_wrappers: SortedSet<CargoBuildWrapper> = Default::default();
    for target in targets {
        let target = target.parse().unwrap();
        if let Some(wrapper) = build_wrapper_for_cross(&host, &target)? {
            required_wrappers.insert(wrapper);
        }
    }

    let mut pip_pkgs: SortedSet<PipPackageName> = Default::default();
    if required_wrappers.contains(&CargoBuildWrapper::ZigBuild) {
        pip_pkgs.insert(PipPackageName::new("cargo-zigbuild".to_owned()));
    }
    if required_wrappers.contains(&CargoBuildWrapper::Xwin) {
        pip_pkgs.insert(PipPackageName::new("cargo-xwin".to_owned()));
    }

    if !pip_pkgs.is_empty() {
        let push_pip_install_lines = |lines: &mut Vec<String>| {
            if host.operating_system == OperatingSystem::Linux {
                // make sure pip is installed — on dnf-based distros we might need to install
                // it (true for e.g. the `quay.io/pypa/manylinux_2_28_x86_64` image)
                //
                // this doesn't work for all distros of course — others might need to be added
                // later. there's no universal way to install tooling in dist right now anyway.
                lines.push("  if ! command -v pip3 > /dev/null 2>&1; then".to_owned());
                lines.push("    dnf install --assumeyes python3-pip".to_owned());
                lines.push("    pip3 install --upgrade pip".to_owned());
                lines.push("  fi".to_owned());
            }
        };

        for pip_pkg in pip_pkgs {
            match pip_pkg.as_str() {
                "cargo-xwin" => {
                    // that one could already be installed
                    lines.push("if ! command -v cargo-xwin > /dev/null 2>&1; then".to_owned());
                    push_pip_install_lines(&mut lines);
                    lines.push("  pip3 install cargo-xwin".to_owned());
                    lines.push("fi".to_owned());
                }
                "cargo-zigbuild" => {
                    // that one could already be installed
                    lines.push("if ! command -v cargo-zigbuild > /dev/null 2>&1; then".to_owned());
                    push_pip_install_lines(&mut lines);
                    lines.push("  pip3 install cargo-zigbuild".to_owned());
                    lines.push("fi".to_owned());
                }
                _ => {
                    lines.push(format!("pip3 install {pip_pkg}"));
                }
            }
        }
    }

    Ok(if lines.is_empty() {
        None
    } else {
        Some(PackageInstallScript::new(lines.join("\n")))
    })
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
        let steps = src
            .deserialize_yaml()
            .map_err(|e| DistError::GithubBuildSetupParse { details: e })?;
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
