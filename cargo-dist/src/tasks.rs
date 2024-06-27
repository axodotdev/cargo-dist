//! Code to compute the tasks cargo-dist should do
//!
//! This is the heart and soul of cargo-dist, and ideally the [`gather_work`][] function
//! should compute every minute detail dist will perform ahead of time. This is done with
//! the DistGraphBuilder, which roughly builds up the work to do as follows:
//!
//! 1. [`config::get_project`][]: find out everything we want to know about the workspace (binaries, configs, etc)
//! 2. compute the TargetTriples we're interested based on ArtifactMode and target configs/flags
//! 3. add Releases for all the binaries selected by the above steps
//! 4. for each TargetTriple, create a ReleaseVariant of each Release
//! 5. add target-specific Binaries to each ReleaseVariant
//! 6. add Artifacts to each Release, which will be propagated to each ReleaseVariant as necessary
//!   1. add archives, propagated to ReleaseVariants
//!   2. add installers, each one decides if it's global or local
//! 7. compute actual BuildSteps from the current graph (a Binary will only induce an actual `cargo build`
//!    here if one of the Artifacts that was added requires outputs from it!)
//! 8. generate release/announcement notes
//!
//! During step 6 a lot of extra magic happens:
//!
//! * We drop artifacts on the ground if the current ArtifactMode disallows them
//! * We also try to automatically detect that a Binary That Needs To Be Built Now
//!   can produce symbols and make an Artifact for that too.
//!
//! In summary, the DistGraph has roughly the following hierarchy
//!
//! * Announcement: all the releases together
//!   * Releases: a specific version of an app (my-app-v1.0.0)
//!    * global Artifacts: artifacts that have only one version across all platforms
//!    * ReleaseVariants: a target-specific part of a Release (my-app-v1.0.0-x86_64-apple-darwin)
//!      * local Artifacts: artifacts that are per-Variant
//!      * Binaries: a binary that should be built for a specific Variant
//!   * BuildSteps: steps we should take to build the artifacts
//!
//! Note that much of this hierarchy is rearranged/simplified in dist-manifest.json!
//!
//! Binaries are a little bit weird in that they are in principle nested under ReleaseVariants
//! but can/should be shared between them when possible (e.g. if you have a crash reporter
//! binary that's shared across various apps). This is... not well-supported and things will
//! go a bit wonky if you actually try to do this right now. Notably what to parent a Symbols
//! Artifact to becomes ambiguous! Probably we should just be fine with duplicating things in
//! this case..?
//!
//! Also note that most of these things have (ideally, unchecked) globally unique "ids"
//! that are used to create ids for things nested under them, to ensure final
//! artifacts/folders/files always have unique names.
//!
//! Also note that the BuildSteps for installers are basically monolithic "build that installer"
//! steps to give them the freedom to do whatever they need to do.

use std::collections::{BTreeMap, HashMap};

use axoprocess::Cmd;
use axoproject::platforms::{
    TARGET_ARM64_LINUX_GNU, TARGET_ARM64_MAC, TARGET_X64_LINUX_GNU, TARGET_X64_MAC,
};
use axoproject::{PackageId, PackageIdx, WorkspaceGraph};
use camino::Utf8PathBuf;
use cargo_dist_schema::{ArtifactId, DistManifest, SystemId, SystemInfo};
use semver::Version;
use serde::Serialize;
use tracing::{info, warn};

use crate::announce::{self, AnnouncementTag, TagMode};
use crate::backend::ci::github::GithubCiInfo;
use crate::backend::ci::CiInfo;
use crate::config::{
    DependencyKind, DirtyMode, ExtraArtifact, GithubReleasePhase, ProductionMode,
    SystemDependencies,
};
use crate::platform::PlatformSupport;
use crate::sign::Signing;
use crate::{
    backend::{
        installer::{
            homebrew::{to_class_case, HomebrewInstallerInfo},
            msi::MsiInstallerInfo,
            npm::NpmInstallerInfo,
            InstallerImpl, InstallerInfo,
        },
        templates::Templates,
    },
    config::{
        self, ArtifactMode, ChecksumStyle, CiStyle, CompressionImpl, Config, DistMetadata,
        HostingStyle, InstallPathStrategy, InstallerStyle, PublishStyle, ZipStyle,
    },
    errors::{DistError, DistResult},
};

/// Key in workspace.metadata or package.metadata for our config
pub const METADATA_DIST: &str = "dist";
/// Dir in target/ for us to build our packages in
/// NOTE: DO NOT GIVE THIS THE SAME NAME AS A PROFILE!
pub const TARGET_DIST: &str = "distrib";
/// The profile we will build with
pub const PROFILE_DIST: &str = "dist";

/// The key for referring to linux as an "os"
pub const OS_LINUX: &str = "linux";
/// The key for referring to macos as an "os"
pub const OS_MACOS: &str = "macos";
/// The key for referring to windows as an "os"
pub const OS_WINDOWS: &str = "windows";

/// The key for referring to 64-bit x86_64 (AKA amd64) as an "cpu"
pub const CPU_X64: &str = "x86_64";
/// The key for referring to 32-bit x86 (AKA i686) as an "cpu"
pub const CPU_X86: &str = "x86";
/// The key for referring to 64-bit arm64 (AKA aarch64) as an "cpu"
pub const CPU_ARM64: &str = "arm64";
/// The key for referring to 32-bit arm as an "cpu"
pub const CPU_ARM: &str = "arm";

/// A rust target-triple (e.g. "x86_64-pc-windows-msvc")
pub type TargetTriple = String;
/// A map where the order doesn't matter
pub type FastMap<K, V> = std::collections::HashMap<K, V>;
/// A map where the order matters
pub type SortedMap<K, V> = std::collections::BTreeMap<K, V>;
/// A set where the order matters
pub type SortedSet<T> = std::collections::BTreeSet<T>;

/// A unique id for a [`Artifact`][]
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
pub struct ArtifactIdx(pub usize);

/// A unique id for a [`ReleaseVariant`][]
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
pub struct ReleaseVariantIdx(pub usize);

/// A unique id for a [`Release`][]
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
pub struct ReleaseIdx(pub usize);

/// A unique id for a [`Binary`][]
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
pub struct BinaryIdx(pub usize);

/// A convenience wrapper around a map of binary aliases
#[derive(Clone, Debug)]
pub struct BinaryAliases(BTreeMap<String, Vec<String>>);

impl BinaryAliases {
    /// Returns a formatted copy of the map, with file extensions added
    /// if necessary.
    pub fn for_target(&self, target: &str) -> BTreeMap<String, Vec<String>> {
        if target.contains("windows") {
            BTreeMap::from_iter(self.0.iter().map(|(k, v)| {
                (
                    format!("{k}.exe"),
                    v.iter().map(|name| format!("{name}.exe")).collect(),
                )
            }))
        } else {
            self.0.clone()
        }
    }

    /// Returns a map of binary aliases for each target triple, with
    /// executable extensions added if necessary.
    pub fn for_targets(
        &self,
        targets: &[String],
    ) -> BTreeMap<String, BTreeMap<String, Vec<String>>> {
        BTreeMap::from_iter(
            targets
                .iter()
                .map(|target| (target.to_owned(), self.for_target(target))),
        )
    }
}

/// The graph of all work that cargo-dist needs to do on this invocation.
///
/// All work is precomputed at the start of execution because only discovering
/// what you need to do in the middle of building/packing things is a mess.
/// It also allows us to report what *should* happen without actually doing it.
#[derive(Debug)]
pub struct DistGraph {
    /// Unique id for the system we're building on.
    ///
    /// Since the whole premise of cargo-dist is to invoke it once on each machine, and no
    /// two machines have any reason to have the exact same CLI args for cargo-dist, we
    /// just use a mangled form of the CLI arguments here.
    pub system_id: SystemId,
    /// Whether it looks like `cargo dist init` has been run
    pub is_init: bool,

    /// Info about the tools we're using to build
    pub tools: Tools,
    /// Signing tools
    pub signer: Signing,
    /// Minijinja templates we might want to render
    pub templates: Templates,

    /// The cargo target dir.
    pub target_dir: Utf8PathBuf,
    /// The root directory of the current cargo workspace.
    pub workspace_dir: Utf8PathBuf,
    /// cargo-dist's target dir (generally nested under `target_dir`).
    pub dist_dir: Utf8PathBuf,
    /// Whether to bother using --package instead of --workspace when building apps
    pub precise_builds: bool,
    /// Whether to try to merge otherwise-parallelizable tasks the same machine
    pub merge_tasks: bool,
    /// Whether failing tasks should make us give up on all other tasks
    pub fail_fast: bool,
    /// Whether CI should include auto-generated local artifacts tasks
    pub build_local_artifacts: bool,
    /// Whether releases should be triggered by explicit dispatch, instead of tags
    pub dispatch_releases: bool,
    /// Whether to create a github release or edit an existing draft
    pub create_release: bool,
    /// Trigger releases with pushes to this branch, instead of tags
    pub release_branch: Option<String>,
    /// \[unstable\] if Some, sign binaries with ssl.com
    pub ssldotcom_windows_sign: Option<ProductionMode>,
    /// Whether to enable GitHub Attestations
    pub github_attestations: bool,
    /// The desired cargo-dist version for handling this project
    pub desired_cargo_dist_version: Option<Version>,
    /// The desired rust toolchain for handling this project
    pub desired_rust_toolchain: Option<String>,
    /// Styles of CI we want to support
    pub ci_style: Vec<CiStyle>,
    /// Which actions to run on pull requests.
    ///
    /// "upload" will build and upload release artifacts, while "plan" will
    /// only plan out the release without running builds and "skip" will disable
    /// pull request runs entirely.
    pub pr_run_mode: cargo_dist_schema::PrRunMode,
    /// Generate targets to skip configuration up to date checks for
    pub allow_dirty: DirtyMode,
    /// Targets we need to build (local artifacts)
    pub local_build_steps: Vec<BuildStep>,
    /// Targets we need to build (global artifacts)
    pub global_build_steps: Vec<BuildStep>,
    /// Distributable artifacts we want to produce for the releases
    pub artifacts: Vec<Artifact>,
    /// Binaries we want to build
    pub binaries: Vec<Binary>,
    /// Variants of Releases
    pub variants: Vec<ReleaseVariant>,
    /// Logical releases that artifacts are grouped under
    pub releases: Vec<Release>,
    /// Info about CI backends
    pub ci: CiInfo,
    /// List of plan jobs to run
    pub plan_jobs: Vec<String>,
    /// List of local artifacts jobs to run
    pub local_artifacts_jobs: Vec<String>,
    /// List of global artifacts jobs to run
    pub global_artifacts_jobs: Vec<String>,
    /// List of host jobs to run
    pub host_jobs: Vec<String>,
    /// List of publish jobs to run
    pub publish_jobs: Vec<PublishStyle>,
    /// Extra user-specified publish jobs to run
    pub user_publish_jobs: Vec<String>,
    /// List of post-announce jobs to run
    pub post_announce_jobs: Vec<String>,
    /// A GitHub repo to publish the Homebrew formula to
    pub tap: Option<String>,
    /// Whether msvc targets should statically link the crt
    pub msvc_crt_static: bool,
    /// List of hosting providers to use
    pub hosting: Option<HostingInfo>,
    /// Additional artifacts to build and upload
    pub extra_artifacts: Vec<ExtraArtifact>,
    /// Custom GitHub runners, mapped by triple target
    pub github_custom_runners: HashMap<String, String>,
    /// LIES ALL LIES
    pub local_builds_are_lies: bool,
    /// Prefix git tags must include to be picked up (also renames release.yml)
    pub tag_namespace: Option<String>,
    /// Whether to install updaters alongside with binaries
    pub install_updater: bool,
    /// Publish GitHub Releases to this other repo
    pub github_releases_repo: Option<config::GithubRepoPair>,
    /// Read the commit to be tagged from the submodule at this path
    pub github_releases_submodule_path: Option<String>,
    /// Which phase to create a GitHub release at
    pub github_release: GithubReleasePhase,
}

