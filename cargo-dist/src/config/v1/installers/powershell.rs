//! TODO

use super::*;

/// Options for homebrew installer
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PowershellInstallerLayer {
    /// Common options
    #[serde(flatten)]
    pub common: CommonInstallerLayer,
}
/// Options for homebrew installer
#[derive(Debug, Default, Clone)]
pub struct PowershellInstallerConfig {
    /// Common options
    pub common: CommonInstallerConfig,
}

impl PowershellInstallerConfig {
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

impl ApplyLayer for PowershellInstallerConfig {
    type Layer = PowershellInstallerLayer;
    fn apply_layer(&mut self, Self::Layer { common }: Self::Layer) {
        self.common.apply_layer(common);
    }
}
impl ApplyLayer for PowershellInstallerLayer {
    type Layer = PowershellInstallerLayer;
    fn apply_layer(&mut self, Self::Layer { common }: Self::Layer) {
        self.common.apply_layer(common);
    }
}

impl std::ops::Deref for PowershellInstallerConfig {
    type Target = CommonInstallerConfig;
    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
