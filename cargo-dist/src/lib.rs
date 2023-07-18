#![deny(missing_docs)]
#![allow(clippy::single_match)]

//! # cargo-dist
//!
//! This is the library at the core of the 'cargo dist' CLI. It currently mostly exists
//! for the sake of internal documentation/testing, and isn't intended to be used by anyone else.
//! That said, if you have a reason to use it, let us know!
//!
//! It's currently not terribly well-suited to being used as a pure library because it happily
//! writes to stderr/stdout whenever it pleases. Suboptimal for a library.

use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::BufReader,
    process::Command,
};

use camino::{Utf8Path, Utf8PathBuf};
use cargo_dist_schema::{Asset, AssetKind, DistManifest, ExecutableAsset};
use flate2::{write::ZlibEncoder, Compression, GzBuilder};
use semver::Version;
use tracing::{info, warn};
use xz2::write::XzEncoder;
use zip::ZipWriter;

use errors::*;
pub use init::{do_init, InitArgs};
use miette::{miette, Context, IntoDiagnostic};
pub use tasks::*;

pub mod ci;
pub mod errors;
mod init;
pub mod installer;
pub mod tasks;
#[cfg(test)]
mod tests;

/// Top level command of cargo_dist -- do everything!
pub fn do_dist(cfg: &Config) -> Result<DistManifest> {
    let dist = tasks::gather_work(cfg)?;
    if !dist.is_init {
        return Err(miette!(
            "please run 'cargo dist init' before running any other commands!"
        ));
    }

    // FIXME: parallelize this by working this like a dependency graph, so we can start
    // bundling up an executable the moment it's built! Note however that you shouldn't
    // parallelize Cargo invocations because it has global state that can get clobbered.
    // Most problematically if you do two builds with different feature flags the final
    // binaries will get copied to the same location and clobber each other :(

    // First set up our target dirs so things don't have to race to do it later
    if !dist.dist_dir.exists() {
        std::fs::create_dir_all(&dist.dist_dir)
            .into_diagnostic()
            .wrap_err_with(|| format!("couldn't create dist target dir at {}", dist.dist_dir))?;
    }

    for artifact in &dist.artifacts {
        eprintln!("bundling {}", artifact.id);
        init_artifact_dir(&dist, artifact)?;
    }

    // Run all the build steps
    for step in &dist.build_steps {
        run_build_step(&dist, step)?;
    }

    for artifact in &dist.artifacts {
        eprintln!("bundled: {}", artifact.file_path);
    }

    Ok(build_manifest(cfg, &dist))
}

/// Just generate the manifest produced by `cargo dist build` without building
pub fn do_manifest(cfg: &Config) -> Result<DistManifest> {
    let dist = gather_work(cfg)?;
    if !dist.is_init {
        return Err(miette!(
            "please run 'cargo dist init' before running any other commands!"
        ));
    }

    Ok(build_manifest(cfg, &dist))
}

fn build_manifest(cfg: &Config, dist: &DistGraph) -> DistManifest {
    // Report the releases
    let mut releases = vec![];
    let mut all_artifacts = BTreeMap::<String, cargo_dist_schema::Artifact>::new();
    for release in &dist.releases {
        // Gather up all the local and global artifacts
        let mut artifacts = vec![];
        for &artifact_idx in &release.global_artifacts {
            let id = &dist.artifact(artifact_idx).id;
            all_artifacts.insert(id.clone(), manifest_artifact(cfg, dist, artifact_idx));
            artifacts.push(id.clone());
        }
        for &variant_idx in &release.variants {
            let variant = dist.variant(variant_idx);
            for &artifact_idx in &variant.local_artifacts {
                let id = &dist.artifact(artifact_idx).id;
                all_artifacts.insert(id.clone(), manifest_artifact(cfg, dist, artifact_idx));
                artifacts.push(id.clone());
            }
        }

        // And report the release
        releases.push(cargo_dist_schema::Release {
            app_name: release.app_name.clone(),
            app_version: release.version.to_string(),
            artifacts,
        })
    }

    let mut manifest = DistManifest::new(releases, all_artifacts);
    manifest.dist_version = Some(env!("CARGO_PKG_VERSION").to_owned());
    manifest.announcement_tag = dist.announcement_tag.clone();
    manifest.announcement_is_prerelease = dist.announcement_is_prerelease;
    manifest.announcement_title = dist.announcement_title.clone();
    manifest.announcement_changelog = dist.announcement_changelog.clone();
    manifest.announcement_github_body = dist.announcement_github_body.clone();
    manifest.system_info = Some(cargo_dist_schema::SystemInfo {
        cargo_version_line: dist.tools.cargo.version_line.clone(),
    });
    manifest
}

