#![deny(missing_docs)]
#![allow(clippy::single_match, clippy::result_large_err)]

//! # dist
//!
//! This is the library at the core of the 'dist' CLI. It currently mostly exists
//! for the sake of internal documentation/testing, and isn't intended to be used by anyone else.
//! That said, if you have a reason to use it, let us know!
//!
//! It's currently not terribly well-suited to being used as a pure library because it happily
//! writes to stderr/stdout whenever it pleases. Suboptimal for a library.

use std::io::Write;

use announce::TagSettings;
use axoasset::LocalAsset;
use axoprocess::Cmd;
use backend::{
    ci::CiInfo,
    installer::{
        self, macpkg::PkgInstallerInfo, msi::MsiInstallerInfo, HomebrewImpl, InstallerImpl,
    },
};
use build::{
    cargo::make_build_cargo_target_command,
    generic::{build_generic_target, run_extra_artifacts_build},
};
use build::{
    cargo::{build_cargo_target, rustup_toolchain},
    fake::{build_fake_cargo_target, build_fake_generic_target},
};
use camino::{Utf8Path, Utf8PathBuf};
use cargo_dist_schema::{ArtifactId, ChecksumValue, ChecksumValueRef, DistManifest, TripleName};
use config::{
    ArtifactMode, ChecksumStyle, CompressionImpl, Config, DirtyMode, GenerateMode, ZipStyle,
};
use console::Term;
use platform::targets::TARGET_ARM64_WINDOWS;
use semver::Version;
use temp_dir::TempDir;
use tracing::info;

use errors::*;
pub use init::{do_init, do_migrate, InitArgs};
pub use tasks::*;

pub mod announce;
pub mod backend;
pub mod build;
pub mod config;
pub mod env;
pub mod errors;
pub mod host;
mod init;
pub mod linkage;
pub mod manifest;
pub mod net;
pub mod platform;
pub mod sign;
pub mod tasks;
#[cfg(test)]
mod tests;

/// dist build -- actually build binaries and installers!
pub fn do_build(cfg: &Config) -> DistResult<DistManifest> {
    check_integrity(cfg)?;

    let (dist, mut manifest) = tasks::gather_work(cfg)?;

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
        if dist.local_builds_are_lies {
            build_fake(&dist, step, &mut manifest)?;
        } else {
            run_build_step(&dist, step, &mut manifest)?;
        }
    }

    // Next the global steps
    for step in &dist.global_build_steps {
        if dist.local_builds_are_lies {
            build_fake(&dist, step, &mut manifest)?;
        } else {
            run_build_step(&dist, step, &mut manifest)?;
        }
    }

    Ok(manifest)
}

/// Just generate the manifest produced by `dist build` without building
pub fn do_manifest(cfg: &Config) -> DistResult<DistManifest> {
    check_integrity(cfg)?;
    let (_dist, manifest) = gather_work(cfg)?;

    Ok(manifest)
}

