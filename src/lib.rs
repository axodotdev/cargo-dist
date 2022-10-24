#![allow(clippy::single_match)]
#![allow(unused_variables)]
#![allow(dead_code)]

use std::{collections::HashMap, fs::File, io::BufReader, process::Command};

use cargo_metadata::{camino::Utf8PathBuf, semver::Version};
use errors::*;
use flate2::{write::ZlibEncoder, Compression, GzBuilder};
use serde::{Deserialize, Serialize};
use tracing::info;
use xz2::write::XzEncoder;
use zip::ZipWriter;

pub mod errors;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {}

/// Key in workspace.metadata or package.metadata for our config
const METADATA_DIST: &str = "dist";
/// Dir in target/ for us to build our package
const TARGET_DIST: &str = "dist";
/// Some files we'll try to grab.
//TODO: LICENSE-* files, somehow!
const BUILTIN_FILES: &[&str] = &["README.md", "CHANGELOG.md", "RELEASES.md"];

/// Contents of METADATA_DIST in Cargo.toml files
#[derive(Deserialize)]
pub struct DistMetadata {}

enum BuildTarget {
    Cargo(CargoBuildTarget), // Other build systems..?
}

struct BuiltTarget {
    /// bin_name => path
    bins: HashMap<String, Utf8PathBuf>,
}

struct CargoBuildTarget {
    target_triple: String,
    features: CargoTargetFeatures,
    version: Version,
    profile: String,
    bin_names: Vec<String>,
}

type BuildTargetId = usize;

struct PackageTarget {
    bundle: BundleStyle,
    target: BuildTargetId,
    assets: Vec<Utf8PathBuf>,
}

struct PackageDir {
    path: Utf8PathBuf,
    name: String,
}

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

enum CompressionImpl {
    /// `.gz`
    Gzip,
    /// `.xz`
    Xzip,
    /// `.zstd`
    Zstd,
}

enum CargoTargetFeatureList {
    All,
    List(Vec<String>),
}

struct CargoTargetFeatures {
    no_default_features: bool,
    features: CargoTargetFeatureList,
}

pub fn build() -> Result<Report> {
    let dist_graph = gather_work()?;
    // TODO: parallelize
    let built_targets = dist_graph
        .targets
        .iter()
        .map(|target| build_target(&dist_graph, target))
        .collect::<Result<Vec<_>>>()?;

    for package in &dist_graph.packages {
        let package_dir = build_package_dir(&dist_graph, &built_targets, package)?;
        let bundle_path = bundle_package(&dist_graph, package, &package_dir)?;
        info!("bundled {}", bundle_path);
    }

    let report = Report {};

    Ok(report)
}

struct DistGraph {
    cargo: String,
    target_dir: Utf8PathBuf,
    workspace_dir: Utf8PathBuf,
    dist_dir: Utf8PathBuf,
    targets: Vec<BuildTarget>,
    packages: Vec<PackageTarget>,
}

fn gather_work() -> Result<DistGraph> {
    let cargo = std::env::var("CARGO").expect("cargo didn't pass itself!?");
    let cmd = cargo_metadata::MetadataCommand::new();

    // TODO: add a bunch of CLI flags for this. Ideally we'd use clap_cargo
    // but it wants us to use `flatten` and then we wouldn't be able to mark
    // the flags as global for all subcommands :(
    let metadata = cmd.exec().unwrap();
    let workspace_config = metadata
        .workspace_metadata
        .get(METADATA_DIST)
        .map(DistMetadata::deserialize)
        .transpose()?;
    let local_config = metadata
        .root_package()
        .and_then(|p| p.metadata.get(METADATA_DIST))
        .map(DistMetadata::deserialize)
        .transpose()?;
    let target_dir = metadata.target_directory.clone();
    let workspace_dir = metadata.workspace_root.clone();

    // Create a target/dist dir:
    let dist_dir = target_dir.join(TARGET_DIST);
    if !dist_dir.exists() {
        std::fs::create_dir_all(&dist_dir)?;
    }

    let host_target = get_host_target(&cargo)?;
    let target_is_windows = host_target.contains("windows");

    // Currently just assume we're not in a workspace, one bin!
    let root_package = metadata.root_package().unwrap();
    let targets = vec![BuildTarget::Cargo(CargoBuildTarget {
        bin_names: vec![root_package.name.clone()],
        // Just use the host target for now
        target_triple: host_target,
        version: root_package.version.clone(),
        // Just use the default build for now
        features: CargoTargetFeatures {
            no_default_features: false,
            features: CargoTargetFeatureList::List(vec![]),
        },
        // Release is the GOAT profile, *obviously*
        profile: String::from("release"),
    })];

    // TODO: make this configurable
    let bundle = if target_is_windows {
        // Windows loves them zips
        BundleStyle::Zip
    } else {
        // tar.xz is well-supported everywhere and much better than tar.gz
        BundleStyle::Tar(CompressionImpl::Xzip)
    };

    // TODO: make this configurable
    let assets = BUILTIN_FILES
        .iter()
        .filter_map(|f| {
            let file = workspace_dir.join(f);
            file.exists().then_some(file)
        })
        .collect();

    // Just one package for now
    let packages = vec![PackageTarget {
        bundle,
        // Only one target for now!
        target: 0,
        assets,
    }];

    Ok(DistGraph {
        cargo,
        target_dir,
        workspace_dir,
        dist_dir,
        targets,
        packages,
    })
}

