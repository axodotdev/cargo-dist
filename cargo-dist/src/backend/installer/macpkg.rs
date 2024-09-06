//! Code for generating installer.pkg

use std::{collections::BTreeMap, fs};

use axoasset::LocalAsset;
use axoprocess::Cmd;
use camino::Utf8PathBuf;
use serde::Serialize;
use temp_dir::TempDir;
use tracing::info;

use crate::{create_tmp, DistResult};

use super::ExecutableZipFragment;

/// Info about a package installer
#[derive(Debug, Clone, Serialize)]
pub struct PkgInstallerInfo {
    /// ExecutableZipFragment for this variant
    pub artifact: ExecutableZipFragment,
    /// Identifier for the final installer
    pub identifier: String,
    /// Default install location
    pub install_location: String,
    /// Final file path of the pkg
    pub file_path: Utf8PathBuf,
    /// Dir stuff goes to
    pub package_dir: Utf8PathBuf,
    /// The app version
    pub version: String,
    /// Executable aliases
    pub bin_aliases: BTreeMap<String, Vec<String>>,
}

impl PkgInstallerInfo {
    /// Build the pkg installer
    pub fn build(&self) -> DistResult<()> {
        info!("building a pkg: {}", self.identifier);

        // We can't build directly from dist_dir because the
        // package installer wants the directory we feed it
        // to have the final package layout, which in this case
        // is going to be an FHS-ish path installed into a public
        // location. So instead we create a new tree with our stuff
        // like we want it, and feed that to pkgbuild.
        let (_build_dir, build_dir) = create_tmp()?;
        let bindir = build_dir.join("bin");
        LocalAsset::create_dir_all(&bindir)?;
        let libdir = build_dir.join("lib");
        LocalAsset::create_dir_all(&libdir)?;

        info!("Copying executables");
        for exe in &self.artifact.executables {
            info!("{} => {:?}", &self.package_dir.join(exe), bindir.join(exe));
            LocalAsset::copy_file_to_file(&self.package_dir.join(exe), bindir.join(exe))?;
        }
        #[cfg(unix)]
        for (bin, targets) in &self.bin_aliases {
            for target in targets {
                std::os::unix::fs::symlink(&bindir.join(bin), &bindir.join(target))?;
            }
        }
        for lib in self
            .artifact
            .cdylibs
            .iter()
            .chain(self.artifact.cstaticlibs.iter())
        {
            LocalAsset::copy_file_to_file(&self.package_dir.join(lib), libdir.join(lib))?;
        }

        // The path the two pkg files get placed in while building
        let pkg_output = TempDir::new()?;
        let pkg_output_path = pkg_output.path();
        let pkg_path = pkg_output_path.join("package.pkg");
        let product_path = pkg_output_path.join("product.pkg");

        let mut pkgcmd = Cmd::new("/usr/bin/pkgbuild", "create individual pkg");
        pkgcmd.arg("--root").arg(build_dir);
        pkgcmd.arg("--identifier").arg(&self.identifier);
        pkgcmd.arg("--install-location").arg(&self.install_location);
        pkgcmd.arg("--version").arg(&self.version);
        pkgcmd.arg(&pkg_path);
        // Ensures stdout from the build process doesn't taint the dist-manifest
        pkgcmd.stdout_to_stderr();
        pkgcmd.run()?;

        // OK, we've made a package. Now wrap it in a product pkg.
        let mut productcmd = Cmd::new("/usr/bin/productbuild", "create final product .pkg");
        productcmd.arg("--package").arg(&pkg_path);
        productcmd.arg(&product_path);
        productcmd.stdout_to_stderr();
        productcmd.run()?;

        fs::copy(&product_path, &self.file_path)?;

        Ok(())
    }
}
