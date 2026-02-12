//! Mirror host

use super::*;

/// Mirror host config (raw)
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MirrorHostLayer {
    /// Common options
    #[serde(flatten)]
    pub common: CommonHostLayer,

    /// URL to download from
    pub download_url: Option<String>,
}

/// Mirror host config (final)
#[derive(Debug, Default, Clone)]
pub struct MirrorHostConfig {
    /// Common options
    pub common: CommonHostConfig,

    /// URL to download from
    pub download_url: String,
}

impl MirrorHostConfig {
    /// Get defaults for the given package
    pub fn defaults_for_workspace(_workspaces: &WorkspaceGraph, common: &CommonHostConfig) -> Self {
        Self {
            common: common.clone(),
            download_url: String::new(),
        }
    }
}

impl ApplyLayer for MirrorHostConfig {
    type Layer = MirrorHostLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            common,
            download_url,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.download_url.apply_val(download_url);
    }
}
impl ApplyLayer for MirrorHostLayer {
    type Layer = MirrorHostLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            common,
            download_url,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.download_url.apply_opt(download_url);
    }
}

impl std::ops::Deref for MirrorHostConfig {
    type Target = CommonHostConfig;
    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