/// Info about artifacts should be hosted
#[derive(Debug, Clone)]
pub struct HostingInfo {
    /// Hosting backends
    pub hosts: Vec<HostingStyle>,
    /// Repo url
    pub repo_url: String,
    /// Source hosting provider (e.g. "github")
    pub source_host: String,
    /// Project owner
    pub owner: String,
    /// Project name
    pub project: String,
}

/// Various tools we have found installed on the system
#[derive(Debug, Clone)]
pub struct Tools {
    /// Info on cargo, which must exist
    pub cargo: CargoInfo,
    /// rustup, useful for getting specific toolchains
    pub rustup: Option<Tool>,
    /// homebrew, only available on macOS
    pub brew: Option<Tool>,
    /// git, used if the repository is a git repo
    pub git: Option<Tool>,
    /// ssl.com's CodeSignTool, for Windows Code Signing
    ///
    /// <https://www.ssl.com/guide/esigner-codesigntool-command-guide/>
    pub code_sign_tool: Option<Tool>,
}

/// Info about the cargo toolchain we're using
#[derive(Debug, Clone)]
pub struct CargoInfo {
    /// The path/command used to refer to cargo (usually from the CARGO env var)
    pub cmd: String,
    /// The first line of running cargo with `-vV`, should be version info
    pub version_line: Option<String>,
    /// The host target triple (obtained from `-vV`)
    pub host_target: String,
}

/// A tool we have found installed on the system
#[derive(Debug, Clone, Default)]
pub struct Tool {
    /// The string to pass to Cmd::new
    pub cmd: String,
    /// The version the tool reported (in case useful)
    pub version: String,
}

/// A binary we want to build (specific to a Variant)
#[derive(Debug)]
pub struct Binary {
    /// A unique id to use for things derived from this binary
    ///
    /// (e.g. my-binary-v1.0.0-x86_64-pc-windows-msvc)
    pub id: String,
    /// The idx of the package this binary is defined by
    pub pkg_idx: PackageIdx,
    /// The cargo package this binary is defined by
    ///
    /// This is an "opaque" string that will show up in things like cargo machine-readable output,
    /// but **this is not the format that cargo -p flags expect**. Use pkg_spec for that.
    pub pkg_id: Option<PackageId>,
    /// An ideally unambiguous way to refer to a package for the purpose of cargo -p flags.
    pub pkg_spec: String,
    /// The name of the binary (as defined by the Cargo.toml)
    pub name: String,
    /// The filename the binary will have
    pub file_name: String,
    /// The target triple to build it for
    pub target: TargetTriple,
    /// The artifact for this Binary's symbols
    pub symbols_artifact: Option<ArtifactIdx>,
    /// Places the executable needs to be copied to
    ///
    /// If this is empty by the time we compute the precise build steps
    /// we will determine that this Binary doesn't actually need to be built.
    pub copy_exe_to: Vec<Utf8PathBuf>,
    /// Places the symbols need to be copied to
    pub copy_symbols_to: Vec<Utf8PathBuf>,
    /// feature flags!
    pub features: CargoTargetFeatures,
}

/// A build step we would like to perform
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum BuildStep {
    /// Do a generic build (and copy the outputs to various locations)
    Generic(GenericBuildStep),
    /// Do a cargo build (and copy the outputs to various locations)
    Cargo(CargoBuildStep),
    /// Do an extra artifact build (and copy the outputs to various locations)
    Extra(ExtraBuildStep),
    /// Run rustup to get a toolchain
    Rustup(RustupStep),
    /// Copy a file
    CopyFile(CopyStep),
    /// Copy a dir
    CopyDir(CopyStep),
    /// Copy a file or dir (unknown, don't check which until the last possible second)
    CopyFileOrDir(CopyStep),
    /// Zip up a directory
    Zip(ZipDirStep),
    /// Generate some kind of installer
    GenerateInstaller(InstallerImpl),
    /// Generates a source tarball
    GenerateSourceTarball(SourceTarballStep),
    /// Checksum a file
    Checksum(ChecksumImpl),
    /// Fetch or build an updater binary
    Updater(UpdaterStep),
    // FIXME: For macos universal builds we'll want
    // Lipo(LipoStep)
}

/// A cargo build (and copy the outputs to various locations)
#[derive(Debug)]
pub struct CargoBuildStep {
    /// The --target triple to pass
    pub target_triple: TargetTriple,
    /// The feature flags to pass
    pub features: CargoTargetFeatures,
    /// What package to build (or "the workspace")
    pub package: CargoTargetPackages,
    /// The --profile to pass
    pub profile: String,
    /// The value to set for RUSTFLAGS
    pub rustflags: String,
    /// Binaries we expect from this build
    pub expected_binaries: Vec<BinaryIdx>,
    /// The working directory to run the build in
    pub working_dir: Utf8PathBuf,
}

/// A cargo build (and copy the outputs to various locations)
#[derive(Debug)]
pub struct GenericBuildStep {
    /// The --target triple to pass
    pub target_triple: TargetTriple,
    /// Binaries we expect from this build
    pub expected_binaries: Vec<BinaryIdx>,
    /// The working directory to run the build in
    pub working_dir: Utf8PathBuf,
    /// The output directory to find build outputs in
    pub out_dir: Utf8PathBuf,
    /// The command to run to produce the expected binaries
    pub build_command: Vec<String>,
}

/// An "extra" build step, producing new sidecar artifacts
#[derive(Debug)]
pub struct ExtraBuildStep {
    /// The dir to run the build_command in
    pub working_dir: Utf8PathBuf,
    /// Relative paths (from the working_dir) to binaries we expect to find
    pub artifact_relpaths: Vec<Utf8PathBuf>,
    /// The command to run to produce the expected binaries
    pub build_command: Vec<String>,
}

/// A cargo build (and copy the outputs to various locations)
#[derive(Debug)]
pub struct RustupStep {
    /// The rustup to invoke (mostly here to prove you Have rustup)
    pub rustup: Tool,
    /// The target to install
    pub target: String,
}

/// zip/tarball some directory
#[derive(Debug)]
pub struct ZipDirStep {
    /// The directory to zip up
    pub src_path: Utf8PathBuf,
    /// The final file path for the output zip
    pub dest_path: Utf8PathBuf,
    /// The name of the dir the tarball/zip will contain
    pub with_root: Option<Utf8PathBuf>,
    /// The kind of zip/tarball to make
    pub zip_style: ZipStyle,
}

/// Copy a file
#[derive(Debug)]
pub struct CopyStep {
    /// from here
    pub src_path: Utf8PathBuf,
    /// to here
    pub dest_path: Utf8PathBuf,
}

/// Create a checksum
#[derive(Debug, Clone)]
pub struct ChecksumImpl {
    /// the checksumming algorithm
    pub checksum: ChecksumStyle,
    /// of this file
    pub src_path: Utf8PathBuf,
    /// potentially write it to here
    pub dest_path: Option<Utf8PathBuf>,
    /// record it for this artifact in the dist-manifest
    pub for_artifact: Option<ArtifactId>,
}

/// Create a source tarball
#[derive(Debug, Clone)]
pub struct SourceTarballStep {
    /// the ref/tag/commit/branch/etc. to archive
    pub committish: String,
    /// A root directory to nest the archive's contents under
    // Note: GitHub uses `appname-tag` for this
    pub prefix: String,
    /// target filename
    pub target: Utf8PathBuf,
    /// The dir to run the git command in
    pub working_dir: Utf8PathBuf,
}

/// Fetch or build an updater
#[derive(Debug, Clone)]
pub struct UpdaterStep {
    /// The target triple this updater is for
    pub target_triple: TargetTriple,
    /// The file this should produce
    pub target_filename: Utf8PathBuf,
}

/// A kind of symbols (debuginfo)
#[derive(Copy, Clone, Debug)]
pub enum SymbolKind {
    /// Microsoft pdbs
    Pdb,
    /// Apple dSYMs
    Dsym,
    /// DWARF DWPs
    Dwp,
}

impl SymbolKind {
    /// Get the file extension for the symbol kind
    pub fn ext(self) -> &'static str {
        match self {
            SymbolKind::Pdb => "pdb",
            SymbolKind::Dsym => "dSYM",
            SymbolKind::Dwp => "dwp",
        }
    }
}

/// A distributable artifact we want to build
#[derive(Clone, Debug)]
pub struct Artifact {
    /// Unique id for the Artifact (its file name)
    ///
    /// i.e. `cargo-dist-v0.1.0-x86_64-pc-windows-msvc.zip`
    pub id: String,
    /// The target platform
    ///
    /// i.e. `x86_64-pc-windows-msvc`
    pub target_triples: Vec<TargetTriple>,
    /// If constructing this artifact involves creating a directory,
    /// copying static files into it, and then zip/tarring it, set this
    /// value to automate those tasks.
    pub archive: Option<Archive>,
    /// The path where the final artifact will appear in the dist dir.
    ///
    /// i.e. `/.../target/dist/cargo-dist-v0.1.0-x86_64-pc-windows-msvc.zip`
    pub file_path: Utf8PathBuf,
    /// The built assets this artifact will contain
    ///
    /// i.e. `cargo-dist.exe`
    pub required_binaries: FastMap<BinaryIdx, Utf8PathBuf>,
    /// The kind of artifact this is
    pub kind: ArtifactKind,
    /// A checksum for this artifact, if any
    pub checksum: Option<ArtifactIdx>,
    /// Indicates whether the artifact is local or global
    pub is_global: bool,
}

/// Info about an archive (zip/tarball) that should be made. Currently this is always part
/// of an Artifact, and the final output will be [`Artifact::file_path`][].
#[derive(Clone, Debug)]
pub struct Archive {
    /// An optional prefix path to nest all the archive contents under
    /// If None then all the archive's contents will be placed in the root
    pub with_root: Option<Utf8PathBuf>,
    /// The path of the directory this artifact's contents will be stored in.
    ///
    /// i.e. `/.../target/dist/cargo-dist-v0.1.0-x86_64-pc-windows-msvc/`
    pub dir_path: Utf8PathBuf,
    /// The style of zip to make
    pub zip_style: ZipStyle,
    /// Static assets to copy to the root of the artifact's dir (path is src)
    ///
    /// In the future this might add a custom relative dest path
    pub static_assets: Vec<(StaticAssetKind, Utf8PathBuf)>,
}

/// A kind of artifact (more specific fields)
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ArtifactKind {
    /// An Archive containing binaries (aka ExecutableZip)
    ExecutableZip(ExecutableZip),
    /// Symbols
    Symbols(Symbols),
    /// An installer
    Installer(InstallerImpl),
    /// A checksum
    Checksum(ChecksumImpl),
    /// A source tarball
    SourceTarball(SourceTarball),
    /// An extra artifact specified via config
    ExtraArtifact(ExtraArtifactImpl),
    /// An updater executable
    Updater(UpdaterImpl),
}

/// An Archive containing binaries (aka ExecutableZip)
#[derive(Clone, Debug)]
pub struct ExecutableZip {
    // everything important is already part of Artifact
}

/// A Symbols/Debuginfo Artifact
#[derive(Clone, Debug)]
pub struct Symbols {
    /// The kind of symbols this is
    kind: SymbolKind,
}

/// A source tarball artifact
#[derive(Clone, Debug)]
pub struct SourceTarball {
    /// the ref/tag/commit/branch/etc. to archive
    pub committish: String,
    /// A root directory to nest the archive's contents under
    // Note: GitHub uses `appname-tag` for this
    pub prefix: String,
    /// target filename
    pub target: Utf8PathBuf,
    /// path to the git checkout
    pub working_dir: Utf8PathBuf,
}

/// An extra artifact of some kind
#[derive(Clone, Debug)]
pub struct ExtraArtifactImpl {
    /// Working dir to run the command in
    pub working_dir: Utf8PathBuf,
    /// The command to run to produce this artifact
    pub command: Vec<String>,
    /// Relative path to the artifact, from the working_dir
    pub artifact_relpath: Utf8PathBuf,
}

