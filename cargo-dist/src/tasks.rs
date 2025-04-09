//! Code to compute the tasks dist should do
//!
//! This is the heart and soul of dist, and ideally the [`gather_work`][] function
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

use std::collections::BTreeMap;

use crate::backend::installer::{ExecutableZipFragment, HomebrewImpl};
use crate::platform::targets::{
    TARGET_ARM64_LINUX_GNU, TARGET_ARM64_MAC, TARGET_X64_LINUX_GNU, TARGET_X64_MAC,
};
use axoasset::AxoClient;
use axoprocess::Cmd;
use axoproject::{PackageId, PackageIdx, WorkspaceGraph};
use camino::{Utf8Path, Utf8PathBuf};
use cargo_dist_schema::target_lexicon::{OperatingSystem, Triple};
use cargo_dist_schema::{
    ArtifactId, BuildEnvironment, DistManifest, HomebrewPackageName, SystemId, SystemInfo,
    TripleName, TripleNameRef,
};
use semver::Version;
use serde::Serialize;
use tracing::{info, warn};

use crate::announce::{self, AnnouncementTag, TagMode};
use crate::backend::ci::github::GithubCiInfo;
use crate::backend::ci::CiInfo;
use crate::backend::installer::homebrew::{to_homebrew_license_format, HomebrewFragments};
use crate::backend::installer::macpkg::PkgInstallerInfo;
use crate::config::v1::builds::cargo::AppCargoBuildConfig;
use crate::config::v1::ci::CiConfig;
use crate::config::v1::installers::CommonInstallerConfig;
use crate::config::v1::publishers::PublisherConfig;
use crate::config::v1::{app_config, workspace_config, AppConfig, WorkspaceConfig};
use crate::config::{DependencyKind, DirtyMode, LibraryStyle};
use crate::linkage::determine_build_environment;
use crate::net::ClientSettings;
use crate::platform::{PlatformSupport, RuntimeConditions};
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
        self, ArtifactMode, ChecksumStyle, CompressionImpl, Config, HostingStyle, InstallerStyle,
        ZipStyle,
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
    pub fn for_target(&self, target: &TripleNameRef) -> BTreeMap<String, Vec<String>> {
        if target.is_windows() {
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
        targets: &[TripleName],
    ) -> BTreeMap<TripleName, BTreeMap<String, Vec<String>>> {
        BTreeMap::from_iter(
            targets
                .iter()
                .map(|target| (target.to_owned(), self.for_target(target))),
        )
    }
}

/// The graph of all work that dist needs to do on this invocation.
///
/// All work is precomputed at the start of execution because only discovering
/// what you need to do in the middle of building/packing things is a mess.
/// It also allows us to report what *should* happen without actually doing it.
#[derive(Debug)]
pub struct DistGraph {
    /// Unique id for the system we're building on.
    ///
    /// Since the whole premise of dist is to invoke it once on each machine, and no
    /// two machines have any reason to have the exact same CLI args for dist, we
    /// just use a mangled form of the CLI arguments here.
    pub system_id: SystemId,
    /// Whether it looks like `dist init` has been run
    pub is_init: bool,
    /// What to allow to be dirty
    pub allow_dirty: DirtyMode,
    /// Homebrew tap all packages agree on
    pub global_homebrew_tap: Option<String>,
    /// builtin publish jobs all packages agree on
    pub global_publishers: Option<PublisherConfig>,
    /// Whether we can just build the workspace or need to build each package
    pub precise_cargo_builds: bool,

    /// Info about the tools we're using to build
    pub tools: Tools,
    /// Signing tools
    pub signer: Signing,
    /// Minijinja templates we might want to render
    pub templates: Templates,

    /// The cargo target dir.
    pub target_dir: Utf8PathBuf,
    /// The root directory of the current git repo
    pub repo_dir: Utf8PathBuf,
    /// The root directory of the current cargo workspace.
    pub workspace_dir: Utf8PathBuf,
    /// dist's target dir (generally nested under `target_dir`).
    pub dist_dir: Utf8PathBuf,
    /// misc workspace-global config
    pub config: WorkspaceConfig,
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
    /// List of hosting providers to use
    pub hosting: Option<HostingInfo>,
    /// LIES ALL LIES
    pub local_builds_are_lies: bool,
    /// HTTP client settings
    pub client_settings: ClientSettings,
    /// A reusable client for basic http fetches
    pub axoclient: AxoClient,
}

/// Info about artifacts should be hosted
#[derive(Debug, Clone)]
pub struct HostingInfo {
    /// Hosting backends
    pub hosts: Vec<HostingStyle>,
    /// The domain at which the repo is hosted, (e.g. `"https://github.com"`)
    pub domain: String,
    /// Path at the domain
    pub repo_path: String,
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
    /// Info on the host
    pub host_target: TripleName,
    /// Info on cargo
    pub cargo: Option<CargoInfo>,
    /// rustup, useful for getting specific toolchains
    pub rustup: Option<Tool>,
    /// homebrew, only available on macOS
    pub brew: Option<Tool>,
    /// git, used if the repository is a git repo
    pub git: Option<Tool>,
    /// omnibor, used for generating OmniBOR Artifact IDs
    pub omnibor: Option<Tool>,
    /// ssl.com's CodeSignTool, for Windows Code Signing
    ///
    /// <https://www.ssl.com/guide/esigner-codesigntool-command-guide/>
    pub code_sign_tool: Option<Tool>,
    /// cargo-auditable, used for auditable builds
    pub cargo_auditable: Option<Tool>,
    /// cargo-cyclonedx, for generating CycloneDX artifacts
    pub cargo_cyclonedx: Option<Tool>,
    /// cargo-xwin, for some cross builds
    pub cargo_xwin: Option<Tool>,
    /// cargo-zigbuild, for some cross builds
    pub cargo_zigbuild: Option<Tool>,
}

impl Tools {
    /// Returns the cargo info or an error
    pub fn cargo(&self) -> DistResult<&CargoInfo> {
        self.cargo.as_ref().ok_or(DistError::ToolMissing {
            tool: "cargo".to_owned(),
        })
    }

    /// Returns the omnibor info or an error
    pub fn omnibor(&self) -> DistResult<&Tool> {
        self.omnibor.as_ref().ok_or(DistError::ToolMissing {
            tool: "omnibor-cli".to_owned(),
        })
    }

    /// Returns cargo-auditable info or an error
    pub fn cargo_auditable(&self) -> DistResult<&Tool> {
        self.cargo_auditable.as_ref().ok_or(DistError::ToolMissing {
            tool: "cargo-auditable".to_owned(),
        })
    }

    /// Returns cargo-cyclonedx info or an error
    pub fn cargo_cyclonedx(&self) -> DistResult<&Tool> {
        self.cargo_cyclonedx.as_ref().ok_or(DistError::ToolMissing {
            tool: "cargo-cyclonedx".to_owned(),
        })
    }

    /// Returns cargo-xwin info or an error
    pub fn cargo_xwin(&self) -> DistResult<&Tool> {
        self.cargo_xwin.as_ref().ok_or(DistError::ToolMissing {
            tool: "cargo-xwin".to_owned(),
        })
    }

    /// Returns cargo-zigbuild info or an error
    pub fn cargo_zigbuild(&self) -> DistResult<&Tool> {
        self.cargo_zigbuild.as_ref().ok_or(DistError::ToolMissing {
            tool: "cargo-zigbuild".to_owned(),
        })
    }
}

/// Info about the cargo toolchain we're using
#[derive(Debug, Clone)]
pub struct CargoInfo {
    /// The path/command used to refer to cargo (usually from the CARGO env var)
    pub cmd: String,
    /// The first line of running cargo with `-vV`, should be version info
    pub version_line: Option<String>,
    /// The host target triple (obtained from `-vV`)
    pub host_target: TripleName,
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
    pub target: TripleName,
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
    /// What kind of binary this is
    pub kind: BinaryKind,
}

/// Different kinds of binaries dist knows about
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BinaryKind {
    /// Standard executable
    Executable,
    /// C-style dynamic library (.so/.dylib/.dll)
    DynamicLibrary,
    /// C-style static library (.a/.lib)
    StaticLibrary,
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
    /// Generate a unified checksum file, containing multiple entries
    UnifiedChecksum(UnifiedChecksumStep),
    /// Generate an OmniBOR Artifact ID
    OmniborArtifactId(OmniborArtifactIdImpl),
    /// Fetch or build an updater binary
    Updater(UpdaterStep),
    // FIXME: For macos universal builds we'll want
    // Lipo(LipoStep)
}

