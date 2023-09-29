//! Code for generating installer.sh

use axoasset::LocalAsset;
use camino::Utf8PathBuf;
use serde::Serialize;

use super::InstallerInfo;
use crate::{
    backend::templates::{Templates, TEMPLATE_INSTALLER_RB},
    errors::DistResult,
    generate_checksum,
    installer::ExecutableZipFragment,
    tasks::DistGraph,
};

/// Info about a Homebrew formula
#[derive(Debug, Clone, Serialize)]
pub struct HomebrewInstallerInfo {
    /// The application's name
    pub name: String,
    /// Formula class name
    pub formula_class: String,
    /// The application's license, in SPDX format
    pub license: Option<String>,
    /// The URL to the application's homepage
    pub homepage: Option<String>,
    /// A brief description of the application
    pub desc: Option<String>,
    /// A GitHub repository to write the formula to, in owner/name format
    pub tap: Option<String>,
    /// AMD64 artifact
    pub x86_64: Option<ExecutableZipFragment>,
    /// sha256 of AMD64 artifact
    pub x86_64_sha256: Option<String>,
    /// ARM64 artifact
    pub arm64: Option<ExecutableZipFragment>,
    /// sha256 of ARM64 artifact
    pub arm64_sha256: Option<String>,
    /// Generic installer info
    pub inner: InstallerInfo,
    /// Additional packages to specify as dependencies
    pub dependencies: Vec<String>,
}

pub(crate) fn write_homebrew_formula(
    templates: &Templates,
    graph: &DistGraph,
    source_info: &HomebrewInstallerInfo,
) -> DistResult<()> {
    let mut info = source_info.clone();

    // Generate sha256 as late as possible; the artifacts might not exist
    // earlier to do that.
    if let Some(arm64_ref) = &info.arm64 {
        let path = Utf8PathBuf::from(&graph.dist_dir).join(&arm64_ref.id);
        if path.exists() {
            let sha256 = generate_checksum(&crate::config::ChecksumStyle::Sha256, &path)?;
            info.arm64_sha256 = Some(sha256);
        }
    }
    if let Some(x86_64_ref) = &info.x86_64 {
        let path = Utf8PathBuf::from(&graph.dist_dir).join(&x86_64_ref.id);
        if path.exists() {
            let sha256 = generate_checksum(&crate::config::ChecksumStyle::Sha256, &path)?;
            info.x86_64_sha256 = Some(sha256);
        }
    }

    let script = templates.render_file_to_clean_string(TEMPLATE_INSTALLER_RB, &info)?;
    LocalAsset::write_new(&script, &info.inner.dest_path)?;
    Ok(())
}