/// An updater executable
#[derive(Clone, Debug)]
pub struct UpdaterImpl {}

/// A logical release of an application that artifacts are grouped under
#[derive(Clone, Debug)]
pub struct Release {
    /// The name of the app
    pub app_name: String,
    /// A brief description of the app
    pub app_desc: Option<String>,
    /// The authors of the app
    pub app_authors: Vec<String>,
    /// The license of the app
    pub app_license: Option<String>,
    /// The URL to the app's source repository
    pub app_repository_url: Option<String>,
    /// The URL to the app's homepage
    pub app_homepage_url: Option<String>,
    /// A list of the app's keywords
    pub app_keywords: Option<Vec<String>>,
    /// The version of the app
    pub version: Version,
    /// The unique id of the release (e.g. "my-app-v1.0.0")
    pub id: String,
    /// Targets this Release has artifacts for
    pub targets: Vec<TargetTriple>,
    /// Binaries that every variant should ostensibly provide
    ///
    /// The string is the name of the binary under that package (without .exe extension)
    pub bins: Vec<(PackageIdx, String)>,
    /// Artifacts that are shared "globally" across all variants (shell-installer, metadata...)
    ///
    /// They might still be limited to some subset of the targets (e.g. powershell scripts are
    /// windows-only), but conceptually there's only "one" for the Release.
    pub global_artifacts: Vec<ArtifactIdx>,
    /// Variants of this Release (e.g. "the macos build") that can have "local" Artifacts.
    pub variants: Vec<ReleaseVariantIdx>,
    /// The body of the changelog for this release
    pub changelog_body: Option<String>,
    /// The title of the changelog for this release
    pub changelog_title: Option<String>,
    /// Archive format to use on windows
    pub windows_archive: ZipStyle,
    /// Archive format to use on non-windows
    pub unix_archive: ZipStyle,
    /// Style of checksum to produce
    pub checksum: ChecksumStyle,
    /// Customize the name of the npm package
    pub npm_package: Option<String>,
    /// The @scope to include in NPM packages
    pub npm_scope: Option<String>,
    /// Static assets that should be included in bundles like archives
    pub static_assets: Vec<(StaticAssetKind, Utf8PathBuf)>,
    /// Strategy for selecting paths to install to
    pub install_path: Vec<InstallPathStrategy>,
    /// Custom message to display on installer success
    pub install_success_msg: String,
    /// GitHub repository to push the Homebrew formula to, if built
    pub tap: Option<String>,
    /// Customize the name of the Homebrew formula
    pub formula: Option<String>,
    /// Packages to install from a system package manager
    pub system_dependencies: SystemDependencies,
    /// Computed support for platforms, gets iteratively refined over time, so check details
    /// as late as possible, if you can!
    pub platform_support: PlatformSupport,
    /// Aliases to publish binaries under, mapped source to target (ln style)
    pub bin_aliases: BinaryAliases,
    /// Whether to advertise the intallers/artifacts for this app in an announcement body
    pub display: Option<bool>,
    /// Custom name to use for the app in announcement bodies
    pub display_name: Option<String>,
}

/// A particular variant of a Release (e.g. "the macos build")
#[derive(Debug)]
pub struct ReleaseVariant {
    /// The target triple this variant is for
    pub target: TargetTriple,
    /// The unique identifying string used for things related to this variant
    /// (e.g. "my-app-v1.0.0-x86_64-pc-windows-msvc")
    pub id: String,
    /// Binaries included in this Release Variant
    pub binaries: Vec<BinaryIdx>,
    /// Static assets that should be included in bundles like archives
    pub static_assets: Vec<(StaticAssetKind, Utf8PathBuf)>,
    /// Artifacts that are "local" to this variant (binaries, symbols, msi-installer...)
    pub local_artifacts: Vec<ArtifactIdx>,
}

/// A particular kind of static asset we're interested in
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StaticAssetKind {
    /// A README file
    Readme,
    /// A LICENSE file
    License,
    /// A CHANGLEOG or RELEASES file
    Changelog,
    /// Some other miscellaneous file
    Other,
}

/// Cargo features a cargo build should use.
#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CargoTargetFeatures {
    /// Whether to disable default features
    pub default_features: bool,
    /// Features to enable
    pub features: CargoTargetFeatureList,
}

/// A list of features to build with
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CargoTargetFeatureList {
    /// All of them
    All,
    /// Some of them
    List(Vec<String>),
}

impl Default for CargoTargetFeatureList {
    fn default() -> Self {
        Self::List(vec![])
    }
}

/// Whether to build a package or workspace
#[derive(Debug)]
pub enum CargoTargetPackages {
    /// Build the workspace
    Workspace,
    /// Just build a package
    ///
    /// Inner string is [`Binary::pkg_spec`][]
    Package(String),
}

pub(crate) struct DistGraphBuilder<'pkg_graph> {
    pub(crate) inner: DistGraph,
    pub(crate) manifest: DistManifest,
    pub(crate) workspaces: &'pkg_graph mut WorkspaceGraph,
    artifact_mode: ArtifactMode,
    binaries_by_id: FastMap<String, BinaryIdx>,
    workspace_metadata: DistMetadata,
    package_metadata: Vec<DistMetadata>,
}

impl<'pkg_graph> DistGraphBuilder<'pkg_graph> {
    pub(crate) fn new(
        system_id: SystemId,
        tools: Tools,
        workspaces: &'pkg_graph mut WorkspaceGraph,
        artifact_mode: ArtifactMode,
        allow_all_dirty: bool,
        announcement_tag_is_implicit: bool,
    ) -> DistResult<Self> {
        let root_workspace_idx = workspaces.root_workspace_idx();
        let root_workspace = workspaces.workspace(root_workspace_idx);
        let target_dir = root_workspace.target_dir.clone();
        let workspace_dir = root_workspace.workspace_dir.clone();
        let dist_dir = target_dir.join(TARGET_DIST);

        let mut workspace_metadata =
            // Read the global config
            config::parse_metadata_table_or_manifest(
                root_workspace.kind,
                &root_workspace.manifest_path,
                root_workspace.cargo_metadata_table.as_ref(),
            )?;

        workspace_metadata.make_relative_to(&root_workspace.workspace_dir);

        // This is intentionally written awkwardly to make you update this
        //
        // This is the ideal place in the code to map/check global config once.
        // It's fine to just lower it to an identical field on DistGraph, but you might
        // want to e.g. `unwrap_or(false)` an `Option<bool>` here.
        let DistMetadata {
            cargo_dist_version,
            rust_toolchain_version,
            precise_builds,
            merge_tasks,
            fail_fast,
            build_local_artifacts,
            dispatch_releases,
            release_branch,
            ssldotcom_windows_sign,
            github_attestations,
            tag_namespace,
            install_updater,
            publish_prereleases,
            force_latest,
            features,
            default_features,
            all_features,
            create_release,
            github_releases_repo,
            github_releases_submodule_path,
            github_release,
            allow_dirty,
            msvc_crt_static,
            extra_artifacts,
            pr_run_mode,
            tap,
            // Partially Processed elsewhere
            //
            // FIXME?: this is the last vestige of us actually needing to keep workspace_metadata
            // after this function, seems like we should finish the job..? (Doing a big
            // refactor already, don't want to mess with this right now.)
            ci: _,
            hosting: _,
            // Only the final value merged into a package_config matters
            //
            // Note that we do *use* an auto-include from the workspace when doing
            // changelogs, but we don't consult this config, and just unconditionally use it.
            // That seems *fine*, but I wanted to note that here.
            auto_includes: _,
            // For the rest of these, only the final value merged into a package_config matters
            targets: _,
            dist: _,
            installers: _,
            formula: _,
            system_dependencies: _,
            windows_archive: _,
            unix_archive: _,
            include: _,
            npm_package: _,
            npm_scope: _,
            checksum: _,
            install_path: _,
            install_success_msg: _,
            plan_jobs: _,
            local_artifacts_jobs: _,
            global_artifacts_jobs: _,
            source_tarball: _,
            host_jobs: _,
            publish_jobs: _,
            post_announce_jobs: _,
            github_custom_runners: _,
            bin_aliases: _,
            display: _,
            display_name: _,
        } = &workspace_metadata;

        let desired_cargo_dist_version = cargo_dist_version.clone();
        let desired_rust_toolchain = rust_toolchain_version.clone();
        if desired_rust_toolchain.is_some() {
            warn!("rust-toolchain-version is deprecated, use rust-toolchain.toml if you want pinned toolchains");
        }

        let merge_tasks = merge_tasks.unwrap_or(false);
        let fail_fast = fail_fast.unwrap_or(false);
        let create_release = create_release.unwrap_or(true);
        let build_local_artifacts = build_local_artifacts.unwrap_or(true);
        let dispatch_releases = dispatch_releases.unwrap_or(false);
        let release_branch = release_branch.clone();
        let msvc_crt_static = msvc_crt_static.unwrap_or(true);
        let local_builds_are_lies = artifact_mode == ArtifactMode::Lies;
        let ssldotcom_windows_sign = ssldotcom_windows_sign.clone();
        let github_attestations = github_attestations.unwrap_or(false);
        let tag_namespace = tag_namespace.clone();
        let github_releases_repo = github_releases_repo.clone();
        let github_releases_submodule_path = github_releases_submodule_path.clone();
        let github_release = github_release.unwrap_or_default();

        let mut packages_with_mismatched_features = vec![];
        // Compute/merge package configs
        let mut package_metadata = vec![];
        for (_idx, package) in workspaces.all_packages() {
            let mut package_config = config::parse_metadata_table(
                &package.manifest_path,
                package.cargo_metadata_table.as_ref(),
            )?;
            package_config.make_relative_to(&package.package_root);
            package_config.merge_workspace_config(&workspace_metadata, &package.manifest_path);
            package_config.validate_install_paths()?;

            // Only do workspace builds if all the packages agree with the workspace feature settings
            if &package_config.features != features
                || &package_config.all_features != all_features
                || &package_config.default_features != default_features
            {
                packages_with_mismatched_features.push(package.name.clone());
            }

            package_metadata.push(package_config);
        }

        let requires_precise = !packages_with_mismatched_features.is_empty();
        let precise_builds = if let Some(precise_builds) = *precise_builds {
            if !precise_builds && requires_precise {
                return Err(DistError::PreciseImpossible {
                    packages: packages_with_mismatched_features,
                });
            }
            precise_builds
        } else {
            info!("force-enabling precise-builds to handle your build features");
            requires_precise
        };

        let plan_jobs = workspace_metadata
            .plan_jobs
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|s| {
                let string = s.to_string();
                if let Some(stripped) = string.strip_prefix("./") {
                    stripped.to_owned()
                } else {
                    string
                }
            })
            .collect();