fn manifest_artifact(
    cfg: &Config,
    dist: &DistGraph,
    artifact_idx: ArtifactIdx,
) -> cargo_dist_schema::Artifact {
    let artifact = dist.artifact(artifact_idx);
    let mut assets = vec![];

    let built_assets = artifact
        .required_binaries
        .iter()
        .map(|(&binary_idx, exe_path)| {
            let binary = &dist.binary(binary_idx);
            let symbols_artifact = binary.symbols_artifact.map(|a| dist.artifact(a).id.clone());
            Asset {
                name: Some(binary.name.clone()),
                // Always copied to the root... for now
                path: Some(exe_path.file_name().unwrap().to_owned()),
                kind: AssetKind::Executable(ExecutableAsset { symbols_artifact }),
            }
        });

    let mut static_assets = artifact
        .archive
        .as_ref()
        .map(|archive| {
            archive
                .static_assets
                .iter()
                .map(|(kind, asset)| {
                    let kind = match kind {
                        StaticAssetKind::Changelog => AssetKind::Changelog,
                        StaticAssetKind::License => AssetKind::License,
                        StaticAssetKind::Readme => AssetKind::Readme,
                        StaticAssetKind::Other => AssetKind::Unknown,
                    };
                    Asset {
                        name: Some(asset.file_name().unwrap().to_owned()),
                        path: Some(asset.file_name().unwrap().to_owned()),
                        kind,
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Record the files that we always add to an npm package
    //
    // These can't be pre-included in the normal static assets list above because
    // they're generated from templates, and not copied from the user's project.
    if let ArtifactKind::Installer(InstallerImpl::Npm(..)) = &artifact.kind {
        for &asset in installer::NPM_PACKAGE_CONTENTS {
            static_assets.push(Asset {
                name: Some(asset.to_owned()),
                path: Some(asset.to_owned()),
                kind: AssetKind::Unknown,
            });
        }
    }

    assets.extend(built_assets);
    assets.extend(static_assets);
    // Sort the assets by name to make things extra stable
    assets.sort_by(|k1, k2| k1.name.cmp(&k2.name));

    let install_hint;
    let description;
    let kind;

    match &artifact.kind {
        ArtifactKind::ExecutableZip(_) => {
            install_hint = None;
            description = None;
            kind = cargo_dist_schema::ArtifactKind::ExecutableZip;
        }
        ArtifactKind::Symbols(_) => {
            install_hint = None;
            description = None;
            kind = cargo_dist_schema::ArtifactKind::Symbols;
        }
        ArtifactKind::Installer(
            InstallerImpl::Powershell(info)
            | InstallerImpl::Shell(info)
            | InstallerImpl::Npm(NpmInstallerInfo { inner: info, .. }),
        ) => {
            install_hint = Some(info.hint.clone());
            description = Some(info.desc.clone());
            kind = cargo_dist_schema::ArtifactKind::Installer;
        }
        ArtifactKind::Checksum(_) => {
            install_hint = None;
            description = None;
            kind = cargo_dist_schema::ArtifactKind::Checksum;
        }
    };

    let checksum = artifact.checksum.map(|idx| dist.artifact(idx).id.clone());

    cargo_dist_schema::Artifact {
        name: Some(artifact.id.clone()),
        path: if cfg.no_local_paths {
            None
        } else {
            Some(artifact.file_path.to_string())
        },
        target_triples: artifact.target_triples.clone(),
        install_hint,
        description,
        assets,
        kind,
        checksum,
    }
}

/// Run some build step
fn run_build_step(dist_graph: &DistGraph, target: &BuildStep) -> Result<()> {
    match target {
        BuildStep::Cargo(target) => build_cargo_target(dist_graph, target),
        BuildStep::Rustup(cmd) => rustup_toolchain(dist_graph, cmd),
        BuildStep::CopyFile(CopyFileStep {
            src_path,
            dest_path,
        }) => copy_file(src_path, dest_path),
        BuildStep::CopyDir(CopyDirStep {
            src_path,
            dest_path,
        }) => copy_dir(src_path, dest_path),
        BuildStep::Zip(ZipDirStep {
            src_path,
            dest_path,
            zip_style,
            dir_name,
        }) => zip_dir(src_path, dest_path, zip_style, dir_name),
        BuildStep::GenerateInstaller(installer) => generate_installer(dist_graph, installer),
        BuildStep::Checksum(ChecksumImpl {
            checksum,
            src_path,
            dest_path,
        }) => Ok(generate_checksum(checksum, src_path, dest_path)?),
    }
}

/// Generate a checksum for the src_path to dest_path
fn generate_checksum(
    checksum: &ChecksumStyle,
    src_path: &Utf8Path,
    dest_path: &Utf8Path,
) -> DistResult<()> {
    use sha2::Digest;
    use std::fmt::Write;

    let file_bytes = axoasset::LocalAsset::load_bytes(src_path.as_str())?;

    let hash = match checksum {
        ChecksumStyle::Sha256 => {
            let mut hasher = sha2::Sha256::new();
            hasher.update(&file_bytes);
            hasher.finalize().as_slice().to_owned()
        }
        ChecksumStyle::Sha512 => {
            let mut hasher = sha2::Sha512::new();
            hasher.update(&file_bytes);
            hasher.finalize().as_slice().to_owned()
        }
        ChecksumStyle::False => {
            unreachable!()
        }
    };
    let mut output = String::new();
    for byte in hash {
        write!(&mut output, "{:02x}", byte).unwrap();
    }
    axoasset::LocalAsset::write_new(
        &output,
        dest_path.file_name().unwrap(),
        dest_path.parent().unwrap().as_str(),
    )?;
    Ok(())
}

/// Build a cargo target
fn build_cargo_target(dist_graph: &DistGraph, target: &CargoBuildStep) -> Result<()> {
    eprint!(
        "building cargo target ({}/{}",
        target.target_triple, target.profile
    );

    let mut command = Command::new(&dist_graph.tools.cargo.cmd);
    command
        .arg("build")
        .arg("--profile")
        .arg(&target.profile)
        .arg("--message-format=json")
        .arg("--target")
        .arg(&target.target_triple)
        .env("RUSTFLAGS", &target.rustflags)
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
            eprintln!(" --workspace)");
        }
        CargoTargetPackages::Package(package) => {
            command.arg("--package").arg(package);
            eprintln!(" --package={})", package);
        }
    }
    info!("exec: {:?}", command);
    let mut task = command
        .spawn()
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to exec cargo build: {command:?}"))?;

    // Create entries for all the binaries we expect to find with empty paths
    // we'll fail if any are still empty at the end!
    let mut expected_exes = HashMap::<String, HashMap<String, (Utf8PathBuf, Utf8PathBuf)>>::new();
    let mut expected_symbols =
        HashMap::<String, HashMap<String, (Utf8PathBuf, Utf8PathBuf)>>::new();
    for &binary_idx in &target.expected_binaries {
        let binary = &dist_graph.binary(binary_idx);
        let package_id = binary.pkg_id.to_string();
        let exe_name = binary.name.clone();
        for exe_dest in &binary.copy_exe_to {
            expected_exes
                .entry(package_id.clone())
                .or_default()
                .insert(exe_name.clone(), (Utf8PathBuf::new(), exe_dest.clone()));
        }
        for sym_dest in &binary.copy_symbols_to {
            expected_symbols
                .entry(package_id.clone())
                .or_default()
                .insert(exe_name.clone(), (Utf8PathBuf::new(), sym_dest.clone()));
        }
    }

    // Collect up the compiler messages to find out where binaries ended up
    let reader = std::io::BufReader::new(task.stdout.take().unwrap());
    for message in cargo_metadata::Message::parse_stream(reader) {
        let Ok(message) = message
            .into_diagnostic()
            .wrap_err("failed to parse cargo json message")
            .map_err(|e| warn!("{:?}", e))
        else {
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
                    if let Some((src_sym_path, _)) = expected_sym {
                        for path in artifact.filenames {
                            // FIXME: unhardcode this when we add support for other symbol kinds!
                            let is_symbols = path.extension().map(|e| e == "pdb").unwrap_or(false);
                            if is_symbols {
                                // These are symbols we expected! Save the path.
                                *src_sym_path = path;
                            }
                        }
                    }

                    // Get the exe path
                    let expected_exe = expected_exes
                        .get_mut(&package_id)
                        .and_then(|m| m.get_mut(exe_name));
                    if let Some(expected) = expected_exe {
                        // This is an exe we expected! Save the path.
                        expected.0 = new_exe;
                    }
                }
            }
            _ => {
                // Nothing else interesting?
            }
        }
    }

    // Check that we got everything we expected, and normalize to ArtifactIdx => Artifact Path
    for (package_id, exes) in expected_exes {
        for (exe_name, (src_path, dest_path)) in &exes {
            if src_path.as_str().is_empty() {
                return Err(miette!("failed to find bin {} ({})", exe_name, package_id));
            }
            copy_file(src_path, dest_path)?;
        }
    }
    for (package_id, symbols) in expected_symbols {
        for (exe, (src_path, dest_path)) in &symbols {
            if src_path.as_str().is_empty() {
                return Err(miette!(
                    "failed to find symbols for bin {} ({})",
                    exe,
                    package_id
                ));
            }
            copy_file(src_path, dest_path)?;
        }
    }

    Ok(())
}

/// Build a cargo target
fn rustup_toolchain(_dist_graph: &DistGraph, cmd: &RustupStep) -> Result<()> {
    eprintln!("running rustup to ensure you have {} installed", cmd.target);
    let status = Command::new(&cmd.rustup.cmd)
        .arg("target")
        .arg("add")
        .arg(&cmd.target)
        .status()
        .into_diagnostic()
        .wrap_err("Failed to install rustup toolchain")?;

    if !status.success() {
        return Err(miette!("Failed to install rustup toolchain"));
    }
    Ok(())
}

/// Initialize the dir for an artifact (and delete the old artifact file).
fn init_artifact_dir(_dist: &DistGraph, artifact: &Artifact) -> Result<()> {
    // Delete any existing bundle
    if artifact.file_path.exists() {
        std::fs::remove_file(&artifact.file_path)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to delete old artifact {}", artifact.file_path))?;
    }

    let Some(archive) = &artifact.archive else {
        // If there's no dir than we're done
        return Ok(());
    };
    info!("recreating artifact dir: {}", archive.dir_path);

    // Clear out the dir we'll build the bundle up in
    if archive.dir_path.exists() {
        std::fs::remove_dir_all(&archive.dir_path)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to delete old artifact dir {}", archive.dir_path))?;
    }
    std::fs::create_dir(&archive.dir_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to create artifact dir {}", archive.dir_path))?;

    Ok(())
}

pub(crate) fn copy_file(src_path: &Utf8Path, dest_path: &Utf8Path) -> Result<()> {
    let _bytes_written = std::fs::copy(src_path, dest_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to copy file {src_path} => {dest_path}"))?;
    Ok(())
}

fn copy_dir(_src_path: &Utf8Path, _dest_path: &Utf8Path) -> Result<()> {
    todo!("copy_dir isn't implemented yet")
}

fn zip_dir(
    src_path: &Utf8Path,
    dest_path: &Utf8Path,
    zip_style: &ZipStyle,
    dir_name: &str,
) -> Result<()> {
    match zip_style {
        ZipStyle::Zip => really_zip_dir(src_path, dest_path, dir_name),
        ZipStyle::Tar(compression) => tar_dir(src_path, dest_path, compression, dir_name),
    }
}

fn tar_dir(
    src_path: &Utf8Path,
    dest_path: &Utf8Path,
    compression: &CompressionImpl,
    dir_name: &str,
) -> Result<()> {
    // Set up the archive/compression
    // The contents of the zip (e.g. a tar)
    let zip_contents_name = format!("{dir_name}.tar");
    let final_zip_file = File::create(dest_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to create file for artifact: {dest_path}"))?;

    match compression {
        CompressionImpl::Gzip => {
            // Wrap our file in compression
            let zip_output = GzBuilder::new()
                .filename(zip_contents_name)
                .write(final_zip_file, Compression::default());

            // Write the tar to the compression stream
            let mut tar = tar::Builder::new(zip_output);

            // Add the whole dir to the tar
            tar.append_dir_all(dir_name, src_path)
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!("failed to copy directory into tar: {src_path} => {dir_name}",)
                })?;
            // Finish up the tarring
            let zip_output = tar
                .into_inner()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write tar: {dest_path}"))?;
            // Finish up the compression
            let _zip_file = zip_output
                .finish()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write archive: {dest_path}"))?;
            // Drop the file to close it
        }
        CompressionImpl::Xzip => {
            let zip_output = XzEncoder::new(final_zip_file, 9);
            // Write the tar to the compression stream
            let mut tar = tar::Builder::new(zip_output);

            // Add the whole dir to the tar
            tar.append_dir_all(dir_name, src_path)
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!("failed to copy directory into tar: {src_path} => {dir_name}",)
                })?;
            // Finish up the tarring
            let zip_output = tar
                .into_inner()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write tar: {dest_path}"))?;
            // Finish up the compression
            let _zip_file = zip_output
                .finish()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write archive: {dest_path}"))?;
            // Drop the file to close it
        }
        CompressionImpl::Zstd => {
            // Wrap our file in compression
            let zip_output = ZlibEncoder::new(final_zip_file, Compression::default());

            // Write the tar to the compression stream
            let mut tar = tar::Builder::new(zip_output);

            // Add the whole dir to the tar
            tar.append_dir_all(dir_name, src_path)
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!("failed to copy directory into tar: {src_path} => {dir_name}",)
                })?;
            // Finish up the tarring
            let zip_output = tar
                .into_inner()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write tar: {dest_path}"))?;
            // Finish up the compression
            let _zip_file = zip_output
                .finish()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write archive: {dest_path}"))?;
            // Drop the file to close it
        }
    }

    info!("artifact created at: {}", dest_path);
    Ok(())
}

