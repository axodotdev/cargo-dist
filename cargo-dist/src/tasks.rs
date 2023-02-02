//! Code to compute the tasks cargo-dist should do
//!
//! This is the heart and soul of cargo-dist, and ideally the [`gather_work`][] function
//! should compute every minute detail dist will perform ahead of time.

use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Read},
    process::Command,
};

use camino::{Utf8Path, Utf8PathBuf};
use cargo_dist_schema::ArtifactKind;
use guppy::{
    graph::{
        BuildTargetId, DependencyDirection, PackageGraph, PackageMetadata, PackageSet, Workspace,
    },
    MetadataCommand, PackageId,
};
use miette::{miette, Context, IntoDiagnostic};
use semver::Version;
use serde::Deserialize;
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

/// Contents of METADATA_DIST in Cargo.toml files
#[derive(Deserialize, Debug)]
pub struct DistMetadata {}

/// Global config for commands
#[derive(Debug)]
pub struct Config {
    /// Whether we'll actually run builds (if false we'll still generate installers)
    pub build: bool,
    /// Whether local paths to files should be in the final dist json output
    pub no_local_paths: bool,
    /// Target triples we want to build for
    pub targets: Vec<String>,
    /// Installers we want to generate
    pub installers: Vec<InstallerStyle>,
}

/// The style of CI we should generate
#[derive(Clone, Copy, Debug)]
pub enum CiStyle {
    /// Genereate Github CI
    Github,
}

/// The style of Installer we should generate
#[derive(Clone, Copy, Debug)]
pub enum InstallerStyle {
    /// Generate a shell script that fetches from a Github Release
    GithubShell,
    /// Generate a powershell script that fetches from a Github Release
    GithubPowershell,
}

/// A unique id for a [`BuildTarget`][]
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
pub struct BuildTargetIdx(pub usize);

/// A unique id for a [`BuiltAsset`][]
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
pub struct BuiltAssetIdx(pub usize);

/// A unique id for a [`ArtifactTarget`][]
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
pub struct ArtifactTargetIdx(pub usize);

/// The graph of all work that cargo-dist needs to do on this invocation.
///
/// All work is precomputed at the start of execution because only discovering
/// what you need to do in the middle of building/packing things is a mess.
/// It also allows us to report what *should* happen without actually doing it.
#[derive(Debug)]
pub struct DistGraph {
    /// The executable cargo told us to find itself at.
    pub cargo: String,
    /// The cargo target dir.
    pub target_dir: Utf8PathBuf,
    /// The root directory of the current cargo workspace.
    pub workspace_dir: Utf8PathBuf,
    /// cargo-dist's target dir (generally nested under `target_dir`).
    pub dist_dir: Utf8PathBuf,
    /// Targets we need to build
    pub targets: Vec<BuildTarget>,
    /// Assets we want to get out of builds
    pub built_assets: Vec<BuiltAsset>,
    /// Distributable artifacts we want to produce for the releases
    pub artifacts: Vec<ArtifactTarget>,
    /// Logical releases that artifacts are grouped under
    pub releases: Vec<ReleaseTarget>,
}

/// A build we need to perform to get artifacts to distribute.
#[derive(Debug)]
pub enum BuildTarget {
    /// A cargo build
    Cargo(CargoBuildTarget),
    // Other build systems..?
}

/// A cargo build
#[derive(Debug)]
pub struct CargoBuildTarget {
    /// The --target triple to pass
    pub target_triple: String,
    /// The feature flags to pass
    pub features: CargoTargetFeatures,
    /// What package to build (or "the workspace")
    pub package: CargoTargetPackages,
    /// The --profile to pass
    pub profile: String,
    /// Assets we expect from this build
    pub expected_assets: Vec<BuiltAssetIdx>,
}

