//! TODO

use super::*;

/// Options for msi installer
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MsiInstallerLayer {
    /// Common options
    #[serde(flatten)]
    pub common: CommonInstallerLayer,
}
/// Options for msi installer
#[derive(Debug, Default, Clone)]
pub struct MsiInstallerConfig {
    /// Common options
    pub common: CommonInstallerConfig,
}

impl MsiInstallerConfig {
    /// Get defaults for the given package
    pub fn defaults_for_package(
        _workspaces: &WorkspaceGraph,
        _pkg_idx: PackageIdx,
        common: &CommonInstallerConfig,
    ) -> Self {
        Self {
            common: common.clone(),
        }
    }
}

impl ApplyLayer for MsiInstallerConfig {
    type Layer = MsiInstallerLayer;
    fn apply_layer(&mut self, Self::Layer { common }: Self::Layer) {
        self.common.apply_layer(common);
    }
}
impl ApplyLayer for MsiInstallerLayer {
    type Layer = MsiInstallerLayer;
    fn apply_layer(&mut self, Self::Layer { common }: Self::Layer) {
        self.common.apply_layer(common);
    }
}

impl std::ops::Deref for MsiInstallerConfig {
    type Target = CommonInstallerConfig;
    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