        let local_artifacts_jobs = workspace_metadata
            .local_artifacts_jobs
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|s| {
                let string = s.to_string();
                if let Some(stripped) = string.strip_prefix("./") {
                    stripped.to_owned()
                } else {
                    string
                }
            })
            .collect();

        let global_artifacts_jobs = workspace_metadata
            .global_artifacts_jobs
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|s| {
                let string = s.to_string();
                if let Some(stripped) = string.strip_prefix("./") {
                    stripped.to_owned()
                } else {
                    string
                }
            })
            .collect();

        let host_jobs = workspace_metadata
            .host_jobs
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|s| {
                let string = s.to_string();
                if let Some(stripped) = string.strip_prefix("./") {
                    stripped.to_owned()
                } else {
                    string
                }
            })
            .collect();

        let post_announce_jobs = workspace_metadata
            .post_announce_jobs
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|s| {
                let string = s.to_string();
                if let Some(stripped) = string.strip_prefix("./") {
                    stripped.to_owned()
                } else {
                    string
                }
            })
            .collect();

        let templates = Templates::new()?;
        let publish_jobs: Vec<PublishStyle>;
        let user_publish_jobs: Vec<PublishStyle>;
        (publish_jobs, user_publish_jobs) = workspace_metadata
            .publish_jobs
            .clone()
            .unwrap_or(vec![])
            .into_iter()
            .partition(|s| !matches!(s, PublishStyle::User(_)));
        let user_publish_jobs = user_publish_jobs
            .into_iter()
            // Remove the ./ suffix for later; we only have user jobs at this
            // point so we no longer need to distinguish
            .map(|s| {
                let string = s.to_string();
                if let Some(stripped) = string.strip_prefix("./") {
                    stripped.to_owned()
                } else {
                    string
                }
            })
            .collect();
        let publish_prereleases = publish_prereleases.unwrap_or(false);
        let force_latest = force_latest.unwrap_or(false);

        let allow_dirty = if allow_all_dirty {
            DirtyMode::AllowAll
        } else {
            DirtyMode::AllowList(allow_dirty.clone().unwrap_or(vec![]))
        };
        let cargo_version_line = tools.cargo.version_line.clone();

        let system = SystemInfo {
            id: system_id.clone(),
            cargo_version_line,
        };
        let systems = SortedMap::from_iter([(system_id.clone(), system)]);

        let signer = Signing::new(
            &tools.cargo.host_target,
            &dist_dir,
            ssldotcom_windows_sign.clone(),
        )?;

        Ok(Self {
            inner: DistGraph {
                system_id,
                is_init: desired_cargo_dist_version.is_some(),
                target_dir,
                workspace_dir,
                dist_dir,
                precise_builds,
                fail_fast,
                merge_tasks,
                build_local_artifacts,
                dispatch_releases,
                release_branch,
                create_release,
                github_releases_repo,
                github_releases_submodule_path,
                github_release,
                ssldotcom_windows_sign,
                github_attestations,
                desired_cargo_dist_version,
                desired_rust_toolchain,
                tag_namespace,
                signer,
                tools,
                local_builds_are_lies,
                templates,
                ci_style: vec![],
                local_build_steps: vec![],
                global_build_steps: vec![],
                artifacts: vec![],
                binaries: vec![],
                variants: vec![],
                releases: vec![],
                ci: CiInfo::default(),
                pr_run_mode: pr_run_mode.unwrap_or_default(),
                tap: tap.clone(),
                plan_jobs,
                local_artifacts_jobs,
                global_artifacts_jobs,
                host_jobs,
                publish_jobs,
                user_publish_jobs,
                post_announce_jobs,
                allow_dirty,
                msvc_crt_static,
                hosting: None,
                extra_artifacts: extra_artifacts.clone().unwrap_or_default(),
                github_custom_runners: workspace_metadata
                    .github_custom_runners
                    .clone()
                    .unwrap_or_default(),
                install_updater: install_updater.unwrap_or_default(),
            },
            manifest: DistManifest {
                dist_version: Some(env!("CARGO_PKG_VERSION").to_owned()),
                system_info: None,
                announcement_tag: None,
                announcement_is_prerelease: false,
                announcement_tag_is_implicit,
                announcement_title: None,
                announcement_changelog: None,
                announcement_github_body: None,
                releases: vec![],
                artifacts: Default::default(),
                systems,
                assets: Default::default(),
                publish_prereleases,
                force_latest,
                ci: None,
                linkage: vec![],
                upload_files: vec![],
                github_attestations,
            },
            package_metadata,
            workspace_metadata,
            workspaces,
            binaries_by_id: FastMap::new(),
            artifact_mode,
        })
    }

    pub(crate) fn package_metadata(&self, idx: PackageIdx) -> &DistMetadata {
        &self.package_metadata[idx.0]
    }

    fn set_ci_style(&mut self, style: Vec<CiStyle>) {
        self.inner.ci_style = style;
    }

    fn add_release(&mut self, pkg_idx: PackageIdx) -> ReleaseIdx {
        let package_info = self.workspaces.package(pkg_idx);
        let DistMetadata {
            tap,
            formula,
            system_dependencies,
            include,
            auto_includes,
            windows_archive,
            unix_archive,
            npm_package,
            npm_scope,
            checksum,
            install_path,
            install_success_msg,
            bin_aliases,
            display,
            display_name,
            // The rest of these are workspace-only
            precise_builds: _,
            merge_tasks: _,
            fail_fast: _,
            build_local_artifacts: _,
            dispatch_releases: _,
            release_branch: _,
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
            github_releases_repo: _,
            github_releases_submodule_path: _,
            ssldotcom_windows_sign: _,
            hosting: _,
            extra_artifacts: _,
            github_custom_runners: _,
            tag_namespace: _,
            install_updater: _,
            cargo_dist_version: _,
            rust_toolchain_version: _,
            dist: _,
            ci: _,
            pr_run_mode: _,
            allow_dirty: _,
            installers: _,
            targets: _,
            msvc_crt_static: _,
            github_attestations: _,
            github_release: _,
        } = self.package_metadata(pkg_idx);

        let version = package_info.version.as_ref().unwrap().semver().clone();
        let app_name = package_info.name.clone();
        let app_desc = package_info.description.clone();
        let app_authors = package_info.authors.clone();
        let app_license = package_info.license.clone();
        let app_repository_url = package_info.repository_url.clone();
        let app_homepage_url = package_info.homepage_url.clone();
        let app_keywords = package_info.keywords.clone();
        let npm_package = npm_package.clone();
        let npm_scope = npm_scope.clone();
        let install_path = install_path
            .clone()
            .unwrap_or(vec![InstallPathStrategy::CargoHome]);
        let install_success_msg = install_success_msg
            .as_deref()
            .unwrap_or("everything's installed!")
            .to_owned();
        let tap = tap.clone();
        let formula = formula.clone();
        let display = *display;
        let display_name = display_name.clone();

        let windows_archive = windows_archive.unwrap_or(ZipStyle::Zip);
        let unix_archive = unix_archive.unwrap_or(ZipStyle::Tar(CompressionImpl::Xzip));
        let checksum = checksum.unwrap_or(ChecksumStyle::Sha256);

        // Add static assets
        let mut static_assets = vec![];
        let auto_includes_enabled = auto_includes.unwrap_or(true);
        if auto_includes_enabled {
            if let Some(readme) = &package_info.readme_file {
                static_assets.push((StaticAssetKind::Readme, readme.clone()));
            }
            if let Some(changelog) = &package_info.changelog_file {
                static_assets.push((StaticAssetKind::Changelog, changelog.clone()));
            }
            for license in &package_info.license_files {
                static_assets.push((StaticAssetKind::License, license.clone()));
            }
        }
        if let Some(include) = &include {
            for static_asset in include {
                static_assets.push((StaticAssetKind::Other, static_asset.clone()));
            }
        }

        let system_dependencies = system_dependencies.clone().unwrap_or_default();

        let bin_aliases = BinaryAliases(bin_aliases.clone().unwrap_or_default());

        let platform_support = PlatformSupport::default();
        let idx = ReleaseIdx(self.inner.releases.len());
        let id = app_name.clone();
        info!("added release {id}");
        self.inner.releases.push(Release {
            app_name,
            app_desc,
            app_authors,
            app_license,
            app_repository_url,
            app_homepage_url,
            app_keywords,
            version,
            id,
            global_artifacts: vec![],
            bins: vec![],
            targets: vec![],
            variants: vec![],
            changelog_body: None,
            changelog_title: None,
            windows_archive,
            unix_archive,
            static_assets,
            checksum,
            npm_package,
            npm_scope,
            install_path,
            install_success_msg,
            tap,
            formula,
            system_dependencies,
            platform_support,
            bin_aliases,
            display,
            display_name,
        });
        idx
    }

    fn add_variant(&mut self, to_release: ReleaseIdx, target: TargetTriple) -> ReleaseVariantIdx {
        let idx = ReleaseVariantIdx(self.inner.variants.len());
        let Release {
            id: release_id,
            variants,
            targets,
            static_assets,
            bins,
            ..
        } = self.release_mut(to_release);
        let static_assets = static_assets.clone();
        let variant_id = format!("{release_id}-{target}");
        info!("added variant {variant_id}");

        variants.push(idx);
        targets.push(target.clone());

        // Add all the binaries of the release to this variant
        let mut binaries = vec![];
        for (pkg_idx, binary_name) in bins.clone() {
            let package = self.workspaces.package(pkg_idx);
            let package_metadata = self.package_metadata(pkg_idx);
            let pkg_id = package.cargo_package_id.clone();
            // For now we just use the name of the package as its package_spec.
            // I'm not sure if there are situations where this is ambiguous when
            // referring to a package in your workspace that you want to build an app for.
            // If they do exist, that's deeply cursed and I want a user to tell me about it.
            let pkg_spec = package.name.clone();
            // FIXME: make this more of a GUID to allow variants to share binaries?
            let bin_id = format!("{variant_id}-{binary_name}");

            let idx = if let Some(&idx) = self.binaries_by_id.get(&bin_id) {
                // If we already are building this binary we don't need to do it again!
                idx
            } else {
                // Compute the rest of the details and add the binary
                let features = CargoTargetFeatures {
                    default_features: package_metadata.default_features.unwrap_or(true),
                    features: if let Some(true) = package_metadata.all_features {
                        CargoTargetFeatureList::All
                    } else {
                        CargoTargetFeatureList::List(
                            package_metadata.features.clone().unwrap_or_default(),
                        )
                    },
                };

                let target_is_windows = target.contains("windows");
                let platform_exe_ext = if target_is_windows { ".exe" } else { "" };

                let file_name = format!("{binary_name}{platform_exe_ext}");

                info!("added binary {bin_id}");
                let idx = BinaryIdx(self.inner.binaries.len());
                let binary = Binary {
                    id: bin_id.clone(),
                    pkg_id,
                    pkg_spec,
                    pkg_idx,
                    name: binary_name,
                    file_name,
                    target: target.clone(),
                    copy_exe_to: vec![],
                    copy_symbols_to: vec![],
                    symbols_artifact: None,
                    features,
                };
                self.inner.binaries.push(binary);
                self.binaries_by_id.insert(bin_id, idx);
                idx
            };

            binaries.push(idx);
        }

        self.inner.variants.push(ReleaseVariant {
            target,
            id: variant_id,
            local_artifacts: vec![],
            binaries,
            static_assets,
        });
        idx
    }

    fn add_binary(&mut self, to_release: ReleaseIdx, pkg_idx: PackageIdx, binary_name: String) {
        let release = self.release_mut(to_release);
        release.bins.push((pkg_idx, binary_name));
    }

    fn add_executable_zip(&mut self, to_release: ReleaseIdx) {
        if !self.local_artifacts_enabled() {
            return;
        }
        info!(
            "adding executable zip to release {}",
            self.release(to_release).id
        );

        // Create an archive for each Variant
        let release = self.release(to_release);
        let variants = release.variants.clone();
        let checksum = release.checksum;
        for variant_idx in variants {
            let (zip_artifact, built_assets) =
                self.make_executable_zip_for_variant(to_release, variant_idx);

            let zip_artifact_idx = self.add_local_artifact(variant_idx, zip_artifact);
            for (binary, dest_path) in built_assets {
                self.require_binary(zip_artifact_idx, variant_idx, binary, dest_path);
            }

            if checksum != ChecksumStyle::False {
                self.add_artifact_checksum(variant_idx, zip_artifact_idx, checksum);
            }
        }
    }

    fn add_extra_artifacts(&mut self, dist_metadata: &DistMetadata, to_release: ReleaseIdx) {
        if !self.global_artifacts_enabled() {
            return;
        }
        let dist_dir = &self.inner.dist_dir.to_owned();
        let artifacts = dist_metadata.extra_artifacts.to_owned().unwrap_or_default();

        for extra in artifacts {
            for artifact_relpath in extra.artifact_relpaths {
                let artifact_name = artifact_relpath
                    .file_name()
                    .expect("extra artifact had no name!?")
                    .to_owned();
                let target_path = dist_dir.join(&artifact_name);
                let artifact = Artifact {
                    id: artifact_name,
                    target_triples: vec![],
                    file_path: target_path.to_owned(),
                    required_binaries: FastMap::new(),
                    archive: None,
                    kind: ArtifactKind::ExtraArtifact(ExtraArtifactImpl {
                        working_dir: extra.working_dir.clone(),
                        command: extra.command.clone(),
                        artifact_relpath,
                    }),
                    checksum: None,
                    is_global: true,
                };

                self.add_global_artifact(to_release, artifact);
            }
        }
    }

    fn add_source_tarball(&mut self, _tag: &str, to_release: ReleaseIdx) {
        if !self.global_artifacts_enabled() {
            return;
        }

        if !self.workspace_metadata.source_tarball.unwrap_or(true) {
            return;
        }

        let git = if let Some(tool) = &self.inner.tools.git {
            tool.cmd.to_owned()
        } else {
            warn!("skipping source tarball; git not installed");
            return;
        };

        let working_dir = self.inner.workspace_dir.clone();

        // It's possible to run cargo-dist in something that's not a git
        // repo, including a brand-new repo that hasn't been `git init`ted yet;
        // we can't act on those.
        //
        // Note we don't need the output of --show-toplevel,
        // just the exit status.
        let status = Cmd::new(&git, "detect a git repo")
            .arg("rev-parse")
            .arg("--show-toplevel")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .check(false)
            .current_dir(&working_dir)
            .status();
        // We'll be stubbing the actual generation in this case
        let is_git_repo = if self.inner.local_builds_are_lies {
            true
        } else if let Ok(status) = status {
            status.success()
        } else {
            false
        };

        let status = Cmd::new(&git, "check for HEAD commit")
            .arg("rev-parse")
            .arg("HEAD")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .check(false)
            .current_dir(&working_dir)
            .status();
        let has_head = if self.inner.local_builds_are_lies {
            true
        } else if let Ok(status) = status {
            status.success()
        } else {
            false
        };

        if !is_git_repo {
            warn!(
                "skipping source tarball; no git repo found at {}",
                self.inner.workspace_dir
            );
            return;
        }

        if !has_head {
            warn!(
                "skipping source tarball; git repo at {} has no commits yet",
                self.inner.workspace_dir
            );
            return;
        }

        let release = self.release(to_release);
        let checksum = release.checksum;
        info!("adding source tarball to release {}", release.id);

        let dist_dir = &self.inner.dist_dir.to_owned();

        let filename = "source.tar.gz".to_owned();
        let target_path = dist_dir.join(&filename);
        let prefix = format!("{}-{}/", release.app_name, release.version);

        let artifact = Artifact {
            id: filename.to_owned(),
            target_triples: vec![],
            file_path: target_path.to_owned(),
            required_binaries: FastMap::new(),
            archive: None,
            kind: ArtifactKind::SourceTarball(SourceTarball {
                // FIXME: it would be nice to verify that HEAD == tag when it Really Must
                // (as in when cutting a real release), but to make everything work when testing
                // locally or in CI without a tag, we just always use HEAD (since releases will
                // checkout the tag anyway, so HEAD==tag should always be true when it matters).
                committish: "HEAD".to_owned(),
                prefix,
                target: target_path.to_owned(),
                working_dir,
            }),
            checksum: None,
            is_global: true,
        };

        let for_artifact = Some(artifact.id.clone());
        let artifact_idx = self.add_global_artifact(to_release, artifact);

        if checksum != ChecksumStyle::False {
            let checksum_id = format!("{filename}.{}", checksum.ext());
            let checksum_path = dist_dir.join(&checksum_id);
            let checksum = Artifact {
                id: checksum_id.to_owned(),
                target_triples: vec![],
                file_path: checksum_path.to_owned(),
                required_binaries: FastMap::new(),
                archive: None,
                kind: ArtifactKind::Checksum(ChecksumImpl {
                    checksum,
                    src_path: target_path,
                    dest_path: Some(checksum_path),
                    for_artifact,
                }),
                checksum: None,
                is_global: true,
            };

            let checksum_idx = self.add_global_artifact(to_release, checksum);
            self.artifact_mut(artifact_idx).checksum = Some(checksum_idx);
        }
    }

    fn add_artifact_checksum(
        &mut self,
        to_variant: ReleaseVariantIdx,
        artifact_idx: ArtifactIdx,
        checksum: ChecksumStyle,
    ) -> ArtifactIdx {
        let artifact = self.artifact(artifact_idx);
        let checksum_artifact = {
            let checksum_ext = checksum.ext();
            let checksum_id = format!("{}.{}", artifact.id, checksum_ext);
            let checksum_path = artifact.file_path.parent().unwrap().join(&checksum_id);
            Artifact {
                id: checksum_id,
                kind: ArtifactKind::Checksum(ChecksumImpl {
                    checksum,
                    src_path: artifact.file_path.clone(),
                    dest_path: Some(checksum_path.clone()),
                    for_artifact: Some(artifact.id.clone()),
                }),

                target_triples: artifact.target_triples.clone(),
                archive: None,
                file_path: checksum_path,
                required_binaries: Default::default(),
                // Who checksums the checksummers...
                checksum: None,
                is_global: false,
            }
        };
        let checksum_idx = self.add_local_artifact(to_variant, checksum_artifact);
        self.artifact_mut(artifact_idx).checksum = Some(checksum_idx);
        checksum_idx
    }

    fn add_updater(&mut self, variant_idx: ReleaseVariantIdx) {
        if !self.local_artifacts_enabled() {
            return;
        }

        let artifact = self.make_updater_for_variant(variant_idx);

        // This adds an updater per variant (eg one per app per target).
        // In the future this could possibly be deduplicated to just one per
        // target, but this is fine for now.
        self.add_local_artifact(variant_idx, artifact);
    }

    pub(crate) fn make_updater_for_variant(&self, variant_idx: ReleaseVariantIdx) -> Artifact {
        let variant = self.variant(variant_idx);
        let filename = format!("{}-update", variant.id);
        let target_path = &self.inner.dist_dir.to_owned().join(&filename);

        Artifact {
            id: filename.to_owned(),
            target_triples: vec![variant.target.to_owned()],
            file_path: target_path.to_owned(),
            required_binaries: FastMap::new(),
            archive: None,
            kind: ArtifactKind::Updater(UpdaterImpl {}),
            checksum: None,
            is_global: false,
        }
    }

    /// Make an executable zip for a variant, but don't yet integrate it into the graph
    ///
    /// This is useful for installers which want to know about *potential* executable zips
    pub(crate) fn make_executable_zip_for_variant(
        &self,
        release_idx: ReleaseIdx,
        variant_idx: ReleaseVariantIdx,
    ) -> (Artifact, Vec<(BinaryIdx, Utf8PathBuf)>) {
        // This is largely just a lot of path/name computation
        let dist_dir = &self.inner.dist_dir;
        let release = self.release(release_idx);
        let variant = self.variant(variant_idx);

        let target_is_windows = variant.target.contains("windows");
        let zip_style = if target_is_windows {
            release.windows_archive
        } else {
            release.unix_archive
        };

        let artifact_dir_name = variant.id.clone();
        let artifact_dir_path = dist_dir.join(&artifact_dir_name);
        let artifact_ext = zip_style.ext();
        let artifact_name = format!("{artifact_dir_name}{artifact_ext}");
        let artifact_path = dist_dir.join(&artifact_name);

        let static_assets = variant.static_assets.clone();
        let mut built_assets = Vec::new();
        for &binary_idx in &variant.binaries {
            let binary = self.binary(binary_idx);
            built_assets.push((binary_idx, artifact_dir_path.join(&binary.file_name)));
        }

        // When unpacking we currently rely on zips being flat, but --strip-prefix=1 tarballs.
        // This is kinda inconsistent, so maybe we should make both flat?
        // (It's hard to strip-prefix zips, so making them both have an extra dir is annoying)
        let with_root = if let ZipStyle::Zip = zip_style {
            None
        } else {
            Some(Utf8PathBuf::from(artifact_dir_name.clone()))
        };

        (
            Artifact {
                id: artifact_name,
                target_triples: vec![variant.target.clone()],
                file_path: artifact_path,
                required_binaries: FastMap::new(),
                archive: Some(Archive {
                    with_root,
                    dir_path: artifact_dir_path,
                    zip_style,
                    static_assets,
                }),
                kind: ArtifactKind::ExecutableZip(ExecutableZip {}),
                // May get filled in later
                checksum: None,
                is_global: false,
            },
            built_assets,
        )
    }

    /// Register that `for_artifact` requires `binary_idx` to actually be built for
    /// `for_variant`.
    ///
    /// `dest_path` is the file path to copy the binary to (used for Archives)
    /// as soon as they're built.
    ///
    /// Note that it's important to use `dest_path`, as cargo does not guarantee that
    /// multiple invocations will not overwrite each other's outputs. Since we always
    /// explicitly pass --target and --profile, this is unlikely to be an issue. But if
    /// we ever introduce the notion of "feature variants" (ReleaseVariants that differ
    /// only in the feature flags they take), this will become a problem.
    fn require_binary(
        &mut self,
        for_artifact: ArtifactIdx,
        for_variant: ReleaseVariantIdx,
        binary_idx: BinaryIdx,
        dest_path: Utf8PathBuf,
    ) {
        let dist_dir = self.inner.dist_dir.clone();
        let binary = self.binary_mut(binary_idx);

        // Tell the binary that it should copy the exe to the given path
        binary.copy_exe_to.push(dest_path.clone());

        // Try to make a symbols artifact for this binary now that we're building it
        if binary.symbols_artifact.is_none() {
            if let Some(symbol_kind) = target_symbol_kind(&binary.target) {
                // FIXME: For some formats these won't be the same but for now stubbed out

                // FIXME: rustc/cargo has so more complex logic to do platform-specifc name remapping
                // (see should_replace_hyphens in src/cargo/core/compiler/build_context/target_info.rs)

                // FIXME: feed info about the expected source symbol name down to build_cargo_target
                // to unhardcode the use of .pdb ...!

                // let src_symbol_ext = symbol_kind.ext();
                let dest_symbol_ext = symbol_kind.ext();
                // let base_name = &binary.name;
                let binary_id = &binary.id;
                // let src_symbol_name = format!("{base_name}.{src_symbol_ext}");
                let dest_symbol_name = format!("{binary_id}.{dest_symbol_ext}");
                let artifact_path = dist_dir.join(&dest_symbol_name);

                let artifact = Artifact {
                    id: dest_symbol_name,
                    target_triples: vec![binary.target.clone()],
                    archive: None,
                    file_path: artifact_path.clone(),
                    required_binaries: FastMap::new(),
                    kind: ArtifactKind::Symbols(Symbols { kind: symbol_kind }),
                    checksum: None,
                    is_global: false,
                };

                // FIXME: strictly speaking a binary could plausibly be shared between Releases,
                // and in such a situation the artifact should also be shared between the Variants.
                // However this kind of breaks the local-artifact concept, as we require a local
                // artifact to be strictly nested under one Variant.
                //
                // For now we pretend this isn't a thing.
                let sym_artifact = self.add_local_artifact(for_variant, artifact);

                // Record that we've made the symbols artifact for this binary
                let binary = self.binary_mut(binary_idx);
                binary.symbols_artifact = Some(sym_artifact);
                binary.copy_symbols_to.push(artifact_path);
            }
        }

        // Tell the original requesting artifact that it will get this binary at the given path
        self.artifact_mut(for_artifact)
            .required_binaries
            .insert(binary_idx, dest_path);
    }

    fn add_installer(
        &mut self,
        to_release: ReleaseIdx,
        installer: &InstallerStyle,
    ) -> DistResult<()> {
        match installer {
            InstallerStyle::Shell => self.add_shell_installer(to_release),
            InstallerStyle::Powershell => self.add_powershell_installer(to_release),
            InstallerStyle::Npm => self.add_npm_installer(to_release),
            InstallerStyle::Homebrew => self.add_homebrew_installer(to_release),
            InstallerStyle::Msi => self.add_msi_installer(to_release)?,
        }
        Ok(())
    }

    fn add_shell_installer(&mut self, to_release: ReleaseIdx) {
        if !self.global_artifacts_enabled() {
            return;
        }
        let release = self.release(to_release);
        let release_id = &release.id;
        let Some(download_url) = self
            .manifest
            .release_by_name(&release.app_name)
            .and_then(|r| r.artifact_download_url())
        else {
            warn!("skipping shell installer: couldn't compute a URL to download artifacts from");
            return;
        };
        let artifact_name = format!("{release_id}-installer.sh");
        let artifact_path = self.inner.dist_dir.join(&artifact_name);
        let installer_url = format!("{download_url}/{artifact_name}");
        let hint = format!("curl --proto '=https' --tlsv1.2 -LsSf {installer_url} | sh");
        let desc = "Install prebuilt binaries via shell script".to_owned();

        // Get the artifacts
        let artifacts = release
            .platform_support
            .fragments()
            .into_iter()
            .filter(|a| !a.target_triple.contains("windows-msvc"))
            .collect::<Vec<_>>();
        let target_triples = artifacts
            .iter()
            .map(|a| a.target_triple.clone())
            .collect::<Vec<_>>();

        if artifacts.is_empty() {
            warn!("skipping shell installer: not building any supported platforms (use --artifacts=global)");
            return;
        };
        let bin_aliases = release.bin_aliases.for_targets(&target_triples);
        let installer_artifact = Artifact {
            id: artifact_name,
            target_triples,
            archive: None,
            file_path: artifact_path.clone(),
            required_binaries: FastMap::new(),
            checksum: None,
            kind: ArtifactKind::Installer(InstallerImpl::Shell(InstallerInfo {
                dest_path: artifact_path,
                app_name: release.app_name.clone(),
                app_version: release.version.to_string(),
                install_paths: release
                    .install_path
                    .iter()
                    .map(|p| p.clone().into_jinja())
                    .collect(),
                install_success_msg: release.install_success_msg.to_owned(),
                base_url: download_url.to_owned(),
                artifacts,
                hint,
                desc,
                receipt: InstallReceipt::from_metadata(&self.inner, release),
                bin_aliases,
            })),
            is_global: true,
        };

        self.add_global_artifact(to_release, installer_artifact);
    }

    fn add_homebrew_installer(&mut self, to_release: ReleaseIdx) {
        if !self.global_artifacts_enabled() {
            return;
        }
        let release = self.release(to_release);
        let formula = if let Some(formula) = &release.formula {
            formula
        } else {
            &release.id
        };
        let Some(download_url) = self
            .manifest
            .release_by_name(&release.id)
            .and_then(|r| r.artifact_download_url())
        else {
            warn!("skipping Homebrew formula: couldn't compute a URL to download artifacts from");
            return;
        };

        let artifact_name = format!("{formula}.rb");
        let artifact_path = self.inner.dist_dir.join(&artifact_name);

        // If tap is specified, include that in the `brew install` message
        let install_target = if let Some(tap) = &self.inner.tap {
            // So that, for example, axodotdev/homebrew-tap becomes axodotdev/tap
            let tap = tap.replace("/homebrew-", "/");
            format!("{tap}/{formula}")
        } else {
            formula.clone()
        };

        let hint = format!("brew install {}", install_target);
        let desc = "Install prebuilt binaries via Homebrew".to_owned();

        let artifacts = release
            .platform_support
            .fragments()
            .into_iter()
            .filter(|a| !a.target_triple.contains("windows-msvc"))
            .collect::<Vec<_>>();
        let target_triples = artifacts
            .iter()
            .map(|a| a.target_triple.clone())
            .collect::<Vec<_>>();
        let x86_64_macos = artifacts
            .iter()
            .find(|a| a.target_triple == TARGET_X64_MAC)
            .cloned();
        let arm64_macos = artifacts
            .iter()
            .find(|a| a.target_triple == TARGET_ARM64_MAC)
            .cloned();
        let x86_64_linux = artifacts
            .iter()
            .find(|a| a.target_triple == TARGET_X64_LINUX_GNU)
            .cloned();
        let arm64_linux = artifacts
            .iter()
            .find(|a| a.target_triple == TARGET_ARM64_LINUX_GNU)
            .cloned();

        if artifacts.is_empty() {
            warn!("skipping Homebrew installer: not building any supported platforms (use --artifacts=global)");
            return;
        };

        let release = self.release(to_release);
        let app_name = release.app_name.clone();
        let app_desc = if release.app_desc.is_none() {
            warn!("The Homebrew publish job is enabled but no description was specified\n  consider adding `description = ` to package in Cargo.toml");
            Some(format!("The {} application", release.app_name))
        } else {
            release.app_desc.clone()
        };
        let app_license = release.app_license.clone();
        let app_homepage_url = if release.app_homepage_url.is_none() {
            warn!("The Homebrew publish job is enabled but no homepage was specified\n  consider adding `homepage = ` to package in Cargo.toml");
            release.app_repository_url.clone()
        } else {
            release.app_homepage_url.clone()
        };
        let tap = release.tap.clone();

        if tap.is_some() && !self.inner.publish_jobs.contains(&PublishStyle::Homebrew) {
            warn!("A Homebrew tap was specified but the Homebrew publish job is disabled\n  consider adding \"homebrew\" to publish-jobs in Cargo.toml");
        }
        if self.inner.publish_jobs.contains(&PublishStyle::Homebrew) && tap.is_none() {
            warn!("The Homebrew publish job is enabled but no tap was specified\n  consider setting the tap field in Cargo.toml");
        }

        let dependencies: Vec<String> = release
            .system_dependencies
            .homebrew
            .clone()
            .into_iter()
            .filter(|(_, package)| package.0.stage_wanted(&DependencyKind::Run))
            .map(|(name, _)| name)
            .collect();
        let bin_aliases = release.bin_aliases.for_targets(&target_triples);
        let installer_artifact = Artifact {
            id: artifact_name,
            target_triples,
            archive: None,
            file_path: artifact_path.clone(),
            required_binaries: FastMap::new(),
            checksum: None,
            kind: ArtifactKind::Installer(InstallerImpl::Homebrew(HomebrewInstallerInfo {
                x86_64_macos,
                x86_64_macos_sha256: None,
                arm64_macos,
                arm64_macos_sha256: None,
                x86_64_linux,
                x86_64_linux_sha256: None,
                arm64_linux,
                arm64_linux_sha256: None,
                name: app_name,
                formula_class: to_class_case(formula),
                desc: app_desc,
                license: app_license,
                homepage: app_homepage_url,
                tap,
                dependencies,
                inner: InstallerInfo {
                    dest_path: artifact_path,
                    app_name: release.app_name.clone(),
                    app_version: release.version.to_string(),
                    install_paths: release
                        .install_path
                        .iter()
                        .map(|p| p.clone().into_jinja())
                        .collect(),
                    install_success_msg: release.install_success_msg.to_owned(),
                    base_url: download_url.to_owned(),
                    artifacts,
                    hint,
                    desc,
                    receipt: None,
                    bin_aliases,
                },
            })),
            is_global: true,
        };

        self.add_global_artifact(to_release, installer_artifact);
    }

    fn add_powershell_installer(&mut self, to_release: ReleaseIdx) {
        if !self.global_artifacts_enabled() {
            return;
        }

        // Get the basic info about the installer
        let release = self.release(to_release);
        let release_id = &release.id;
        let Some(download_url) = self
            .manifest
            .release_by_name(&release.app_name)
            .and_then(|r| r.artifact_download_url())
        else {
            warn!(
                "skipping powershell installer: couldn't compute a URL to download artifacts from"
            );
            return;
        };
        let artifact_name = format!("{release_id}-installer.ps1");
        let artifact_path = self.inner.dist_dir.join(&artifact_name);
        let installer_url = format!("{download_url}/{artifact_name}");
        let hint = format!(r#"powershell -c "irm {installer_url} | iex""#);
        let desc = "Install prebuilt binaries via powershell script".to_owned();

        // Gather up the bundles the installer supports
        let artifacts = release
            .platform_support
            .fragments()
            .into_iter()
            .filter(|a| a.target_triple.contains("windows-msvc"))
            .collect::<Vec<_>>();
        let target_triples = artifacts
            .iter()
            .map(|a| a.target_triple.clone())
            .collect::<Vec<_>>();
        if artifacts.is_empty() {
            warn!("skipping powershell installer: not building any supported platforms (use --artifacts=global)");
            return;
        };
        let bin_aliases = release.bin_aliases.for_targets(&target_triples);
        let installer_artifact = Artifact {
            id: artifact_name,
            target_triples,
            file_path: artifact_path.clone(),
            required_binaries: FastMap::new(),
            archive: None,
            checksum: None,
            kind: ArtifactKind::Installer(InstallerImpl::Powershell(InstallerInfo {
                dest_path: artifact_path,
                app_name: release.app_name.clone(),
                app_version: release.version.to_string(),
                install_paths: release
                    .install_path
                    .iter()
                    .map(|p| p.clone().into_jinja())
                    .collect(),
                install_success_msg: release.install_success_msg.to_owned(),
                base_url: download_url.to_owned(),
                artifacts,
                hint,
                desc,
                receipt: InstallReceipt::from_metadata(&self.inner, release),
                bin_aliases,
            })),
            is_global: true,
        };

        self.add_global_artifact(to_release, installer_artifact);
    }

    fn add_npm_installer(&mut self, to_release: ReleaseIdx) {
        if !self.global_artifacts_enabled() {
            return;
        }
        let release = self.release(to_release);
        let release_id = &release.id;
        let Some(download_url) = self
            .manifest
            .release_by_name(&release.app_name)
            .and_then(|r| r.artifact_download_url())
        else {
            warn!("skipping npm installer: couldn't compute a URL to download artifacts from");
            return;
        };

        let app_name = if let Some(name) = &release.npm_package {
            name.clone()
        } else {
            release.app_name.clone()
        };
        let npm_package_name = if let Some(scope) = &release.npm_scope {
            format!("{scope}/{}", app_name)
        } else {
            app_name.clone()
        };
        let npm_package_version = release.version.to_string();
        let npm_package_desc = release.app_desc.clone();
        let npm_package_authors = release.app_authors.clone();
        let npm_package_license = release.app_license.clone();
        let npm_package_repository_url = release.app_repository_url.clone();
        let npm_package_homepage_url = release.app_homepage_url.clone();
        let npm_package_keywords = release.app_keywords.clone();

        let static_assets = release.static_assets.clone();
        let dir_name = format!("{release_id}-npm-package");
        let dir_path = self.inner.dist_dir.join(&dir_name);
        let zip_style = ZipStyle::Tar(CompressionImpl::Gzip);
        let zip_ext = zip_style.ext();
        let artifact_name = format!("{dir_name}{zip_ext}");
        let artifact_path = self.inner.dist_dir.join(&artifact_name);
        // let installer_url = format!("{download_url}/{artifact_name}");
        let hint = format!("npm install {npm_package_name}@{npm_package_version}");
        let desc = "Install prebuilt binaries into your npm project".to_owned();

        let artifacts = release.platform_support.fragments();
        let target_triples = artifacts
            .iter()
            .map(|a| a.target_triple.clone())
            .collect::<Vec<_>>();
        let has_sketchy_archives = artifacts
            .iter()
            .any(|a| a.zip_style != ZipStyle::Tar(CompressionImpl::Gzip));

        if has_sketchy_archives {
            warn!("the npm installer currently only knows how to unpack .tar.gz archives\n  consider setting windows-archive and unix-archive to .tar.gz in your config");
        }
        if artifacts.is_empty() {
            warn!("skipping npm installer: not building any supported platforms (use --artifacts=global)");
            return;
        };
        let bin_aliases = release.bin_aliases.for_targets(&target_triples);
        let installer_artifact = Artifact {
            id: artifact_name,
            target_triples,
            archive: Some(Archive {
                // npm specifically expects the dir inside the tarball to be called "package"
                with_root: Some("package".into()),
                dir_path: dir_path.clone(),
                zip_style,
                static_assets,
            }),
            file_path: artifact_path.clone(),
            required_binaries: FastMap::new(),
            checksum: None,
            kind: ArtifactKind::Installer(InstallerImpl::Npm(NpmInstallerInfo {
                npm_package_name,
                npm_package_version,
                npm_package_desc,
                npm_package_authors,
                npm_package_license,
                npm_package_repository_url,
                npm_package_homepage_url,
                npm_package_keywords,
                package_dir: dir_path,
                inner: InstallerInfo {
                    dest_path: artifact_path,
                    app_name,
                    app_version: release.version.to_string(),
                    install_paths: release
                        .install_path
                        .iter()
                        .map(|p| p.clone().into_jinja())
                        .collect(),
                    install_success_msg: release.install_success_msg.to_owned(),
                    base_url: download_url.to_owned(),
                    artifacts,
                    hint,
                    desc,
                    receipt: None,
                    bin_aliases,
                },
            })),
            is_global: true,
        };

        self.add_global_artifact(to_release, installer_artifact);
    }

    fn add_msi_installer(&mut self, to_release: ReleaseIdx) -> DistResult<()> {
        if !self.local_artifacts_enabled() {
            return Ok(());
        }

        // Clone info we need from the release to avoid borrowing across the loop
        let release = self.release(to_release);
        let variants = release.variants.clone();
        let checksum = release.checksum;

        // Make an msi for every windows platform
        for variant_idx in variants {
            let variant = self.variant(variant_idx);
            let binaries = variant.binaries.clone();
            let target = &variant.target;
            if !target.contains("windows") {
                continue;
            }

            let variant_id = &variant.id;
            let artifact_name = format!("{variant_id}.msi");
            let artifact_path = self.inner.dist_dir.join(&artifact_name);
            let dir_name = format!("{variant_id}_msi");
            let dir_path = self.inner.dist_dir.join(&dir_name);

            // Compute which package we're actually building, based on the binaries
            let mut package_info: Option<(String, PackageIdx)> = None;
            for &binary_idx in &binaries {
                let binary = self.binary(binary_idx);
                if let Some((existing_spec, _)) = &package_info {
                    // cargo-wix doesn't clearly support multi-package, so bail
                    if existing_spec != &binary.pkg_spec {
                        return Err(DistError::MultiPackageMsi {
                            artifact_name,
                            spec1: existing_spec.clone(),
                            spec2: binary.pkg_spec.clone(),
                        })?;
                    }
                } else {
                    package_info = Some((binary.pkg_spec.clone(), binary.pkg_idx));
                }
            }
            let Some((pkg_spec, pkg_idx)) = package_info else {
                return Err(DistError::NoPackageMsi { artifact_name })?;
            };
            let manifest_path = self.workspaces.package(pkg_idx).manifest_path.clone();
            let wxs_path = manifest_path
                .parent()
                .expect("Cargo.toml had no parent dir!?")
                .join("wix")
                .join("main.wxs");

            // Gather up the bundles the installer supports
            let installer_artifact = Artifact {
                id: artifact_name,
                target_triples: vec![target.clone()],
                file_path: artifact_path.clone(),
                required_binaries: FastMap::new(),
                archive: Some(Archive {
                    with_root: None,
                    dir_path: dir_path.clone(),
                    zip_style: ZipStyle::TempDir,
                    static_assets: vec![],
                }),
                checksum: None,
                kind: ArtifactKind::Installer(InstallerImpl::Msi(MsiInstallerInfo {
                    package_dir: dir_path.clone(),
                    pkg_spec,
                    target: target.clone(),
                    file_path: artifact_path.clone(),
                    wxs_path,
                    manifest_path,
                })),
                is_global: false,
            };

            // Register the artifact to various things
            let installer_idx = self.add_local_artifact(variant_idx, installer_artifact);
            for binary_idx in binaries {
                let binary = self.binary(binary_idx);
                self.require_binary(
                    installer_idx,
                    variant_idx,
                    binary_idx,
                    dir_path.join(&binary.file_name),
                );
            }
            if checksum != ChecksumStyle::False {
                self.add_artifact_checksum(variant_idx, installer_idx, checksum);
            }
        }

        Ok(())
    }

    fn add_local_artifact(
        &mut self,
        to_variant: ReleaseVariantIdx,
        artifact: Artifact,
    ) -> ArtifactIdx {
        assert!(self.local_artifacts_enabled());
        assert!(!artifact.is_global);

        let idx = ArtifactIdx(self.inner.artifacts.len());
        let ReleaseVariant {
            local_artifacts, ..
        } = self.variant_mut(to_variant);
        local_artifacts.push(idx);

        self.inner.artifacts.push(artifact);
        idx
    }

    fn add_global_artifact(&mut self, to_release: ReleaseIdx, artifact: Artifact) -> ArtifactIdx {
        assert!(self.global_artifacts_enabled());
        assert!(artifact.is_global);

        let idx = ArtifactIdx(self.inner.artifacts.len());
        let Release {
            global_artifacts, ..
        } = self.release_mut(to_release);
        global_artifacts.push(idx);

        self.inner.artifacts.push(artifact);
        idx
    }

    fn compute_build_steps(&mut self) {
        // FIXME: more intelligently schedule these in a proper graph?

        let mut local_build_steps = vec![];
        let mut global_build_steps = vec![];

        for workspace_idx in self.workspaces.all_workspace_indices() {
            let workspace_kind = self.workspaces.workspace(workspace_idx).kind;
            let builds = match workspace_kind {
                axoproject::WorkspaceKind::Javascript => unimplemented!("npm builds not supported"),
                axoproject::WorkspaceKind::Generic => self.compute_generic_builds(workspace_idx),
                axoproject::WorkspaceKind::Rust => self.compute_cargo_builds(workspace_idx),
            };
            local_build_steps.extend(builds);
        }
        global_build_steps.extend(self.compute_extra_builds());

        Self::add_build_steps_for_artifacts(
            &self
                .inner
                .artifacts
                .iter()
                .filter(|a| !a.is_global)
                .collect(),
            &mut local_build_steps,
        );
        Self::add_build_steps_for_artifacts(
            &self
                .inner
                .artifacts
                .iter()
                .filter(|a| a.is_global)
                .collect(),
            &mut global_build_steps,
        );

        self.inner.local_build_steps = local_build_steps;
        self.inner.global_build_steps = global_build_steps;
    }

    fn add_build_steps_for_artifacts(artifacts: &Vec<&Artifact>, build_steps: &mut Vec<BuildStep>) {
        for artifact in artifacts {
            match &artifact.kind {
                ArtifactKind::ExecutableZip(_zip) => {
                    // compute_cargo_builds and artifact.archive handle everything
                }
                ArtifactKind::Symbols(symbols) => {
                    match symbols.kind {
                        SymbolKind::Pdb => {
                            // No additional steps needed, the file is PERFECT (for now)
                        }
                        SymbolKind::Dsym => {
                            // FIXME: compress the dSYM in a .tar.xz, it's a actually a directory!
                        }
                        SymbolKind::Dwp => {
                            // No additional steps needed?
                        }
                    }
                }
                ArtifactKind::Installer(installer) => {
                    // Installer generation is complex enough that they just get monolithic impls
                    build_steps.push(BuildStep::GenerateInstaller(installer.clone()));
                }
                ArtifactKind::Checksum(checksum) => {
                    build_steps.push(BuildStep::Checksum(checksum.clone()));
                }
                ArtifactKind::SourceTarball(tarball) => {
                    build_steps.push(BuildStep::GenerateSourceTarball(SourceTarballStep {
                        committish: tarball.committish.to_owned(),
                        prefix: tarball.prefix.to_owned(),
                        target: tarball.target.to_owned(),
                        working_dir: tarball.working_dir.to_owned(),
                    }));
                }
                ArtifactKind::ExtraArtifact(_) => {
                    // compute_extra_builds handles this
                }
                ArtifactKind::Updater(_) => {
                    build_steps.push(BuildStep::Updater(UpdaterStep {
                        // There should only be one triple per artifact
                        target_triple: artifact.target_triples.first().unwrap().to_owned(),
                        target_filename: artifact.file_path.to_owned(),
                    }))
                }
            }

            if let Some(archive) = &artifact.archive {
                let artifact_dir = &archive.dir_path;
                // Copy all the static assets
                for (_, src_path) in &archive.static_assets {
                    let src_path = src_path.clone();
                    let file_name = src_path.file_name().unwrap();
                    let dest_path = artifact_dir.join(file_name);
                    // We want to let this path be created by build.rs, so we defer
                    // checking if it's a file or a dir until the last possible second
                    build_steps.push(BuildStep::CopyFileOrDir(CopyStep {
                        src_path,
                        dest_path,
                    }))
                }

                // Zip up the artifact
                build_steps.push(BuildStep::Zip(ZipDirStep {
                    src_path: artifact_dir.to_owned(),
                    dest_path: artifact.file_path.clone(),
                    with_root: archive.with_root.clone(),
                    zip_style: archive.zip_style,
                }));
                // and get its sha256 checksum into the metadata
                build_steps.push(BuildStep::Checksum(ChecksumImpl {
                    checksum: ChecksumStyle::Sha256,
                    src_path: artifact.file_path.clone(),
                    dest_path: None,
                    for_artifact: Some(artifact.id.clone()),
                }))
            }
        }
    }

    fn compute_releases(
        &mut self,
        cfg: &Config,
        announcing: &AnnouncementTag,
        triples: &[String],
        bypass_package_target_prefs: bool,
    ) -> DistResult<()> {
        // Create a Release for each package
        for (pkg_idx, binaries) in &announcing.rust_releases {
            // FIXME: this clone is hacky but I'm in the middle of a nasty refactor
            let package_config = self.package_metadata(*pkg_idx).clone();

            // Create a Release for this binary
            let release = self.add_release(*pkg_idx);

            // Don't bother with any of this without binaries
            // (releases a library, nothing to Build)
            if binaries.is_empty() {
                continue;
            }

            // Tell the Release to include these binaries
            for binary in binaries {
                self.add_binary(release, *pkg_idx, (*binary).clone());
            }

            // Create variants for this Release for each target
            for target in triples {
                // This logic ensures that (outside of host mode) we only select targets that are a
                // subset of the ones the package claims to support
                let use_target = bypass_package_target_prefs
                    || package_config
                        .targets
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .any(|t| t == target);
                if !use_target {
                    continue;
                }

                // Create the variant
                let variant = self.add_variant(release, target.clone());

                if self.inner.install_updater {
                    self.add_updater(variant);
                }
            }
            // Add executable zips to the Release
            self.add_executable_zip(release);

            // Get initial platform support for installers to use
            self.compute_platform_support(release);

            // Add the source tarball if appropriate
            self.add_source_tarball(&announcing.tag, release);

            // Add any extra artifacts defined in the config
            self.add_extra_artifacts(&package_config, release);

            // Add installers to the Release
            // Prefer the CLI's choices (`cfg`) if they're non-empty
            let installers = if cfg.installers.is_empty() {
                package_config.installers.as_deref().unwrap_or_default()
            } else {
                &cfg.installers[..]
            };

            for installer in installers {
                // This logic ensures that (outside of host mode) we only select installers that are a
                // subset of the ones the package claims to support
                let use_installer = package_config
                    .installers
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .any(|i| i == installer);
                if !use_installer {
                    continue;
                }

                // Create the variant
                self.add_installer(release, installer)?;
            }
        }

        // Translate the result to DistManifest
        crate::manifest::add_releases_to_manifest(cfg, &self.inner, &mut self.manifest)?;

        Ok(())
    }

    fn compute_ci(&mut self) -> DistResult<()> {
        for ci in &self.inner.ci_style {
            match ci {
                CiStyle::Github => {
                    self.inner.ci.github = Some(GithubCiInfo::new(&self.inner));
                }
            }
        }

        let external_repo_commit = self
            .inner
            .github_releases_submodule_path
            .as_ref()
            .map(|path| submodule_head(&self.inner.workspace_dir.join(path)))
            .transpose()?
            .flatten();

        // apply to manifest
        if !self.inner.ci_style.is_empty() {
            let CiInfo { github } = &self.inner.ci;
            let github = github.as_ref().map(|info| cargo_dist_schema::GithubCiInfo {
                artifacts_matrix: Some(info.artifacts_matrix.clone()),
                pr_run_mode: Some(info.pr_run_mode),
                external_repo_commit,
            });

            self.manifest.ci = Some(cargo_dist_schema::CiInfo { github });
        }

        Ok(())
    }

    fn compute_platform_support(&mut self, release: ReleaseIdx) {
        let support = PlatformSupport::new(self, release);
        self.release_mut(release).platform_support = support;
    }

    pub(crate) fn binary(&self, idx: BinaryIdx) -> &Binary {
        &self.inner.binaries[idx.0]
    }
    pub(crate) fn binary_mut(&mut self, idx: BinaryIdx) -> &mut Binary {
        &mut self.inner.binaries[idx.0]
    }
    pub(crate) fn artifact(&self, idx: ArtifactIdx) -> &Artifact {
        &self.inner.artifacts[idx.0]
    }
    pub(crate) fn artifact_mut(&mut self, idx: ArtifactIdx) -> &mut Artifact {
        &mut self.inner.artifacts[idx.0]
    }
    pub(crate) fn release(&self, idx: ReleaseIdx) -> &Release {
        &self.inner.releases[idx.0]
    }
    pub(crate) fn release_mut(&mut self, idx: ReleaseIdx) -> &mut Release {
        &mut self.inner.releases[idx.0]
    }
    pub(crate) fn variant(&self, idx: ReleaseVariantIdx) -> &ReleaseVariant {
        &self.inner.variants[idx.0]
    }
    pub(crate) fn variant_mut(&mut self, idx: ReleaseVariantIdx) -> &mut ReleaseVariant {
        &mut self.inner.variants[idx.0]
    }
    pub(crate) fn local_artifacts_enabled(&self) -> bool {
        match self.artifact_mode {
            ArtifactMode::Local => true,
            ArtifactMode::Global => false,
            ArtifactMode::Host => true,
            ArtifactMode::All => true,
            ArtifactMode::Lies => true,
        }
    }
    pub(crate) fn global_artifacts_enabled(&self) -> bool {
        match self.artifact_mode {
            ArtifactMode::Local => false,
            ArtifactMode::Global => true,
            ArtifactMode::Host => true,
            ArtifactMode::All => true,
            ArtifactMode::Lies => true,
        }
    }
}

