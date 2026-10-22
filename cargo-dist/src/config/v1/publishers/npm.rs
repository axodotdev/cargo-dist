//! npm publisher config

use super::*;

/// Options for npm publishes
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct NpmPublisherLayer {
    /// Common options
    pub common: CommonPublisherLayer,
    /// Custom npm registry URL (e.g. "https://wombat-dressing-room.appspot.com")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<String>,
}
/// Options for npm publishes
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NpmPublisherConfig {
    /// Common options
    pub common: CommonPublisherConfig,
    /// Custom npm registry URL
    pub registry: Option<String>,
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
            registry: None,
        }
    }
}

impl ApplyLayer for NpmPublisherConfig {
    type Layer = NpmPublisherLayer;
    fn apply_layer(&mut self, Self::Layer { common, registry }: Self::Layer) {
        self.common.apply_layer(common);
        self.registry.apply_opt(registry);
    }
}
impl ApplyLayer for NpmPublisherLayer {
    type Layer = NpmPublisherLayer;
    fn apply_layer(&mut self, Self::Layer { common, registry }: Self::Layer) {
        self.common.apply_layer(common);
        self.registry.apply_opt(registry);
    }
}

impl std::ops::Deref for NpmPublisherConfig {
    type Target = CommonPublisherConfig;
    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
