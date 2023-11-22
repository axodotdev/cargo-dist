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

use std::process::Command;

use axoproject::{PackageId, PackageIdx, WorkspaceInfo};
use camino::Utf8PathBuf;
use cargo_dist_schema::DistManifest;
use miette::{miette, Context, IntoDiagnostic};
use semver::Version;
use tracing::{info, warn};

use crate::announce::{self, AnnouncementTag};
use crate::backend::ci::github::GithubCiInfo;
use crate::backend::ci::CiInfo;
use crate::config::{DependencyKind, DirtyMode, ProductionMode, SystemDependencies};
use crate::{
    backend::{
        installer::{
            homebrew::{to_class_case, HomebrewInstallerInfo},
            msi::MsiInstallerInfo,
            npm::NpmInstallerInfo,
            ExecutableZipFragment, InstallerImpl, InstallerInfo,
        },
        templates::Templates,
    },
    config::{
        self, ArtifactMode, ChecksumStyle, CiStyle, CompressionImpl, Config, DistMetadata,
        HostingStyle, InstallPathStrategy, InstallerStyle, PublishStyle, ZipStyle,
    },
    errors::{DistError, DistResult, Result},
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

/// The graph of all work that cargo-dist needs to do on this invocation.
///
/// All work is precomputed at the start of execution because only discovering
/// what you need to do in the middle of building/packing things is a mess.
/// It also allows us to report what *should* happen without actually doing it.
#[derive(Debug)]
pub struct DistGraph {
    /// Whether it looks like `cargo dist init` has been run
    pub is_init: bool,

    /// Info about the tools we're using to build
    pub tools: Tools,
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
    /// Whether to create a github release or edit an existing draft
    pub create_release: bool,
    /// \[unstable\] if Some, sign binaries with ssl.com
    pub ssldotcom_windows_sign: Option<ProductionMode>,
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
    /// List of publish jobs to run
    pub publish_jobs: Vec<PublishStyle>,
    /// Extra user-specified publish jobs to run
    pub user_publish_jobs: Vec<String>,
    /// A GitHub repo to publish the Homebrew formula to
    pub tap: Option<String>,
    /// Whether msvc targets should statically link the crt
    pub msvc_crt_static: bool,
    /// List of hosting providers to use
    pub hosting: Option<HostingInfo>,
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
    /// The string to pass to Command::new
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
    /// The package this binary is defined by
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
    pkg_idx: PackageIdx,
}

/// A build step we would like to perform
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum BuildStep {
    /// Do a generic build (and copy the outputs to various locations)
    Generic(GenericBuildStep),
    /// Do a cargo build (and copy the outputs to various locations)
    Cargo(CargoBuildStep),
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
    /// Checksum a file
    Checksum(ChecksumImpl),
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
}

/// A cargo build (and copy the outputs to various locations)
#[derive(Debug)]
pub struct GenericBuildStep {
    /// The --target triple to pass
    pub target_triple: TargetTriple,
    /// Binaries we expect from this build
    pub expected_binaries: Vec<BinaryIdx>,
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
    /// and write it to here
    pub dest_path: Utf8PathBuf,
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
    /// The @scope to include in NPM packages
    pub npm_scope: Option<String>,
    /// Static assets that should be included in bundles like archives
    pub static_assets: Vec<(StaticAssetKind, Utf8PathBuf)>,
    /// Strategy for selecting paths to install to
    pub install_path: InstallPathStrategy,
    /// GitHub repository to push the Homebrew formula to, if built
    pub tap: Option<String>,
    /// Packages to install from a system package manager
    pub system_dependencies: SystemDependencies,
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
    pub(crate) workspace: &'pkg_graph WorkspaceInfo,
    artifact_mode: ArtifactMode,
    binaries_by_id: FastMap<String, BinaryIdx>,
    workspace_metadata: DistMetadata,
    package_metadata: Vec<DistMetadata>,
}

impl<'pkg_graph> DistGraphBuilder<'pkg_graph> {
    pub(crate) fn new(
        tools: Tools,
        workspace: &'pkg_graph WorkspaceInfo,
        artifact_mode: ArtifactMode,
        allow_all_dirty: bool,
    ) -> DistResult<Self> {
        let target_dir = workspace.target_dir.clone();
        let workspace_dir = workspace.workspace_dir.clone();
        let dist_dir = target_dir.join(TARGET_DIST);

        let mut workspace_metadata =
            // Read the global config
            config::parse_metadata_table_or_manifest(
                workspace.kind,
                &workspace.manifest_path,
                workspace.cargo_metadata_table.as_ref(),
            )?;

        workspace_metadata.make_relative_to(&workspace.workspace_dir);

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
            ssldotcom_windows_sign,
            // Partially Processed elsewhere
            //
            // FIXME?: this is the last vestige of us actually needing to keep workspace_metadata
            // after this function, seems like we should finish the job..? (Doing a big
            // refactor already, don't want to mess with this right now.)
            ci,
            // Only the final value merged into a package_config matters
            //
            // Note that we do *use* an auto-include from the workspace when doing
            // changelogs, but we don't consult this config, and just unconditionally use it.
            // That seems *fine*, but I wanted to note that here.
            auto_includes: _,
            // Only the final value merged into a package_config matters
            targets: _,
            // Only the final value merged into a package_config matters
            dist: _,
            // Only the final value merged into a package_config matters
            installers: _,
            // Only the final value merged into a package_config matters
            tap: _,
            // Only the final value merged into a package_config matters
            system_dependencies: _,
            // Only the final value merged into a package_config matters
            windows_archive: _,
            // Only the final value merged into a package_config matters
            unix_archive: _,
            // Only the final value merged into a package_config matters
            include: _,
            // Only the final value merged into a package_config matters
            npm_scope: _,
            // Only the final value merged into a package_config matters
            checksum: _,
            // Only the final value merged into a package_config matters
            install_path: _,
            // Only the final value merged into a package_config matters
            publish_jobs: _,
            publish_prereleases,
            features,
            default_features,
            all_features,
            create_release,
            pr_run_mode: _,
            allow_dirty,
            msvc_crt_static,
            hosting,
        } = &workspace_metadata;

        let desired_cargo_dist_version = cargo_dist_version.clone();
        let desired_rust_toolchain = rust_toolchain_version.clone();
        if desired_rust_toolchain.is_some() {
            warn!("rust-toolchain-version is deprecated, use rust-toolchain.toml if you want pinned toolchains");
        }
        let merge_tasks = merge_tasks.unwrap_or(false);
        let fail_fast = fail_fast.unwrap_or(false);
        let create_release = create_release.unwrap_or(true);
        let msvc_crt_static = msvc_crt_static.unwrap_or(true);
        let ssldotcom_windows_sign = ssldotcom_windows_sign.clone();

        let mut packages_with_mismatched_features = vec![];
        // Compute/merge package configs
        let mut package_metadata = vec![];
        for package in &workspace.package_info {
            let mut package_config = config::parse_metadata_table(
                &package.manifest_path,
                package.cargo_metadata_table.as_ref(),
            )?;
            package_config.make_relative_to(&package.package_root);
            package_config.merge_workspace_config(&workspace_metadata, &package.manifest_path);

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

        let allow_dirty = if allow_all_dirty {
            DirtyMode::AllowAll
        } else {
            DirtyMode::AllowList(allow_dirty.clone().unwrap_or(vec![]))
        };
        let cargo_version_line = tools.cargo.version_line.clone();

        let hosting = crate::host::select_hosting(workspace, hosting.clone(), ci.as_deref());

        Ok(Self {
            inner: DistGraph {
                is_init: desired_cargo_dist_version.is_some(),
                target_dir,
                workspace_dir,
                dist_dir,
                precise_builds,
                fail_fast,
                merge_tasks,
                create_release,
                ssldotcom_windows_sign,
                desired_cargo_dist_version,
                desired_rust_toolchain,
                tools,
                templates,
                ci_style: vec![],
                local_build_steps: vec![],
                global_build_steps: vec![],
                artifacts: vec![],
                binaries: vec![],
                variants: vec![],
                releases: vec![],
                ci: CiInfo::default(),
                pr_run_mode: workspace_metadata.pr_run_mode.unwrap_or_default(),
                tap: workspace_metadata.tap.clone(),
                publish_jobs,
                user_publish_jobs,
                allow_dirty,
                msvc_crt_static,
                hosting,
            },
            manifest: DistManifest {
                dist_version: Some(env!("CARGO_PKG_VERSION").to_owned()),
                system_info: Some(cargo_dist_schema::SystemInfo { cargo_version_line }),
                announcement_tag: None,
                announcement_is_prerelease: false,
                announcement_title: None,
                announcement_changelog: None,
                announcement_github_body: None,
                releases: vec![],
                artifacts: Default::default(),
                publish_prereleases,
                ci: None,
                linkage: vec![],
            },
            package_metadata,
            workspace_metadata,
            workspace,
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
        let package_info = self.workspace().package(pkg_idx);
        let package_config = self.package_metadata(pkg_idx);

        let version = package_info.version.as_ref().unwrap().semver().clone();
        let app_name = package_info.name.clone();
        let app_desc = package_info.description.clone();
        let app_authors = package_info.authors.clone();
        let app_license = package_info.license.clone();
        let app_repository_url = package_info.repository_url.clone();
        let app_homepage_url = package_info.homepage_url.clone();
        let app_keywords = package_info.keywords.clone();
        let npm_scope = package_config.npm_scope.clone();
        let install_path = package_config
            .install_path
            .clone()
            .unwrap_or(InstallPathStrategy::CargoHome);
        let tap = package_config.tap.clone();

        let windows_archive = package_config.windows_archive.unwrap_or(ZipStyle::Zip);
        let unix_archive = package_config
            .unix_archive
            .unwrap_or(ZipStyle::Tar(CompressionImpl::Xzip));
        let checksum = package_config.checksum.unwrap_or(ChecksumStyle::Sha256);

        // Add static assets
        let mut static_assets = vec![];
        let auto_includes_enabled = package_config.auto_includes.unwrap_or(true);
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
        if let Some(include) = &package_config.include {
            for static_asset in include {
                static_assets.push((StaticAssetKind::Other, static_asset.clone()));
            }
        }

        let system_dependencies = package_config
            .system_dependencies
            .clone()
            .unwrap_or_default();

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
            npm_scope,
            install_path,
            tap,
            system_dependencies,
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
        let id = format!("{release_id}-{target}");
        info!("added variant {id}");

        variants.push(idx);
        targets.push(target.clone());

        // Add all the binaries of the release to this variant
        let mut binaries = vec![];
        for (pkg_idx, binary_name) in bins.clone() {
            let package = self.workspace.package(pkg_idx);
            let package_metadata = self.package_metadata(pkg_idx);
            let version = package
                .version
                .as_ref()
                .expect("Package version is mandatory!")
                .semver();
            let pkg_id = package.cargo_package_id.clone();
            // For now we just use the name of the package as its package_spec.
            // I'm not sure if there are situations where this is ambiguous when
            // referring to a package in your workspace that you want to build an app for.
            // If they do exist, that's deeply cursed and I want a user to tell me about it.
            let pkg_spec = package.name.clone();
            let id = format!("{binary_name}-v{version}-{target}");

            let idx = if let Some(&idx) = self.binaries_by_id.get(&id) {
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

                info!("added binary {id}");
                let idx = BinaryIdx(self.inner.binaries.len());
                let binary = Binary {
                    id,
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
                idx
            };

            binaries.push(idx);
        }

        self.inner.variants.push(ReleaseVariant {
            target,
            id,
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
                    dest_path: checksum_path.clone(),
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

    /// Make an executable zip for a variant, but don't yet integrate it into the graph
    ///
    /// This is useful for installers which want to know about *potential* executable zips
    fn make_executable_zip_for_variant(
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

        // If they have an x64 macos build but not an arm64 one, add a fallback entry
        // to try to install x64 on arm64 and let rosetta2 deal with it.
        //
        // (This isn't strictly correct because rosetta2 isn't installed by default
        // on macos, and the auto-installer only triggers for "real" apps, and not CLIs.
        // Still, we think this is better than not trying at all.)
        const X64_MACOS: &str = "x86_64-apple-darwin";
        const ARM64_MACOS: &str = "aarch64-apple-darwin";
        const X64_GNU: &str = "x86_64-unknown-linux-gnu";
        const X64_MUSL: &str = "x86_64-unknown-linux-musl";
        const X64_MUSL_STATIC: &str = "x86_64-unknown-linux-musl-static";
        const X64_MUSL_DYNAMIC: &str = "x86_64-unknown-linux-musl-dynamic";
        let mut has_x64_apple = false;
        let mut has_arm_apple = false;
        let mut has_gnu_linux = false;
        let mut has_static_musl_linux = false;
        // Currently always false, someday this build will exist
        let has_dynamic_musl_linux = false;
        for &variant_idx in &release.variants {
            let variant = self.variant(variant_idx);
            let target = &variant.target;
            if target == X64_MACOS {
                has_x64_apple = true;
            }
            if target == ARM64_MACOS {
                has_arm_apple = true;
            }
            if target == X64_GNU {
                has_gnu_linux = true;
            }
            if target == X64_MUSL {
                has_static_musl_linux = true;
            }
        }
        let do_rosetta_fallback = has_x64_apple && !has_arm_apple;
        let do_gnu_to_musl_fallback = !has_gnu_linux && has_static_musl_linux;
        let do_musl_to_musl_fallback = has_static_musl_linux && !has_dynamic_musl_linux;

        // Gather up the bundles the installer supports
        let mut artifacts = vec![];
        let mut target_triples = SortedSet::new();
        for &variant_idx in &release.variants {
            let variant = self.variant(variant_idx);
            let target = &variant.target;
            if target.contains("windows") {
                continue;
            }
            // Compute the artifact zip this variant *would* make *if* it were built
            // FIXME: this is a kind of hacky workaround for the fact that we don't have a good
            // way to add artifacts to the graph and then say "ok but don't build it".
            let (artifact, binaries) =
                self.make_executable_zip_for_variant(to_release, variant_idx);
            target_triples.insert(target.clone());
            let mut fragment = ExecutableZipFragment {
                id: artifact.id,
                target_triples: artifact.target_triples,
                zip_style: artifact.archive.as_ref().unwrap().zip_style,
                binaries: binaries
                    .into_iter()
                    .map(|(_, dest_path)| dest_path.file_name().unwrap().to_owned())
                    .collect(),
            };
            if do_rosetta_fallback && target == X64_MACOS {
                // Copy the info but respecify it to be arm64 macos
                let mut arm_fragment = fragment.clone();
                arm_fragment.target_triples = vec![ARM64_MACOS.to_owned()];
                artifacts.push(arm_fragment);
            }
            if target == X64_MUSL {
                // musl-static is actually kind of a fake triple we've invented
                // to let us specify which is which; we want to ensure it exists
                // for the installer to act on
                fragment.target_triples = vec![X64_MUSL_STATIC.to_owned()];
            }
            if do_gnu_to_musl_fallback && target == X64_MUSL {
                // Copy the info but lie that it's actually glibc
                let mut musl_fragment = fragment.clone();
                musl_fragment.target_triples = vec![X64_GNU.to_owned()];
                artifacts.push(musl_fragment);
            }
            if do_musl_to_musl_fallback && target == X64_MUSL {
                // Copy the info but lie that it's actually dynamic musl
                let mut musl_fragment = fragment.clone();
                musl_fragment.target_triples = vec![X64_MUSL_DYNAMIC.to_owned()];
                artifacts.push(musl_fragment);
            }

            artifacts.push(fragment);
        }

        if artifacts.is_empty() {
            warn!("skipping shell installer: not building any supported platforms (use --artifacts=global)");
            return;
        };

        let installer_artifact = Artifact {
            id: artifact_name,
            target_triples: target_triples.into_iter().collect(),
            archive: None,
            file_path: artifact_path.clone(),
            required_binaries: FastMap::new(),
            checksum: None,
            kind: ArtifactKind::Installer(InstallerImpl::Shell(InstallerInfo {
                dest_path: artifact_path,
                app_name: release.app_name.clone(),
                app_version: release.version.to_string(),
                install_path: release.install_path.clone().into_jinja(),
                base_url: download_url.to_owned(),
                artifacts,
                hint,
                desc,
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
        let release_id = &release.id;
        let Some(download_url) = self
            .manifest
            .release_by_name(&release.app_name)
            .and_then(|r| r.artifact_download_url())
        else {
            warn!("skipping Homebrew formula: couldn't compute a URL to download artifacts from");
            return;
        };

        let artifact_name = format!("{release_id}.rb");
        let artifact_path = self.inner.dist_dir.join(&artifact_name);

        // If tap is specified, include that in the `brew install` message
        let mut install_target = release.app_name.clone();
        if let Some(tap) = &self.inner.tap {
            install_target = format!("{tap}/{install_target}").to_owned();
        }

        let hint = format!("brew install {}", install_target);
        let desc = "Install prebuilt binaries via Homebrew".to_owned();

        // If they have an x64 macos build but not an arm64 one, add a fallback entry
        // to try to install x64 on arm64 and let rosetta2 deal with it.
        //
        // (This isn't strictly correct because rosetta2 isn't installed by default
        // on macos, and the auto-installer only triggers for "real" apps, and not CLIs.
        // Still, we think this is better than not trying at all.)
        const X64_MACOS: &str = "x86_64-apple-darwin";
        const ARM64_MACOS: &str = "aarch64-apple-darwin";
        let mut has_x64_apple = false;
        let mut has_arm_apple = false;
        for &variant_idx in &release.variants {
            let variant = self.variant(variant_idx);
            let target = &variant.target;
            if target == X64_MACOS {
                has_x64_apple = true;
            }
            if target == ARM64_MACOS {
                has_arm_apple = true;
            }
        }
        let do_rosetta_fallback = has_x64_apple && !has_arm_apple;

        let mut arm64 = None;
        let mut x86_64 = None;

        // Gather up the bundles the installer supports
        let mut artifacts = vec![];
        let mut target_triples = SortedSet::new();
        for &variant_idx in &release.variants {
            let variant = self.variant(variant_idx);
            let target = &variant.target;
            if target.contains("windows") || target.contains("linux-gnu") {
                continue;
            }
            // Compute the artifact zip this variant *would* make *if* it were built
            // FIXME: this is a kind of hacky workaround for the fact that we don't have a good
            // way to add artifacts to the graph and then say "ok but don't build it".
            let (artifact, binaries) =
                self.make_executable_zip_for_variant(to_release, variant_idx);
            target_triples.insert(target.clone());
            let fragment = ExecutableZipFragment {
                id: artifact.id,
                target_triples: artifact.target_triples,
                zip_style: artifact.archive.as_ref().unwrap().zip_style,
                binaries: binaries
                    .into_iter()
                    .map(|(_, dest_path)| dest_path.file_name().unwrap().to_owned())
                    .collect(),
            };

            if target == X64_MACOS {
                x86_64 = Some(fragment.clone());
            }
            if target == ARM64_MACOS {
                arm64 = Some(fragment.clone());
            }

            if do_rosetta_fallback && target == X64_MACOS {
                // Copy the info but respecify it to be arm64 macos
                let mut arm_fragment = fragment.clone();
                arm_fragment.target_triples = vec![ARM64_MACOS.to_owned()];
                artifacts.push(arm_fragment.clone());
                arm64 = Some(arm_fragment);
            }
            artifacts.push(fragment);
        }
        if artifacts.is_empty() {
            warn!("skipping Homebrew installer: not building any supported platforms (use --artifacts=global)");
            return;
        };

        let release = self.release(to_release);
        let app_name = release.app_name.clone();
        let app_desc = release.app_desc.clone();
        let app_license = release.app_license.clone();
        let app_homepage_url = release.app_homepage_url.clone();
        let tap = release.tap.clone();

        if tap.is_some() && !self.inner.publish_jobs.contains(&PublishStyle::Homebrew) {
            warn!("A Homebrew tap was specified but the Homebrew publish job is disabled\n  consider adding \"homebrew\" to publish-jobs in Cargo.toml");
        }
        if self.inner.publish_jobs.contains(&PublishStyle::Homebrew) && tap.is_none() {
            warn!("The Homebrew publish job is enabled but no tap was specified\n  consider setting the tap field in Cargo.toml");
        }

        let formula_name = to_class_case(&app_name);

        let dependencies: Vec<String> = release
            .system_dependencies
            .homebrew
            .clone()
            .into_iter()
            .filter(|(_, package)| package.0.stage_wanted(&DependencyKind::Run))
            .map(|(name, _)| name)
            .collect();

        let installer_artifact = Artifact {
            id: artifact_name,
            target_triples: target_triples.into_iter().collect(),
            archive: None,
            file_path: artifact_path.clone(),
            required_binaries: FastMap::new(),
            checksum: None,
            kind: ArtifactKind::Installer(InstallerImpl::Homebrew(HomebrewInstallerInfo {
                arm64,
                arm64_sha256: None,
                x86_64,
                x86_64_sha256: None,
                name: app_name,
                formula_class: formula_name,
                desc: app_desc,
                license: app_license,
                homepage: app_homepage_url,
                tap,
                dependencies,
                inner: InstallerInfo {
                    dest_path: artifact_path,
                    app_name: release.app_name.clone(),
                    app_version: release.version.to_string(),
                    install_path: release.install_path.clone().into_jinja(),
                    base_url: download_url.to_owned(),
                    artifacts,
                    hint,
                    desc,
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
        let hint = format!("irm {installer_url} | iex");
        let desc = "Install prebuilt binaries via powershell script".to_owned();

        // Gather up the bundles the installer supports
        let mut artifacts = vec![];
        let mut target_triples = SortedSet::new();
        for &variant_idx in &release.variants {
            let variant = self.variant(variant_idx);
            let target = &variant.target;
            if !target.contains("windows") {
                continue;
            }
            // Compute the artifact zip this variant *would* make *if* it were built
            // FIXME: this is a kind of hacky workaround for the fact that we don't have a good
            // way to add artifacts to the graph and then say "ok but don't build it".
            let (artifact, binaries) =
                self.make_executable_zip_for_variant(to_release, variant_idx);
            target_triples.insert(target.clone());
            artifacts.push(ExecutableZipFragment {
                id: artifact.id,
                target_triples: artifact.target_triples,
                zip_style: artifact.archive.as_ref().unwrap().zip_style,
                binaries: binaries
                    .into_iter()
                    .map(|(_, dest_path)| dest_path.file_name().unwrap().to_owned())
                    .collect(),
            });
        }
        if artifacts.is_empty() {
            warn!("skipping powershell installer: not building any supported platforms (use --artifacts=global)");
            return;
        };

        let installer_artifact = Artifact {
            id: artifact_name,
            target_triples: target_triples.into_iter().collect(),
            file_path: artifact_path.clone(),
            required_binaries: FastMap::new(),
            archive: None,
            checksum: None,
            kind: ArtifactKind::Installer(InstallerImpl::Powershell(InstallerInfo {
                dest_path: artifact_path,
                app_name: release.app_name.clone(),
                app_version: release.version.to_string(),
                install_path: release.install_path.clone().into_jinja(),
                base_url: download_url.to_owned(),
                artifacts,
                hint,
                desc,
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

        if release.bins.len() > 1 {
            warn!("skipping npm installer: packages with multiple binaries are unsupported\n  let us know if you have a use for this, and what should happen!");
            return;
        }
        let bin = release.bins[0].1.clone();

        let npm_package_name = if let Some(scope) = &release.npm_scope {
            format!("{scope}/{}", release.app_name)
        } else {
            release.app_name.clone()
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

        // See comments above
        const X64_MACOS: &str = "x86_64-apple-darwin";
        const ARM64_MACOS: &str = "aarch64-apple-darwin";
        const X64_GNU: &str = "x86_64-unknown-linux-gnu";
        const X64_MUSL: &str = "x86_64-unknown-linux-musl";
        const X64_MUSL_STATIC: &str = "x86_64-unknown-linux-musl-static";
        const X64_MUSL_DYNAMIC: &str = "x86_64-unknown-linux-musl-dynamic";
        let mut has_x64_apple = false;
        let mut has_arm_apple = false;
        let mut has_gnu_linux = false;
        let mut has_static_musl_linux = false;
        // Currently always false, someday this build will exist
        let has_dynamic_musl_linux = false;
        for &variant_idx in &release.variants {
            let variant = self.variant(variant_idx);
            let target = &variant.target;
            if target == X64_MACOS {
                has_x64_apple = true;
            }
            if target == ARM64_MACOS {
                has_arm_apple = true;
            }
            if target == X64_GNU {
                has_gnu_linux = true;
            }
            if target == X64_MUSL {
                has_static_musl_linux = true;
            }
        }
        let do_rosetta_fallback = has_x64_apple && !has_arm_apple;
        let do_gnu_to_musl_fallback = !has_gnu_linux && has_static_musl_linux;
        let do_musl_to_musl_fallback = has_static_musl_linux && !has_dynamic_musl_linux;

        // Gather up the bundles the installer supports
        let mut artifacts = vec![];
        let mut target_triples = SortedSet::new();
        let mut has_sketchy_archives = false;
        for &variant_idx in &release.variants {
            let variant = self.variant(variant_idx);
            let target = &variant.target;
            // Compute the artifact zip this variant *would* make *if* it were built
            // FIXME: this is a kind of hacky workaround for the fact that we don't have a good
            // way to add artifacts to the graph and then say "ok but don't build it".
            let (artifact, binaries) =
                self.make_executable_zip_for_variant(to_release, variant_idx);
            target_triples.insert(target.clone());

            let variant_zip_style = artifact.archive.as_ref().unwrap().zip_style;
            if variant_zip_style != ZipStyle::Tar(CompressionImpl::Gzip) {
                has_sketchy_archives = true;
            }
            let mut fragment = ExecutableZipFragment {
                id: artifact.id,
                target_triples: artifact.target_triples,
                zip_style: variant_zip_style,
                binaries: binaries
                    .into_iter()
                    .map(|(_, dest_path)| dest_path.file_name().unwrap().to_owned())
                    .collect(),
            };
            if do_rosetta_fallback && target == X64_MACOS {
                // Copy the info but respecify it to be arm64 macos
                let mut arm_fragment = fragment.clone();
                arm_fragment.target_triples = vec![ARM64_MACOS.to_owned()];
                artifacts.push(arm_fragment);
            }
            if target == X64_MUSL {
                // musl-static is actually kind of a fake triple we've invented
                // to let us specify which is which; we want to ensure it exists
                // for the installer to act on
                fragment.target_triples = vec![X64_MUSL_STATIC.to_owned()];
            }
            if do_gnu_to_musl_fallback && target == X64_MUSL {
                // Copy the info but lie that it's actually glibc
                let mut musl_fragment = fragment.clone();
                musl_fragment.target_triples = vec![X64_GNU.to_owned()];
                artifacts.push(musl_fragment);
            }
            if do_musl_to_musl_fallback && target == X64_MUSL {
                // Copy the info but lie that it's actually dynamic musl
                let mut musl_fragment = fragment.clone();
                musl_fragment.target_triples = vec![X64_MUSL_DYNAMIC.to_owned()];
                artifacts.push(musl_fragment);
            }

            artifacts.push(fragment);
        }

        if has_sketchy_archives {
            warn!("the npm installer currently only knows how to unpack .tar.gz archives\n  consider setting windows-archive and unix-archive to .tar.gz in your config");
        }
        if artifacts.is_empty() {
            warn!("skipping npm installer: not building any supported platforms (use --artifacts=global)");
            return;
        };

        let installer_artifact = Artifact {
            id: artifact_name,
            target_triples: target_triples.into_iter().collect(),
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
                bin,
                inner: InstallerInfo {
                    dest_path: artifact_path,
                    app_name: release.app_name.clone(),
                    app_version: release.version.to_string(),
                    install_path: release.install_path.clone().into_jinja(),
                    base_url: download_url.to_owned(),
                    artifacts,
                    hint,
                    desc,
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
            let manifest_path = self.workspace.package(pkg_idx).manifest_path.clone();
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
        let builds = match self.workspace.kind {
            axoproject::WorkspaceKind::Generic => self.compute_generic_builds(),
            axoproject::WorkspaceKind::Rust => self.compute_cargo_builds(),
        };
        local_build_steps.extend(builds);

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
                self.add_variant(release, target.clone());
            }
            // Add executable zips to the Release
            self.add_executable_zip(release);

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

    fn compute_ci(&mut self) {
        for ci in &self.inner.ci_style {
            match ci {
                CiStyle::Github => {
                    self.inner.ci.github = Some(GithubCiInfo::new(&self.inner));
                }
            }
        }

        // apply to manifest
        if !self.inner.ci_style.is_empty() {
            let CiInfo { github } = &self.inner.ci;
            let github = github.as_ref().map(|info| cargo_dist_schema::GithubCiInfo {
                artifacts_matrix: Some(info.artifacts_matrix.clone()),
                pr_run_mode: Some(info.pr_run_mode),
            });

            self.manifest.ci = Some(cargo_dist_schema::CiInfo { github });
        }
    }

    pub(crate) fn workspace(&self) -> &'pkg_graph WorkspaceInfo {
        self.workspace
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
        }
    }
    pub(crate) fn global_artifacts_enabled(&self) -> bool {
        match self.artifact_mode {
            ArtifactMode::Local => false,
            ArtifactMode::Global => true,
            ArtifactMode::Host => true,
            ArtifactMode::All => true,
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
pub fn gather_work(cfg: &Config) -> Result<(DistGraph, DistManifest)> {
    info!("analyzing workspace:");
    let tools = tool_info()?;
    let workspace = crate::config::get_project()?;
    let mut graph =
        DistGraphBuilder::new(tools, &workspace, cfg.artifact_mode, cfg.allow_all_dirty)?;

    // Immediately check if there's other manifests kicking around that provide info
    // we don't want to recompute (lets us move towards more of an architecture where
    // `plan` figures out what to do and subsequent steps Simply Obey).
    crate::manifest::load_and_merge_manifests(&graph.inner.dist_dir, &mut graph.manifest)?;

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
        .workspace
        .packages()
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
            return Err(miette!("You specified --artifacts, disabling host mode, but specified no targets to build!"));
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
    let announcing = announce::select_tag(
        &graph,
        cfg.announcement_tag.as_deref(),
        cfg.needs_coherent_announcement_tag,
    )?;

    // Figure out how artifacts should be hosted
    graph.compute_hosting(cfg, &announcing)?;

    // Figure out what we're releasing/building
    graph.compute_releases(cfg, &announcing, triples, bypass_package_target_prefs)?;

    // Prep the announcement's release notes and whatnot
    graph.compute_announcement_info(&announcing);

    // Finally compute all the build steps!
    graph.compute_build_steps();

    // And now figure out how to orchestrate the result in CI
    graph.compute_ci();

    Ok((graph.inner, graph.manifest))
}

/// Get the path/command to invoke Cargo
pub fn cargo() -> Result<String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    Ok(cargo)
}

/// Get the host target triple from cargo
pub fn get_host_target(cargo: String) -> Result<CargoInfo> {
    let mut command = Command::new(&cargo);
    command.arg("-vV");
    info!("exec: {:?}", command);
    let output = command
        .output()
        .into_diagnostic()
        .wrap_err("failed to run 'cargo -vV' (trying to get info about host platform)")?;
    let output = String::from_utf8(output.stdout)
        .into_diagnostic()
        .wrap_err("'cargo -vV' wasn't utf8? Really?")?;
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
    Err(miette!(
        "'cargo -vV' failed to report its host target? Really?"
    ))
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

fn tool_info() -> Result<Tools> {
    let cargo_cmd = cargo()?;
    let cargo = get_host_target(cargo_cmd)?;
    Ok(Tools {
        cargo,
        rustup: find_tool("rustup", "-V"),
        brew: find_tool("brew", "--version"),
    })
}

fn find_tool(name: &str, test_flag: &str) -> Option<Tool> {
    let output = Command::new(name).arg(test_flag).output().ok()?;
    let string_output = String::from_utf8(output.stdout).ok()?;
    let version = string_output.lines().next()?;
    Some(Tool {
        cmd: name.to_owned(),
        version: version.to_owned(),
    })
}
