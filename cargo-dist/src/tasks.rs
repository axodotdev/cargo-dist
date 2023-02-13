//! Code to compute the tasks cargo-dist should do
//!
//! This is the heart and soul of cargo-dist, and ideally the [`gather_work`][] function
//! should compute every minute detail dist will perform ahead of time. This is done with
//! the [`DistGraphBuilder`][], which roughly builds up the work to do as follows:
//!
//! 1. [`workspace_info`][]: find out everything we want to know about the workspace (binaries, configs, etc)
//! 2. compute the TargetTriples we're interested based on ArtifactMode and target configs/flags
//! 3. add Releases for all the binaries selected by the above steps
//! 4. for each TargetTriple, create a ReleaseVariant of each Release
//! 5. add target-specific Binaries to each ReleaseVariant
//! 6. add Artifacts to each Release, which will be propagated to each ReleaseVariant as necessary
//!   1. add executable-zips, propagated to ReleaseVariants
//!   2. add installers, each one decides if it's global or local
//! 7. compute actual BuildSteps from the current graph (a Binary will only induce an actual `cargo build`
//!    here if one of the Artifacts that was added requires outputs from it!)
//! 8. (NOT YET IMPLEMENTED) generate release/announcement notes
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
//!      * Binaries: a binary that should be built for a specifc Variant
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

use std::{
    fs::File,
    io::{BufReader, Read},
    process::Command,
};

use camino::{Utf8Path, Utf8PathBuf};
use guppy::{
    graph::{
        BuildTargetId, DependencyDirection, PackageGraph, PackageMetadata, PackageSet, Workspace,
    },
    MetadataCommand, PackageId,
};
use miette::{miette, Context, IntoDiagnostic};
use semver::Version;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::errors::Result;

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

/// Contents of METADATA_DIST in Cargo.toml files
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct DistMetadata {
    /// The intended version of cargo-dist to build with. (normal Cargo SemVer syntax)
    ///
    /// When generating full tasks graphs (such as CI scripts) we will pick this version.
    ///
    /// TODO: Should we produce a warning if running locally with a different version? In theory
    /// it shouldn't be a problem and newer versions should just be Better... probably you
    /// Really want to have the exact version when running generate-ci to avoid generating
    /// things other cargo-dist versions can't handle!
    #[serde(rename = "cargo-dist-version")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cargo_dist_version: Option<Version>,

    /// The intended version of Rust/Cargo to build with (rustup toolchain syntax)
    ///
    /// When generating full tasks graphs (such as CI scripts) we will pick this version.
    #[serde(rename = "rust-toolchain-version")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rust_toolchain_version: Option<String>,

    /// Whether the package should be distributed/built by cargo-dist
    ///
    /// This mainly exists to be set to `false` to make cargo-dist ignore the existence of this
    /// package. Note that we may still build the package as a side-effect of building the
    /// workspace -- we just won't bundle it up and report it.
    ///
    /// TODO: maybe you should also be allowed to make this a list of binary names..?
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dist: Option<bool>,

    /// CI environments you wish to target.
    ///
    /// Currently only accepts "github".
    ///
    /// When running `generate-ci` with no arguments this list will be used.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ci: Vec<CiStyle>,

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
    /// TODO: list off what values are accepted (msi's are currently only hypothetical!)
    ///
    /// TODO: determine how to use this field when doing a simple local build. I think we
    /// should build every installer that *can* be built on the current platform..?
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installers: Option<Vec<InstallerStyle>>,

    /// The full set of target triples to build for.
    ///
    /// When generating full task graphs (such as CI scripts) we will to try to generate these.
    ///
    /// The inputs should be valid rustc target triples (see `rustc --print target-list`) such
    /// as `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, or `x86_64-unknown-linux-gnu`.
    ///
    /// TODO: We also accept one magic target: `universal2-apple-darwin`. This will induce us to
    /// build both of `x86_64-apple-darwin` and `aarch64-apple-darwin` (arm64) and then combine
    /// them into a "universal" binary that can run on either arch (using apple's `lipo` tool).
    ///
    /// TODO: Allow higher level requests like "[macos, windows, linux] x [x86_64, aarch64]"?
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<String>>,

    /// Include the following static files in bundles like executable-zips.
    ///
    /// Paths are relative to the Cargo.toml this is defined in.
    ///
    /// Files like `README*`, `(UN)LICENSE*`, `RELEASES*`, and `CHANGELOG*` are already
    /// automatically detected and included (use [`DistMetadata::auto_includes`][] to prevent this).
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<Utf8PathBuf>,

    /// Whether to auto-include files like `README*`, `(UN)LICENSE*`, `RELEASES*`, and `CHANGELOG*`
    ///
    /// Defaults to true.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "auto-includes")]
    pub auto_includes: Option<bool>,
}

impl DistMetadata {
    /// Apply the base path to any relative paths contained in this DistMetadata
    fn make_relative_to(&mut self, base_path: &Utf8Path) {
        for include in &mut self.include {
            *include = base_path.join(&*include);
        }
    }

    fn merge_workspace_config(&mut self, workspace_config: &Self) {
        // These fields omitted because they should only be set globally at the workspace root
        // and we should just error if you set them multiple times or locally!
        //
        // * cargo_dist_version
        // * rust_toolchain_version
        // * ci

        if self.installers.is_none() {
            self.installers = workspace_config.installers.clone();
        }
        if self.targets.is_none() {
            self.targets = workspace_config.targets.clone();
        }
        if self.dist.is_none() {
            self.dist = workspace_config.dist;
        }
        if self.auto_includes.is_none() {
            self.auto_includes = workspace_config.auto_includes;
        }
        self.include
            .extend(workspace_config.include.iter().cloned());
    }
}

/// Parts of a [profile.*] entry in a Cargo.toml we care about
#[derive(Debug, Clone)]
pub struct CargoProfile {
    /// Whether debuginfo is enabled.
    ///
    /// can be 0, 1, 2, true (=2), false (=0).
    pub debug: Option<i64>,
    /// Whether split-debuginfo is enabled.
    ///
    /// Can be "off", "packed", or "unpacked".
    ///
    /// If "packed" then we expect a pdb/dsym/dwp artifact.
    pub split_debuginfo: Option<String>,
}

/// Global config for commands
#[derive(Debug)]
pub struct Config {
    /// Whether we'll actually run builds (if false we'll still generate installers)
    pub artifact_mode: ArtifactMode,
    /// Whether local paths to files should be in the final dist json output
    pub no_local_paths: bool,
    /// Target triples we want to build for
    pub targets: Vec<TargetTriple>,
    /// CI kinds we want to support
    pub ci: Vec<CiStyle>,
    /// Installers we want to generate
    pub installers: Vec<InstallerStyle>,
}

/// How we should select the artifacts to build
#[derive(Clone, Copy, Debug)]
pub enum ArtifactMode {
    /// Build target-specific artifacts like executable-zips, symbols, MSIs...
    Local,
    /// Build globally unique artifacts like curl-sh installers, npm packages, metadata...
    Global,
    /// Fuzzily build "as much as possible" for the host system
    Host,
    /// Build all the artifacts; only really appropriate for `cargo-dist manifest`
    All,
}

