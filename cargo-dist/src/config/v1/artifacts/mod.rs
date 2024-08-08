//! TODO
//! TODO

pub mod archives;

use super::*;
use archives::*;

/// TODO
#[derive(Debug, Clone)]
pub struct ArtifactConfig {
    /// TODO
    pub archives: ArchiveConfig,
    /// Whether to generate and dist a tarball containing your app's source code
    pub source_tarball: bool,
    /// Any extra artifacts and their buildscripts
    pub extra: Vec<ExtraArtifact>,
    /// How to checksum
    pub checksum: ChecksumStyle,
}
/// TODO
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ArtifactLayer {
    /// TODO
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archives: Option<ArchiveLayer>,

    /// Whether to generate and dist a tarball containing your app's source code
    ///
    /// (defaults to true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_tarball: Option<bool>,

    /// Any extra artifacts and their buildscripts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Vec<ExtraArtifact>>,

    /// How to checksum
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<ChecksumStyle>,
}
impl ArtifactConfig {
    /// TODO
    pub fn defaults_for_package(workspaces: &WorkspaceGraph, pkg_idx: PackageIdx) -> Self {
        Self {
            archives: ArchiveConfig::defaults_for_package(workspaces, pkg_idx),
            source_tarball: true,
            extra: vec![],
            checksum: ChecksumStyle::Sha256,
        }
    }
}
impl ApplyLayer for ArtifactConfig {
    type Layer = ArtifactLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            archives,
            source_tarball,
            extra,
            checksum,
        }: Self::Layer,
    ) {
        self.archives.apply_val_layer(archives);
        self.source_tarball.apply_val(source_tarball);
        self.extra.apply_val(extra);
        self.checksum.apply_val(checksum);
    }
}
