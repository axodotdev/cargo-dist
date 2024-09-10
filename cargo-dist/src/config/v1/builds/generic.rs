//! generic build config

use super::*;

/// generic build config (final)
#[derive(Debug, Clone)]
pub struct GenericBuildConfig {
    /// inheritable fields
    pub common: CommonBuildConfig,
}

/// generic build config (raw from file)
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct GenericBuildLayer {
    /// inheritable fields
    #[serde(flatten)]
    pub common: CommonBuildLayer,
}

impl GenericBuildConfig {
    /// Get defaults for the given package
    pub fn defaults_for_package(
        _workspaces: &WorkspaceGraph,
        _pkg_idx: PackageIdx,
        common: &CommonBuildConfig,
    ) -> Self {
        Self {
            common: common.clone(),
        }
    }
}

impl ApplyLayer for GenericBuildConfig {
    type Layer = GenericBuildLayer;
    fn apply_layer(&mut self, Self::Layer { common }: Self::Layer) {
        self.common.apply_layer(common);
    }
}
impl ApplyLayer for GenericBuildLayer {
    type Layer = GenericBuildLayer;
    fn apply_layer(&mut self, Self::Layer { common }: Self::Layer) {
        self.common.apply_layer(common);
    }
}

impl std::ops::Deref for GenericBuildConfig {
    type Target = CommonBuildConfig;
    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