/// An asset we need from our builds
#[derive(Debug)]
pub enum BuiltAsset {
    /// An executable
    Executable(ExecutableBuiltAsset),
    /// Symbols for an executable
    Symbols(SymbolsBuiltAsset),
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

/// An executable we need from our builds
#[derive(Debug)]
pub struct ExecutableBuiltAsset {
    /// The name of the executable (without a file extension)
    pub exe_name: String,
    /// The cargo package this executable is defined by
    pub package_id: PackageId,
    /// The [`BuildTarget`][] that should produce this.
    pub build_target: BuildTargetIdx,
    /// The artifact containing symbols for this
    pub symbols_artifact: Option<ArtifactTargetIdx>,
}

/// Symbols we need from our builds
#[derive(Debug)]
pub struct SymbolsBuiltAsset {
    /// The name of the executable these symbols are for (without a file extension)
    pub exe_name: String,
    /// The cargo package this executable is defined by
    pub package_id: PackageId,
    /// The [`BuildTarget`][] that should produce this.
    pub build_target: BuildTargetIdx,
    /// The kind of symbols this is
    pub symbol_kind: SymbolKind,
}

/// A distributable artifact we want to build
#[derive(Debug)]
pub struct ArtifactTarget {
    /// The target platform
    ///
    /// i.e. `x86_64-pc-windows-msvc`
    pub target_triples: Vec<String>,
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
    /// The file name of the artifact when it's completed and placed in the root of the dist dir.
    ///
    /// i.e. `cargo-dist-v0.1.0-x86_64-pc-windows-msvc.zip`
    pub file_name: String,
    /// The path where the final artifact will appear in the dist dir.
    ///
    /// i.e. `/.../target/dist/cargo-dist-v0.1.0-x86_64-pc-windows-msvc.zip`
    pub file_path: Utf8PathBuf,
    /// The bundling method (zip, tar.gz, ...)
    pub bundle: BundleStyle,
    /// The built assets this artifact will contain
    ///
    /// i.e. `cargo-dist.exe`
    pub built_assets: HashMap<BuiltAssetIdx, Utf8PathBuf>,
    /// Additional static assets to add to the artifact
    ///
    /// i.e. `README.md`
    pub static_assets: Vec<(StaticAssetKind, Utf8PathBuf)>,
    /// The kind of artifact this is
    pub kind: ArtifactKind,
}

/// A logical release of an application that artifacts are grouped under
#[derive(Debug)]
pub struct ReleaseTarget {
    /// The name of the app
    pub app_name: String,
    /// The version of the app
    pub version: Version,
    /// The artifacts this release includes
    pub artifacts: Vec<ArtifactTargetIdx>,
    /// The body of the changelog for this release
    pub changelog_body: Option<String>,
    /// The title of the changelog for this release
    pub changelog_title: Option<String>,
}

/// A particular kind of static asset we're interested in
#[derive(Debug)]
pub enum StaticAssetKind {
    /// A README file
    Readme,
    /// A LICENSE file
    License,
    /// A CHANGLEOG or RELEASES file
    Changelog,
}

/// The style of bundle for a [`ArtifactTarget`][].
#[derive(Debug)]
pub enum BundleStyle {
    /// Just a single uncompressed file
    UncompressedFile,
    /// `.zip`
    Zip,
    /// `.tar.<compression>`
    Tar(CompressionImpl),
    /// An installer
    Installer(InstallerImpl),
    // TODO: Microsoft MSI installer
    // TODO: Apple .dmg "installer"
    // TODO: flatpak?
    // TODO: snap? (ostensibly "obsoleted" by flatpak)
    // TODO: various linux package manager manifests? (.deb, .rpm, ... do these make sense?)
}

/// Compression impls (used by [`BundleStyle::Tar`][])
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
    pub package_info: HashMap<&'pkg_graph PackageId, PackageInfo>,
    /// Path to the Cargo.toml of the workspace (may be a package's Cargo.toml)
    pub manifest_path: Utf8PathBuf,
    /// If the manifest_path points to a package, this is the one.
    ///
    /// If this is None, the workspace Cargo.toml is a virtual manifest.
    pub root_package: Option<PackageMetadata<'pkg_graph>>,
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
}

