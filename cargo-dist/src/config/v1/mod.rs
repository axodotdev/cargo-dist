//! TODO
pub mod layer;

pub mod artifacts;
pub mod builds;
pub mod ci;
pub mod hosts;
pub mod installers;
pub mod publishers;

use axoproject::{PackageIdx, WorkspaceGraph};
use semver::Version;

use super::*;
use layer::*;

use artifacts::*;
use builds::*;
use ci::*;
use hosts::*;
use installers::*;
use publishers::*;

/// Compute the workspace-level config
pub fn workspace_config(workspaces: &WorkspaceGraph, global_config: TomlLayer) -> WorkspaceConfig {
    let mut config = WorkspaceConfigInheritable::defaults_for_workspace(workspaces);
    config.apply_layer(global_config);
    config.apply_inheritance_for_workspace(workspaces)
}

/// Compute the package-level config
pub fn app_config(
    workspaces: &WorkspaceGraph,
    pkg_idx: PackageIdx,
    global_config: TomlLayer,
    local_config: TomlLayer,
) -> AppConfig {
    let mut config = AppConfigInheritable::defaults_for_package(workspaces, pkg_idx);
    config.apply_layer(global_config);
    config.apply_layer(local_config);
    config.apply_inheritance_for_package(workspaces, pkg_idx)
}

/// TODO
#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    /// The intended version of cargo-dist to build with. (normal Cargo SemVer syntax)
    pub dist_version: Option<Version>,
    /// Generate targets whose cargo-dist should avoid checking for up-to-dateness.
    pub allow_dirty: Vec<GenerateMode>,
    /// TODO
    pub ci: CiConfig,
}
/// TODO
#[derive(Debug, Clone)]
pub struct WorkspaceConfigInheritable {
    /// TODO
    pub ci: CiConfigInheritable,
    /// The intended version of cargo-dist to build with. (normal Cargo SemVer syntax)
    pub dist_version: Option<Version>,
    /// Generate targets whose cargo-dist should avoid checking for up-to-dateness.
    pub allow_dirty: Vec<GenerateMode>,
}
impl WorkspaceConfigInheritable {
    /// TODO
    pub fn defaults_for_workspace(workspaces: &WorkspaceGraph) -> Self {
        Self {
            ci: CiConfigInheritable::defaults_for_workspace(workspaces),
            dist_version: None,
            allow_dirty: vec![],
        }
    }
    /// TODO
    pub fn apply_inheritance_for_workspace(self, workspaces: &WorkspaceGraph) -> WorkspaceConfig {
        let Self {
            ci,
            dist_version,
            allow_dirty,
        } = self;
        WorkspaceConfig {
            ci: ci.apply_inheritance_for_workspace(workspaces),
            dist_version,
            allow_dirty,
        }
    }
}
impl ApplyLayer for WorkspaceConfigInheritable {
    type Layer = TomlLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            ci,
            allow_dirty,
            dist_version,
            // app-scope only
            artifacts: _,
            builds: _,
            hosts: _,
            installers: _,
            publishers: _,
            dist: _,
            targets: _,
        }: Self::Layer,
    ) {
        self.ci.apply_val_layer(ci);
        self.dist_version.apply_opt(dist_version);
        self.allow_dirty.apply_val(allow_dirty);
    }
}

