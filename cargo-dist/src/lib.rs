#![deny(missing_docs)]
#![allow(clippy::single_match, clippy::result_large_err)]

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
    io::{Cursor, Read},
    process::Command,
};

use axoasset::{LocalAsset, SourceFile};
use backend::{
    ci::CiInfo,
    installer::{self, homebrew::HomebrewInstallerInfo, npm::NpmInstallerInfo, InstallerImpl},
    templates::{TemplateEntry, TEMPLATE_INSTALLER_NPM},
};
use camino::{Utf8Path, Utf8PathBuf};
use cargo_dist_schema::{Asset, AssetKind, DistManifest, ExecutableAsset};
use comfy_table::{presets::UTF8_FULL, Table};
use config::{
    ArtifactMode, ChecksumStyle, CompressionImpl, Config, DirtyMode, GenerateMode, ZipStyle,
};
use goblin::Object;
use mach_object::{LoadCommand, OFile};
use semver::Version;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use errors::*;
pub use init::{do_init, InitArgs};
use miette::{miette, Context, IntoDiagnostic};
pub use tasks::*;

pub mod backend;
pub mod config;
pub mod errors;
mod init;
pub mod tasks;
#[cfg(test)]
mod tests;

/// cargo dist build -- actually build binaries and installers!
pub fn do_build(cfg: &Config) -> Result<DistManifest> {
    check_integrity(cfg)?;

    let dist = tasks::gather_work(cfg)?;

    // FIXME: parallelize this by working this like a dependency graph, so we can start
    // bundling up an executable the moment it's built! Note however that you shouldn't
    // parallelize Cargo invocations because it has global state that can get clobbered.
    // Most problematically if you do two builds with different feature flags the final
    // binaries will get copied to the same location and clobber each other :(

    // First set up our target dirs so things don't have to race to do it later
    if !dist.dist_dir.exists() {
        LocalAsset::create_dir_all(&dist.dist_dir)?;
    }

    eprintln!("building artifacts:");
    for artifact in &dist.artifacts {
        eprintln!("  {}", artifact.id);
        init_artifact_dir(&dist, artifact)?;
    }
    eprintln!();

    // Run all the local build steps first
    for step in &dist.local_build_steps {
        run_build_step(&dist, step, &[])?;
    }

    // Calculate a temporary build manifest from the output of the local builds
    // (includes their linkage data)
    let manifests = vec![build_manifest(cfg, &dist)?];

    // Next the global steps
    for step in &dist.global_build_steps {
        run_build_step(&dist, step, &manifests)?;
    }

    Ok(build_manifest(cfg, &dist)?)
}

/// Just generate the manifest produced by `cargo dist build` without building
pub fn do_manifest(cfg: &Config) -> Result<DistManifest> {
    check_integrity(cfg)?;
    let dist = gather_work(cfg)?;

    Ok(build_manifest(cfg, &dist)?)
}

