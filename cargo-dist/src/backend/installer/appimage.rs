//! Appimage installer

use std::{fs::Permissions, io::BufRead, os::unix::fs::PermissionsExt};

use axoproject::Version;
use camino::Utf8PathBuf;
use itertools::Itertools;
use serde::Serialize;

use crate::{backend::templates::TEMPLATE_LINUX_DESKTOP, DistGraph, DistResult};

#[derive(Debug, Clone)]
/// Info needed to build an Appimage
pub struct AppImageInfo {
    /// An ideally unambiguous way to refer to a package for the purpose of cargo -p flags.
    pub pkg_spec: String,
    /// Final file path of the msi
    pub file_path: Utf8PathBuf,
    /// Dir stuff goes to
    pub package_dir: Utf8PathBuf,
    /// Binaries we'll be baking into the msi
    pub target: String,
    /// Path to the package Cargo.toml associated with this msi
    pub version: Version,
}

#[derive(Serialize)]
struct DesktopEntry {
    pub pkg: String,
    pub version: String,
}

impl AppImageInfo {
    /// Build the appimage installer
    pub fn build(&self, dist: &DistGraph) -> DistResult<()> {
        let bin_path = self.package_dir.join("usr").join("bin");
        std::fs::create_dir_all(&bin_path)?;
        let app_bin_path = bin_path.join(&self.pkg_spec);

        std::fs::rename(self.package_dir.join(&self.pkg_spec), &app_bin_path)?;

        let output = std::process::Command::new("ldd")
            .arg(app_bin_path)
            .output()?;

        assert!(output.status.success());

        let lib_path = self.package_dir.join("lib");
        let lib64_path = self.package_dir.join("lib64");
        std::fs::create_dir_all(&lib_path)?;
        std::fs::create_dir_all(&lib64_path)?;

        for dep in output.stdout.lines() {
            let dep = dep.unwrap();
            let temp = dep.trim().split_whitespace().collect_vec();
            let lib = match temp.len() {
                2 => temp[0],
                4 => temp[2],
                _ => unreachable!(),
            };

            if lib.starts_with("/lib64/") {
                let fname = lib.strip_prefix("/lib64/").unwrap();
                std::fs::copy(lib, lib64_path.join(fname))?;
            } else if lib.starts_with("/lib/") {
                let fname = lib.strip_prefix("/lib/").unwrap();
                std::fs::copy(lib, lib_path.join(fname))?;
            }
        }

        let desktop_vals = DesktopEntry {
            pkg: self.pkg_spec.clone(),
            version: self.version.to_string(),
        };

        let desktop_entry = dist
            .templates
            .render_file_to_clean_string(TEMPLATE_LINUX_DESKTOP, &desktop_vals)?;

        std::fs::write(
            self.package_dir.join(format!("{}.desktop", self.pkg_spec)),
            desktop_entry,
        )?;

        // TODO: Add actual icon
        let icon_path = self.package_dir.join("icon.png");
        std::fs::write(icon_path, [])?;

        // TODO: Maybe generate our own AppRun
        let app_run_path = self.package_dir.join("AppRun");
        let handle = tokio::runtime::Handle::current();
        handle.block_on(async {
            dist.axoclient.load_and_write_to_file(
                "https://raw.githubusercontent.com/AppImage/AppImageKit/master/resources/AppRun",
                &app_run_path
            ).await
        })?;
        std::fs::set_permissions(app_run_path, Permissions::from_mode(0777))?;

        let output = std::process::Command::new("appimagetool")
            .args([&self.package_dir, &self.file_path])
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            Err(crate::DistError::MissingAppImageTool)
        }
    }
}
