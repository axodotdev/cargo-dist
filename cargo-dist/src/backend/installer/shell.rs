//! Code for generating installer.sh

use axoasset::LocalAsset;

use crate::{
    backend::templates::{Templates, TEMPLATE_INSTALLER_SH},
    errors::DistResult,
};

use super::InstallerInfo;

pub(crate) fn write_install_sh_script(
    templates: &Templates,
    info: &InstallerInfo,
) -> DistResult<()> {
    let script = templates.render_file_to_clean_string(TEMPLATE_INSTALLER_SH, info)?;
    LocalAsset::write_new(&script, &info.dest_path)?;
    Ok(())
}