/// Run some build step
fn run_build_step(
    dist_graph: &DistGraph,
    target: &BuildStep,
    manifest: &mut DistManifest,
) -> DistResult<()> {
    match target {
        BuildStep::Generic(target) => build_generic_target(dist_graph, manifest, target)?,
        BuildStep::Cargo(target) => build_cargo_target(dist_graph, manifest, target)?,
        BuildStep::Rustup(cmd) => rustup_toolchain(dist_graph, cmd)?,
        BuildStep::CopyFile(CopyStep {
            src_path,
            dest_path,
        }) => copy_file(src_path, dest_path)?,
        BuildStep::CopyDir(CopyStep {
            src_path,
            dest_path,
        }) => copy_dir(src_path, dest_path)?,
        BuildStep::CopyFileOrDir(CopyStep {
            src_path,
            dest_path,
        }) => copy_file_or_dir(src_path, dest_path)?,
        BuildStep::Zip(ZipDirStep {
            src_path,
            dest_path,
            zip_style,
            with_root,
        }) => zip_dir(src_path, dest_path, zip_style, with_root.as_deref())?,
        BuildStep::GenerateInstaller(installer) => {
            generate_installer(dist_graph, installer, manifest)?
        }
        BuildStep::Checksum(ChecksumImpl {
            checksum,
            src_path,
            dest_path,
            for_artifact,
        }) => generate_and_write_checksum(
            manifest,
            checksum,
            src_path,
            dest_path.as_deref(),
            for_artifact.as_ref(),
        )?,
        BuildStep::UnifiedChecksum(UnifiedChecksumStep {
            checksum,
            dest_path,
        }) => generate_unified_checksum(manifest, *checksum, dest_path)?,
        BuildStep::OmniborArtifactId(OmniborArtifactIdImpl {
            src_path,
            dest_path,
        }) => generate_omnibor_artifact_id(dist_graph, src_path, dest_path)?,
        BuildStep::GenerateSourceTarball(SourceTarballStep {
            committish,
            prefix,
            target,
            working_dir,
        }) => generate_source_tarball(dist_graph, committish, prefix, target, working_dir)?,
        BuildStep::Extra(target) => run_extra_artifacts_build(dist_graph, target)?,
        BuildStep::Updater(updater) => fetch_updater(dist_graph, updater)?,
    };
    Ok(())
}

const AXOUPDATER_ASSET_ROOT: &str =
    "https://github.com/axodotdev/axoupdater/releases/latest/download";
const AXOUPDATER_MINIMUM_VERSION: &str = "0.7.0";
const AXOUPDATER_GIT_URL: &str = "https://github.com/axodotdev/axoupdater.git";

/// Fetches an installer executable and installs it in the expected target path.
pub fn fetch_updater(dist_graph: &DistGraph, updater: &UpdaterStep) -> DistResult<()> {
    let ext = if updater.target_triple.is_windows() {
        ".zip"
    } else {
        ".tar.xz"
    };
    let expected_url = format!(
        "{AXOUPDATER_ASSET_ROOT}/axoupdater-cli-{}{ext}",
        updater.target_triple
    );

    let handle = tokio::runtime::Handle::current();
    let resp = handle
        .block_on(dist_graph.axoclient.head(&expected_url))
        .map_err(|_| DistError::AxoupdaterReleaseCheckFailed {})?;

    // If we have a prebuilt asset, use it
    if resp.status().is_success() {
        fetch_updater_from_binary(dist_graph, updater, &expected_url)
    // If we got a 404, there's no asset, so we have to build from source
    } else if resp.status() == axoasset::reqwest::StatusCode::NOT_FOUND {
        fetch_updater_from_source(dist_graph, updater)
    // Some unexpected result that wasn't 200 or 404
    } else {
        Err(DistError::AxoupdaterReleaseCheckFailed {})
    }
}

