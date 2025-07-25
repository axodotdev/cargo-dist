//! artifact config

pub mod archives;

use super::*;
use archives::*;

/// app-specific artifact config (final)
#[derive(Debug, Clone)]
pub struct AppArtifactConfig {
    /// archive config
    pub archives: ArchiveConfig,
    /// Any extra artifacts and their buildscripts
    pub extra: Vec<ExtraArtifact>,
}

/// workspace artifact config (final)
#[derive(Debug, Clone)]
pub struct WorkspaceArtifactConfig {
    /// Whether to generate and dist a tarball containing your app's source code
    pub source_tarball: bool,
    /// Whether tarballs should include submodules
    pub recursive_tarball: bool,
    /// How to checksum
    pub checksum: ChecksumStyle,
}
/// artifact config (raw from file)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ArtifactLayer {
    /// archive config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archives: Option<ArchiveLayer>,

    /// Whether to generate and dist a tarball containing your app's source code
    ///
    /// (defaults to true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_tarball: Option<bool>,

    /// Whether source tarballs should include submodules
    ///
    /// (defaults to false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recursive_tarball: Option<bool>,

    /// Any extra artifacts and their buildscripts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Vec<ExtraArtifact>>,

    /// How to checksum
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<ChecksumStyle>,
}
impl AppArtifactConfig {
    /// get the defaults for a package
    pub fn defaults_for_package(workspaces: &WorkspaceGraph, pkg_idx: PackageIdx) -> Self {
        Self {
            archives: ArchiveConfig::defaults_for_package(workspaces, pkg_idx),
            extra: vec![],
        }
    }
}

impl WorkspaceArtifactConfig {
    /// get the defaults for a workspace
    pub fn defaults_for_workspace(_workspaces: &WorkspaceGraph) -> Self {
        Self {
            source_tarball: true,
            recursive_tarball: false,
            checksum: ChecksumStyle::Sha256,
        }
    }
}

impl ApplyLayer for AppArtifactConfig {
    type Layer = ArtifactLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            archives,
            extra,
            // these are all workspace-only
            source_tarball: _,
            recursive_tarball: _,
            checksum: _,
        }: Self::Layer,
    ) {
        self.archives.apply_val_layer(archives);
        self.extra.apply_val(extra);
    }
}

impl ApplyLayer for WorkspaceArtifactConfig {
    type Layer = ArtifactLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            source_tarball,
            recursive_tarball,
            checksum,
            // these are all app-only
            archives: _,
            extra: _,
        }: Self::Layer,
    ) {
        self.source_tarball.apply_val(source_tarball);
        self.recursive_tarball.apply_val(recursive_tarball);
        self.checksum.apply_val(checksum);
    }
}
