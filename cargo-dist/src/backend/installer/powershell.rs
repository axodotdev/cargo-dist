//! Code for generating installer.ps1

use axoasset::LocalAsset;

use crate::{
    backend::templates::{Templates, TEMPLATE_INSTALLER_PS1},
    errors::DistResult,
};

use super::InstallerInfo;

pub(crate) fn write_install_ps_script(
    templates: &Templates,
    info: &InstallerInfo,
) -> DistResult<()> {
    let script = templates.render_file_to_clean_string(TEMPLATE_INSTALLER_PS1, info)?;
    LocalAsset::write_new(&script, &info.dest_path)?;
    Ok(())
}
