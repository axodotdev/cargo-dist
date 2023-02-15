//! Installer Generation
//!
//! In the future this might get split up into submodules.

use std::fs::File;

use miette::{Context, IntoDiagnostic};

use crate::{DistGraph, InstallerInfo, ZipStyle};

////////////////////////////////////////////////////////////////
// Github Shell
////////////////////////////////////////////////////////////////

pub(crate) fn generate_github_install_sh_script(
    dist: &DistGraph,
    info: &InstallerInfo,
) -> Result<(), miette::Report> {
    let installer_file = &info.dest_path;
    let mut file = File::create(installer_file)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to create installer file {installer_file}"))?;
    write_github_install_sh_script(&mut file, dist, info)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to write to installer file {installer_file}"))?;
    Ok(())
}

fn write_github_install_sh_script<W: std::io::Write>(
    f: &mut W,
    _dist: &DistGraph,
    info: &InstallerInfo,
) -> Result<(), std::io::Error> {
    let InstallerInfo {
        app_name,
        app_version,
        artifacts,
        base_url,
        ..
    } = info;

    let mut base_name = None;
    for artifact in artifacts {
        assert!(artifact.target_triples.len() == 1, "It's awesome you made multi-arch executable-zips, but now you need to implement support in the sh installer!");
        let target = &artifact.target_triples[0];
        assert_eq!(artifact.zip_style, ZipStyle::Tar(crate::CompressionImpl::Xzip), "If you're trying to make zip styles configurable, but now you need to implement support in the sh installer!");
        let zip_ext = artifact.zip_style.ext();

        // temp hack for a commit or two: strip stuff to get basename
        base_name = Some(
            artifact
                .id
                .strip_suffix(zip_ext)
                .unwrap()
                .strip_suffix(target)
                .unwrap(),
        )

        /*
        let mut bins = String::new();
        let mut multi_bin = false;
        for (_bin, bin_path) in artifact.required_binaries {
            // FIXME: we should really stop pervasively assuming things are copied to the root...
            let rel_path = bin_path.file_name().unwrap();
            if multi_bin {
                bins.push_str(", ");
            } else {
                multi_bin = true;
            }
            write!(bins, "\"{}\"", rel_path).unwrap();
        }

        let entry = platform_entry_template
            .replace("{{TARGET}}", target)
            .replace("{{BINS}}", &bins)
            .replace("{{ZIP_EXT}}", zip_ext);
        entries.push_str(&entry);
        */
    }

    let install_script = include_str!("installer.sh");
    let install_script = install_script
        .replace("{{APP_NAME}}", app_name)
        .replace("{{APP_VERSION}}", app_version)
        .replace("{{ARTIFACT_BASE_NAME}}", base_name.unwrap())
        .replace("{{ARTIFACT_DOWNLOAD_URL}}", base_url);

    f.write_all(install_script.as_bytes())?;

    Ok(())
}

////////////////////////////////////////////////////////////////
// Github Powershell
////////////////////////////////////////////////////////////////

pub(crate) fn generate_github_install_ps_script(
    dist: &DistGraph,
    info: &InstallerInfo,
) -> Result<(), miette::Report> {
    let installer_file = &info.dest_path;
    let mut file = File::create(installer_file)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to create installer file {installer_file}"))?;
    write_github_install_ps_script(&mut file, dist, info)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to write to installer file {installer_file}"))?;
    Ok(())
}

fn write_github_install_ps_script<W: std::io::Write>(
    f: &mut W,
    _dist: &DistGraph,
    info: &InstallerInfo,
) -> Result<(), std::io::Error> {
    use std::fmt::Write;
    let InstallerInfo {
        app_name,
        app_version,
        artifacts,
        base_url,
        ..
    } = info;

    let platform_info_template = r###"@{{{ENTRIES}}
  }"###;
    let platform_entry_template = r###"
    "{{TARGET}}" = @{
      "artifact_name" = "{{ARTIFACT_NAME}}"
      "bins" = {{BINS}}
      "zip_ext" = "{{ZIP_EXT}}"
    }"###;

    let mut entries = String::new();
    for artifact in artifacts {
        assert!(artifact.target_triples.len() == 1, "It's awesome you made multi-arch executable-zips, but now you need to implement support in the ps1 installer!");
        let target = &artifact.target_triples[0];
        assert_eq!(artifact.zip_style, ZipStyle::Zip, "If you're trying to make zip styles configurable, but now you need to implement support in the ps1 installer!");
        let zip_ext = artifact.zip_style.ext();
        let artifact_name = &artifact.id;

        let mut bins = String::new();
        let mut multi_bin = false;
        for bin in &artifact.binaries {
            // FIXME: we should really stop pervasively assuming things are copied to the root...
            let rel_path = bin;
            if multi_bin {
                bins.push_str(", ");
            } else {
                multi_bin = true;
            }
            write!(bins, "\"{}\"", rel_path).unwrap();
        }

        let entry = platform_entry_template
            .replace("{{TARGET}}", target)
            .replace("{{ARTIFACT_NAME}}", artifact_name)
            .replace("{{BINS}}", &bins)
            .replace("{{ZIP_EXT}}", zip_ext);
        entries.push_str(&entry);
    }
    let platform_info = platform_info_template.replace("{{ENTRIES}}", &entries);
    let install_script = include_str!("installer.ps1");
    let install_script = install_script
        .replace("{{APP_NAME}}", app_name)
        .replace("{{APP_VERSION}}", app_version)
        .replace("{{ARTIFACT_DOWNLOAD_URL}}", base_url)
        .replace("{{PLATFORM_INFO}}", &platform_info);

    f.write_all(install_script.as_bytes())?;

    Ok(())
}
