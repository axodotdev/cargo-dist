#![deny(missing_docs)]

//! # cargo-dist-schema
//!
//! This crate exists to serialize and deserialize the dist-manifest.json produced
//! by dist. Ideally it should be reasonably forward and backward compatible
//! with different versions of this format.
//!
//! The root type of the schema is [`DistManifest`][].

pub mod macros;
pub use target_lexicon;

use std::{collections::BTreeMap, str::FromStr};

use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};
use target_lexicon::Triple;

declare_strongly_typed_string! {
    /// A rustc-like target triple/tuple (e.g. "x86_64-pc-windows-msvc")
    pub struct TripleName => &TripleNameRef;
}

impl TripleNameRef {
    /// Parse as a [`Triple`]
    pub fn parse(&self) -> Result<Triple, <Triple as FromStr>::Err> {
        Triple::from_str(self.as_str())
    }

    /// Returns true if this target triple contains the word "musl"
    pub fn is_musl(&self) -> bool {
        self.0.contains("musl")
    }

    /// Returns true if this target triple contains the word "linux"
    pub fn is_linux(&self) -> bool {
        self.0.contains("linux")
    }

    /// Returns true if this target triple contains the word "apple"
    pub fn is_apple(&self) -> bool {
        self.0.contains("apple")
    }

    /// Returns true if this target triple contains the word "darwin"
    pub fn is_darwin(&self) -> bool {
        self.0.contains("darwin")
    }

    /// Returns true if this target triple contains the word "windows"
    pub fn is_windows(&self) -> bool {
        self.0.contains("windows")
    }

    /// Returns true if this target triple contains the word "x86_64"
    pub fn is_x86_64(&self) -> bool {
        self.0.contains("x86_64")
    }

    /// Returns true if this target triple contains the word "aarch64"
    pub fn is_aarch64(&self) -> bool {
        self.0.contains("aarch64")
    }

    //---------------------------
    // common combinations

    /// Returns true if this target triple contains the string "linux-musl"
    pub fn is_linux_musl(&self) -> bool {
        self.0.contains("linux-musl")
    }

    /// Returns true if this target triple contains the string "windows-msvc"
    pub fn is_windows_msvc(&self) -> bool {
        self.0.contains("windows-msvc")
    }
}
declare_strongly_typed_string! {
    /// The name of a Github Actions Runner, like `ubuntu-22.04` or `macos-13`
    pub struct GithubRunner => &GithubRunnerRef;

    /// A container image, like `quay.io/pypa/manylinux_2_28_x86_64`
    pub struct ContainerImage => &ContainerImageRef;
}

/// Github runners configuration (which github image/container should be used
/// to build which target).
pub type GithubRunners = BTreeMap<TripleName, GithubRunnerConfig>;

impl GithubRunnerRef {
    /// Does the runner name contain the word "buildjet"?
    pub fn is_buildjet(&self) -> bool {
        self.as_str().contains("buildjet")
    }
}

/// A value or just a string
///
/// This allows us to have a simple string-based version of a config while still
/// allowing for a more advanced version to exist.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(untagged)]
pub enum StringLikeOr<S, T> {
    /// They gave the simple string-like value (see `declare_strongly_typed_string!`)
    StringLike(S),
    /// They gave a more interesting value
    Val(T),
}

impl<S, T> StringLikeOr<S, T> {
    /// Constructs a new `StringLikeOr` from the string-like value `s`
    pub fn from_s(s: S) -> Self {
        Self::StringLike(s)
    }

    /// Constructs a new `StringLikeOr` from the more interesting value `t`
    pub fn from_t(t: T) -> Self {
        Self::Val(t)
    }
}

/// A local system path on the machine dist was run.
///
/// This is a String because when deserializing this may be a path format from a different OS!
pub type LocalPath = String;
/// A relative path inside an artifact
///
/// This is a String because when deserializing this may be a path format from a different OS!
///
/// (Should we normalize this one?)
pub type RelPath = String;

declare_strongly_typed_string! {
    /// The unique ID of an Artifact
    pub struct ArtifactId => &ArtifactIdRef;
}

/// The unique ID of a System
pub type SystemId = String;
/// The unique ID of an Asset
pub type AssetId = String;
/// A sorted set of values
pub type SortedSet<T> = std::collections::BTreeSet<T>;

