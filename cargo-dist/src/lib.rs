//! # cargo-dist
//!
//!

#![allow(clippy::single_match)]
#![allow(dead_code)]

use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::BufReader,
    path::PathBuf,
    process::Command,
};

use camino::{Utf8Path, Utf8PathBuf};
use cargo_dist_schema::{Artifact, DistReport, Distributable, ExecutableArtifact, Release};
use flate2::{write::ZlibEncoder, Compression, GzBuilder};
use guppy::{
    graph::{BuildTargetId, DependencyDirection, PackageMetadata},
    MetadataCommand, PackageId,
};
use semver::Version;
use serde::Deserialize;
use tracing::{info, warn};
use xz2::write::XzEncoder;
use zip::ZipWriter;

use errors::*;
use miette::{miette, Context, IntoDiagnostic};

pub mod errors;
#[cfg(test)]
mod tests;

/// Key in workspace.metadata or package.metadata for our config
const METADATA_DIST: &str = "dist";
/// Dir in target/ for us to build our packages in
/// NOTE: DO NOT GIVE THIS THE SAME NAME AS A PROFILE!
const TARGET_DIST: &str = "distrib";
/// Some files we'll try to grab.
//TODO: LICENSE-* files, somehow!
const BUILTIN_FILES: &[&str] = &["README.md", "CHANGELOG.md", "RELEASES.md"];

/// Contents of METADATA_DIST in Cargo.toml files
#[derive(Deserialize)]
pub struct DistMetadata {}

/// A unique id for a [`BuildTarget`][]
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
struct BuildTargetIdx(usize);

/// A unique id for a [`BuildArtifact`][]
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
struct BuildArtifactIdx(usize);

/// A unique id for a [`DistributableTarget`][]
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash, Debug)]
struct DistributableTargetIdx(usize);

/// The graph of all work that cargo-dist needs to do on this invocation.
///
/// All work is precomputed at the start of execution because only discovering
/// what you need to do in the middle of building/packing things is a mess.
/// It also allows us to report what *should* happen without actually doing it.
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
    /// Artifacts we want to get out of targets
    artifacts: Vec<BuildArtifact>,
    /// Distributable bundles we want to build for our artifacts
    distributables: Vec<DistributableTarget>,
    /// Logical releases that distributable bundles are grouped under
    releases: Vec<ReleaseTarget>,
}

/// A build we need to perform to get artifacts to distribute.
enum BuildTarget {
    /// A cargo build
    Cargo(CargoBuildTarget),
    // Other build systems..?
}

/// A cargo build
struct CargoBuildTarget {
    /// The --target triple to pass
    target_triple: String,
    /// The feature flags to pass
    features: CargoTargetFeatures,
    /// What package to build (or "the workspace")
    package: CargoTargetPackages,
    /// The --profile to pass
    profile: String,
    /// Artifacts we expect from this build
    expected_artifacts: Vec<BuildArtifactIdx>,
}

/// An artifact we need from our builds
enum BuildArtifact {
    /// An executable
    Executable(ExecutableBuildArtifact),
}

/// An executable we need from our builds
struct ExecutableBuildArtifact {
    /// The name of the executable (without a file extension)
    exe_name: String,
    /// The cargo package this executable is defined by
    package_id: PackageId,
    /// The [`BuildTarget`][] that should produce this.
    build_target: BuildTargetIdx,
}

