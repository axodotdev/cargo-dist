//! TODO

use super::*;

/// Options for npm publishes
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct NpmPublisherLayer {
    /// Common options
    pub common: CommonPublisherLayer,
}
/// Options for npm publishes
#[derive(Debug, Default, Clone)]
pub struct NpmPublisherConfig {
    /// Common options
    pub common: CommonPublisherConfig,
}

impl NpmPublisherConfig {
    /// Get defaults for the given package
    pub fn defaults_for_package(
        _workspaces: &WorkspaceGraph,
        _pkg_idx: PackageIdx,
        common: &CommonPublisherConfig,
    ) -> Self {
        Self {
            common: common.clone(),
        }
    }
}

impl ApplyLayer for NpmPublisherConfig {
    type Layer = NpmPublisherLayer;
    fn apply_layer(&mut self, Self::Layer { common }: Self::Layer) {
        self.common.apply_layer(common);
    }
}
impl ApplyLayer for NpmPublisherLayer {
    type Layer = NpmPublisherLayer;
    fn apply_layer(&mut self, Self::Layer { common }: Self::Layer) {
        self.common.apply_layer(common);
    }
}

impl std::ops::Deref for NpmPublisherConfig {
    type Target = CommonPublisherConfig;
    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
