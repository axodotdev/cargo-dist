//! Config types (for workspace.metadata.dist)

use std::collections::{BTreeMap, HashMap};

use axoasset::{toml_edit, SourceFile};
use axoprocess::Cmd;
use axoproject::WorkspaceKind;
use camino::{Utf8Path, Utf8PathBuf};
use semver::Version;
use serde::{Deserialize, Serialize};
use tracing::log::warn;

use crate::announce::TagSettings;
use crate::{
    errors::{DistError, DistResult},
    TargetTriple, METADATA_DIST,
};

/// A container to assist deserializing metadata from generic, non-Cargo projects
#[derive(Debug, Deserialize)]
struct GenericConfig {
    /// The dist field within dist.toml
    dist: DistMetadata,
}

/// Contents of METADATA_DIST in Cargo.toml files
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct DistMetadata {
    /// The intended version of cargo-dist to build with. (normal Cargo SemVer syntax)
    ///
    /// When generating full tasks graphs (such as CI scripts) we will pick this version.
    ///
    /// FIXME: Should we produce a warning if running locally with a different version? In theory
    /// it shouldn't be a problem and newer versions should just be Better... probably you
    /// Really want to have the exact version when running generate to avoid generating
    /// things other cargo-dist versions can't handle!
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cargo_dist_version: Option<Version>,

    /// (deprecated) The intended version of Rust/Cargo to build with (rustup toolchain syntax)
    ///
    /// When generating full tasks graphs (such as CI scripts) we will pick this version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rust_toolchain_version: Option<String>,

    /// Whether the package should be distributed/built by cargo-dist
    ///
    /// This mainly exists to be set to `false` to make cargo-dist ignore the existence of this
    /// package. Note that we may still build the package as a side-effect of building the
    /// workspace -- we just won't bundle it up and report it.
    ///
    /// FIXME: maybe you should also be allowed to make this a list of binary names..?
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dist: Option<bool>,

    /// CI environments you wish to target.
    ///
    /// Currently only accepts "github".
    ///
    /// When running `generate` this list will be used if it's Some, otherwise all known
    /// CI backends will be used.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, with = "opt_string_or_vec")]
    pub ci: Option<Vec<CiStyle>>,

    /// Which actions to run on pull requests.
    ///
    /// "upload" will build and upload release artifacts, while "plan" will
    /// only plan out the release without running builds and "skip" will disable
    /// pull request runs entirely.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_run_mode: Option<cargo_dist_schema::PrRunMode>,

    /// Generate targets whose cargo-dist should avoid checking for up-to-dateness.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_dirty: Option<Vec<GenerateMode>>,

    /// The full set of installers you would like to produce
    ///
    /// When generating full task graphs (such as CI scripts) we will try to generate these.
    ///
    /// Some installers can be generated on any platform (like shell scripts) while others
    /// may (currently) require platform-specific toolchains (like .msi installers). Some
    /// installers may also be "per release" while others are "per build". Again, shell script
    /// vs msi is a good comparison here -- you want a universal shell script that figures
    /// out which binary to install, but you might end up with an msi for each supported arch!
    ///
    /// Currently accepted values:
    ///
    /// * shell
    /// * powershell
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installers: Option<Vec<InstallerStyle>>,

    /// Custom sucess message for installers
    ///
    /// When an shell or powershell installer succeeds at installing your app it
    /// will out put a message to the user. This config allows a user to specify
    /// a custom message as opposed to the default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_success_msg: Option<String>,

    /// A Homebrew tap to push the Homebrew formula to, if built
    pub tap: Option<String>,
    /// Customize the name of the Homebrew formula
    pub formula: Option<String>,

    /// A set of packages to install before building
    #[serde(rename = "dependencies")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_dependencies: Option<SystemDependencies>,

    /// The full set of target triples to build for.
    ///
    /// When generating full task graphs (such as CI scripts) we will to try to generate these.
    ///
    /// The inputs should be valid rustc target triples (see `rustc --print target-list`) such
    /// as `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, or `x86_64-unknown-linux-gnu`.
    ///
    /// FIXME: We should also accept one magic target: `universal2-apple-darwin`. This will induce
    /// us to build `x86_64-apple-darwin` and `aarch64-apple-darwin` (arm64) and then combine
    /// them into a "universal" binary that can run on either arch (using apple's `lipo` tool).
    ///
    /// FIXME: Allow higher level requests like "[macos, windows, linux] x [x86_64, aarch64]"?
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<String>>,

    /// Include the following static files in bundles like archives.
    ///
    /// Paths are relative to the Cargo.toml this is defined in.
    ///
    /// Files like `README*`, `(UN)LICENSE*`, `RELEASES*`, and `CHANGELOG*` are already
    /// automatically detected and included (use [`DistMetadata::auto_includes`][] to prevent this).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<Utf8PathBuf>>,

    /// Whether to auto-include files like `README*`, `(UN)LICENSE*`, `RELEASES*`, and `CHANGELOG*`
    ///
    /// Defaults to true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_includes: Option<bool>,

    /// Whether msvc targets should statically link the crt
    ///
    /// Defaults to true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msvc_crt_static: Option<bool>,

    /// The archive format to use for windows builds (defaults .zip)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub windows_archive: Option<ZipStyle>,

    /// The archive format to use for non-windows builds (defaults .tar.xz)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unix_archive: Option<ZipStyle>,

    /// Replace the app's name with this value for the npm package's name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npm_package: Option<String>,

    /// A scope to prefix npm packages with (@ should be included).
    ///
    /// This is required if you're using an npm installer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npm_scope: Option<String>,

    /// A scope to prefix npm packages with (@ should be included).
    ///
    /// This is required if you're using an npm installer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<ChecksumStyle>,

    /// Build only the required packages, and individually (since 0.1.0) (default: false)
    ///
    /// By default when we need to build anything in your workspace, we build your entire workspace
    /// with --workspace. This setting tells cargo-dist to instead build each app individually.
    ///
    /// On balance, the Rust experts we've consulted with find building with --workspace to
    /// be a safer/better default, as it provides some of the benefits of a more manual
    /// [workspace-hack][], without the user needing to be aware that this is a thing.
    ///
    /// TL;DR: cargo prefers building one copy of each dependency in a build, so if two apps in
    /// your workspace depend on e.g. serde with different features, building with --workspace,
    /// will build serde once with the features unioned together. However if you build each
    /// package individually it will more precisely build two copies of serde with different
    /// feature sets.
    ///
    /// The downside of using --workspace is that if your workspace has lots of example/test
    /// crates, or if you release only parts of your workspace at a time, we build a lot of
    /// gunk that's not needed, and potentially bloat up your app with unnecessary features.
    ///
    /// If that downside is big enough for you, this setting is a good idea.
    ///
    /// [workspace-hack]: https://docs.rs/cargo-hakari/latest/cargo_hakari/about/index.html
    #[serde(skip_serializing_if = "Option::is_none")]
    pub precise_builds: Option<bool>,

    /// Whether we should try to merge otherwise-parallelizable tasks onto the same machine,
    /// sacrificing latency and fault-isolation for more the sake of minor effeciency gains.
    ///
    /// (defaults to false)
    ///
    /// For example, if you build for x64 macos and arm64 macos, by default we will generate ci
    /// which builds those independently on separate logical machines. With this enabled we will
    /// build both of those platforms together on the same machine, making it take twice as long
    /// as any other build and making it impossible for only one of them to succeed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_tasks: Option<bool>,

    /// Whether failing tasks should make us give up on all other tasks
    ///
    /// (defaults to false)
    ///
    /// When building a release you might discover that an obscure platform's build is broken.
    /// When this happens you have two options: give up on the release entirely (`fail-fast = true`),
    /// or keep trying to build all the other platforms anyway (`fail-fast = false`).
    ///
    /// cargo-dist was designed around the "keep trying" approach, as we create a draft Release
    /// and upload results to it over time, undrafting the release only if all tasks succeeded.
    /// The idea is that even if a platform fails to build, you can decide that's acceptable
    /// and manually undraft the release with some missing platforms.
    ///
    /// (Note that the dist-manifest.json is produced before anything else, and so it will assume
    /// that all tasks succeeded when listing out supported platforms/artifacts. This may make
    /// you sad if you do this kind of undrafting and also trust the dist-manifest to be correct.)
    ///
    /// Prior to 0.1.0 we didn't set the correct flags in our CI scripts to do this, but now we do.
    /// This flag was introduced to allow you to restore the old behaviour if you prefer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_fast: Option<bool>,

    /// Whether CI should include logic to build local artifacts (default true)
    ///
    /// If false, it will be assumed that the local_artifacts_jobs will include custom
    /// jobs to build them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_local_artifacts: Option<bool>,

    /// Whether CI should trigger releases by dispatch instead of tag push (default false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispatch_releases: Option<bool>,

    /// Instead of triggering releases on tags, trigger on pushing to a specific branch
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_branch: Option<String>,

    /// The strategy to use for selecting a path to install things at:
    ///
    /// * `CARGO_HOME`: (default) install as if cargo did
    ///   (try `$CARGO_HOME/bin/`, but if `$CARGO_HOME` isn't set use `$HOME/.cargo/bin/`)
    /// * `~/some/subdir/`: install to the given subdir of the user's `$HOME`
    /// * `$SOME_VAR/some/subdir`: install to the given subdir of the dir defined by `$SOME_VAR`
    ///
    /// All of these error out if the required env-vars aren't set. In the future this may
    /// allow for the input to be an array of options to try in sequence.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, with = "opt_string_or_vec")]
    pub install_path: Option<Vec<InstallPathStrategy>>,
    /// A list of features to enable when building a package with cargo-dist
    ///
    /// (defaults to none)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<Vec<String>>,
    /// Whether to enable when building a package with cargo-dist
    ///
    /// (defaults to true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_features: Option<bool>,
    /// Whether to enable all features building a package with cargo-dist
    ///
    /// (defaults to false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all_features: Option<bool>,

    /// Plan jobs to run in CI
    ///
    /// The core plan job is always run, but this allows additional hooks
    /// to be added to the process to run concurrently with plan.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_jobs: Option<Vec<JobStyle>>,

    /// Local artifacts jobs to run in CI
    ///
    /// The core build job is always run, but this allows additional hooks
    /// to be added to the process to run concurrently with "upload local artifacts".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_artifacts_jobs: Option<Vec<JobStyle>>,

    /// Global artifacts jobs to run in CI
    ///
    /// The core build job is always run, but this allows additional hooks
    /// to be added to the process to run concurrently with "upload global artifacts".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_artifacts_jobs: Option<Vec<JobStyle>>,

    /// Whether to generate and dist a tarball containing your app's source code
    ///
    /// (defaults to true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_tarball: Option<bool>,

    /// Host jobs to run in CI
    ///
    /// The core build job is always run, but this allows additional hooks
    /// to be added to the process to run concurrently with host.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_jobs: Option<Vec<JobStyle>>,

    /// Publish jobs to run in CI
    ///
    /// (defaults to none)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_jobs: Option<Vec<PublishStyle>>,

    /// Post-announce jobs to run in CI
    ///
    /// This allows custom jobs to be configured to run after the announce job
    // runs in its entirety.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_announce_jobs: Option<Vec<JobStyle>>,

    /// Whether to publish prereleases to package managers
    ///
    /// (defaults to false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_prereleases: Option<bool>,

    /// Always regard releases as stable
    ///
    /// (defaults to false)
    ///
    /// Ordinarily, cargo-dist tries to detect if your release
    /// is a prerelease based on its version number using
    /// semver standards. If it's a prerelease, it will be
    /// marked as a prerelease in hosting services such as
    /// GitHub and Axo Releases.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_latest: Option<bool>,

    /// Whether we should create the Github Release for you when you push a tag.
    ///
    /// If true (default), cargo-dist will create a new Github Release and generate
    /// a title/body for it based on your changelog.
    ///
    /// If false, cargo-dist will assume a draft Github Release already exists
    /// with the title/body you want. At the end of a successful publish it will
    /// undraft the Github Release.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_release: Option<bool>,

    /// Publish GitHub Releases to this repo instead of the current one
    ///
    /// The user must also set GH_RELEASES_TOKEN in their SECRETS
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_releases_repo: Option<GithubRepoPair>,

    /// If github-releases-repo is used, the commit ref to used will
    /// be read from the HEAD of the submodule at this path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_releases_submodule_path: Option<String>,

    /// If enabled, the GitHub
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_release: Option<GithubReleasePhase>,

    /// \[unstable\] Whether we should sign windows binaries with ssl.com
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssldotcom_windows_sign: Option<ProductionMode>,

    /// Whether GitHub Attestations is enabled (default false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_attestations: Option<bool>,

    /// Hosting provider
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, with = "opt_string_or_vec")]
    pub hosting: Option<Vec<HostingStyle>>,

    /// Any extra artifacts and their buildscripts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_artifacts: Option<Vec<ExtraArtifact>>,

    /// Custom GitHub runners, mapped by triple target
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_custom_runners: Option<HashMap<String, String>>,

    /// Aliases to install binaries as
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin_aliases: Option<BTreeMap<String, Vec<String>>>,

    /// a prefix to add to the release.yml and tag pattern so that
    /// cargo-dist can co-exist with other release workflows in complex workspaces
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_namespace: Option<String>,

    /// Whether to install an updater program alongside the software
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_updater: Option<bool>,

    /// Whether artifacts/installers for this app should be displayed in release bodies
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<bool>,
    /// How to refer to the app in release bodies
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

