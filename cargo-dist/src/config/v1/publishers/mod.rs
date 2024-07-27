//! TODO

//! TODO

pub mod homebrew;
pub mod npm;

use super::*;

use homebrew::*;
use npm::*;

/// TODO
#[derive(Debug, Default, Clone)]
pub struct PublisherConfig {
    /// TODO
    pub homebrew: Option<HomebrewPublisherConfig>,
    /// TODO
    pub npm: Option<NpmPublisherConfig>,
}

/// TODO
#[derive(Debug, Clone)]
pub struct PublisherConfigInheritable {
    /// TODO
    pub common: CommonPublisherConfig,
    /// TODO
    pub homebrew: Option<HomebrewPublisherLayer>,
    /// TODO
    pub npm: Option<NpmPublisherLayer>,
}

/// TODO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublisherLayer {
    /// TODO
    #[serde(flatten)]
    pub common: CommonPublisherLayer,
    /// TODO
    pub homebrew: Option<BoolOr<HomebrewPublisherLayer>>,
    /// TODO
    pub npm: Option<BoolOr<NpmPublisherLayer>>,
}
impl PublisherConfigInheritable {
    /// TODO
    pub fn defaults_for_package(workspaces: &WorkspaceGraph, pkg_idx: PackageIdx) -> Self {
        Self {
            common: CommonPublisherConfig::defaults_for_package(workspaces, pkg_idx),
            homebrew: None,
            npm: None,
        }
    }
    /// TODO
    pub fn apply_inheritance_for_package(
        self,
        workspaces: &WorkspaceGraph,
        pkg_idx: PackageIdx,
    ) -> PublisherConfig {
        let Self {
            common,
            homebrew,
            npm,
        } = self;
        let homebrew = homebrew.map(|homebrew| {
            let mut default =
                HomebrewPublisherConfig::defaults_for_package(workspaces, pkg_idx, &common);
            default.apply_layer(homebrew);
            default
        });
        let npm = npm.map(|npm| {
            let mut default =
                NpmPublisherConfig::defaults_for_package(workspaces, pkg_idx, &common);
            default.apply_layer(npm);
            default
        });
        PublisherConfig { homebrew, npm }
    }
}
impl ApplyLayer for PublisherConfigInheritable {
    type Layer = PublisherLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            common,
            homebrew,
            npm,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.homebrew.apply_bool_layer(homebrew);
        self.npm.apply_bool_layer(npm);
    }
}

/// TODO
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CommonPublisherLayer {
    /// Whether to publish prereleases (defaults to false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prereleases: Option<bool>,
}
/// TODO
#[derive(Debug, Default, Clone)]
pub struct CommonPublisherConfig {
    /// Whether to publish prereleases (defaults to false)
    pub prereleases: bool,
}
impl CommonPublisherConfig {
    /// TODO
    pub fn defaults_for_package(_workspaces: &WorkspaceGraph, _pkg_idx: PackageIdx) -> Self {
        Self { prereleases: false }
    }
}
impl ApplyLayer for CommonPublisherConfig {
    type Layer = CommonPublisherLayer;
    fn apply_layer(&mut self, Self::Layer { prereleases }: Self::Layer) {
        self.prereleases.apply_val(prereleases);
    }
}
impl ApplyLayer for CommonPublisherLayer {
    type Layer = CommonPublisherLayer;
    fn apply_layer(&mut self, Self::Layer { prereleases }: Self::Layer) {
        self.prereleases.apply_opt(prereleases);
    }
}