/// TODO
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// TODO
    pub artifacts: ArtifactConfig,
    /// TODO
    pub builds: BuildConfig,
    /// TODO
    pub hosts: HostConfig,
    /// TODO
    pub installers: InstallerConfig,
    /// TODO
    pub publishers: PublisherConfig,
    /// Whether the package should be distributed/built by cargo-dist
    pub dist: Option<bool>,
    /// The full set of target triples to build for.
    pub targets: Vec<String>,
}
/// TODO
#[derive(Debug, Clone)]
pub struct AppConfigInheritable {
    /// TODO
    pub artifacts: ArtifactConfig,
    /// TODO
    pub builds: BuildConfigInheritable,
    /// TODO
    pub hosts: HostConfigInheritable,
    /// TODO
    pub installers: InstallerConfigInheritable,
    /// TODO
    pub publishers: PublisherConfigInheritable,
    /// Whether the package should be distributed/built by cargo-dist
    pub dist: Option<bool>,
    /// The full set of target triples to build for.
    pub targets: Vec<String>,
}
impl AppConfigInheritable {
    /// TODO
    pub fn defaults_for_package(workspaces: &WorkspaceGraph, pkg_idx: PackageIdx) -> Self {
        Self {
            artifacts: ArtifactConfig::defaults_for_package(workspaces, pkg_idx),
            builds: BuildConfigInheritable::defaults_for_package(workspaces, pkg_idx),
            hosts: HostConfigInheritable::defaults_for_package(workspaces, pkg_idx),
            installers: InstallerConfigInheritable::defaults_for_package(workspaces, pkg_idx),
            publishers: PublisherConfigInheritable::defaults_for_package(workspaces, pkg_idx),
            dist: None,
            targets: vec![],
        }
    }
    /// TODO
    pub fn apply_inheritance_for_package(
        self,
        workspaces: &WorkspaceGraph,
        pkg_idx: PackageIdx,
    ) -> AppConfig {
        let Self {
            artifacts,
            builds,
            hosts,
            installers,
            publishers,
            dist: do_dist,
            targets,
        } = self;
        AppConfig {
            artifacts,
            builds: builds.apply_inheritance_for_package(workspaces, pkg_idx),
            hosts: hosts.apply_inheritance_for_package(workspaces, pkg_idx),
            installers: installers.apply_inheritance_for_package(workspaces, pkg_idx),
            publishers: publishers.apply_inheritance_for_package(workspaces, pkg_idx),
            dist: do_dist,
            targets,
        }
    }
}
impl ApplyLayer for AppConfigInheritable {
    type Layer = TomlLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            artifacts,
            builds,
            hosts,
            installers,
            publishers,
            dist,
            targets,
            // workspace-scope only
            ci: _,
            allow_dirty: _,
            dist_version: _,
        }: Self::Layer,
    ) {
        self.artifacts.apply_val_layer(artifacts);
        self.builds.apply_val_layer(builds);
        self.hosts.apply_val_layer(hosts);
        self.installers.apply_val_layer(installers);
        self.publishers.apply_val_layer(publishers);
        self.dist.apply_opt(dist);
        self.targets.apply_val(targets);
    }
}

/// TODO
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TomlLayer {
    /// The intended version of cargo-dist to build with. (normal Cargo SemVer syntax)
    ///
    /// When generating full tasks graphs (such as CI scripts) we will pick this version.
    ///
    /// FIXME: Should we produce a warning if running locally with a different version? In theory
    /// it shouldn't be a problem and newer versions should just be Better... probably you
    /// Really want to have the exact version when running generate to avoid generating
    /// things other cargo-dist versions can't handle!
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dist_version: Option<Version>,

    /// Whether the package should be distributed/built by cargo-dist
    ///
    /// This mainly exists to be set to `false` to make cargo-dist ignore the existence of this
    /// package. Note that we may still build the package as a side-effect of building the
    /// workspace -- we just won't bundle it up and report it.
    ///
    /// FIXME: maybe you should also be allowed to make this a list of binary names..?
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dist: Option<bool>,

    /// Generate targets whose cargo-dist should avoid checking for up-to-dateness.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_dirty: Option<Vec<GenerateMode>>,

    /// The full set of target triples to build for.
    ///
    /// When generating full task graphs (such as CI scripts) we will to try to generate these.
    ///
    /// The inputs should be valid rustc target triples (see `rustc --print target-list`) such
    /// as `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, or `x86_64-unknown-linux-gnu`.
    ///
    /// FIXME: We should also accept one magic target: `universal2-apple-darwin`. This will induce
    /// us to build `x86_64-apple-darwin` and `aarch64-apple-darwin` (arm64) and then combine
    /// them into a "universal" binary that can run on either arch (using apple's `lipo` tool).
    ///
    /// FIXME: Allow higher level requests like "[macos, windows, linux] x [x86_64, aarch64]"?
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<String>>,

    /// TODO
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<ArtifactLayer>,
    /// TODO
    #[serde(skip_serializing_if = "Option::is_none")]
    pub builds: Option<BuildLayer>,
    /// TODO
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ci: Option<CiLayer>,
    /// TODO
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hosts: Option<HostLayer>,
    /// TODO
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installers: Option<InstallerLayer>,
    /// TODO
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publishers: Option<PublisherLayer>,
}