impl DistMetadata {
    /// Apply the base path to any relative paths contained in this DistMetadata
    pub fn make_relative_to(&mut self, base_path: &Utf8Path) {
        // This is intentionally written awkwardly to make you update it
        let DistMetadata {
            include,
            extra_artifacts,
            // The rest of these don't include relative paths
            cargo_dist_version: _,
            rust_toolchain_version: _,
            dist: _,
            ci: _,
            installers: _,
            install_success_msg: _,
            tap: _,
            formula: _,
            system_dependencies: _,
            targets: _,
            auto_includes: _,
            windows_archive: _,
            unix_archive: _,
            npm_package: _,
            npm_scope: _,
            checksum: _,
            precise_builds: _,
            fail_fast: _,
            merge_tasks: _,
            build_local_artifacts: _,
            dispatch_releases: _,
            release_branch: _,
            install_path: _,
            features: _,
            default_features: _,
            all_features: _,
            plan_jobs: _,
            local_artifacts_jobs: _,
            global_artifacts_jobs: _,
            source_tarball: _,
            host_jobs: _,
            publish_jobs: _,
            post_announce_jobs: _,
            publish_prereleases: _,
            force_latest: _,
            create_release: _,
            pr_run_mode: _,
            allow_dirty: _,
            github_release: _,
            ssldotcom_windows_sign: _,
            github_attestations: _,
            msvc_crt_static: _,
            hosting: _,
            github_custom_runners: _,
            bin_aliases: _,
            tag_namespace: _,
            install_updater: _,
            github_releases_repo: _,
            github_releases_submodule_path: _,
            display: _,
            display_name: _,
        } = self;
        if let Some(include) = include {
            for include in include {
                *include = base_path.join(&*include);
            }
        }
        if let Some(extra_artifacts) = extra_artifacts {
            for extra in extra_artifacts {
                // We update the working_dir to be relative to this file
                // (by default it's empty and this just sets the working dir to base_path),
                // but we don't update the paths to the artifacts as those are assumed to be
                // relative to the working_dir.
                extra.working_dir = base_path.join(&extra.working_dir);
            }
        }
    }