fn build_manifest(cfg: &Config, dist: &DistGraph) -> DistResult<DistManifest> {
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

    let linkage = fetch_linkage(
        cfg.targets.clone(),
        dist.artifacts.clone(),
        dist.dist_dir.clone(),
    )?;
    let linkage = linkage.iter().map(|l| l.to_schema()).collect();

    let mut manifest = DistManifest::new(releases, all_artifacts);

    // build metadata
    manifest.dist_version = Some(env!("CARGO_PKG_VERSION").to_owned());
    manifest.system_info = Some(cargo_dist_schema::SystemInfo {
        cargo_version_line: dist.tools.cargo.version_line.clone(),
    });

    // announcement metadata
    manifest.announcement_tag = dist.announcement_tag.clone();
    manifest.announcement_is_prerelease = dist.announcement_is_prerelease;
    manifest.announcement_title = dist.announcement_title.clone();
    manifest.announcement_changelog = dist.announcement_changelog.clone();
    manifest.announcement_github_body = dist.announcement_github_body.clone();
    manifest.system_info = Some(cargo_dist_schema::SystemInfo {
        cargo_version_line: dist.tools.cargo.version_line.clone(),
    });

    // ci metadata
    if !dist.ci_style.is_empty() {
        let CiInfo { github } = &dist.ci;
        let github = github.as_ref().map(|info| cargo_dist_schema::GithubCiInfo {
            artifacts_matrix: Some(info.artifacts_matrix.clone()),
            pr_run_mode: Some(info.pr_run_mode),
        });

        manifest.ci = Some(cargo_dist_schema::CiInfo { github });
    }

    manifest.publish_prereleases = dist.publish_prereleases;

    manifest.linkage = linkage;

    Ok(manifest)
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
        let root_dir = dist
            .templates
            .get_template_dir(TEMPLATE_INSTALLER_NPM)
            .expect("npm template missing!?");
        let mut queue = vec![root_dir];
        while let Some(dir) = queue.pop() {
            for entry in dir.entries.values() {
                match entry {
                    TemplateEntry::Dir(dir) => {
                        queue.push(dir);
                    }
                    TemplateEntry::File(file) => {
                        static_assets.push(Asset {
                            name: Some(file.name.clone()),
                            path: Some(file.path_from_ancestor(root_dir).to_string()),
                            kind: AssetKind::Unknown,
                        });
                    }
                }
            }
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
            | InstallerImpl::Homebrew(HomebrewInstallerInfo { inner: info, .. })
            | InstallerImpl::Npm(NpmInstallerInfo { inner: info, .. }),
        ) => {
            install_hint = Some(info.hint.clone());
            description = Some(info.desc.clone());
            kind = cargo_dist_schema::ArtifactKind::Installer;
        }
        ArtifactKind::Installer(InstallerImpl::Msi(..)) => {
            install_hint = None;
            description = Some("install via msi".to_owned());
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
fn run_build_step(
    dist_graph: &DistGraph,
    target: &BuildStep,
    manifests: &[DistManifest],
) -> Result<()> {
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
            with_root,
        }) => zip_dir(src_path, dest_path, zip_style, with_root.as_deref()),
        BuildStep::GenerateInstaller(installer) => {
            generate_installer(dist_graph, installer, manifests)
        }
        BuildStep::Checksum(ChecksumImpl {
            checksum,
            src_path,
            dest_path,
        }) => Ok(generate_and_write_checksum(checksum, src_path, dest_path)?),
    }
}

/// Generate a checksum for the src_path to dest_path
fn generate_and_write_checksum(
    checksum: &ChecksumStyle,
    src_path: &Utf8Path,
    dest_path: &Utf8Path,
) -> DistResult<()> {
    let output = generate_checksum(checksum, src_path)?;
    write_checksum(&output, src_path, dest_path)
}

/// Generate a checksum for the src_path and return it as a string
fn generate_checksum(checksum: &ChecksumStyle, src_path: &Utf8Path) -> DistResult<String> {
    info!("generating {checksum:?} for {src_path}");
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
    Ok(output)
}