/// The style of CI we should generate
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum CiStyle {
    /// Generate Github CI
    #[serde(rename = "github")]
    Github,
}

/// The style of Installer we should generate
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum InstallerStyle {
    /// Generate a shell script that fetches from a Github Release
    #[serde(rename = "github-shell")]
    GithubShell,
    /// Generate a powershell script that fetches from a Github Release
    #[serde(rename = "github-powershell")]
    GithubPowershell,
}

/// A unique id for a [`ArtifactTarget`][]
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
pub struct ArtifactIdx(pub usize);

/// A unique id for a [`ReleaseVariant`][]
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
pub struct ReleaseVariantIdx(pub usize);

/// A unique id for a [`ReleaseTarget`][]
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
    // Some simple global facts
    /// The executable cargo told us to find itself at.
    pub cargo: String,
    /// The cargo target dir.
    pub target_dir: Utf8PathBuf,
    /// The root directory of the current cargo workspace.
    pub workspace_dir: Utf8PathBuf,
    /// cargo-dist's target dir (generally nested under `target_dir`).
    pub dist_dir: Utf8PathBuf,

    // Announcement info
    /// Styles of CI we want to support
    pub ci_style: Vec<CiStyle>,
    /// The git tag used for the announcement (e.g. v1.0.0)
    ///
    /// This is important for certain URLs like Github Releases
    pub announce_tag: String,
    /// Title of the announcement
    pub announce_title: Option<String>,
    /// Raw changelog for the announcement
    pub announce_changelog: Option<String>,
    /// Github Releases body for the announcement
    pub announce_github_body: Option<String>,

    // The actual task graph
    /// Targets we need to build
    pub build_steps: Vec<BuildStep>,
    /// Distributable artifacts we want to produce for the releases
    pub artifacts: Vec<Artifact>,
    /// Binaries we want to build
    pub binaries: Vec<Binary>,
    /// Variants of Releases
    pub variants: Vec<ReleaseVariant>,
    /// Logical releases that artifacts are grouped under
    pub releases: Vec<Release>,
}

/// A binary we want to build (specific to a Variant)
#[derive(Debug)]
pub struct Binary {
    /// A unique id to use for things derived from this binary
    ///
    /// (e.g. my-binary-v1.0.0-x86_64-pc-windows-msvc)
    pub id: String,
    /// The package this binary is defined by
    pub pkg_id: PackageId,
    /// The name of the binary (as defined by the Cargo.toml)
    pub name: String,
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
    // In the future this might include feature-flags
}