/// Builds an installer executable from source and installs it in the expected target path.
pub fn fetch_updater_from_source(dist_graph: &DistGraph, updater: &UpdaterStep) -> DistResult<()> {
    let (_tmp_dir, tmp_root) = create_tmp()?;

    // cargo-xwin can't currently build one of axoupdater's dependencies:
    // https://github.com/rust-cross/cargo-xwin/issues/76
    let host = cargo_dist_schema::target_lexicon::HOST;
    let target = updater.target_triple.parse()?;
    if host != target && updater.target_triple == TARGET_ARM64_WINDOWS {
        return Err(DistError::AxoupdaterInvalidCross {
            host: TripleName::new(host.to_string()),
            target: updater.target_triple.to_owned(),
        });
    }

    let Some(git) = &dist_graph.tools.git else {
        return Err(DistError::ToolMissing {
            tool: "git".to_owned(),
        });
    };
    // We can't use `cargo install` due to the cross-compile wrappers,
    // so fetch the repo ahead of time.
    let mut cmd = Cmd::new(&git.cmd, "fetch axoupdater");
    cmd.arg("clone").arg(AXOUPDATER_GIT_URL).arg(&tmp_root);
    cmd.run()?;

    let features = CargoTargetFeatures {
        default_features: true,
        features: CargoTargetFeatureList::List(vec!["tls_native_roots".to_owned()]),
    };
    let step = CargoBuildStep {
        target_triple: updater.target_triple.to_owned(),
        features,
        package: CargoTargetPackages::Workspace,
        profile: "dist".to_string(),
        rustflags: "".to_owned(),
        expected_binaries: vec![],
        working_dir: tmp_root.clone(),
    };
    let cargo = dist_graph.tools.cargo()?;
    let mut cmd = make_build_cargo_target_command(&host, &cargo.cmd, "", &step, false)?;
    cmd.arg("--bin").arg("axoupdater");

    cmd.run()?;

    // OK, now we have a binary in the tempdir
    let mut source = tmp_root.join("target").join("release").join("axoupdater");
    if updater.target_triple.is_windows() {
        source.set_extension("exe");
    }
    LocalAsset::copy_file_to_file(source, dist_graph.target_dir.join(&updater.target_filename))?;

    Ok(())
}

/// Creates a temporary directory, returning the directory and
/// its path as a Utf8PathBuf.
pub fn create_tmp() -> DistResult<(TempDir, Utf8PathBuf)> {
    let tmp_dir = TempDir::new()?;
    let tmp_root =
        Utf8PathBuf::from_path_buf(tmp_dir.path().to_owned()).expect("tempdir isn't utf8!?");
    Ok((tmp_dir, tmp_root))
}

/// Fetches an installer executable from a preexisting binary and installs it in the expected target path.
fn fetch_updater_from_binary(
    dist_graph: &DistGraph,
    updater: &UpdaterStep,
    asset_url: &str,
) -> DistResult<()> {
    let (_tmp_dir, tmp_root) = create_tmp()?;
    let zipball_target = tmp_root.join("archive");

    let handle = tokio::runtime::Handle::current();
    handle.block_on(
        dist_graph
            .axoclient
            .load_and_write_to_file(asset_url, &zipball_target),
    )?;
    let suffix = if updater.target_triple.is_windows() {
        ".exe"
    } else {
        ""
    };
    let requested_filename = format!("axoupdater{suffix}");

    let bytes = if asset_url.ends_with(".tar.xz") {
        LocalAsset::untar_xz_file(&zipball_target, &requested_filename)?
    } else if asset_url.ends_with(".tar.gz") {
        LocalAsset::untar_gz_file(&zipball_target, &requested_filename)?
    } else if asset_url.ends_with(".zip") {
        LocalAsset::unzip_file(&zipball_target, &requested_filename)?
    } else {
        let extension = Utf8PathBuf::from(asset_url)
            .extension()
            .unwrap_or("unable to determine")
            .to_owned();
        return Err(DistError::UnrecognizedCompression { extension });
    };

    let target = dist_graph.target_dir.join(&updater.target_filename);
    std::fs::write(target, bytes)?;

    Ok(())
}