/// A report of the releases and artifacts that dist generated
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DistManifest {
    /// The version of dist that generated this
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dist_version: Option<String>,
    /// The (git) tag associated with this announcement
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub announcement_tag: Option<String>,
    /// True if --tag wasn't explicitly passed to dist. This usually indicates
    /// some kind of dry-run state like pr-run-mode=upload. Some third-party tools
    /// may use this as a proxy for "is dry run"
    #[serde(default)]
    pub announcement_tag_is_implicit: bool,
    /// Whether this announcement appears to be a prerelease
    #[serde(default)]
    pub announcement_is_prerelease: bool,
    /// A title for the announcement
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub announcement_title: Option<String>,
    /// A changelog for the announcement
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub announcement_changelog: Option<String>,
    /// A Github Releases body for the announcement
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub announcement_github_body: Option<String>,
    /// Info about the toolchain used to build this announcement
    ///
    /// DEPRECATED: never appears anymore
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_info: Option<SystemInfo>,
    /// App releases we're distributing
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub releases: Vec<Release>,
    /// The artifacts included in this Announcement, referenced by releases.
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub artifacts: BTreeMap<ArtifactId, Artifact>,
    /// The systems that artifacts were built on
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub systems: BTreeMap<SystemId, SystemInfo>,
    /// The assets contained within artifacts (binaries)
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub assets: BTreeMap<AssetId, AssetInfo>,
    /// Whether to publish prereleases to package managers
    #[serde(default)]
    pub publish_prereleases: bool,
    /// Where possible, announce/publish a release as "latest" regardless of semver version
    #[serde(default)]
    pub force_latest: bool,
    /// ci backend info
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ci: Option<CiInfo>,
    /// Data about dynamic linkage in the built libraries
    #[serde(default)]
    // FIXME: turn on this skip_serializing_if at some point.
    // old dist-manifest consumers don't think this field can
    // be missing, so it's unsafe to stop emitting it, but
    // we want to deprecate it at some point.
    // #[serde(skip_serializing_if = "Vec::is_empty")]
    pub linkage: Vec<Linkage>,
    /// Files to upload
    #[serde(default)]
    // We need to make sure we always serialize this when it's empty,
    // because we index into this array unconditionally during upload.
    pub upload_files: Vec<String>,
    /// Whether Artifact Attestations should be found in the GitHub Release
    ///
    /// <https://github.blog/2024-05-02-introducing-artifact-attestations-now-in-public-beta/>
    #[serde(default)]
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub github_attestations: bool,
    /// Patterns to attest when creating Artifact Attestations
    #[serde(default)]
    #[serde(skip_serializing_if = "GithubAttestationsFilters::is_default")]
    pub github_attestations_filters: GithubAttestationsFilters,
    /// When to generate Artifact Attestations
    ///
    /// Defaults to "build-local-artifacts" for backwards compatibility
    #[serde(default)]
    #[serde(skip_serializing_if = "GithubAttestationsPhase::is_default")]
    pub github_attestations_phase: GithubAttestationsPhase,
}

/// Information about the build environment on this system
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum BuildEnvironment {
    /// Linux-specific information
    #[serde(rename = "linux")]
    Linux {
        /// The builder's glibc version, relevant to glibc-based
        /// builds.
        glibc_version: Option<GlibcVersion>,
    },
    /// macOS-specific information
    #[serde(rename = "macos")]
    MacOS {
        /// The version of macOS used by the builder
        os_version: String,
    },
    /// Windows-specific information
    #[serde(rename = "windows")]
    Windows,
    /// Unable to determine what the host OS was - error?
    #[serde(rename = "indeterminate")]
    Indeterminate,
}

/// Minimum glibc version required to run software
#[derive(
    Debug, Clone, Serialize, Deserialize, JsonSchema, Hash, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct GlibcVersion {
    /// Major version
    pub major: u64,
    /// Series (minor) version
    pub series: u64,
}

impl Default for GlibcVersion {
    fn default() -> Self {
        Self {
            // Values from the default Ubuntu runner
            major: 2,
            series: 31,
        }
    }
}

/// Info about an Asset (binary)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AssetInfo {
    /// unique id of the Asset
    pub id: AssetId,
    /// filename of the Asset
    pub name: String,
    /// the system it was built on
    pub system: SystemId,
    /// rust-style target triples the Asset natively supports
    ///
    /// * length 0: not a meaningful question, maybe some static file
    /// * length 1: typical of binaries
    /// * length 2+: some kind of universal binary
    pub target_triples: Vec<TripleName>,
    /// the linkage of this Asset
    pub linkage: Option<Linkage>,
}

/// CI backend info
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CiInfo {
    /// GitHub CI backend
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github: Option<GithubCiInfo>,
}

/// Github CI backend
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GithubCiInfo {
    /// Github CI Matrix for upload-artifacts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifacts_matrix: Option<GithubMatrix>,

    /// What kind of job to run on pull request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_run_mode: Option<PrRunMode>,

    /// A specific commit to tag in an external repository
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_repo_commit: Option<String>,
}

/// Github CI Matrix
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GithubMatrix {
    /// define each task manually rather than doing cross-product stuff
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<GithubLocalJobConfig>,
}

impl GithubMatrix {
    /// Gets if the matrix has no entries
    ///
    /// this is useful for checking if there should be No matrix
    pub fn is_empty(&self) -> bool {
        self.include.is_empty()
    }
}