/// A cargo build (and copy the outputs to various locations)
#[derive(Debug)]
pub struct CargoBuildStep {
    /// The --target triple to pass
    pub target_triple: TripleName,
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

/// A wrapper to use instead of `cargo build`, generally used for cross-compilation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CargoBuildWrapper {
    /// Run 'cargo zigbuild' to cross-compile, e.g. from `x86_64-unknown-linux-gnu` to `aarch64-unknown-linux-gnu`
    /// cf. <https://github.com/rust-cross/cargo-zigbuild>
    ZigBuild,

    /// Run 'cargo xwin' to cross-compile, e.g. from `aarch64-apple-darwin` to `x86_64-pc-windows-msvc`
    /// cf. <https://github.com/rust-cross/cargo-xwin>
    Xwin,
}

impl std::fmt::Display for CargoBuildWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(match self {
            CargoBuildWrapper::ZigBuild => "cargo-zigbuild",
            CargoBuildWrapper::Xwin => "cargo-xwin",
        })
    }
}

/// Returns the cargo build wrapper required to perform a certain cross-compilation
pub fn build_wrapper_for_cross(
    host: &Triple,
    target: &Triple,
) -> DistResult<Option<CargoBuildWrapper>> {
    if host.operating_system == target.operating_system && host.architecture == target.architecture
    {
        // we're not cross-compiling, not really... maybe we're making a GNU binary from a "musl" host but meh.
        return Ok(None);
    }

    match target.operating_system {
        // compiling for macOS (making Mach-O binaries, .dylib files, etc.)
        OperatingSystem::Darwin => match host.operating_system {
            OperatingSystem::Darwin => {
                // from mac to mac, even if we do aarch64 => x86_64, or the other way
                // around, _all we need_ is to add the target to rustup
                Ok(None)
            }
            _ => {
                Err(DistError::UnsupportedCrossCompile {
                    host: host.clone(),
                    target: target.clone(),
                    details: "cross-compiling to macOS is a road paved with sadness — we cowardly refuse to walk it.".to_string(),
                })
            }
        },
        // compiling for Linux (making ELF binaries, .so files, etc.)
        OperatingSystem::Linux => match host.operating_system {
            OperatingSystem::Linux | OperatingSystem::Darwin | OperatingSystem::Windows => {
                // zigbuild works for e.g. x86_64-unknown-linux-gnu => aarch64-unknown-linux-gnu
                Ok(Some(CargoBuildWrapper::ZigBuild))
            }
            _ => {
                Err(DistError::UnsupportedCrossCompile {
                    host: host.clone(),
                    target: target.clone(),
                    details: format!("no idea how to cross-compile from {host} to linux"),
                })
            }
        },
        // compiling for Windows (making PE binaries, .dll files, etc.)
        OperatingSystem::Windows => match host.operating_system {
            OperatingSystem::Linux | OperatingSystem::Darwin => {
                // cargo-xwin is made for that
                Ok(Some(CargoBuildWrapper::Xwin))
            }
            _ => {
                Err(DistError::UnsupportedCrossCompile {
                    host: host.clone(),
                    target: target.clone(),
                    details: format!("no idea how to cross-compile from {host} to windows with architecture {}", target.architecture),
                })
            }
        },
        _ => {
            Err(DistError::UnsupportedCrossCompile {
                host: host.clone(),
                target: target.clone(),
                details: format!("no idea how to cross-compile from anything (including the current host, {host}) to {target}"),
            })
        }
    }
}

/// A cargo build (and copy the outputs to various locations)
#[derive(Debug)]
pub struct GenericBuildStep {
    /// The --target triple to pass
    pub target_triple: TripleName,
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
    pub target: TripleName,
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

/// Create a unified checksum file, containing sums for
/// all artifacts, save for the unified checksum itself,
/// of course.
///
/// The result is something like `sha256.sum` which can be
/// checked by common tools like `sha256sum -c`. Even though
/// the type system lets each checksum have a different style,
/// the setting is per-release so in practice they end up being
/// the same.
#[derive(Debug, Clone)]
pub struct UnifiedChecksumStep {
    /// the checksum style to use
    pub checksum: ChecksumStyle,

    /// record the unified checksum to this path
    pub dest_path: Utf8PathBuf,
}

/// Create a file containing the OmniBOR Artifact ID for a specific file.
#[derive(Debug, Clone)]
pub struct OmniborArtifactIdImpl {
    /// file to generate the Artifact ID for
    pub src_path: Utf8PathBuf,
    /// file to write the Artifact ID to
    pub dest_path: Utf8PathBuf,
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
    pub target_triple: TripleName,
    /// The file this should produce
    pub target_filename: Utf8PathBuf,
    /// Whether to use the latest release instead of a fixed version
    pub use_latest: bool,
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
    pub id: ArtifactId,
    /// The target platform
    ///
    /// i.e. `x86_64-pc-windows-msvc`
    pub target_triples: Vec<TripleName>,
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
    /// A unified checksum file, like `sha256.sum`
    UnifiedChecksum(UnifiedChecksumStep),
    /// A source tarball
    SourceTarball(SourceTarball),
    /// An extra artifact specified via config
    ExtraArtifact(ExtraArtifactImpl),
    /// An updater executable
    Updater(UpdaterImpl),
    /// An existing file representing a Software Bill Of Materials
    SBOM(SBOMImpl),
    /// An OmniBOR Artifact ID.
    OmniborArtifactId(OmniborArtifactIdImpl),
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
pub struct UpdaterImpl {
    /// Whether to use the latest or a specific known-good version
    pub use_latest: bool,
}

/// A file containing a Software Bill Of Materials
#[derive(Clone, Debug)]
pub struct SBOMImpl {}

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
    /// The package this release is based on
    pub pkg_idx: PackageIdx,
    /// The version of the app
    pub version: Version,
    /// The unique id of the release (e.g. "my-app-v1.0.0")
    pub id: String,
    /// misc app-specific config
    pub config: AppConfig,
    /// Targets this Release has artifacts for
    pub targets: Vec<TripleName>,
    /// Binaries that every variant should ostensibly provide
    ///
    /// The string is the name of the binary under that package (without .exe extension)
    pub bins: Vec<(PackageIdx, String)>,
    /// C dynamic libraries that every variant should ostensibly provide
    ///
    /// The string is the name of the library, without lib prefix, and without platform-specific suffix (.so, .dylib, .dll)
    /// Note: Windows won't include lib prefix in the final lib.
    pub cdylibs: Vec<(PackageIdx, String)>,
    /// C static libraries that every variant should ostensibly provide
    ///
    /// The string is the name of the library, without lib prefix, and without platform-specific suffix (.a, .lib)
    /// Note: Windows won't include lib prefix in the final lib.
    pub cstaticlibs: Vec<(PackageIdx, String)>,
    /// They might still be limited to some subset of the targets (e.g. powershell scripts are
    /// windows-only), but conceptually there's only "one" for the Release.
    pub global_artifacts: Vec<ArtifactIdx>,
    /// Variants of this Release (e.g. "the macos build") that can have "local" Artifacts.
    pub variants: Vec<ReleaseVariantIdx>,
    /// The body of the changelog for this release
    pub changelog_body: Option<String>,
    /// The title of the changelog for this release
    pub changelog_title: Option<String>,
    /// Static assets that should be included in bundles like archives
    pub static_assets: Vec<(StaticAssetKind, Utf8PathBuf)>,
    /// Computed support for platforms, gets iteratively refined over time, so check details
    /// as late as possible, if you can!
    pub platform_support: PlatformSupport,
}

/// A particular variant of a Release (e.g. "the macos build")
#[derive(Debug)]
pub struct ReleaseVariant {
    /// The target triple this variant is for
    pub target: TripleName,
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
    package_configs: Vec<AppConfig>,
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

        // Complain if someone still has [workspace.metadata.dist] in a dist-workspace.toml scenario
        if let Some(dist_manifest_path) = root_workspace.dist_manifest_path.as_deref() {
            for workspace_idx in workspaces.all_workspace_indices() {
                if workspace_idx == root_workspace_idx {
                    continue;
                }
                let workspace = workspaces.workspace(workspace_idx);
                config::reject_metadata_table(
                    &workspace.manifest_path,
                    dist_manifest_path,
                    workspace.cargo_metadata_table.as_ref(),
                )?;
            }
        }

        let target_dir = root_workspace.target_dir.clone();
        let workspace_dir = root_workspace.workspace_dir.clone();
        let repo_dir = if let Some(repo) = &workspaces.repo {
            repo.path.to_owned()
        } else {
            // Fallback if we're not building in a git repo
            workspace_dir.clone()
        };
        let dist_dir = target_dir.join(TARGET_DIST);

        let mut workspace_metadata =
            // Read the global config
            config::parse_metadata_table_or_manifest(
                &root_workspace.manifest_path,
                root_workspace.dist_manifest_path.as_deref(),
                root_workspace.cargo_metadata_table.as_ref(),
            )?;