    /// Determines whether the configured install paths are compatible with each other
    pub fn validate_install_paths(&self) -> DistResult<()> {
        if let Some(paths) = &self.install_path {
            if paths.len() > 1 && paths.contains(&InstallPathStrategy::CargoHome) {
                return Err(DistError::IncompatibleInstallPathConfiguration {});
            }
        }

        Ok(())
    }

    /// Merge a workspace config into a package config (self)
    pub fn merge_workspace_config(
        &mut self,
        workspace_config: &Self,
        package_manifest_path: &Utf8Path,
    ) {
        // This is intentionally written awkwardly to make you update it
        let DistMetadata {
            cargo_dist_version,
            rust_toolchain_version,
            dist,
            ci,
            installers,
            install_success_msg,
            tap,
            formula,
            system_dependencies,
            targets,
            include,
            auto_includes,
            windows_archive,
            unix_archive,
            npm_package,
            npm_scope,
            checksum,
            precise_builds,
            merge_tasks,
            fail_fast,
            build_local_artifacts,
            dispatch_releases,
            release_branch,
            install_path,
            features,
            default_features,
            all_features,
            plan_jobs,
            local_artifacts_jobs,
            global_artifacts_jobs,
            source_tarball,
            host_jobs,
            publish_jobs,
            post_announce_jobs,
            publish_prereleases,
            force_latest,
            create_release,
            pr_run_mode,
            allow_dirty,
            github_release,
            ssldotcom_windows_sign,
            github_attestations,
            msvc_crt_static,
            hosting,
            extra_artifacts,
            github_custom_runners,
            bin_aliases,
            tag_namespace,
            install_updater,
            github_releases_repo,
            github_releases_submodule_path,
            display,
            display_name,
        } = self;

        // Check for global settings on local packages
        if cargo_dist_version.is_some() {
            warn!("package.metadata.dist.cargo-dist-version is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if rust_toolchain_version.is_some() {
            warn!("package.metadata.dist.rust-toolchain-version is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if ci.is_some() {
            warn!("package.metadata.dist.ci is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if precise_builds.is_some() {
            warn!("package.metadata.dist.precise-builds is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if merge_tasks.is_some() {
            warn!("package.metadata.dist.merge-tasks is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if fail_fast.is_some() {
            warn!("package.metadata.dist.fail-fast is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if build_local_artifacts.is_some() {
            warn!("package.metadata.dist.build-local-artifacts is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if dispatch_releases.is_some() {
            warn!("package.metadata.dist.dispatch-releases is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if release_branch.is_some() {
            warn!("package.metadata.dist.release-branch is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if create_release.is_some() {
            warn!("package.metadata.dist.create-release is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if github_releases_repo.is_some() {
            warn!("package.metadata.dist.github-releases-repo is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if github_releases_submodule_path.is_some() {
            warn!("package.metadata.dist.github-releases-submodule-path is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        // Arguably should be package-local for things like msi installers, but doesn't make sense for CI,
        // so let's not support that yet for its complexity!
        if allow_dirty.is_some() {
            warn!("package.metadata.dist.allow-dirty is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if publish_prereleases.is_some() {
            warn!("package.metadata.dist.publish-prereleases is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if force_latest.is_some() {
            warn!("package.metadata.dist.force-stable is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if pr_run_mode.is_some() {
            warn!("package.metadata.dist.pr-run-mode is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if ssldotcom_windows_sign.is_some() {
            warn!("package.metadata.dist.ssldotcom-windows-sign is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if github_attestations.is_some() {
            warn!("package.metadata.dist.github-attestations is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if msvc_crt_static.is_some() {
            warn!("package.metadata.dist.msvc-crt-static is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if hosting.is_some() {
            warn!("package.metadata.dist.hosting is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if plan_jobs.is_some() {
            warn!("package.metadata.dist.plan-jobs is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if local_artifacts_jobs.is_some() {
            warn!("package.metadata.dist.local-artifacts-jobs is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if global_artifacts_jobs.is_some() {
            warn!("package.metadata.dist.global-artifacts-jobs is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if source_tarball.is_some() {
            warn!("package.metadata.dist.source-tarball is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if host_jobs.is_some() {
            warn!("package.metadata.dist.host-jobs is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if publish_jobs.is_some() {
            warn!("package.metadata.dist.publish-jobs is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if post_announce_jobs.is_some() {
            warn!("package.metadata.dist.post-announce-jobs is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if tag_namespace.is_some() {
            warn!("package.metadata.dist.tag-namespace is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if github_release.is_some() {
            warn!("package.metadata.dist.github-create-release-phase is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }

        // Merge non-global settings
        if installers.is_none() {
            installers.clone_from(&workspace_config.installers);
        }
        if targets.is_none() {
            targets.clone_from(&workspace_config.targets);
        }
        if dist.is_none() {
            *dist = workspace_config.dist;
        }
        if auto_includes.is_none() {
            *auto_includes = workspace_config.auto_includes;
        }
        if windows_archive.is_none() {
            *windows_archive = workspace_config.windows_archive;
        }
        if unix_archive.is_none() {
            *unix_archive = workspace_config.unix_archive;
        }
        if npm_package.is_none() {
            npm_package.clone_from(&workspace_config.npm_package);
        }
        if npm_scope.is_none() {
            npm_scope.clone_from(&workspace_config.npm_scope);
        }
        if checksum.is_none() {
            *checksum = workspace_config.checksum;
        }
        if install_path.is_none() {
            install_path.clone_from(&workspace_config.install_path);
        }
        if install_success_msg.is_none() {
            install_success_msg.clone_from(&workspace_config.install_success_msg);
        }
        if features.is_none() {
            features.clone_from(&workspace_config.features);
        }
        if default_features.is_none() {
            *default_features = workspace_config.default_features;
        }
        if all_features.is_none() {
            *all_features = workspace_config.all_features;
        }
        if tap.is_none() {
            tap.clone_from(&workspace_config.tap);
        }
        if formula.is_none() {
            formula.clone_from(&workspace_config.formula);
        }
        if system_dependencies.is_none() {
            system_dependencies.clone_from(&workspace_config.system_dependencies);
        }
        if extra_artifacts.is_none() {
            extra_artifacts.clone_from(&workspace_config.extra_artifacts);
        }
        if github_custom_runners.is_none() {
            github_custom_runners.clone_from(&workspace_config.github_custom_runners);
        }
        if bin_aliases.is_none() {
            bin_aliases.clone_from(&workspace_config.bin_aliases);
        }
        if install_updater.is_none() {
            *install_updater = workspace_config.install_updater;
        }
        if display.is_none() {
            *display = workspace_config.display;
        }
        if display_name.is_none() {
            display_name.clone_from(&workspace_config.display_name);
        }

        // This was historically implemented as extend, but I'm not convinced the
        // inconsistency is worth the inconvenience...
        if let Some(include) = include {
            if let Some(workspace_include) = &workspace_config.include {
                include.extend(workspace_include.iter().cloned());
            }
        } else {
            include.clone_from(&workspace_config.include);
        }
    }
}

/// Global config for commands
#[derive(Debug, Clone)]
pub struct Config {
    /// Settings for the announcement tag
    pub tag_settings: TagSettings,
    /// Whether to actually try to side-effectfully create a hosting directory on a server
    ///
    /// this is used for compute_hosting
    pub create_hosting: bool,
    /// The subset of artifacts we want to build
    pub artifact_mode: ArtifactMode,
    /// Whether local paths to files should be in the final dist json output
    pub no_local_paths: bool,
    /// If true, override allow-dirty in the config and ignore all dirtyness
    pub allow_all_dirty: bool,
    /// Target triples we want to build for
    pub targets: Vec<TargetTriple>,
    /// CI kinds we want to support
    pub ci: Vec<CiStyle>,
    /// Installers we want to generate
    pub installers: Vec<InstallerStyle>,
    /// What command was being invoked here, used for SystemIds
    pub root_cmd: String,
}

/// How we should select the artifacts to build
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtifactMode {
    /// Build target-specific artifacts like archives, symbols, msi installers
    Local,
    /// Build globally unique artifacts like curl-sh installers, npm packages, metadata...
    Global,
    /// Fuzzily build "as much as possible" for the host system
    Host,
    /// Build all the artifacts; only really appropriate for `cargo-dist manifest`
    All,
    /// Fake all the artifacts; useful for testing/mocking/staging
    Lies,
}

impl std::fmt::Display for ArtifactMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            ArtifactMode::Local => "local",
            ArtifactMode::Global => "global",
            ArtifactMode::Host => "host",
            ArtifactMode::All => "all",
            ArtifactMode::Lies => "lies",
        };
        string.fmt(f)
    }
}

/// The style of CI we should generate
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum CiStyle {
    /// Generate Github CI
    Github,
}
impl CiStyle {
    /// If the CI provider provides a native release hosting system, get it
    pub(crate) fn native_hosting(&self) -> Option<HostingStyle> {
        match self {
            CiStyle::Github => Some(HostingStyle::Github),
        }
    }
}

impl std::fmt::Display for CiStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            CiStyle::Github => "github",
        };
        string.fmt(f)
    }
}

impl std::str::FromStr for CiStyle {
    type Err = DistError;
    fn from_str(val: &str) -> DistResult<Self> {
        let res = match val {
            "github" => CiStyle::Github,
            s => {
                return Err(DistError::UnrecognizedCiStyle {
                    style: s.to_string(),
                })
            }
        };
        Ok(res)
    }
}

/// The style of Installer we should generate
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum InstallerStyle {
    /// Generate a shell script that fetches from [`cargo_dist_schema::Release::artifact_download_url`][]
    Shell,
    /// Generate a powershell script that fetches from [`cargo_dist_schema::Release::artifact_download_url`][]
    Powershell,
    /// Generate an npm project that fetches from [`cargo_dist_schema::Release::artifact_download_url`][]
    Npm,
    /// Generate a Homebrew formula that fetches from [`cargo_dist_schema::Release::artifact_download_url`][]
    Homebrew,
    /// Generate an msi installer that embeds the binary
    Msi,
}

impl std::fmt::Display for InstallerStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            InstallerStyle::Shell => "shell",
            InstallerStyle::Powershell => "powershell",
            InstallerStyle::Npm => "npm",
            InstallerStyle::Homebrew => "homebrew",
            InstallerStyle::Msi => "msi",
        };
        string.fmt(f)
    }
}

/// When to create GitHub releases
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum GithubReleasePhase {
    /// Release position depends on whether axo releases is enabled
    #[default]
    Auto,
    /// Create release during the "host" stage, before npm and Homebrew
    Host,
    /// Create release during the "announce" stage, after all publish jobs
    Announce,
}

impl std::fmt::Display for GithubReleasePhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            GithubReleasePhase::Auto => "auto",
            GithubReleasePhase::Host => "host",
            GithubReleasePhase::Announce => "announce",
        };
        string.fmt(f)
    }
}

/// The style of hosting we should use for artifacts
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum HostingStyle {
    /// Host on Github Releases
    Github,
    /// Host on Axo Releases ("Abyss")
    Axodotdev,
}

impl std::fmt::Display for HostingStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            HostingStyle::Github => "github",
            HostingStyle::Axodotdev => "axodotdev",
        };
        string.fmt(f)
    }
}

impl std::str::FromStr for HostingStyle {
    type Err = DistError;
    fn from_str(val: &str) -> DistResult<Self> {
        let res = match val {
            "github" => HostingStyle::Github,
            "axodotdev" => HostingStyle::Axodotdev,
            s => {
                return Err(DistError::UnrecognizedHostingStyle {
                    style: s.to_string(),
                })
            }
        };
        Ok(res)
    }
}

/// The publish jobs we should run
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PublishStyle {
    /// Publish a Homebrew formula to a tap repository
    Homebrew,
    /// Publish an npm pkg to the global npm registry
    Npm,
    /// User-supplied value
    User(String),
}

impl std::str::FromStr for PublishStyle {
    type Err = DistError;
    fn from_str(s: &str) -> DistResult<Self> {
        if let Some(slug) = s.strip_prefix("./") {
            Ok(Self::User(slug.to_owned()))
        } else if s == "homebrew" {
            Ok(Self::Homebrew)
        } else if s == "npm" {
            Ok(Self::Npm)
        } else {
            Err(DistError::UnrecognizedJobStyle {
                style: s.to_owned(),
            })
        }
    }
}

impl<'de> serde::Deserialize<'de> for PublishStyle {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let path = String::deserialize(deserializer)?;
        path.parse().map_err(|e| D::Error::custom(format!("{e}")))
    }
}

impl std::fmt::Display for PublishStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PublishStyle::Homebrew => write!(f, "homebrew"),
            PublishStyle::Npm => write!(f, "npm"),
            PublishStyle::User(s) => write!(f, "./{s}"),
        }
    }
}

/// Extra CI jobs we should run
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum JobStyle {
    /// User-supplied value
    User(String),
}

impl std::str::FromStr for JobStyle {
    type Err = DistError;
    fn from_str(s: &str) -> DistResult<Self> {
        if let Some(slug) = s.strip_prefix("./") {
            Ok(Self::User(slug.to_owned()))
        } else {
            Err(DistError::UnrecognizedJobStyle {
                style: s.to_owned(),
            })
        }
    }
}

impl<'de> serde::Deserialize<'de> for JobStyle {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let path = String::deserialize(deserializer)?;
        path.parse().map_err(|e| D::Error::custom(format!("{e}")))
    }
}

impl std::fmt::Display for JobStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStyle::User(s) => write!(f, "./{s}"),
        }
    }
}

/// The style of zip/tarball to make
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipStyle {
    /// `.zip`
    Zip,
    /// `.tar.<compression>`
    Tar(CompressionImpl),
    /// Don't bundle/compress this, it's just a temp dir
    TempDir,
}

/// Compression impls (used by [`ZipStyle::Tar`][])
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CompressionImpl {
    /// `.gz`
    Gzip,
    /// `.xz`
    Xzip,
    /// `.zst`
    Zstd,
}
impl ZipStyle {
    /// Get the extension used for this kind of zip
    pub fn ext(&self) -> &'static str {
        match self {
            ZipStyle::TempDir => "",
            ZipStyle::Zip => ".zip",
            ZipStyle::Tar(compression) => match compression {
                CompressionImpl::Gzip => ".tar.gz",
                CompressionImpl::Xzip => ".tar.xz",
                CompressionImpl::Zstd => ".tar.zst",
            },
        }
    }
}

impl Serialize for ZipStyle {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.ext())
    }
}

impl<'de> Deserialize<'de> for ZipStyle {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let ext = String::deserialize(deserializer)?;
        match &*ext {
            ".zip" => Ok(ZipStyle::Zip),
            ".tar.gz" => Ok(ZipStyle::Tar(CompressionImpl::Gzip)),
            ".tar.xz" => Ok(ZipStyle::Tar(CompressionImpl::Xzip)),
            ".tar.zstd" | ".tar.zst" => Ok(ZipStyle::Tar(CompressionImpl::Zstd)),
            _ => Err(D::Error::custom(format!(
                "unknown archive format {ext}, expected one of: .zip, .tar.gz, .tar.xz, .tar.zstd, .tar.zst"
            ))),
        }
    }
}

/// key for the install-path config that selects [`InstallPathStrategyCargoHome`][]
const CARGO_HOME_INSTALL_PATH: &str = "CARGO_HOME";

/// Strategy for install binaries
#[derive(Debug, Clone, PartialEq)]
pub enum InstallPathStrategy {
    /// install to $CARGO_HOME, falling back to ~/.cargo/
    CargoHome,
    /// install to this subdir of the user's home
    ///
    /// syntax: `~/subdir`
    HomeSubdir {
        /// The subdir of home to install to
        subdir: String,
    },
    /// install to this subdir of this env var
    ///
    /// syntax: `$ENV_VAR/subdir`
    EnvSubdir {
        /// The env var to get the base of the path from
        env_key: String,
        /// The subdir to install to
        subdir: String,
    },
}

impl std::str::FromStr for InstallPathStrategy {
    type Err = DistError;
    fn from_str(path: &str) -> DistResult<Self> {
        if path == CARGO_HOME_INSTALL_PATH {
            Ok(InstallPathStrategy::CargoHome)
        } else if let Some(subdir) = path.strip_prefix("~/") {
            if subdir.is_empty() {
                Err(DistError::InstallPathHomeSubdir {
                    path: path.to_owned(),
                })
            } else {
                Ok(InstallPathStrategy::HomeSubdir {
                    // If there's a trailing slash, strip it to be uniform
                    subdir: subdir.strip_suffix('/').unwrap_or(subdir).to_owned(),
                })
            }
        } else if let Some(val) = path.strip_prefix('$') {
            if let Some((env_key, subdir)) = val.split_once('/') {
                Ok(InstallPathStrategy::EnvSubdir {
                    env_key: env_key.to_owned(),
                    // If there's a trailing slash, strip it to be uniform
                    subdir: subdir.strip_suffix('/').unwrap_or(subdir).to_owned(),
                })
            } else {
                Err(DistError::InstallPathEnvSlash {
                    path: path.to_owned(),
                })
            }
        } else {
            Err(DistError::InstallPathInvalid {
                path: path.to_owned(),
            })
        }
    }
}

impl std::fmt::Display for InstallPathStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallPathStrategy::CargoHome => write!(f, "{}", CARGO_HOME_INSTALL_PATH),
            InstallPathStrategy::HomeSubdir { subdir } => write!(f, "~/{subdir}"),
            InstallPathStrategy::EnvSubdir { env_key, subdir } => write!(f, "${env_key}/{subdir}"),
        }
    }
}

impl serde::Serialize for InstallPathStrategy {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for InstallPathStrategy {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let path = String::deserialize(deserializer)?;
        path.parse().map_err(|e| D::Error::custom(format!("{e}")))
    }
}

/// A GitHub repo like 'axodotdev/axolotlsay'
#[derive(Debug, Clone, PartialEq)]
pub struct GithubRepoPair {
    /// owner (axodotdev)
    pub owner: String,
    /// repo (axolotlsay)
    pub repo: String,
}

impl std::str::FromStr for GithubRepoPair {
    type Err = DistError;
    fn from_str(pair: &str) -> DistResult<Self> {
        let Some((owner, repo)) = pair.split_once('/') else {
            return Err(DistError::GithubRepoPairParse {
                pair: pair.to_owned(),
            });
        };
        Ok(GithubRepoPair {
            owner: owner.to_owned(),
            repo: repo.to_owned(),
        })
    }
}

impl std::fmt::Display for GithubRepoPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.owner, self.repo)
    }
}