/// A distributable bundle we want to build
struct DistributableTarget {
    /// The target platform
    ///
    /// i.e. `x86_64-pc-windows-msvc`
    target_triple: String,
    /// The full name of the distributable
    ///
    /// i.e. `cargo-dist-v0.1.0-x86_64-pc-windows-msvc`
    full_name: String,
    /// The path to the directory where this distributable's
    /// contents will be gathered before bundling.
    ///
    /// i.e. `/.../target/dist/cargo-dist-v0.1.0-x86_64-pc-windows-msvc/`
    dir_path: Utf8PathBuf,
    /// The file name of the distributable
    ///
    /// i.e. `cargo-dist-v0.1.0-x86_64-pc-windows-msvc.zip`
    file_name: String,
    /// The path where the final distributable will appear
    ///
    /// i.e. `/.../target/dist/cargo-dist-v0.1.0-x86_64-pc-windows-msvc.zip`
    file_path: Utf8PathBuf,
    /// The bundling method (zip, tar.gz, ...)
    bundle: BundleStyle,
    /// The build artifacts this distributable will contain
    ///
    /// i.e. `cargo-dist.exe`
    required_artifacts: HashSet<BuildArtifactIdx>,
    /// Additional static assets to add to the distributable
    ///
    /// i.e. `README.md`
    assets: Vec<Utf8PathBuf>,
}

/// A logical release of an application that distributables are grouped under
struct ReleaseTarget {
    /// The name of the app
    app_name: String,
    /// The version of the app
    version: Version,
    /// The distributables this release includes
    distributables: Vec<DistributableTargetIdx>,
}

/// The style of bundle for a [`DistributableTarget`][].
enum BundleStyle {
    /// `.zip`
    Zip,
    /// `.tar.<compression>`
    Tar(CompressionImpl),
    // TODO: Microsoft MSI installer
    // TODO: Apple .dmg "installer"
    // TODO: flatpak?
    // TODO: snap? (ostensibly "obsoleted" by flatpak)
    // TODO: various linux package manager manifests? (.deb, .rpm, ... do these make sense?)
}

/// Compression impls (used by [`BundleStyle::Tar`][])
enum CompressionImpl {
    /// `.gz`
    Gzip,
    /// `.xz`
    Xzip,
    /// `.zstd`
    Zstd,
}

/// Cargo features a [`CargoBuildTarget`][] should use.
struct CargoTargetFeatures {
    /// Whether to disable default features
    no_default_features: bool,
    /// Features to enable
    features: CargoTargetFeatureList,
}

/// A list of features to build with
enum CargoTargetFeatureList {
    /// All of them
    All,
    /// Some of them
    List(Vec<String>),
}

/// Whether to build a package or workspace
enum CargoTargetPackages {
    /// Build the workspace
    Workspace,
    /// Just build a package
    Package(PackageId),
}

/// Top level command of cargo_dist -- do everything!
pub fn do_dist() -> Result<DistReport> {
    let dist = gather_work()?;

    // TODO: parallelize this by working this like a dependency graph, so we can start
    // bundling up an executable the moment it's built!

    // First set up our target dirs so things don't have to race to do it later
    if !dist.dist_dir.exists() {
        std::fs::create_dir_all(&dist.dist_dir)
            .into_diagnostic()
            .wrap_err_with(|| format!("couldn't create dist target dir at {}", dist.dist_dir))?;
    }

    for distrib in &dist.distributables {
        eprintln!("bundling {}", distrib.file_name);
        init_distributable_dir(&dist, distrib)?;
    }

    let mut built_artifacts = HashMap::new();
    // Run all the builds
    for target in &dist.targets {
        let new_built_artifacts = build_target(&dist, target)?;
        // Copy the artifacts as soon as possible, future builds may clobber them!
        for (&artifact_idx, built_artifact) in &new_built_artifacts {
            populate_distributable_dirs_with_built_artifact(&dist, artifact_idx, built_artifact)?;
        }
        built_artifacts.extend(new_built_artifacts);
    }

    // Build all the bundles
    for distrib in &dist.distributables {
        populate_distributable_dir_with_assets(&dist, distrib)?;
        bundle_distributable(&dist, distrib)?;
        eprintln!("bundled {}", distrib.file_path);
    }

    // Report the releases
    let mut releases = vec![];
    for release in &dist.releases {
        releases.push(Release {
            app_name: release.app_name.clone(),
            app_version: release.version.to_string(),
            distributables: release
                .distributables
                .iter()
                .map(|distrib_idx| {
                    let distrib = &dist.distributables[distrib_idx.0];
                    Distributable {
                        path: distrib.file_path.clone().into_std_path_buf(),
                        target_triple: distrib.target_triple.clone(),
                        artifacts: distrib
                            .required_artifacts
                            .iter()
                            .map(|artifact_idx| {
                                let artifact = &dist.artifacts[artifact_idx.0];
                                let artifact_path = &built_artifacts[artifact_idx];
                                match artifact {
                                    BuildArtifact::Executable(exe) => {
                                        Artifact::Executable(ExecutableArtifact {
                                            name: exe.exe_name.clone(),
                                            path: PathBuf::from(artifact_path.file_name().unwrap()),
                                        })
                                    }
                                }
                            })
                            .collect(),
                        kind: cargo_dist_schema::DistributableKind::Zip,
                    }
                })
                .collect(),
        })
    }
    Ok(DistReport::new(releases))
}