        let workspace_layer = workspace_metadata.to_toml_layer(true);
        workspace_metadata.make_relative_to(&root_workspace.workspace_dir);

        let config = workspace_config(workspaces, workspace_layer.clone());

        if config.builds.cargo.rust_toolchain_version.is_some() {
            warn!("rust-toolchain-version is deprecated, use rust-toolchain.toml if you want pinned toolchains");
        }

        let local_builds_are_lies = artifact_mode == ArtifactMode::Lies;

        // Compute/merge package configs
        let mut package_metadatas = vec![];
        let mut package_configs = vec![];

        for (pkg_idx, package) in workspaces.all_packages() {
            let mut package_metadata = config::parse_metadata_table_or_manifest(
                &package.manifest_path,
                package.dist_manifest_path.as_deref(),
                package.cargo_metadata_table.as_ref(),
            )?;
            package_configs.push(app_config(
                workspaces,
                pkg_idx,
                workspace_layer.clone(),
                package_metadata.to_toml_layer(false),
            ));

            package_metadata.make_relative_to(&package.package_root);
            package_metadata.merge_workspace_config(&workspace_metadata, &package.manifest_path);
            package_metadata.validate_install_paths()?;
            package_metadatas.push(package_metadata);
        }

        // check cargo build settings for precise-builds
        let mut global_cargo_build_config = None::<AppCargoBuildConfig>;
        let mut packages_with_mismatched_features = vec![];
        for ((_idx, package), package_config) in workspaces.all_packages().zip(&package_configs) {
            if let Some(cargo_build_config) = &global_cargo_build_config {
                if package_config.builds.cargo.features != cargo_build_config.features
                    || package_config.builds.cargo.all_features != cargo_build_config.all_features
                    || package_config.builds.cargo.default_features
                        != cargo_build_config.default_features
                {
                    packages_with_mismatched_features.push(
                        package
                            .dist_manifest_path
                            .clone()
                            .unwrap_or(package.manifest_path.clone()),
                    );
                }
            } else {
                global_cargo_build_config = Some(package_config.builds.cargo.clone());
                // This package gets to be the archetype, so if there's a mismatch it will
                // always be implicated. So push it to the error list, and only say there's an
                // error if there's two entries in this at the end.
                packages_with_mismatched_features.push(
                    package
                        .dist_manifest_path
                        .clone()
                        .unwrap_or(package.manifest_path.clone()),
                );
            };
        }
        // Only do workspace builds if all the packages agree with the workspace feature settings
        let requires_precise = packages_with_mismatched_features.len() > 1;
        let precise_cargo_builds = if let Some(precise_builds) = config.builds.cargo.precise_builds
        {
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

        // check homebrew taps for global publish jobs
        // FIXME: when we add `dist publish` we can drop this,
        // as we can support granular publish settings
        let mut global_homebrew_tap = None;
        let mut packages_with_mismatched_taps = vec![];
        for ((_idx, package), package_config) in workspaces.all_packages().zip(&package_configs) {
            if let Some(homebrew) = &package_config.installers.homebrew {
                if let Some(new_tap) = &homebrew.tap {
                    if let Some(current_tap) = &global_homebrew_tap {
                        if current_tap != new_tap {
                            packages_with_mismatched_taps.push(
                                package
                                    .dist_manifest_path
                                    .clone()
                                    .unwrap_or(package.manifest_path.clone()),
                            );
                        }
                    } else {
                        // This package gets to be the archetype, so if there's a mismatch it will
                        // always be implicated. So push it to the error list, and only say there's an
                        // error if there's two entries in this at the end.
                        packages_with_mismatched_taps.push(
                            package
                                .dist_manifest_path
                                .clone()
                                .unwrap_or(package.manifest_path.clone()),
                        );
                        global_homebrew_tap = Some(new_tap.clone());
                    }
                }
            }
        }
        if packages_with_mismatched_taps.len() > 1 {
            return Err(DistError::MismatchedTaps {
                packages: packages_with_mismatched_taps,
            });
        }

        // check publish jobs for global publish jobs
        // FIXME: when we add `dist publish` we can drop this,
        // as we can support granular publish settings
        let mut global_publishers = None;
        let mut packages_with_mismatched_publishers = vec![];
        for ((_idx, package), package_config) in workspaces.all_packages().zip(&package_configs) {
            if let Some(cur_publishers) = &global_publishers {
                if cur_publishers != &package_config.publishers {
                    packages_with_mismatched_publishers.push(
                        package
                            .dist_manifest_path
                            .clone()
                            .unwrap_or(package.manifest_path.clone()),
                    );
                }
            } else {
                // This package gets to be the archetype, so if there's a mismatch it will
                // always be implicated. So push it to the error list, and only say there's an
                // error if there's two entries in this at the end.
                packages_with_mismatched_publishers.push(
                    package
                        .dist_manifest_path
                        .clone()
                        .unwrap_or(package.manifest_path.clone()),
                );
                global_publishers = Some(package_config.publishers.clone());
            }
        }
        if packages_with_mismatched_publishers.len() > 1 {
            return Err(DistError::MismatchedPublishers {
                packages: packages_with_mismatched_publishers,
            });
        }
        let global_publish_prereleases = global_publishers
            .as_ref()
            .map(|p| {
                // until we have `dist publish` we need to enforce everyone agreeing on `prereleases`
                let PublisherConfig { homebrew, npm } = p;
                let h_pre = homebrew.as_ref().map(|p| p.prereleases);
                let npm_pre = npm.as_ref().map(|p| p.prereleases);
                let choices = [h_pre, npm_pre];
                let mut global_choice = None;
                #[allow(clippy::manual_flatten)]
                for choice in choices {
                    if let Some(choice) = choice {
                        if let Some(cur_choice) = global_choice {
                            if cur_choice != choice {
                                return Err(DistError::MismatchedPrereleases);
                            }
                        } else {
                            global_choice = Some(choice);
                        }
                    }
                }
                Ok(global_choice.unwrap_or(false))
            })
            .transpose()?
            .unwrap_or(false);

        let templates = Templates::new()?;
        let allow_dirty = if allow_all_dirty {
            DirtyMode::AllowAll
        } else {
            DirtyMode::AllowList(config.allow_dirty.clone())
        };
        let cargo_version_line = tools.cargo.as_ref().and_then(|c| c.version_line.to_owned());
        let build_environment = if local_builds_are_lies {
            BuildEnvironment::Indeterminate
        } else {
            determine_build_environment(&tools.host_target)
        };

        let system = SystemInfo {
            id: system_id.clone(),
            cargo_version_line,
            build_environment,
        };
        let systems = SortedMap::from_iter([(system_id.clone(), system)]);

        let client_settings = ClientSettings::new();
        let axoclient = crate::net::create_axoasset_client(&client_settings)?;

        let signer = Signing::new(
            &axoclient,
            &tools.host_target,
            &dist_dir,
            config.builds.ssldotcom_windows_sign.clone(),
            config.builds.macos_sign,
        )?;
        let github_attestations = config
            .hosts
            .github
            .as_ref()
            .map(|g| g.attestations)
            .unwrap_or(false);
        let force_latest = config.hosts.force_latest;
        Ok(Self {
            inner: DistGraph {
                system_id,
                is_init: config.dist_version.is_some(),
                allow_dirty,
                global_homebrew_tap,
                global_publishers,
                precise_cargo_builds,
                target_dir,
                repo_dir,
                workspace_dir,
                dist_dir,
                config,
                signer,
                tools,
                local_builds_are_lies,
                templates,
                local_build_steps: vec![],
                global_build_steps: vec![],
                artifacts: vec![],
                binaries: vec![],
                variants: vec![],
                releases: vec![],
                ci: CiInfo::default(),
                hosting: None,
                client_settings,
                axoclient,
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
                publish_prereleases: global_publish_prereleases,
                force_latest,
                ci: None,
                linkage: vec![],
                upload_files: vec![],
                github_attestations,
            },
            package_configs,
            workspaces,
            binaries_by_id: FastMap::new(),
            artifact_mode,
        })
    }

    fn add_release(&mut self, pkg_idx: PackageIdx) -> ReleaseIdx {
        let package_info = self.workspaces.package(pkg_idx);
        let config = self.package_config(pkg_idx).clone();

        let version = package_info.version.as_ref().unwrap().semver().clone();
        let app_name = package_info.name.clone();
        let app_desc = package_info.description.clone();
        let app_authors = package_info.authors.clone();
        let app_license = package_info.license.clone();
        let app_repository_url = package_info.repository_url.clone();
        let app_homepage_url = package_info.homepage_url.clone();
        let app_keywords = package_info.keywords.clone();

        // Add static assets
        let mut static_assets = vec![];
        if config.artifacts.archives.auto_includes {
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
        for static_asset in &config.artifacts.archives.include {
            static_assets.push((StaticAssetKind::Other, static_asset.clone()));
        }

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
            pkg_idx,
            global_artifacts: vec![],
            bins: vec![],
            cdylibs: vec![],
            cstaticlibs: vec![],
            targets: vec![],
            variants: vec![],
            changelog_body: None,
            changelog_title: None,
            config,
            static_assets,
            platform_support,
        });
        idx
    }