/// Write the checksum to dest_path
fn write_checksum(checksum: &str, src_path: &Utf8Path, dest_path: &Utf8Path) -> DistResult<()> {
    // Tools like sha256sum expect a new-line-delimited format of
    // <checksum> <mode><path>
    //
    // * checksum is the checksum in hex
    // * mode is ` ` for "text" and `*` for "binary" (we mostly have binaries)
    // * path is a relative path to the thing being checksumed (usually just a filename)
    //
    // We also make sure there's a trailing newline as is traditional.
    //
    // By following this format we support commands like `sha256sum --check my-app.tar.gz.sha256`
    let file_path = src_path.file_name().expect("hashing file with no name!?");
    let line = format!("{checksum} *{file_path}\n");
    axoasset::LocalAsset::write_new(&line, dest_path)?;
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
        .arg("--message-format=json-render-diagnostics")
        .arg("--target")
        .arg(&target.target_triple)
        .env("RUSTFLAGS", &target.rustflags)
        .stdout(std::process::Stdio::piped());
    if !target.features.default_features {
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

    // Create entries for all the binaries we expect to find, and the paths they should
    // be copied to (according to the copy_exe_to subscribers list).
    //
    // Structure is:
    //
    // package-id (key)
    //    binary-name (key)
    //       subscribers (list)
    //          src-path (initially blank, must be filled in by rustc)
    //          dest-path (where to copy the file to)
    let mut expected_exes =
        HashMap::<String, HashMap<String, Vec<(Utf8PathBuf, Utf8PathBuf)>>>::new();
    let mut expected_symbols =
        HashMap::<String, HashMap<String, Vec<(Utf8PathBuf, Utf8PathBuf)>>>::new();
    for &binary_idx in &target.expected_binaries {
        let binary = &dist_graph.binary(binary_idx);
        let package_id = binary.pkg_id.to_string();
        let exe_name = binary.name.clone();
        for exe_dest in &binary.copy_exe_to {
            expected_exes
                .entry(package_id.clone())
                .or_default()
                .entry(exe_name.clone())
                .or_default()
                .push((Utf8PathBuf::new(), exe_dest.clone()));
        }
        for sym_dest in &binary.copy_symbols_to {
            expected_symbols
                .entry(package_id.clone())
                .or_default()
                .entry(exe_name.clone())
                .or_default()
                .push((Utf8PathBuf::new(), sym_dest.clone()));
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
                    if let Some(expected) = expected_sym {
                        for (src_sym_path, _) in expected {
                            for path in &artifact.filenames {
                                // FIXME: unhardcode this when we add support for other symbol kinds!
                                let is_symbols =
                                    path.extension().map(|e| e == "pdb").unwrap_or(false);
                                if is_symbols {
                                    // These are symbols we expected! Save the path.
                                    *src_sym_path = path.to_owned();
                                }
                            }
                        }
                    }

                    // Get the exe path
                    let expected_exe = expected_exes
                        .get_mut(&package_id)
                        .and_then(|m| m.get_mut(exe_name));
                    if let Some(expected) = expected_exe {
                        for (src_bin_path, _) in expected {
                            // This is an exe we expected! Save the path.
                            *src_bin_path = new_exe.clone();
                        }
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
        for (exe_name, to_copy) in &exes {
            for (src_path, dest_path) in to_copy {
                if src_path.as_str().is_empty() {
                    return Err(miette!(
                        "failed to find bin {} ({}) -- did the cargo build above have errors?",
                        exe_name,
                        package_id
                    ));
                }
                copy_file(src_path, dest_path)?;
            }
        }
    }
    for (package_id, symbols) in expected_symbols {
        for (exe, to_copy) in &symbols {
            for (src_path, dest_path) in to_copy {
                if src_path.as_str().is_empty() {
                    return Err(miette!(
                        "failed to find symbols for bin {} ({}) -- did the cargo build above have errors?",
                        exe,
                        package_id
                    ));
                }
                copy_file(src_path, dest_path)?;
            }
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
        LocalAsset::remove_file(&artifact.file_path)?;
    }

    let Some(archive) = &artifact.archive else {
        // If there's no dir than we're done
        return Ok(());
    };
    info!("recreating artifact dir: {}", archive.dir_path);

    // Clear out the dir we'll build the bundle up in
    if archive.dir_path.exists() {
        LocalAsset::remove_dir_all(&archive.dir_path)?;
    }
    LocalAsset::create_dir(&archive.dir_path)?;

    Ok(())
}

pub(crate) fn copy_file(src_path: &Utf8Path, dest_path: &Utf8Path) -> Result<()> {
    LocalAsset::copy_named(src_path, dest_path)?;
    Ok(())
}

pub(crate) fn copy_dir(src_path: &Utf8Path, dest_path: &Utf8Path) -> Result<()> {
    LocalAsset::copy_dir_named(src_path, dest_path)?;
    Ok(())
}

fn zip_dir(
    src_path: &Utf8Path,
    dest_path: &Utf8Path,
    zip_style: &ZipStyle,
    with_root: Option<&Utf8Path>,
) -> Result<()> {
    match zip_style {
        ZipStyle::Zip => LocalAsset::zip_dir(src_path, dest_path, with_root)?,
        ZipStyle::Tar(CompressionImpl::Gzip) => {
            LocalAsset::tar_gz_dir(src_path, dest_path, with_root)?
        }
        ZipStyle::Tar(CompressionImpl::Xzip) => {
            LocalAsset::tar_xz_dir(src_path, dest_path, with_root)?
        }
        ZipStyle::Tar(CompressionImpl::Zstd) => {
            LocalAsset::tar_zstd_dir(src_path, dest_path, with_root)?
        }
        ZipStyle::TempDir => {
            // no-op
        }
    }
    Ok(())
}

/// Arguments for `cargo dist generate` ([`do_generate`][])
#[derive(Debug)]
pub struct GenerateArgs {
    /// Check whether the output differs without writing to disk
    pub check: bool,
    /// Which type(s) of config to generate
    pub modes: Vec<GenerateMode>,
}

/// Arguments for `cargo dist linkage` ([`do_linkage][])
#[derive(Debug)]
pub struct LinkageArgs {
    /// Print human-readable output
    pub print_output: bool,
    /// Print output as JSON
    pub print_json: bool,
    /// Read linkage data from JSON rather than performing a live check
    pub from_json: Option<String>,
}

fn do_generate_preflight_checks(dist: &DistGraph) -> Result<()> {
    // Enforce cargo-dist-version, unless...
    //
    // * It's a magic vX.Y.Z-github-BRANCHNAME version,
    //   which we use for testing against a PR branch. In that case the current_version
    //   should be irrelevant (so sayeth the person who made and uses this feature).
    //
    // * The user passed --allow-dirty to the CLI (probably means it's our own tests)
    if let Some(desired_version) = &dist.desired_cargo_dist_version {
        let current_version: Version = std::env!("CARGO_PKG_VERSION").parse().unwrap();
        if desired_version != &current_version
            && !desired_version.pre.starts_with("github-")
            && !matches!(dist.allow_dirty, DirtyMode::AllowAll)
        {
            return Err(miette!("you're running cargo-dist {}, but 'cargo-dist-version = {}' is set in your Cargo.toml\n\nYou should update cargo-dist-version if you want to update to this version", current_version, desired_version));
        }
    }
    if !dist.is_init {
        return Err(miette!(
            "please run 'cargo dist init' before running any other commands!"
        ));
    }

    Ok(())
}

/// Generate any scripts which are relevant (impl of `cargo dist generate`)
pub fn do_generate(cfg: &Config, args: &GenerateArgs) -> Result<()> {
    let dist = gather_work(cfg)?;

    run_generate(&dist, args)?;

    Ok(())
}

/// The inner impl of do_generate
pub fn run_generate(dist: &DistGraph, args: &GenerateArgs) -> Result<()> {
    do_generate_preflight_checks(dist)?;

    // If specific modes are specified, operate *only* on those modes
    // Otherwise, choose any modes that are appropriate
    let inferred = args.modes.is_empty();
    let modes = if inferred {
        &[GenerateMode::Ci, GenerateMode::Msi]
    } else {
        // Check that we're not being told to do a contradiction
        for &mode in &args.modes {
            if !dist.allow_dirty.should_run(mode)
                && matches!(dist.allow_dirty, DirtyMode::AllowList(..))
            {
                Err(DistError::ContradictoryGenerateModes {
                    generate_mode: mode,
                })?;
            }
        }
        &args.modes[..]
    };

    // generate everything we need to
    // HEY! if you're adding a case to this, add it to the inferred list above!
    for &mode in modes {
        if dist.allow_dirty.should_run(mode) {
            match mode {
                GenerateMode::Ci => {
                    // If you add a CI backend, call it here
                    let CiInfo { github } = &dist.ci;
                    if let Some(github) = github {
                        if args.check {
                            github.check(dist)?;
                        } else {
                            github.write_to_disk(dist)?;
                        }
                    }
                }
                GenerateMode::Msi => {
                    for artifact in &dist.artifacts {
                        if let ArtifactKind::Installer(InstallerImpl::Msi(msi)) = &artifact.kind {
                            if args.check {
                                msi.check_config()?;
                            } else {
                                msi.write_config_to_disk()?;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn fetch_linkage(
    targets: Vec<String>,
    artifacts: Vec<Artifact>,
    dist_dir: Utf8PathBuf,
) -> DistResult<Vec<Linkage>> {
    let mut reports = vec![];

    for target in targets {
        let artifacts: Vec<Artifact> = artifacts
            .clone()
            .into_iter()
            .filter(|r| r.target_triples.contains(&target))
            .collect();

        if artifacts.is_empty() {
            eprintln!("No matching artifact for target {target}");
            continue;
        }

        for artifact in artifacts {
            let path = Utf8PathBuf::from(&dist_dir).join(format!("{}-{target}", artifact.id));

            for (_, binary) in artifact.required_binaries {
                let bin_path = path.join(binary);
                if !bin_path.exists() {
                    eprintln!("Binary {bin_path} missing; skipping check");
                } else {
                    reports.push(determine_linkage(&bin_path, &target)?);
                }
            }
        }
    }

    Ok(reports)
}

/// Determinage dynamic linkage of built artifacts (impl of `cargo dist linkage`)
pub fn do_linkage(cfg: &Config, args: &LinkageArgs) -> Result<()> {
    let dist = gather_work(cfg)?;

    let reports: Vec<Linkage> = if let Some(target) = args.from_json.clone() {
        let file = SourceFile::load_local(&target)?;
        file.deserialize_json()?
    } else {
        fetch_linkage(cfg.targets.clone(), dist.artifacts, dist.dist_dir)?
    };

    if args.print_output {
        for report in &reports {
            eprintln!("{}", report.report());
        }
    }
    if args.print_json {
        let j = serde_json::to_string(&reports).unwrap();
        println!("{}", j);
    }

    Ok(())
}

/// Information about dynamic libraries used by a binary
#[derive(Debug, Deserialize, Serialize)]
pub struct Linkage {
    /// The filename of the binary
    pub binary: String,
    /// The target triple for which the binary was built
    pub target: String,
    /// Libraries included with the operating system
    pub system: Vec<Library>,
    /// Libraries provided by the Homebrew package manager
    pub homebrew: Vec<Library>,
    /// Public libraries not provided by the system and not managed by any package manager
    pub public_unmanaged: Vec<Library>,
    /// Libraries which don't fall into any other categories
    pub other: Vec<Library>,
    /// Frameworks, only used on macOS
    pub frameworks: Vec<Library>,
}

impl Linkage {
    /// Formatted human-readable output
    pub fn report(&self) -> String {
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .set_header(vec!["Category", "Libraries"])
            .add_row(vec![
                "System",
                self.system
                    .clone()
                    .into_iter()
                    .map(|l| l.to_string_pretty())
                    .collect::<Vec<String>>()
                    .join("\n")
                    .as_str(),
            ])
            .add_row(vec![
                "Homebrew",
                self.homebrew
                    .clone()
                    .into_iter()
                    .map(|l| l.to_string_pretty())
                    .collect::<Vec<String>>()
                    .join("\n")
                    .as_str(),
            ])
            .add_row(vec![
                "Public (unmanaged)",
                self.public_unmanaged
                    .clone()
                    .into_iter()
                    .map(|l| l.path)
                    .collect::<Vec<String>>()
                    .join("\n")
                    .as_str(),
            ])
            .add_row(vec![
                "Frameworks",
                self.frameworks
                    .clone()
                    .into_iter()
                    .map(|l| l.path)
                    .collect::<Vec<String>>()
                    .join("\n")
                    .as_str(),
            ])
            .add_row(vec![
                "Other",
                self.other
                    .clone()
                    .into_iter()
                    .map(|l| l.to_string_pretty())
                    .collect::<Vec<String>>()
                    .join("\n")
                    .as_str(),
            ]);

        let s = format!(
            r#"{} ({}):

{table}"#,
            self.binary, self.target,
        );

        s.to_owned()
    }

    fn to_schema(&self) -> cargo_dist_schema::Linkage {
        cargo_dist_schema::Linkage {
            binary: self.binary.clone(),
            target: self.target.clone(),
            system: self.system.iter().map(|s| s.to_schema()).collect(),
            homebrew: self.homebrew.iter().map(|s| s.to_schema()).collect(),
            public_unmanaged: self
                .public_unmanaged
                .iter()
                .map(|s| s.to_schema())
                .collect(),
            other: self.other.iter().map(|s| s.to_schema()).collect(),
            frameworks: self.frameworks.iter().map(|s| s.to_schema()).collect(),
        }
    }

    /// Constructs a Linkage from a cargo_dist_schema::Linkage
    pub fn from_schema(other: &cargo_dist_schema::Linkage) -> Self {
        Self {
            binary: other.binary.clone(),
            target: other.target.clone(),
            system: other.system.iter().map(Library::from_schema).collect(),
            homebrew: other.homebrew.iter().map(Library::from_schema).collect(),
            public_unmanaged: other
                .public_unmanaged
                .iter()
                .map(Library::from_schema)
                .collect(),
            other: other.other.iter().map(Library::from_schema).collect(),
            frameworks: other.frameworks.iter().map(Library::from_schema).collect(),
        }
    }
}

/// Represents a dynamic library located somewhere on the system
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Library {
    /// The path to the library; on platforms without that information, it will be a basename instead
    pub path: String,
    /// The package from which a library comes, if relevant
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl Library {
    fn new(library: String) -> Self {
        Self {
            path: library,
            source: None,
        }
    }

    fn to_schema(&self) -> cargo_dist_schema::Library {
        cargo_dist_schema::Library {
            path: self.path.clone(),
            source: self.source.clone(),
        }
    }

    fn from_schema(other: &cargo_dist_schema::Library) -> Self {
        Self {
            path: other.path.clone(),
            source: other.source.clone(),
        }
    }

    fn from_homebrew(library: String) -> Self {
        // Doesn't currently support Homebrew installations in
        // non-default locations
        let brew_prefix = if library.starts_with("/opt/homebrew/opt/") {
            Some("/opt/homebrew/opt/")
        } else if library.starts_with("/usr/local/opt/") {
            Some("/usr/local/opt/")
        } else {
            None
        };

        if let Some(prefix) = brew_prefix {
            let cloned = library.clone();
            let stripped = cloned.strip_prefix(prefix).unwrap();
            let mut package = stripped.split('/').nth(0).unwrap().to_owned();

            // The path alone isn't enough to determine the tap the formula
            // came from. If the install receipt exists, we can use it to
            // get the name of the source tap.
            let receipt = Utf8PathBuf::from(&prefix)
                .join(&package)
                .join("INSTALL_RECEIPT.json");

            // If the receipt doesn't exist or can't be loaded, that's not an
            // error; we can fall back to the package basename we parsed out
            // of the path.
            if receipt.exists() {
                let _ = SourceFile::load_local(&receipt)
                    .and_then(|file| file.deserialize_json())
                    .map(|parsed: serde_json::Value| {
                        if let Some(tap) = parsed["source"]["tap"].as_str() {
                            if tap != "homebrew/core" {
                                package = format!("{tap}/{package}");
                            }
                        }
                    });
            }

            Self {
                path: library,
                source: Some(package.to_owned()),
            }
        } else {
            Self {
                path: library,
                source: None,
            }
        }
    }

    fn maybe_apt(library: String) -> DistResult<Self> {
        // We can't get this information on other OSs
        if std::env::consts::OS != "linux" {
            return Ok(Self {
                path: library,
                source: None,
            });
        }

        let process = Command::new("dpkg")
            .arg("--search")
            .arg(&library)
            .output()
            .into_diagnostic();
        match process {
            Ok(output) => {
                let output = String::from_utf8(output.stdout)?;

                let package = output.split(':').nth(0).unwrap();

                Ok(Self {
                    path: library,
                    source: Some(package.to_owned()),
                })
            }
            // Couldn't find a package for this file
            Err(_) => Ok(Self {
                path: library,
                source: None,
            }),
        }
    }

    fn to_string_pretty(&self) -> String {
        if let Some(package) = &self.source {
            format!("{} ({package})", self.path).to_owned()
        } else {
            self.path.clone()
        }
    }
}

fn do_otool(path: &Utf8PathBuf) -> DistResult<Vec<String>> {
    let mut libraries = vec![];

    let mut f = File::open(path)?;
    let mut buf = vec![];
    let size = f.read_to_end(&mut buf).unwrap();
    let mut cur = Cursor::new(&buf[..size]);
    if let OFile::MachFile {
        header: _,
        commands,
    } = OFile::parse(&mut cur).unwrap()
    {
        let commands = commands
            .iter()
            .map(|load| load.command())
            .cloned()
            .collect::<Vec<LoadCommand>>();

        for command in commands {
            match command {
                LoadCommand::IdDyLib(ref dylib)
                | LoadCommand::LoadDyLib(ref dylib)
                | LoadCommand::LoadWeakDyLib(ref dylib)
                | LoadCommand::ReexportDyLib(ref dylib)
                | LoadCommand::LoadUpwardDylib(ref dylib)
                | LoadCommand::LazyLoadDylib(ref dylib) => {
                    libraries.push(dylib.name.to_string());
                }
                _ => {}
            }
        }
    }

    Ok(libraries)
}

fn do_ldd(path: &Utf8PathBuf) -> DistResult<Vec<String>> {
    let mut libraries = vec![];

    let output = Command::new("ldd")
        .arg(path)
        .output()
        .expect("Unable to run ldd");

    let result = String::from_utf8_lossy(&output.stdout).to_string();
    let lines = result.trim_end().split('\n');

    for line in lines {
        let line = line.trim();

        // There's no dynamic linkage at all; we can safely break,
        // there will be nothing useful to us here.
        if line.starts_with("not a dynamic executable") {
            break;
        }

        // Not a library that actually concerns us
        if line.starts_with("linux-vdso") {
            continue;
        }

        // Format: libname.so.1 => /path/to/libname.so.1 (address)
        if let Some(path) = line.split(" => ").nth(1) {
            libraries.push((path.split(' ').next().unwrap()).to_owned());
        } else {
            continue;
        }
    }

    Ok(libraries)
}

fn do_pe(path: &Utf8PathBuf) -> DistResult<Vec<String>> {
    let buf = std::fs::read(path)?;
    match Object::parse(&buf)? {
        Object::PE(pe) => Ok(pe.libraries.into_iter().map(|s| s.to_owned()).collect()),
        _ => Err(DistError::LinkageCheckUnsupportedBinary {}),
    }
}

fn determine_linkage(path: &Utf8PathBuf, target: &str) -> DistResult<Linkage> {
    let libraries = match target {
        // Can be run on any OS
        "i686-apple-darwin" | "x86_64-apple-darwin" | "aarch64-apple-darwin" => do_otool(path)?,
        "i686-unknown-linux-gnu" | "x86_64-unknown-linux-gnu" | "aarch64-unknown-linux-gnu" => {
            // Currently can only be run on Linux
            if std::env::consts::OS != "linux" {
                return Err(DistError::LinkageCheckInvalidOS {
                    host: std::env::consts::OS.to_owned(),
                    target: target.to_owned(),
                });
            }
            do_ldd(path)?
        }
        // Can be run on any OS
        "i686-pc-windows-msvc" | "x86_64-pc-windows-msvc" | "aarch64-pc-windows-msvc" => {
            do_pe(path)?
        }
        _ => return Err(DistError::LinkageCheckUnsupportedBinary {}),
    };

    let mut linkage = Linkage {
        binary: path.file_name().unwrap().to_owned(),
        target: target.to_owned(),
        system: vec![],
        homebrew: vec![],
        public_unmanaged: vec![],
        frameworks: vec![],
        other: vec![],
    };
    for library in libraries {
        if library.starts_with("/opt/homebrew") {
            linkage
                .homebrew
                .push(Library::from_homebrew(library.clone()));
        } else if library.starts_with("/usr/lib") || library.starts_with("/lib") {
            linkage.system.push(Library::maybe_apt(library.clone())?);
        } else if library.starts_with("/System/Library/Frameworks")
            || library.starts_with("/Library/Frameworks")
        {
            linkage.frameworks.push(Library::new(library.clone()));
        } else if library.starts_with("/usr/local") {
            if std::fs::canonicalize(&library)?.starts_with("/usr/local/Cellar") {
                linkage
                    .homebrew
                    .push(Library::from_homebrew(library.clone()));
            } else {
                linkage.public_unmanaged.push(Library::new(library.clone()));
            }
        } else {
            linkage.other.push(Library::maybe_apt(library.clone())?);
        }
    }

    Ok(linkage)
}

/// Run any necessary integrity checks for "primary" commands like build/plan
///
/// (This is currently equivalent to `cargo dist generate --check`)
pub fn check_integrity(cfg: &Config) -> Result<()> {
    // We need to avoid overwriting any parts of configuration from CLI here,
    // so construct a clean copy of config to run the check generate
    let check_config = Config {
        needs_coherent_announcement_tag: false,
        artifact_mode: ArtifactMode::All,
        no_local_paths: false,
        allow_all_dirty: cfg.allow_all_dirty,
        targets: vec![],
        ci: vec![],
        installers: vec![],
        announcement_tag: None,
    };
    let dist = tasks::gather_work(&check_config)?;

    run_generate(
        &dist,
        &GenerateArgs {
            modes: vec![],
            check: true,
        },
    )
}

/// Build a cargo target
fn generate_installer(
    dist: &DistGraph,
    style: &InstallerImpl,
    manifests: &[DistManifest],
) -> Result<()> {
    match style {
        InstallerImpl::Shell(info) => {
            installer::shell::write_install_sh_script(&dist.templates, info)?
        }
        InstallerImpl::Powershell(info) => {
            installer::powershell::write_install_ps_script(&dist.templates, info)?
        }
        InstallerImpl::Npm(info) => installer::npm::write_npm_project(&dist.templates, info)?,
        InstallerImpl::Homebrew(info) => {
            installer::homebrew::write_homebrew_formula(&dist.templates, dist, info, manifests)?
        }
        InstallerImpl::Msi(info) => info.build()?,
    }
    Ok(())
}

/// Get the default list of targets
pub fn default_desktop_targets() -> Vec<String> {
    vec![
        // Everyone can build x64!
        axoproject::platforms::TARGET_X64_LINUX_GNU.to_owned(),
        axoproject::platforms::TARGET_X64_WINDOWS.to_owned(),
        axoproject::platforms::TARGET_X64_MAC.to_owned(),
        // Apple is really easy to cross from Apple
        axoproject::platforms::TARGET_ARM64_MAC.to_owned(),
        // other cross-compiles not yet supported
        // axoproject::platforms::TARGET_ARM64_LINUX_GNU.to_owned(),
        // axoproject::platforms::TARGET_ARM64_WINDOWS.to_owned(),
    ]
}
