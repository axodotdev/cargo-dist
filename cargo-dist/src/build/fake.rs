//! real fake binaries, no substance, all style
//!
//! used by --artifacts=lies to reproduce as much of our builds as possible
//! without needing to actually run platform-specific builds

use axoasset::LocalAsset;
use camino::Utf8PathBuf;
use cargo_dist_schema::DistManifest;

use crate::{BinaryIdx, CargoBuildStep, DistGraph, DistResult, GenericBuildStep};

use super::BuildExpectations;

/// pretend to build a cargo target
///
/// This produces empty binaries but otherwise emulates the build process as much as possible.
pub fn build_fake_cargo_target(
    dist: &DistGraph,
    manifest: &mut DistManifest,
    target: &CargoBuildStep,
) -> DistResult<()> {
    build_fake_binaries(dist, manifest, &target.expected_binaries)
}

/// build a fake generic target
///
/// This produces empty binaries but otherwise emulates the build process as much as possible.
pub fn build_fake_generic_target(
    dist: &DistGraph,
    manifest: &mut DistManifest,
    target: &GenericBuildStep,
) -> DistResult<()> {
    build_fake_binaries(dist, manifest, &target.expected_binaries)
}

/// build fake binaries, and emulate the build process as much as possible
fn build_fake_binaries(
    dist: &DistGraph,
    manifest: &mut DistManifest,
    binaries: &[BinaryIdx],
) -> DistResult<()> {
    // Shove these in a temp dir inside the dist dir, where it's safe for us to do whatever
    let tmp = temp_dir::TempDir::new()?;
    let tempdir =
        Utf8PathBuf::from_path_buf(tmp.path().to_owned()).expect("temp_dir made non-utf8 path!?");
    let mut expectations = BuildExpectations::new_fake(dist, binaries);

    for idx in binaries {
        let binary = dist.binary(*idx);
        let real_fake_bin = tempdir.join(&binary.file_name);
        let package_id = super::package_id_string(binary.pkg_id.as_ref());
        LocalAsset::write_new_all("", &real_fake_bin)?;
        expectations.found_bin(package_id, real_fake_bin, vec![]);
    }

    expectations.process_bins(dist, manifest)?;

    Ok(())
}