impl DistGraph {
    /// Get a binary
    pub fn binary(&self, idx: BinaryIdx) -> &Binary {
        &self.binaries[idx.0]
    }
    /// Get a binary
    pub fn artifact(&self, idx: ArtifactIdx) -> &Artifact {
        &self.artifacts[idx.0]
    }
    /// Get a release
    pub fn release(&self, idx: ReleaseIdx) -> &Release {
        &self.releases[idx.0]
    }
    /// Get a variant
    pub fn variant(&self, idx: ReleaseVariantIdx) -> &ReleaseVariant {
        &self.variants[idx.0]
    }
}

/// Precompute all the work this invocation will need to do
pub fn gather_work(cfg: &Config) -> DistResult<(DistGraph, DistManifest)> {
    info!("analyzing workspace:");
    let tools = tool_info()?;
    let mut workspaces = crate::config::get_project()?;
    let system_id = format!(
        "{}:{}:{}",
        cfg.root_cmd,
        cfg.artifact_mode,
        cfg.targets.join(",")
    );
    let mut graph = DistGraphBuilder::new(
        system_id,
        tools,
        &mut workspaces,
        cfg.artifact_mode,
        cfg.allow_all_dirty,
        matches!(cfg.tag_settings.tag, TagMode::Infer),
    )?;

    // Prefer the CLI (cfg) if it's non-empty, but only select a subset
    // of what the workspace supports if it's non-empty
    let workspace_ci = graph.workspace_metadata.ci.clone().unwrap_or_default();
    if cfg.ci.is_empty() {
        graph.set_ci_style(workspace_ci);
    } else {
        let cfg_ci = SortedSet::from_iter(cfg.ci.clone());
        let workspace_ci = SortedSet::from_iter(workspace_ci);
        let shared_ci = cfg_ci.intersection(&workspace_ci).cloned().collect();
        graph.set_ci_style(shared_ci);
    }

    // If no targets were specified, just use the host target
    let host_target_triple = [graph.inner.tools.cargo.host_target.clone()];
    // If all targets specified, union together the targets our packages support
    // Note that this uses BTreeSet as an intermediate to make the order stable
    let all_target_triples = graph
        .workspaces
        .all_packages()
        .flat_map(|(id, _)| graph.package_metadata(id).targets.iter().flatten())
        .collect::<SortedSet<_>>()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();

    // Choose which set of target triples we're building for
    let mut bypass_package_target_prefs = false;
    let triples = if cfg.targets.is_empty() {
        if matches!(cfg.artifact_mode, ArtifactMode::Host) {
            info!("using host target-triple");
            // In "host" mode we want to build for the host arch regardless of what the
            // packages claim they support.
            //
            // FIXME: may cause sadness for "truly platform-specific" bins like a windows-only util
            // FIXME: it would be nice to do "easy" crosses like x64 mac => arm64 + universal2
            bypass_package_target_prefs = true;
            &host_target_triple
        } else if all_target_triples.is_empty() {
            return Err(DistError::CliMissingTargets {
                host_target: graph.inner.tools.cargo.host_target.clone(),
            });
        } else {
            info!("using all target-triples");
            // Otherwise assume the user wants all targets (desirable for --artifacts=global)
            &all_target_triples[..]
        }
    } else {
        info!("using explicit target-triples");
        // If the CLI has explicit targets, only use those!
        &cfg.targets[..]
    };
    info!("selected triples: {:?}", triples);

    // Figure out what packages we're announcing
    let announcing = announce::select_tag(&mut graph, &cfg.tag_settings)?;

    // Immediately check if there's other manifests kicking around that provide info
    // we don't want to recompute (lets us move towards more of an architecture where
    // `plan` figures out what to do and subsequent steps Simply Obey).
    crate::manifest::load_and_merge_manifests(
        &graph.inner.dist_dir,
        &mut graph.manifest,
        &announcing,
    )?;

    // Figure out how artifacts should be hosted
    graph.compute_hosting(
        cfg,
        &announcing,
        graph.workspace_metadata.hosting.clone(),
        graph.workspace_metadata.ci.clone(),
    )?;

    // Figure out what we're releasing/building
    graph.compute_releases(cfg, &announcing, triples, bypass_package_target_prefs)?;

    // Prep the announcement's release notes and whatnot
    graph.compute_announcement_info(&announcing);

    // Finally compute all the build steps!
    graph.compute_build_steps();

    // And now figure out how to orchestrate the result in CI
    graph.compute_ci()?;

    Ok((graph.inner, graph.manifest))
}