fn get_host_target(cargo: &str) -> Result<String> {
    let mut command = Command::new(cargo);
    command.arg("-vV");
    info!("exec: {:?}", command);
    let output = command.output()?;
    let output = String::from_utf8(output.stdout).expect("argh cargo use utf8!!");
    for line in output.lines() {
        if let Some(target) = line.strip_prefix("host: ") {
            info!("host target is {target}");
            return Ok(target.to_owned());
        }
    }
    panic!("cargo failed to report its host target!?");
}

fn build_target(dist_graph: &DistGraph, target: &BuildTarget) -> Result<BuiltTarget> {
    match target {
        BuildTarget::Cargo(target) => build_cargo_target(dist_graph, target),
    }
}

fn build_cargo_target(dist_graph: &DistGraph, target: &CargoBuildTarget) -> Result<BuiltTarget> {
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
    info!("exec: {:?}", command);
    let mut task = command.spawn()?;

    // Create entries for all the binaries we expect to find with empty paths
    // we'll fail if any are still empty at the end!
    let mut bins = target
        .bin_names
        .iter()
        .map(|n| (n.clone(), Utf8PathBuf::new()))
        .collect::<HashMap<_, _>>();

    // Collect up the compiler messages to find out where binaries ended up
    let reader = std::io::BufReader::new(task.stdout.take().unwrap());
    for message in cargo_metadata::Message::parse_stream(reader) {
        match message? {
            cargo_metadata::Message::CompilerArtifact(artifact) => {
                if let Some(new_exe) = artifact.executable {
                    info!("got a new exe: {}", new_exe);
                    let bin_name = new_exe.file_stem().unwrap();
                    if bins.contains_key(bin_name) {
                        bins.insert(bin_name.to_owned(), new_exe);
                    }
                }
            }
            _ => {
                // Nothing else interesting?
            }
        }
    }

    for (bin_name, bin_path) in &bins {
        if bin_path.as_str().is_empty() {
            panic!("failed to find bin {}", bin_name);
        }
    }

    Ok(BuiltTarget { bins })
}

fn build_package_dir(
    dist: &DistGraph,
    built_targets: &[BuiltTarget],
    package: &PackageTarget,
) -> Result<PackageDir> {
    let target = &dist.targets[package.target];
    let built_target = &built_targets[package.target];

    // For now assume the first binary is the app name!
    let app_name = built_target.bins.keys().next().unwrap();
    let (version, target_triple) = match target {
        BuildTarget::Cargo(target) => (&target.version, &target.target_triple),
    };

    // Re(create) this specific package dir
    let package_dir_name = format!("{app_name}-v{version}-{target_triple}");
    let package_dir = dist.dist_dir.join(&package_dir_name);
    info!("recreating package dir: {}", package_dir);
    if package_dir.exists() {
        std::fs::remove_dir_all(&package_dir)?;
    }
    std::fs::create_dir(&package_dir)?;

    // Copy built artifacts
    for bin in built_target.bins.values() {
        let bin_file_name = bin.file_name().unwrap();
        let packaged_bin = package_dir.join(bin_file_name);
        info!("  adding {packaged_bin}");
        std::fs::copy(&bin, &packaged_bin)?;
    }

    // Copy assets
    for asset in &package.assets {
        let asset_file_name = asset.file_name().unwrap();
        let packaged_asset = package_dir.join(asset_file_name);
        info!("  adding {packaged_asset}");
        std::fs::copy(asset, packaged_asset)?;
    }

    Ok(PackageDir {
        path: package_dir,
        name: package_dir_name,
    })
}