/// A build step we would like to perform
#[derive(Debug)]
pub enum BuildStep {
    /// Do a cargo build (and copy the outputs to various locations)
    Cargo(CargoBuildStep),
    /// Copy a file
    CopyFile(CopyFileStep),
    /// Copy a dir
    CopyDir(CopyDirStep),
    /// Zip up a directory
    Zip(ZipDirStep),
    /// Generate some kind of installer
    GenerateInstaller(InstallerImpl), // TODO: For macos universal builds we'll want
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

/// zip/tarball some directory
#[derive(Debug)]
pub struct ZipDirStep {
    /// The directory to zip up
    pub src_path: Utf8PathBuf,
    /// The final file path for the output zip
    pub dest_path: Utf8PathBuf,
    /// The kind of zip/tarball to make
    pub zip_style: ZipStyle,
}

/// Copy a file
#[derive(Debug)]
pub struct CopyFileStep {
    /// from here
    pub src_path: Utf8PathBuf,
    /// to here
    pub dest_path: Utf8PathBuf,
}

/// Copy a dir
#[derive(Debug)]
pub struct CopyDirStep {
    /// from here
    pub src_path: Utf8PathBuf,
    /// to here
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
#[derive(Debug)]
pub struct Artifact {
    /// Unique id for the Artifact (its file name)
    ///
    /// i.e. `cargo-dist-v0.1.0-x86_64-pc-windows-msvc.zip`
    pub id: String,
    /// The target platform
    ///
    /// i.e. `x86_64-pc-windows-msvc`
    pub target_triples: Vec<TargetTriple>,
    /// The name of the directory this artifact's contents will be stored in (if necessary).
    ///
    /// This directory is technically a transient thing but it will show up as the name of
    /// the directory in a `tar`. Single file artifacts don't need this.
    ///
    /// i.e. `cargo-dist-v0.1.0-x86_64-pc-windows-msvc`
    pub dir_name: Option<String>,
    /// The path of the directory this artifact's contents will be stored in (if necessary).
    ///
    /// i.e. `/.../target/dist/cargo-dist-v0.1.0-x86_64-pc-windows-msvc/`
    pub dir_path: Option<Utf8PathBuf>,
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
}

/// A kind of artifact (more specific fields)
#[derive(Debug)]
pub enum ArtifactKind {
    /// An executable zip
    ExecutableZip(ExecutableZip),
    /// Symbols
    Symbols(Symbols),
    /// An installer
    Installer(InstallerImpl),
}

/// An ExecutableZip Artifact
#[derive(Debug)]
pub struct ExecutableZip {
    /// The style of zip to make
    pub zip_style: ZipStyle,
    /// Static assets to copy to the root of the artifact's dir (path is src)
    ///
    /// In the future this might add a custom relative dest path
    pub static_assets: Vec<(StaticAssetKind, Utf8PathBuf)>,
}

/// A Symbols/Debuginfo Artifact
#[derive(Debug)]
pub struct Symbols {
    /// The kind of symbols this is
    kind: SymbolKind,
}

/// A logical release of an application that artifacts are grouped under
#[derive(Debug)]
pub struct Release {
    /// The name of the app
    pub app_name: String,
    /// The version of the app
    pub version: Version,
    /// The unique id of the release (e.g. "my-app-v1.0.0")
    pub id: String,
    /// Targets this Release has artifacts for
    pub targets: Vec<TargetTriple>,
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
    /// Static assets that should be included in bundles like executable-zips
    pub static_assets: Vec<(StaticAssetKind, Utf8PathBuf)>,
    /// Artifacts that are "local" to this variant (binaries, symbols, msi-installer...)
    pub local_artifacts: Vec<ArtifactIdx>,
}

/// A particular kind of static asset we're interested in
#[derive(Debug, Clone, Copy)]
pub enum StaticAssetKind {
    /// A README file
    Readme,
    /// A LICENSE file
    License,
    /// A CHANGLEOG or RELEASES file
    Changelog,
    /// Some other miscellanious file
    Other,
}

/// The style of zip/tarball to make
#[derive(Debug, Clone)]
pub enum ZipStyle {
    /// `.zip`
    Zip,
    /// `.tar.<compression>`
    Tar(CompressionImpl),
}

/// Compression impls (used by [`ZipStyle::Tar`][])
#[derive(Debug, Copy, Clone)]
pub enum CompressionImpl {
    /// `.gz`
    Gzip,
    /// `.xz`
    Xzip,
    /// `.zstd`
    Zstd,
}

/// A kind of an installer
#[derive(Debug, Clone)]
pub enum InstallerImpl {
    /// Github Releases shell installer script
    GithubShell(InstallerInfo),
    /// Github Releases powershell installer script
    GithubPowershell(InstallerInfo),
}

/// Generic info about an installer
#[derive(Debug, Clone)]
pub struct InstallerInfo {
    /// The path to generate the installer at
    pub dest_path: Utf8PathBuf,
    /// App name to use
    pub app_name: String,
    /// App version to use
    pub app_version: String,
    /// URL to the repo
    pub repo_url: String,
    /// Description of the installer
    pub desc: String,
    /// Hint for how to run the installer
    pub hint: String,
}

/// Cargo features a [`CargoBuildTarget`][] should use.
#[derive(Debug)]
pub struct CargoTargetFeatures {
    /// Whether to disable default features
    pub no_default_features: bool,
    /// Features to enable
    pub features: CargoTargetFeatureList,
}

/// A list of features to build with
#[derive(Debug)]
pub enum CargoTargetFeatureList {
    /// All of them
    All,
    /// Some of them
    List(Vec<String>),
}

/// Whether to build a package or workspace
#[derive(Debug)]
pub enum CargoTargetPackages {
    /// Build the workspace
    Workspace,
    /// Just build a package
    Package(PackageId),
}

/// Info on the current workspace
pub struct WorkspaceInfo<'pkg_graph> {
    /// Most info on the workspace.
    pub info: Workspace<'pkg_graph>,
    /// The workspace members.
    pub members: PackageSet<'pkg_graph>,
    /// Computed info about the packages beyond what Guppy tells us
    ///
    /// This notably includes finding readmes and licenses even if the user didn't
    /// specify their location -- something Cargo does but Guppy (and cargo-metadata) don't.
    pub package_info: FastMap<&'pkg_graph PackageId, PackageInfo>,
    /// Path to the Cargo.toml of the workspace (may be a package's Cargo.toml)
    pub manifest_path: Utf8PathBuf,
    /// If the manifest_path points to a package, this is the one.
    ///
    /// If this is None, the workspace Cargo.toml is a virtual manifest.
    pub root_package: Option<PackageMetadata<'pkg_graph>>,
    /// The desired cargo-dist version for handling this project
    pub desired_cargo_dist_version: Option<Version>,
    /// The desired rust toolchain for handling this project
    pub desired_rust_toolchain: Option<String>,
    /// The desired ci backends for this project
    pub ci_kinds: Vec<CiStyle>,
    /// Contents of [profile.dist] in the root Cargo.toml
    ///
    /// This is used to determine if we expect split-debuginfo from builds.
    pub dist_profile: Option<CargoProfile>,
    /// A consensus URL for the repo according the workspace
    pub repo_url: Option<String>,
}

/// Computed info about the packages beyond what Guppy tells us
///
/// This notably includes finding readmes and licenses even if the user didn't
/// specify their location -- something Cargo does but Guppy (and cargo-metadata) don't.
#[derive(Debug)]
pub struct PackageInfo {
    /// Name of the package
    pub name: String,
    /// Version of the package
    pub version: Version,
    /// A brief description of the package
    pub description: Option<String>,
    /// Authors of the package (may be empty)
    pub authors: Vec<String>,
    /// The license the package is provided under
    pub license: Option<String>,
    /// False if they set publish=false, true otherwise
    pub publish: bool,
    /// URL to the repository for this package
    ///
    /// This URL can be used by various CI/Installer helpers. In the future we
    /// might also use it for auto-detecting "hey you're using github, here's the
    /// recommended github setup".
    ///
    /// i.e. `--installer=github-shell` uses this as the base URL for fetching from
    /// a Github Release™️.
    pub repository_url: Option<String>,
    /// URL to the homepage for this package.
    ///
    /// Currently this isn't terribly important or useful?
    pub homepage_url: Option<String>,
    /// URL to the documentation for this package.
    ///
    /// This will default to docs.rs if not specified, which is the default crates.io behaviour.
    ///
    /// Currently this isn't terribly important or useful?
    pub documentation_url: Option<String>,
    /// Path to the README file for this package.
    ///
    /// By default this should be copied into a zip containing this package's binary.
    pub readme_file: Option<Utf8PathBuf>,
    /// Paths to the LICENSE files for this package.
    ///
    /// By default these should be copied into a zip containing this package's binary.
    ///
    /// Cargo only lets you specify one such path, but that's because the license path
    /// primarily exists as an escape hatch for someone's whacky-wild custom license.
    /// But for our usecase we want to find those paths even if they're bog standard
    /// MIT/Apache, which traditionally involves two separate license files.
    pub license_files: Vec<Utf8PathBuf>,
    /// Paths to the CHANGELOG or RELEASES file for this package
    ///
    /// By default this should be copied into a zip containing this package's binary.
    ///
    /// We will *try* to parse this
    pub changelog_file: Option<Utf8PathBuf>,
    /// Names of binaries this package defines
    pub binaries: Vec<String>,
    /// DistMetadata for the package (with workspace merged and paths made absolute)
    pub config: DistMetadata,
}

struct DistGraphBuilder<'pkg_graph> {
    inner: DistGraph,
    workspace: &'pkg_graph WorkspaceInfo<'pkg_graph>,
    artifact_mode: ArtifactMode,
    binaries_by_id: FastMap<String, BinaryIdx>,
}

impl<'pkg_graph> DistGraphBuilder<'pkg_graph> {
    fn new(
        cargo: String,
        workspace: &'pkg_graph WorkspaceInfo<'pkg_graph>,
        artifact_mode: ArtifactMode,
    ) -> Self {
        let target_dir = workspace.info.target_directory().to_owned();
        let workspace_dir = workspace.info.root().to_owned();
        let dist_dir = target_dir.join(TARGET_DIST);

        // The git tag associated with this announcement, which is used in things
        // like github-releases URLs, so we need it to be defined globally.
        // TODO: source this properly from a CLI flag and/or auto-detection
        let announce_tag = "v1.0.0-FAKEVER".to_owned();

        Self {
            inner: DistGraph {
                cargo,
                target_dir,
                workspace_dir,
                dist_dir,
                announce_tag,
                announce_changelog: None,
                announce_github_body: None,
                announce_title: None,
                ci_style: vec![],
                build_steps: vec![],
                artifacts: vec![],
                binaries: vec![],
                variants: vec![],
                releases: vec![],
            },
            workspace,
            binaries_by_id: FastMap::new(),
            artifact_mode,
        }
    }

    fn set_ci_style(&mut self, style: Vec<CiStyle>) {
        self.inner.ci_style = style;
    }

    fn add_release(&mut self, app_name: String, version: Version) -> ReleaseIdx {
        let idx = ReleaseIdx(self.inner.releases.len());
        let id = format!("{app_name}-v{version}");
        self.inner.releases.push(Release {
            app_name,
            version,
            id,
            global_artifacts: vec![],
            targets: vec![],
            variants: vec![],
            changelog_body: None,
            changelog_title: None,
        });
        idx
    }

    fn add_variant(&mut self, to_release: ReleaseIdx, target: TargetTriple) -> ReleaseVariantIdx {
        let idx = ReleaseVariantIdx(self.inner.variants.len());
        let Release {
            id: release_id,
            variants,
            targets,
            ..
        } = self.release_mut(to_release);
        let id = format!("{release_id}-{target}");

        variants.push(idx);
        targets.push(target.clone());

        self.inner.variants.push(ReleaseVariant {
            target,
            id,
            local_artifacts: vec![],
            binaries: vec![],
            static_assets: vec![],
        });
        idx
    }

