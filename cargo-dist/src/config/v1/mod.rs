//! The cargo-dist 1.0 config format (as opposed to the old v0 format)
//!
//! This is the config subsystem!
//!
//! # Concepts
//!
//! It's responsible for loading, merging, and auto-detecting all the various config
//! sources. There are two closely related families of types:
//!
//! - `...Config` types are the "complete" values that will be passed around to the rest
//!   of the program. All of these types get shoved into the top-level [`Config`][] type.
//!
//! - `...Layer` types are "partial" values that are loaded and parsed before being merged
//!   into the final [`Config`][]. Notably the oranda.json is loaded as [`OrandaLayer`][] and
//!   Cargo.toml/package.json gets loaded as [`AxoprojectLayer`][].
//!
//! Nested types like [`InstallerConifg`][] usually have a paired layer ([`InstallerLayer`][]),
//! with an almost identical definition. The differences usually lie in the Layer having far more
//! Options, because you don't need to specify it in your oranda.json but we want the rest of our
//! code to have the final result fully resolved.
//!
//! The Big Idea is that:
//!
//! - a `...Config` type implements [`Default`][] manually to specify default values
//! - a `...Config` type implements [`ApplyLayer`][] to specify how its `...Layer` gets combined
//!
//! Conveniences like [`ApplyValExt::apply_val`][] and [`ApplyOptExt::apply_opt`][]
//! exist to help merge simple values like `bool <- Option<bool>` where overwriting the entire
//! value is acceptable.
//!
//! [`ApplyBoolLayerExt::apply_bool_layer`][] exists to apply [`BoolOr`][] wrappers
//! which lets config say things like `homebrew = false` when [`HomebrewInstallerConfig`][]
//! is actually an entire struct.
//!
//!
//! # Top-Level Layers
//!
//! TODO: rewrite for cargo-dist
//!
//! These are the current top-level """layers""" that get constructed and merged into
//! the top-level [`Config`][]. They are merged more free-form, but try to quickly shell
//! out to [`ApplyLayer`][] for consistency/reliability.
//!
//! The top-level layers are applied in the following order, with the later ones winning:
//!
//! - **The Default Layer** comes from [`Config::default`][] and the recursive [`Default`][]
//!   impls on the other `...Config` structs.
//!
//! - **[`AxoprojectLayer`][]** comes from a project manifest file. We currently
//!   support `Cargo.toml` and `package.json`, but could support any manifest
//!   that provides information like `name`, `description`, `repository`...
//!
//! - **[`OrandaLayer`][]**, AKA "the custom layer", comes from an `oranda.json` file.
//!   It's basically a complete replica of [`Config`][] but with way more Options.
//!
//! - **The Autodetect Layer** is just a convention where configs have an opportunity
//!   to try to find missing values, erroring out if they fail while the user
//!   was clearly trying to enable the feature.
//!
//! Note that several of these config merges are seemingly pedantic about preserving/merging
//! old values when only one source sets it in practice. This is to make the code more reliable,
//! consistent, and robust in the face of future config/layer additions without you having to
//! know exactly all the ways a value can be set.


// We very intentionally manually implement Default a lot in this submodule
// to keep things very explicit and clear
#![allow(clippy::derivable_impls)]

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
pub fn workspace_config(
    workspaces: &WorkspaceGraph,
    mut global_config: TomlLayer,
) -> WorkspaceConfig {
    // Rewrite config-file-relative paths
    global_config.make_relative_to(&workspaces.root_workspace().workspace_dir);

    let mut config = WorkspaceConfigInheritable::defaults_for_workspace(workspaces);
    config.apply_layer(global_config);
    config.apply_inheritance_for_workspace(workspaces)
}

/// Compute the package-level config
pub fn app_config(
    workspaces: &WorkspaceGraph,
    pkg_idx: PackageIdx,
    mut global_config: TomlLayer,
    mut local_config: TomlLayer,
) -> AppConfig {
    // Rewrite config-file-relative paths
    let package = workspaces.package(pkg_idx);
    global_config.make_relative_to(&workspaces.root_workspace().workspace_dir);
    local_config.make_relative_to(&package.package_root);

    let mut config = AppConfigInheritable::defaults_for_package(workspaces, pkg_idx);
    config.apply_layer(global_config);
    config.apply_layer(local_config);
    config.apply_inheritance_for_package(workspaces, pkg_idx)
}

