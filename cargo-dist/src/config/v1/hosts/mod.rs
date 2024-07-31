//! TODO

//! TODO

pub mod axodotdev;
pub mod github;

use super::*;

use axodotdev::*;
use github::*;

#[derive(Debug, Clone)]
/// TODO
pub struct AppHostConfig {
    /// Whether artifacts/installers for this app should be displayed in release bodies
    pub display: bool,
    /// How to refer to the app in release bodies
    pub display_name: String,
}

#[derive(Debug, Clone)]
/// TODO
pub struct WorkspaceHostConfig {
    /// Always regard releases as stable
    pub force_latest: bool,
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
    /// Always regard releases as stable
    pub force_latest: Option<bool>,
    /// Whether artifacts/installers for this app should be displayed in release bodies
    pub display: Option<bool>,
    /// How to refer to the app in release bodies
    pub display_name: Option<String>,
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
            force_latest: None,
            display: None,
            display_name: None,
        }
    }
    /// TODO
    pub fn defaults_for_workspace(workspaces: &WorkspaceGraph) -> Self {
        Self {
            common: CommonHostConfig::defaults_for_workspace(workspaces),
            github: None,
            axodotdev: None,
            force_latest: None,
            display: None,
            display_name: None,
        }
    }
    /// TODO
    pub fn apply_inheritance_for_package(
        self,
        workspaces: &WorkspaceGraph,
        pkg_idx: PackageIdx,
    ) -> AppHostConfig {
        let Self {
            common: _,
            github: _,
            axodotdev: _,
            force_latest: _,
            display,
            display_name,
        } = self;
        let package = workspaces.package(pkg_idx);
        AppHostConfig {
            display: display.unwrap_or(true),
            display_name: display_name.unwrap_or_else(|| package.name.clone()),
        }
    }

    /// TODO
    pub fn apply_inheritance_for_workspace(
        self,
        workspaces: &WorkspaceGraph,
    ) -> WorkspaceHostConfig {
        let Self {
            common,
            github,
            axodotdev,
            force_latest,
            display: _,
            display_name: _,
        } = self;
        let github = github.map(|github| {
            let mut default = GithubHostConfig::defaults_for_workspace(workspaces, &common);
            default.apply_layer(github);
            default
        });
        let axodotdev = axodotdev.map(|axodotdev| {
            let mut default = AxodotdevHostConfig::defaults_for_workspace(workspaces, &common);
            default.apply_layer(axodotdev);
            default
        });
        WorkspaceHostConfig {
            github,
            axodotdev,
            force_latest: force_latest.unwrap_or(false),
        }
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
            force_latest,
            display,
            display_name,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.github.apply_bool_layer(github);
        self.axodotdev.apply_bool_layer(axodotdev);
        self.force_latest.apply_opt(force_latest);
        self.display.apply_opt(display);
        self.display_name.apply_opt(display_name);
    }
}

/// TODO
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CommonHostLayer {}
/// TODO
#[derive(Debug, Default, Clone)]
pub struct CommonHostConfig {}
impl CommonHostConfig {
    /// TODO
    pub fn defaults_for_package(_workspaces: &WorkspaceGraph, _pkg_idx: PackageIdx) -> Self {
        Self {}
    }
    /// TODO
    pub fn defaults_for_workspace(_workspaces: &WorkspaceGraph) -> Self {
        Self {}
    }
}
impl ApplyLayer for CommonHostConfig {
    type Layer = CommonHostLayer;
    fn apply_layer(&mut self, Self::Layer {}: Self::Layer) {}
}
impl ApplyLayer for CommonHostLayer {
    type Layer = CommonHostLayer;
    fn apply_layer(&mut self, Self::Layer {}: Self::Layer) {}
}
