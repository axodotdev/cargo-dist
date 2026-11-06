//! Code for generating installer.ps1

use axoasset::LocalAsset;
use cargo_dist_schema::DistManifest;
use serde::Serialize;

use crate::{backend::templates::TEMPLATE_INSTALLER_PS1, errors::DistResult, DistGraph, SortedMap};

use super::InstallerInfo;

#[derive(Serialize)]
struct PowershellInstallerInfo<'a> {
    #[serde(flatten)]
    installer: &'a InstallerInfo,
    checksums: SortedMap<&'a str, &'a str>,
}

pub(crate) fn write_install_ps_script(
    dist: &DistGraph,
    info: &InstallerInfo,
    manifest: &DistManifest,
) -> DistResult<()> {
    let checksums = if dist.local_builds_are_lies {
        // Fake builds produce checksums for placeholder archives, not the downloads.
        SortedMap::new()
    } else {
        // Every built archive has a SHA-256 in the manifest, independently of the
        // algorithm selected for checksum files. PowerShell can verify it natively.
        info.artifacts
            .iter()
            .filter_map(|artifact| {
                let checksum = manifest
                    .artifacts
                    .get(&artifact.id)?
                    .checksums
                    .get("sha256")?;
                Some((artifact.id.as_str(), checksum.as_str()))
            })
            .collect()
    };
    let context = PowershellInstallerInfo {
        installer: info,
        checksums,
    };
    let script = dist
        .templates
        .render_file_to_clean_string(TEMPLATE_INSTALLER_PS1, &context)?;
    LocalAsset::write_new(&script, &info.dest_path)?;
    dist.signer.sign(&info.dest_path)?;
    Ok(())
}
