//! Compiling Things

use axoproject::PackageId;
use camino::Utf8PathBuf;
use tracing::info;

use crate::{
    copy_file,
    linkage::{determine_linkage, Linkage},
    Binary, BinaryIdx, DistError, DistGraph, DistResult, FastMap, TargetTriple,
};

pub mod cargo;
pub mod generic;

/// Output expectations for builds, and computed facts (all packages)
pub struct BuildExpectations {
    /// Expectations grouped by package
    pub packages: FastMap<String, BinaryExpectations>,
}

/// Output expectations for builds, and computed facts (one package)
#[derive(Default)]
pub struct BinaryExpectations {
    /// Expected binaries
    pub binaries: FastMap<String, ExpectedBinary>,
}

/// Binaries we expect
pub struct ExpectedBinary {
    /// idx of the binary in the DistGraph
    pub idx: BinaryIdx,
    /// path to the binary in the build output
    ///
    /// Initially this is None, but should be Some by the end of the build from calls to found_bin
    pub src_path: Option<Utf8PathBuf>,
    /// paths to the symbols of this binary in the build output
    ///
    /// Initially this is empty, but should be Some by the end of the build from calls to found_bin
    pub sym_paths: Vec<Utf8PathBuf>,
    /// linkage for the binary
    ///
    /// Initially this is None, but can be Some by the end of process_bins
    pub linkage: Option<Linkage>,
}

impl BuildExpectations {
    /// Create a new BuildExpectations
    pub fn new(dist: &DistGraph, expected_binaries: &[BinaryIdx]) -> Self {
        let mut packages = FastMap::<String, BinaryExpectations>::new();
        for &binary_idx in expected_binaries {
            let binary = &dist.binary(binary_idx);

            // Get the package id or an empty string (for generic builds)
            let package_id = package_id_string(binary.pkg_id.as_ref());
            let exe_name = binary.name.clone();

            packages.entry(package_id).or_default().binaries.insert(
                exe_name,
                ExpectedBinary {
                    idx: binary_idx,
                    src_path: None,
                    sym_paths: vec![],
                    linkage: None,
                },
            );
        }

        Self { packages }
    }

    /// Report that a binary was found, which might have been expected
    ///
    /// This subroutine is responsible for sorting out whether we care about the binary,
    /// and if the maybe_symbols are in fact symbols we care about.
    pub fn found_bin(
        &mut self,
        pkg_id: String,
        src_path: Utf8PathBuf,
        maybe_symbols: Vec<Utf8PathBuf>,
    ) {
        info!("got a new binary: {}", src_path);

        // lookup the package
        let Some(pkg) = self.packages.get_mut(&pkg_id) else {
            return;
        };

        // lookup the binary in the package
        let Some(bin_name) = src_path.file_stem() else {
            return;
        };
        let Some(bin_result) = pkg.binaries.get_mut(bin_name) else {
            return;
        };

        // Cool, we expected this binary, register its location!
        bin_result.src_path = Some(src_path);

        // Also register symbols
        for sym_path in maybe_symbols {
            // FIXME: unhardcode this when we add support for other symbol kinds!
            let is_symbols = sym_path.extension().map(|e| e == "pdb").unwrap_or(false);
            if !is_symbols {
                continue;
            }

            // These are symbols we expected! Save the path.
            bin_result.sym_paths.push(sym_path);
        }
    }

    /// Assuming the build is now complete, process the binaries as needed
    ///
    /// Currently this is:
    ///
    /// * checking src_path was set by found_bin
    /// * computing linkage for the binary
    /// * copying the binary and symbols to their final homes
    ///
    /// In the future this may also include:
    ///
    /// * code signing / hashing
    /// * stripping
    pub fn process_bins(&mut self, dist: &DistGraph) -> DistResult<()> {
        let mut missing = vec![];
        for (pkg_id, pkg) in self.packages.iter_mut() {
            for (bin_name, result_bin) in pkg.binaries.iter_mut() {
                // If the src_path is missing, everything is bad
                let Some(src_path) = result_bin.src_path.as_deref() else {
                    missing.push((pkg_id.to_owned(), bin_name.to_owned()));
                    continue;
                };
                if !src_path.exists() {
                    missing.push((pkg_id.to_owned(), bin_name.to_owned()));
                    continue;
                }
                let bin = dist.binary(result_bin.idx);

                // compute linkage for the binary
                compute_linkage(result_bin, &bin.target)?;

                // copy files to their final homes
                copy_assets(result_bin, bin)?;
            }
        }

        // FIXME: properly bulk these together instead of just returning the first
        #[allow(clippy::never_loop)]
        for (pkg_name, bin_name) in missing {
            return Err(DistError::MissingBinaries { pkg_name, bin_name });
        }

        Ok(())
    }
}

// Compute the linkage info for this binary
fn compute_linkage(src: &mut ExpectedBinary, target: &TargetTriple) -> DistResult<()> {
    let src_path = src
        .src_path
        .as_ref()
        .expect("bin src_path should have been checked by caller");
    let linkage = determine_linkage(src_path, target)?;
    src.linkage = Some(linkage);
    Ok(())
}

// Copy the assets for this binary
fn copy_assets(src: &ExpectedBinary, dests: &Binary) -> DistResult<()> {
    // Copy the main binary
    let src_path = src
        .src_path
        .as_deref()
        .expect("bin src_path should have been checked by caller");
    for dest_path in &dests.copy_exe_to {
        copy_file(src_path, dest_path)?;
    }

    // Copy the symbols
    for sym_path in &src.sym_paths {
        for dest_path in &dests.copy_symbols_to {
            copy_file(sym_path, dest_path)?;
        }
    }

    Ok(())
}

fn package_id_string(id: Option<&PackageId>) -> String {
    id.map(ToString::to_string).unwrap_or_default()
}