declare_strongly_typed_string! {
    /// A bit of shell script to install brew/apt/chocolatey/etc. packages
    pub struct PackageInstallScript => &PackageInstallScriptRef;
}

/// The version of `GithubRunnerConfig` that's deserialized from the config file: it
/// has optional fields that are computed later.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GithubRunnerConfigInput {
    /// GHA's `runs-on` key: Github Runner image to use, see <https://github.com/actions/runner-images>
    /// and <https://docs.github.com/en/actions/writing-workflows/choosing-where-your-workflow-runs/choosing-the-runner-for-a-job>
    ///
    /// This is not necessarily a well-known runner, it could be something self-hosted, it
    /// could be from BuildJet, Namespace, etc.
    ///
    /// If not specified, `container` has to be set.
    pub runner: Option<GithubRunner>,

    /// Host triple of the runner (well-known, custom, or best guess).
    /// If the runner is one of GitHub's official runner images, the platform
    /// is hardcoded. If it's custom, then we have a `target_triple => runner` in the config
    pub host: Option<TripleName>,

    /// Container image to run the job in, using GitHub's builtin
    /// container support, see <https://docs.github.com/en/actions/writing-workflows/choosing-where-your-workflow-runs/running-jobs-in-a-container>
    ///
    /// This doesn't allow mounting volumes, or anything, because we're only able
    /// to set the `container` key to something stringy
    ///
    /// If not specified, `runner` has to be set.
    pub container: Option<StringLikeOr<ContainerImage, ContainerConfigInput>>,
}

/// GitHub config that's common between different kinds of jobs (global, local)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
pub struct GithubRunnerConfig {
    /// GHA's `runs-on` key: Github Runner image to use, see <https://github.com/actions/runner-images>
    /// and <https://docs.github.com/en/actions/writing-workflows/choosing-where-your-workflow-runs/choosing-the-runner-for-a-job>
    ///
    /// This is not necessarily a well-known runner, it could be something self-hosted, it
    /// could be from BuildJet, Namespace, etc.
    pub runner: GithubRunner,

    /// Host triple of the runner (well-known, custom, or best guess).
    /// If the runner is one of GitHub's official runner images, the platform
    /// is hardcoded. If it's custom, then we have a `target_triple => runner` in the config
    pub host: TripleName,

    /// Container image to run the job in, using GitHub's builtin
    /// container support, see <https://docs.github.com/en/actions/writing-workflows/choosing-where-your-workflow-runs/running-jobs-in-a-container>
    ///
    /// This doesn't allow mounting volumes, or anything, because we're only able
    /// to set the `container` key to something stringy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container: Option<ContainerConfig>,
}

impl GithubRunnerConfig {
    /// If the container runs through a container, that container might have a different
    /// architecture than the outer VM — this returns the container's triple if any,
    /// and falls back to the "machine"'s triple if not.
    pub fn real_triple_name(&self) -> &TripleNameRef {
        if let Some(container) = &self.container {
            &container.host
        } else {
            &self.host
        }
    }

    /// cf. [`Self::real_triple_name`], but parsed
    pub fn real_triple(&self) -> Triple {
        self.real_triple_name().parse().unwrap()
    }
}

/// GitHub config that's common between different kinds of jobs (global, local)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContainerConfigInput {
    /// The container image to run, something like `ubuntu:22.04` or
    /// `quay.io/pypa/manylinux_2_28_x86_64`
    pub image: ContainerImage,

    /// The host triple of the container, something like `x86_64-unknown-linux-gnu`
    /// or `aarch64-unknown-linux-musl` or whatever.
    pub host: Option<TripleName>,

    /// The package manager to use within the container, like `apt`.
    #[serde(rename = "package-manager")]
    pub package_manager: Option<PackageManager>,
}

/// GitHub config that's common between different kinds of jobs (global, local)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
pub struct ContainerConfig {
    /// The container image to run, something like `ubuntu:22.04` or
    /// `quay.io/pypa/manylinux_2_28_x86_64`
    pub image: ContainerImage,

    /// The host triple of the container, something like `x86_64-unknown-linux-gnu`
    /// or `aarch64-unknown-linux-musl` or whatever.
    pub host: TripleName,

    /// The package manager to use within the container, like `apt`.
    pub package_manager: Option<PackageManager>,
}

/// Used in `github/release.yml.j2` to template out "global" build jobs
/// (plan, global assets, announce, etc)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GithubGlobalJobConfig {
    /// Where to run this job?
    #[serde(flatten)]
    pub runner: GithubRunnerConfig,

    /// Expression to execute to install dist
    pub install_dist: GhaRunStep,

    /// Arguments to pass to dist
    pub dist_args: String,

    /// Expression to execute to install cargo-cyclonedx
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_cargo_cyclonedx: Option<GhaRunStep>,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Expression to execute to install omnibor-cli
    pub install_omnibor: Option<GhaRunStep>,
}

