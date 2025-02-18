//! publisher config

pub mod homebrew;
pub mod npm;
pub mod user;

use super::*;

use homebrew::*;
use npm::*;
use user::*;

/// the final publisher config
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PublisherConfig {
    /// homebrew publisher
    pub homebrew: Option<HomebrewPublisherConfig>,
    /// npm publisher
    pub npm: Option<NpmPublisherConfig>,
    /// user specified publisher
    pub user: Option<UserPublisherConfig>,
}

/// the publisher config
///
/// but with inheritance not yet folded in
#[derive(Debug, Clone)]
pub struct PublisherConfigInheritable {
    /// common fields that each publisher inherits
    pub common: CommonPublisherConfig,
    /// homebrew publisher
    pub homebrew: Option<HomebrewPublisherLayer>,
    /// npm publisher
    pub npm: Option<NpmPublisherLayer>,
    /// user specified publisher
    pub user: Option<UserPublisherLayer>,
}

/// "raw" publisher config from presum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublisherLayer {
    /// common fields that each publisher inherits
    #[serde(flatten)]
    pub common: CommonPublisherLayer,
    /// homebrew publisher
    pub homebrew: Option<BoolOr<HomebrewPublisherLayer>>,
    /// npm publisher
    pub npm: Option<BoolOr<NpmPublisherLayer>>,
    /// user-specified publisher
    pub user: Option<BoolOr<UserPublisherLayer>>,
}
impl PublisherConfigInheritable {
    /// get the defaults for a given package
    pub fn defaults_for_package(workspaces: &WorkspaceGraph, pkg_idx: PackageIdx) -> Self {
        Self {
            common: CommonPublisherConfig::defaults_for_package(workspaces, pkg_idx),
            homebrew: None,
            npm: None,
            user: None,
        }
    }
    /// fold the inherited fields in to get the final publisher config
    pub fn apply_inheritance_for_package(
        self,
        workspaces: &WorkspaceGraph,
        pkg_idx: PackageIdx,
    ) -> PublisherConfig {
        let Self {
            common,
            homebrew,
            npm,
            user,
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
        let user = user.map(|user| {
            let mut default =
                UserPublisherConfig::defaults_for_package(workspaces, pkg_idx, &common);
            default.apply_layer(user);
            default
        });
        PublisherConfig {
            homebrew,
            npm,
            user,
        }
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
            user,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.homebrew.apply_bool_layer(homebrew);
        self.npm.apply_bool_layer(npm);
        self.user.apply_bool_layer(user);
    }
}

/// fields that each publisher inherits (raw)
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CommonPublisherLayer {
    /// Whether to publish prereleases (defaults to false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prereleases: Option<bool>,
}
/// fields that each publisher inherits (final)
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CommonPublisherConfig {
    /// Whether to publish prereleases (defaults to false)
    pub prereleases: bool,
}
impl CommonPublisherConfig {
    /// get the defaults for a given package
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