fn really_zip_dir(src_path: &Utf8Path, dest_path: &Utf8Path, _dir_name: &str) -> Result<()> {
    // FIXME: we don't respect dir_name in that we don't add an extra wrapper dir
    // ...but now things expect that and honestly that's more convenient for how
    // we use these (installers --strip-prefix=1 our tarballs but there's no such option for unzip).
    // Set up the archive/compression
    let final_zip_file = File::create(dest_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to create file for artifact: {dest_path}"))?;

    // Wrap our file in compression
    let mut zip = ZipWriter::new(final_zip_file);

    let dir = std::fs::read_dir(src_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to read artifact dir: {src_path}"))?;
    for entry in dir {
        let entry = entry.into_diagnostic()?;
        if entry.file_type().into_diagnostic()?.is_file() {
            let options = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            let file = File::open(entry.path()).into_diagnostic()?;
            let mut buf = BufReader::new(file);
            let file_name = entry.file_name();
            // FIXME: ...don't do this lossy conversion?
            let utf8_file_name = file_name.to_string_lossy();
            zip.start_file(utf8_file_name.clone(), options)
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!("failed to create file {utf8_file_name} in zip: {dest_path}")
                })?;
            std::io::copy(&mut buf, &mut zip).into_diagnostic()?;
        } else {
            todo!("implement zip subdirs! (or was this a symlink?)");
        }
    }

    // Finish up the compression
    let _zip_file = zip
        .finish()
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to write archive: {dest_path}"))?;
    // Drop the file to close it
    info!("artifact created at: {}", dest_path);
    Ok(())
}

