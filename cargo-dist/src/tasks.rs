//! Code to compute the tasks cargo-dist should do
//!
//! This is the heart and soul of cargo-dist, and ideally the [`gather_work`][] function
//! should compute every minute detail dist will perform ahead of time. This is done with
//! the DistGraphBuilder, which roughly builds up the work to do as follows:
//!
//! 1. [`get_project`][]: find out everything we want to know about the workspace (binaries, configs, etc)
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

use axoproject::{PackageIdx, WorkspaceInfo, WorkspaceSearch};
use camino::{Utf8Path, Utf8PathBuf};
use guppy::PackageId;
use miette::{miette, Context, IntoDiagnostic, Report};
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
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct DistMetadata {
    /// The intended version of cargo-dist to build with. (normal Cargo SemVer syntax)
    ///
    /// When generating full tasks graphs (such as CI scripts) we will pick this version.
    ///
    /// FIXME: Should we produce a warning if running locally with a different version? In theory
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
    /// FIXME: maybe you should also be allowed to make this a list of binary names..?
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
    /// The value is an Option to allow individual packages to override the workspace setting
    /// if None (no key in the config), the workspace value is inherited. `installers = []`
    /// instead discards all the workspace values.
    ///
    /// Currently accepted values:
    ///
    /// * shell
    /// * powershell
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
    /// FIXME: We should also accept one magic target: `universal2-apple-darwin`. This will induce
    /// us to build `x86_64-apple-darwin` and `aarch64-apple-darwin` (arm64) and then combine
    /// them into a "universal" binary that can run on either arch (using apple's `lipo` tool).
    ///
    /// FIXME: Allow higher level requests like "[macos, windows, linux] x [x86_64, aarch64]"?
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

    /// The archive format to use for windows builds (defaults .zip)
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "windows-archive")]
    pub windows_archive: Option<ZipStyle>,

    /// The archive format to use for non-windows builds (defaults .tar.xz)
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "unix-archive")]
    pub unix_archive: Option<ZipStyle>,

    /// A scope to prefix npm packages with (@ should be included).
    ///
    /// This is required if you're using an npm installer.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "npm-scope")]
    pub npm_scope: Option<String>,
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
        if self.windows_archive.is_none() {
            self.windows_archive = workspace_config.windows_archive;
        }
        if self.unix_archive.is_none() {
            self.unix_archive = workspace_config.unix_archive;
        }
        if self.npm_scope.is_none() {
            self.npm_scope = workspace_config.npm_scope.clone();
        }
        self.include
            .extend(workspace_config.include.iter().cloned());
    }
}

/// Global config for commands
#[derive(Debug)]
pub struct Config {
    /// Whether we need to compute an announcement tag or if we can fudge it
    ///
    /// Commands like generate-ci and init don't need announcements, but want to run gather_work
    pub needs_coherent_announcement_tag: bool,
    /// The subset of artifacts we want to build
    pub artifact_mode: ArtifactMode,
    /// Whether local paths to files should be in the final dist json output
    pub no_local_paths: bool,
    /// Target triples we want to build for
    pub targets: Vec<TargetTriple>,
    /// CI kinds we want to support
    pub ci: Vec<CiStyle>,
    /// Installers we want to generate
    pub installers: Vec<InstallerStyle>,
    /// The (git) tag to use for this Announcement.
    pub announcement_tag: Option<String>,
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
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum CiStyle {
    /// Generate Github CI
    #[serde(rename = "github")]
    Github,
}

/// The style of Installer we should generate
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum InstallerStyle {
    /// Generate a shell script that fetches from [`DistGraph::artifact_download_url`][]
    #[serde(rename = "shell")]
    Shell,
    /// Generate a powershell script that fetches from [`DistGraph::artifact_download_url`][]
    #[serde(rename = "powershell")]
    Powershell,
    /// Generate an npm project that fetches from [`DistGraph::artifact_download_url`][]
    #[serde(rename = "npm")]
    Npm,
}

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

    // Some simple global facts
    /// The executable cargo told us to find itself at.
    pub cargo: String,
    /// The cargo target dir.
    pub target_dir: Utf8PathBuf,
    /// The root directory of the current cargo workspace.
    pub workspace_dir: Utf8PathBuf,
    /// cargo-dist's target dir (generally nested under `target_dir`).
    pub dist_dir: Utf8PathBuf,
    /// The desired cargo-dist version for handling this project
    pub desired_cargo_dist_version: Option<Version>,
    /// The desired rust toolchain for handling this project
    pub desired_rust_toolchain: Option<String>,
    /// The desired ci backends for this project
    pub tools: Tools,
    /// The target triple of the current system
    pub host_target: String,