fn build_fake(
    dist_graph: &DistGraph,
    target: &BuildStep,
    manifest: &mut DistManifest,
) -> DistResult<()> {
    match target {
        // These two are the meat: don't actually run these at all, just
        // fake them out
        BuildStep::Generic(target) => build_fake_generic_target(dist_graph, manifest, target)?,
        BuildStep::Cargo(target) => build_fake_cargo_target(dist_graph, manifest, target)?,
        // Never run rustup
        BuildStep::Rustup(_) => {}
        // Copying files is fairly safe
        BuildStep::CopyFile(CopyStep {
            src_path,
            dest_path,
        }) => copy_file(src_path, dest_path)?,
        BuildStep::CopyDir(CopyStep {
            src_path,
            dest_path,
        }) => copy_dir(src_path, dest_path)?,
        BuildStep::CopyFileOrDir(CopyStep {
            src_path,
            dest_path,
        }) => copy_file_or_dir(src_path, dest_path)?,
        // The remainder of these are mostly safe to run as fake steps
        BuildStep::Zip(ZipDirStep {
            src_path,
            dest_path,
            zip_style,
            with_root,
        }) => zip_dir(src_path, dest_path, zip_style, with_root.as_deref())?,
        BuildStep::GenerateInstaller(installer) => match installer {
            // MSI and pkg, unlike other installers, aren't safe to generate on any platform
            InstallerImpl::Msi(msi) => generate_fake_msi(dist_graph, msi, manifest)?,
            InstallerImpl::Pkg(pkg) => generate_fake_pkg(dist_graph, pkg, manifest)?,
            _ => generate_installer(dist_graph, installer, manifest)?,
        },
        BuildStep::Checksum(ChecksumImpl {
            checksum,
            src_path,
            dest_path,
            for_artifact,
        }) => generate_and_write_checksum(
            manifest,
            checksum,
            src_path,
            dest_path.as_deref(),
            for_artifact.as_ref(),
        )?,
        BuildStep::UnifiedChecksum(UnifiedChecksumStep {
            checksum,
            dest_path,
        }) => generate_unified_checksum(manifest, *checksum, dest_path)?,
        BuildStep::OmniborArtifactId(OmniborArtifactIdImpl {
            src_path,
            dest_path,
        }) => generate_omnibor_artifact_id(dist_graph, src_path, dest_path)?,
        // Except source tarballs, which are definitely not okay
        // We mock these because it requires:
        // 1. git to be installed;
        // 2. the app to be a git checkout
        // The latter case is true during CI, but might not be in other
        // circumstances. Notably, this fixes our tests during nix's builds,
        // which runs in an unpacked tarball rather than a git checkout.
        BuildStep::GenerateSourceTarball(SourceTarballStep {
            committish,
            prefix,
            target,
            working_dir,
        }) => generate_fake_source_tarball(dist_graph, committish, prefix, target, working_dir)?,
        // Or extra artifacts, which may involve real builds
        BuildStep::Extra(target) => run_fake_extra_artifacts_build(dist_graph, target)?,
        BuildStep::Updater(_) => unimplemented!(),
    }
    Ok(())
}

fn run_fake_extra_artifacts_build(dist: &DistGraph, target: &ExtraBuildStep) -> DistResult<()> {
    for artifact in &target.artifact_relpaths {
        let path = dist.dist_dir.join(artifact);
        LocalAsset::write_new_all("", &path)?;
    }

    Ok(())
}

fn generate_fake_msi(
    _dist: &DistGraph,
    msi: &MsiInstallerInfo,
    _manifest: &DistManifest,
) -> DistResult<()> {
    LocalAsset::write_new_all("", &msi.file_path)?;

    Ok(())
}

fn generate_fake_pkg(
    _dist: &DistGraph,
    pkg: &PkgInstallerInfo,
    _manifest: &DistManifest,
) -> DistResult<()> {
    LocalAsset::write_new_all("", &pkg.file_path)?;

    Ok(())
}

fn generate_omnibor_artifact_id(
    dist_graph: &DistGraph,
    src_path: &Utf8Path,
    dest_path: &Utf8Path,
) -> DistResult<()> {
    let omnibor = dist_graph.tools.omnibor()?;
    let mut cmd = Cmd::new(&omnibor.cmd, "generate an OmniBOR Artifact ID");
    cmd.arg("artifact")
        .arg("id")
        .arg("--format")
        .arg("short")
        .arg("--path")
        .arg(src_path);

    let output = cmd.output()?.stdout;
    let output = String::from_utf8_lossy(&output);

    LocalAsset::write_new_all(&output, dest_path)?;

    Ok(())
}

