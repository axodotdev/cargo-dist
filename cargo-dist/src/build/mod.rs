//! Compiling Things

use axoproject::PackageId;
use camino::Utf8PathBuf;
use cargo_dist_schema::{AssetInfo, DistManifest, TargetTripleRef};
use tracing::info;

use crate::{
    copy_file, linkage::determine_linkage, Binary, BinaryIdx, BinaryKind, DistError, DistGraph,
    DistResult, SortedMap,
};

pub mod cargo;
pub mod fake;
pub mod generic;

/// Output expectations for builds, and computed facts (all packages)
pub struct BuildExpectations {
    /// Expectations grouped by package
    pub packages: SortedMap<String, BinaryExpectations>,
    /// Whether this is fake (--artifacts=lies)
    fake: bool,
}

/// Output expectations for builds, and computed facts (one package)
#[derive(Default)]
pub struct BinaryExpectations {
    /// Expected binaries
    pub binaries: SortedMap<String, ExpectedBinary>,
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
    /// What kind of binary this is (executable, dynamic library, etc.)
    pub kind: BinaryKind,
}

impl BuildExpectations {
    /// Create a new BuildExpectations
    pub fn new(dist: &DistGraph, expected_binaries: &[BinaryIdx]) -> Self {
        let mut packages = SortedMap::<String, BinaryExpectations>::new();
        for &binary_idx in expected_binaries {
            let binary = &dist.binary(binary_idx);

            // Get the package id or an empty string (for generic builds)
            let package_id = package_id_string(binary.pkg_id.as_ref());
            let exe_name = binary.file_name.clone();

            packages.entry(package_id).or_default().binaries.insert(
                exe_name,
                ExpectedBinary {
                    idx: binary_idx,
                    src_path: None,
                    sym_paths: vec![],
                    kind: binary.kind,
                },
            );
        }

        Self {
            packages,
            fake: false,
        }
    }

    /// Create a new BuildExpectations, but don't sweat things being faked
    ///
    /// This is used for --artifacts=lies
    pub fn new_fake(dist: &DistGraph, expected_binaries: &[BinaryIdx]) -> Self {
        let mut out = Self::new(dist, expected_binaries);
        out.fake = true;
        out
    }

    /// Report that a binary was found, which might have been expected
    ///
    /// This subroutine is responsible for sorting out whether we care about the binary,
    /// and if the maybe_symbols are in fact symbols we care about.
    ///
    /// NOTE: it is correct/expected to hand this a bunch of random paths to things
    /// that vaguely might be what we want, assuming it knows how to pick the right ones out.
    pub fn found_bins(&mut self, pkg_id: String, filenames: Vec<Utf8PathBuf>) {
        // The files we're given by cargo are one big mush of
        // the executables, libraries, symbols, etc.
        // Try to partition this into what's probably symbols
        // and probably exes/libs
        let (maybe_symbols, maybe_bins): (Vec<_>, Vec<_>) = filenames
            .into_iter()
            // FIXME: unhardcode this when we add support for other symbol kinds!
            .partition(|f| f.extension().map(|e| e == "pdb").unwrap_or(false));

        // lookup the package
        let Some(pkg) = self.packages.get_mut(&pkg_id) else {
            return;
        };

        // NOTE: its expected for these early returns to trigger. It's this functions
        // jobs to sort through build outputs and grab the ones we actually care about.
        //
        // For instance if you build a cargo workspace (something we prefer for stability
        // of feature resolution), this can produce a bunch of binaries for examples or
        // packages you don't care about, which cargo/rustc will happily report back to us,
        // and we need to be aware enough to throw those irrelevant results out.
        for src_path in maybe_bins {
            info!("got a new binary: {}", src_path);

            // lookup the binary in the package
            let Some(bin_name) = src_path.file_name().map(String::from) else {
                continue;
            };
            let Some(bin_result) = pkg.binaries.get_mut(&bin_name) else {
                continue;
            };
            // used to compare to the symbol stem further down
            let Some(bin_stem) = src_path.file_stem().map(String::from) else {
                continue;
            };

            // Cool, we expected this binary, register its location!
            bin_result.src_path = Some(src_path);

            // Also register symbols
            for sym_path in &maybe_symbols {
                let is_for_this_bin = sym_path
                    .file_stem()
                    .map(|stem| stem == bin_stem)
                    .unwrap_or(false);
                if !is_for_this_bin {
                    continue;
                }

                // These are symbols we expected! Save the path.
                bin_result.sym_paths.push(sym_path.to_owned());
            }
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
    pub fn process_bins(&self, dist: &DistGraph, manifest: &mut DistManifest) -> DistResult<()> {
        let mut missing = vec![];
        for (pkg_id, pkg) in &self.packages {
            for (bin_name, result_bin) in &pkg.binaries {
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
                self.compute_linkage_and_sign(dist, manifest, result_bin, &bin.target)?;

                // copy files to their final homes
                self.copy_assets(result_bin, bin)?;
            }
        }

        // FIXME: properly bulk these together instead of just returning the first
        #[allow(clippy::never_loop)]
        for (pkg_name, bin_name) in missing {
            return Err(DistError::MissingBinaries { pkg_name, bin_name });
        }

        Ok(())
    }

    // Compute the linkage info for this binary and sign it
    fn compute_linkage_and_sign(
        &self,
        dist: &DistGraph,
        manifest: &mut DistManifest,
        src: &ExpectedBinary,
        target: &TargetTripleRef,
    ) -> DistResult<()> {
        let src_path = src
            .src_path
            .as_ref()
            .expect("bin src_path should have been checked by caller");

        dist.signer.sign(src_path)?;

        // If we're faking it, don't run the linkage stuff
        let linkage = if self.fake {
            // FIXME: fake this more interestingly!
            let mut linkage = cargo_dist_schema::Linkage::default();
            linkage.other.insert(cargo_dist_schema::Library {
                path: "fakelib".to_owned(),
                source: None,
                package_manager: None,
            });
            linkage
        } else {
            determine_linkage(src_path, target)
        };

        let bin = dist.binary(src.idx);
        manifest.assets.insert(
            bin.id.clone(),
            AssetInfo {
                id: bin.id.clone(),
                name: bin.name.clone(),
                system: dist.system_id.clone(),
                linkage: Some(linkage),
                target_triples: vec![target.to_owned()],
            },
        );
        Ok(())
    }

    // Copy the assets for this binary
    fn copy_assets(&self, src: &ExpectedBinary, dests: &Binary) -> DistResult<()> {
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
}

fn package_id_string(id: Option<&PackageId>) -> String {
    id.map(ToString::to_string).unwrap_or_default()
}