impl serde::Serialize for GithubRepoPair {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for GithubRepoPair {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let path = String::deserialize(deserializer)?;
        path.parse().map_err(|e| D::Error::custom(format!("{e}")))
    }
}

impl GithubRepoPair {
    /// Convert this into a jinja-friendly form
    pub fn into_jinja(self) -> JinjaGithubRepoPair {
        JinjaGithubRepoPair {
            owner: self.owner,
            repo: self.repo,
        }
    }
}

/// Jinja-friendly version of [`GithubRepoPair`][]
#[derive(Debug, Clone, Serialize)]
pub struct JinjaGithubRepoPair {
    /// owner
    pub owner: String,
    /// repo
    pub repo: String,
}

/// Strategy for install binaries (replica to have different Serialize for jinja)
///
/// The serialize/deserialize impls are already required for loading/saving the config
/// from toml/json, and that serialize impl just creates a plain string again. To allow
/// jinja templates to have richer context we have use duplicate type with a more
/// conventional derived serialize.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum JinjaInstallPathStrategy {
    /// install to $CARGO_HOME, falling back to ~/.cargo/
    CargoHome,
    /// install to this subdir of the user's home
    ///
    /// syntax: `~/subdir`
    HomeSubdir {
        /// The subdir of home to install to
        subdir: String,
    },
    /// install to this subdir of this env var
    ///
    /// syntax: `$ENV_VAR/subdir`
    EnvSubdir {
        /// The env var to get the base of the path from
        env_key: String,
        /// The subdir to install to
        subdir: String,
    },
}