/// Generate a checksum for the src_path to dest_path
fn generate_and_write_checksum(
    manifest: &mut DistManifest,
    checksum: &ChecksumStyle,
    src_path: &Utf8Path,
    dest_path: Option<&Utf8Path>,
    for_artifact: Option<&ArtifactId>,
) -> DistResult<()> {
    let output = generate_checksum(checksum, src_path)?;
    if let Some(dest_path) = dest_path {
        let name = src_path.file_name().expect("hashing file with no name!?");
        write_checksum_file(&[(name, &output)], dest_path)?;
    }
    if let Some(artifact_id) = for_artifact {
        if let Some(artifact) = manifest.artifacts.get_mut(artifact_id) {
            artifact.checksums.insert(checksum.ext().to_owned(), output);
        }
    }
    Ok(())
}

/// Collect all checksums for all artifacts and write them to a unified checksum file
fn generate_unified_checksum(
    manifest: &DistManifest,
    checksum: ChecksumStyle,
    dest_path: &Utf8Path,
) -> DistResult<()> {
    let expected_checksum_ext = checksum.ext();
    let mut entries: Vec<(&str, &ChecksumValueRef)> = vec![];

    for artifact in manifest.artifacts.values() {
        let artifact_name = if let Some(artifact_name) = artifact.name.as_deref() {
            artifact_name
        } else {
            continue;
        };

        for (checksum_ext, checksum) in &artifact.checksums {
            if checksum_ext == expected_checksum_ext {
                entries.push((artifact_name.as_str(), checksum));
            }
        }
    }
    write_checksum_file(&entries, dest_path)?;

    Ok(())
}

/// Generate a checksum for the src_path and return it as a string
fn generate_checksum(checksum: &ChecksumStyle, src_path: &Utf8Path) -> DistResult<ChecksumValue> {
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
        ChecksumStyle::Sha3_256 => {
            let mut hasher = sha3::Sha3_256::new();
            hasher.update(&file_bytes);
            hasher.finalize().as_slice().to_owned()
        }
        ChecksumStyle::Sha3_512 => {
            let mut hasher = sha3::Sha3_512::new();
            hasher.update(&file_bytes);
            hasher.finalize().as_slice().to_owned()
        }
        ChecksumStyle::Blake2s => {
            let mut hasher = blake2::Blake2s256::new();
            hasher.update(&file_bytes);
            hasher.finalize().as_slice().to_owned()
        }
        ChecksumStyle::Blake2b => {
            let mut hasher = blake2::Blake2b512::new();
            hasher.update(&file_bytes);
            hasher.finalize().as_slice().to_owned()
        }
        ChecksumStyle::False => {
            unreachable!()
        }
    };
    let mut output = String::with_capacity(hash.len() * 2);
    for byte in hash {
        write!(&mut output, "{:02x}", byte).unwrap();
    }
    Ok(ChecksumValue::new(output))
}

/// Creates a source code tarball from the git archive from
/// tag/ref/commit `committish`, with the directory prefix `prefix`,
/// at the output file `target`.
fn generate_source_tarball(
    graph: &DistGraph,
    committish: &str,
    prefix: &str,
    target: &Utf8Path,
    working_dir: &Utf8Path,
) -> DistResult<()> {
    let git = if let Some(tool) = &graph.tools.git {
        tool.cmd.to_owned()
    } else {
        return Err(DistError::ToolMissing {
            tool: "git".to_owned(),
        });
    };

    Cmd::new(git, "generate a source tarball for your project")
        .arg("archive")
        .arg(committish)
        .arg("--format=tar.gz")
        .arg("--prefix")
        .arg(prefix)
        .arg("--output")
        .arg(target)
        .current_dir(working_dir)
        .run()?;

    Ok(())
}

