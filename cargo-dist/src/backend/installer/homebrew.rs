//! Code for generating installer.sh

use axoasset::{LocalAsset, SourceFile};
use camino::Utf8PathBuf;
use cargo_dist_schema::DistManifest;
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
    manifests: &[DistManifest],
) -> DistResult<()> {
    let mut info = source_info.clone();

    // Collect all dist-manifests and fetch the appropriate Mac ones
    let mut manifests = manifests.to_owned();
    for file in graph.dist_dir.read_dir()? {
        let path = file?.path();
        if let Some(filename) = path.file_name() {
            if !filename.to_string_lossy().ends_with("-dist-manifest.json") {
                continue;
            }
        }

        let json_path = Utf8PathBuf::try_from(path)?;
        let data = SourceFile::load_local(json_path)?;
        let manifest: DistManifest = data.deserialize_json()?;

        if manifest.linkage.iter().any(|l| {
            info.arm64.is_some() && l.target == "aarch64-apple-darwin"
                || info.x86_64.is_some() && l.target == "x86_64-apple-darwin"
        }) {
            manifests.push(manifest);
        }
    }

    // Fetch any detected dependencies from the linkage data
    let dependencies = manifests.into_iter().flat_map(|m| {
        m.linkage
            .into_iter()
            .flat_map(|l| l.homebrew.into_iter().filter_map(|lib| lib.source))
    });

    // Merge with the manually-specified deps
    info.dependencies.extend(dependencies);

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