/// Used in `github/release.yml.j2` to template out "local" build jobs
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GithubLocalJobConfig {
    /// Where to run this job?
    #[serde(flatten)]
    pub runner: GithubRunnerConfig,

    /// Expression to execute to install dist
    pub install_dist: GhaRunStep,

    /// Arguments to pass to dist
    pub dist_args: String,

    /// Target triples to build for
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<TripleName>>,

    /// Expression to execute to install cargo-auditable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_cargo_auditable: Option<GhaRunStep>,

    /// Expression to execute to install omnibor-cli
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_omnibor: Option<GhaRunStep>,

    /// Command to run to install dependencies
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packages_install: Option<PackageInstallScript>,

    /// What cache provider to use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_provider: Option<String>,
}

/// Used to capture GitHub Attestations filters
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct GithubAttestationsFilters(Vec<String>);

impl Default for GithubAttestationsFilters {
    fn default() -> Self {
        Self(vec!["*".to_string()])
    }
}

impl<'a> IntoIterator for &'a GithubAttestationsFilters {
    type Item = &'a String;
    type IntoIter = std::slice::Iter<'a, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl GithubAttestationsFilters {
    fn is_default(&self) -> bool {
        *self == Default::default()
    }
}

/// Phase in which to generate GitHub attestations
#[derive(Debug, Copy, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub enum GithubAttestationsPhase {
    /// Generate attestations during the `host` phase
    #[serde(rename = "host")]
    Host,
    /// Generate attestations during `build-local-artifacts` (default for backwards compatibility)
    #[default]
    #[serde(rename = "build-local-artifacts")]
    BuildLocalArtifacts,
}

impl GithubAttestationsPhase {
    fn is_default(&self) -> bool {
        matches!(self, GithubAttestationsPhase::BuildLocalArtifacts)
    }
}

impl std::fmt::Display for GithubAttestationsPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GithubAttestationsPhase::Host => write!(f, "host"),
            GithubAttestationsPhase::BuildLocalArtifacts => write!(f, "build-local-artifacts"),
        }
    }
}

/// A GitHub Actions "run" step, either bash or powershell
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
// this mirrors GHA's structure, see
//   * <https://serde.rs/enum-representations.html>
//   * <https://docs.github.com/en/actions/writing-workflows/workflow-syntax-for-github-actions#jobsjob_idstepsshell>
#[serde(tag = "shell", content = "run")]
pub enum GhaRunStep {
    /// see [`DashScript`]
    #[serde(rename = "sh")]
    Dash(DashScript),
    /// see [`PowershellScript`]
    #[serde(rename = "pwsh")]
    Powershell(PowershellScript),
}

impl From<DashScript> for GhaRunStep {
    fn from(bash: DashScript) -> Self {
        Self::Dash(bash)
    }
}

impl From<PowershellScript> for GhaRunStep {
    fn from(powershell: PowershellScript) -> Self {
        Self::Powershell(powershell)
    }
}

declare_strongly_typed_string! {
    /// A bit of shell script (that can run with `/bin/sh`), ran on CI runners. Can be multi-line.
    pub struct DashScript => &DashScriptRef;

    /// A bit of powershell script, ran on CI runners. Can be multi-line.
    pub struct PowershellScript => &PowershellScriptRef;
}

/// Type of job to run on pull request
#[derive(
    Debug, Copy, Clone, Serialize, Deserialize, JsonSchema, Default, PartialEq, Eq, PartialOrd, Ord,
)]
pub enum PrRunMode {
    /// Do not run on pull requests at all
    #[serde(rename = "skip")]
    Skip,
    /// Only run the plan step
    #[default]
    #[serde(rename = "plan")]
    Plan,
    /// Build and upload artifacts
    #[serde(rename = "upload")]
    Upload,
}

impl std::fmt::Display for PrRunMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrRunMode::Skip => write!(f, "skip"),
            PrRunMode::Plan => write!(f, "plan"),
            PrRunMode::Upload => write!(f, "upload"),
        }
    }
}

/// Info about a system used to build this announcement.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SystemInfo {
    /// The unique id of the System
    pub id: SystemId,
    /// The version of Cargo used (first line of cargo -vV)
    pub cargo_version_line: Option<String>,
    /// Environment of the System
    pub build_environment: BuildEnvironment,
}