fn generate_fake_source_tarball(
    _graph: &DistGraph,
    _committish: &str,
    _prefix: &str,
    target: &Utf8Path,
    _working_dir: &Utf8Path,
) -> DistResult<()> {
    LocalAsset::write_new_all("", target)?;

    Ok(())
}

/// Write the checksum to dest_path
fn write_checksum_file(
    entries: &[(&str, &ChecksumValueRef)],
    dest_path: &Utf8Path,
) -> DistResult<()> {
    // Tools like sha256sum expect a new-line-delimited format of
    // <checksum> <mode><path>
    //
    // * checksum is the checksum in hex
    // * mode is ` ` for "text" and `*` for "binary" — "text" is for CRLF support, we don't want it.
    // * path is a relative path to the thing being checksummed (usually just a filename)
    //
    // We also make sure there's a trailing newline as is traditional.
    //
    // By following this format we support commands like `sha256sum --check sha256.sum`,
    // both the GNU coreutils and Darwin variants, and also Perl `shasum` utility.
    let mut contents = String::new();
    for (file_path, checksum) in entries {
        use std::fmt::Write;
        writeln!(&mut contents, "{checksum} *{file_path}",).unwrap();
    }
    // leave a trailing newline
    contents.push('\n');

    axoasset::LocalAsset::write_new(&contents, dest_path)?;
    Ok(())
}

