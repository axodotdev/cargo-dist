//! Code for generating npm-installer.tar.gz

// FIXME(#283): migrate this to minijinja (steal logic from oranda to load a whole dir)

use axoasset::LocalAsset;
use camino::{Utf8Path, Utf8PathBuf};
use newline_converter::dos2unix;
use serde::Serialize;
use serde_json::json;

use super::InstallerInfo;
use crate::{backend::templates::Templates, errors::Result};

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

const TEMPLATE1: &str = include_str!("../../../templates/installer/npm/.gitignore");
const TEMPLATE2: &str = include_str!("../../../templates/installer/npm/binary.js");
const TEMPLATE3: &str = include_str!("../../../templates/installer/npm/install.js");
const TEMPLATE4: &str = include_str!("../../../templates/installer/npm/npm-shrinkwrap.json");
const TEMPLATE5: &str = include_str!("../../../templates/installer/npm/package.json");
const TEMPLATE6: &str = include_str!("../../../templates/installer/npm/run.js");

/// Info about an npm installer
#[derive(Debug, Clone, Serialize)]
pub struct NpmInstallerInfo {
    /// The name of the npm package
    pub npm_package_name: String,
    /// The version of the npm package
    pub npm_package_version: String,
    /// Short description of the package
    pub npm_package_desc: Option<String>,
    /// URL to repository
    pub npm_package_repository_url: Option<String>,
    /// URL to homepage
    pub npm_package_homepage_url: Option<String>,
    /// Short description of the package
    pub npm_package_authors: Vec<String>,
    /// Short description of the package
    pub npm_package_license: Option<String>,
    /// Array of keywords for this package
    pub npm_package_keywords: Option<Vec<String>>,
    /// Name of the binary this package installs (without .exe extension)
    pub bin: String,
    /// Dir to build the package in
    pub package_dir: Utf8PathBuf,
    /// Generic installer info
    pub inner: InstallerInfo,
}

pub(crate) fn write_install_npm_project(
    _templates: &Templates,
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

    let output = dos2unix(&output);
    let file_path = target_dir.join(rel_path);
    LocalAsset::write_new(&output, file_path)?;

    Ok(())
}
