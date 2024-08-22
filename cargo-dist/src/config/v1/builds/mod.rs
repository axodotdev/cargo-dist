//! build config

pub mod cargo;
pub mod generic;

use super::*;
use cargo::*;
use generic::*;

/// workspace build config
#[derive(Debug, Clone)]
pub struct WorkspaceBuildConfig {
    /// cargo builds
    pub cargo: WorkspaceCargoBuildConfig,
    /// whether to sign windows binaries with ssl.com
    pub ssldotcom_windows_sign: Option<ProductionMode>,
}

/// app-scoped build config
#[derive(Debug, Clone)]
pub struct AppBuildConfig {
    /// cargo builds
    pub cargo: AppCargoBuildConfig,
    /// generic builds
    pub generic: GenericBuildConfig,
    /// A set of packages to install before building
    pub system_dependencies: SystemDependencies,
}

/// build config (inheritance not yet folded)
#[derive(Debug, Clone)]
pub struct BuildConfigInheritable {
    /// inheritable fields
    pub common: CommonBuildConfig,
    /// whether to sign windows binaries with ssl.com
    pub ssldotcom_windows_sign: Option<ProductionMode>,
    /// cargo builds
    pub cargo: Option<CargoBuildLayer>,
    /// generic builds
    pub generic: Option<GenericBuildLayer>,
    /// A set of packages to install before building
    pub system_dependencies: SystemDependencies,
}

/// build config (raw from file)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BuildLayer {
    /// inheritable fields
    #[serde(flatten)]
    pub common: CommonBuildLayer,

    /// Whether we should sign windows binaries with ssl.com
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssldotcom_windows_sign: Option<ProductionMode>,

    /// cargo builds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cargo: Option<BoolOr<CargoBuildLayer>>,
    /// generic builds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generic: Option<BoolOr<GenericBuildLayer>>,
    /// A set of packages to install before building
    #[serde(rename = "dependencies")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_dependencies: Option<SystemDependencies>,
}
impl BuildConfigInheritable {
    /// get defaults for a package
    pub fn defaults_for_package(workspaces: &WorkspaceGraph, pkg_idx: PackageIdx) -> Self {
        Self {
            common: CommonBuildConfig::defaults_for_package(workspaces, pkg_idx),
            cargo: None,
            generic: None,
            system_dependencies: Default::default(),
            ssldotcom_windows_sign: None,
        }
    }
    /// get defaults for a workspace
    pub fn defaults_for_workspace(workspaces: &WorkspaceGraph) -> Self {
        Self {
            common: CommonBuildConfig::defaults_for_workspace(workspaces),
            cargo: None,
            generic: None,
            system_dependencies: Default::default(),
            ssldotcom_windows_sign: None,
        }
    }
    /// apply inheritance to get final workspace config
    pub fn apply_inheritance_for_workspace(
        self,
        workspaces: &WorkspaceGraph,
    ) -> WorkspaceBuildConfig {
        let Self {
            common,
            cargo,
            ssldotcom_windows_sign,
            // local-only
            generic: _,
            system_dependencies: _,
        } = self;
        let mut cargo_out = WorkspaceCargoBuildConfig::defaults_for_workspace(workspaces, &common);
        if let Some(cargo) = cargo {
            cargo_out.apply_layer(cargo);
        }
        WorkspaceBuildConfig {
            cargo: cargo_out,
            ssldotcom_windows_sign,
        }
    }
    /// apply inheritance to get final package config
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
            // local-only
            ssldotcom_windows_sign: _,
        } = self;
        let mut cargo_out = AppCargoBuildConfig::defaults_for_package(workspaces, pkg_idx, &common);
        if let Some(cargo) = cargo {
            cargo_out.apply_layer(cargo);
        }
        let mut generic_out =
            GenericBuildConfig::defaults_for_package(workspaces, pkg_idx, &common);
        if let Some(generic) = generic {
            generic_out.apply_layer(generic);
        }

        AppBuildConfig {
            cargo: cargo_out,
            generic: generic_out,
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
            ssldotcom_windows_sign,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.cargo.apply_bool_layer(cargo);
        self.generic.apply_bool_layer(generic);
        self.system_dependencies.apply_val(system_dependencies);
        self.ssldotcom_windows_sign
            .apply_opt(ssldotcom_windows_sign);
    }
}

/// inheritable build fields (final)
#[derive(Debug, Clone)]
pub struct CommonBuildConfig {}
/// inheritable build fields (raw from file)
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CommonBuildLayer {}

impl CommonBuildConfig {
    /// defaults for a given package
    pub fn defaults_for_package(_workspaces: &WorkspaceGraph, _pkg_idx: PackageIdx) -> Self {
        Self {}
    }
    /// defaults for a given workspace
    pub fn defaults_for_workspace(_workspaces: &WorkspaceGraph) -> Self {
        Self {}
    }
}
impl ApplyLayer for CommonBuildConfig {
    type Layer = CommonBuildLayer;
    fn apply_layer(&mut self, Self::Layer {}: Self::Layer) {}
}
impl ApplyLayer for CommonBuildLayer {
    type Layer = CommonBuildLayer;
    fn apply_layer(&mut self, Self::Layer {}: Self::Layer) {}
}