/// Get the path/command to invoke Cargo
pub fn cargo() -> DistResult<String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    Ok(cargo)
}

/// Get the host target triple from cargo
pub fn get_host_target(cargo: String) -> DistResult<CargoInfo> {
    let mut command = Cmd::new(&cargo, "get your Rust toolchain's version");
    command.arg("-vV");
    let output = command.output()?;
    let output = String::from_utf8(output.stdout).map_err(|_| DistError::FailedCargoVersion)?;
    let mut lines = output.lines();
    let version_line = lines.next().map(|s| s.to_owned());
    for line in lines {
        if let Some(target) = line.strip_prefix("host: ") {
            info!("host target is {target}");
            return Ok(CargoInfo {
                cmd: cargo,
                version_line,
                host_target: target.to_owned(),
            });
        }
    }
    Err(DistError::FailedCargoVersion)
}

fn target_symbol_kind(target: &str) -> Option<SymbolKind> {
    #[allow(clippy::if_same_then_else)]
    if target.contains("windows-msvc") {
        // Temporary disabled pending redesign of symbol handling!

        // Some(SymbolKind::Pdb)
        None
    } else if target.contains("apple") {
        // Macos dSYM files are real and work but things
        // freak out because it turns out they're directories
        // and not "real" files? Temporarily disabling this
        // until I have time to figure out what to do

        // Some(SymbolKind::Dsym)
        None
    } else {
        // Linux has DWPs but cargo doesn't properly uplift them
        // See: https://github.com/rust-lang/cargo/pull/11384

        // Some(SymbolKind::Dwp)
        None
    }
}