    /// Styles of CI we want to support
    pub ci_style: Vec<CiStyle>,
    /// The git tag used for the announcement (e.g. v1.0.0)
    ///
    /// This is important for certain URLs like Github Releases
    pub announcement_tag: Option<String>,
    /// Whether the announcement appears to be a prerelease
    pub announcement_is_prerelease: bool,
    /// Title of the announcement
    pub announcement_title: Option<String>,
    /// Raw changelog for the announcement
    pub announcement_changelog: Option<String>,
    /// Github Releases body for the announcement
    pub announcement_github_body: Option<String>,
    /// Base URL that artifacts are downloadable from ("{artifact_download_url}/{artifact.id}")
    pub artifact_download_url: Option<String>,

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

/// Various tools we have found installed on the system
#[derive(Debug, Clone, Default)]
pub struct Tools {
    /// rustup, useful for getting specific toolchains
    pub rustup: Option<Tool>,
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
#[allow(clippy::large_enum_variant)]
pub enum BuildStep {
    /// Do a cargo build (and copy the outputs to various locations)
    Cargo(CargoBuildStep),
    /// Run rustup to get a toolchain
    Rustup(RustupStep),
    /// Copy a file
    CopyFile(CopyFileStep),
    /// Copy a dir
    CopyDir(CopyDirStep),
    /// Zip up a directory
    Zip(ZipDirStep),
    /// Generate some kind of installer
    GenerateInstaller(InstallerImpl),
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
}

/// Info about an archive (zip/tarball) that should be made. Currently this is always part
/// of an Artifact, and the final output will be [`Artifact::file_path`][].
#[derive(Debug)]
pub struct Archive {
    /// The name of the directory this artifact's contents will be stored in.
    ///
    /// This directory is technically a transient thing but it will show up as the name of
    /// the directory in a `tar`.
    ///
    /// i.e. `cargo-dist-v0.1.0-x86_64-pc-windows-msvc`
    pub dir_name: String,
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
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
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
    // everything important is already part of Artifact
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
    /// Static assets that should be included in bundles like executable-zips
    pub static_assets: Vec<(StaticAssetKind, Utf8PathBuf)>,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipStyle {
    /// `.zip`
    Zip,
    /// `.tar.<compression>`
    Tar(CompressionImpl),
}

/// Compression impls (used by [`ZipStyle::Tar`][])
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CompressionImpl {
    /// `.gz`
    Gzip,
    /// `.xz`
    Xzip,
    /// `.zstd`
    Zstd,
}
impl ZipStyle {
    /// Get the extension used for this kind of zip
    pub fn ext(&self) -> &'static str {
        match self {
            ZipStyle::Zip => ".zip",
            ZipStyle::Tar(compression) => match compression {
                CompressionImpl::Gzip => ".tar.gz",
                CompressionImpl::Xzip => ".tar.xz",
                CompressionImpl::Zstd => ".tar.zstd",
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
            ".tar.zstd" => Ok(ZipStyle::Tar(CompressionImpl::Zstd)),
            _ => Err(D::Error::custom(format!(
                "unknown archive format {ext}, expected one of: .zip, .tar.gz, .tar.xz, .tar.zstd"
            ))),
        }
    }
}

/// A kind of an installer
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum InstallerImpl {
    /// shell installer script
    Shell(InstallerInfo),
    /// powershell installer script
    Powershell(InstallerInfo),
    /// npm installer package
    Npm(NpmInstallerInfo),
}

/// Generic info about an installer
#[derive(Debug, Clone)]
pub struct InstallerInfo {
    /// The path to generate the installer at
    pub dest_path: Utf8PathBuf,
    /// App name to use (display only)
    pub app_name: String,
    /// App version to use (display only)
    pub app_version: String,
    /// URL of the directory where artifacts can be fetched from
    pub base_url: String,
    /// Artifacts this installer can fetch
    pub artifacts: Vec<ExecutableZipFragment>,
    /// Description of the installer (a good heading)
    pub desc: String,
    /// Hint for how to run the installer
    pub hint: String,
}

/// Info about an npm installer
#[derive(Debug, Clone)]
pub struct NpmInstallerInfo {
    /// The name of the npm package
    pub npm_package_name: String,
    /// The version of the npm package
    pub npm_package_version: String,
    /// Short description of the package
    pub npm_package_desc: Option<String>,
    /// URL to repository
    pub npm_package_repository_url: Option<String>,
    /// URL to homepage
    pub npm_package_homepage_url: Option<String>,
    /// Short description of the package
    pub npm_package_authors: Vec<String>,
    /// Short description of the package
    pub npm_package_license: Option<String>,
    /// Array of keywords for this package
    pub npm_package_keywords: Option<Vec<String>>,
    /// Name of the binary this package installs (without .exe extension)
    pub bin: String,
    /// Dir to build the package in
    pub package_dir: Utf8PathBuf,
    /// Generic installer info
    pub inner: InstallerInfo,
}

/// A fake fragment of an ExecutableZip artifact for installers
#[derive(Debug, Clone)]
pub struct ExecutableZipFragment {
    /// The id of the artifact
    pub id: String,
    /// The targets the artifact supports
    pub target_triples: Vec<TargetTriple>,
    /// The binaries the artifact contains (name, assumed at root)
    pub binaries: Vec<String>,
    /// The style of zip this is
    pub zip_style: ZipStyle,
}

/// Cargo features a cargo build should use.
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

struct DistGraphBuilder<'pkg_graph> {
    inner: DistGraph,
    workspace: &'pkg_graph WorkspaceInfo,
    artifact_mode: ArtifactMode,
    binaries_by_id: FastMap<String, BinaryIdx>,
    workspace_metadata: DistMetadata,
    package_metadata: Vec<DistMetadata>,
}

impl<'pkg_graph> DistGraphBuilder<'pkg_graph> {
    fn new(
        cargo: String,
        workspace: &'pkg_graph WorkspaceInfo,
        artifact_mode: ArtifactMode,
        host_target: String,
    ) -> Self {
        let target_dir = workspace.target_dir.clone();
        let workspace_dir = workspace.workspace_dir.clone();
        let dist_dir = target_dir.join(TARGET_DIST);

        let dist_profile = workspace.cargo_profiles.get(PROFILE_DIST);
        let mut workspace_metadata = parse_metadata_table(workspace.cargo_metadata_table.as_ref());
        workspace_metadata.make_relative_to(&workspace.workspace_dir);
        let desired_cargo_dist_version = workspace_metadata.cargo_dist_version.clone();
        let desired_rust_toolchain = workspace_metadata.rust_toolchain_version.clone();

        let mut package_metadata = vec![];
        for package in &workspace.package_info {
            let mut package_config = parse_metadata_table(package.cargo_metadata_table.as_ref());

            // Check for global settings on local packages
            if package_config.cargo_dist_version.is_some() {
                warn!("package.metadata.dist.cargo-dist-version is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package.manifest_path);
            }
            if package_config.cargo_dist_version.is_some() {
                warn!("package.metadata.dist.rust-toolchain-version is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package.manifest_path);
            }
            if !package_config.ci.is_empty() {
                warn!("package.metadata.dist.ci is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package.manifest_path);
            }

            package_config.make_relative_to(&package.package_root);
            package_config.merge_workspace_config(&workspace_metadata);
            package_metadata.push(package_config);
        }

        Self {
            inner: DistGraph {
                is_init: dist_profile.is_some(),
                cargo,
                target_dir,
                workspace_dir,
                dist_dir,
                desired_cargo_dist_version,
                desired_rust_toolchain,
                tools: Tools::default(),
                host_target,
                announcement_tag: None,
                announcement_is_prerelease: false,
                announcement_changelog: None,
                announcement_github_body: None,
                announcement_title: None,
                artifact_download_url: None,
                ci_style: vec![],
                build_steps: vec![],
                artifacts: vec![],
                binaries: vec![],
                variants: vec![],
                releases: vec![],
            },
            package_metadata,
            workspace_metadata,
            workspace,
            binaries_by_id: FastMap::new(),
            artifact_mode,
        }
    }

    fn package_metadata(&self, idx: PackageIdx) -> &DistMetadata {
        &self.package_metadata[idx.0]
    }

    fn set_ci_style(&mut self, style: Vec<CiStyle>) {
        self.inner.ci_style = style;
    }

    fn add_release(&mut self, pkg_idx: PackageIdx) -> ReleaseIdx {
        let package_info = self.workspace().package(pkg_idx);
        let package_config = self.package_metadata(pkg_idx);

        let version = package_info.version.as_ref().unwrap().cargo().clone();
        let app_name = package_info.name.clone();
        let app_desc = package_info.description.clone();
        let app_authors = package_info.authors.clone();
        let app_license = package_info.license.clone();
        let app_repository_url = package_info.repository_url.clone();
        let app_homepage_url = package_info.homepage_url.clone();
        let app_keywords = package_info.keywords.clone();

        let windows_archive = package_config.windows_archive.unwrap_or(ZipStyle::Zip);
        let unix_archive = package_config
            .unix_archive
            .unwrap_or(ZipStyle::Tar(CompressionImpl::Xzip));

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
        for static_asset in &package_config.include {
            static_assets.push((StaticAssetKind::Other, static_asset.clone()));
        }

        let idx = ReleaseIdx(self.inner.releases.len());
        let id = format!("{app_name}-v{version}");
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
            let version = package.version.as_ref().unwrap().cargo();
            let pkg_id = package.cargo_package_id.clone().unwrap();
            let id = format!("{binary_name}-v{version}-{target}");

            // If we already are building this binary we don't need to do it again!
            let idx = if let Some(&idx) = self.binaries_by_id.get(&id) {
                idx
            } else {
                info!("added binary {id}");
                let idx = BinaryIdx(self.inner.binaries.len());
                let binary = Binary {
                    id,
                    pkg_id,
                    name: binary_name,
                    target: target.clone(),
                    copy_exe_to: vec![],
                    copy_symbols_to: vec![],
                    symbols_artifact: None,
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

        // Create an executable-zip for each Variant
        let release = self.release(to_release);
        let variants = release.variants.clone();
        for variant_idx in variants {
            let (zip_artifact, built_assets) =
                self.make_executable_zip_for_variant(to_release, variant_idx);

            let zip_artifact_idx = self.add_local_artifact(variant_idx, zip_artifact);
            for (binary, dest_path) in built_assets {
                self.require_binary(zip_artifact_idx, variant_idx, binary, dest_path);
            }
        }
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
        let platform_exe_ext = if target_is_windows { ".exe" } else { "" };

        let artifact_dir_name = variant.id.clone();
        let artifact_dir_path = dist_dir.join(&artifact_dir_name);
        let artifact_ext = zip_style.ext();
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

        (
            Artifact {
                id: artifact_name,
                target_triples: vec![variant.target.clone()],
                file_path: artifact_path,
                required_binaries: FastMap::new(),
                archive: Some(Archive {
                    dir_name: artifact_dir_name,
                    dir_path: artifact_dir_path,
                    zip_style,
                    static_assets,
                }),
                kind: ArtifactKind::ExecutableZip(ExecutableZip {}),
            },
            built_assets,
        )
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
                    archive: None,
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
            InstallerStyle::Shell => self.add_shell_installer(to_release),
            InstallerStyle::Powershell => self.add_powershell_installer(to_release),
            InstallerStyle::Npm => self.add_npm_installer(to_release),
        }
    }

    fn add_shell_installer(&mut self, to_release: ReleaseIdx) {
        if !self.global_artifacts_enabled() {
            return;
        }
        let release = self.release(to_release);
        let release_id = &release.id;
        let Some(download_url) = &self.inner.artifact_download_url else {
            warn!("skipping shell installer: couldn't compute a URL to download artifacts from");
            return;
        };
        let artifact_name = format!("{release_id}-installer.sh");
        let artifact_path = self.inner.dist_dir.join(&artifact_name);
        let installer_url = format!("{download_url}/{artifact_name}");
        let hint = format!("# WARNING: this installer is experimental\ncurl --proto '=https' --tlsv1.2 -LsSf {installer_url} | sh");
        let desc = "Install prebuilt binaries via shell script".to_owned();

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
            warn!("skipping shell installer: not building any supported platforms (use --artifacts=global)");
            return;
        };

        let installer_artifact = Artifact {
            id: artifact_name,
            target_triples: target_triples.into_iter().collect(),
            archive: None,
            file_path: artifact_path.clone(),
            required_binaries: FastMap::new(),
            kind: ArtifactKind::Installer(InstallerImpl::Shell(InstallerInfo {
                dest_path: artifact_path,
                app_name: release.app_name.clone(),
                app_version: release.version.to_string(),
                base_url: download_url.clone(),
                artifacts,
                hint,
                desc,
            })),
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
        let Some(download_url) = &self.inner.artifact_download_url else {
            warn!("skipping powershell installer: couldn't compute a URL to download artifacts from");
            return;
        };
        let artifact_name = format!("{release_id}-installer.ps1");
        let artifact_path = self.inner.dist_dir.join(&artifact_name);
        let installer_url = format!("{download_url}/{artifact_name}");
        let hint = format!("# WARNING: this installer is experimental\nirm {installer_url} | iex");
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
            kind: ArtifactKind::Installer(InstallerImpl::Powershell(InstallerInfo {
                dest_path: artifact_path,
                app_name: release.app_name.clone(),
                app_version: release.version.to_string(),
                base_url: download_url.clone(),
                artifacts,
                hint,
                desc,
            })),
        };

        self.add_global_artifact(to_release, installer_artifact);
    }

    fn add_npm_installer(&mut self, to_release: ReleaseIdx) {
        if !self.global_artifacts_enabled() {
            return;
        }
        let release = self.release(to_release);
        let release_id = &release.id;
        let Some(download_url) = &self.inner.artifact_download_url else {
            warn!("skipping npm installer: couldn't compute a URL to download artifacts from");
            return;
        };

        if release.bins.len() > 1 {
            warn!("skipping npm installer: packages with multiple binaries are unsupported\n  let us know if you have a use for this, and what should happen!");
            return;
        }
        let bin = release.bins[0].1.clone();

        let npm_package_name = if let Some(scope) = &self.workspace_metadata.npm_scope {
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

            artifacts.push(ExecutableZipFragment {
                id: artifact.id,
                target_triples: artifact.target_triples,
                zip_style: variant_zip_style,
                binaries: binaries
                    .into_iter()
                    .map(|(_, dest_path)| dest_path.file_name().unwrap().to_owned())
                    .collect(),
            });
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
                dir_name,
                dir_path: dir_path.clone(),
                zip_style,
                static_assets,
            }),
            file_path: artifact_path.clone(),
            required_binaries: FastMap::new(),
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
                    base_url: download_url.clone(),
                    artifacts,
                    hint,
                    desc,
                },
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
        build_steps.extend(cargo_builds.into_iter());

        for artifact in &self.inner.artifacts {
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
            }

            if let Some(archive) = &artifact.archive {
                let artifact_dir = &archive.dir_path;
                // Copy all the static assets
                for (_, src_path) in &archive.static_assets {
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
                    zip_style: archive.zip_style,
                }));
            }
        }

