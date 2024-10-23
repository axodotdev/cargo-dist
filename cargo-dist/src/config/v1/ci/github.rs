//! github ci config

use cargo_dist_schema::TargetTriple;

use super::*;

/// github ci config (raw from file)
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct GithubCiLayer {
    /// Common options
    #[serde(flatten)]
    pub common: CommonCiLayer,

    /// Custom GitHub runners, mapped by triple target
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runners: Option<SortedMap<TargetTriple, String>>,

    /// Custom permissions for jobs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<SortedMap<String, GithubPermissionMap>>,

    /// Custom permissions for jobs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_setup: Option<String>,
}
/// github ci config (final)
#[derive(Debug, Default, Clone)]
pub struct GithubCiConfig {
    /// Common options
    pub common: CommonCiConfig,
    /// Custom GitHub runners, mapped by triple target
    pub runners: SortedMap<TargetTriple, String>,
    /// Custom permissions for jobs
    pub permissions: SortedMap<String, GithubPermissionMap>,
    /// Custom permissions for jobs
    pub build_setup: Option<String>,
}

impl GithubCiConfig {
    /// Get defaults for the given package
    pub fn defaults_for_workspace(_workspaces: &WorkspaceGraph, common: &CommonCiConfig) -> Self {
        Self {
            common: common.clone(),
            runners: Default::default(),
            permissions: Default::default(),
            build_setup: None,
        }
    }
}

impl ApplyLayer for GithubCiConfig {
    type Layer = GithubCiLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            common,
            runners,
            permissions,
            build_setup,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.runners.apply_val(runners);
        self.permissions.apply_val(permissions);
        self.build_setup.apply_opt(build_setup);
    }
}
impl ApplyLayer for GithubCiLayer {
    type Layer = GithubCiLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            common,
            runners,
            permissions,
            build_setup,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.runners.apply_opt(runners);
        self.permissions.apply_opt(permissions);
        self.build_setup.apply_opt(build_setup);
    }
}

impl std::ops::Deref for GithubCiConfig {
    type Target = CommonCiConfig;
    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
