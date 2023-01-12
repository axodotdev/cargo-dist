//! # cargo-dist
//!
//!

#![allow(clippy::single_match)]
#![allow(dead_code)]

use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Read},
    process::Command,
};

use camino::{Utf8Path, Utf8PathBuf};
use cargo_dist_schema::{
    Artifact, ArtifactKind, Asset, AssetKind, DistManifest, ExecutableAsset, Release,
};
use flate2::{write::ZlibEncoder, Compression, GzBuilder};
use guppy::{
    graph::{
        BuildTargetId, DependencyDirection, PackageGraph, PackageMetadata, PackageSet, Workspace,
    },
    MetadataCommand, PackageId,
};
use semver::Version;
use serde::Deserialize;
use tracing::{info, warn};
use xz2::write::XzEncoder;
use zip::ZipWriter;

use errors::*;
use miette::{miette, Context, IntoDiagnostic};

pub mod ci;
pub mod errors;
pub mod installer;
#[cfg(test)]
mod tests;

/// Key in workspace.metadata or package.metadata for our config
const METADATA_DIST: &str = "dist";
/// Dir in target/ for us to build our packages in
/// NOTE: DO NOT GIVE THIS THE SAME NAME AS A PROFILE!
const TARGET_DIST: &str = "distrib";
/// The profile we will build with
const PROFILE_DIST: &str = "dist";
/// Some files we'll try to grab.
//TODO: LICENSE-* files, somehow!
const BUILTIN_FILES: &[&str] = &["README.md", "CHANGELOG.md", "RELEASES.md"];

/// The key for referring to linux as an "os"
const OS_LINUX: &str = "linux";
/// The key for referring to macos as an "os"
const OS_MACOS: &str = "macos";
/// The key for referring to windows as an "os"
const OS_WINDOWS: &str = "windows";

/// The key for referring to 64-bit x86_64 (AKA amd64) as an "cpu"
const CPU_X64: &str = "x86_64";
/// The key for referring to 32-bit x86 (AKA i686) as an "cpu"
const CPU_X86: &str = "x86";
/// The key for referring to 64-bit arm64 (AKA aarch64) as an "cpu"
const CPU_ARM64: &str = "arm64";
/// The key for referring to 32-bit arm as an "cpu"
const CPU_ARM: &str = "arm";

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

#[derive(Clone, Copy, Debug)]
pub enum CiStyle {
    Github,
}

#[derive(Clone, Copy, Debug)]
pub enum InstallerStyle {
    GithubShell,
    GithubPowershell,
}

/// A unique id for a [`BuildTarget`][]
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
struct BuildTargetIdx(usize);

/// A unique id for a [`BuiltAsset`][]
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
struct BuiltAssetIdx(usize);

/// A unique id for a [`ArtifactTarget`][]
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
struct ArtifactTargetIdx(usize);

/// The graph of all work that cargo-dist needs to do on this invocation.
///
/// All work is precomputed at the start of execution because only discovering
/// what you need to do in the middle of building/packing things is a mess.
/// It also allows us to report what *should* happen without actually doing it.
#[derive(Debug)]
struct DistGraph {
    /// The executable cargo told us to find itself at.
    cargo: String,
    /// The cargo target dir.
    target_dir: Utf8PathBuf,
    /// The root directory of the current cargo workspace.
    workspace_dir: Utf8PathBuf,
    /// cargo-dist's target dir (generally nested under `target_dir`).
    dist_dir: Utf8PathBuf,
    /// Targets we need to build
    targets: Vec<BuildTarget>,
    /// Assets we want to get out of builds
    built_assets: Vec<BuiltAsset>,
    /// Distributable artifacts we want to produce for the releases
    artifacts: Vec<ArtifactTarget>,
    /// Logical releases that artifacts are grouped under
    releases: Vec<ReleaseTarget>,
}

/// A build we need to perform to get artifacts to distribute.
#[derive(Debug)]
enum BuildTarget {
    /// A cargo build
    Cargo(CargoBuildTarget),
    // Other build systems..?
}

/// A cargo build
#[derive(Debug)]
struct CargoBuildTarget {
    /// The --target triple to pass
    target_triple: String,
    /// The feature flags to pass
    features: CargoTargetFeatures,
    /// What package to build (or "the workspace")
    package: CargoTargetPackages,
    /// The --profile to pass
    profile: String,
    /// Assets we expect from this build
    expected_assets: Vec<BuiltAssetIdx>,
}

/// An asset we need from our builds
#[derive(Debug)]
enum BuiltAsset {
    /// An executable
    Executable(ExecutableBuiltAsset),
    /// Symbols for an executable
    Symbols(SymbolsBuiltAsset),
}

#[derive(Copy, Clone, Debug)]
enum SymbolKind {
    Pdb,
    Dsym,
    Dwp,
}

impl SymbolKind {
    fn ext(self) -> &'static str {
        match self {
            SymbolKind::Pdb => "pdb",
            SymbolKind::Dsym => "dSYM",
            SymbolKind::Dwp => "dwp",
        }
    }
}

/// An executable we need from our builds
#[derive(Debug)]
struct ExecutableBuiltAsset {
    /// The name of the executable (without a file extension)
    exe_name: String,
    /// The cargo package this executable is defined by
    package_id: PackageId,
    /// The [`BuildTarget`][] that should produce this.
    build_target: BuildTargetIdx,
    /// The artifact containing symbols for this
    symbols_artifact: Option<ArtifactTargetIdx>,
}

