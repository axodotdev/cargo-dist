//! TODO

//! TODO

pub mod axodotdev;
pub mod github;

use super::*;

use axodotdev::*;
use github::*;

/// TODO
#[derive(Debug, Default, Clone)]
pub struct HostConfig {
    /// TODO
    pub github: Option<GithubHostConfig>,
    /// TODO
    pub axodotdev: Option<AxodotdevHostConfig>,
}

/// TODO
#[derive(Debug, Clone)]
pub struct HostConfigInheritable {
    /// TODO
    pub common: CommonHostConfig,
    /// TODO
    pub github: Option<GithubHostLayer>,
    /// TODO
    pub axodotdev: Option<AxodotdevHostLayer>,
}

/// TODO
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct HostLayer {
    /// TODO
    #[serde(flatten)]
    pub common: CommonHostLayer,
    /// TODO
    pub github: Option<BoolOr<GithubHostLayer>>,
    /// TODO
    pub axodotdev: Option<BoolOr<AxodotdevHostLayer>>,
}
impl HostConfigInheritable {
    /// TODO
    pub fn defaults_for_package(workspaces: &WorkspaceGraph, pkg_idx: PackageIdx) -> Self {
        Self {
            common: CommonHostConfig::defaults_for_package(workspaces, pkg_idx),
            github: None,
            axodotdev: None,
        }
    }
    /// TODO
    pub fn apply_inheritance_for_package(
        self,
        workspaces: &WorkspaceGraph,
        pkg_idx: PackageIdx,
    ) -> HostConfig {
        let Self {
            common,
            github,
            axodotdev,
        } = self;
        let github = github.map(|github| {
            let mut default = GithubHostConfig::defaults_for_package(workspaces, pkg_idx, &common);
            default.apply_layer(github);
            default
        });
        let axodotdev = axodotdev.map(|axodotdev| {
            let mut default =
                AxodotdevHostConfig::defaults_for_package(workspaces, pkg_idx, &common);
            default.apply_layer(axodotdev);
            default
        });
        HostConfig { github, axodotdev }
    }
}
impl ApplyLayer for HostConfigInheritable {
    type Layer = HostLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            common,
            github,
            axodotdev,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.github.apply_bool_layer(github);
        self.axodotdev.apply_bool_layer(axodotdev);
    }
}

/// TODO
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CommonHostLayer {
    /// Always regard releases as stable
    ///
    /// (defaults to false)
    ///
    /// Ordinarily, cargo-dist tries to detect if your release
    /// is a prerelease based on its version number using
    /// semver standards. If it's a prerelease, it will be
    /// marked as a prerelease in hosting services such as
    /// GitHub and Axo Releases.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_latest: Option<bool>,

    /// Whether artifacts/installers for this app should be displayed in release bodies
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<bool>,

    /// How to refer to the app in release bodies
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}
/// TODO
#[derive(Debug, Default, Clone)]
pub struct CommonHostConfig {
    /// Always regard releases as stable
    pub force_latest: bool,

    /// Whether artifacts/installers for this app should be displayed in release bodies
    pub display: bool,

    /// How to refer to the app in release bodies
    pub display_name: String,
}
impl CommonHostConfig {
    /// TODO
    pub fn defaults_for_package(workspaces: &WorkspaceGraph, pkg_idx: PackageIdx) -> Self {
        let pkg = workspaces.package(pkg_idx);
        Self {
            force_latest: false,
            display: true,
            display_name: pkg.name.clone(),
        }
    }
}
impl ApplyLayer for CommonHostConfig {
    type Layer = CommonHostLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            force_latest,
            display,
            display_name,
        }: Self::Layer,
    ) {
        self.force_latest.apply_val(force_latest);
        self.display.apply_val(display);
        self.display_name.apply_val(display_name);
    }
}
impl ApplyLayer for CommonHostLayer {
    type Layer = CommonHostLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            force_latest,
            display,
            display_name,
        }: Self::Layer,
    ) {
        self.force_latest.apply_opt(force_latest);
        self.display.apply_opt(display);
        self.display_name.apply_opt(display_name);
    }
}
