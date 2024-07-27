//! TODO

use super::*;

/// Options for homebrew publishes
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct HomebrewPublisherLayer {
    /// Common options
    pub common: CommonPublisherLayer,
}
/// Options for homebrew publishes
#[derive(Debug, Default, Clone)]
pub struct HomebrewPublisherConfig {
    /// Common options
    pub common: CommonPublisherConfig,
}

impl HomebrewPublisherConfig {
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

impl ApplyLayer for HomebrewPublisherConfig {
    type Layer = HomebrewPublisherLayer;
    fn apply_layer(&mut self, Self::Layer { common }: Self::Layer) {
        self.common.apply_layer(common);
    }
}
impl ApplyLayer for HomebrewPublisherLayer {
    type Layer = HomebrewPublisherLayer;
    fn apply_layer(&mut self, Self::Layer { common }: Self::Layer) {
        self.common.apply_layer(common);
    }
}

impl std::ops::Deref for HomebrewPublisherConfig {
    type Target = CommonPublisherConfig;
    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
