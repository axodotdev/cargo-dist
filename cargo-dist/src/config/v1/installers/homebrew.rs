//! TODO

use super::*;

/// Options for homebrew installer
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct HomebrewInstallerLayer {
    /// Common options
    #[serde(flatten)]
    pub common: CommonInstallerLayer,
    /// A Homebrew tap to push the Homebrew formula to, if built
    pub tap: Option<String>,
    /// Customize the name of the Homebrew formula
    pub formula: Option<String>,
}
/// Options for homebrew installer
#[derive(Debug, Default, Clone)]
pub struct HomebrewInstallerConfig {
    /// Common options
    pub common: CommonInstallerConfig,
    /// A Homebrew tap to push the Homebrew formula to, if built
    pub tap: Option<String>,
    /// Customize the name of the Homebrew formula
    pub formula: Option<String>,
}

impl HomebrewInstallerConfig {
    /// Get defaults for the given package
    pub fn defaults_for_package(
        _workspaces: &WorkspaceGraph,
        _pkg_idx: PackageIdx,
        common: &CommonInstallerConfig,
    ) -> Self {
        Self {
            common: common.clone(),
            tap: None,
            formula: None,
        }
    }
}

impl ApplyLayer for HomebrewInstallerConfig {
    type Layer = HomebrewInstallerLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            common,
            tap,
            formula,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.tap.apply_opt(tap);
        self.formula.apply_opt(formula);
    }
}
impl ApplyLayer for HomebrewInstallerLayer {
    type Layer = HomebrewInstallerLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            common,
            tap,
            formula,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.tap.apply_opt(tap);
        self.formula.apply_opt(formula);
    }
}

impl std::ops::Deref for HomebrewInstallerConfig {
    type Target = CommonInstallerConfig;
    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