impl InstallPathStrategy {
    /// Convert this into a jinja-friendly form
    pub fn into_jinja(self) -> JinjaInstallPathStrategy {
        match self {
            InstallPathStrategy::CargoHome => JinjaInstallPathStrategy::CargoHome,
            InstallPathStrategy::HomeSubdir { subdir } => {
                JinjaInstallPathStrategy::HomeSubdir { subdir }
            }
            InstallPathStrategy::EnvSubdir { env_key, subdir } => {
                JinjaInstallPathStrategy::EnvSubdir { env_key, subdir }
            }
        }
    }
}

/// A checksumming algorithm
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChecksumStyle {
    /// sha256sum (using the sha2 crate)
    Sha256,
    /// sha512sum (using the sha2 crate)
    Sha512,
    /// sha3-256sum (using the sha3 crate)
    Sha3_256,
    /// sha3-512sum (using the sha3 crate)
    Sha3_512,
    /// b2sum (using the blake2 crate)
    Blake2s,
    /// b2sum (using the blake2 crate)
    Blake2b,
    /// Do not checksum
    False,
}

impl ChecksumStyle {
    /// Get the extension of a checksum
    pub fn ext(self) -> &'static str {
        match self {
            ChecksumStyle::Sha256 => "sha256",
            ChecksumStyle::Sha512 => "sha512",
            ChecksumStyle::Sha3_256 => "sha3-256",
            ChecksumStyle::Sha3_512 => "sha3-512",
            ChecksumStyle::Blake2s => "blake2s",
            ChecksumStyle::Blake2b => "blake2b",
            ChecksumStyle::False => "false",
        }
    }
}

