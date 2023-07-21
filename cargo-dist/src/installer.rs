//! Installer Generation
//!
//! In the future this might get split up into submodules.

use std::{fs::File, io::BufWriter};

use axoasset::LocalAsset;
use camino::Utf8Path;
use miette::{Context, IntoDiagnostic};
use newline_converter::dos2unix;
use serde::Serialize;
use serde_json::json;

use crate::{errors::*, TEMPLATE_INSTALLER_PS, TEMPLATE_INSTALLER_SH};
use crate::{InstallerInfo, NpmInstallerInfo};

#[derive(Serialize)]
struct PlatformEntry {
    target: String,
    artifact_name: String,
    zip_ext: String,
    bins: Vec<String>,
}

////////////////////////////////////////////////////////////////
// Shell Installer
////////////////////////////////////////////////////////////////

pub(crate) fn write_install_sh_script(
    templates: &minijinja::Environment,
    info: &InstallerInfo,
) -> DistResult<()> {
    let script = generate_install_sh_script(templates, info)?;
    LocalAsset::write_new(&script, &info.dest_path)?;
    Ok(())
}

fn generate_install_sh_script(
    templates: &minijinja::Environment,
    info: &InstallerInfo,
) -> DistResult<String> {
    let tmpl = templates.get_template(TEMPLATE_INSTALLER_SH)?;
    let rendered = tmpl.render(info)?;
    let final_script = dos2unix(&rendered).into_owned();
    Ok(final_script)
}

////////////////////////////////////////////////////////////////
// Powershell Installer
////////////////////////////////////////////////////////////////

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

////////////////////////////////////////////////////////////////
// NPM Installer
////////////////////////////////////////////////////////////////

// FIXME(#283): migrate this to minijinja (steal logic from oranda to load a whole dir)

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

pub(crate) fn write_install_npm_project(
    _templates: &minijinja::Environment,
    info: &NpmInstallerInfo,
) -> Result<()> {
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
) -> Result<()> {
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

    let platform_entry_template = r#"
  {{TARGET}}: {
    "artifact_name": {{ARTIFACT_NAME}},
    "bins": {{BINS}},
    "zip_ext": {{ZIP_EXT}}
  }"#;

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
