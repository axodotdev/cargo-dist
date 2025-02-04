//! user specified publisher config

use super::*;

/// Options for user specified publishes
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct UserPublisherLayer {
    /// Common options
    pub common: CommonPublisherLayer,
}
/// Options for user specified publishes
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct UserPublisherConfig {
    /// Common options
    pub common: CommonPublisherConfig,
}

impl UserPublisherConfig {
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

impl ApplyLayer for UserPublisherConfig {
    type Layer = UserPublisherLayer;
    fn apply_layer(&mut self, Self::Layer { common }: Self::Layer) {
        self.common.apply_layer(common);
    }
}
impl ApplyLayer for UserPublisherLayer {
    type Layer = UserPublisherLayer;
    fn apply_layer(&mut self, Self::Layer { common }: Self::Layer) {
        self.common.apply_layer(common);
    }
}

impl std::ops::Deref for UserPublisherConfig {
    type Target = CommonPublisherConfig;
    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
