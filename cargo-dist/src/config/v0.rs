//! v0 config

use camino::{Utf8Path, Utf8PathBuf};
use cargo_dist_schema::{
    declare_strongly_typed_string, GithubRunner, GithubRunnerConfigInput, StringLikeOr,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use tracing::log::warn;

use super::*;
use crate::platform::MinGlibcVersion;
use crate::SortedMap;

/// A container to assist deserializing metadata from dist(-workspace).tomls
#[derive(Debug, Deserialize)]
pub struct GenericConfig {
    /// The dist field within dist.toml
    pub dist: Option<DistMetadata>,
}

declare_strongly_typed_string! {
    /// A URL to use to install `cargo-dist` (with the installer script).
    /// This overwrites `cargo_dist_version` and expects the URL to have
    /// a similar structure to `./target/distrib` after running `dist build`
    /// on itself.
    pub struct CargoDistUrlOverride => &CargoDistUrlOverrideRef;
}

/// Contents of METADATA_DIST in Cargo.toml files
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct DistMetadata {
    /// The intended version of dist to build with. (normal Cargo SemVer syntax)
    ///
    /// When generating full tasks graphs (such as CI scripts) we will pick this version.
    ///
    /// FIXME: Should we produce a warning if running locally with a different version? In theory
    /// it shouldn't be a problem and newer versions should just be Better... probably you
    /// Really want to have the exact version when running generate to avoid generating
    /// things other dist versions can't handle!
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cargo_dist_version: Option<Version>,

    /// See [`CargoDistUrlOverride`]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cargo_dist_url_override: Option<CargoDistUrlOverride>,

    /// (deprecated) The intended version of Rust/Cargo to build with (rustup toolchain syntax)
    ///
    /// When generating full tasks graphs (such as CI scripts) we will pick this version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rust_toolchain_version: Option<String>,

    /// Whether the package should be distributed/built by dist
    ///
    /// This mainly exists to be set to `false` to make dist ignore the existence of this
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

    /// Generate targets whose dist should avoid checking for up-to-dateness.
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

    /// Custom success message for installers
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
    pub targets: Option<Vec<TripleName>>,

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

    /// Which checksum algorithm to use, from: sha256, sha512, sha3-256,
    /// sha3-512, blake2s, blake2b, or false (to disable checksums)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<ChecksumStyle>,

    /// Build only the required packages, and individually (since 0.1.0) (default: false)
    ///
    /// By default when we need to build anything in your workspace, we build your entire workspace
    /// with --workspace. This setting tells dist to instead build each app individually.
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
    /// sacrificing latency and fault-isolation for more the sake of minor efficiency gains.
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
    /// dist was designed around the "keep trying" approach, as we create a draft Release
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

    /// Whether CI tasks should have build caches enabled.
    ///
    /// Defaults false because currently Cargo.toml / Cargo.lock changes
    /// invalidate the cache, making it useless for typical usage
    /// (since bumping your version changes both those files).
    ///
    /// As of this writing the two major exceptions to when it would be
    /// useful are `pr-run-mode = "upload"` and `release-branch = "my-branch"`
    /// which can run the CI action frequently and without Cargo.toml changes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_builds: Option<bool>,

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
    /// A list of features to enable when building a package with dist
    ///
    /// (defaults to none)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<Vec<String>>,
    /// Whether to enable when building a package with dist
    ///
    /// (defaults to true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_features: Option<bool>,
    /// Whether to enable all features building a package with dist
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
    /// Ordinarily, dist tries to detect if your release
    /// is a prerelease based on its version number using
    /// semver standards. If it's a prerelease, it will be
    /// marked as a prerelease in hosting services such as
    /// GitHub and Axo Releases.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_latest: Option<bool>,

    /// Whether we should create the Github Release for you when you push a tag.
    ///
    /// If true (default), dist will create a new Github Release and generate
    /// a title/body for it based on your changelog.
    ///
    /// If false, dist will assume a draft Github Release already exists
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

    /// Whether we should sign Mac binaries
    #[serde(skip_serializing_if = "Option::is_none")]
    pub macos_sign: Option<bool>,

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
    pub github_custom_runners:
        Option<SortedMap<TripleName, StringLikeOr<GithubRunner, GithubRunnerConfigInput>>>,

    /// Custom permissions for jobs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_custom_job_permissions: Option<SortedMap<String, GithubPermissionMap>>,

    /// Use these specific commits of these specific actions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_action_commits: Option<SortedMap<String, String>>,

    /// Aliases to install binaries as
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin_aliases: Option<SortedMap<String, Vec<String>>>,

    /// a prefix to add to the release.yml and tag pattern so that
    /// dist can co-exist with other release workflows in complex workspaces
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_namespace: Option<String>,

    /// Whether to install an updater program alongside the software
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_updater: Option<bool>,

    /// Whether to always use the latest axoupdater instead of a known-good version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub always_use_latest_updater: Option<bool>,

    /// Whether artifacts/installers for this app should be displayed in release bodies
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<bool>,

    /// How to refer to the app in release bodies
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    /// Whether to include built libraries in the release archive
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, with = "opt_string_or_vec")]
    pub package_libraries: Option<Vec<LibraryStyle>>,

    /// Whether installers should install libraries from the release archive
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, with = "opt_string_or_vec")]
    pub install_libraries: Option<Vec<LibraryStyle>>,

    /// Any additional steps that need to be performed before building local artifacts
    #[serde(default)]
    pub github_build_setup: Option<String>,

    /// Configuration specific to Mac .pkg installers
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub mac_pkg_config: Option<MacPkgConfig>,

    /// Override the native glibc version, if it isn't auto-detected correctly
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub min_glibc_version: Option<MinGlibcVersion>,

    /// Whether to embed dependency information in the executable.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub cargo_auditable: Option<bool>,

    /// Whether to use cargo-cyclonedx to generate an SBOM.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub cargo_cyclonedx: Option<bool>,

    /// Whether to generate OmniBOR artifact IDs.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub omnibor: Option<bool>,
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
            cargo_dist_url_override: _,
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
            cache_builds: _,
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
            macos_sign: _,
            github_attestations: _,
            msvc_crt_static: _,
            hosting: _,
            github_custom_runners: _,
            github_custom_job_permissions: _,
            github_action_commits: _,
            bin_aliases: _,
            tag_namespace: _,
            install_updater: _,
            always_use_latest_updater: _,
            github_releases_repo: _,
            github_releases_submodule_path: _,
            display: _,
            display_name: _,
            package_libraries: _,
            install_libraries: _,
            github_build_setup: _,
            mac_pkg_config: _,
            min_glibc_version: _,
            cargo_auditable: _,
            cargo_cyclonedx: _,
            omnibor: _,
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
            cargo_dist_url_override,
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
            cache_builds,
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
            macos_sign,
            github_attestations,
            msvc_crt_static,
            hosting,
            extra_artifacts,
            github_custom_runners,
            github_custom_job_permissions,
            github_action_commits,
            bin_aliases,
            tag_namespace,
            install_updater,
            always_use_latest_updater,
            github_releases_repo,
            github_releases_submodule_path,
            display,
            display_name,
            package_libraries,
            install_libraries,
            github_build_setup,
            mac_pkg_config,
            min_glibc_version,
            cargo_auditable,
            cargo_cyclonedx,
            omnibor,
        } = self;

        // Check for global settings on local packages
        if cargo_dist_version.is_some() {
            warn!("package.metadata.dist.cargo-dist-version is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if cargo_dist_url_override.is_some() {
            warn!("package.metadata.dist.cargo-dist-url-override is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
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
        if cache_builds.is_some() {
            warn!("package.metadata.dist.cache-builds is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
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
        if macos_sign.is_some() {
            warn!("package.metadata.dist.macos-sign is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
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
            warn!("package.metadata.dist.github-release is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if github_custom_job_permissions.is_some() {
            warn!("package.metadata.dist.github-custom-job-permissions is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if github_action_commits.is_some() {
            warn!("package.metadata.dist.github-action-commits is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if github_custom_runners.is_some() {
            warn!("package.metadata.dist.github-custom-runners is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if github_build_setup.is_some() {
            warn!("package.metadata.dist.github-build-setup is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
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
        if bin_aliases.is_none() {
            bin_aliases.clone_from(&workspace_config.bin_aliases);
        }
        if install_updater.is_none() {
            *install_updater = workspace_config.install_updater;
        }
        if always_use_latest_updater.is_none() {
            *always_use_latest_updater = workspace_config.always_use_latest_updater;
        }
        if display.is_none() {
            *display = workspace_config.display;
        }
        if display_name.is_none() {
            display_name.clone_from(&workspace_config.display_name);
        }
        if package_libraries.is_none() {
            package_libraries.clone_from(&workspace_config.package_libraries);
        }
        if install_libraries.is_none() {
            install_libraries.clone_from(&workspace_config.install_libraries);
        }
        if mac_pkg_config.is_none() {
            mac_pkg_config.clone_from(&workspace_config.mac_pkg_config);
        }
        if min_glibc_version.is_none() {
            min_glibc_version.clone_from(&workspace_config.min_glibc_version);
        }
        if cargo_auditable.is_none() {
            cargo_auditable.clone_from(&workspace_config.cargo_auditable);
        }
        if cargo_cyclonedx.is_none() {
            cargo_cyclonedx.clone_from(&workspace_config.cargo_cyclonedx);
        }
        if omnibor.is_none() {
            omnibor.clone_from(&workspace_config.omnibor);
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