/// Arguments for `cargo dist generate-ci` ([`do_generate_ci][])
#[derive(Debug)]
pub struct GenerateCiArgs {}

/// Generate CI scripts (impl of `cargo dist generate-ci`)
pub fn do_generate_ci(cfg: &Config, _args: &GenerateCiArgs) -> Result<()> {
    let dist = gather_work(cfg)?;
    // Enforce cargo-dist-version, unless it's a magic vX.Y.Z-github-BRANCHNAME version,
    // which we use for testing against a PR branch. In that case the current_version
    // should be irrelevant (so sayeth the person who made and uses this feature).
    if let Some(desired_version) = &dist.desired_cargo_dist_version {
        let current_version: Version = std::env!("CARGO_PKG_VERSION").parse().unwrap();
        if desired_version != &current_version && !desired_version.pre.starts_with("github-") {
            return Err(miette!("you're running cargo-dist {}, but 'cargo-dist-version = {}' is set in your Cargo.toml\n\nYou should update cargo-dist-version if you want to update to this version", current_version, desired_version));
        }
    }
    if !dist.is_init {
        return Err(miette!(
            "please run 'cargo dist init' before running any other commands!"
        ));
    }

    for style in &dist.ci_style {
        match style {
            CiStyle::Github => ci::generate_github_ci(&dist)?,
        }
    }
    Ok(())
}

/// Build a cargo target
fn generate_installer(dist: &DistGraph, style: &InstallerImpl) -> Result<()> {
    match style {
        InstallerImpl::Shell(info) => installer::generate_install_sh_script(dist, info),
        InstallerImpl::Powershell(info) => installer::generate_install_ps_script(dist, info),
        InstallerImpl::Npm(info) => installer::generate_install_npm_project(dist, info),
    }
}