/// Which style(s) of configuration to generate
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GenerateMode {
    /// Generate CI scripts for orchestrating cargo-dist
    #[serde(rename = "ci")]
    Ci,
    /// Generate wsx (WiX) templates for msi installers
    #[serde(rename = "msi")]
    Msi,
}

impl std::fmt::Display for GenerateMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GenerateMode::Ci => "ci".fmt(f),
            GenerateMode::Msi => "msi".fmt(f),
        }
    }
}

/// Arguments to `cargo dist host`
#[derive(Clone, Debug)]
pub struct HostArgs {
    /// Which hosting steps to run
    pub steps: Vec<HostStyle>,
}

/// What parts of hosting to perform
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HostStyle {
    /// Check that hosting API keys are working
    Check,
    /// Create a location to host artifacts
    Create,
    /// Upload artifacts
    Upload,
    /// Release artifacts
    Release,
    /// Announce artifacts
    Announce,
}

impl std::fmt::Display for HostStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            HostStyle::Check => "check",
            HostStyle::Create => "create",
            HostStyle::Upload => "upload",
            HostStyle::Release => "release",
            HostStyle::Announce => "announce",
        };
        string.fmt(f)
    }
}

/// Packages to install before build from the system package manager
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SystemDependencies {
    /// Packages to install in Homebrew
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    // #[serde(with = "sysdep_derive")]
    pub homebrew: BTreeMap<String, SystemDependency>,
    /// Packages to install in apt
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub apt: BTreeMap<String, SystemDependency>,
    /// Package to install in Chocolatey
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub chocolatey: BTreeMap<String, SystemDependency>,
}