/// Precompute all the work this invocation will need to do
fn gather_work() -> Result<DistGraph> {
    let cargo = std::env::var("CARGO").expect("cargo didn't pass itself!?");
    let mut metadata_cmd = MetadataCommand::new();
    // guppy will source from the same place as us, but let's be paranoid and make sure
    // EVERYTHING is DEFINITELY ALWAYS using the same Cargo!
    metadata_cmd.cargo_path(&cargo);

    // TODO: add a bunch of CLI flags for this. Ideally we'd use clap_cargo
    // but it wants us to use `flatten` and then we wouldn't be able to mark
    // the flags as global for all subcommands :(
    let pkg_graph = metadata_cmd
        .build_graph()
        .into_diagnostic()
        .wrap_err("failed to read 'cargo metadata'")?;
    let workspace = pkg_graph.workspace();
    let workspace_members = pkg_graph.resolve_workspace();

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

    let target_dir = workspace.target_directory().to_owned();
    let workspace_dir = workspace.root().to_owned();
    let dist_dir = target_dir.join(TARGET_DIST);

    // Currently just build the host target
    let host_target_triple = get_host_target(&cargo)?;
    let mut targets = vec![BuildTarget::Cargo(CargoBuildTarget {
        // Just use the host target for now
        target_triple: host_target_triple,
        // Just build the whole workspace for now
        package: CargoTargetPackages::Workspace,
        // Just use the default build for now
        features: CargoTargetFeatures {
            no_default_features: false,
            features: CargoTargetFeatureList::List(vec![]),
        },
        // Release is the GOAT profile, *obviously*
        profile: String::from("release"),
        // Populated later
        expected_artifacts: vec![],
    })];

    // Find all the binaries that each target will build
    let mut artifacts = vec![];
    for (idx, target) in targets.iter_mut().enumerate() {
        let target_idx = BuildTargetIdx(idx);
        match target {
            BuildTarget::Cargo(target) => {
                let new_artifacts = match &target.package {
                    CargoTargetPackages::Workspace => artifacts_for_cargo_packages(
                        target_idx,
                        workspace_members.packages(DependencyDirection::Forward),
                    ),
                    CargoTargetPackages::Package(package_id) => {
                        artifacts_for_cargo_packages(target_idx, pkg_graph.metadata(package_id))
                    }
                };
                let new_artifact_idxs = artifacts.len()..artifacts.len() + new_artifacts.len();
                artifacts.extend(new_artifacts);
                target
                    .expected_artifacts
                    .extend(new_artifact_idxs.map(BuildArtifactIdx));
            }
        }
    }

    // Give each artifact its own distributable (for now)
    let mut distributables = vec![];
    let mut releases = HashMap::<(String, Version), ReleaseTarget>::new();
    for (idx, artifact) in artifacts.iter().enumerate() {
        let artifact_idx = BuildArtifactIdx(idx);
        match artifact {
            BuildArtifact::Executable(exe) => {
                let build_target = &targets[exe.build_target.0];
                let target_triple = match build_target {
                    BuildTarget::Cargo(target) => target.target_triple.clone(),
                };

                // TODO: make bundle style configurable
                let target_is_windows = target_triple.contains("windows");
                let bundle = if target_is_windows {
                    // Windows loves them zips
                    BundleStyle::Zip
                } else {
                    // tar.xz is well-supported everywhere and much better than tar.gz
                    BundleStyle::Tar(CompressionImpl::Xzip)
                };

                // TODO: make bundled assets configurable
                // TODO: narrow this scope to the package of the binary..?
                let assets = BUILTIN_FILES
                    .iter()
                    .filter_map(|f| {
                        let file = workspace_dir.join(f);
                        file.exists().then_some(file)
                    })
                    .collect();

                // TODO: make app name configurable? Use some other fields in the PackageMetadata?
                let app_name = exe.exe_name.clone();
                // TODO: allow apps to be versioned separately from packages?
                let version = pkg_graph
                    .metadata(&exe.package_id)
                    .unwrap()
                    .version()
                    .clone();
                // TODO: make the bundle name configurable?
                let full_name = format!("{app_name}-v{version}-{target_triple}");
                let dir_path = dist_dir.join(&full_name);
                let file_ext = match bundle {
                    BundleStyle::Zip => "zip",
                    BundleStyle::Tar(CompressionImpl::Gzip) => "tar.gz",
                    BundleStyle::Tar(CompressionImpl::Zstd) => "tar.zstd",
                    BundleStyle::Tar(CompressionImpl::Xzip) => "tar.xz",
                };
                let file_name = format!("{full_name}.{file_ext}");
                let file_path = dist_dir.join(&file_name);

                let distributable_idx = DistributableTargetIdx(distributables.len());
                distributables.push(DistributableTarget {
                    target_triple,
                    full_name,
                    file_path,
                    file_name,
                    dir_path,
                    bundle,
                    required_artifacts: Some(artifact_idx).into_iter().collect(),
                    assets,
                });
                let release = releases
                    .entry((app_name.clone(), version.clone()))
                    .or_insert_with(|| ReleaseTarget {
                        app_name,
                        version,
                        distributables: vec![],
                    });
                release.distributables.push(distributable_idx);
            }
        }
    }

    let releases = releases.into_iter().map(|e| e.1).collect();
    Ok(DistGraph {
        cargo,
        target_dir,
        workspace_dir,
        dist_dir,
        targets,
        artifacts,
        distributables,
        releases,
    })
}