    fn add_binary(
        &mut self,
        to_variant: ReleaseVariantIdx,
        pkg_id: &PackageId,
        binary_name: String,
    ) {
        let variant = self.variant(to_variant);
        let pkg_id = pkg_id.clone();
        let target = variant.target.clone();
        let version = &self.workspace.package_info[&pkg_id].version;
        let id = format!("{binary_name}-v{version}-{target}");

        // If we already are building this binary we don't need to do it again!
        let idx = if let Some(&idx) = self.binaries_by_id.get(&id) {
            idx
        } else {
            let idx = BinaryIdx(self.inner.binaries.len());
            let binary = Binary {
                id,
                pkg_id: pkg_id.clone(),
                name: binary_name,
                target: variant.target.clone(),
                copy_exe_to: vec![],
                copy_symbols_to: vec![],
                symbols_artifact: None,
            };
            self.inner.binaries.push(binary);
            idx
        };

        self.variant_mut(to_variant).binaries.push(idx);
    }

    fn add_static_asset(
        &mut self,
        to_variant: ReleaseVariantIdx,
        kind: StaticAssetKind,
        path: Utf8PathBuf,
    ) {
        let ReleaseVariant { static_assets, .. } = self.variant_mut(to_variant);
        static_assets.push((kind, path));
    }

    fn add_executable_zip(&mut self, to_release: ReleaseIdx) {
        if !self.local_artifacts_enabled() {
            return;
        }

        // Create an executable-zip for each Variant
        let release = self.release(to_release);
        let variants = release.variants.clone();
        for variant_idx in variants {
            // This is largely just a lot of path/name computation
            let dist_dir = &self.inner.dist_dir;
            let variant = self.variant(variant_idx);

            // FIXME: this should be configurable
            let target_is_windows = variant.target.contains("windows");
            let zip_style = if target_is_windows {
                // Windows loves them zips
                ZipStyle::Zip
            } else {
                // tar.xz is well-supported everywhere and much better than tar.gz
                ZipStyle::Tar(CompressionImpl::Xzip)
            };
            let platform_exe_ext = if target_is_windows { ".exe" } else { "" };

            let artifact_dir_name = variant.id.clone();
            let artifact_dir_path = dist_dir.join(&artifact_dir_name);
            let artifact_ext = match zip_style {
                ZipStyle::Zip => ".zip",
                ZipStyle::Tar(CompressionImpl::Gzip) => ".tar.gz",
                ZipStyle::Tar(CompressionImpl::Zstd) => ".tar.zstd",
                ZipStyle::Tar(CompressionImpl::Xzip) => ".tar.xz",
            };
            let artifact_name = format!("{artifact_dir_name}{artifact_ext}");
            let artifact_path = dist_dir.join(&artifact_name);

            let static_assets = variant.static_assets.clone();
            let mut built_assets = Vec::new();
            for &binary_idx in &variant.binaries {
                let binary = self.binary(binary_idx);
                let exe_name = &binary.name;
                let exe_filename = format!("{exe_name}{platform_exe_ext}");
                built_assets.push((binary_idx, artifact_dir_path.join(exe_filename)));
            }

            let zip_artifact = Artifact {
                id: artifact_name,
                target_triples: vec![variant.target.clone()],
                dir_name: Some(artifact_dir_name),
                dir_path: Some(artifact_dir_path),
                file_path: artifact_path,
                required_binaries: FastMap::new(),
                kind: ArtifactKind::ExecutableZip(ExecutableZip {
                    zip_style,
                    static_assets,
                }),
            };

            let zip_artifact_idx = self.add_local_artifact(variant_idx, zip_artifact);
            for (binary, dest_path) in built_assets {
                self.require_binary(zip_artifact_idx, variant_idx, binary, dest_path);
            }
        }
    }

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
                    dir_name: None,
                    dir_path: None,
                    file_path: artifact_path.clone(),
                    required_binaries: FastMap::new(),
                    kind: ArtifactKind::Symbols(Symbols { kind: symbol_kind }),
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

    fn add_installer(&mut self, to_release: ReleaseIdx, installer: &InstallerStyle) {
        match installer {
            InstallerStyle::GithubShell => self.add_github_shell_installer(to_release),
            InstallerStyle::GithubPowershell => self.add_github_powershell_installer(to_release),
        }
    }

    fn add_github_shell_installer(&mut self, to_release: ReleaseIdx) {
        // TODO: completely rework this and the impl in installer.rs to properly use `announce_tag`
        if !self.global_artifacts_enabled() {
            return;
        }
        let release = self.release(to_release);
        let release_id = &release.id;
        let repo = self.workspace().repo_url.clone(); // &self.workspace.repo_url;
        let Some(repo_url) = repo else {
            warn!("skipping --installer=github-shell: 'repository' isn't consistently set in your Cargo.tomls");
            return;
        };
        let artifact_name = format!("{release_id}-installer.sh");
        let artifact_path = self.inner.dist_dir.join(&artifact_name);
        // All the triples we know about, sans windows (windows-gnu isn't handled...)
        let target_triples = release
            .targets
            .iter()
            .filter(|s| s.contains("windows"))
            .cloned()
            .collect::<Vec<_>>();
        let tag = &self.inner.announce_tag;
        let base_url = format!("{repo_url}/releases/download/{tag}");
        let download_url = format!("{base_url}/{artifact_name}");
        let hint = format!("# WARNING: this installer is experimental\ncurl --proto '=https' --tlsv1.2 -L -sSf {download_url} | sh");
        let desc = "Install prebuilt binaries via shell script".to_owned();

        let installer_artifact = Artifact {
            id: artifact_name,
            target_triples,
            dir_name: None,
            dir_path: None,
            file_path: artifact_path.clone(),
            required_binaries: FastMap::new(),
            kind: ArtifactKind::Installer(InstallerImpl::GithubShell(InstallerInfo {
                dest_path: artifact_path,
                app_name: release.app_name.clone(),
                app_version: format!("v{}", release.version),
                repo_url,
                hint,
                desc,
            })),
        };

        self.add_global_artifact(to_release, installer_artifact);
    }

    fn add_github_powershell_installer(&mut self, to_release: ReleaseIdx) {
        // TODO: completely rework this and the impl in installer.rs to properly use `announce_tag`
        if !self.global_artifacts_enabled() {
            return;
        }
        let release = self.release(to_release);
        let release_id = &release.id;
        let repo = self.workspace().repo_url.clone(); // &self.workspace.repo_url;
        let Some(repo_url) = repo else {
            warn!("skipping --installer=github-powershell: 'repository' isn't consistently set in your Cargo.tomls");
            return;
        };
        let artifact_name = format!("{release_id}-installer.ps1");
        let artifact_path = self.inner.dist_dir.join(&artifact_name);
        // All the triples we know about, sans windows (windows-gnu isn't handled...)
        let target_triples = release
            .targets
            .iter()
            .filter(|s| !s.contains("windows"))
            .cloned()
            .collect::<Vec<_>>();
        let tag = &self.inner.announce_tag;
        let base_url = format!("{repo_url}/releases/download/{tag}");
        let download_url = format!("{base_url}/{artifact_name}");
        let hint = format!("# WARNING: this installer is experimental\nirm '{download_url}' | iex");
        let desc = "Install prebuilt binaries via shell script".to_owned();

        let installer_artifact = Artifact {
            id: artifact_name,
            target_triples,
            dir_name: None,
            dir_path: None,
            file_path: artifact_path.clone(),
            required_binaries: FastMap::new(),
            kind: ArtifactKind::Installer(InstallerImpl::GithubPowershell(InstallerInfo {
                dest_path: artifact_path,
                app_name: release.app_name.clone(),
                app_version: format!("v{}", release.version),
                repo_url,
                hint,
                desc,
            })),
        };

        self.add_global_artifact(to_release, installer_artifact);
    }

