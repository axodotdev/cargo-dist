//! Code for generating installer.ps1

use axoasset::LocalAsset;
use newline_converter::dos2unix;

use crate::{backend::TEMPLATE_INSTALLER_PS, errors::DistResult};

use super::InstallerInfo;

pub(crate) fn write_install_ps_script(
    templates: &minijinja::Environment,
    info: &InstallerInfo,
) -> DistResult<()> {
    let script = generate_install_ps_script(templates, info)?;
    LocalAsset::write_new(&script, &info.dest_path)?;
    Ok(())
}

fn generate_install_ps_script(
    templates: &minijinja::Environment,
    info: &InstallerInfo,
) -> DistResult<String> {
    let tmpl = templates.get_template(TEMPLATE_INSTALLER_PS)?;
    let rendered = tmpl.render(info)?;
    // Intentionally making unixy newlines in powershell, it's supported and nice to be uniform
    let final_script = dos2unix(&rendered).into_owned();
    Ok(final_script)
}
