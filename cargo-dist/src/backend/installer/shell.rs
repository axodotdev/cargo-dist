//! Code for generating installer.sh

use axoasset::LocalAsset;
use dist_schema::DistManifest;

use crate::{backend::templates::TEMPLATE_INSTALLER_SH, errors::DistResult, DistGraph};

use super::InstallerInfo;

pub(crate) fn write_install_sh_script(
    dist: &DistGraph,
    info: &InstallerInfo,
    manifest: &DistManifest,
) -> DistResult<()> {
    let mut info = info.clone();
    let platform_support = dist.release(info.release).platform_support.clone();

    info.platform_support = Some(if dist.local_builds_are_lies {
        // if local builds are lies, the artifacts that are "fake-built" have a different
        // checksum every time, so we can't use those in the generated installer
        platform_support
    } else {
        platform_support.with_checksums_from_manifest(manifest)
    });

    let script = dist
        .templates
        .render_file_to_clean_string(TEMPLATE_INSTALLER_SH, &info)?;
    LocalAsset::write_new(&script, &info.dest_path)?;
    Ok(())
}
