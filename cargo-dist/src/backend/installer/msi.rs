//! msi installer

use axoasset::{toml_edit, LocalAsset};
use camino::{Utf8Path, Utf8PathBuf};
use cargo_dist_schema::TargetTriple;
use tracing::info;
use wix::print::{wxs::WxsRenders, RenderOutput};

use crate::{backend::diff_files, config, errors::*, DistGraph};

const METADATA_WIX: &str = "wix";
const WIX_GUID_KEYS: &[&str] = &["upgrade-guid", "path-guid"];
const WIX_LICENSE_KEYS: &[&str] = &["license", "eula"];

/// Info needed to build an msi
#[derive(Debug, Clone)]
pub struct MsiInstallerInfo {
    /// An ideally unambiguous way to refer to a package for the purpose of cargo -p flags.
    pub pkg_spec: String,
    /// Binaries we'll be baking into the msi
    pub target: TargetTriple,
    /// Final file path of the msi
    pub file_path: Utf8PathBuf,
    /// Dir stuff goes to
    pub package_dir: Utf8PathBuf,
    /// Path to the wxs file this installer uses
    pub wxs_path: Utf8PathBuf,
    /// Path to the package Cargo.toml associated with this msi
    pub manifest_path: Utf8PathBuf,
}

impl MsiInstallerInfo {
    /// Build the msi installer
    ///
    /// Note that this assumes `write_wsx_to_disk` was run beforehand (via `cargo dist generate`),
    /// which should be enforced by `check_wsx` (via `cargo dist generate --check`).
    pub fn build(&self, dist: &DistGraph) -> DistResult<()> {
        info!("building an msi: {}", self.file_path);

        let mut b = wix::create::Builder::new();
        // Build this specific package
        b.package(Some(&self.pkg_spec));
        // cargo-dist already did the build for us
        b.no_build(true);
        // It built with the `dist` profile
        b.profile(Some("dist"));
        // It explicitly built with this --target
        b.target(Some(self.target.as_str()));
        // We want the output to go here
        b.output(Some(self.file_path.as_str()));
        // Binaries are over here
        b.target_bin_dir(Some(self.package_dir.as_str()));
        // FIXME: Would love to give users better output from WiX but it prints to stdout,
        // which messes up `--output-format=json`
        b.capture_output(true);

        let exec = b.build();
        exec.run().map_err(|e| DistError::Wix {
            msi: self.file_path.file_name().unwrap().to_owned(),
            details: e,
        })?;

        assert!(self.file_path.exists());
        dist.signer.sign(&self.file_path)?;
        Ok(())
    }

    /// run `cargo wix print wxs` to get what the msi should contain
    pub fn generate_wxs_string(&self) -> DistResult<WxsRenders> {
        let mut b = wix::print::wxs::Builder::new();
        b.input(Some(self.manifest_path.as_str()));
        // Build this specific package
        b.package(Some(&self.pkg_spec));
        let output = self
            .manifest_path
            .parent()
            .unwrap()
            .join("wix")
            .join("main.wxs");
        b.output(Some(output.as_str()));
        let exec = b.build();
        let renders = exec.render().map_err(|e| DistError::WixInit {
            package: self.pkg_spec.clone(),
            details: e,
        })?;
        Ok(renders)
    }

    /// msi's impl of `cargo dist genenerate --check`
    pub fn check_config(&self) -> DistResult<()> {
        self.check_wix_guids()?;
        self.check_wxs()?;
        Ok(())
    }
    /// msi's impl of `cargo dist genenerate`
    pub fn write_config_to_disk(&self) -> DistResult<()> {
        self.write_wix_guids_to_disk()?;
        self.write_wxs_to_disk()?;
        Ok(())
    }

    /// Write the wxs to disk
    fn write_wxs_to_disk(&self) -> DistResult<()> {
        let file = &self.wxs_path;
        let rendered = self.generate_wxs_string()?;

        let WxsRenders { wxs, license, eula } = rendered;

        write_render(Some(wxs))?;
        write_render(license)?;
        write_render(eula)?;

        eprintln!("generated msi definition to {}", file);

        Ok(())
    }

    /// Check whether the new configuration differs from the config on disk
    /// writhout actually writing the result.
    fn check_wxs(&self) -> DistResult<()> {
        let rendered = self.generate_wxs_string()?;

        let WxsRenders { wxs, license, eula } = rendered;

        diff_render(Some(wxs))?;
        diff_render(license)?;
        diff_render(eula)?;

        Ok(())
    }

    /// Check that wix GUIDs are set in the package's Cargo.toml
    fn check_wix_guids(&self) -> DistResult<()> {
        // Ok we have changes to make, let's load the toml
        let mut package_toml = config::load_cargo_toml(&self.manifest_path)?;
        if update_wix_metadata(&mut package_toml) {
            Err(DistError::MissingWixGuids {
                manifest_path: self.manifest_path.clone(),
                keys: WIX_GUID_KEYS,
            })
        } else {
            Ok(())
        }
    }

    /// Write wix GUIDs to the package's Cargo.toml
    fn write_wix_guids_to_disk(&self) -> DistResult<()> {
        let mut package_toml = config::load_cargo_toml(&self.manifest_path)?;
        if update_wix_metadata(&mut package_toml) {
            config::save_cargo_toml(&self.manifest_path, package_toml)?;
        }
        Ok(())
    }
}

fn write_render(render: Option<RenderOutput>) -> DistResult<()> {
    let Some(render) = render else {
        return Ok(());
    };
    let path = render.path.expect("no path!?");
    let path = Utf8Path::from_path(&path).expect("non utf8 path");

    LocalAsset::write_new_all(&render.rendered, path)?;
    Ok(())
}

fn diff_render(render: Option<RenderOutput>) -> DistResult<()> {
    let Some(render) = render else {
        return Ok(());
    };
    let path = render.path.expect("no path!?");
    let path = Utf8Path::from_path(&path).expect("non utf8 path");

    diff_files(path, &render.rendered)?;
    Ok(())
}

/// Ensure [package.metadata.wix] has persisted GUIDs.
///
/// This ensures that regenerating the installer produces a stable result.
/// Returns whether modifications were made (and should be written to disk)
fn update_wix_metadata(package_toml: &mut toml_edit::DocumentMut) -> bool {
    let metadata = config::get_toml_metadata(package_toml, false);

    // Get the subtable
    let wix_metadata = &mut metadata[METADATA_WIX];
    // If there's no table, make one
    if !wix_metadata.is_table() {
        *wix_metadata = toml_edit::table();
    }
    let table = wix_metadata.as_table_mut().unwrap();

    // Ensure the GUIDs exist, generating them if not
    let mut modified = false;
    for key in WIX_GUID_KEYS {
        if !table.contains_key(key) {
            modified = true;
            let val = uuid::Uuid::new_v4()
                .as_hyphenated()
                .to_string()
                .to_uppercase();
            table.insert(key, toml_edit::value(val));
        }
    }
    // Default to disabling auto-license/eula logic
    for key in WIX_LICENSE_KEYS {
        if !table.contains_key(key) {
            modified = true;
            table.insert(key, toml_edit::value(false));
        }
    }

    modified
}