/// Release-specific environment variables
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EnvironmentVariables {
    /// Environment variable to force an install location
    pub install_dir_env_var: String,
    /// Environment variable to force an unmanaged install location
    pub unmanaged_dir_env_var: String,
    /// Environment variable to disable updater features
    pub disable_update_env_var: String,
    /// Environment variable to disable modifying the path
    pub no_modify_path_env_var: String,
    /// Environment variable to make the installer more quiet
    pub print_quiet_env_var: String,
    /// Environment variable to make the installer more verbose
    pub print_verbose_env_var: String,
    /// Environment variable to override the URL to download from
    ///
    /// This trumps the base_url env vars below.
    pub download_url_env_var: String,
    /// Environment variable to set the GitHub base URL
    ///
    /// `{owner}/{repo}` will be added to the end of this value to
    /// construct the installer_download_url.
    pub github_base_url_env_var: String,
    /// Environment variable to set the GitHub Enterprise base URL
    ///
    /// `{owner}/{repo}` will be added to the end of this value to
    /// construct the installer_download_url.
    pub ghe_base_url_env_var: String,
    /// Environment variable to set the GitHub BEARER token when fetching archives
    pub github_token_env_var: String,
}

/// A Release of an Application
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Release {
    /// The name of the app
    pub app_name: String,
    /// The version of the app
    // FIXME: should be a Version but JsonSchema doesn't support (yet?)
    pub app_version: String,
    /// Environment variables which control this release's installer's behaviour
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<EnvironmentVariables>,
    /// Alternative display name that can be prettier
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Whether to advertise this app's installers/artifacts in announcements
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<bool>,
    /// The artifacts for this release (zips, debuginfo, metadata...)
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ArtifactId>,
    /// Hosting info
    #[serde(default)]
    #[serde(skip_serializing_if = "Hosting::is_empty")]
    pub hosting: Hosting,
}

declare_strongly_typed_string! {
    /// A lowercase descriptor for a checksum algorithm, like "sha256"
    /// or "blake2b".
    ///
    /// TODO(amos): Honestly this type should not exist, it's just what
    /// `ChecksumStyle` serializes to. `ChecksumsStyle` should just
    /// be serializable, that's it.
    pub struct ChecksumExtension => &ChecksumExtensionRef;

    /// A checksum value, usually the lower-cased hex string of the checksum
    pub struct ChecksumValue => &ChecksumValueRef;
}

/// A distributable artifact that's part of a Release
///
/// i.e. a zip or installer
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Artifact {
    /// The unique name of the artifact (e.g. `myapp-v1.0.0-x86_64-pc-windows-msvc.zip`)
    ///
    /// If this is missing then that indicates the artifact is purely informative and has
    /// no physical files associated with it. This may be used (in the future) to e.g.
    /// indicate you can install the application with `cargo install` or `npm install`.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub name: Option<ArtifactId>,
    /// The kind of artifact this is (e.g. "executable-zip")
    #[serde(flatten)]
    pub kind: ArtifactKind,
    /// The target triple of the bundle
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub target_triples: Vec<TripleName>,
    /// The location of the artifact on the local system
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub path: Option<LocalPath>,
    /// Assets included in the bundle (like executables and READMEs)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub assets: Vec<Asset>,
    /// A string describing how to install this
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub install_hint: Option<String>,
    /// A brief description of what this artifact is
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub description: Option<String>,
    /// id of an Artifact that contains the checksum for this Artifact
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub checksum: Option<ArtifactId>,
    /// checksums for this artifact
    ///
    /// keys are the name of an algorithm like "sha256" or "sha512"
    /// values are the actual hex string of the checksum
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub checksums: BTreeMap<ChecksumExtension, ChecksumValue>,
}

/// An asset contained in an artifact (executable, license, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Asset {
    /// A unique opaque id for an Asset
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The high-level name of the asset
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The path of the asset relative to the root of the artifact
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<RelPath>,
    /// The kind of asset this is
    #[serde(flatten)]
    pub kind: AssetKind,
}

/// An artifact included in a Distributable
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind")]
#[non_exhaustive]
pub enum AssetKind {
    /// An executable artifact
    #[serde(rename = "executable")]
    Executable(ExecutableAsset),
    /// A C dynamic library
    #[serde(rename = "c_dynamic_library")]
    CDynamicLibrary(DynamicLibraryAsset),
    /// A C static library
    #[serde(rename = "c_static_library")]
    CStaticLibrary(StaticLibraryAsset),
    /// A README file
    #[serde(rename = "readme")]
    Readme,
    /// A LICENSE file
    #[serde(rename = "license")]
    License,
    /// A CHANGELOG or RELEASES file
    #[serde(rename = "changelog")]
    Changelog,
    /// Unknown to this version of cargo-dist-schema
    ///
    /// This is a fallback for forward/backward-compat
    #[serde(other)]
    #[serde(rename = "unknown")]
    Unknown,
}

