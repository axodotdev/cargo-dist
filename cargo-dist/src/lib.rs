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
    process::Command,
};

use axoasset::LocalAsset;
use backend::{
    ci::CiInfo,
    installer::{self, homebrew::HomebrewInstallerInfo, npm::NpmInstallerInfo, InstallerImpl},
    templates::{TemplateEntry, TEMPLATE_INSTALLER_NPM},
};
use camino::{Utf8Path, Utf8PathBuf};
use cargo_dist_schema::{Asset, AssetKind, DistManifest, ExecutableAsset};
use config::{
    ArtifactMode, ChecksumStyle, CompressionImpl, Config, DirtyMode, GenerateMode, ZipStyle,
};
use semver::Version;
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

    // Run all the build steps
    for step in &dist.build_steps {
        run_build_step(&dist, step)?;
    }

    Ok(build_manifest(cfg, &dist))
}

/// Just generate the manifest produced by `cargo dist build` without building
pub fn do_manifest(cfg: &Config) -> Result<DistManifest> {
    check_integrity(cfg)?;
    let dist = gather_work(cfg)?;

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
            with_root,
        }) => zip_dir(src_path, dest_path, zip_style, with_root.as_deref()),
        BuildStep::GenerateInstaller(installer) => generate_installer(dist_graph, installer),
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
                    return Err(miette!("failed to find bin {} ({})", exe_name, package_id));
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
                        "failed to find symbols for bin {} ({})",
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

/// Arguments for `cargo dist generate` ([`do_generate][])
#[derive(Debug)]
pub struct GenerateArgs {
    /// Check whether the output differs without writing to disk
    pub check: bool,
    /// Which type(s) of config to generate
    pub modes: Vec<GenerateMode>,
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
                return Err(DistError::ContradictoryGenerateModes {
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
fn generate_installer(dist: &DistGraph, style: &InstallerImpl) -> Result<()> {
    match style {
        InstallerImpl::Shell(info) => {
            installer::shell::write_install_sh_script(&dist.templates, info)?
        }
        InstallerImpl::Powershell(info) => {
            installer::powershell::write_install_ps_script(&dist.templates, info)?
        }
        InstallerImpl::Npm(info) => installer::npm::write_npm_project(&dist.templates, info)?,
        InstallerImpl::Homebrew(info) => {
            installer::homebrew::write_homebrew_formula(&dist.templates, dist, info)?
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
