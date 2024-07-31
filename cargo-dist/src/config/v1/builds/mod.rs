//! TODO

pub mod cargo;
pub mod generic;

use super::*;
use cargo::*;
use generic::*;

/// TODO
#[derive(Debug, Default, Clone)]
pub struct WorkspaceBuildConfig {
    /// TODO
    pub cargo: Option<CargoBuildConfig>,
    /// TODO
    pub generic: Option<GenericBuildConfig>,
    /// A set of packages to install before building
    pub system_dependencies: SystemDependencies,
}

/// TODO
#[derive(Debug, Default, Clone)]
pub struct AppBuildConfig {
    /// TODO
    pub cargo: Option<CargoBuildConfig>,
    /// TODO
    pub generic: Option<GenericBuildConfig>,
    /// A set of packages to install before building
    pub system_dependencies: SystemDependencies,
}

/// TODO
#[derive(Debug, Clone)]
pub struct BuildConfigInheritable {
    /// TODO
    pub common: CommonBuildConfig,
    /// TODO
    pub cargo: Option<CargoBuildLayer>,
    /// TODO
    pub generic: Option<GenericBuildLayer>,
    /// A set of packages to install before building
    pub system_dependencies: SystemDependencies,
}

/// TODO
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BuildLayer {
    /// TODO
    #[serde(flatten)]
    pub common: CommonBuildLayer,

    /// \[unstable\] Whether we should sign windows binaries with ssl.com
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssldotcom_windows_sign: Option<ProductionMode>,

    /// Whether msvc targets should statically link the crt
    ///
    /// Defaults to true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msvc_crt_static: Option<bool>,

    /// TODO
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cargo: Option<BoolOr<CargoBuildLayer>>,
    /// TODO
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generic: Option<BoolOr<GenericBuildLayer>>,
    /// A set of packages to install before building
    #[serde(rename = "dependencies")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_dependencies: Option<SystemDependencies>,
}
impl BuildConfigInheritable {
    /// TODO
    pub fn defaults_for_package(workspaces: &WorkspaceGraph, pkg_idx: PackageIdx) -> Self {
        Self {
            common: CommonBuildConfig::defaults_for_package(workspaces, pkg_idx),
            cargo: None,
            generic: None,
            system_dependencies: Default::default(),
        }
    }
    /// TODO
    pub fn apply_inheritance_for_workspace(
        self,
        workspaces: &WorkspaceGraph,
        pkg_idx: PackageIdx,
    ) -> WorkspaceBuildConfig {
        let Self {
            common,
            cargo,
            generic,
            system_dependencies,
        } = self;
        let cargo = cargo.map(|cargo| {
            let mut default = CargoBuildConfig::defaults_for_package(workspaces, pkg_idx, &common);
            default.apply_layer(cargo);
            default
        });
        let generic = generic.map(|generic| {
            let mut default =
                GenericBuildConfig::defaults_for_package(workspaces, pkg_idx, &common);
            default.apply_layer(generic);
            default
        });
        WorkspaceBuildConfig {
            cargo,
            generic,
            system_dependencies,
        }
    }
    /// TODO
    pub fn apply_inheritance_for_package(
        self,
        workspaces: &WorkspaceGraph,
        pkg_idx: PackageIdx,
    ) -> AppBuildConfig {
        let Self {
            common,
            cargo,
            generic,
            system_dependencies,
        } = self;
        let cargo = cargo.map(|cargo| {
            let mut default = CargoBuildConfig::defaults_for_package(workspaces, pkg_idx, &common);
            default.apply_layer(cargo);
            default
        });
        let generic = generic.map(|generic| {
            let mut default =
                GenericBuildConfig::defaults_for_package(workspaces, pkg_idx, &common);
            default.apply_layer(generic);
            default
        });
        AppBuildConfig {
            cargo,
            generic,
            system_dependencies,
        }
    }

}
impl ApplyLayer for BuildConfigInheritable {
    type Layer = BuildLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            common,
            cargo,
            generic,
            system_dependencies,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.cargo.apply_bool_layer(cargo);
        self.generic.apply_bool_layer(generic);
        self.system_dependencies.apply_val(system_dependencies);
    }
}

/// TODO
#[derive(Debug, Clone)]
pub struct CommonBuildConfig {

}
/// TODO
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CommonBuildLayer {

}

impl CommonBuildConfig {
    /// TODO
    pub fn defaults_for_package(_workspaces: &WorkspaceGraph, _pkg_idx: PackageIdx) -> Self {
        Self {

        }
    }
}
impl ApplyLayer for CommonBuildConfig {
    type Layer = CommonBuildLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
        }: Self::Layer,
    ) {
    }
}
impl ApplyLayer for CommonBuildLayer {
    type Layer = CommonBuildLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            ssldotcom_windows_sign,
            msvc_crt_static,
        }: Self::Layer,
    ) {
    }
}