/// A kind of Artifact
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind")]
#[non_exhaustive]
pub enum ArtifactKind {
    /// A zip or a tarball
    #[serde(rename = "executable-zip")]
    ExecutableZip,
    /// Standalone Symbols/Debuginfo for a build
    #[serde(rename = "symbols")]
    Symbols,
    /// Installer
    #[serde(rename = "installer")]
    Installer,
    /// A checksum of another artifact
    #[serde(rename = "checksum")]
    Checksum,
    /// The checksums of many artifacts
    #[serde(rename = "unified-checksum")]
    UnifiedChecksum,
    /// A tarball containing the source code
    #[serde(rename = "source-tarball")]
    SourceTarball,
    /// Some form of extra artifact produced by a sidecar build
    #[serde(rename = "extra-artifact")]
    ExtraArtifact,
    /// An updater executable
    #[serde(rename = "updater")]
    Updater,
    /// A file that already exists
    #[serde(rename = "sbom")]
    SBOM,
    /// An OmniBOR Artifact ID
    #[serde(rename = "omnibor-artifact-id")]
    OmniborArtifactId,
    /// Unknown to this version of cargo-dist-schema
    ///
    /// This is a fallback for forward/backward-compat
    #[serde(other)]
    #[serde(rename = "unknown")]
    Unknown,
}

/// An executable artifact (exe/binary)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExecutableAsset {
    /// The name of the Artifact containing symbols for this executable
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub symbols_artifact: Option<ArtifactId>,
}

/// A C dynamic library artifact (so/dylib/dll)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DynamicLibraryAsset {
    /// The name of the Artifact containing symbols for this library
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub symbols_artifact: Option<ArtifactId>,
}

/// A C static library artifact (a/lib)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StaticLibraryAsset {
    /// The name of the Artifact containing symbols for this library
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub symbols_artifact: Option<ArtifactId>,
}

/// Info about a manifest version
pub struct VersionInfo {
    /// The version
    pub version: Version,
    /// The rough epoch of the format
    pub format: Format,
}

/// The current version of cargo-dist-schema
pub const SELF_VERSION: &str = env!("CARGO_PKG_VERSION");
/// The first epoch of cargo-dist, after this version a bunch of things changed
/// and we don't support that design anymore!
pub const DIST_EPOCH_1_MAX: &str = "0.0.3-prerelease8";
/// Second epoch of cargo-dist, after this we stopped putting versions in artifact ids.
/// This changes the download URL, but everything else works the same.
pub const DIST_EPOCH_2_MAX: &str = "0.0.6-prerelease6";

/// More coarse-grained version info, indicating periods when significant changes were made
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Format {
    /// THE BEFORE TIMES -- Unsupported
    Epoch1,
    /// First stable versions; during this epoch artifact names/ids contained their version numbers.
    Epoch2,
    /// Same as Epoch2, but now artifact names/ids don't include the version number,
    /// making /latest/ a stable path/url you can perma-link. This only affects download URLs.
    Epoch3,
    /// The version is newer than this version of cargo-dist-schema, so we don't know. Most
    /// likely it's compatible/readable, but maybe a breaking change was made?
    Future,
}

impl Format {
    /// Whether this format is too old to be supported
    pub fn unsupported(&self) -> bool {
        self <= &Format::Epoch1
    }
    /// Whether this format has version numbers in artifact names
    pub fn artifact_names_contain_versions(&self) -> bool {
        self <= &Format::Epoch2
    }
}

impl DistManifest {
    /// Create a new DistManifest
    pub fn new(releases: Vec<Release>, artifacts: BTreeMap<ArtifactId, Artifact>) -> Self {
        Self {
            dist_version: None,
            announcement_tag: None,
            announcement_tag_is_implicit: false,
            announcement_is_prerelease: false,
            announcement_title: None,
            announcement_changelog: None,
            announcement_github_body: None,
            github_attestations: false,
            github_attestations_filters: Default::default(),
            github_attestations_phase: Default::default(),
            system_info: None,
            releases,
            artifacts,
            systems: Default::default(),
            assets: Default::default(),
            publish_prereleases: false,
            force_latest: false,
            ci: None,
            linkage: vec![],
            upload_files: vec![],
        }
    }

    /// Get the JSON Schema for a DistManifest
    pub fn json_schema() -> schemars::schema::RootSchema {
        schemars::schema_for!(DistManifest)
    }

    /// Get the format of the manifest
    ///
    /// If anything goes wrong we'll default to Format::Future
    pub fn format(&self) -> Format {
        self.dist_version
            .as_ref()
            .and_then(|v| v.parse().ok())
            .map(|v| format_of_version(&v))
            .unwrap_or(Format::Future)
    }