/// Get all the artifacts built by this list of cargo packages
fn artifacts_for_cargo_packages<'a>(
    target_idx: BuildTargetIdx,
    packages: impl IntoIterator<Item = PackageMetadata<'a>>,
) -> Vec<BuildArtifact> {
    packages
        .into_iter()
        .flat_map(|package| {
            package.build_targets().filter_map(move |target| {
                let build_id = target.id();
                if let BuildTargetId::Binary(name) = build_id {
                    Some(BuildArtifact::Executable(ExecutableBuildArtifact {
                        exe_name: name.to_owned(),
                        package_id: package.id().clone(),
                        build_target: target_idx,
                    }))
                } else {
                    None
                }
            })
        })
        .collect::<Vec<_>>()
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
) -> Result<HashMap<BuildArtifactIdx, Utf8PathBuf>> {
    match target {
        BuildTarget::Cargo(target) => build_cargo_target(dist_graph, target),
    }
}

/// Build a cargo target
fn build_cargo_target(
    dist_graph: &DistGraph,
    target: &CargoBuildTarget,
) -> Result<HashMap<BuildArtifactIdx, Utf8PathBuf>> {
    eprintln!(
        "building cargo target ({}/{})",
        target.target_triple, target.profile
    );
    // Run the build
    // TODO: add flags for things like split-debuginfo (annoyingly platform-specific mess)
    // TODO: add flags for opt-level=2 (strip after)
    // TODO: should we create a profile..?
    let mut command = Command::new(&dist_graph.cargo);
    command
        .arg("build")
        .arg("--profile")
        .arg(&target.profile)
        .arg("--message-format=json")
        .stdout(std::process::Stdio::piped());
    if target.features.no_default_features {
        command.arg("--no-defauly-features");
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
        .wrap_err_with(|| format!("failed to exec cargo build: {:?}", command))?;

    // Create entries for all the binaries we expect to find with empty paths
    // we'll fail if any are still empty at the end!
    let mut expected_exes =
        HashMap::<String, HashMap<String, (BuildArtifactIdx, Utf8PathBuf)>>::new();
    for artifact_idx in &target.expected_artifacts {
        let artifact = &dist_graph.artifacts[artifact_idx.0];
        let BuildArtifact::Executable(exe) = artifact;
        {
            let package_id = exe.package_id.to_string();
            let exe_name = exe.exe_name.clone();
            expected_exes
                .entry(package_id)
                .or_default()
                .insert(exe_name, (*artifact_idx, Utf8PathBuf::new()));
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
                    let expected_exe = expected_exes
                        .get_mut(&package_id)
                        .and_then(|m| m.get_mut(exe_name));
                    if let Some(expected) = expected_exe {
                        // It is! Save the path.
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
    let mut built_exes = HashMap::new();
    for (package_id, exes) in expected_exes {
        for (exe, (artifact_idx, exe_path)) in exes {
            if exe_path.as_str().is_empty() {
                return Err(miette!("failed to find bin {} for {}", exe, package_id));
            }
            built_exes.insert(artifact_idx, exe_path);
        }
    }

    Ok(built_exes)
}

/// Initialize the dir for a distributable (and delete the old distributable file).
fn init_distributable_dir(_dist: &DistGraph, distrib: &DistributableTarget) -> Result<()> {
    info!("recreating distributable dir: {}", distrib.dir_path);

    // Clear out the dir we'll build the bundle up in
    if distrib.dir_path.exists() {
        std::fs::remove_dir_all(&distrib.dir_path)
            .into_diagnostic()
            .wrap_err_with(|| {
                format!(
                    "failed to delete old distributable dir {}",
                    distrib.dir_path
                )
            })?;
    }
    std::fs::create_dir(&distrib.dir_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to create distributable dir {}", distrib.dir_path))?;

    // Delete any existing bundle
    if distrib.file_path.exists() {
        std::fs::remove_file(&distrib.file_path)
            .into_diagnostic()
            .wrap_err_with(|| {
                format!("failed to delete old distributable {}", distrib.file_path)
            })?;
    }

    Ok(())
}

fn populate_distributable_dirs_with_built_artifact(
    dist: &DistGraph,
    artifact_idx: BuildArtifactIdx,
    built_artifact: &Utf8Path,
) -> Result<()> {
    for distrib in &dist.distributables {
        if distrib.required_artifacts.contains(&artifact_idx) {
            let artifact_file_name = built_artifact.file_name().unwrap();
            let bundled_artifact = distrib.dir_path.join(artifact_file_name);
            info!("  adding {built_artifact} to {}", distrib.dir_path);
            std::fs::copy(built_artifact, &bundled_artifact)
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!(
                        "failed to copy bundled artifact to distributable: {} => {}",
                        built_artifact, bundled_artifact
                    )
                })?;
        }
    }
    Ok(())
}

fn populate_distributable_dir_with_assets(
    _dist: &DistGraph,
    distrib: &DistributableTarget,
) -> Result<()> {
    info!("populating distributable dir: {}", distrib.dir_path);
    // Copy assets
    for asset in &distrib.assets {
        let asset_file_name = asset.file_name().unwrap();
        let bundled_asset = distrib.dir_path.join(asset_file_name);
        info!("  adding {bundled_asset}");
        std::fs::copy(asset, &bundled_asset)
            .into_diagnostic()
            .wrap_err_with(|| {
                format!(
                    "failed to copy bundled asset to distributable: {} => {}",
                    asset, bundled_asset
                )
            })?;
    }

    Ok(())
}

fn bundle_distributable(dist_graph: &DistGraph, distrib: &DistributableTarget) -> Result<()> {
    info!("bundling distributable: {}", distrib.file_path);
    match &distrib.bundle {
        BundleStyle::Zip => zip_distributable(dist_graph, distrib),
        BundleStyle::Tar(compression) => tar_distributable(dist_graph, distrib, compression),
    }
}

fn tar_distributable(
    _dist_graph: &DistGraph,
    distrib: &DistributableTarget,
    compression: &CompressionImpl,
) -> Result<()> {
    // Set up the archive/compression
    // The contents of the zip (e.g. a tar)
    let distrib_dir_name = &distrib.full_name;
    let zip_contents_name = format!("{distrib_dir_name}.tar");
    let final_zip_path = &distrib.file_path;
    let final_zip_file = File::create(final_zip_path)
        .into_diagnostic()
        .wrap_err_with(|| {
            format!(
                "failed to create file for distributable: {}",
                final_zip_path
            )
        })?;

    match compression {
        CompressionImpl::Gzip => {
            // Wrap our file in compression
            let zip_output = GzBuilder::new()
                .filename(zip_contents_name)
                .write(final_zip_file, Compression::default());

            // Write the tar to the compression stream
            let mut tar = tar::Builder::new(zip_output);

            // Add the whole dir to the tar
            tar.append_dir_all(distrib_dir_name, &distrib.dir_path)
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!(
                        "failed to copy directory into tar: {} => {}",
                        distrib.dir_path, distrib_dir_name
                    )
                })?;
            // Finish up the tarring
            let zip_output = tar
                .into_inner()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write tar: {}", final_zip_path))?;
            // Finish up the compression
            let _zip_file = zip_output
                .finish()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write archive: {}", final_zip_path))?;
            // Drop the file to close it
        }
        CompressionImpl::Xzip => {
            let zip_output = XzEncoder::new(final_zip_file, 9);
            // Write the tar to the compression stream
            let mut tar = tar::Builder::new(zip_output);

            // Add the whole dir to the tar
            tar.append_dir_all(distrib_dir_name, &distrib.dir_path)
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!(
                        "failed to copy directory into tar: {} => {}",
                        distrib.dir_path, distrib_dir_name
                    )
                })?;
            // Finish up the tarring
            let zip_output = tar
                .into_inner()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write tar: {}", final_zip_path))?;
            // Finish up the compression
            let _zip_file = zip_output
                .finish()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write archive: {}", final_zip_path))?;
            // Drop the file to close it
        }
        CompressionImpl::Zstd => {
            // Wrap our file in compression
            let zip_output = ZlibEncoder::new(final_zip_file, Compression::default());

            // Write the tar to the compression stream
            let mut tar = tar::Builder::new(zip_output);

            // Add the whole dir to the tar
            tar.append_dir_all(distrib_dir_name, &distrib.dir_path)
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!(
                        "failed to copy directory into tar: {} => {}",
                        distrib.dir_path, distrib_dir_name
                    )
                })?;
            // Finish up the tarring
            let zip_output = tar
                .into_inner()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write tar: {}", final_zip_path))?;
            // Finish up the compression
            let _zip_file = zip_output
                .finish()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write archive: {}", final_zip_path))?;
            // Drop the file to close it
        }
    }

    info!("distributable created at: {}", final_zip_path);
    Ok(())
}

fn zip_distributable(_dist_graph: &DistGraph, distrib: &DistributableTarget) -> Result<()> {
    // Set up the archive/compression
    let final_zip_path = &distrib.file_path;
    let final_zip_file = File::create(final_zip_path)
        .into_diagnostic()
        .wrap_err_with(|| {
            format!(
                "failed to create file for distributable: {}",
                final_zip_path
            )
        })?;

    // Wrap our file in compression
    let mut zip = ZipWriter::new(final_zip_file);

    let dir = std::fs::read_dir(&distrib.dir_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to read distributable dir: {}", distrib.dir_path))?;
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
                    format!(
                        "failed to create file {} in zip: {}",
                        utf8_file_name, final_zip_path
                    )
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
        .wrap_err_with(|| format!("failed to write archive: {}", final_zip_path))?;
    // Drop the file to close it
    info!("distributable created at: {}", final_zip_path);
    Ok(())
}