/// config that is global to the entire workspace
#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    /// The intended version of cargo-dist to build with. (normal Cargo SemVer syntax)
    pub dist_version: Option<Version>,
    /// Generate targets whose cargo-dist should avoid checking for up-to-dateness.
    pub allow_dirty: Vec<GenerateMode>,
    /// ci config
    pub ci: CiConfig,
    /// artifact config
    pub artifacts: WorkspaceArtifactConfig,
    /// host config
    pub hosts: WorkspaceHostConfig,
    /// build config
    pub builds: WorkspaceBuildConfig,
    /// installer config
    pub installers: WorkspaceInstallerConfig,
}
/// config that is global to the entire workspace
///
/// but inheritance relationships haven't been folded in yet.
#[derive(Debug, Clone)]
pub struct WorkspaceConfigInheritable {
    /// The intended version of cargo-dist to build with. (normal Cargo SemVer syntax)
    pub dist_version: Option<Version>,
    /// Generate targets whose cargo-dist should avoid checking for up-to-dateness.
    pub allow_dirty: Vec<GenerateMode>,
    /// artifact config
    pub artifacts: WorkspaceArtifactConfig,
    /// ci config
    pub ci: CiConfigInheritable,
    /// host config
    pub hosts: HostConfigInheritable,
    /// build config
    pub builds: BuildConfigInheritable,
    /// installer config
    pub installers: InstallerConfigInheritable,
}
impl WorkspaceConfigInheritable {
    /// Get the defaults for workspace-level config
    pub fn defaults_for_workspace(workspaces: &WorkspaceGraph) -> Self {
        Self {
            artifacts: WorkspaceArtifactConfig::defaults_for_workspace(workspaces),
            ci: CiConfigInheritable::defaults_for_workspace(workspaces),
            hosts: HostConfigInheritable::defaults_for_workspace(workspaces),
            builds: BuildConfigInheritable::defaults_for_workspace(workspaces),
            installers: InstallerConfigInheritable::defaults_for_workspace(workspaces),
            dist_version: None,
            allow_dirty: vec![],
        }
    }
    /// Apply the inheritance to ge tthe final WorkspaceConfig
    pub fn apply_inheritance_for_workspace(self, workspaces: &WorkspaceGraph) -> WorkspaceConfig {
        let Self {
            artifacts,
            ci,
            hosts,
            builds,
            installers,
            dist_version,
            allow_dirty,
        } = self;
        WorkspaceConfig {
            artifacts,
            ci: ci.apply_inheritance_for_workspace(workspaces),
            hosts: hosts.apply_inheritance_for_workspace(workspaces),
            builds: builds.apply_inheritance_for_workspace(workspaces),
            installers: installers.apply_inheritance_for_workspace(workspaces),
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
            artifacts,
            builds,
            hosts,
            installers,
            ci,
            allow_dirty,
            dist_version,
            // app-scope only
            dist: _,
            targets: _,
            publishers: _,
        }: Self::Layer,
    ) {
        self.artifacts.apply_val_layer(artifacts);
        self.builds.apply_val_layer(builds);
        self.hosts.apply_val_layer(hosts);
        self.installers.apply_val_layer(installers);
        self.ci.apply_val_layer(ci);
        self.dist_version.apply_opt(dist_version);
        self.allow_dirty.apply_val(allow_dirty);
    }
}

/// Config scoped to a particular App
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// artifact config
    pub artifacts: AppArtifactConfig,
    /// build config
    pub builds: AppBuildConfig,
    /// host config
    pub hosts: AppHostConfig,
    /// installer config
    pub installers: AppInstallerConfig,
    /// publisher config
    pub publishers: PublisherConfig,
    /// Whether the package should be distributed/built by cargo-dist
    pub dist: Option<bool>,
    /// The full set of target triples to build for.
    pub targets: Vec<String>,
}
/// Config scoped to a particular App
///
/// but inheritance relationships haven't been folded in yet.
#[derive(Debug, Clone)]
pub struct AppConfigInheritable {
    /// artifact config
    pub artifacts: AppArtifactConfig,
    /// build config
    pub builds: BuildConfigInheritable,
    /// host config
    pub hosts: HostConfigInheritable,
    /// installer config
    pub installers: InstallerConfigInheritable,
    /// publisher config
    pub publishers: PublisherConfigInheritable,
    /// Whether the package should be distributed/built by cargo-dist
    pub dist: Option<bool>,
    /// The full set of target triples to build for.
    pub targets: Vec<String>,
}
impl AppConfigInheritable {
    /// Get the defaults for the given package
    pub fn defaults_for_package(workspaces: &WorkspaceGraph, pkg_idx: PackageIdx) -> Self {
        Self {
            artifacts: AppArtifactConfig::defaults_for_package(workspaces, pkg_idx),
            builds: BuildConfigInheritable::defaults_for_package(workspaces, pkg_idx),
            hosts: HostConfigInheritable::defaults_for_package(workspaces, pkg_idx),
            installers: InstallerConfigInheritable::defaults_for_package(workspaces, pkg_idx),
            publishers: PublisherConfigInheritable::defaults_for_package(workspaces, pkg_idx),
            dist: None,
            targets: vec![],
        }
    }
    /// Fold in inheritance relationships to get the final package config
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

/// The "raw" input from a toml file containing config
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

    /// artifact config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<ArtifactLayer>,
    /// build config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub builds: Option<BuildLayer>,
    /// ci config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ci: Option<CiLayer>,
    /// host config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hosts: Option<HostLayer>,
    /// installer config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installers: Option<InstallerLayer>,
    /// publisher config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publishers: Option<PublisherLayer>,
}

impl TomlLayer {
    /// Take any configs that contain paths that are *relative to the file they came from*
    /// and make them relative to the given basepath.
    ///
    /// This is important to do eagerly, because once we start merging configs
    /// we'll forget what file they came from!
    fn make_relative_to(&mut self, base_path: &Utf8Path) {
        // It's kind of unfortunate that we don't exhaustively match this to
        // force you to update it BUT almost no config is ever applicable for
        // this so even when we used to, everyone just skimmed over this so
        // whatever just Get Good and remember this transform is necessary
        // if you every add another config-file-relative path to the config
        if let Some(artifacts) = &mut self.artifacts {
            if let Some(archives) = &mut artifacts.archives {
                if let Some(include) = &mut archives.include {
                    for path in include {
                        make_path_relative_to(path, base_path);
                    }
                }
            }
            if let Some(extras) = &mut artifacts.extra {
                for extra in extras {
                    make_path_relative_to(&mut extra.working_dir, base_path);
                }
            }
        }
        if let Some(hosts) = &mut self.hosts {
            if let Some(BoolOr::Val(github)) = &mut hosts.github {
                if let Some(path) = &mut github.submodule_path {
                    make_path_relative_to(path, base_path);
                }
            }
        }
    }
}

fn make_path_relative_to(path: &mut Utf8PathBuf, base_path: &Utf8Path) {
    // TODO: should absolute paths be a hard error? Or should we force them relative?
    if !path.is_absolute() {
        *path = base_path.join(&path);
    }
}