impl SystemDependencies {
    /// Extends `self` with the elements of `other`.
    pub fn append(&mut self, other: &mut Self) {
        self.homebrew.append(&mut other.homebrew);
        self.apt.append(&mut other.apt);
        self.chocolatey.append(&mut other.chocolatey);
    }
}

/// Represents a package from a system package manager
// newtype wrapper to hang a manual derive impl off of
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct SystemDependency(pub SystemDependencyComplex);

/// Backing type for SystemDependency
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub struct SystemDependencyComplex {
    /// The version to install, as expected by the underlying package manager
    pub version: Option<String>,
    /// Stages at which the dependency is required
    #[serde(default)]
    pub stage: Vec<DependencyKind>,
    /// One or more targets this package should be installed on; defaults to all targets if not specified
    #[serde(default)]
    pub targets: Vec<String>,
}

impl SystemDependencyComplex {
    /// Checks if this dependency should be installed on the specified target.
    pub fn wanted_for_target(&self, target: &String) -> bool {
        if self.targets.is_empty() {
            true
        } else {
            self.targets.contains(target)
        }
    }

    /// Checks if this dependency should used in the specified stage.
    pub fn stage_wanted(&self, stage: &DependencyKind) -> bool {
        if self.stage.is_empty() {
            match stage {
                DependencyKind::Build => true,
                DependencyKind::Run => false,
            }
        } else {
            self.stage.contains(stage)
        }
    }
}

/// Definition for a single package
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SystemDependencyKind {
    /// Simple specification format, parsed as cmake = 'version'
    /// The special string "*" is parsed as a None version
    Untagged(String),
    /// Complex specification format
    Tagged(SystemDependencyComplex),
}

/// Provides detail on when a specific dependency is required
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DependencyKind {
    /// A dependency that must be present when the software is being built
    Build,
    /// A dependency that must be present when the software is being used
    Run,
}

impl std::fmt::Display for DependencyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DependencyKind::Build => "build".fmt(f),
            DependencyKind::Run => "run".fmt(f),
        }
    }
}

impl<'de> Deserialize<'de> for SystemDependency {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let kind: SystemDependencyKind = SystemDependencyKind::deserialize(deserializer)?;

        let res = match kind {
            SystemDependencyKind::Untagged(version) => {
                let v = if version == "*" { None } else { Some(version) };
                SystemDependencyComplex {
                    version: v,
                    stage: vec![],
                    targets: vec![],
                }
            }
            SystemDependencyKind::Tagged(dep) => dep,
        };

        Ok(SystemDependency(res))
    }
}

/// Settings for which Generate targets can be dirty
#[derive(Debug, Clone)]
pub enum DirtyMode {
    /// Allow only these targets
    AllowList(Vec<GenerateMode>),
    /// Allow all targets
    AllowAll,
}

impl DirtyMode {
    /// Do we need to run this Generate Mode
    pub fn should_run(&self, mode: GenerateMode) -> bool {
        match self {
            DirtyMode::AllowAll => false,
            DirtyMode::AllowList(list) => !list.contains(&mode),
        }
    }
}

/// For features that can be generated in "test" or "production" mode
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductionMode {
    /// test mode
    Test,
    /// production mode
    Prod,
}

/// An extra artifact to upload alongside the release tarballs,
/// and the build command which produces it.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ExtraArtifact {
    /// The working dir to run the command in
    ///
    /// If blank, the directory of the manifest that defines this is used.
    #[serde(default)]
    #[serde(skip_serializing_if = "path_is_empty")]
    pub working_dir: Utf8PathBuf,
    /// The build command to invoke in the working_dir
    #[serde(rename = "build")]
    pub command: Vec<String>,
    /// Relative paths (from the working_dir) to artifacts that should be included
    #[serde(rename = "artifacts")]
    pub artifact_relpaths: Vec<Utf8PathBuf>,
}

/// Why doesn't this exist omg
fn path_is_empty(p: &Utf8PathBuf) -> bool {
    p.as_str().is_empty()
}

impl std::fmt::Display for ProductionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProductionMode::Test => "test".fmt(f),
            ProductionMode::Prod => "prod".fmt(f),
        }
    }
}