fn tool_info() -> DistResult<Tools> {
    let cargo_cmd = cargo()?;
    let cargo = get_host_target(cargo_cmd)?;
    Ok(Tools {
        cargo,
        rustup: find_tool("rustup", "-V"),
        brew: find_tool("brew", "--version"),
        git: find_tool("git", "--version"),
        // Computed later if needed
        code_sign_tool: None,
    })
}

fn find_tool(name: &str, test_flag: &str) -> Option<Tool> {
    let output = Cmd::new(name, "detect tool")
        .arg(test_flag)
        .check(false)
        .output()
        .ok()?;
    let string_output = String::from_utf8(output.stdout).ok()?;
    let version = string_output.lines().next()?;
    Some(Tool {
        cmd: name.to_owned(),
        version: version.to_owned(),
    })
}

/// Represents the source for the canonical form of this app's releases
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseSourceType {
    /// GitHub Releases
    GitHub,
    /// Axo releases
    Axo,
}

/// Where to look up releases for this app
#[derive(Clone, Debug, Serialize)]
pub struct ReleaseSource {
    /// Which type of remote resource to look up
    pub release_type: ReleaseSourceType,
    /// The owner, from the owner/name format
    pub owner: String,
    /// The name, from the owner/name format
    pub name: String,
    /// The app's name
    pub app_name: String,
}

