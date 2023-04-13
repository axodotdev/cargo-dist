//! Installer Generation
//!
//! In the future this might get split up into submodules.

use std::fs::File;

use miette::{Context, IntoDiagnostic};
use newline_converter::{dos2unix, unix2dos};

use crate::{DistGraph, InstallerInfo, ZipStyle};

////////////////////////////////////////////////////////////////
// Shell Installer
////////////////////////////////////////////////////////////////

pub(crate) fn generate_install_sh_script(
    dist: &DistGraph,
    info: &InstallerInfo,
) -> Result<(), miette::Report> {
    let installer_file = &info.dest_path;
    let mut file = File::create(installer_file)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to create installer file {installer_file}"))?;
    write_install_sh_script(&mut file, dist, info)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to write to installer file {installer_file}"))?;
    Ok(())
}

fn write_install_sh_script<W: std::io::Write>(
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

    let platform_entry_template = r###"
        "{{TARGET}}")
            _artifact_name="{{ARTIFACT_NAME}}"
            _zip_ext="{{ZIP_EXT}}"
            _bins="{{BINS}}"
            ;;"###;

    let mut entries = String::new();

    // If they have an x64 macos build but not an arm64 one, add a fallback entry
    // to try to install x64 on arm64 and let rosetta deal with it
    const X64_MACOS: &str = "x86_64-apple-darwin";
    const ARM64_MACOS: &str = "aarch64-apple-darwin";
    let has_x64_apple = artifacts
        .iter()
        .any(|a| a.target_triples.iter().any(|s| s == X64_MACOS));
    let has_arm_apple = artifacts
        .iter()
        .any(|a| a.target_triples.iter().any(|s| s == ARM64_MACOS));
    let do_rosetta_fallback = has_x64_apple && !has_arm_apple;

    for artifact in artifacts {
        assert!(artifact.target_triples.len() == 1, "It's awesome you made multi-arch executable-zips, but now you need to implement support in the sh installer!");
        let target = &artifact.target_triples[0];
        assert_eq!(artifact.zip_style, ZipStyle::Tar(crate::CompressionImpl::Xzip), "If you're trying to make zip styles configurable, but now you need to implement support in the sh installer!");
        let zip_ext = artifact.zip_style.ext();
        let artifact_name = &artifact.id;

        let mut bins = String::new();
        let mut multi_bin = false;
        for bin in &artifact.binaries {
            // FIXME: we should really stop pervasively assuming things are copied to the root...
            let rel_path = bin;
            if multi_bin {
                bins.push(' ');
            } else {
                multi_bin = true;
            }
            bins.push_str(rel_path);
        }

        let entry = platform_entry_template
            .replace("{{TARGET}}", target)
            .replace("{{ARTIFACT_NAME}}", artifact_name)
            .replace("{{BINS}}", &bins)
            .replace("{{ZIP_EXT}}", zip_ext);
        entries.push_str(&entry);

        if do_rosetta_fallback && target == X64_MACOS {
            let entry = platform_entry_template
                .replace("{{TARGET}}", ARM64_MACOS)
                .replace("{{ARTIFACT_NAME}}", artifact_name)
                .replace("{{BINS}}", &bins)
                .replace("{{ZIP_EXT}}", zip_ext);
            entries.push_str(&entry);
        }
    }

    let install_script = include_str!("../templates/installer.sh");
    let install_script = install_script
        .replace("{{APP_NAME}}", app_name)
        .replace("{{APP_VERSION}}", app_version)
        .replace("{{ARTIFACT_DOWNLOAD_URL}}", base_url)
        .replace("{{PLATFORM_INFO}}", &entries);

    f.write_all(dos2unix(&install_script).as_bytes())?;

    Ok(())
}

////////////////////////////////////////////////////////////////
// Powershell Installer
////////////////////////////////////////////////////////////////

pub(crate) fn generate_install_ps_script(
    dist: &DistGraph,
    info: &InstallerInfo,
) -> Result<(), miette::Report> {
    let installer_file = &info.dest_path;
    let mut file = File::create(installer_file)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to create installer file {installer_file}"))?;
    write_install_ps_script(&mut file, dist, info)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to write to installer file {installer_file}"))?;
    Ok(())
}

fn write_install_ps_script<W: std::io::Write>(
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
    let install_script = include_str!("../templates/installer.ps1");
    let install_script = install_script
        .replace("{{APP_NAME}}", app_name)
        .replace("{{APP_VERSION}}", app_version)
        .replace("{{ARTIFACT_DOWNLOAD_URL}}", base_url)
        .replace("{{PLATFORM_INFO}}", &platform_info);

    f.write_all(unix2dos(&install_script).as_bytes())?;

    Ok(())
}