/// Precompute all the work this invocation will need to do
pub fn gather_work(cfg: &Config) -> Result<DistGraph> {
    let cargo = cargo()?;
    let pkg_graph = package_graph(&cargo)?;
    let workspace = workspace_info(&pkg_graph)?;

    // TODO: use this (currently empty)
    let _workspace_config = pkg_graph
        .workspace()
        .metadata_table()
        .get(METADATA_DIST)
        .map(DistMetadata::deserialize)
        .transpose()
        .into_diagnostic()
        .wrap_err("couldn't parse [workspace.metadata.dist]")?;

    // Currently just assume we're in a workspace, no single package!
    /*
    let root_package = binaries.get(0).map(|(p, _)| p).unwrap();
    let local_config = binaries
        .get(0)
        .and_then(|(p, _)| p.metadata_table().get(METADATA_DIST))
        .map(DistMetadata::deserialize)
        .transpose()
        .into_diagnostic()
        .wrap_err("couldn't parse package's [metadata.dist]")?;
     */

    let target_dir = workspace.info.target_directory().to_owned();
    let workspace_dir = workspace.info.root().to_owned();
    let dist_dir = target_dir.join(TARGET_DIST);

    // If no targets were specified, just use the host target
    let host_target_triple = [get_host_target(&cargo)?];
    let triples = if cfg.targets.is_empty() {
        &host_target_triple
    } else {
        &cfg.targets[..]
    };

    let mut targets = triples
        .iter()
        .map(|target_triple| {
            BuildTarget::Cargo(CargoBuildTarget {
                target_triple: target_triple.clone(),
                // Just build the whole workspace for now
                package: CargoTargetPackages::Workspace,
                // Just use the default build for now
                features: CargoTargetFeatures {
                    no_default_features: false,
                    features: CargoTargetFeatureList::List(vec![]),
                },
                // Release is the GOAT profile, *obviously*
                profile: String::from(PROFILE_DIST),
                // Populated later
                expected_assets: vec![],
            })
        })
        .collect::<Vec<_>>();

    // Find all the executables that each target will build
    let mut executables = vec![];
    for (idx, target) in targets.iter_mut().enumerate() {
        let target_idx = BuildTargetIdx(idx);
        match target {
            BuildTarget::Cargo(target) => {
                let new_executables = match &target.package {
                    CargoTargetPackages::Workspace => binaries_for_cargo_packages(
                        target_idx,
                        workspace.members.packages(DependencyDirection::Forward),
                    ),
                    CargoTargetPackages::Package(package_id) => {
                        binaries_for_cargo_packages(target_idx, pkg_graph.metadata(package_id))
                    }
                };
                executables.extend(new_executables);
            }
        }
    }

    // Give each binary its own artifact (for now)
    let mut artifacts = vec![];
    let mut built_assets = vec![];
    let mut releases = HashMap::<(String, Version), (PackageId, ReleaseTarget)>::new();
    for exe in executables {
        // TODO: make app name configurable? Use some other fields in the PackageMetadata?
        let app_name = exe.exe_name.clone();
        let package_id = exe.package_id.clone();
        let package_info = &workspace.package_info[&&package_id];
        // TODO: allow apps to be versioned separately from packages?
        let version = package_info.version.clone();
        let build_target = &mut targets[exe.build_target.0];

        // Register this executable as an asset we'll build
        let exe_asset_idx = BuiltAssetIdx(built_assets.len());
        built_assets.push(BuiltAsset::Executable(exe));

        let target_triple = match build_target {
            BuildTarget::Cargo(target) => target.target_triple.clone(),
        };

        // TODO: make bundle style configurable
        let target_is_windows = target_triple.contains("windows");
        let exe_bundle = if target_is_windows {
            // Windows loves them zips
            BundleStyle::Zip
        } else {
            // tar.xz is well-supported everywhere and much better than tar.gz
            BundleStyle::Tar(CompressionImpl::Xzip)
        };
        let platform_exe_ext = if target_is_windows { ".exe" } else { "" };

        // TODO: make bundled assets configurable
        // TODO: narrow this scope to the package of the binary..?
        let mut exe_static_assets = vec![];
        if let Some(readme) = &package_info.readme_file {
            exe_static_assets.push((StaticAssetKind::Readme, readme.clone()));
        }
        if let Some(changelog) = &package_info.changelog_file {
            exe_static_assets.push((StaticAssetKind::Changelog, changelog.clone()));
        }
        for license in &package_info.license_files {
            exe_static_assets.push((StaticAssetKind::License, license.clone()));
        }

        // TODO: make the bundle name configurable?
        let exe_dir_name = format!("{app_name}-v{version}-{target_triple}");
        let exe_dir_path = dist_dir.join(&exe_dir_name);
        let exe_file_ext = match exe_bundle {
            BundleStyle::UncompressedFile => platform_exe_ext,
            BundleStyle::Zip => ".zip",
            BundleStyle::Tar(CompressionImpl::Gzip) => ".tar.gz",
            BundleStyle::Tar(CompressionImpl::Zstd) => ".tar.zstd",
            BundleStyle::Tar(CompressionImpl::Xzip) => ".tar.xz",
            BundleStyle::Installer(_) => unreachable!("exe's shouldn't be installers"),
        };
        let exe_bundle_name = format!("{exe_dir_name}{exe_file_ext}");
        let exe_bundle_path = dist_dir.join(&exe_bundle_name);
        let exe_file_name = format!("{app_name}{platform_exe_ext}");

        // Ensure the release exists
        let (_, release) = releases
            .entry((app_name.clone(), version.clone()))
            .or_insert_with(|| {
                (
                    package_id,
                    ReleaseTarget {
                        app_name: app_name.clone(),
                        version: version.clone(),
                        artifacts: vec![],
                        changelog_body: None,
                        changelog_title: None,
                    },
                )
            });

        // Tell the target about this BuiltAsset is needs to make
        #[allow(irrefutable_let_patterns)]
        if let BuildTarget::Cargo(cargo_build_target) = build_target {
            cargo_build_target.expected_assets.push(exe_asset_idx);

            // If we support symbols, makes assets/artifacts for them too
            if let Some(symbol_kind) = target_symbol_kind(&cargo_build_target.target_triple) {
                let BuiltAsset::Executable(exe_asset) = &built_assets[exe_asset_idx.0] else {
                    unreachable!();
                };

                // Create a BuiltAsset for the symbols
                let sym_asset_idx = BuiltAssetIdx(built_assets.len());
                let sym_asset = BuiltAsset::Symbols(SymbolsBuiltAsset {
                    exe_name: exe_asset.exe_name.clone(),
                    package_id: exe_asset.package_id.clone(),
                    build_target: exe_asset.build_target,
                    symbol_kind,
                });
                built_assets.push(sym_asset);

                // Add the asset to the target
                cargo_build_target.expected_assets.push(sym_asset_idx);

                // Create a dedicated artifact for this asset
                let sym_ext = symbol_kind.ext();
                let sym_file_name = format!("{exe_dir_name}.{sym_ext}");
                let sym_file_path = dist_dir.join(&sym_file_name);

                let sym_artifact_idx = ArtifactTargetIdx(artifacts.len());
                artifacts.push(ArtifactTarget {
                    target_triples: vec![target_triple.clone()],
                    dir_name: None,
                    dir_path: None,
                    file_name: sym_file_name,
                    file_path: sym_file_path,
                    bundle: BundleStyle::UncompressedFile,
                    built_assets: Some((sym_asset_idx, Utf8PathBuf::new()))
                        .into_iter()
                        .collect(),
                    static_assets: Default::default(),
                    kind: ArtifactKind::Symbols,
                });
                release.artifacts.push(sym_artifact_idx);

                let BuiltAsset::Executable(exe_asset) = &mut built_assets[exe_asset_idx.0] else {
                    unreachable!();
                };
                exe_asset.symbols_artifact = Some(sym_artifact_idx);
            }
        }

        let exe_artifact_idx = ArtifactTargetIdx(artifacts.len());
        artifacts.push(ArtifactTarget {
            target_triples: vec![target_triple],
            dir_name: Some(exe_dir_name),
            dir_path: Some(exe_dir_path),
            file_path: exe_bundle_path,
            file_name: exe_bundle_name.clone(),
            bundle: exe_bundle,
            built_assets: Some((exe_asset_idx, Utf8PathBuf::from(exe_file_name)))
                .into_iter()
                .collect(),
            static_assets: exe_static_assets,
            kind: ArtifactKind::ExecutableZip,
        });
        release.artifacts.push(exe_artifact_idx);
    }

    // Add installers (currently all 1:1 with releases rather than targets)
    for ((app_name, version), (package_id, release)) in &mut releases {
        let package_info = &workspace.package_info[&&*package_id];
        let repo = package_info.repository_url.as_deref();
        for installer in &cfg.installers {
            let file_path;
            let file_name;
            let installer_impl;
            let target_triples;

            match installer {
                InstallerStyle::GithubShell => {
                    let Some(repo_url) = repo else {
                        warn!("skipping --installer=github-shell: 'repository' isn't set in Cargo.toml");
                        continue;
                    };
                    file_name = "installer.sh".to_owned();
                    file_path = dist_dir.join(&file_name);
                    // All the triples we know about, sans windows (windows-gnu isn't handled...)
                    target_triples = triples
                        .iter()
                        .filter(|s| !s.contains("windows"))
                        .cloned()
                        .collect::<Vec<_>>();
                    let app_version = format!("v{version}");
                    let hint = format!("# WARNING: this installer is experimental\ncurl --proto '=https' --tlsv1.2 -L -sSf {repo_url}/releases/download/{app_version}/installer.sh | sh");
                    let desc = "Install prebuilt binaries via shell script".to_owned();
                    installer_impl = InstallerImpl::GithubShell(InstallerInfo {
                        app_name: app_name.clone(),
                        app_version,
                        repo_url: repo_url.to_owned(),
                        hint,
                        desc,
                    });
                }
                InstallerStyle::GithubPowershell => {
                    let Some(repo_url) = repo else {
                        warn!("skipping --installer=github-powershell: 'repository' isn't set in Cargo.toml");
                        continue;
                    };
                    file_name = "installer.ps1".to_owned();
                    file_path = dist_dir.join(&file_name);
                    // Currently hardcoded to this one windows triple
                    target_triples = vec!["x86_64-pc-windows-msvc".to_owned()];
                    let app_version = format!("v{version}");
                    let hint = format!("# WARNING: this installer is experimental\nirm '{repo_url}/releases/download/{app_version}/installer.ps1' | iex");
                    let desc = "Install prebuilt binaries via powershell script".to_owned();
                    installer_impl = InstallerImpl::GithubPowershell(InstallerInfo {
                        app_name: app_name.clone(),
                        app_version,
                        repo_url: repo_url.to_owned(),
                        hint,
                        desc,
                    });
                }
            }

            let installer_artifact_idx = ArtifactTargetIdx(artifacts.len());
            artifacts.push(ArtifactTarget {
                target_triples,
                dir_name: None,
                dir_path: None,
                file_path,
                file_name,
                built_assets: HashMap::new(),
                bundle: BundleStyle::Installer(installer_impl),
                static_assets: vec![],
                kind: ArtifactKind::Installer,
            });
            release.artifacts.push(installer_artifact_idx);
        }
    }

    // Add release notes
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
        for artifact_idx in &release.artifacts {
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

    let releases = releases.into_iter().map(|e| e.1 .1).collect();
    Ok(DistGraph {
        cargo,
        target_dir,
        workspace_dir,
        dist_dir,
        targets,
        built_assets,
        artifacts,
        releases,
    })
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
    let root_package = members
        .packages(DependencyDirection::Forward)
        .find(|p| p.manifest_path() == manifest_path);

    let workspace_root = workspace.root();
    let mut package_info = HashMap::new();
    for package in members.packages(DependencyDirection::Forward) {
        let info = compute_package_info(workspace_root, &package)?;
        package_info.insert(package.id(), info);
    }

    Ok(WorkspaceInfo {
        info: workspace,
        members,
        package_info,
        manifest_path,
        root_package,
    })
}

fn compute_package_info(
    workspace_root: &Utf8Path,
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

    let mut info = PackageInfo {
        name: package.name().to_owned(),
        version: package.version().to_owned(),
        description: package.description().map(ToOwned::to_owned),
        authors: package.authors().to_vec(),
        license: package.license().map(ToOwned::to_owned),
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

/// Get all the artifacts built by this list of cargo packages
fn binaries_for_cargo_packages<'a>(
    target_idx: BuildTargetIdx,
    packages: impl IntoIterator<Item = PackageMetadata<'a>>,
) -> Vec<ExecutableBuiltAsset> {
    let mut built_assets = Vec::new();
    for package in packages {
        for target in package.build_targets() {
            let build_id = target.id();
            if let BuildTargetId::Binary(name) = build_id {
                built_assets.push(ExecutableBuiltAsset {
                    exe_name: name.to_owned(),
                    package_id: package.id().clone(),
                    build_target: target_idx,
                    // This will be filled in later
                    symbols_artifact: None,
                });
            }
        }
    }
    built_assets
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