/// The software which installed this receipt
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderSource {
    /// cargo-dist
    CargoDist,
}

/// Information about the source of this receipt
#[derive(Clone, Debug, Serialize)]
pub struct Provider {
    /// The software this receipt was installed via
    pub source: ProviderSource,
    /// The version of the above software
    pub version: String,
}

/// Struct representing an install receipt
#[derive(Clone, Debug, Serialize)]
pub struct InstallReceipt {
    /// The location on disk where this app was installed
    pub install_prefix: String,
    /// A list of all binaries installed by this app
    pub binaries: Vec<String>,
    /// Information about where to request information on new releases
    pub source: ReleaseSource,
    /// The version that was installed
    pub version: String,
    /// The software which installed this receipt
    pub provider: Provider,
    /// A list of aliases binaries were installed under
    pub binary_aliases: BTreeMap<String, Vec<String>>,
}

impl InstallReceipt {
    /// Produces an install receipt for the given DistGraph.
    pub fn from_metadata(manifest: &DistGraph, release: &Release) -> Option<InstallReceipt> {
        let hosting = if let Some(hosting) = &manifest.hosting {
            hosting
        } else {
            return None;
        };
        let source_type = if hosting.hosts.contains(&HostingStyle::Axodotdev) {
            ReleaseSourceType::Axo
        } else {
            ReleaseSourceType::GitHub
        };

        Some(InstallReceipt {
            // These first two are placeholder values which the installer will update
            install_prefix: "AXO_INSTALL_PREFIX".to_owned(),
            binaries: vec!["CARGO_DIST_BINS".to_owned()],
            version: release.version.to_string(),
            source: ReleaseSource {
                release_type: source_type,
                owner: hosting.owner.to_owned(),
                name: hosting.project.to_owned(),
                app_name: release.app_name.to_owned(),
            },
            provider: Provider {
                source: ProviderSource::CargoDist,
                version: env!("CARGO_PKG_VERSION").to_owned(),
            },
            binary_aliases: BTreeMap::default(),
        })
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