    /// Convenience for iterating artifacts
    pub fn artifacts_for_release<'a>(
        &'a self,
        release: &'a Release,
    ) -> impl Iterator<Item = (&'a ArtifactIdRef, &'a Artifact)> {
        release
            .artifacts
            .iter()
            .filter_map(|k| Some((&**k, self.artifacts.get(k)?)))
    }

    /// Look up a release by its name
    pub fn release_by_name(&self, name: &str) -> Option<&Release> {
        self.releases.iter().find(|r| r.app_name == name)
    }

    /// Either get the release with the given name, or make a minimal one
    /// with no hosting/artifacts (to be populated)
    pub fn ensure_release(&mut self, name: String, version: String) -> &mut Release {
        // Written slightly awkwardly to make the borrowchecker happy :/
        if let Some(position) = self.releases.iter().position(|r| r.app_name == name) {
            &mut self.releases[position]
        } else {
            let env_app_name = name.to_ascii_uppercase().replace('-', "_");
            let install_dir_env_var = format!("{env_app_name}_INSTALL_DIR");
            let download_url_env_var = format!("{env_app_name}_DOWNLOAD_URL");
            let unmanaged_dir_env_var = format!("{env_app_name}_UNMANAGED_INSTALL");
            let disable_update_env_var = format!("{env_app_name}_DISABLE_UPDATE");
            let print_quiet_env_var = format!("{env_app_name}_PRINT_QUIET");
            let print_verbose_env_var = format!("{env_app_name}_PRINT_VERBOSE");
            let no_modify_path_env_var = format!("{env_app_name}_NO_MODIFY_PATH");
            let github_base_url_env_var = format!("{env_app_name}_INSTALLER_GITHUB_BASE_URL");
            let ghe_base_url_env_var = format!("{env_app_name}_INSTALLER_GHE_BASE_URL");
            let github_token_env_var = format!("{env_app_name}_GITHUB_TOKEN");

            let environment_variables = EnvironmentVariables {
                install_dir_env_var,
                download_url_env_var,
                unmanaged_dir_env_var,
                disable_update_env_var,
                print_quiet_env_var,
                print_verbose_env_var,
                no_modify_path_env_var,
                github_base_url_env_var,
                ghe_base_url_env_var,
                github_token_env_var,
            };

            self.releases.push(Release {
                app_name: name,
                app_version: version,
                env: Some(environment_variables),
                artifacts: vec![],
                hosting: Hosting::default(),
                display: None,
                display_name: None,
            });
            self.releases.last_mut().unwrap()
        }
    }

    /// Get the merged linkage for an artifact
    ///
    /// This lets you know what system dependencies an entire archive of binaries requires
    pub fn linkage_for_artifact(&self, artifact_id: &ArtifactId) -> Linkage {
        let mut output = Linkage::default();

        let Some(artifact) = self.artifacts.get(artifact_id) else {
            return output;
        };
        for base_asset in &artifact.assets {
            let Some(asset_id) = &base_asset.id else {
                continue;
            };
            let Some(true_asset) = self.assets.get(asset_id) else {
                continue;
            };
            let Some(linkage) = &true_asset.linkage else {
                continue;
            };
            output.extend(linkage);
        }

        output
    }
}

impl Release {
    /// Get the base URL that artifacts should be downloaded from (append the artifact name to the URL)
    pub fn artifact_download_url(&self) -> Option<String> {
        self.hosting.artifact_download_url()
    }
}

/// Possible hosting providers
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub struct Hosting {
    /// Hosted on Github Releases
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github: Option<GithubHosting>,
}

/// Github Hosting
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct GithubHosting {
    /// The URL of the host for GitHub, usually `"https://github.com"`
    /// (This can vary for GitHub Enterprise)
    pub artifact_base_url: String,
    /// The path of the release without the base URL
    ///
    /// e.g. `/myowner/myrepo/releases/download/v1.0.0/`
    pub artifact_download_path: String,
    /// The owner of the repo
    pub owner: String,
    /// The name of the repo
    pub repo: String,
}

impl Hosting {
    /// Get the base URL that artifacts should be downloaded from (append the artifact name to the URL)
    pub fn artifact_download_url(&self) -> Option<String> {
        let Hosting { github } = &self;
        if let Some(host) = &github {
            return Some(format!(
                "{}{}",
                host.artifact_base_url, host.artifact_download_path
            ));
        }
        None
    }
    /// Gets whether there's no hosting
    pub fn is_empty(&self) -> bool {
        let Hosting { github } = &self;
        github.is_none()
    }
}

/// Information about dynamic libraries used by a binary
#[derive(Clone, Default, Debug, Deserialize, Serialize, JsonSchema)]
pub struct Linkage {
    /// Libraries included with the operating system
    #[serde(default)]
    #[serde(skip_serializing_if = "SortedSet::is_empty")]
    pub system: SortedSet<Library>,
    /// Libraries provided by the Homebrew package manager
    #[serde(default)]
    #[serde(skip_serializing_if = "SortedSet::is_empty")]
    pub homebrew: SortedSet<Library>,
    /// Public libraries not provided by the system and not managed by any package manager
    #[serde(default)]
    #[serde(skip_serializing_if = "SortedSet::is_empty")]
    pub public_unmanaged: SortedSet<Library>,
    /// Libraries which don't fall into any other categories
    #[serde(default)]
    #[serde(skip_serializing_if = "SortedSet::is_empty")]
    pub other: SortedSet<Library>,
    /// Frameworks, only used on macOS
    #[serde(default)]
    #[serde(skip_serializing_if = "SortedSet::is_empty")]
    pub frameworks: SortedSet<Library>,
}