    fn add_variant(
        &mut self,
        to_release: ReleaseIdx,
        target: TripleName,
    ) -> DistResult<ReleaseVariantIdx> {
        let idx = ReleaseVariantIdx(self.inner.variants.len());
        let Release {
            id: release_id,
            variants,
            targets,
            static_assets,
            bins,
            cdylibs,
            cstaticlibs,
            config,
            pkg_idx,
            ..
        } = self.release_mut(to_release);
        let static_assets = static_assets.clone();
        let variant_id = format!("{release_id}-{target}");
        info!("added variant {variant_id}");
        let binaries_map = &config.artifacts.archives.binaries;

        variants.push(idx);
        targets.push(target.clone());

        // Apply binary list overrides
        let mapped_bins = binaries_map
            .get(target.as_str())
            .or_else(|| binaries_map.get("*"));
        let mut packageables: Vec<(PackageIdx, String, BinaryKind)> =
            if let Some(mapped_bins) = mapped_bins {
                mapped_bins
                    .iter()
                    .map(|b| (*pkg_idx, b.to_string(), BinaryKind::Executable))
                    .collect()
            } else {
                bins.clone()
                    .into_iter()
                    .map(|(idx, b)| (idx, b, BinaryKind::Executable))
                    .collect()
            };

        // If we're not packaging libraries here, avoid chaining them
        // into the list we're iterating over
        if config
            .artifacts
            .archives
            .package_libraries
            .contains(&LibraryStyle::CDynamic)
        {
            let all_dylibs = cdylibs
                .clone()
                .into_iter()
                .map(|(idx, l)| (idx, l, BinaryKind::DynamicLibrary));
            packageables = packageables.into_iter().chain(all_dylibs).collect();
        }
        if config
            .artifacts
            .archives
            .package_libraries
            .contains(&LibraryStyle::CStatic)
        {
            let all_cstaticlibs = cstaticlibs
                .clone()
                .into_iter()
                .map(|(idx, l)| (idx, l, BinaryKind::StaticLibrary));
            packageables = packageables.into_iter().chain(all_cstaticlibs).collect();
        }

        // Add all the binaries of the release to this variant
        let mut binaries = vec![];
        for (pkg_idx, binary_name, kind) in packageables {
            let package = self.workspaces.package(pkg_idx);
            let package_config = self.package_config(pkg_idx);
            let pkg_id = package.cargo_package_id.clone();
            // For now we just use the name of the package as its package_spec.
            // I'm not sure if there are situations where this is ambiguous when
            // referring to a package in your workspace that you want to build an app for.
            // If they do exist, that's deeply cursed and I want a user to tell me about it.
            let pkg_spec = package.true_name.clone();
            let kind_label = match kind {
                BinaryKind::Executable => "exe",
                BinaryKind::DynamicLibrary => "cdylib",
                BinaryKind::StaticLibrary => "cstaticlib",
            };
            // FIXME: make this more of a GUID to allow variants to share binaries?
            let bin_id = format!("{variant_id}-{kind_label}-{binary_name}");

            let idx = if let Some(&idx) = self.binaries_by_id.get(&bin_id) {
                // If we already are building this binary we don't need to do it again!
                idx
            } else {
                // Compute the rest of the details and add the binary
                let features = CargoTargetFeatures {
                    default_features: package_config.builds.cargo.default_features,
                    features: if package_config.builds.cargo.all_features {
                        CargoTargetFeatureList::All
                    } else {
                        CargoTargetFeatureList::List(package_config.builds.cargo.features.clone())
                    },
                };

                let target_is_windows = target.is_windows();
                let platform_exe_ext;
                let platform_lib_prefix;
                if target_is_windows {
                    platform_exe_ext = ".exe";
                    platform_lib_prefix = "";
                } else {
                    platform_exe_ext = "";
                    platform_lib_prefix = "lib";
                };

                let platform_lib_ext;
                let platform_staticlib_ext;
                if target_is_windows {
                    platform_lib_ext = ".dll";
                    platform_staticlib_ext = ".lib";
                } else if target.is_linux() {
                    platform_lib_ext = ".so";
                    platform_staticlib_ext = ".a";
                } else if target.is_darwin() {
                    platform_lib_ext = ".dylib";
                    platform_staticlib_ext = ".a";
                } else {
                    return Err(DistError::UnrecognizedTarget { target });
                };

                let file_name = match kind {
                    BinaryKind::Executable => format!("{binary_name}{platform_exe_ext}"),
                    BinaryKind::DynamicLibrary => {
                        format!("{platform_lib_prefix}{binary_name}{platform_lib_ext}")
                    }
                    BinaryKind::StaticLibrary => {
                        format!("{platform_lib_prefix}{binary_name}{platform_staticlib_ext}")
                    }
                };

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
                    kind,
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
        Ok(idx)
    }

    fn add_binary(&mut self, to_release: ReleaseIdx, pkg_idx: PackageIdx, binary_name: String) {
        let release = self.release_mut(to_release);
        release.bins.push((pkg_idx, binary_name));
    }

    fn add_library(&mut self, to_release: ReleaseIdx, pkg_idx: PackageIdx, binary_name: String) {
        let release = self.release_mut(to_release);
        release.cdylibs.push((pkg_idx, binary_name));
    }

    fn add_static_library(
        &mut self,
        to_release: ReleaseIdx,
        pkg_idx: PackageIdx,
        binary_name: String,
    ) {
        let release = self.release_mut(to_release);
        release.cstaticlibs.push((pkg_idx, binary_name));
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
        let checksum = self.inner.config.artifacts.checksum;
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

            if self.inner.config.builds.omnibor {
                let omnibor = self.create_omnibor_artifact(zip_artifact_idx, false);
                self.add_local_artifact(variant_idx, omnibor);
            }
        }
    }

    fn add_extra_artifacts(&mut self, app_config: &AppConfig, to_release: ReleaseIdx) {
        if !self.global_artifacts_enabled() {
            return;
        }
        let dist_dir = &self.inner.dist_dir.to_owned();
        let artifacts = app_config.artifacts.extra.clone();

        for extra in artifacts {
            for artifact_relpath in extra.artifact_relpaths {
                let artifact_name = ArtifactId::new(
                    artifact_relpath
                        .file_name()
                        .expect("extra artifact had no name!?")
                        .to_owned(),
                );
                let target_path = dist_dir.join(artifact_name.as_str());
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

    fn add_cyclonedx_sbom_file(&mut self, to_package: PackageIdx, to_release: ReleaseIdx) {
        let release = self.release(to_release);

        if !self.global_artifacts_enabled() || !release.config.builds.cargo.cargo_cyclonedx {
            return;
        }

        let package = self.workspaces.package(to_package);

        let file_name = format!("{}.cdx.xml", package.true_name);
        let file_path = Utf8Path::new("target/distrib/").join(file_name.clone());
        self.add_global_artifact(
            to_release,
            Artifact {
                id: ArtifactId::new(file_name),
                target_triples: Default::default(),
                archive: None,
                file_path: file_path.clone(),
                required_binaries: Default::default(),
                kind: ArtifactKind::SBOM(SBOMImpl {}),
                checksum: None,
                is_global: true,
            },
        );
    }

    fn create_omnibor_artifact(&mut self, artifact_idx: ArtifactIdx, is_global: bool) -> Artifact {
        let artifact = self.artifact(artifact_idx);
        let id = artifact.id.clone();
        let src_path = artifact.file_path.clone();

        let extension = src_path
            .extension()
            .map_or("omnibor".to_string(), |e| format!("{e}.omnibor"));
        let dest_path = src_path.with_extension(extension);

        let new_id = format!("{}.omnibor", id);

        Artifact {
            id: ArtifactId::new(new_id),
            target_triples: Default::default(),
            archive: None,
            file_path: dest_path.clone(),
            required_binaries: Default::default(),
            kind: ArtifactKind::OmniborArtifactId(OmniborArtifactIdImpl {
                src_path,
                dest_path,
            }),
            checksum: None,
            is_global,
        }
    }

    fn add_unified_checksum_file(&mut self, to_release: ReleaseIdx) {
        if !self.global_artifacts_enabled() {
            return;
        }

        let dist_dir = &self.inner.dist_dir;
        let checksum = self.inner.config.artifacts.checksum;
        let file_name = ArtifactId::new(format!("{}.sum", checksum.ext()));
        let file_path = dist_dir.join(file_name.as_str());

        self.add_global_artifact(
            to_release,
            Artifact {
                id: file_name,
                target_triples: Default::default(),
                archive: None,
                file_path: file_path.clone(),
                required_binaries: Default::default(),
                kind: ArtifactKind::UnifiedChecksum(UnifiedChecksumStep {
                    checksum,
                    dest_path: file_path,
                }),
                checksum: None, // who checksums the checksummers...
                is_global: true,
            },
        );
    }

    fn add_source_tarball(&mut self, _tag: &str, to_release: ReleaseIdx) {
        if !self.global_artifacts_enabled() {
            return;
        }

        if !self.inner.config.artifacts.source_tarball {
            return;
        }

        if self.inner.tools.git.is_none() {
            warn!("skipping source tarball; git not installed");
            return;
        }

        let working_dir = self.inner.workspace_dir.clone();

        let workspace_repo = &self.workspaces.repo;

        // We'll be stubbing the actual generation in this case
        let is_git_repo = if self.inner.local_builds_are_lies {
            true
        } else {
            workspace_repo.is_some()
        };

        let has_head = if self.inner.local_builds_are_lies {
            true
        } else if let Some(repo) = workspace_repo {
            repo.head.is_some()
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
        let checksum = self.inner.config.artifacts.checksum;
        info!("adding source tarball to release {}", release.id);

        let dist_dir = &self.inner.dist_dir.to_owned();

        let artifact_name = ArtifactId::new("source.tar.gz".to_owned());
        let target_path = dist_dir.join(artifact_name.as_str());
        let prefix = format!("{}-{}/", release.app_name, release.version);

        let artifact = Artifact {
            id: artifact_name.to_owned(),
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
            let checksum_id = ArtifactId::new(format!("{artifact_name}.{}", checksum.ext()));
            let checksum_path = dist_dir.join(checksum_id.as_str());
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

        if self.inner.config.builds.omnibor {
            let omnibor = self.create_omnibor_artifact(artifact_idx, true);
            self.add_global_artifact(to_release, omnibor);
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
            let checksum_id = ArtifactId::new(format!("{}.{}", artifact.id, checksum_ext));
            let checksum_path = artifact
                .file_path
                .parent()
                .unwrap()
                .join(checksum_id.as_str());
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
        let filename = ArtifactId::new(format!("{}-update", variant.id));
        let target_path = &self.inner.dist_dir.to_owned().join(filename.as_str());

        Artifact {
            id: filename.to_owned(),
            target_triples: vec![variant.target.to_owned()],
            file_path: target_path.to_owned(),
            required_binaries: FastMap::new(),
            archive: None,
            kind: ArtifactKind::Updater(UpdaterImpl {
                use_latest: self.inner.config.installers.always_use_latest_updater,
            }),
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

        let target_is_windows = variant.target.is_windows();
        let zip_style = if target_is_windows {
            release.config.artifacts.archives.windows_archive
        } else {
            release.config.artifacts.archives.unix_archive
        };

        let artifact_dir_name = variant.id.clone();
        let artifact_dir_path = dist_dir.join(&artifact_dir_name);
        let artifact_ext = zip_style.ext();
        let artifact_name = ArtifactId::new(format!("{artifact_dir_name}{artifact_ext}"));
        let artifact_path = dist_dir.join(artifact_name.as_str());

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

                // FIXME: rustc/cargo has so more complex logic to do platform-specific name remapping
                // (see should_replace_hyphens in src/cargo/core/compiler/build_context/target_info.rs)

                // FIXME: feed info about the expected source symbol name down to build_cargo_target
                // to unhardcode the use of .pdb ...!

                // let src_symbol_ext = symbol_kind.ext();
                let dest_symbol_ext = symbol_kind.ext();
                // let base_name = &binary.name;
                let binary_id = &binary.id;
                // let src_symbol_name = format!("{base_name}.{src_symbol_ext}");
                let dest_symbol_name = ArtifactId::new(format!("{binary_id}.{dest_symbol_ext}"));
                let artifact_path = dist_dir.join(dest_symbol_name.as_str());

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

    fn add_shell_installer(&mut self, to_release: ReleaseIdx) -> DistResult<()> {
        if !self.global_artifacts_enabled() {
            return Ok(());
        }
        let release = self.release(to_release);
        let Some(config) = &release.config.installers.shell else {
            return Ok(());
        };
        require_nonempty_installer(release, config)?;
        let release_id = &release.id;
        let schema_release = self
            .manifest
            .release_by_name(&release.app_name)
            .expect("couldn't find the release!?");

        let env_vars = schema_release.env.clone();

        let download_url = schema_release
            .artifact_download_url()
            .expect("couldn't compute a URL to download artifacts from!?");
        let hosting = schema_release.hosting.clone();
        let artifact_name = ArtifactId::new(format!("{release_id}-installer.sh"));
        let artifact_path = self.inner.dist_dir.join(artifact_name.as_str());
        let installer_url = format!("{download_url}/{artifact_name}");
        let hint = format!("curl --proto '=https' --tlsv1.2 -LsSf {installer_url} | sh");
        let desc = "Install prebuilt binaries via shell script".to_owned();

        // Get the artifacts
        let artifacts = release
            .platform_support
            .fragments()
            .into_iter()
            .filter(|a| !a.target_triple.is_windows_msvc())
            .collect::<Vec<_>>();
        let target_triples = artifacts
            .iter()
            .map(|a| a.target_triple.clone())
            .collect::<Vec<_>>();

        if artifacts.is_empty() {
            warn!("skipping shell installer: not building any supported platforms (use --artifacts=global)");
            return Ok(());
        };
        let bin_aliases = BinaryAliases(config.bin_aliases.clone()).for_targets(&target_triples);

        let runtime_conditions = release.platform_support.safe_conflated_runtime_conditions();

        let installer_artifact = Artifact {
            id: artifact_name,
            target_triples,
            archive: None,
            file_path: artifact_path.clone(),
            required_binaries: FastMap::new(),
            checksum: None,
            kind: ArtifactKind::Installer(InstallerImpl::Shell(InstallerInfo {
                release: to_release,
                dest_path: artifact_path,
                app_name: release.app_name.clone(),
                app_version: release.version.to_string(),
                install_paths: config
                    .install_path
                    .iter()
                    .map(|p| p.clone().into_jinja())
                    .collect(),
                install_success_msg: config.install_success_msg.to_owned(),
                base_url: download_url.to_owned(),
                hosting,
                artifacts,
                hint,
                desc,
                receipt: InstallReceipt::from_metadata(&self.inner, release)?,
                bin_aliases,
                install_libraries: config.install_libraries.clone(),
                runtime_conditions,
                platform_support: None,
                env_vars,
            })),
            is_global: true,
        };

        self.add_global_artifact(to_release, installer_artifact);
        Ok(())
    }

    fn add_homebrew_installer(&mut self, to_release: ReleaseIdx) -> DistResult<()> {
        if !self.global_artifacts_enabled() {
            return Ok(());
        }
        let release = self.release(to_release);
        let Some(config) = &release.config.installers.homebrew else {
            return Ok(());
        };
        require_nonempty_installer(release, config)?;
        let formula = if let Some(formula) = &config.formula {
            formula
        } else {
            &release.id
        };
        let schema_release = self
            .manifest
            .release_by_name(&release.id)
            .expect("couldn't find the release!?");
        let download_url = schema_release
            .artifact_download_url()
            .expect("couldn't compute a URL to download artifacts from!?");
        let hosting = schema_release.hosting.clone();

        let artifact_name = ArtifactId::new(format!("{formula}.rb"));
        let artifact_path = self.inner.dist_dir.join(artifact_name.as_str());

        // If tap is specified, include that in the `brew install` message
        let install_target = if let Some(tap) = &self.inner.global_homebrew_tap {
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
            .filter(|a| !a.target_triple.is_windows_msvc())
            .collect::<Vec<_>>();
        if artifacts.is_empty() {
            warn!("skipping Homebrew installer: not building any supported platforms (use --artifacts=global)");
            return Ok(());
        };

        let target_triples = artifacts
            .iter()
            .map(|a| a.target_triple.clone())
            .collect::<Vec<_>>();

        let find_fragment = |triple: &TripleNameRef| -> Option<ExecutableZipFragment> {
            artifacts
                .iter()
                .find(|a| a.target_triple == triple)
                .cloned()
        };
        let fragments = HomebrewFragments {
            x86_64_macos: find_fragment(TARGET_X64_MAC),
            arm64_macos: find_fragment(TARGET_ARM64_MAC),
            x86_64_linux: find_fragment(TARGET_X64_LINUX_GNU),
            arm64_linux: find_fragment(TARGET_ARM64_LINUX_GNU),
        };

        let release = self.release(to_release);
        let app_name = release.app_name.clone();
        let app_desc = release.app_desc.clone().unwrap_or_else(|| {
            warn!("The Homebrew publish job is enabled but no description was specified\n  consider adding `description = ` to package in Cargo.toml");
            format!("The {} application", release.app_name)
        });
        let app_license = release.app_license.clone();
        let homebrew_dsl_license = app_license.as_ref().map(|app_license| {
            // Parse SPDX license expression and convert to Homebrew's Ruby license DSL.
            // If expression is malformed, fall back to plain input license string.
            to_homebrew_license_format(app_license).unwrap_or(format!("\"{app_license}\""))
        });
        let app_homepage_url = if release.app_homepage_url.is_none() {
            warn!("The Homebrew publish job is enabled but no homepage was specified\n  consider adding `homepage = ` to package in Cargo.toml");
            release.app_repository_url.clone()
        } else {
            release.app_homepage_url.clone()
        };
        let tap = config.tap.clone();

        if tap.is_some() && release.config.publishers.homebrew.is_none() {
            warn!("A Homebrew tap was specified but the Homebrew publish job is disabled\n  consider adding \"homebrew\" to publish-jobs in Cargo.toml");
        }
        if release.config.publishers.homebrew.is_some() && tap.is_none() {
            warn!("The Homebrew publish job is enabled but no tap was specified\n  consider setting the tap field in Cargo.toml");
        }

        let runtime_conditions = release.platform_support.safe_conflated_runtime_conditions();

        let dependencies: Vec<HomebrewPackageName> = release
            .config
            .builds
            .system_dependencies
            .homebrew
            .clone()
            .into_iter()
            .filter(|(_, package)| package.0.stage_wanted(&DependencyKind::Run))
            .map(|(name, _)| name)
            .collect();
        let bin_aliases = BinaryAliases(config.bin_aliases.clone()).for_targets(&target_triples);

        let inner = InstallerInfo {
            release: to_release,
            dest_path: artifact_path.clone(),
            app_name: release.app_name.clone(),
            app_version: release.version.to_string(),
            install_paths: config
                .install_path
                .iter()
                .map(|p| p.clone().into_jinja())
                .collect(),
            install_success_msg: config.install_success_msg.to_owned(),
            base_url: download_url.to_owned(),
            hosting,
            artifacts,
            hint,
            desc,
            receipt: None,
            bin_aliases,
            install_libraries: config.install_libraries.clone(),
            runtime_conditions,
            platform_support: None,
            // Not actually needed for this installer type
            env_vars: None,
        };

        let installer_artifact = Artifact {
            id: artifact_name,
            target_triples,
            archive: None,
            file_path: artifact_path,
            required_binaries: Default::default(),
            checksum: None,
            kind: ArtifactKind::Installer(InstallerImpl::Homebrew(HomebrewImpl {
                info: HomebrewInstallerInfo {
                    name: app_name,
                    formula_class: to_class_case(formula),
                    desc: app_desc,
                    license: homebrew_dsl_license,
                    homepage: app_homepage_url,
                    tap,
                    dependencies,
                    inner,
                    install_libraries: config.install_libraries.clone(),
                },
                fragments,
            })),
            is_global: true,
        };

        self.add_global_artifact(to_release, installer_artifact);
        Ok(())
    }

    fn add_powershell_installer(&mut self, to_release: ReleaseIdx) -> DistResult<()> {
        if !self.global_artifacts_enabled() {
            return Ok(());
        }

        // Get the basic info about the installer
        let release = self.release(to_release);
        let Some(config) = &release.config.installers.powershell else {
            return Ok(());
        };
        require_nonempty_installer(release, config)?;
        let release_id = &release.id;
        let schema_release = self
            .manifest
            .release_by_name(&release.app_name)
            .expect("couldn't find the release!?");

        let env_vars = schema_release.env.clone();

        let download_url = schema_release
            .artifact_download_url()
            .expect("couldn't compute a URL to download artifacts from!?");
        let hosting = schema_release.hosting.clone();
        let artifact_name = ArtifactId::new(format!("{release_id}-installer.ps1"));
        let artifact_path = self.inner.dist_dir.join(artifact_name.as_str());
        let installer_url = format!("{download_url}/{artifact_name}");
        let hint = format!(r#"powershell -ExecutionPolicy Bypass -c "irm {installer_url} | iex""#);
        let desc = "Install prebuilt binaries via powershell script".to_owned();

        // Gather up the bundles the installer supports
        let artifacts = release
            .platform_support
            .fragments()
            .into_iter()
            .filter(|a| a.target_triple.is_windows())
            .collect::<Vec<_>>();
        let target_triples = artifacts
            .iter()
            .map(|a| a.target_triple.clone())
            .collect::<Vec<_>>();
        if artifacts.is_empty() {
            warn!("skipping powershell installer: not building any supported platforms (use --artifacts=global)");
            return Ok(());
        };
        let bin_aliases = BinaryAliases(config.bin_aliases.clone()).for_targets(&target_triples);
        let installer_artifact = Artifact {
            id: artifact_name,
            target_triples,
            file_path: artifact_path.clone(),
            required_binaries: FastMap::new(),
            archive: None,
            checksum: None,
            kind: ArtifactKind::Installer(InstallerImpl::Powershell(InstallerInfo {
                release: to_release,
                dest_path: artifact_path,
                app_name: release.app_name.clone(),
                app_version: release.version.to_string(),
                install_paths: config
                    .install_path
                    .iter()
                    .map(|p| p.clone().into_jinja())
                    .collect(),
                install_success_msg: config.install_success_msg.to_owned(),
                base_url: download_url.to_owned(),
                hosting,
                artifacts,
                hint,
                desc,
                receipt: InstallReceipt::from_metadata(&self.inner, release)?,
                bin_aliases,
                install_libraries: config.install_libraries.clone(),
                runtime_conditions: RuntimeConditions::default(),
                platform_support: None,
                env_vars,
            })),
            is_global: true,
        };

        self.add_global_artifact(to_release, installer_artifact);
        Ok(())
    }

    fn add_npm_installer(&mut self, to_release: ReleaseIdx) -> DistResult<()> {
        if !self.global_artifacts_enabled() {
            return Ok(());
        }
        let release = self.release(to_release);
        let Some(config) = &release.config.installers.npm else {
            return Ok(());
        };
        require_nonempty_installer(release, config)?;
        let release_id = &release.id;
        let schema_release = self
            .manifest
            .release_by_name(&release.app_name)
            .expect("couldn't find the release!?");
        let download_url = schema_release
            .artifact_download_url()
            .expect("couldn't compute a URL to download artifacts from!?");
        let hosting = schema_release.hosting.clone();

        let app_name = config.package.clone();
        let npm_package_name = if let Some(scope) = &config.scope {
            if scope.to_ascii_lowercase() != *scope {
                return Err(DistError::ScopeMustBeLowercase {
                    scope: scope.to_owned(),
                });
            }

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
        let artifact_name = ArtifactId::new(format!("{dir_name}{zip_ext}"));
        let artifact_path = self.inner.dist_dir.join(artifact_name.as_str());
        // let installer_url = format!("{download_url}/{artifact_name}");
        let hint = format!("npm install {npm_package_name}@{npm_package_version}");
        let desc = "Install prebuilt binaries into your npm project".to_owned();

        let artifacts = release.platform_support.fragments();
        let target_triples = artifacts
            .iter()
            .map(|a| a.target_triple.clone())
            .collect::<Vec<_>>();

        if artifacts.is_empty() {
            warn!("skipping npm installer: not building any supported platforms (use --artifacts=global)");
            return Ok(());
        };
        let bin_aliases = BinaryAliases(config.bin_aliases.clone()).for_targets(&target_triples);

        let runtime_conditions = release.platform_support.safe_conflated_runtime_conditions();

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
                    release: to_release,
                    dest_path: artifact_path,
                    app_name,
                    app_version: release.version.to_string(),
                    install_paths: config
                        .install_path
                        .iter()
                        .map(|p| p.clone().into_jinja())
                        .collect(),
                    install_success_msg: config.install_success_msg.to_owned(),
                    base_url: download_url.to_owned(),
                    hosting,
                    artifacts,
                    hint,
                    desc,
                    receipt: None,
                    bin_aliases,
                    install_libraries: config.install_libraries.clone(),
                    runtime_conditions,
                    platform_support: None,
                    // Not actually needed for this installer type
                    env_vars: None,
                },
            })),
            is_global: true,
        };

        self.add_global_artifact(to_release, installer_artifact);
        Ok(())
    }

    fn add_msi_installer(&mut self, to_release: ReleaseIdx) -> DistResult<()> {
        if !self.local_artifacts_enabled() {
            return Ok(());
        }

        // Clone info we need from the release to avoid borrowing across the loop
        let release = self.release(to_release);
        // FIXME: because we use cargo-wix and cargo-wix's config,
        // msi installers really don't respect any of our own config!
        // (We still look it up because it determines whether enabled or not.)
        let Some(_config) = &release.config.installers.msi else {
            return Ok(());
        };
        // FIXME: MSI installer contents don't actually respect this
        // require_nonempty_installer(release, config)?;
        let variants = release.variants.clone();
        let checksum = self.inner.config.artifacts.checksum;

        // Make an msi for every windows platform
        for variant_idx in variants {
            let variant = self.variant(variant_idx);
            let binaries = variant.binaries.clone();
            let target = &variant.target;
            if !target.is_windows() {
                continue;
            }

            let variant_id = &variant.id;
            let artifact_name = ArtifactId::new(format!("{variant_id}.msi"));
            let artifact_path = self.inner.dist_dir.join(artifact_name.as_str());
            let dir_name = format!("{variant_id}_msi");
            let dir_path = self.inner.dist_dir.join(&dir_name);

            // Compute which package we're actually building, based on the binaries
            let mut package_info: Option<(String, PackageIdx)> = None;
            for &binary_idx in &binaries {
                let binary = self.binary(binary_idx);
                if let Some((existing_spec, _)) = &package_info {
                    // cargo-wix doesn't clearly support multi-package, so bail
                    if existing_spec != &binary.pkg_spec {
                        return Err(DistError::MultiPackage {
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
                return Err(DistError::NoPackage { artifact_name })?;
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
            if self.inner.config.builds.omnibor {
                let omnibor = self.create_omnibor_artifact(installer_idx, false);
                self.add_local_artifact(variant_idx, omnibor);
            }
        }

        Ok(())
    }

    fn add_pkg_installer(&mut self, to_release: ReleaseIdx) -> DistResult<()> {
        if !self.local_artifacts_enabled() {
            return Ok(());
        }

        // Clone info we need from the release to avoid borrowing across the loop
        let release = self.release(to_release);
        let Some(config) = release.config.installers.pkg.clone() else {
            return Ok(());
        };
        require_nonempty_installer(release, &config)?;
        let version = release.version.clone();
        let fragments = release.platform_support.fragments();

        let variants = release.variants.clone();
        let checksum = self.inner.config.artifacts.checksum;

        // Make a pkg for every darwin platform
        for variant_idx in variants {
            let variant = self.variant(variant_idx);
            let binaries = variant.binaries.clone();
            let bin_aliases = BinaryAliases(config.bin_aliases.clone());
            let target = &variant.target;
            if !target.is_darwin() {
                continue;
            }

            let variant_id = &variant.id;
            let artifact_name = ArtifactId::new(format!("{variant_id}.pkg"));
            let artifact_path = self.inner.dist_dir.join(artifact_name.as_str());
            let dir_name = format!("{variant_id}_pkg");
            let dir_path = self.inner.dist_dir.join(&dir_name);

            // Compute which package we're actually building, based on the binaries
            let mut package_info: Option<(String, PackageIdx)> = None;
            for &binary_idx in &binaries {
                let binary = self.binary(binary_idx);
                if let Some((existing_spec, _)) = &package_info {
                    // we haven't set ourselves up to bundle multiple packages yet
                    if existing_spec != &binary.pkg_spec {
                        return Err(DistError::MultiPackage {
                            artifact_name,
                            spec1: existing_spec.clone(),
                            spec2: binary.pkg_spec.clone(),
                        })?;
                    }
                } else {
                    package_info = Some((binary.pkg_spec.clone(), binary.pkg_idx));
                }
            }

            let Some(artifact) = fragments
                .clone()
                .into_iter()
                .find(|a| a.target_triple == variant.target)
            else {
                return Err(DistError::NoPackage { artifact_name })?;
            };

            let bin_aliases = bin_aliases.for_target(&variant.target);

            let identifier = if let Some(id) = &config.identifier {
                id.to_owned()
            } else {
                return Err(DistError::MacPkgBundleIdentifierMissing {});
            };

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
                kind: ArtifactKind::Installer(InstallerImpl::Pkg(PkgInstallerInfo {
                    file_path: artifact_path.clone(),
                    artifact,
                    package_dir: dir_path.clone(),
                    identifier,
                    install_location: config.install_location.clone(),
                    version: version.to_string(),
                    bin_aliases,
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
            if self.inner.config.builds.omnibor {
                let omnibor = self.create_omnibor_artifact(installer_idx, false);
                self.add_local_artifact(variant_idx, omnibor);
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

    fn compute_build_steps(&mut self) -> DistResult<()> {
        // FIXME: more intelligently schedule these in a proper graph?

        let mut local_build_steps = vec![];
        let mut global_build_steps = vec![];

        for workspace_idx in self.workspaces.all_workspace_indices() {
            let workspace_kind = self.workspaces.workspace(workspace_idx).kind;
            let builds = match workspace_kind {
                axoproject::WorkspaceKind::Javascript => {
                    self.compute_generic_builds(workspace_idx)?
                }
                axoproject::WorkspaceKind::Generic => self.compute_generic_builds(workspace_idx)?,
                axoproject::WorkspaceKind::Rust => self.compute_cargo_builds(workspace_idx)?,
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

        Ok(())
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
                ArtifactKind::UnifiedChecksum(unified_checksum) => {
                    build_steps.push(BuildStep::UnifiedChecksum(unified_checksum.clone()));
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
                ArtifactKind::Updater(UpdaterImpl { use_latest }) => {
                    build_steps.push(BuildStep::Updater(UpdaterStep {
                        // There should only be one triple per artifact
                        target_triple: artifact.target_triples.first().unwrap().to_owned(),
                        target_filename: artifact.file_path.to_owned(),
                        use_latest: *use_latest,
                    }))
                }
                ArtifactKind::SBOM(_) => {
                    // The SBOM is already generated.
                }
                ArtifactKind::OmniborArtifactId(src) => {
                    let src_path = src.src_path.clone();
                    let old_extension = src_path.extension().unwrap_or("");
                    let dest_path = src_path.with_extension(format!("{}.omnibor", old_extension));

                    build_steps.push(BuildStep::OmniborArtifactId(OmniborArtifactIdImpl {
                        src_path,
                        dest_path,
                    }));
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

    fn validate_distable_packages(&self, announcing: &AnnouncementTag) -> DistResult<()> {
        for release in &announcing.rust_releases {
            let package = self.workspaces.package(release.package_idx);
            let workspace_idx = self.workspaces.workspace_for_package(release.package_idx);
            let package_workspace = self.workspaces.workspace(workspace_idx);
            let package_kind = package_workspace.kind;
            if announcing.package.is_none() {
                match package_kind {
                    axoproject::WorkspaceKind::Generic | axoproject::WorkspaceKind::Javascript => {
                        if let Some(build_command) = &package.build_command {
                            if build_command.len() == 1
                                && build_command.first().unwrap().contains(' ')
                            {
                                return Err(DistError::SusBuildCommand {
                                    manifest: package
                                        .dist_manifest_path
                                        .clone()
                                        .unwrap_or_else(|| package.manifest_path.clone()),
                                    command: build_command[0].clone(),
                                });
                            } else if build_command.is_empty() {
                                return Err(DistError::NoBuildCommand {
                                    manifest: package
                                        .dist_manifest_path
                                        .clone()
                                        .unwrap_or_else(|| package.manifest_path.clone()),
                                });
                            }
                        } else if package_kind == axoproject::WorkspaceKind::Javascript {
                            return Err(DistError::NoDistScript {
                                manifest: package.manifest_path.clone(),
                            });
                        } else {
                            return Err(DistError::NoBuildCommand {
                                manifest: package
                                    .dist_manifest_path
                                    .clone()
                                    .unwrap_or_else(|| package.manifest_path.clone()),
                            });
                        }
                    }
                    axoproject::WorkspaceKind::Rust => {
                        if package.build_command.is_some() {
                            return Err(DistError::UnexpectedBuildCommand {
                                manifest: package
                                    .dist_manifest_path
                                    .clone()
                                    .unwrap_or_else(|| package.manifest_path.clone()),
                            });
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn compute_releases(
        &mut self,
        cfg: &Config,
        announcing: &AnnouncementTag,
        triples: &[TripleName],
        bypass_package_target_prefs: bool,
    ) -> DistResult<()> {
        // Create a Release for each package
        for info in &announcing.rust_releases {
            // FIXME: this clone is hacky but I'm in the middle of a nasty refactor
            let app_config = self.package_config(info.package_idx).clone();

            // Create a Release for this binary
            let release = self.add_release(info.package_idx);

            // Don't bother with any of this without binaries
            // or C libraries
            // (releases a Rust library, nothing to Build)
            if info.executables.is_empty() && info.cdylibs.is_empty() && info.cstaticlibs.is_empty()
            {
                continue;
            }

            // Tell the Release to include these binaries
            for binary in &info.executables {
                self.add_binary(release, info.package_idx, binary.to_owned());
            }

            for lib in &info.cdylibs {
                self.add_library(release, info.package_idx, lib.to_owned());
            }

            for lib in &info.cstaticlibs {
                self.add_static_library(release, info.package_idx, lib.to_owned());
            }

            // Create variants for this Release for each target
            for target in triples {
                // This logic ensures that (outside of host mode) we only select targets that are a
                // subset of the ones the package claims to support
                let use_target =
                    bypass_package_target_prefs || app_config.targets.iter().any(|t| t == target);
                if !use_target {
                    continue;
                }

                // Create the variant
                let variant = self.add_variant(release, target.clone())?;

                if self.inner.config.installers.updater {
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
            self.add_extra_artifacts(&app_config, release);

            // Add installers to the Release
            // Prefer the CLI's choices (`cfg`) if they're non-empty
            let installers = if cfg.installers.is_empty() {
                &[
                    InstallerStyle::Shell,
                    InstallerStyle::Powershell,
                    InstallerStyle::Homebrew,
                    InstallerStyle::Npm,
                    InstallerStyle::Msi,
                    InstallerStyle::Pkg,
                ]
            } else {
                &cfg.installers[..]
            };

            for installer in installers {
                match installer {
                    InstallerStyle::Shell => self.add_shell_installer(release)?,
                    InstallerStyle::Powershell => self.add_powershell_installer(release)?,
                    InstallerStyle::Homebrew => self.add_homebrew_installer(release)?,
                    InstallerStyle::Npm => self.add_npm_installer(release)?,
                    InstallerStyle::Msi => self.add_msi_installer(release)?,
                    InstallerStyle::Pkg => self.add_pkg_installer(release)?,
                }
            }

            // Add SBOM file, if it exists.
            self.add_cyclonedx_sbom_file(info.package_idx, release);

            // Add the unified checksum file
            self.add_unified_checksum_file(release);
        }

        // Translate the result to DistManifest
        crate::manifest::add_releases_to_manifest(cfg, &self.inner, &mut self.manifest)?;

        Ok(())
    }

    fn compute_ci(&mut self) -> DistResult<()> {
        let CiConfig { github } = &self.inner.config.ci;

        let mut has_ci = false;
        if let Some(github_config) = github {
            has_ci = true;
            self.inner.ci.github = Some(GithubCiInfo::new(&self.inner, github_config)?);
        }

        // apply to manifest
        if has_ci {
            let CiInfo { github } = &self.inner.ci;
            let github = github.as_ref().map(|info| {
                let external_repo_commit = info
                    .github_release
                    .as_ref()
                    .and_then(|r| r.external_repo_commit.clone());
                cargo_dist_schema::GithubCiInfo {
                    artifacts_matrix: Some(info.artifacts_matrix.clone()),
                    pr_run_mode: Some(info.pr_run_mode),
                    external_repo_commit,
                }
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

    pub(crate) fn package_config(&self, pkg_idx: PackageIdx) -> &AppConfig {
        &self.package_configs[pkg_idx.0]
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

    // If no targets were specified, just use the host target
    let host_target_triple = [graph.inner.tools.host_target.clone()];
    // If all targets specified, union together the targets our packages support
    // Note that this uses BTreeSet as an intermediate to make the order stable
    let all_target_triples = graph
        .workspaces
        .all_packages()
        .flat_map(|(id, _)| &graph.package_config(id).targets)
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
                host_target: graph.inner.tools.host_target.clone(),
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

    graph.validate_distable_packages(&announcing)?;

    // Immediately check if there's other manifests kicking around that provide info
    // we don't want to recompute (lets us move towards more of an architecture where
    // `plan` figures out what to do and subsequent steps Simply Obey).
    crate::manifest::load_and_merge_manifests(
        &graph.inner.dist_dir,
        &mut graph.manifest,
        &announcing,
    )?;

    // Figure out how artifacts should be hosted
    graph.compute_hosting(cfg, &announcing)?;

    // Figure out what we're releasing/building
    graph.compute_releases(cfg, &announcing, triples, bypass_package_target_prefs)?;

    // Prep the announcement's release notes and whatnot
    graph.compute_announcement_info(&announcing);

    // Finally compute all the build steps!
    graph.compute_build_steps()?;

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
pub fn get_cargo_info(cargo: String) -> DistResult<CargoInfo> {
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
                host_target: TripleName::new(target.to_owned()),
            });
        }
    }
    Err(DistError::FailedCargoVersion)
}

fn target_symbol_kind(target: &TripleNameRef) -> Option<SymbolKind> {
    #[allow(clippy::if_same_then_else)]
    if target.is_windows_msvc() {
        // Temporary disabled pending redesign of symbol handling!

        // Some(SymbolKind::Pdb)
        None
    } else if target.is_apple() {
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
    let cargo = if let Ok(cargo_cmd) = cargo() {
        get_cargo_info(cargo_cmd).ok()
    } else {
        None
    };
    Ok(Tools {
        host_target: TripleName::new(current_platform::CURRENT_PLATFORM.to_owned()),
        cargo,
        rustup: find_tool("rustup", "-V"),
        brew: find_tool("brew", "--version"),
        git: find_tool("git", "--version"),
        omnibor: find_tool("omnibor", "--version"),
        // Computed later if needed
        code_sign_tool: None,

        // NOTE: This doesn't actually give us cargo-auditable's version info,
        // but it does confirm it's installed, which is what we care about.
        cargo_auditable: find_cargo_subcommand("cargo", "auditable", "--version"),

        cargo_cyclonedx: find_cargo_subcommand("cargo", "cyclonedx", "--version"),
        cargo_xwin: find_cargo_subcommand("cargo", "xwin", "--version"),
        cargo_zigbuild: find_tool("cargo-zigbuild", "--version"),
    })
}

fn find_cargo_subcommand(name: &str, arg: &str, test_flag: &str) -> Option<Tool> {
    let output = Cmd::new(name, "detect tool")
        .arg(arg)
        .arg(test_flag)
        .check(false)
        .output()
        .ok()?;
    let string_output = String::from_utf8(output.stdout).ok()?;
    let version = string_output.lines().next()?;
    Some(Tool {
        cmd: format!("{} {}", name, arg),
        version: version.to_owned(),
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

/// Which style of installation layout this app uses
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallLayout {
    /// Not specified; will be determined later
    Unspecified,
    /// All files are in a single directory
    Flat,
    /// Separated into file type-specific directories
    Hierarchical,
    /// Like Hierarchical, but with only a bin subdirectory
    CargoHome,
}

/// Struct representing an install receipt
#[derive(Clone, Debug, Serialize)]
pub struct InstallReceipt {
    /// The location on disk where this app was installed
    pub install_prefix: String,
    /// The layout within the above prefix
    pub install_layout: InstallLayout,
    /// A list of all binaries installed by this app
    pub binaries: Vec<String>,
    /// A list of all C dynamic libraries installed by this app
    pub cdylibs: Vec<String>,
    /// A list of all C static libraries installed by this app
    pub cstaticlibs: Vec<String>,
    /// Information about where to request information on new releases
    pub source: ReleaseSource,
    /// The version that was installed
    pub version: String,
    /// The software which installed this receipt
    pub provider: Provider,
    /// A list of aliases binaries were installed under
    pub binary_aliases: BTreeMap<String, Vec<String>>,
    /// Whether or not to modify system paths when installing
    pub modify_path: bool,
}

impl InstallReceipt {
    /// Produces an install receipt for the given DistGraph.
    pub fn from_metadata(
        manifest: &DistGraph,
        release: &Release,
    ) -> DistResult<Option<InstallReceipt>> {
        let hosting = if let Some(hosting) = &manifest.hosting {
            hosting
        } else {
            return Ok(None);
        };
        let source_type = if hosting.hosts.contains(&HostingStyle::Github) {
            ReleaseSourceType::GitHub
        } else {
            return Err(DistError::NoGitHubHosting {});
        };

        Ok(Some(InstallReceipt {
            // These first five are placeholder values which the installer will update
            install_prefix: "AXO_INSTALL_PREFIX".to_owned(),
            install_layout: InstallLayout::Unspecified,
            binaries: vec!["CARGO_DIST_BINS".to_owned()],
            cdylibs: vec!["CARGO_DIST_DYLIBS".to_owned()],
            cstaticlibs: vec!["CARGO_DIST_STATICLIBS".to_owned()],
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
            modify_path: true,
        }))
    }
}

fn require_nonempty_installer(release: &Release, config: &CommonInstallerConfig) -> DistResult<()> {
    if config.install_libraries.is_empty() && release.bins.is_empty() {
        Err(DistError::EmptyInstaller {})
    } else {
        Ok(())
    }
}
