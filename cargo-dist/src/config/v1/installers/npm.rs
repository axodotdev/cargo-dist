//! npm installer config

use super::*;

/// Options for npm installer (~raw config file contents)
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct NpmInstallerLayer {
    /// Common options
    #[serde(flatten)]
    pub common: CommonInstallerLayer,

    /// Replace the app's name with this value for the npm package's name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,

    /// A scope to prefix the npm package with (@ should be included).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,

    /// Whether to generate an npm-shrinkwrap.json
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shrinkwrap: Option<bool>,
}

/// Options for npm installer (final)
#[derive(Debug, Default, Clone)]
pub struct NpmInstallerConfig {
    /// Common options
    pub common: CommonInstallerConfig,

    /// The app's name with this value for the npm package's name
    pub package: String,

    /// A scope to prefix the npm package with (@ should be included).
    pub scope: Option<String>,

    /// Whether to generate an npm-shrinkwrap.json
    pub shrinkwrap: bool,
}

impl NpmInstallerConfig {
    /// Get defaults for the given package
    pub fn defaults_for_package(
        workspaces: &WorkspaceGraph,
        pkg_idx: PackageIdx,
        common: &CommonInstallerConfig,
    ) -> Self {
        let pkg = workspaces.package(pkg_idx);
        Self {
            common: common.clone(),
            package: pkg.name.clone(),
            scope: pkg.npm_scope.clone(),
            shrinkwrap: true,
        }
    }
}

impl ApplyLayer for NpmInstallerConfig {
    type Layer = NpmInstallerLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            common,
            scope,
            package,
            shrinkwrap,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.scope.apply_opt(scope);
        self.package.apply_val(package);
        self.shrinkwrap.apply_val(shrinkwrap);
    }
}
impl ApplyLayer for NpmInstallerLayer {
    type Layer = NpmInstallerLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            common,
            scope,
            package,
            shrinkwrap,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.scope.apply_opt(scope);
        self.package.apply_opt(package);
        self.shrinkwrap.apply_opt(shrinkwrap);
    }
}

impl std::ops::Deref for NpmInstallerConfig {
    type Target = CommonInstallerConfig;
    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
