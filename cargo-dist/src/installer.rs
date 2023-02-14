//! Installer Generation
//!
//! In the future this might get split up into submodules.

use std::fs::File;

use miette::{Context, IntoDiagnostic};

use crate::InstallerInfo;

////////////////////////////////////////////////////////////////
// Github Shell
////////////////////////////////////////////////////////////////

pub(crate) fn generate_github_install_sh_script(
    info: &InstallerInfo,
) -> Result<(), miette::Report> {
    let installer_file = &info.dest_path;
    let mut file = File::create(installer_file)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to create installer file {installer_file}"))?;
    write_github_install_sh_script(&mut file, info)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to write to installer file {installer_file}"))?;
    Ok(())
}

fn write_github_install_sh_script(
    f: &mut File,
    info: &InstallerInfo,
) -> Result<(), std::io::Error> {
    use std::io::Write;
    let InstallerInfo {
        app_name,
        app_version,
        base_name,
        base_url,
        ..
    } = info;

    let install_script = include_str!("installer.sh");
    let install_script = install_script
        .replace("{{APP_NAME}}", app_name)
        .replace("{{APP_VERSION}}", app_version)
        .replace("{{ARTIFACT_BASE_NAME}}", base_name)
        .replace("{{ARTIFACT_DOWNLOAD_URL}}", base_url);

    f.write_all(install_script.as_bytes())?;

    Ok(())
}

////////////////////////////////////////////////////////////////
// Github Powershell
////////////////////////////////////////////////////////////////

pub(crate) fn generate_github_install_ps_script(
    info: &InstallerInfo,
) -> Result<(), miette::Report> {
    let installer_file = &info.dest_path;
    let mut file = File::create(installer_file)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to create installer file {installer_file}"))?;
    write_github_install_ps_script(&mut file, info)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to write to installer file {installer_file}"))?;
    Ok(())
}

fn write_github_install_ps_script(
    f: &mut File,
    info: &InstallerInfo,
) -> Result<(), std::io::Error> {
    use std::io::Write;
    let InstallerInfo {
        app_name,
        app_version,
        base_name,
        base_url,
        ..
    } = info;

    let install_script = include_str!("installer.ps1");
    let install_script = install_script
        .replace("{{APP_NAME}}", app_name)
        .replace("{{APP_VERSION}}", app_version)
        .replace("{{ARTIFACT_BASE_NAME}}", base_name)
        .replace("{{ARTIFACT_DOWNLOAD_URL}}", base_url);

    f.write_all(install_script.as_bytes())?;

    Ok(())
}
