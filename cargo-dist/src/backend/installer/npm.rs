//! Code for generating npm-installer.tar.gz

use axoasset::LocalAsset;
use camino::Utf8PathBuf;
use serde::Serialize;

use super::InstallerInfo;
use crate::{
    backend::templates::{Templates, TEMPLATE_INSTALLER_NPM},
    errors::DistResult,
};

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

pub(crate) fn write_npm_project(templates: &Templates, info: &NpmInstallerInfo) -> DistResult<()> {
    let zip_dir = &info.package_dir;
    let results = templates.render_dir_to_clean_strings(TEMPLATE_INSTALLER_NPM, info)?;
    for (relpath, rendered) in results {
        LocalAsset::write_new_all(&rendered, zip_dir.join(relpath))?;
    }

    Ok(())
}
