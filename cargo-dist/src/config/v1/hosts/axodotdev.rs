//! axodotdev host config

use super::*;

/// axodotdev host (raw)
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AxodotdevHostLayer {
    /// Common options
    pub common: CommonHostLayer,
}
/// axodotdev host (final)
#[derive(Debug, Default, Clone)]
pub struct AxodotdevHostConfig {
    /// Common options
    pub common: CommonHostConfig,
}

impl AxodotdevHostConfig {
    /// Get defaults for the given workspace
    pub fn defaults_for_workspace(_workspaces: &WorkspaceGraph, common: &CommonHostConfig) -> Self {
        Self {
            common: common.clone(),
        }
    }
}

impl ApplyLayer for AxodotdevHostConfig {
    type Layer = AxodotdevHostLayer;
    fn apply_layer(&mut self, Self::Layer { common }: Self::Layer) {
        self.common.apply_layer(common);
    }
}
impl ApplyLayer for AxodotdevHostLayer {
    type Layer = AxodotdevHostLayer;
    fn apply_layer(&mut self, Self::Layer { common }: Self::Layer) {
        self.common.apply_layer(common);
    }
}

impl std::ops::Deref for AxodotdevHostConfig {
    type Target = CommonHostConfig;
    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