/// Symbols we need from our builds
#[derive(Debug)]
struct SymbolsBuiltAsset {
    /// The name of the executable these symbols are for (without a file extension)
    exe_name: String,
    /// The cargo package this executable is defined by
    package_id: PackageId,
    /// The [`BuildTarget`][] that should produce this.
    build_target: BuildTargetIdx,
    /// The kind of symbols this is
    symbol_kind: SymbolKind,
}

/// A distributable artifact we want to build
#[derive(Debug)]
pub(crate) struct ArtifactTarget {
    /// The target platform
    ///
    /// i.e. `x86_64-pc-windows-msvc`
    target_triples: Vec<String>,
    /// The name of the directory this artifact's contents will be stored in (if necessary).
    ///
    /// This directory is technically a transient thing but it will show up as the name of
    /// the directory in a `tar`. Single file artifacts don't need this.
    ///
    /// i.e. `cargo-dist-v0.1.0-x86_64-pc-windows-msvc`
    dir_name: Option<String>,
    /// The path of the directory this artifact's contents will be stored in (if necessary).
    ///
    /// i.e. `/.../target/dist/cargo-dist-v0.1.0-x86_64-pc-windows-msvc/`
    dir_path: Option<Utf8PathBuf>,
    /// The file name of the artifact when it's completed and placed in the root of the dist dir.
    ///
    /// i.e. `cargo-dist-v0.1.0-x86_64-pc-windows-msvc.zip`
    pub(crate) file_name: String,
    /// The path where the final artifact will appear in the dist dir.
    ///
    /// i.e. `/.../target/dist/cargo-dist-v0.1.0-x86_64-pc-windows-msvc.zip`
    pub(crate) file_path: Utf8PathBuf,
    /// The bundling method (zip, tar.gz, ...)
    bundle: BundleStyle,
    /// The built assets this artifact will contain
    ///
    /// i.e. `cargo-dist.exe`
    built_assets: HashMap<BuiltAssetIdx, Utf8PathBuf>,
    /// Additional static assets to add to the artifact
    ///
    /// i.e. `README.md`
    static_assets: Vec<Utf8PathBuf>,
    /// The kind of artifact this is
    kind: ArtifactKind,
}

/// A logical release of an application that artifacts are grouped under
#[derive(Debug)]
struct ReleaseTarget {
    /// The name of the app
    app_name: String,
    /// The version of the app
    version: Version,
    /// The artifacts this release includes
    artifacts: Vec<ArtifactTargetIdx>,
}

/// The style of bundle for a [`ArtifactTarget`][].
#[derive(Debug)]
enum BundleStyle {
    /// Just a single uncompressed file
    UncompressedFile,
    /// `.zip`
    Zip,
    /// `.tar.<compression>`
    Tar(CompressionImpl),
    Installer(InstallerImpl),
    // TODO: Microsoft MSI installer
    // TODO: Apple .dmg "installer"
    // TODO: flatpak?
    // TODO: snap? (ostensibly "obsoleted" by flatpak)
    // TODO: various linux package manager manifests? (.deb, .rpm, ... do these make sense?)
}

/// Compression impls (used by [`BundleStyle::Tar`][])
#[derive(Debug, Copy, Clone)]
enum CompressionImpl {
    /// `.gz`
    Gzip,
    /// `.xz`
    Xzip,
    /// `.zstd`
    Zstd,
}

#[derive(Debug, Clone)]
enum InstallerImpl {
    GithubShell(InstallerInfo),
    GithubPowershell(InstallerInfo),
}

#[derive(Debug, Clone)]
pub(crate) struct InstallerInfo {
    pub(crate) app_name: String,
    pub(crate) app_version: String,
    pub(crate) repo_url: String,
}

/// Cargo features a [`CargoBuildTarget`][] should use.
#[derive(Debug)]
struct CargoTargetFeatures {
    /// Whether to disable default features
    no_default_features: bool,
    /// Features to enable
    features: CargoTargetFeatureList,
}

/// A list of features to build with
#[derive(Debug)]
enum CargoTargetFeatureList {
    /// All of them
    All,
    /// Some of them
    List(Vec<String>),
}

/// Whether to build a package or workspace
#[derive(Debug)]
enum CargoTargetPackages {
    /// Build the workspace
    Workspace,
    /// Just build a package
    Package(PackageId),
}

/// Top level command of cargo_dist -- do everything!
pub fn do_dist(cfg: &Config) -> Result<DistManifest> {
    let dist = gather_work(cfg)?;

    // TODO: parallelize this by working this like a dependency graph, so we can start
    // bundling up an executable the moment it's built!

    // First set up our target dirs so things don't have to race to do it later
    if !dist.dist_dir.exists() {
        std::fs::create_dir_all(&dist.dist_dir)
            .into_diagnostic()
            .wrap_err_with(|| format!("couldn't create dist target dir at {}", dist.dist_dir))?;
    }

    for artifact in &dist.artifacts {
        eprintln!("bundling {}", artifact.file_name);
        init_artifact_dir(&dist, artifact)?;
    }

    let mut built_assets = HashMap::new();
    if cfg.build {
        // Run all the builds
        for target in &dist.targets {
            let new_built_assets = build_target(&dist, target)?;
            // Copy the artifacts as soon as possible, future builds may clobber them!
            for (&built_asset_idx, built_asset) in &new_built_assets {
                populate_artifact_dirs_with_built_assets(&dist, built_asset_idx, built_asset)?;
            }
            built_assets.extend(new_built_assets);
        }
    }

    // Build all the bundles
    for artifact in &dist.artifacts {
        populate_artifact_dir_with_static_assets(&dist, artifact)?;
        bundle_artifact(&dist, artifact)?;
        eprintln!("bundled {}", artifact.file_path);
    }

    Ok(build_manifest(cfg, &dist))
}