fn bundle_package(
    dist_graph: &DistGraph,
    package: &PackageTarget,
    package_dir: &PackageDir,
) -> Result<Utf8PathBuf> {
    match &package.bundle {
        BundleStyle::Zip => zip_package(dist_graph, package, package_dir),
        BundleStyle::Tar(compression) => tar_package(dist_graph, package, package_dir, compression),
    }
}

fn tar_package(
    dist_graph: &DistGraph,
    _package: &PackageTarget,
    package_dir: &PackageDir,
    compression: &CompressionImpl,
) -> Result<Utf8PathBuf> {
    // Set up the archive/compression
    // The contents of the zip (e.g. a tar)
    let package_dir_name = &package_dir.name;
    let zip_contents_ext = "tar";
    let zip_contents_name = format!("{package_dir_name}.{zip_contents_ext}");

    // The full zip (e.g. tar.gz)
    let final_zip_ext = match compression {
        CompressionImpl::Gzip => "tar.gz",
        CompressionImpl::Xzip => "tar.xz",
        CompressionImpl::Zstd => "tar.zstd",
    };
    let final_zip_name = format!("{package_dir_name}.{final_zip_ext}");
    let final_zip_path = dist_graph.dist_dir.join(final_zip_name);
    let final_zip_file = File::create(&final_zip_path)?;

    match compression {
        CompressionImpl::Gzip => {
            // Wrap our file in compression
            let zip_output = GzBuilder::new()
                .filename(zip_contents_name)
                .write(final_zip_file, Compression::default());

            // Write the tar to the compression stream
            let mut tar = tar::Builder::new(zip_output);

            // Add the whole dir to the tar
            tar.append_dir_all(package_dir_name, &package_dir.path)?;
            // Finish up the tarring
            let zip_output = tar.into_inner()?;
            // Finish up the compression
            let _zip_file = zip_output.finish()?;
            // Drop the file to close it
        }
        CompressionImpl::Xzip => {
            let zip_output = XzEncoder::new(final_zip_file, 9);
            // Write the tar to the compression stream
            let mut tar = tar::Builder::new(zip_output);

            // Add the whole dir to the tar
            tar.append_dir_all(package_dir_name, &package_dir.path)?;
            // Finish up the tarring
            let zip_output = tar.into_inner()?;
            // Finish up the compression
            let _zip_file = zip_output.finish()?;
            // Drop the file to close it
        }
        CompressionImpl::Zstd => {
            // Wrap our file in compression
            let zip_output = ZlibEncoder::new(final_zip_file, Compression::default());

            // Write the tar to the compression stream
            let mut tar = tar::Builder::new(zip_output);

            // Add the whole dir to the tar
            tar.append_dir_all(package_dir_name, &package_dir.path)?;
            // Finish up the tarring
            let zip_output = tar.into_inner()?;
            // Finish up the compression
            let _zip_file = zip_output.finish()?;
            // Drop the file to close it
        }
    }
    info!("package created at: {}", final_zip_path);

    Ok(final_zip_path)
}

fn zip_package(
    dist_graph: &DistGraph,
    _package: &PackageTarget,
    package_dir: &PackageDir,
) -> Result<Utf8PathBuf> {
    // Set up the archive/compression
    // The contents of the zip (e.g. a tar)
    let package_dir_name = &package_dir.name;
    let final_zip_name = format!("{package_dir_name}.zip");
    let final_zip_path = dist_graph.dist_dir.join(final_zip_name);
    let final_zip_file = File::create(&final_zip_path)?;

    // Wrap our file in compression
    let mut zip = ZipWriter::new(final_zip_file);

    for entry in std::fs::read_dir(&package_dir.path)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            let options = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            let file = File::open(entry.path())?;
            let mut buf = BufReader::new(file);
            zip.start_file(entry.file_name().to_string_lossy(), options)?;
            std::io::copy(&mut buf, &mut zip)?;
        } else {
            panic!("TODO: implement zip subdirs! (or was this a symlink?)");
        }
    }

    // Finish up the compression
    let _zip_file = zip.finish()?;
    // Drop the file to close it

    Ok(final_zip_path)
}