        self.inner.build_steps = build_steps;
    }

    fn compute_cargo_builds(&mut self) -> Vec<BuildStep> {
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

            // FIXME: is there a more principled way for us to add things to RUSTFLAGS
            // without breaking everything. Cargo has some builtin ways like keys
            // in [target...] tables that will get "merged" with the flags it wants
            // to set. More blunt approaches like actually setting the environment
            // variable I think can result in overwriting flags other places set
            // (which is defensible, having spaghetti flags randomly injected by
            // a dozen different tools is a build maintenance nightmare!)

            // You're *supposed* to link libc statically on windows but Rust has a bad default.
            // See: https://rust-lang.github.io/rfcs/1721-crt-static.html
            if target.contains("windows-msvc") {
                rustflags.push_str(" -Ctarget-feature=+crt-static");
            }

            // If we're trying to cross-compile on macOS, ensure the rustup toolchain
            // is setup!
            if target.ends_with("apple-darwin")
                && self.inner.host_target.ends_with("apple-darwin")
                && target != self.inner.host_target
            {
                if let Some(rustup) = self.inner.tools.rustup.clone() {
                    builds.push(BuildStep::Rustup(RustupStep {
                        rustup,
                        target: target.clone(),
                    }));
                } else {
                    warn!("You're trying to cross-compile on macOS, but I can't find rustup to ensure you have the rust toolchains for it!")
                }
            }
            builds.push(BuildStep::Cargo(CargoBuildStep {
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
            }));
        }
        builds
    }

    fn compute_announcement_info(&mut self, announcing_version: Option<&Version>) {
        // Default to using the tag as a title
        self.inner.announcement_title = self.inner.announcement_tag.clone();

        self.compute_announcement_changelog(announcing_version);
        self.compute_announcement_github();
    }

    /// Try to compute changelogs for the announcement
    fn compute_announcement_changelog(&mut self, announcing_version: Option<&Version>) {
        // FIXME: currently this only supports a top-level changelog, it would be nice
        // to allow individual apps to have individual streams

        // FIXME: derive changelogs from semantic commits?

        // Try to find the version we're announcing in the top level CHANGELOG/RELEASES
        let Some(announcing_version) = announcing_version else {
            info!("not announcing a consistent version, skipping changelog generation");
            return;
        };

        // Load and parse the changelog
        let Some(changelog_path) = &self.workspace.root_auto_includes.changelog else {
            info!("no root changelog found, skipping changelog generation");
            return;
        };
        let Ok(changelog_str) = try_load_changelog(changelog_path) else {
            info!("failed to load changelog, skipping changelog generation");
            return;
        };
        let changelogs = parse_changelog::parse(&changelog_str)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to parse changelog at {changelog_path}"));
        let changelogs = match changelogs {
            Ok(changelogs) => changelogs,
            Err(e) => {
                info!(
                    "failed to parse changelog, skipping changelog generation\n{:?}",
                    e
                );
                return;
            }
        };

        // Try to extract the changelog entry for the announcing version.
        // First, try to find the exact version. If that fails, try to find this version without
        // the prerelease suffix.
        // Because releasing a prerelease for a version that was already published before does not
        // make much sense it is fairly safe to assume that this entry is in fact just our WIP state
        // of the release notes.
        let Some((title, notes)) =
            try_extract_changelog_exact(&changelogs, announcing_version)
                .or_else(|| try_extract_changelog_normalized(&changelogs, announcing_version))
            else {
                info!("failed to find {announcing_version} in changelogs, skipping changelog generation");
                return;
            };

        info!("succesfully parsed changelog!");
        self.inner.announcement_title = Some(title);
        self.inner.announcement_changelog = Some(notes);
    }

    /// If we're publishing to Github, generate some Github notes
    fn compute_announcement_github(&mut self) {
        use std::fmt::Write;

        if !self.inner.ci_style.contains(&CiStyle::Github) {
            info!("not publishing to Github, skipping Github Release Notes");
            return;
        }

        let mut gh_body = String::new();
        let download_url = self.inner.artifact_download_url.as_ref();

        // add release notes
        if let Some(changelog) = self.inner.announcement_changelog.as_ref() {
            gh_body.push_str("## Release Notes\n\n");
            gh_body.push_str(changelog);
            gh_body.push_str("\n\n");
        }

        // Add the contents of each Release to the body
        for release in &self.inner.releases {
            let heading_suffix = format!("{} {}", release.app_name, release.version);

            // Delineate releases if there's more than 1
            if self.inner.releases.len() > 1 {
                writeln!(gh_body, "# {heading_suffix}\n").unwrap();
            }

            // Sort out all the artifacts in this Release
            let mut global_installers = vec![];
            let mut local_installers = vec![];
            let mut bundles = vec![];
            let mut symbols = vec![];

            for &artifact_idx in &release.global_artifacts {
                let artifact = self.artifact(artifact_idx);
                match &artifact.kind {
                    ArtifactKind::ExecutableZip(zip) => bundles.push((artifact, zip)),
                    ArtifactKind::Symbols(syms) => symbols.push((artifact, syms)),
                    ArtifactKind::Installer(installer) => {
                        global_installers.push((artifact, installer))
                    }
                }
            }

            for &variant_idx in &release.variants {
                let variant = self.variant(variant_idx);
                for &artifact_idx in &variant.local_artifacts {
                    let artifact = self.artifact(artifact_idx);
                    match &artifact.kind {
                        ArtifactKind::ExecutableZip(zip) => bundles.push((artifact, zip)),
                        ArtifactKind::Symbols(syms) => symbols.push((artifact, syms)),
                        ArtifactKind::Installer(installer) => {
                            local_installers.push((artifact, installer))
                        }
                    }
                }
            }

            if !global_installers.is_empty() {
                writeln!(gh_body, "## Install {heading_suffix}\n").unwrap();
                for (_installer, details) in global_installers {
                    let (InstallerImpl::Shell(info)
                    | InstallerImpl::Powershell(info)
                    | InstallerImpl::Npm(NpmInstallerInfo { inner: info, .. })) = details;

                    writeln!(&mut gh_body, "### {}\n", info.desc).unwrap();
                    writeln!(&mut gh_body, "```shell\n{}\n```\n", info.hint).unwrap();
                }
            }

            let other_artifacts: Vec<_> = bundles
                .iter()
                .map(|i| i.0)
                .chain(local_installers.iter().map(|i| i.0))
                .chain(symbols.iter().map(|i| i.0))
                .collect();
            if !other_artifacts.is_empty() && download_url.is_some() {
                let download_url = download_url.as_ref().unwrap();
                writeln!(gh_body, "## Download {heading_suffix}\n",).unwrap();
                gh_body.push_str("| target | kind | download |\n");
                gh_body.push_str("|--------|------|----------|\n");

                for artifact in other_artifacts {
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
                        ArtifactKind::ExecutableZip(_) => "tarball",
                        ArtifactKind::Symbols(_) => "symbols",
                        ArtifactKind::Installer(_) => "installer",
                    };
                    let name = &artifact.id;
                    let artifact_download_url = format!("{download_url}/{name}");
                    let download = format!("[{name}]({artifact_download_url})");
                    writeln!(&mut gh_body, "| {targets} | {kind} | {download} |").unwrap();
                }
                writeln!(&mut gh_body).unwrap();
            }
        }

        info!("succesfully generated github release body!");
        // self.inner.artifact_download_url = Some(download_url);
        self.inner.announcement_github_body = Some(gh_body);
    }

    fn workspace(&self) -> &'pkg_graph WorkspaceInfo {
        self.workspace
    }
    fn binary(&self, idx: BinaryIdx) -> &Binary {
        &self.inner.binaries[idx.0]
    }
    fn binary_mut(&mut self, idx: BinaryIdx) -> &mut Binary {
        &mut self.inner.binaries[idx.0]
    }
    fn artifact(&self, idx: ArtifactIdx) -> &Artifact {
        &self.inner.artifacts[idx.0]
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
    let workspace = crate::get_project()?;
    let host_target = get_host_target(&cargo)?;
    let mut graph = DistGraphBuilder::new(cargo, &workspace, cfg.artifact_mode, host_target);
    graph.inner.tools = tool_info();

    // First thing's first: if they gave us an announcement tag then we should try to parse it
    let mut announcing_package = None;
    let mut announcing_version = None;
    let mut announcing_prerelease = false;
    let mut announcement_tag = cfg.announcement_tag.clone();
    if let Some(tag) = &announcement_tag {
        // First check if it matches any package
        for (pkg_id, package) in workspace.packages() {
            let package_version = package.version.as_ref().unwrap().cargo();
            let package_tag = format!("{}-v{}", package.name, package_version);
            if &package_tag == tag {
                info!(
                    "announcement tag matched {}@{}",
                    package.name, package_version
                );
                assert!(
                    announcing_package.is_none(),
                    "how on earth do you have two packages that match {package_tag}!?"
                );
                announcing_prerelease = !package_version.pre.is_empty();
                announcing_package = Some(pkg_id);
            }
        }

        // If it doesn't match any package then try to parse it as v{VERSION}
        if announcing_package.is_none() {
            if let Some(version) = tag
                .strip_prefix('v')
                .and_then(|v| v.parse::<Version>().ok())
            {
                announcing_prerelease = !version.pre.is_empty();
                announcing_version = Some(version);
            }
        }

        // If none of the approaches work, refuse to proceed
        if announcing_package.is_none() && announcing_version.is_none() {
            return Err(miette!(
                "The provided announcement tag ({tag}) didn't match any Package or Version"
            ));
        }
    }

    // Prefer the CLI (cfg) if it's non-empty, but only select a subset
    // of what the workspace supports if it's non-empty
    if cfg.ci.is_empty() {
        graph.set_ci_style(graph.workspace_metadata.ci.clone());
    } else {
        let cfg_ci = SortedSet::from_iter(cfg.ci.clone());
        let workspace_ci = SortedSet::from_iter(graph.workspace_metadata.ci.clone());
        let shared_ci = cfg_ci.intersection(&workspace_ci).cloned().collect();
        graph.set_ci_style(shared_ci);
    }

    // If no targets were specified, just use the host target
    let host_target_triple = [graph.inner.host_target.clone()];
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

    // Choose which binaries we want to release
    let disabled_sty = console::Style::new().dim();
    let enabled_sty = console::Style::new();
    let mut rust_releases = vec![];
    for (pkg_id, pkg) in workspace.packages() {
        let pkg_name = &pkg.name;

        // Determine if this package's binaries should be Released
        let disabled_reason = check_dist_package(
            &graph,
            pkg_id,
            pkg,
            announcement_tag.as_deref(),
            announcing_package,
            announcing_version.as_ref(),
        );

        // Report our conclusion/discoveries
        let sty;
        if let Some(reason) = &disabled_reason {
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
            rust_releases.push((pkg_id, rust_binaries));
        }
    }
    eprintln!();

    // Don't proceed if this doesn't make sense
    if rust_releases.is_empty() {
        if announcing_package.is_some() {
            warn!("You're trying to explicitly Release a library, only minimal functionality will work");
        } else {
            return Err(miette!(
                "This workspace doesn't have anything for cargo-dist to Release!"
            ));
        }
    }
    // If we don't have a tag yet we MUST successfully select one here or fail
    if announcement_tag.is_none() {
        let mut versions = SortedMap::<&Version, Vec<PackageIdx>>::new();
        for (pkg_idx, _) in &rust_releases {
            let info = graph.workspace().package(*pkg_idx);
            let version = info.version.as_ref().unwrap().cargo();
            versions.entry(version).or_default().push(*pkg_idx);
        }
        if versions.len() == 1 {
            let version = *versions.first_key_value().unwrap().0;
            let tag = format!("v{version}");
            info!("inferred Announcement tag: {}", tag);
            announcement_tag = Some(tag);
            announcing_prerelease = !version.pre.is_empty();
            announcing_version = Some(version.clone());
        } else if cfg.needs_coherent_announcement_tag {
            use std::fmt::Write;
            let mut msg = String::new();
            msg.push_str(
                "There are too many unrelated apps in your workspace to coherently Announce!\n\n",
            );
            msg.push_str("Please either specify --tag, or give them all the same version\n\n");
            msg.push_str("Here are some options:\n\n");
            for (version, packages) in &versions {
                write!(msg, "--tag=v{version} will Announce: ").unwrap();
                let mut multi_package = false;
                for &pkg_id in packages {
                    let info = &graph.workspace().package(pkg_id);
                    if multi_package {
                        write!(msg, ", ").unwrap();
                    } else {
                        multi_package = true;
                    }
                    write!(msg, "{}", info.name).unwrap();
                }
                writeln!(msg).unwrap();
            }
            msg.push('\n');
            let some_pkg = *versions.first_key_value().unwrap().1.first().unwrap();
            let info = &graph.workspace().package(some_pkg);
            let some_tag = format!(
                "--tag={}-v{}",
                info.name,
                info.version.as_ref().unwrap().cargo()
            );
            writeln!(
                msg,
                "you can also request any single package with {some_tag}"
            )
            .unwrap();
            return Err(miette!("{}", msg));
        } else {
            // We don't need a coherent announcement tag so use a fake one to continue on
            announcement_tag = Some("v1.0.0-FAKEVER".to_owned());
            announcing_prerelease = true;
            announcing_version = Some("1.0.0-FAKEVER".parse().unwrap());
        }
    }
    assert!(
        announcement_tag.is_some(),
        "integrity error: failed to select announcement tag"
    );
    graph.inner.announcement_tag = announcement_tag;
    graph.inner.announcement_is_prerelease = announcing_prerelease;
    if let Some(repo_url) = workspace.repository_url.as_ref() {
        let tag = graph.inner.announcement_tag.as_ref().unwrap();
        graph.inner.artifact_download_url = Some(format!("{repo_url}/releases/download/{tag}"));
    }

    // Create a Release for each package
    for (pkg_idx, binaries) in &rust_releases {
        // FIXME: this clone is hacky but I'm in the middle of a nasty refactor
        let package_config = graph.package_metadata(*pkg_idx).clone();

        // Create a Release for this binary
        let release = graph.add_release(*pkg_idx);

        // Tell the Release to include these binaries
        for binary in binaries {
            graph.add_binary(release, *pkg_idx, (*binary).clone());
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
            graph.add_variant(release, target.clone());
        }
        // Add executable zips to the Release
        graph.add_executable_zip(release);

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
            graph.add_installer(release, installer);
        }
    }

    // Prep the announcement's release notes and whatnot
    graph.compute_announcement_info(announcing_version.as_ref());

    // Finally compute all the build steps!
    graph.compute_build_steps();

    Ok(graph.inner)
}

