//! Code for generating installer.sh

use axoasset::LocalAsset;

use crate::{backend::templates::TEMPLATE_INSTALLER_SH, errors::DistResult, DistGraph};

use super::InstallerInfo;

pub(crate) fn write_install_sh_script(dist: &DistGraph, info: &InstallerInfo) -> DistResult<()> {
    let mut info = info.clone();
    info.platform_support = Some(dist.release(info.release).platform_support.clone());

    let script = dist
        .templates
        .render_file_to_clean_string(TEMPLATE_INSTALLER_SH, &info)?;
    LocalAsset::write_new(&script, &info.dest_path)?;
    Ok(())
}