/// Just generate the manifest produced by `cargo dist build` without building
pub fn do_manifest(cfg: &Config) -> Result<DistManifest> {
    let dist = gather_work(cfg)?;
    Ok(build_manifest(cfg, &dist))
}

/// Precompute all the work this invocation will need to do
fn gather_work(cfg: &Config) -> Result<DistGraph> {
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
        // TODO: allow apps to be versioned separately from packages?
        let version = pkg_graph
            .metadata(&exe.package_id)
            .unwrap()
            .version()
            .clone();
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
        let exe_static_assets = BUILTIN_FILES
            .iter()
            .filter_map(|f| {
                let file = workspace_dir.join(f);
                file.exists().then_some(file)
            })
            .collect();

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
        let repo = pkg_graph
            .metadata(package_id)
            .ok()
            .and_then(|pkg| pkg.repository());
        for installer in &cfg.installers {
            let file_path;
            let file_name;
            let installer_impl;
            let target_triples;

            match installer {
                InstallerStyle::GithubShell => {
                    file_name = "installer.sh".to_owned();
                    file_path = dist_dir.join(&file_name);
                    // All the triples we know about, sans windows (windows-gnu isn't handled...)
                    target_triples = triples
                        .iter()
                        .filter(|s| !s.contains("windows"))
                        .cloned()
                        .collect::<Vec<_>>();
                    installer_impl = InstallerImpl::GithubShell(InstallerInfo {
                        app_name: app_name.clone(),
                        app_version: format!("v{version}"),
                        repo_url: repo.unwrap_or("").to_owned(),
                    });
                }
                InstallerStyle::GithubPowershell => {
                    file_name = "installer.ps1".to_owned();
                    file_path = dist_dir.join(&file_name);
                    // Currently hardcoded to this one windows triple
                    target_triples = vec!["x86_64-pc-windows-msvc".to_owned()];
                    installer_impl = InstallerImpl::GithubPowershell(InstallerInfo {
                        app_name: app_name.clone(),
                        app_version: format!("v{version}"),
                        repo_url: repo.unwrap_or("").to_owned(),
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

fn build_manifest(cfg: &Config, dist: &DistGraph) -> DistManifest {
    // Report the releases
    let mut releases = vec![];
    for release in &dist.releases {
        releases.push(Release {
            app_name: release.app_name.clone(),
            app_version: release.version.to_string(),
            artifacts: release
                .artifacts
                .iter()
                .map(|artifact_idx| {
                    let artifact = &dist.artifacts[artifact_idx.0];
                    Artifact {
                        name: artifact.file_path.file_name().unwrap().to_owned(),
                        path: if cfg.no_local_paths {
                            None
                        } else {
                            Some(artifact.file_path.clone().into_std_path_buf())
                        },
                        target_triples: artifact.target_triples.clone(),
                        install_hint: match &artifact.bundle {
                            BundleStyle::Installer(InstallerImpl::GithubShell(info)) => {
                                let InstallerInfo { repo_url, app_version, .. } = info;
                                let install_unix = format!("curl --proto '=https' --tlsv1.2 -L -sSf {repo_url}/releases/download/{app_version}/installer.sh | sh");
                                Some(install_unix)
                            }
                            BundleStyle::Installer(InstallerImpl::GithubPowershell(info)) => {
                                let InstallerInfo { repo_url, app_version, .. } = info;
                                let install_windows = format!("irm '{repo_url}/releases/download/{app_version}/installer.ps1' | iex");
                                Some(install_windows)
                            }
                            BundleStyle::Zip => None,
                            BundleStyle::Tar(_) => None,
                            BundleStyle::UncompressedFile => None,
                        },
                        assets: artifact
                            .built_assets
                            .iter()
                            .filter_map(|(asset_idx, asset_path)| {
                                let asset = &dist.built_assets[asset_idx.0];
                                match asset {
                                    BuiltAsset::Executable(exe) => {
                                        let symbols_artifact = exe.symbols_artifact.map(|a| {
                                            dist.artifacts[a.0]
                                                .file_path
                                                .file_name()
                                                .unwrap()
                                                .to_owned()
                                        });
                                        Some(Asset {
                                            name: exe.exe_name.clone(),
                                            path: asset_path.clone().into_std_path_buf(),
                                            kind: AssetKind::Executable(ExecutableAsset {
                                                symbols_artifact,
                                            }),
                                        })
                                    }
                                    BuiltAsset::Symbols(_sym) => {
                                        // Symbols are their own assets, so no need to report
                                        None
                                    }
                                }
                            })
                            .collect(),
                        kind: artifact.kind.clone(),
                    }
                })
                .collect(),
        })
    }
    DistManifest::new(releases)
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

/// Get the host target triple from cargo
fn get_host_target(cargo: &str) -> Result<String> {
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

/// Build a target
fn build_target(
    dist_graph: &DistGraph,
    target: &BuildTarget,
) -> Result<HashMap<BuiltAssetIdx, Utf8PathBuf>> {
    match target {
        BuildTarget::Cargo(target) => build_cargo_target(dist_graph, target),
    }
}

/// Build a cargo target
fn build_cargo_target(
    dist_graph: &DistGraph,
    target: &CargoBuildTarget,
) -> Result<HashMap<BuiltAssetIdx, Utf8PathBuf>> {
    eprintln!(
        "building cargo target ({}/{})",
        target.target_triple, target.profile
    );
    // Run the build
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
    // For similar reasons we may want to perfer targetting "linux-musl" over
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

    if target.target_triple.contains("windows-msvc") {
        rustflags.push_str(" -Ctarget-feature=+crt-static");
    }

    // TODO: maybe set RUSTFLAGS="-Cforce-frame-pointers=yes"
    //
    // On linux and macos this can make the unwind tables (debuginfo) smaller, more reliable,
    // and faster at minimal runtime cost (these days). This can be a big win for profilers
    // and crash reporters, which both want to unwind in "weird" places quickly and reliably.
    //
    // On windows this setting is unfortunately useless because Microsoft specified
    // it to be... Wrong. Specifically it points "somewhere" in the frame (instead of
    // at the start), and exists only to enable things like -Oz.
    // See: https://github.com/rust-lang/rust/issues/82333

    // TODO: maybe set RUSTFLAGS="-Csymbol-mangling-version=v0"
    // See: https://github.com/rust-lang/rust/issues/60705
    //
    // Despite the name, v0 is actually the *second* mangling format for Rust symbols.
    // The first was more unprincipled and adhoc, and is just the unnamed current
    // default. In the future v0 should become the default. Currently we're waiting
    // for as many tools as possible to add support (and then make it onto dev machines).
    //
    // The v0 scheme is bigger and contains more rich information (with its own fancy
    // compression scheme to try to compensate). Unclear on the exact pros/cons of
    // opting into it earlier.

    // TODO: is there *any* world where we can help the user use Profile Guided Optimization (PGO)?
    // See: https://doc.rust-lang.org/rustc/profile-guided-optimization.html
    // See: https://blog.rust-lang.org/inside-rust/2020/11/11/exploring-pgo-for-the-rust-compiler.html
    //
    // In essence PGO is a ~three-step process:
    //
    // 1. Build your program
    // 2. Run it on a "representative" workload and record traces of the execution ("a profile")
    // 3. Rebuild your program with the profile to Guide Optimization
    //
    // For instance the compiler might see that a certain branch (if) always goes one way
    // in the profile, and optimize the code to go faster if that holds true (by say outlining
    // the other path).
    //
    // PGO can get *huge* wins but is at the mercy of step 2, which is difficult/impossible
    // for a tool like cargo-dist to provide "automatically". But maybe we can streamline
    // some of the rough edges? This is also possibly a place where A Better Telemetry Solution
    // could do some interesting things for dev-controlled production environments.

    // TODO: can we productively use RUSTFLAGS="--remap-path-prefix"?
    // See: https://doc.rust-lang.org/rustc/command-line-arguments.html#--remap-path-prefix-remap-source-names-in-output
    // See: https://github.com/rust-lang/rust/issues/87805
    //
    // Compiler toolchains like stuffing absolute host system paths in metadata/debuginfo,
    // which can make things Bigger and also leak a modicum of private info. This flag
    // lets you specify a rewrite rule for a prefix of the path, letting you map e.g.
    // "C:\Users\Aria\checkouts\cargo-dist\src\main.rs" to ".\cargo-dist\src\main.rs".
    //
    // Unfortunately this is a VERY blunt instrument which does legit exact string matching
    // and can miss paths in places rustc doesn't Expect/See. Still it might be worth
    // setting it in case it Helps?

    let mut command = Command::new(&dist_graph.cargo);
    command
        .arg("build")
        .arg("--profile")
        .arg(&target.profile)
        .arg("--message-format=json")
        .arg("--target")
        .arg(&target.target_triple)
        .env("RUSTFLAGS", rustflags)
        .stdout(std::process::Stdio::piped());
    if target.features.no_default_features {
        command.arg("--no-default-features");
    }
    match &target.features.features {
        CargoTargetFeatureList::All => {
            command.arg("--all-features");
        }
        CargoTargetFeatureList::List(features) => {
            if !features.is_empty() {
                command.arg("--features");
                for feature in features {
                    command.arg(feature);
                }
            }
        }
    }
    match &target.package {
        CargoTargetPackages::Workspace => {
            command.arg("--workspace");
        }
        CargoTargetPackages::Package(package) => {
            command.arg("--package").arg(package.to_string());
        }
    }
    info!("exec: {:?}", command);
    let mut task = command
        .spawn()
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to exec cargo build: {command:?}"))?;

    // Create entries for all the binaries we expect to find with empty paths
    // we'll fail if any are still empty at the end!
    let mut expected_exes = HashMap::<String, HashMap<String, (BuiltAssetIdx, Utf8PathBuf)>>::new();
    let mut expected_symbols =
        HashMap::<String, HashMap<String, (BuiltAssetIdx, Utf8PathBuf)>>::new();
    for asset_idx in &target.expected_assets {
        let asset = &dist_graph.built_assets[asset_idx.0];
        match asset {
            BuiltAsset::Executable(exe) => {
                let package_id = exe.package_id.to_string();
                let exe_name = exe.exe_name.clone();
                expected_exes
                    .entry(package_id)
                    .or_default()
                    .insert(exe_name, (*asset_idx, Utf8PathBuf::new()));
            }
            BuiltAsset::Symbols(sym) => {
                let package_id = sym.package_id.to_string();
                let exe_name = sym.exe_name.clone();
                expected_symbols
                    .entry(package_id)
                    .or_default()
                    .insert(exe_name, (*asset_idx, Utf8PathBuf::new()));
            }
        }
    }

    // Collect up the compiler messages to find out where binaries ended up
    let reader = std::io::BufReader::new(task.stdout.take().unwrap());
    for message in cargo_metadata::Message::parse_stream(reader) {
        let Ok(message) = message.into_diagnostic().wrap_err("failed to parse cargo json message").map_err(|e| warn!("{:?}", e)) else {
            // It's ok for there to be messages we don't understand if we don't care about them.
            // At the end we'll check if we got the messages we *do* need.
            continue;
        };
        match message {
            cargo_metadata::Message::CompilerArtifact(artifact) => {
                // Hey we got an executable, is it one we wanted?
                if let Some(new_exe) = artifact.executable {
                    info!("got a new exe: {}", new_exe);
                    let package_id = artifact.package_id.to_string();
                    let exe_name = new_exe.file_stem().unwrap();

                    // If we expected some symbols, pull them out of the paths of this executable
                    let expected_sym = expected_symbols
                        .get_mut(&package_id)
                        .and_then(|m| m.get_mut(exe_name));
                    if let Some((expected_sym_asset, sym_path)) = expected_sym {
                        let expected_sym_asset = &dist_graph.built_assets[expected_sym_asset.0];
                        let BuiltAsset::Symbols(expected_sym_asset) = expected_sym_asset else {
                            unreachable!()
                        };
                        for path in artifact.filenames {
                            let is_symbols = path
                                .extension()
                                .map(|e| e == expected_sym_asset.symbol_kind.ext())
                                .unwrap_or(false);
                            if is_symbols {
                                // These are symbols we expected! Save the path.
                                *sym_path = path;
                            }
                        }
                    }

                    // Get the exe path
                    let expected_exe = expected_exes
                        .get_mut(&package_id)
                        .and_then(|m| m.get_mut(exe_name));
                    if let Some(expected) = expected_exe {
                        // This is an exe we expected! Save the path.
                        expected.1 = new_exe;
                    }
                }
            }
            _ => {
                // Nothing else interesting?
            }
        }
    }

    // Check that we got everything we expected, and normalize to ArtifactIdx => Artifact Path
    let mut built_assets = HashMap::new();
    for (package_id, exes) in expected_exes {
        for (exe, (artifact_idx, exe_path)) in exes {
            if exe_path.as_str().is_empty() {
                return Err(miette!("failed to find bin {} for {}", exe, package_id));
            }
            built_assets.insert(artifact_idx, exe_path);
        }
    }
    for (package_id, symbols) in expected_symbols {
        for (exe, (artifact_idx, sym_path)) in symbols {
            if sym_path.as_str().is_empty() {
                return Err(miette!("failed to find symbols {} for {}", exe, package_id));
            }
            built_assets.insert(artifact_idx, sym_path);
        }
    }

    Ok(built_assets)
}

/// Initialize the dir for an artifact (and delete the old artifact file).
fn init_artifact_dir(_dist: &DistGraph, artifact: &ArtifactTarget) -> Result<()> {
    // Delete any existing bundle
    if artifact.file_path.exists() {
        std::fs::remove_file(&artifact.file_path)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to delete old artifact {}", artifact.file_path))?;
    }

    let Some(artifact_dir_path) = &artifact.dir_path else {
        // If there's no dir than we're done
        return Ok(());
    };
    info!("recreating artifact dir: {artifact_dir_path}");

    // Clear out the dir we'll build the bundle up in
    if artifact_dir_path.exists() {
        std::fs::remove_dir_all(artifact_dir_path)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to delete old artifact dir {artifact_dir_path}"))?;
    }
    std::fs::create_dir(artifact_dir_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to create artifact dir {artifact_dir_path}"))?;

    Ok(())
}

fn populate_artifact_dirs_with_built_assets(
    dist: &DistGraph,
    built_asset_idx: BuiltAssetIdx,
    built_asset_path: &Utf8Path,
) -> Result<()> {
    for artifact in &dist.artifacts {
        if let Some(rel_asset_path) = artifact.built_assets.get(&built_asset_idx) {
            let bundled_asset = if let BundleStyle::UncompressedFile = artifact.bundle {
                // If the asset is a single uncompressed file, we can just copy it to its final dest
                info!("  copying {built_asset_path} to {}", artifact.file_path);
                artifact.file_path.clone()
            } else {
                let artifact_dir_path = artifact
                    .dir_path
                    .as_ref()
                    .expect("compressed bundle didn't have a dir path?!");
                info!("  adding {built_asset_path} to {}", artifact_dir_path);
                artifact_dir_path.join(rel_asset_path)
            };

            std::fs::copy(built_asset_path, &bundled_asset)
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!(
                        "failed to copy built asset to artifact: {built_asset_path} => {bundled_asset}"
                    )
                })?;
        }
    }
    Ok(())
}

fn populate_artifact_dir_with_static_assets(
    _dist: &DistGraph,
    artifact: &ArtifactTarget,
) -> Result<()> {
    let Some(artifact_dir_path) = &artifact.dir_path else {
        assert!(artifact.static_assets.is_empty(), "had static assets but didn't have a dir path?!");
        // If there's no dir than we're done
        return Ok(());
    };

    info!("populating artifact dir: {}", artifact_dir_path);
    // Copy assets
    for asset in &artifact.static_assets {
        let asset_file_name = asset.file_name().unwrap();
        let bundled_asset = artifact_dir_path.join(asset_file_name);
        info!("  adding {bundled_asset}");
        std::fs::copy(asset, &bundled_asset)
            .into_diagnostic()
            .wrap_err_with(|| {
                format!("failed to copy bundled asset to artifact: {asset} => {bundled_asset}")
            })?;
    }

    Ok(())
}

fn bundle_artifact(dist_graph: &DistGraph, artifact: &ArtifactTarget) -> Result<()> {
    info!("bundling artifact: {}", artifact.file_path);
    match &artifact.bundle {
        BundleStyle::Zip => zip_artifact(dist_graph, artifact),
        BundleStyle::Tar(compression) => tar_artifact(dist_graph, artifact, compression),
        BundleStyle::Installer(installer) => generate_installer(dist_graph, artifact, installer),
        BundleStyle::UncompressedFile => {
            // Already handled by populate_artifact_dirs_with_built_assets
            info!("artifact created at: {}", artifact.file_path);
            Ok(())
        }
    }
}

fn tar_artifact(
    _dist_graph: &DistGraph,
    artifact: &ArtifactTarget,
    compression: &CompressionImpl,
) -> Result<()> {
    // Set up the archive/compression
    // The contents of the zip (e.g. a tar)
    let artifact_dir_path = artifact.dir_path.as_ref().unwrap();
    let artifact_dir_name = &artifact.dir_name.as_ref().unwrap();
    let zip_contents_name = format!("{artifact_dir_name}.tar");
    let final_zip_path = &artifact.file_path;
    let final_zip_file = File::create(final_zip_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to create file for artifact: {final_zip_path}"))?;

    match compression {
        CompressionImpl::Gzip => {
            // Wrap our file in compression
            let zip_output = GzBuilder::new()
                .filename(zip_contents_name)
                .write(final_zip_file, Compression::default());

            // Write the tar to the compression stream
            let mut tar = tar::Builder::new(zip_output);

            // Add the whole dir to the tar
            tar.append_dir_all(artifact_dir_name, artifact_dir_path)
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!(
                        "failed to copy directory into tar: {artifact_dir_path} => {artifact_dir_name}",
                    )
                })?;
            // Finish up the tarring
            let zip_output = tar
                .into_inner()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write tar: {final_zip_path}"))?;
            // Finish up the compression
            let _zip_file = zip_output
                .finish()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write archive: {final_zip_path}"))?;
            // Drop the file to close it
        }
        CompressionImpl::Xzip => {
            let zip_output = XzEncoder::new(final_zip_file, 9);
            // Write the tar to the compression stream
            let mut tar = tar::Builder::new(zip_output);

            // Add the whole dir to the tar
            tar.append_dir_all(artifact_dir_name, artifact_dir_path)
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!(
                        "failed to copy directory into tar: {artifact_dir_path} => {artifact_dir_name}",
                    )
                })?;
            // Finish up the tarring
            let zip_output = tar
                .into_inner()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write tar: {final_zip_path}"))?;
            // Finish up the compression
            let _zip_file = zip_output
                .finish()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write archive: {final_zip_path}"))?;
            // Drop the file to close it
        }
        CompressionImpl::Zstd => {
            // Wrap our file in compression
            let zip_output = ZlibEncoder::new(final_zip_file, Compression::default());

            // Write the tar to the compression stream
            let mut tar = tar::Builder::new(zip_output);

            // Add the whole dir to the tar
            tar.append_dir_all(artifact_dir_name, artifact_dir_path)
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!(
                        "failed to copy directory into tar: {artifact_dir_path} => {artifact_dir_name}",
                    )
                })?;
            // Finish up the tarring
            let zip_output = tar
                .into_inner()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write tar: {final_zip_path}"))?;
            // Finish up the compression
            let _zip_file = zip_output
                .finish()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write archive: {final_zip_path}"))?;
            // Drop the file to close it
        }
    }

    info!("artifact created at: {}", final_zip_path);
    Ok(())
}

fn zip_artifact(_dist_graph: &DistGraph, artifact: &ArtifactTarget) -> Result<()> {
    // Set up the archive/compression
    let artifact_dir_path = artifact.dir_path.as_ref().unwrap();
    let final_zip_path = &artifact.file_path;
    let final_zip_file = File::create(final_zip_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to create file for artifact: {final_zip_path}"))?;

    // Wrap our file in compression
    let mut zip = ZipWriter::new(final_zip_file);

    let dir = std::fs::read_dir(artifact_dir_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to read artifact dir: {artifact_dir_path}"))?;
    for entry in dir {
        let entry = entry.into_diagnostic()?;
        if entry.file_type().into_diagnostic()?.is_file() {
            let options = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            let file = File::open(entry.path()).into_diagnostic()?;
            let mut buf = BufReader::new(file);
            let file_name = entry.file_name();
            // TODO: ...don't do this lossy conversion?
            let utf8_file_name = file_name.to_string_lossy();
            zip.start_file(utf8_file_name.clone(), options)
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!("failed to create file {utf8_file_name} in zip: {final_zip_path}")
                })?;
            std::io::copy(&mut buf, &mut zip).into_diagnostic()?;
        } else {
            panic!("TODO: implement zip subdirs! (or was this a symlink?)");
        }
    }

    // Finish up the compression
    let _zip_file = zip
        .finish()
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to write archive: {final_zip_path}"))?;
    // Drop the file to close it
    info!("artifact created at: {}", final_zip_path);
    Ok(())
}

/// Get the path/command to invoke Cargo
fn cargo() -> Result<String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    Ok(cargo)
}

/// Get the PackageGraph for the current workspace
fn package_graph(cargo: &str) -> Result<PackageGraph> {
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

/// Info on the current workspace
struct WorkspaceInfo<'pkg_graph> {
    /// Most info on the workspace.
    info: Workspace<'pkg_graph>,
    /// The workspace members.
    members: PackageSet<'pkg_graph>,
    /// Path to the Cargo.toml of the workspace (may be a package's Cargo.toml)
    manifest_path: Utf8PathBuf,
    /// If the manifest_path points to a package, this is the one.
    ///
    /// If this is None, the workspace Cargo.toml is a virtual manifest.
    root_package: Option<PackageMetadata<'pkg_graph>>,
}

/// Computes [`WorkspaceInfo`][] for the current workspace.
fn workspace_info(pkg_graph: &PackageGraph) -> Result<WorkspaceInfo> {
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

    Ok(WorkspaceInfo {
        info: workspace,
        members,
        manifest_path,
        root_package,
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

#[derive(Debug)]
pub struct InitArgs {
    pub ci_styles: Vec<CiStyle>,
}

/// Run 'cargo dist init'
pub fn do_init(cfg: &Config, args: &InitArgs) -> Result<()> {
    let cargo = cargo()?;
    let pkg_graph = package_graph(&cargo)?;
    let workspace = workspace_info(&pkg_graph)?;

    // Load in the workspace toml to edit and write back
    let mut workspace_toml = {
        let mut workspace_toml_file = File::open(&workspace.manifest_path)
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
            .wrap_err("couldn't parse root workspace Cargo.toml")?
    };

    // Setup the [profile.dist]
    {
        let profiles = workspace_toml["profile"].or_insert(toml_edit::table());
        if let Some(t) = profiles.as_table_mut() {
            t.set_implicit(true)
        }
        let dist_profile = &mut profiles[PROFILE_DIST];
        if !dist_profile.is_none() {
            return Err(miette!(
                "already init! (based on [profile.dist] existing in your Cargo.toml)"
            ));
        }
        let mut new_profile = toml_edit::table();
        {
            let new_profile = new_profile.as_table_mut().unwrap();
            // We're building for release, so this is a good base!
            new_profile.insert("inherits", toml_edit::value("release"));
            // We want *full* debuginfo for good crashreporting/profiling
            // This doesn't bloat the final binary because we use split-debuginfo below
            new_profile.insert("debug", toml_edit::value(true));
            // Ensure that all debuginfo is pulled out of the binary and tossed
            // into a separate file from the final binary. This should ideally be
            // uploaded to something like a symbol server to be fetched on demand.
            // This is already the default on windows (.pdb) and macos (.dsym) but
            // is rather bleeding on other platforms (.dwp) -- it requires Rust 1.65,
            // which as of this writing in the latest stable rust release! If anyone
            // ever makes a big deal with building final binaries with an older MSRV
            // we may need to more intelligently select this.
            new_profile.insert("split-debuginfo", toml_edit::value("packed"));

            // TODO: set codegen-units=1? (Probably Not!)
            //
            // Ok so there's an inherent tradeoff in compilers where if the compiler does
            // everything in a very serial/global way, it can discover more places where
            // optimizations can be done and theoretically make things faster/smaller
            // using all the information at its fingertips... at the cost of your builds
            // taking forever. Compiler optimizations generally take super-linear time,
            // so if you let the compiler see and think about EVERYTHING your builds
            // can literally take *days* for codebases on the order of LLVM itself.
            //
            // To keep compile times tractable, we generally break up the program
            // into "codegen units" (AKA "translation units") that get compiled
            // independently and then combined by the linker. This keeps the super-linear
            // scaling under control, but prevents optimizations like inlining across
            // units. (This process is why we have things like "object files" and "rlibs",
            // those are the intermediate artifacts fed to the linker.)
            //
            // Compared to C, Rust codegen units are quite monolithic. Where each C
            // *file* might gets its own codegen unit, Rust prefers scoping them to
            // an entire *crate*.  As it turns out, neither of these answers is right in
            // all cases, and being able to tune the unit size is useful.
            //
            // Large C++ codebases like Firefox have "unified" builds where they basically
            // concatenate files together to get bigger units. Rust provides the
            // opposite: the codegen-units=N option tells rustc that it should try to
            // break up a crate into at most N different units. This is done with some
            // heuristics and contraints to try to still get the most out of each unit
            // (i.e. try to keep functions that call eachother together for inlining).
            //
            // In the --release profile, codegen-units is set to 16, which attempts
            // to strike a balance between The Best Binaries and Ever Finishing Compiles.
            // In principle, tuning this down to 1 could be profitable, but LTO
            // (see the next TODO) does most of that work for us. As such we can probably
            // leave this alone to keep compile times reasonable.

            // TODO: set lto="thin" (or "fat")? (Probably "fat"!)
            //
            // LTO, Link Time Optimization, is basically hijacking the step where we
            // would link together everything and going back to the compiler (LLVM) to
            // do global optimizations across codegen-units (see the previous TODO).
            // Better Binaries, Slower Build Times.
            //
            // LTO can be "fat" (or "full") or "thin".
            //
            // Fat LTO is the "obvious" implementation: once you're done individually
            // optimizing the LLVM bitcode (IR) for each compilation unit, you concatenate
            // all the units and optimize it all together. Extremely serial, extremely
            // slow, but thorough as hell. For *enormous* codebases (millions of lines)
            // this can become intractably expensive and crash the compiler.
            //
            // Thin LTO is newer and more complicated: instead of unconditionally putting
            // everything together, we want to optimize each unit with other "useful" units
            // pulled in for inlining and other analysis. This grouping is done with
            // similar heuristics that rustc uses to break crates into codegen-units.
            // This is much faster than Fat LTO and can scale to arbitrarily big
            // codebases, but does produce slightly worse results.
            //
            // Release builds currently default to lto=false, which, despite the name,
            // actually still does LTO (lto="off" *really* turns it off)! Specifically it
            // does Thin LTO but *only* between the codegen units for a single crate.
            // This theoretically negates the disadvantages of codegen-units=16 while
            // still getting most of the advantages! Neat!
            //
            // Since most users will have codebases significantly smaller than An Entire
            // Browser, we can probably go all the way to default lto="fat", and they
            // can tune that down if it's problematic. If a user has "nightly" and "stable"
            // builds, it might be the case that they want lto="thin" for the nightlies
            // to keep them timely.
            //
            // > Aside: you may be wondering "why still have codegen units at all if using
            // > Fat LTO" and the best answer I can give you is "doing things in parallel
            // > at first lets you throw out a lot of junk and trim down the input before
            // > starting the really expensive super-linear global analysis, without losing
            // > too much of the important information". The less charitable answer is that
            // > compiler infra is built around codegen units and so this is a pragmatic hack.
            // >
            // > Thin LTO of course *really* benefits from still having codegen units.

            // TODO: set panic="abort"?
            //
            // PROBABLY NOT, but here's the discussion anyway!
            //
            // The default is panic="unwind", and things can be relying on unwinding
            // for correctness. Unwinding support bloats up the binary and can make
            // code run slower (because each place that *can* unwind is essentially
            // an early-return the compiler needs to be cautious of).
            //
            // panic="abort" immediately crashes the program if you panic,
            // but does still run the panic handler, so you *can* get things like
            // backtraces/crashreports out at that point.
            //
            // See RUSTFLAGS="-Cforce-unwind-tables" for the semi-orthogonal flag
            // that adjusts whether unwinding tables are emitted at all.
            //
            // Major C++ applications like Firefox already build with this flag,
            // the Rust ecosystem largely works fine with either.

            new_profile
                .decor_mut()
                .set_prefix("\n# generated by 'cargo dist init'\n")
        }
        dist_profile.or_insert(new_profile);
    }
    // Setup [workspace.metadata.dist] or [package.metadata.dist]
    {
        let metadata_pre_key = if workspace.root_package.is_some() {
            "package"
        } else {
            "workspace"
        };
        let workspace = workspace_toml[metadata_pre_key].or_insert(toml_edit::table());
        if let Some(t) = workspace.as_table_mut() {
            t.set_implicit(true)
        }
        let metadata = workspace["metadata"].or_insert(toml_edit::table());
        if let Some(t) = metadata.as_table_mut() {
            t.set_implicit(true)
        }
        let dist_metadata = &mut metadata[METADATA_DIST];
        if !dist_metadata.is_none() {
            return Err(miette!(
                "already init! (based on [workspace.metadata.dist] existing in your Cargo.toml)"
            ));
        }
        let mut new_metadata = toml_edit::table();
        {
            let new_metadata = new_metadata.as_table_mut().unwrap();
            new_metadata.insert(
                "os",
                toml_edit::Item::Value([OS_WINDOWS, OS_MACOS, OS_LINUX].into_iter().collect()),
            );
            new_metadata.insert(
                "cpu",
                toml_edit::Item::Value([CPU_X64, CPU_ARM64].into_iter().collect()),
            );
            new_metadata.decor_mut().set_prefix(
                "\n# These keys are generated by 'cargo dist init' and are fake placeholders\n",
            );
        }

        dist_metadata.or_insert(new_metadata);
    }
    {
        use std::io::Write;
        let mut workspace_toml_file = File::options()
            .write(true)
            .open(&workspace.manifest_path)
            .into_diagnostic()
            .wrap_err("couldn't load root workspace Cargo.toml")?;
        writeln!(&mut workspace_toml_file, "{workspace_toml}")
            .into_diagnostic()
            .wrap_err("failed to write to Cargo.toml")?;
    }
    if !args.ci_styles.is_empty() {
        let ci_args = GenerateCiArgs {
            ci_styles: args.ci_styles.clone(),
        };
        do_generate_ci(cfg, &ci_args)?;
    }
    Ok(())
}

#[derive(Debug)]
pub struct GenerateCiArgs {
    pub ci_styles: Vec<CiStyle>,
}

pub fn do_generate_ci(cfg: &Config, args: &GenerateCiArgs) -> Result<()> {
    let graph = gather_work(cfg)?;
    for style in &args.ci_styles {
        match style {
            CiStyle::Github => {
                ci::generate_github_ci(&graph.workspace_dir, &cfg.targets, &cfg.installers)?
            }
        }
    }
    Ok(())
}

/// Build a cargo target
fn generate_installer(
    _dist_graph: &DistGraph,
    target: &ArtifactTarget,
    style: &InstallerImpl,
) -> Result<()> {
    match style {
        InstallerImpl::GithubShell(info) => {
            installer::generate_github_install_sh_script(target, info)
        }
        InstallerImpl::GithubPowershell(info) => {
            installer::generate_github_install_ps_script(target, info)
        }
    }
}