/// Represents the package manager a library was installed by
#[derive(
    Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord,
)]
#[serde(rename_all = "lowercase")]
pub enum PackageManager {
    /// Homebrew (usually for Mac)
    Homebrew,
    /// Apt (Debian, Ubuntu, etc)
    Apt,
}

declare_strongly_typed_string! {
    /// A homebrew package name, cf. <https://formulae.brew.sh/>
    pub struct HomebrewPackageName => &HomebrewPackageNameRef;

    /// An APT package name, cf. <https://en.wikipedia.org/wiki/APT_(software)>
    pub struct AptPackageName => &AptPackageNameRef;

    /// A chocolatey package name, cf. <https://community.chocolatey.org/packages>
    pub struct ChocolateyPackageName => &ChocolateyPackageNameRef;

    /// A pip package name
    pub struct PipPackageName => &PipPackageNameRef;

    /// A package version
    pub struct PackageVersion => &PackageVersionRef;
}

/// Represents a dynamic library located somewhere on the system
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
pub struct Library {
    /// The path to the library; on platforms without that information, it will be a basename instead
    pub path: String,
    /// The package from which a library comes, if relevant
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Which package manager provided this library
    pub package_manager: Option<PackageManager>,
    // FIXME: `HomebrewPackageName` and others are now strongly-typed, which makes having this
    // source/packagemanager thingy problematic. Maybe we could just have an enum, with Apt,
    // Homebrew, and Chocolatey variants? That would change the schema though.
}

impl Linkage {
    /// merge another linkage into this one
    pub fn extend(&mut self, val: &Linkage) {
        let Linkage {
            system,
            homebrew,
            public_unmanaged,
            other,
            frameworks,
        } = val;
        self.system.extend(system.iter().cloned());
        self.homebrew.extend(homebrew.iter().cloned());
        self.public_unmanaged
            .extend(public_unmanaged.iter().cloned());
        self.other.extend(other.iter().cloned());
        self.frameworks.extend(frameworks.iter().cloned());
    }
}

impl Library {
    /// Make a new Library with the given path and no source
    pub fn new(path: String) -> Self {
        Self {
            path,
            source: None,
            package_manager: None,
        }
    }

    /// Attempts to guess whether this specific library is glibc or not
    pub fn is_glibc(&self) -> bool {
        // If we were able to parse the source, we can be pretty precise
        if let Some(source) = &self.source {
            source == "libc6"
        } else {
            // Both patterns seen on Ubuntu (on the same system!)
            self.path.contains("libc.so.6") ||
            // This one will also contain the series version but
            // we don't want to be too precise here to avoid
            // filtering out later OS releases
            // Specifically we want to avoid `libc-musl` or `libc.musl`
            self.path.contains("libc-2")
        }
    }
}

impl std::fmt::Display for Library {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(package) = &self.source {
            write!(f, "{} ({package})", self.path)
        } else {
            write!(f, "{}", self.path)
        }
    }
}

/// Helper to read the raw version from serialized json
fn dist_version(input: &str) -> Option<Version> {
    #[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
    struct PartialDistManifest {
        /// The version of dist that generated this
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub dist_version: Option<String>,
    }

    let manifest: PartialDistManifest = serde_json::from_str(input).ok()?;
    let version: Version = manifest.dist_version?.parse().ok()?;
    Some(version)
}

/// Take serialized json and minimally parse out version info
pub fn check_version(input: &str) -> Option<VersionInfo> {
    let version = dist_version(input)?;
    let format = format_of_version(&version);
    Some(VersionInfo { version, format })
}

/// Get the format for a given version
pub fn format_of_version(version: &Version) -> Format {
    let epoch1 = Version::parse(DIST_EPOCH_1_MAX).unwrap();
    let epoch2 = Version::parse(DIST_EPOCH_2_MAX).unwrap();
    let self_ver = Version::parse(SELF_VERSION).unwrap();
    if version > &self_ver {
        Format::Future
    } else if version > &epoch2 {
        Format::Epoch3
    } else if version > &epoch1 {
        Format::Epoch2
    } else {
        Format::Epoch1
    }
}

#[test]
fn emit() {
    let schema = DistManifest::json_schema();
    let json_schema = serde_json::to_string_pretty(&schema).unwrap();
    insta::assert_snapshot!(json_schema);
}