/// See if we should dist this package
///
/// Some(disabled_reason) is returned if it shouldn't be
fn check_dist_package(
    graph: &DistGraphBuilder,
    pkg_id: PackageIdx,
    pkg: &axoproject::PackageInfo,
    announcement_tag: Option<&str>,
    announcing_package: Option<PackageIdx>,
    announcing_version: Option<&Version>,
) -> Option<String> {
    // Nothing to publish if there's no binaries!
    if pkg.binaries.is_empty() {
        return Some("no binaries".to_owned());
    }

    // If [metadata.dist].dist is explicitly set, respect it!
    let override_publish = if let Some(do_dist) = graph.package_metadata(pkg_id).dist {
        if !do_dist {
            return Some("dist = false".to_owned());
        } else {
            true
        }
    } else {
        false
    };

    // Otherwise defer to Cargo's `publish = false`
    if !pkg.publish && !override_publish {
        return Some("publish = false".to_owned());
    }

    // If we're announcing a package, reject every other package
    if let Some(id) = announcing_package {
        if pkg_id != id {
            return Some(format!(
                "didn't match tag {}",
                announcement_tag.as_ref().unwrap()
            ));
        }
    }

    // If we're announcing a version, ignore everything that doesn't match that
    if let Some(ver) = &announcing_version {
        if pkg.version.as_ref().unwrap().cargo() != *ver {
            return Some(format!(
                "didn't match tag {}",
                announcement_tag.as_ref().unwrap()
            ));
        }
    }

    // If it passes the guantlet, dist it
    None
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

/// Tries to find a changelog entry with the exact version given and returns its title and notes.
fn try_extract_changelog_exact(
    changelogs: &parse_changelog::Changelog,
    version: &Version,
) -> Option<(String, String)> {
    let version_string = format!("{}", version);

    changelogs.get(&*version_string).map(|release_notes| {
        (
            release_notes.title.to_string(),
            release_notes.notes.to_string(),
        )
    })
}

/// Tries to find a changelog entry that matches the given version's normalized form. That is, just
/// the `major.minor.patch` part. If successful, the entry's title is modified to include the
/// version's prerelease part before it is returned together with the notes.
///
/// Noop if the given version is already normalized.
fn try_extract_changelog_normalized(
    changelogs: &parse_changelog::Changelog,
    version: &Version,
) -> Option<(String, String)> {
    if version.pre.is_empty() {
        return None;
    }

    let version_normalized = Version::new(version.major, version.minor, version.patch);
    let version_normalized_string = format!("{}", version_normalized);

    let release_notes = changelogs.get(&*version_normalized_string)?;

    // title looks something like '<prefix><version><freeform>'
    // prefix could be 'v' or 'Version ' for example
    let (prefix_and_version, freeform) = release_notes.title.split_at(
        release_notes
            .title
            .find(&*version_normalized_string)
            .unwrap() // impossible that this version string is not present in the header
            + version_normalized_string.len(),
    );

    // insert prerelease suffix into the title
    let title = format!(
        "{}-{} {}",
        prefix_and_version.trim(),
        version.pre,
        freeform.trim()
    );

    Some((title.trim().to_string(), release_notes.notes.to_string()))
}

fn tool_info() -> Tools {
    Tools {
        rustup: find_tool("rustup"),
    }
}

fn find_tool(name: &str) -> Option<Tool> {
    let output = Command::new(name).arg("-V").output().ok()?;
    let string_output = String::from_utf8(output.stdout).ok()?;
    let version = string_output.lines().next()?;
    Some(Tool {
        cmd: name.to_owned(),
        version: version.to_owned(),
    })
}

pub(crate) fn parse_metadata_table(metadata_table: Option<&serde_json::Value>) -> DistMetadata {
    metadata_table
        .and_then(|t| t.get(METADATA_DIST))
        .map(DistMetadata::deserialize)
        .transpose()
        .ok()
        .unwrap_or_default()
        .unwrap_or_default()
}

/// Get the general info about the project (via axo-project)
pub fn get_project() -> Result<axoproject::WorkspaceInfo> {
    let start_dir = std::env::current_dir().expect("couldn't get current working dir!?");
    let start_dir = Utf8PathBuf::from_path_buf(start_dir).expect("project path isn't utf8!?");
    match axoproject::rust::get_workspace(None, &start_dir) {
        WorkspaceSearch::Found(mut workspace) => {
            // This is a goofy as heck workaround for two facts:
            //   * the convenient Report approach requires us to provide an Error by-val
            //   * many error types (like std::io::Error) don't impl Clone, so we can't
            //     clone axoproject Errors.
            //
            // So we temporarily take ownership of the warnings and then pull them back
            // out of the Report with runtime reflection to put them back in :)
            let mut warnings = std::mem::take(&mut workspace.warnings);
            for warning in warnings.drain(..) {
                let report = Report::new(warning);
                warn!("{:?}", report);
                workspace.warnings.push(report.downcast().unwrap());
            }
            Ok(workspace)
        }
        WorkspaceSearch::Missing(e) => Err(Report::new(e).wrap_err("no cargo workspace found")),
        WorkspaceSearch::Broken(e) => {
            Err(Report::new(e).wrap_err("your cargo workspace has an issue"))
        }
    }
}
