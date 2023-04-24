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
    ops::Not,
    process::Command,
};

use axoproject::{errors::AxoprojectError, WorkspaceInfo};
use camino::{Utf8Path, Utf8PathBuf};
use cargo_dist_schema::{Asset, AssetKind, DistManifest, ExecutableAsset, Release};
use flate2::{write::ZlibEncoder, Compression, GzBuilder};
use semver::Version;
use tracing::{info, warn};
use xz2::write::XzEncoder;
use zip::ZipWriter;

use errors::*;
use miette::{miette, Context, IntoDiagnostic};
pub use tasks::*;

pub mod ci;
pub mod errors;
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
    // binaries will get copied to the same location and clobber eachother :(

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
        releases.push(Release {
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
    };

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
        }) => zip_dir(src_path, dest_path, zip_style),
        BuildStep::GenerateInstaller(installer) => generate_installer(dist_graph, installer),
    }
}

/// Build a cargo target
fn build_cargo_target(dist_graph: &DistGraph, target: &CargoBuildStep) -> Result<()> {
    eprintln!(
        "building cargo target ({}/{})",
        target.target_triple, target.profile
    );

    let mut command = Command::new(&dist_graph.cargo);
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

fn zip_dir(src_path: &Utf8Path, dest_path: &Utf8Path, zip_style: &ZipStyle) -> Result<()> {
    match zip_style {
        ZipStyle::Zip => really_zip_dir(src_path, dest_path),
        ZipStyle::Tar(compression) => tar_dir(src_path, dest_path, compression),
    }
}

fn tar_dir(src_path: &Utf8Path, dest_path: &Utf8Path, compression: &CompressionImpl) -> Result<()> {
    // Set up the archive/compression
    // The contents of the zip (e.g. a tar)
    let dir_name = src_path.file_name().unwrap();
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

fn really_zip_dir(src_path: &Utf8Path, dest_path: &Utf8Path) -> Result<()> {
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

/// Arguments for `cargo dist init` ([`do_init`][])
#[derive(Debug)]
pub struct InitArgs {}

/// Run 'cargo dist init'
pub fn do_init(cfg: &Config, _args: &InitArgs) -> Result<()> {
    let workspace = tasks::get_project()?;

    // Load in the workspace toml to edit and write back
    let mut workspace_toml = tasks::load_root_cargo_toml(&workspace.manifest_path)?;

    // Init things
    let mut did_anything = false;
    if init_dist_profile(cfg, &mut workspace_toml)? {
        eprintln!("added [profile.dist] to your root Cargo.toml");
        did_anything = true;
    } else {
        eprintln!("[profile.dist] already exists, nothing to do");
    }

    let (worked, gen_ci) = init_dist_metadata(cfg, &workspace, &mut workspace_toml)?;
    if worked {
        eprintln!("added [workspace.metadata.dist] to your root Cargo.toml");
        did_anything = true;
    } else {
        eprintln!("[workspace.metadata.dist] already exists, nothing to do");
    }

    if did_anything {
        use std::io::Write;
        let mut workspace_toml_file = File::options()
            .write(true)
            .open(&workspace.manifest_path)
            .into_diagnostic()
            .wrap_err("couldn't load root workspace Cargo.toml")?;
        write!(&mut workspace_toml_file, "{workspace_toml}")
            .into_diagnostic()
            .wrap_err("failed to write to Cargo.toml")?;
    }
    if gen_ci {
        let ci_args = GenerateCiArgs {};
        do_generate_ci(cfg, &ci_args)?;
    }
    Ok(())
}

fn init_dist_profile(_cfg: &Config, workspace_toml: &mut toml_edit::Document) -> Result<bool> {
    let profiles = workspace_toml["profile"].or_insert(toml_edit::table());
    if let Some(t) = profiles.as_table_mut() {
        t.set_implicit(true)
    }
    let dist_profile = &mut profiles[PROFILE_DIST];
    if !dist_profile.is_none() {
        return Ok(false);
    }
    let mut new_profile = toml_edit::table();
    {
        // For some detailed discussion, see: https://github.com/axodotdev/cargo-dist/issues/118
        let new_profile = new_profile.as_table_mut().unwrap();
        // We're building for release, so this is a good base!
        new_profile.insert("inherits", toml_edit::value("release"));
        // We're building for SUPER DUPER release, so lto is a good idea to enable!
        //
        // There's a decent argument for lto=true (aka "fat") here but the cost-benefit
        // is a bit complex. Fat LTO can be way more expensive to compute (to the extent
        // that enormous applications like chromium can become unbuildable), but definitely
        // eeks out a bit more from your binaries.
        //
        // In principle cargo-dist is targetting True Shippable Binaries and so it's
        // worth it to go nuts getting every last drop out of your binaries... but a lot
        // of people are going to build binaries that might never even be used, so really
        // we're just burning a bunch of CI time for nothing.
        //
        // The user has the freedom to crank this up higher (and/or set codegen-units=1)
        // if they think it's worth it, but we otherwise probably shouldn't set the planet
        // on fire just because Number Theoretically Go Up.
        new_profile.insert("lto", toml_edit::value("thin"));
        new_profile
            .decor_mut()
            .set_prefix("\n# The profile that 'cargo dist' will build with\n")
    }
    dist_profile.or_insert(new_profile);

    Ok(true)
}

/// Initialize [workspace.metadata.dist] with default values based on what was passed on the CLI
///
/// Returns whether the initialization was actually done
/// and whether ci was set
fn init_dist_metadata(
    cfg: &Config,
    workspace_info: &WorkspaceInfo,
    workspace_toml: &mut toml_edit::Document,
) -> DistResult<(bool, bool)> {
    use dialoguer::{theme::SimpleTheme, Confirm, Input, MultiSelect};
    use toml_edit::{value, Item};
    // Setup [workspace.metadata.dist]
    let workspace = workspace_toml["workspace"].or_insert(toml_edit::table());
    if let Some(t) = workspace.as_table_mut() {
        t.set_implicit(true)
    }
    let metadata = workspace["metadata"].or_insert(toml_edit::table());
    if let Some(t) = metadata.as_table_mut() {
        t.set_implicit(true)
    }
    let dist_metadata = &mut metadata[METADATA_DIST];
    let mut meta = if dist_metadata.is_none() {
        DistMetadata {
            // If they init with this version we're gonna try to stick to it!
            cargo_dist_version: Some(std::env!("CARGO_PKG_VERSION").parse().unwrap()),
            // latest stable release at this precise moment
            // maybe there's something more clever we can do here, but, *shrug*
            rust_toolchain_version: Some("1.67.1".to_owned()),
            ci: vec![],
            installers: None,
            targets: cfg.targets.is_empty().not().then(|| cfg.targets.clone()),
            dist: None,
            include: vec![],
            auto_includes: None,
            windows_archive: None,
            unix_archive: None,
            npm_scope: None,
        }
    } else {
        tasks::parse_metadata_table(workspace_info.cargo_metadata_table.as_ref())
    };

    // Clone this to simplify checking for settings changes
    let orig_meta = meta.clone();

    // Keys/descriptions
    const KEY_RUST_VERSION: &str = "rust-toolchain-version";
    const DESC_RUST_VERSION: &str =
        "# The preferred Rust toolchain to use in CI (rustup toolchain syntax)\n";

    const KEY_DIST_VERSION: &str = "cargo-dist-version";
    const DESC_DIST_VERSION: &str =
        "# The preferred cargo-dist version to use in CI (Cargo.toml SemVer syntax)\n";

    const KEY_CI: &str = "ci";
    const DESC_CI: &str = "# CI backends to support (see 'cargo dist generate-ci')\n";

    const KEY_INSTALLERS: &str = "installers";
    const DESC_INSTALLERS: &str = "# The installers to generate for each app\n";

    const KEY_TARGETS: &str = "targets";
    const DESC_TARGETS: &str = "# Target platforms to build apps for (Rust target-triple syntax)\n";

    const KEY_DIST: &str = "dist";
    const DESC_DIST: &str =
        "# Whether to consider the binaries in a package for distribution (defaults true)\n";

    const KEY_INCLUDE: &str = "include";
    const DESC_INCLUDE: &str =
        "# Extra static files to include in each App (path relative to this Cargo.toml's dir)\n";

    const KEY_AUTO_INCLUDE: &str = "auto-includes";
    const DESC_AUTO_INCLUDE: &str =
        "# Whether to auto-include files like READMEs, LICENSEs, and CHANGELOGs (default true)\n";

    const KEY_WIN_ARCHIVE: &str = "windows-archive";
    const DESC_WIN_ARCHIVE: &str =
        "# The archive format to use for windows builds (defaults .zip)\n";

    const KEY_UNIX_ARCHIVE: &str = "unix-archive";
    const DESC_UNIX_ARCHIVE: &str =
        "# The archive format to use for non-windows builds (defaults .tar.xz)\n";

    const KEY_NPM_SCOPE: &str = "npm-scope";
    const DESC_NPM_SCOPE: &str =
        "# A namespace to use when publishing this package to the npm registry\n";

    // Now prompt the user interactively to initialize these...

    let theme = SimpleTheme;

    // Set cargo-dist-version
    let current_version: Version = std::env!("CARGO_PKG_VERSION").parse().unwrap();
    if let Some(desired_version) = &meta.cargo_dist_version {
        if desired_version != &current_version && !desired_version.pre.starts_with("github-") {
            let prompt = format!(
                "update your project to this version of cargo-dist? ({} => {})",
                desired_version, current_version
            );
            if Confirm::with_theme(&theme)
                .with_prompt(prompt)
                .default(true)
                .interact()?
            {
                meta.cargo_dist_version = Some(current_version);
            } else {
                return Err(DistError::NoUpdateVersion {
                    project_version: desired_version.clone(),
                    running_version: current_version,
                })?;
            }
        }
    } else {
        let prompt = format!(
            "looks like you deleted the cargo-dist-version key, add it back? ({})",
            current_version
        );
        if Confirm::with_theme(&theme)
            .with_prompt(prompt)
            .default(true)
            .interact()?
        {
            meta.cargo_dist_version = Some(current_version);
        } else {
            // Not recommended but technically ok...
        }
    }

    // Enable CI backends
    {
        let known = &[CiStyle::Github];
        let mut defaults = vec![];
        let mut keys = vec![];
        for item in known {
            // If this CI style is in their config, keep it
            // If they passed it on the CLI, flip it on
            let mut default = meta.ci.contains(item) || cfg.ci.contains(item);

            // If they have a well-defined repo url and it's github, default enable it
            #[allow(irrefutable_let_patterns)]
            if let CiStyle::Github = item {
                if let Some(repo_url) = &workspace_info.repository_url {
                    if repo_url.contains("github.com") {
                        default = true;
                    }
                }
            }
            defaults.push(default);
            // This match is here to remind you to add new CiStyles
            // to `known` above!
            keys.push(match item {
                CiStyle::Github => "github",
            });
        }

        // Prompt the user
        let prompt = "enable ci (select with arrow keys and space, submit with enter)";
        let selected = MultiSelect::with_theme(&theme)
            .items(&keys)
            .defaults(&defaults)
            .with_prompt(prompt)
            .interact()?;

        // Apply the results
        meta.ci = selected.into_iter().map(|i| known[i]).collect();
    }

    // Enforce repository url right away
    if meta.ci.contains(&CiStyle::Github) && workspace_info.repository_url.is_none() {
        // If axoproject complained about inconsistency, forward that
        // Massively jank manual implementation of "clone" here because lots of error types
        // (like std::io::Error) don't implement Clone and so axoproject errors can't either
        let conflict = workspace_info.warnings.iter().find_map(|w| {
            if let AxoprojectError::InconsistentRepositoryKey {
                file1,
                url1,
                file2,
                url2,
            } = w
            {
                Some(AxoprojectError::InconsistentRepositoryKey {
                    file1: file1.clone(),
                    url1: url1.clone(),
                    file2: file2.clone(),
                    url2: url2.clone(),
                })
            } else {
                None
            }
        });
        if let Some(inner) = conflict {
            return Err(DistError::CantEnableGithubUrlInconsistent { inner })?;
        } else {
            // Otherwise assume no URL
            return Err(DistError::CantEnableGithubNoUrl)?;
        }
    }

    // Enable installer backends (if they have a CI backend that can provide URLs)
    // In the future, "vendored" installers like MSIs could be enabled in this situation!
    if !meta.ci.is_empty() {
        let known = &[
            InstallerStyle::Shell,
            InstallerStyle::Powershell,
            InstallerStyle::Npm,
        ];
        let mut defaults = vec![];
        let mut keys = vec![];
        for item in known {
            // If this CI style is in their config, keep it
            // If they passed it on the CLI, flip it on
            let config_had_it = meta
                .installers
                .as_deref()
                .unwrap_or_default()
                .contains(item);
            let cli_had_it = cfg.installers.contains(item);

            let default = config_had_it || cli_had_it;
            defaults.push(default);

            // This match is here to remind you to add new InstallerStyles
            // to `known` above!
            keys.push(match item {
                InstallerStyle::Shell => "shell",
                InstallerStyle::Powershell => "powershell",
                InstallerStyle::Npm => "npm",
            });
        }

        // Prompt the user
        let prompt = "enable installers (select with arrow keys and space, submit with enter)";
        let selected = MultiSelect::with_theme(&theme)
            .items(&keys)
            .defaults(&defaults)
            .with_prompt(prompt)
            .interact()?;

        // Apply the results
        meta.installers = Some(selected.into_iter().map(|i| known[i]).collect());
    } else {
        eprintln!("no CI backends enabled, skipping installers which require URLs to fetch from");
    }

    // Special handling of the npm installer
    if meta
        .installers
        .as_deref()
        .unwrap_or_default()
        .contains(&InstallerStyle::Npm)
    {
        const TAR_GZ: Option<ZipStyle> = Some(ZipStyle::Tar(CompressionImpl::Gzip));

        // If npm is being newly enabled here, prompt for a @scope
        let npm_is_new = !orig_meta
            .installers
            .as_deref()
            .unwrap_or_default()
            .contains(&InstallerStyle::Npm);
        if npm_is_new {
            let prompt = "you've enabled npm support, please enter the @scope you want to publish under (leave blank to publish globally)";
            let scope: String = Input::with_theme(&theme)
                .with_prompt(prompt)
                .allow_empty(true)
                .validate_with(|v: &String| {
                    let v = v.trim();
                    if v.is_empty() {
                        Ok(())
                    } else if let Some(v) = v.strip_prefix('@') {
                        if v.is_empty() {
                            Err("@ must be followed by something")
                        } else {
                            Ok(())
                        }
                    } else {
                        Err("npm scopes must start with @")
                    }
                })
                .interact_text()?;
            let scope = scope.trim();
            if scope.is_empty() {
                meta.npm_scope = None;
            } else {
                meta.npm_scope = Some(scope.to_owned());
            }
        }

        // FIXME (#226): If they have an npm installer, force on tar.gz compression
        let prompt = "the npm installer currently requires all artifacts be .tar.gz, is that ok?";
        let force_targz = Confirm::with_theme(&theme)
            .with_prompt(prompt)
            .default(true)
            .interact()?;
        if force_targz {
            meta.unix_archive = TAR_GZ;
            meta.windows_archive = TAR_GZ;
        }
    }

    // Ok, we're done getting values, now edit the toml!!!

    // If there's no table, make one
    if !dist_metadata.is_table() {
        *dist_metadata = toml_edit::table();
    }

    // Apply formatted/commented values
    let table = dist_metadata.as_table_mut().unwrap();
    if let Some(val) = meta.cargo_dist_version {
        table.insert(KEY_DIST_VERSION, value(val.to_string()));
        table
            .key_decor_mut(KEY_DIST_VERSION)
            .unwrap()
            .set_prefix(DESC_DIST_VERSION);
    }
    if let Some(val) = meta.rust_toolchain_version {
        table.insert(KEY_RUST_VERSION, value(val));
        table
            .key_decor_mut(KEY_RUST_VERSION)
            .unwrap()
            .set_prefix(DESC_RUST_VERSION);
    }
    if !meta.ci.is_empty() {
        table.insert(
            KEY_CI,
            Item::Value(
                meta.ci
                    .iter()
                    .map(|ci| match ci {
                        CiStyle::Github => "github",
                    })
                    .collect(),
            ),
        );
        table.key_decor_mut(KEY_CI).unwrap().set_prefix(DESC_CI);
    }
    if let Some(val) = meta.installers {
        table.insert(
            KEY_INSTALLERS,
            Item::Value(
                val.iter()
                    .map(|installer| match installer {
                        InstallerStyle::Powershell => "powershell",
                        InstallerStyle::Shell => "shell",
                        InstallerStyle::Npm => "npm",
                    })
                    .collect(),
            ),
        );
        table
            .key_decor_mut(KEY_INSTALLERS)
            .unwrap()
            .set_prefix(DESC_INSTALLERS);
    }
    if let Some(val) = meta.targets {
        table.insert(KEY_TARGETS, Item::Value(val.into_iter().collect()));
        table
            .key_decor_mut(KEY_TARGETS)
            .unwrap()
            .set_prefix(DESC_TARGETS);
    }
    if let Some(val) = meta.dist {
        table.insert(KEY_DIST, value(val));
        table.key_decor_mut(KEY_DIST).unwrap().set_prefix(DESC_DIST);
    }
    if !meta.include.is_empty() {
        table.insert(
            KEY_INCLUDE,
            Item::Value(meta.include.iter().map(ToString::to_string).collect()),
        );
        table
            .key_decor_mut(KEY_INCLUDE)
            .unwrap()
            .set_prefix(DESC_INCLUDE);
    }
    if let Some(val) = meta.auto_includes {
        table.insert(KEY_AUTO_INCLUDE, value(val));
        table
            .key_decor_mut(KEY_AUTO_INCLUDE)
            .unwrap()
            .set_prefix(DESC_AUTO_INCLUDE);
    }
    if let Some(val) = meta.windows_archive {
        table.insert(KEY_WIN_ARCHIVE, value(val.ext()));
        table
            .key_decor_mut(KEY_WIN_ARCHIVE)
            .unwrap()
            .set_prefix(DESC_WIN_ARCHIVE);
    }
    if let Some(val) = meta.unix_archive {
        table.insert(KEY_UNIX_ARCHIVE, value(val.ext()));
        table
            .key_decor_mut(KEY_UNIX_ARCHIVE)
            .unwrap()
            .set_prefix(DESC_UNIX_ARCHIVE);
    }
    if let Some(val) = meta.npm_scope {
        table.insert(KEY_NPM_SCOPE, value(val));
        table
            .key_decor_mut(KEY_NPM_SCOPE)
            .unwrap()
            .set_prefix(DESC_NPM_SCOPE);
    }
    table
        .decor_mut()
        .set_prefix("\n# Config for 'cargo dist'\n");

    Ok((true, !meta.ci.is_empty()))
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
