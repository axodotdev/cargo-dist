//! TODO

use super::*;

/// Options for shell installer
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ShellInstallerLayer {
    /// Common options
    #[serde(flatten)]
    pub common: CommonInstallerLayer,
}
/// Options for shell installer
#[derive(Debug, Default, Clone)]
pub struct ShellInstallerConfig {
    /// Common options
    pub common: CommonInstallerConfig,
}

impl ShellInstallerConfig {
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

impl ApplyLayer for ShellInstallerConfig {
    type Layer = ShellInstallerLayer;
    fn apply_layer(&mut self, Self::Layer { common }: Self::Layer) {
        self.common.apply_layer(common);
    }
}
impl ApplyLayer for ShellInstallerLayer {
    type Layer = ShellInstallerLayer;
    fn apply_layer(&mut self, Self::Layer { common }: Self::Layer) {
        self.common.apply_layer(common);
    }
}

impl std::ops::Deref for ShellInstallerConfig {
    type Target = CommonInstallerConfig;
    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