    fn add_local_artifact(
        &mut self,
        to_variant: ReleaseVariantIdx,
        artifact: Artifact,
    ) -> ArtifactIdx {
        assert!(self.local_artifacts_enabled());

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

        let mut build_steps = vec![];
        let cargo_builds = self.compute_cargo_builds();
        build_steps.extend(cargo_builds.into_iter().map(BuildStep::Cargo));

        for artifact in &self.inner.artifacts {
            match &artifact.kind {
                ArtifactKind::ExecutableZip(zip) => {
                    let artifact_dir = artifact.dir_path.as_ref().unwrap();
                    // Copy all the static assets
                    for (_, src_path) in &zip.static_assets {
                        let src_path = src_path.clone();
                        let file_name = src_path.file_name().unwrap();
                        let dest_path = artifact_dir.join(file_name);
                        if src_path.is_dir() {
                            build_steps.push(BuildStep::CopyDir(CopyDirStep {
                                src_path,
                                dest_path,
                            }))
                        } else {
                            build_steps.push(BuildStep::CopyFile(CopyFileStep {
                                src_path,
                                dest_path,
                            }))
                        }
                    }
                    // Zip up the artifact
                    build_steps.push(BuildStep::Zip(ZipDirStep {
                        src_path: artifact_dir.to_owned(),
                        dest_path: artifact.file_path.clone(),
                        zip_style: zip.zip_style.clone(),
                    }));
                }
                ArtifactKind::Symbols(symbols) => {
                    match symbols.kind {
                        SymbolKind::Pdb => {
                            // No additional steps needed, the file is PERFECT (for now)
                        }
                        SymbolKind::Dsym => {
                            // TODO: compress the dSYM in a .tar.xz, it's a actually a directory!
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
            }
        }

        self.inner.build_steps = build_steps;
    }

    fn compute_cargo_builds(&mut self) -> Vec<CargoBuildStep> {
        // For now we can be really simplistic and just do a workspace build for every
        // target-triple we have a binary-that-needs-a-real-build for.
        let mut targets = SortedMap::<TargetTriple, Vec<BinaryIdx>>::new();
        for (binary_idx, binary) in self.inner.binaries.iter().enumerate() {
            if !binary.copy_exe_to.is_empty() || !binary.copy_symbols_to.is_empty() {
                targets
                    .entry(binary.target.clone())
                    .or_default()
                    .push(BinaryIdx(binary_idx));
            }
        }

        let mut builds = vec![];
        for (target, binaries) in targets {
            let mut rustflags = std::env::var("RUSTFLAGS").unwrap_or_default();

            // TODO: figure out a principled way for us to add things to RUSTFLAGS
            // without breaking everything. Cargo has some builtin ways like keys
            // in [target...] tables that will get "merged" with the flags it wants
            // to set. More blunt approaches like actually setting the environment
            // variable I think can result in overwriting flags other places set
            // (which is defensible, having spaghetti flags randomly injected by
            // a dozen different tools is a build maintenance nightmare!)

            // TODO: on windows, set RUSTFLAGS="-Ctarget-feature=+crt-static"
            // See: https://rust-lang.github.io/rfcs/1721-crt-static.html
            //
            // Essentially you're *supposed* to be statically linking the windows """libc"""
            // because it's actually a wrapper around more fundamental DLLs and not
            // actually guaranteed to be on the system. This is why lots of games
            // install a C/C++ runtime in their wizards! Unclear what the cost/benefit
            // is of "install" vs "statically link", especially if you only need C
            // and not all of C++. I am however unclear on "which" "libc" you're statically
            // linking. More Research Needed.
            //
            // For similar reasons we may want to perfer targeting "linux-musl" over
            // "linux-gnu" -- the former statically links libc and makes us more portable
            // to "weird" linux setups like NixOS which apparently doesn't have like
            // /etc or /lib to try to try to force things to properly specify their deps
            // (statically linking libc says "no deps pls" (except for specific linux syscalls probably)).
            // I am however vaguely aware of issues where some system magic is hidden away
            // in the gnu libc (glibc) and musl subsequently diverges and acts wonky?
            // This is all vague folklore to me, so More Research Needed.
            //
            // Just to round things out, let's discuss macos. I've never heard of these kinds
            // of issues wrt macos! However I am vaguely aware that macos has an "sdk version"
            // system, which vaguely specifies what APIs you're allowing yourself to use so
            // you can be compatible with any system at least that new (so the older the SDK,
            // the more compatible you are). Do we need to care about that? More Research Needed.

            if target.contains("windows-msvc") {
                rustflags.push_str(" -Ctarget-feature=+crt-static");
            }

            builds.push(CargoBuildStep {
                target_triple: target.clone(),
                // Just build the whole workspace for now
                package: CargoTargetPackages::Workspace,
                // Just use the default build for now
                features: CargoTargetFeatures {
                    no_default_features: false,
                    features: CargoTargetFeatureList::List(vec![]),
                },
                rustflags,
                profile: String::from(PROFILE_DIST),
                expected_binaries: binaries,
            });
        }
        builds
    }

    fn compute_announcement_info(&mut self) {
        // Add release notes
        /*
        for ((_app_name, version), (package_id, release)) in &mut releases {
            let package_info = &workspace.package_info[&&*package_id];

            // Try to parse out relevant parts of the changelog
            // FIXME: ...this is kind of excessive to do eagerly and for each crate in the workspace
            if let Some(changelog_path) = &package_info.changelog_file {
                if let Ok(changelog_str) = try_load_changelog(changelog_path) {
                    let changelogs = parse_changelog::parse(&changelog_str)
                        .into_diagnostic()
                        .wrap_err_with(|| format!("failed to parse changelog at {changelog_path}"));
                    if let Ok(changelogs) = changelogs {
                        let version_string = format!("{}", package_info.version);
                        if let Some(release_notes) = changelogs.get(&*version_string) {
                            release.changelog_title = Some(release_notes.title.to_owned());
                            release.changelog_body = Some(release_notes.notes.to_owned());
                        }
                    }
                }
            }

            use std::fmt::Write;
            let mut changelog_body = String::new();

            let mut installers = vec![];
            let mut bundles = vec![];
            let mut symbols = vec![];
            let mut other = vec![];
            for artifact_idx in &release.global_artifacts {
                let artifact = &artifacts[artifact_idx.0];
                match artifact.kind {
                    ArtifactKind::ExecutableZip => {
                        bundles.push(artifact);
                    }
                    ArtifactKind::Symbols => {
                        symbols.push(artifact);
                    }
                    ArtifactKind::DistMetadata => {
                        // Do nothing
                    }
                    ArtifactKind::Installer => {
                        installers.push(artifact);
                    }
                    ArtifactKind::Unknown => {
                        other.push(artifact);
                    }
                    _ => todo!(),
                }
            }

            if !installers.is_empty() {
                changelog_body.push_str("## Install\n\n");
                for installer in installers {
                    let install_hint;
                    let description;

                    match &installer.bundle {
                        BundleStyle::Installer(InstallerImpl::GithubShell(info)) => {
                            install_hint = Some(info.hint.clone());
                            description = Some(info.desc.clone());
                        }
                        BundleStyle::Installer(InstallerImpl::GithubPowershell(info)) => {
                            install_hint = Some(info.hint.clone());
                            description = Some(info.desc.clone());
                        }
                        BundleStyle::Zip | BundleStyle::Tar(_) | BundleStyle::UncompressedFile => {
                            unreachable!()
                        }
                    }

                    let (Some(hint), Some(desc)) = (install_hint, description) else {
                        continue;
                    };

                    writeln!(&mut changelog_body, "### {desc}\n").unwrap();
                    writeln!(&mut changelog_body, "```shell\n{hint}\n```\n").unwrap();
                }
            }

            let repo_url = package_info.repository_url.as_deref();
            if (bundles.is_empty() || !symbols.is_empty() || !other.is_empty()) && repo_url.is_some() {
                // FIXME: this is a nasty cludge and we should use --ci=github here to take this path
                #[allow(clippy::unnecessary_unwrap)]
                let repo_url = repo_url.unwrap();

                changelog_body.push_str("## Download\n\n");
                changelog_body.push_str("| target | kind | download |\n");
                changelog_body.push_str("|--------|------|----------|\n");
                for artifact in bundles.iter().chain(&symbols).chain(&other) {
                    let mut targets = String::new();
                    let mut multi_target = false;
                    for target in &artifact.target_triples {
                        if multi_target {
                            targets.push_str(", ");
                        }
                        targets.push_str(target);
                        multi_target = true;
                    }
                    let kind = match artifact.kind {
                        ArtifactKind::ExecutableZip => "tarball",
                        ArtifactKind::Symbols => "symbols",
                        ArtifactKind::DistMetadata => "dist-manifest.json",
                        ArtifactKind::Installer => unreachable!(),
                        _ => "other",
                    };
                    let name = artifact.file_path.file_name().unwrap().to_owned();
                    let app_version = format!("v{version}");

                    let download_url = format!("{repo_url}/releases/download/{app_version}/{name}");
                    let download = format!("[{name}]({download_url})");
                    writeln!(&mut changelog_body, "| {targets} | {kind} | {download} |").unwrap();
                }
                writeln!(&mut changelog_body).unwrap();
            }

            if let Some(old_changelog_body) = release.changelog_body.take() {
                changelog_body.push_str("## Release Notes\n\n");
                changelog_body.push_str(&old_changelog_body);
            }

            release.changelog_title = release
                .changelog_title
                .take()
                .or_else(|| Some(format!("v{version}")));
            release.changelog_body = Some(changelog_body);
        }
         */
    }

    fn cargo(&self) -> &str {
        &self.inner.cargo
    }
    fn workspace(&self) -> &'pkg_graph WorkspaceInfo<'pkg_graph> {
        self.workspace
    }
    fn binary(&self, idx: BinaryIdx) -> &Binary {
        &self.inner.binaries[idx.0]
    }
    fn binary_mut(&mut self, idx: BinaryIdx) -> &mut Binary {
        &mut self.inner.binaries[idx.0]
    }
    fn artifact_mut(&mut self, idx: ArtifactIdx) -> &mut Artifact {
        &mut self.inner.artifacts[idx.0]
    }
    fn release(&self, idx: ReleaseIdx) -> &Release {
        &self.inner.releases[idx.0]
    }
    fn release_mut(&mut self, idx: ReleaseIdx) -> &mut Release {
        &mut self.inner.releases[idx.0]
    }
    fn variant(&self, idx: ReleaseVariantIdx) -> &ReleaseVariant {
        &self.inner.variants[idx.0]
    }
    fn variant_mut(&mut self, idx: ReleaseVariantIdx) -> &mut ReleaseVariant {
        &mut self.inner.variants[idx.0]
    }
    fn local_artifacts_enabled(&self) -> bool {
        match self.artifact_mode {
            ArtifactMode::Local => true,
            ArtifactMode::Global => false,
            ArtifactMode::Host => true,
            ArtifactMode::All => true,
        }
    }
    fn global_artifacts_enabled(&self) -> bool {
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
pub fn gather_work(cfg: &Config) -> Result<DistGraph> {
    eprintln!("analyzing workspace:");
    let cargo = cargo()?;
    let package_graph = package_graph(&cargo)?;
    let workspace = workspace_info(&package_graph)?;
    let mut graph = DistGraphBuilder::new(cargo, &workspace, cfg.artifact_mode);

    if cfg.ci.is_empty() {
        graph.set_ci_style(workspace.ci_kinds.clone());
    } else {
        graph.set_ci_style(cfg.ci.clone());
    }

    // If no targets were specified, just use the host target
    let host_target_triple = [get_host_target(graph.cargo())?];
    // If all targets specified, union together the targets our packages support
    // Note that this uses BTreeSet as an intermediate to make the order stable
    let all_target_triples = graph
        .workspace
        .package_info
        .iter()
        .flat_map(|(_, info)| info.config.targets.iter().flatten())
        .collect::<SortedSet<_>>()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();

    // Choose which set of target triples we're building for
    let mut bypass_package_target_prefs = false;
    let triples = if cfg.targets.is_empty() {
        if let ArtifactMode::Host = cfg.artifact_mode {
            // In "host" mode we want to build for the host arch regardless of what the
            // packages claim they support.
            //
            // FIXME: may cause sadness for "truly platform-specific" bins like a windows-only util
            // FIXME: it would be nice to do "easy" crosses like x64 mac => arm64 + universal2
            bypass_package_target_prefs = true;
            &host_target_triple
        } else {
            // Otherwise assume the user wants all targets (desirable for --artifacts=global)
            &all_target_triples[..]
        }
    } else {
        // If the CLI has explicit targets, only use those!
        &cfg.targets[..]
    };

    // Choose which binaries we want to release
    let disabled_sty = console::Style::new().dim();
    let enabled_sty = console::Style::new();
    let mut rust_releases = vec![];
    for (pkg_id, pkg) in &workspace.package_info {
        let pkg_name = &pkg.name;

        // Determine if this package's binaries should be Released
        let mut disabled_reason = None;
        if pkg.binaries.is_empty() {
            // Nothing to publish if there's no binaries!
            disabled_reason = Some("no binaries");
        } else if let Some(do_dist) = pkg.config.dist {
            // If [metadata.dist].dist is explicitly set, respect it!
            if !do_dist {
                disabled_reason = Some("dist = false");
            }
        } else if !pkg.publish {
            // Otherwise defer to Cargo's `publish = false`
            disabled_reason = Some("publish = false");
        }

        // Report our conclusion/discoveries
        let sty;
        if let Some(reason) = disabled_reason {
            sty = &disabled_sty;
            eprintln!("  {}", sty.apply_to(format!("{pkg_name} ({reason})")));
        } else {
            sty = &enabled_sty;
            eprintln!("  {}", sty.apply_to(pkg_name));
        }

        // Report each binary and potentially add it to the Release for this package
        let mut rust_binaries = vec![];
        for binary in &pkg.binaries {
            eprintln!("    {}", sty.apply_to(format!("[bin] {}", binary)));
            // In the future might want to allow this to be granular for each binary
            if disabled_reason.is_none() {
                rust_binaries.push(binary);
            }
        }

        // If any binaries were accepted for this package, it's a Release!
        if !rust_binaries.is_empty() {
            rust_releases.push((*pkg_id, rust_binaries));
        }
    }
    eprintln!();

    // Give a friendly hint
    if rust_releases.is_empty() {
        let sty = console::Style::new().red();
        eprintln!(
            "{}",
            sty.apply_to("...hey this workspace doesn't have anything to dist!")
        );
        eprintln!();
    }

    // Create a Release for each package
    for (pkg_id, binaries) in &rust_releases {
        // TODO: make app name configurable? Use some other fields in the PackageMetadata?
        let package_info = &graph.workspace().package_info[pkg_id];
        let app_name = package_info.name.clone();
        let version = package_info.version.clone();

        // Create a Release for this binary
        let release = graph.add_release(app_name, version);

        // Create variants for this Release for each target
        for target in triples {
            let use_target = bypass_package_target_prefs
                || package_info
                    .config
                    .targets
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .any(|t| t == target);
            if !use_target {
                continue;
            }

            // Create the variant
            let variant = graph.add_variant(release, target.clone());

            // Tell the variant to include this binary
            for binary in binaries {
                graph.add_binary(variant, pkg_id, (*binary).clone());
            }

            // Add static assets
            let auto_includes_enabled = package_info.config.auto_includes.unwrap_or(true);
            if auto_includes_enabled {
                if let Some(readme) = &package_info.readme_file {
                    graph.add_static_asset(variant, StaticAssetKind::Readme, readme.clone());
                }
                if let Some(changelog) = &package_info.changelog_file {
                    graph.add_static_asset(variant, StaticAssetKind::Changelog, changelog.clone());
                }
                for license in &package_info.license_files {
                    graph.add_static_asset(variant, StaticAssetKind::License, license.clone());
                }
            }
            for static_asset in &package_info.config.include {
                graph.add_static_asset(variant, StaticAssetKind::Other, static_asset.clone())
            }
        }
        // Add executable zips to the Release
        graph.add_executable_zip(release);

        // Add installers to the Release
        let installers = if cfg.installers.is_empty() {
            package_info
                .config
                .installers
                .as_deref()
                .unwrap_or_default()
        } else {
            &cfg.installers[..]
        };
        for installer in installers {
            graph.add_installer(release, installer);
        }
    }

    // Prep the announcement's release notes and whatnot
    graph.compute_announcement_info();

    // Finally compute all the build steps!
    graph.compute_build_steps();

    Ok(graph.inner)
}

/// Get the path/command to invoke Cargo
pub fn cargo() -> Result<String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    Ok(cargo)
}

/// Get the host target triple from cargo
pub fn get_host_target(cargo: &str) -> Result<String> {
    let mut command = Command::new(cargo);
    command.arg("-vV");
    info!("exec: {:?}", command);
    let output = command
        .output()
        .into_diagnostic()
        .wrap_err("failed to run 'cargo -vV' (trying to get info about host platform)")?;
    let output = String::from_utf8(output.stdout)
        .into_diagnostic()
        .wrap_err("'cargo -vV' wasn't utf8? Really?")?;
    for line in output.lines() {
        if let Some(target) = line.strip_prefix("host: ") {
            info!("host target is {target}");
            return Ok(target.to_owned());
        }
    }
    Err(miette!(
        "'cargo -vV' failed to report its host target? Really?"
    ))
}

/// Get the PackageGraph for the current workspace
pub fn package_graph(cargo: &str) -> Result<PackageGraph> {
    let mut metadata_cmd = MetadataCommand::new();
    // guppy will source from the same place as us, but let's be paranoid and make sure
    // EVERYTHING is DEFINITELY ALWAYS using the same Cargo!
    metadata_cmd.cargo_path(cargo);

    // TODO: add a bunch of CLI flags for this. Ideally we'd use clap_cargo
    // but it wants us to use `flatten` and then we wouldn't be able to mark
    // the flags as global for all subcommands :(
    let pkg_graph = metadata_cmd
        .build_graph()
        .into_diagnostic()
        .wrap_err("failed to read 'cargo metadata'")?;

    Ok(pkg_graph)
}

/// Computes [`WorkspaceInfo`][] for the current workspace.
pub fn workspace_info(pkg_graph: &PackageGraph) -> Result<WorkspaceInfo> {
    let workspace = pkg_graph.workspace();
    let members = pkg_graph.resolve_workspace();

    let manifest_path = workspace.root().join("Cargo.toml");
    if !manifest_path.exists() {
        return Err(miette!("couldn't find root workspace Cargo.toml"));
    }
    // If this is Some, then the root Cargo.toml is for a specific package and not a virtual (workspace) manifest.
    // This affects things like [workspace.metadata] vs [package.metadata]
    //
    // FIXME: actually I think this works more uniformly than I thought and maybe we don't need to pay attention
    // to this at all? The root package can still have [workspace.metadata] and in fact we're mandating that
    // for "global" settings like ci/cargo-dist-version.
    let root_package = members
        .packages(DependencyDirection::Forward)
        .find(|p| p.manifest_path() == manifest_path);

    // Get the [workspace.metadata.dist] table, which can be set either in a virtual
    // manifest or a root package (this code handles them uniformly).
    let mut workspace_config = workspace
        .metadata_table()
        .get(METADATA_DIST)
        .map(DistMetadata::deserialize)
        .transpose()
        .into_diagnostic()
        .wrap_err("couldn't parse [workspace.metadata.dist]")?
        .unwrap_or_default();

    let dist_profile = get_dist_profile(&manifest_path)
        .map_err(|e| {
            let err = e.wrap_err("failed to load [profile.dist] from toml");
            info!("{:?}", err);
        })
        .ok();

    let workspace_root = workspace.root();
    workspace_config.make_relative_to(workspace_root);

    let mut repo_url_conflicted = false;
    let mut repo_url = None;
    let mut all_package_info = FastMap::new();
    for package in members.packages(DependencyDirection::Forward) {
        let info = package_info(workspace_root, &workspace_config, &package)?;

        if info.config.cargo_dist_version.is_some() {
            warn!("package.metadata.dist.cargo-dist-version is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package.manifest_path());
        }
        if info.config.cargo_dist_version.is_some() {
            warn!("package.metadata.dist.rust-toolchain-version is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package.manifest_path());
        }
        if !info.config.ci.is_empty() {
            warn!("package.metadata.dist.ci is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package.manifest_path());
        }

        // Try to find repo URL consensus
        if !repo_url_conflicted {
            if let Some(new_url) = &info.repository_url {
                if let Some(cur_url) = &repo_url {
                    if new_url == cur_url {
                        // great! consensus!
                    } else {
                        repo_url_conflicted = true;
                        repo_url = None;
                    }
                } else {
                    repo_url = info.repository_url.clone();
                }
            }
        }

        all_package_info.insert(package.id(), info);
    }

    Ok(WorkspaceInfo {
        info: workspace,
        members,
        package_info: all_package_info,
        manifest_path,
        root_package,
        dist_profile,
        desired_cargo_dist_version: workspace_config.cargo_dist_version,
        desired_rust_toolchain: workspace_config.rust_toolchain_version,
        ci_kinds: workspace_config.ci,
        repo_url,
    })
}

fn package_info(
    workspace_root: &Utf8Path,
    workspace_config: &DistMetadata,
    package: &PackageMetadata,
) -> Result<PackageInfo> {
    // Is there a better way to get the path to sniff?
    // Should we spider more than just package_root and workspace_root?
    // Should we more carefully prevent grabbing LICENSES from both dirs?
    // Should we not spider the workspace root for README since Cargo has a proper field for this?
    // Should we check for a "readme=..." on the workspace root Cargo.toml?
    let manifest_path = package.manifest_path();
    let package_root = manifest_path
        .parent()
        .expect("package manifest had no parent!?");

    let mut package_config = package
        .metadata_table()
        .get(METADATA_DIST)
        .map(DistMetadata::deserialize)
        .transpose()
        .into_diagnostic()
        .wrap_err("couldn't parse [package.metadata.dist]")?
        .unwrap_or_default();
    package_config.make_relative_to(package_root);
    package_config.merge_workspace_config(workspace_config);

    let mut binaries = vec![];
    for target in package.build_targets() {
        let build_id = target.id();
        if let BuildTargetId::Binary(name) = build_id {
            binaries.push(name.to_owned());
        }
    }

    let mut info = PackageInfo {
        name: package.name().to_owned(),
        version: package.version().to_owned(),
        description: package.description().map(ToOwned::to_owned),
        authors: package.authors().to_vec(),
        license: package.license().map(ToOwned::to_owned),
        publish: !package.publish().is_never(),
        repository_url: package.repository().map(ToOwned::to_owned),
        homepage_url: package.homepage().map(ToOwned::to_owned),
        documentation_url: package.documentation().map(ToOwned::to_owned),
        readme_file: package.readme().map(|readme| package_root.join(readme)),
        license_files: package
            .license_file()
            .map(ToOwned::to_owned)
            .into_iter()
            .collect(),
        changelog_file: None,
        binaries,
        config: package_config,
    };

    // We don't want to search for any license files if one is manually given
    // (need to check that here since we can find multiple licenses).
    let search_for_license_file = info.license_files.is_empty();

    // If there's no documentation URL provided, default assume it's docs.rs like crates.io does
    if info.documentation_url.is_none() {
        info.documentation_url = Some(format!("https://docs.rs/{}/{}", info.name, info.version));
    }

    for dir in &[package_root, workspace_root] {
        let entries = dir
            .read_dir_utf8()
            .into_diagnostic()
            .wrap_err("Failed to read workspace dir")?;
        for entry in entries {
            let entry = entry
                .into_diagnostic()
                .wrap_err("Failed to read workspace dir entry")?;
            let meta = entry
                .file_type()
                .into_diagnostic()
                .wrap_err("Failed to read workspace dir entry's metadata")?;
            if !meta.is_file() {
                continue;
            }
            let file_name = entry.file_name();
            if file_name.starts_with("README") {
                if info.readme_file.is_none() {
                    let path = entry.path().to_owned();
                    info!("Found README for {}: {}", info.name, path);
                    info.readme_file = Some(path);
                } else {
                    info!(
                        "Ignoring candidate README for {}: {}",
                        info.name,
                        entry.path()
                    );
                }
            } else if file_name.starts_with("LICENSE") || file_name.starts_with("UNLICENSE") {
                if search_for_license_file {
                    let path = entry.path().to_owned();
                    info!("Found LICENSE for {}: {}", info.name, path);
                    info.license_files.push(path);
                } else {
                    info!(
                        "Ignoring candidate LICENSE for {}: {}",
                        info.name,
                        entry.path()
                    );
                }
            } else if file_name.starts_with("CHANGELOG") || file_name.starts_with("RELEASES") {
                if info.changelog_file.is_none() {
                    let path = entry.path().to_owned();
                    info!("Found CHANGELOG for {}: {}", info.name, path);
                    info.changelog_file = Some(path);
                } else {
                    info!(
                        "Ignoring candidate CHANGELOG for {}: {}",
                        info.name,
                        entry.path()
                    );
                }
            }
        }
    }

    Ok(info)
}

/// Load the root workspace toml into toml-edit form
pub fn load_root_cargo_toml(manifest_path: &Utf8Path) -> Result<toml_edit::Document> {
    // FIXME: this should really be factored out into some sort of i/o module
    let mut workspace_toml_file = File::open(manifest_path)
        .into_diagnostic()
        .wrap_err("couldn't load root workspace Cargo.toml")?;
    let mut workspace_toml_str = String::new();
    workspace_toml_file
        .read_to_string(&mut workspace_toml_str)
        .into_diagnostic()
        .wrap_err("couldn't read root workspace Cargo.toml")?;
    workspace_toml_str
        .parse::<toml_edit::Document>()
        .into_diagnostic()
        .wrap_err("couldn't parse root workspace Cargo.toml")
}

fn get_dist_profile(manifest_path: &Utf8Path) -> Result<CargoProfile> {
    let workspace_toml = load_root_cargo_toml(manifest_path)?;
    let dist_profile = &workspace_toml
        .get("profile")
        .and_then(|p| p.get(PROFILE_DIST));

    let Some(dist_profile) = dist_profile else {
        return Err(miette!("[profile.dist] didn't exist"));
    };

    // Get the fields we care about
    let debug = &dist_profile["debug"];
    let split_debuginfo = &dist_profile["split-debuginfo"];

    // clean up the true/false sugar for "debug"
    let debug = if let Some(val) = debug.as_bool() {
        if val {
            Some(2)
        } else {
            Some(0)
        }
    } else {
        debug.as_integer()
    };

    // Just capture split-debuginfo directly
    let split_debuginfo = split_debuginfo.as_str().map(ToOwned::to_owned);

    Ok(CargoProfile {
        debug,
        split_debuginfo,
    })
}

fn target_symbol_kind(target: &str) -> Option<SymbolKind> {
    #[allow(clippy::if_same_then_else)]
    if target.contains("windows-msvc") {
        Some(SymbolKind::Pdb)
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

/// Load a changelog to a string
fn try_load_changelog(changelog_path: &Utf8Path) -> Result<String> {
    let file = File::open(changelog_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to open changelog at {changelog_path}"))?;
    let mut data = BufReader::new(file);
    let mut changelog_str = String::new();
    data.read_to_string(&mut changelog_str)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to read changelog at {changelog_path}"))?;
    Ok(changelog_str)
}
