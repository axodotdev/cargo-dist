//! Code for generating installer.ps1

use axoasset::LocalAsset;

use crate::{backend::templates::TEMPLATE_INSTALLER_PS1, errors::DistResult, DistGraph};

use super::InstallerInfo;

pub(crate) fn write_install_ps_script(dist: &DistGraph, info: &InstallerInfo) -> DistResult<()> {
    let script = dist
        .templates
        .render_file_to_clean_string(TEMPLATE_INSTALLER_PS1, info)?;
    LocalAsset::write_new(&script, &info.dest_path)?;
    dist.signer.sign(&info.dest_path)?;
    Ok(())
}