pub(crate) fn parse_metadata_table_or_manifest(
    workspace_type: WorkspaceKind,
    manifest_path: &Utf8Path,
    metadata_table: Option<&serde_json::Value>,
) -> DistResult<DistMetadata> {
    match workspace_type {
        WorkspaceKind::Javascript => unimplemented!("npm packages not yet supported here"),
        // Pre-parsed Rust metadata table
        WorkspaceKind::Rust => parse_metadata_table(manifest_path, metadata_table),
        // Generic dist.toml
        WorkspaceKind::Generic => {
            let src = SourceFile::load_local(manifest_path)?;
            parse_generic_config(src)
        }
    }
}

pub(crate) fn parse_generic_config(src: SourceFile) -> DistResult<DistMetadata> {
    let config: GenericConfig = src.deserialize_toml()?;
    Ok(config.dist)
}

pub(crate) fn parse_metadata_table(
    manifest_path: &Utf8Path,
    metadata_table: Option<&serde_json::Value>,
) -> DistResult<DistMetadata> {
    Ok(metadata_table
        .and_then(|t| t.get(METADATA_DIST))
        .map(DistMetadata::deserialize)
        .transpose()
        .map_err(|cause| DistError::CargoTomlParse {
            manifest_path: manifest_path.to_owned(),
            cause,
        })?
        .unwrap_or_default())
}

fn get_git_repo_root(run_in: &Utf8PathBuf) -> DistResult<Utf8PathBuf> {
    let mut command = Cmd::new("git", "get git repo root");
    command
        .arg("-C")
        .arg(run_in)
        .arg("rev-parse")
        .arg("--show-toplevel");

    let string = String::from_utf8(command.output()?.stdout)?
        .trim_end()
        .to_owned();

    Ok(Utf8PathBuf::from(string))
}

/// Find the dist workspaces relative to the current directory
pub fn get_project() -> Result<axoproject::WorkspaceGraph, axoproject::errors::ProjectError> {
    let start_dir = std::env::current_dir().expect("couldn't get current working dir!?");
    let start_dir = Utf8PathBuf::from_path_buf(start_dir).expect("project path isn't utf8!?");
    let clamp_to_dir = get_git_repo_root(&start_dir).ok();
    let workspaces = axoproject::WorkspaceGraph::find(&start_dir, clamp_to_dir.as_deref())?;
    Ok(workspaces)
}

/// Load a Cargo.toml into toml-edit form
pub fn load_cargo_toml(manifest_path: &Utf8Path) -> DistResult<toml_edit::DocumentMut> {
    let src = axoasset::SourceFile::load_local(manifest_path)?;
    let toml = src.deserialize_toml_edit()?;
    Ok(toml)
}

/// Save a Cargo.toml from toml-edit form
pub fn save_cargo_toml(manifest_path: &Utf8Path, toml: toml_edit::DocumentMut) -> DistResult<()> {
    let toml_text = toml.to_string();
    axoasset::LocalAsset::write_new(&toml_text, manifest_path)?;
    Ok(())
}

/// Get the `[workspace.metadata]` or `[package.metadata]` (based on `is_workspace`)
pub fn get_toml_metadata(
    toml: &mut toml_edit::DocumentMut,
    is_workspace: bool,
) -> &mut toml_edit::Item {
    // Walk down/prepare the components...
    let root_key = if is_workspace { "workspace" } else { "package" };
    let workspace = toml[root_key].or_insert(toml_edit::table());
    if let Some(t) = workspace.as_table_mut() {
        t.set_implicit(true)
    }
    let metadata = workspace["metadata"].or_insert(toml_edit::table());
    if let Some(t) = metadata.as_table_mut() {
        t.set_implicit(true)
    }

    metadata
}

/// This module implements support for serializing and deserializing
/// `Option<Vec<T>>> where T: Display + FromStr`
/// when we want both of these syntaxes to be valid:
///
/// * install-path = "~/.mycompany"
/// * install-path = ["$MY_COMPANY", "~/.mycompany"]
///
/// Notable corners of roundtripping:
///
/// * `["one_elem"]`` will be force-rewritten as `"one_elem"` (totally equivalent and prettier)
/// * `[]` will be preserved as `[]` (it's semantically distinct from None when cascading config)
///
/// This is a variation on a documented serde idiom for "string or struct":
/// <https://serde.rs/string-or-struct.html>
mod opt_string_or_vec {
    use super::*;
    use serde::de::Error;

    pub fn serialize<S, T>(v: &Option<Vec<T>>, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
        T: std::fmt::Display,
    {
        // If none, do none
        let Some(vec) = v else {
            return s.serialize_none();
        };
        // If one item, make it a string
        if vec.len() == 1 {
            s.serialize_str(&vec[0].to_string())
        // If many items (or zero), make it a list
        } else {
            let string_vec = Vec::from_iter(vec.iter().map(ToString::to_string));
            string_vec.serialize(s)
        }
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<Vec<T>>, D::Error>
    where
        D: serde::Deserializer<'de>,
        T: std::str::FromStr,
        T::Err: std::fmt::Display,
    {
        struct StringOrVec<T>(std::marker::PhantomData<T>);

        impl<'de, T> serde::de::Visitor<'de> for StringOrVec<T>
        where
            T: std::str::FromStr,
            T::Err: std::fmt::Display,
        {
            type Value = Option<Vec<T>>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("string or list of strings")
            }

            // if none, return none
            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(None)
            }

            // if string, parse it and make a single-element list
            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(Some(vec![s
                    .parse()
                    .map_err(|e| E::custom(format!("{e}")))?]))
            }

            // if a sequence, parse the whole thing
            fn visit_seq<S>(self, seq: S) -> Result<Self::Value, S::Error>
            where
                S: serde::de::SeqAccess<'de>,
            {
                let vec: Vec<String> =
                    Deserialize::deserialize(serde::de::value::SeqAccessDeserializer::new(seq))?;
                let parsed: Result<Vec<T>, S::Error> = vec
                    .iter()
                    .map(|s| s.parse::<T>().map_err(|e| S::Error::custom(format!("{e}"))))
                    .collect();
                Ok(Some(parsed?))
            }
        }

        deserializer.deserialize_any(StringOrVec::<T>(std::marker::PhantomData))
    }
}