/// Initialize the dir for an artifact (and delete the old artifact file).
fn init_artifact_dir(_dist: &DistGraph, artifact: &Artifact) -> DistResult<()> {
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

pub(crate) fn copy_file(src_path: &Utf8Path, dest_path: &Utf8Path) -> DistResult<()> {
    LocalAsset::copy_file_to_file(src_path, dest_path)?;
    Ok(())
}

pub(crate) fn copy_dir(src_path: &Utf8Path, dest_path: &Utf8Path) -> DistResult<()> {
    LocalAsset::copy_dir_to_dir(src_path, dest_path)?;
    Ok(())
}

pub(crate) fn copy_file_or_dir(src_path: &Utf8Path, dest_path: &Utf8Path) -> DistResult<()> {
    if src_path.is_dir() {
        copy_dir(src_path, dest_path)
    } else {
        copy_file(src_path, dest_path)
    }
}

fn zip_dir(
    src_path: &Utf8Path,
    dest_path: &Utf8Path,
    zip_style: &ZipStyle,
    with_root: Option<&Utf8Path>,
) -> DistResult<()> {
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

/// Arguments for `dist generate` ([`do_generate`][])
#[derive(Debug)]
pub struct GenerateArgs {
    /// Check whether the output differs without writing to disk
    pub check: bool,
    /// Which type(s) of config to generate
    pub modes: Vec<GenerateMode>,
}

fn do_generate_preflight_checks(dist: &DistGraph) -> DistResult<()> {
    // Enforce cargo-dist-version, unless...
    //
    // * It's a magic vX.Y.Z-github-BRANCHNAME version,
    //   which we use for testing against a PR branch. In that case the current_version
    //   should be irrelevant (so sayeth the person who made and uses this feature).
    //
    // * The user passed --allow-dirty to the CLI (probably means it's our own tests)
    if let Some(desired_version) = &dist.config.dist_version {
        let current_version: Version = std::env!("CARGO_PKG_VERSION").parse().unwrap();
        if desired_version != &current_version
            && !desired_version.pre.starts_with("github-")
            && !matches!(dist.allow_dirty, DirtyMode::AllowAll)
        {
            return Err(DistError::MismatchedDistVersion {
                config_version: desired_version.to_string(),
                running_version: current_version.to_string(),
            });
        }
    }
    if !dist.is_init {
        return Err(DistError::NeedsInit);
    }

    Ok(())
}

/// Generate any scripts which are relevant (impl of `dist generate`)
pub fn do_generate(cfg: &Config, args: &GenerateArgs) -> DistResult<()> {
    let (dist, _manifest) = gather_work(cfg)?;

    run_generate(&dist, args)?;

    Ok(())
}

/// The inner impl of do_generate
pub fn run_generate(dist: &DistGraph, args: &GenerateArgs) -> DistResult<()> {
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

/// Run any necessary integrity checks for "primary" commands like build/plan
///
/// (This is currently equivalent to `dist generate --check`)
pub fn check_integrity(cfg: &Config) -> DistResult<()> {
    // We need to avoid overwriting any parts of configuration from CLI here,
    // so construct a clean copy of config to run the check generate
    let check_config = Config {
        // check the whole system is in a good state
        tag_settings: TagSettings {
            needs_coherence: false,
            // Keeping the tag ensures if dist is run in library mode, we
            // actually check things in library mode.
            // If we don't do this, `dist plan --tag={name}-{version} will
            // always fail if there's no bins.
            tag: cfg.tag_settings.tag.clone(),
        },
        // don't do side-effecting networking
        create_hosting: false,
        artifact_mode: ArtifactMode::All,
        no_local_paths: false,
        allow_all_dirty: cfg.allow_all_dirty,
        targets: vec![],
        ci: vec![],
        installers: vec![],
        root_cmd: "check".to_owned(),
    };
    let (dist, _manifest) = tasks::gather_work(&check_config)?;

    if let Some(hosting) = &dist.hosting {
        if hosting.hosts.contains(&config::HostingStyle::Axodotdev) {
            let mut out = Term::stderr();
            let info = "INFO:";
            let message = r"You've enabled Axo Releases, which is currently in Closed Beta.
If you haven't yet signed up, please join our discord
(https://discord.gg/ECnWuUUXQk) or message hello@axo.dev to get started!
";

            writeln!(out, "{} {}", out.style().yellow().apply_to(info), message).unwrap();
        }
    }

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
    manifest: &DistManifest,
) -> DistResult<()> {
    match style {
        InstallerImpl::Shell(info) => {
            installer::shell::write_install_sh_script(dist, info, manifest)?
        }
        InstallerImpl::Powershell(info) => {
            installer::powershell::write_install_ps_script(dist, info)?
        }
        InstallerImpl::Npm(info) => installer::npm::write_npm_project(dist, info)?,
        InstallerImpl::Homebrew(HomebrewImpl { info, fragments }) => {
            installer::homebrew::write_homebrew_formula(dist, info, fragments, manifest)?
        }
        InstallerImpl::Msi(info) => info.build(dist)?,
        InstallerImpl::Pkg(info) => info.build()?,
    }
    Ok(())
}

/// Get the default list of targets
pub fn default_desktop_targets() -> Vec<TripleName> {
    use crate::platform::targets as t;

    vec![
        // Everyone can build x64!
        t::TARGET_X64_LINUX_GNU.to_owned(),
        t::TARGET_X64_WINDOWS.to_owned(),
        t::TARGET_X64_MAC.to_owned(),
        t::TARGET_ARM64_MAC.to_owned(),
        t::TARGET_ARM64_LINUX_GNU.to_owned(),
        // that one requires a bit of config (use the `messense/cargo-xwin` image)
        // t::TARGET_ARM64_WINDOWS.to_owned(),
    ]
}

/// Get the list of all known targets
pub fn known_desktop_targets() -> Vec<TripleName> {
    use crate::platform::targets as t;

    vec![
        // Everyone can build x64!
        t::TARGET_X64_LINUX_GNU.to_owned(),
        t::TARGET_X64_LINUX_MUSL.to_owned(),
        t::TARGET_X64_WINDOWS.to_owned(),
        t::TARGET_X64_MAC.to_owned(),
        t::TARGET_ARM64_MAC.to_owned(),
        t::TARGET_ARM64_LINUX_GNU.to_owned(),
        t::TARGET_ARM64_WINDOWS.to_owned(),
    ]
}
