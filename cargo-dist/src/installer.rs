//! Installer Generation
//!
//! In the future this might get split up into submodules.

use std::{fs::File, io::BufWriter};

use camino::Utf8Path;
use miette::{Context, IntoDiagnostic};
use newline_converter::{dos2unix, unix2dos};
use serde_json::json;

use crate::{DistGraph, InstallerInfo, NpmInstallerInfo};

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

////////////////////////////////////////////////////////////////
// NPM Installer
////////////////////////////////////////////////////////////////

/// Names of all the files we add to the npm installer's tarball,
/// to add them to the manifest.
pub const NPM_PACKAGE_CONTENTS: &[&str] = &[
    TEMPLATE1_NAME,
    TEMPLATE2_NAME,
    TEMPLATE3_NAME,
    TEMPLATE4_NAME,
    TEMPLATE5_NAME,
    TEMPLATE6_NAME,
];

const TEMPLATE1_NAME: &str = ".gitignore";
const TEMPLATE2_NAME: &str = "binary.js";
const TEMPLATE3_NAME: &str = "install.js";
const TEMPLATE4_NAME: &str = "npm-shrinkwrap.json";
const TEMPLATE5_NAME: &str = "package.json";
const TEMPLATE6_NAME: &str = "run.js";

const TEMPLATE1: &str = include_str!("../templates/npm/.gitignore");
const TEMPLATE2: &str = include_str!("../templates/npm/binary.js");
const TEMPLATE3: &str = include_str!("../templates/npm/install.js");
const TEMPLATE4: &str = include_str!("../templates/npm/npm-shrinkwrap.json");
const TEMPLATE5: &str = include_str!("../templates/npm/package.json");
const TEMPLATE6: &str = include_str!("../templates/npm/run.js");

pub(crate) fn generate_install_npm_project(
    _dist: &DistGraph,
    info: &NpmInstallerInfo,
) -> Result<(), miette::Report> {
    let zip_dir = &info.package_dir;
    apply_npm_templates(TEMPLATE1, zip_dir, TEMPLATE1_NAME, info)?;
    apply_npm_templates(TEMPLATE2, zip_dir, TEMPLATE2_NAME, info)?;
    apply_npm_templates(TEMPLATE3, zip_dir, TEMPLATE3_NAME, info)?;
    apply_npm_templates(TEMPLATE4, zip_dir, TEMPLATE4_NAME, info)?;
    apply_npm_templates(TEMPLATE5, zip_dir, TEMPLATE5_NAME, info)?;
    apply_npm_templates(TEMPLATE6, zip_dir, TEMPLATE6_NAME, info)?;

    Ok(())
}

fn apply_npm_templates(
    input: &str,
    target_dir: &Utf8Path,
    rel_path: &str,
    info: &NpmInstallerInfo,
) -> Result<(), miette::Report> {
    let file_path = target_dir.join(rel_path);
    let file = File::create(&file_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to create installer file {file_path}"))?;
    let mut f = BufWriter::new(file);

    // FIXME: escape these strings!?

    let package_name = format!("{}", json!(&info.npm_package_name));
    let package_version = format!("{}", json!(info.npm_package_version));
    let app_name = format!("{}", json!(&info.inner.app_name));
    let artifact_download_url = format!("{}", json!(&info.inner.base_url));

    let desc = info
        .npm_package_desc
        .as_ref()
        .map(|desc| format!(r#""description": {},"#, json!(desc)))
        .unwrap_or_default();
    let repository_url = info
        .npm_package_repository_url
        .as_ref()
        .map(|url| format!(r#""repository": {},"#, json!(url)))
        .unwrap_or_default();
    let homepage_url = info
        .npm_package_homepage_url
        .as_ref()
        .map(|url| format!(r#""homepage": {},"#, json!(url)))
        .unwrap_or_default();
    let license = info
        .npm_package_license
        .as_ref()
        .map(|license| format!(r#""license": {},"#, json!(license)))
        .unwrap_or_default();

    let authors = match info.npm_package_authors.len() {
        0 => String::new(),
        1 => format!(r#""author": {},"#, json!(&info.npm_package_authors[0])),
        _ => format!(r#""contributors": {},"#, json!(&info.npm_package_authors)),
    };

    let keywords = if info.npm_package_keywords.is_none() {
        String::new()
    } else {
        format!(r#""keywords": {},"#, json!(&info.npm_package_keywords))
    };

    let bin = format!(
        r#""bin": {{
    {}: "run.js"
  }},"#,
        json!(info.bin)
    );

    let platform_entry_template = r###"
  {{TARGET}}: {
    "artifact_name": {{ARTIFACT_NAME}},
    "bins": {{BINS}},
    "zip_ext": {{ZIP_EXT}}
  }"###;

    let mut platform_info = String::new();
    let last_platform = info.inner.artifacts.len() - 1;
    for (idx, artifact) in info.inner.artifacts.iter().enumerate() {
        assert!(artifact.target_triples.len() == 1, "It's awesome you made multi-arch executable-zips, but now you need to implement support in the npm installer!");
        let target = &artifact.target_triples[0];
        let zip_ext = artifact.zip_style.ext();
        let artifact_name = &artifact.id;
        let entry = platform_entry_template
            .replace("{{TARGET}}", &json!(target).to_string())
            .replace("{{ARTIFACT_NAME}}", &json!(artifact_name).to_string())
            .replace("{{BINS}}", &json!(&artifact.binaries).to_string())
            .replace("{{ZIP_EXT}}", &json!(zip_ext).to_string());
        platform_info.push_str(&entry);
        if idx != last_platform {
            platform_info.push(',');
        } else {
            platform_info.push('\n');
        }
    }
    let output = input
        .replace("{{PACKAGE_NAME}}", &package_name)
        .replace("{{PACKAGE_VERSION}}", &package_version)
        .replace("{{KEY_DESCRIPTION}}", &desc)
        .replace("{{KEY_REPOSITORY_URL}}", &repository_url)
        .replace("{{KEY_AUTHORS}}", &authors)
        .replace("{{KEY_LICENSE}}", &license)
        .replace("{{KEY_HOMEPAGE_URL}}", &homepage_url)
        .replace("{{KEY_KEYWORDS}}", &keywords)
        .replace("{{KEY_BIN}}", &bin)
        .replace("\"{{APP_NAME}}\"", &app_name)
        .replace("\"{{ARTIFACT_DOWNLOAD_URL}}\"", &artifact_download_url)
        .replace("/*PLATFORM_INFO*/", &platform_info);

    {
        use std::io::Write;
        f.write_all(dos2unix(&output).as_bytes())
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to write to installer file {file_path}"))?;
    }

    Ok(())
}
