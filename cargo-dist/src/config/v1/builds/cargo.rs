//! cargo build config

use super::*;

/// cargo build config for the whole workspace
#[derive(Debug, Clone)]
pub struct WorkspaceCargoBuildConfig {
    /// Whether msvc targets should statically link the crt
    pub msvc_crt_static: bool,

    /// (deprecated) The intended version of Rust/Cargo to build with (rustup toolchain syntax)
    ///
    /// When generating full tasks graphs (such as CI scripts) we will pick this version.
    pub rust_toolchain_version: Option<String>,

    /// Build only the required packages, and individually
    pub precise_builds: Option<bool>,

    /// Whether to embed dependency information in the executable.
    pub cargo_auditable: bool,

    /// Whether to run cargo-cyclonedx on the workspace.
    pub cargo_cyclonedx: bool,
}

/// cargo build config for a specific app
#[derive(Debug, Clone)]
pub struct AppCargoBuildConfig {
    /// common build config
    pub common: CommonBuildConfig,

    /// A list of features to enable when building a package with dist
    pub features: Vec<String>,
    /// Whether to enable when building a package with dist
    ///
    /// (defaults to true)
    pub default_features: bool,
    /// Whether to enable all features building a package with dist
    ///
    /// (defaults to false)
    pub all_features: bool,
    /// Whether to embed dependency information in the executable.
    ///
    /// (defaults to false)
    pub cargo_auditable: bool,
    /// Whether to use cargo-cyclonedx to generate an SBOM.
    ///
    /// (defaults to false)
    pub cargo_cyclonedx: bool,
}

/// cargo build config (raw)
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CargoBuildLayer {
    /// inheritable cargo build config
    #[serde(flatten)]
    pub common: CommonBuildLayer,

    /// (deprecated) The intended version of Rust/Cargo to build with (rustup toolchain syntax)
    ///
    /// When generating full tasks graphs (such as CI scripts) we will pick this version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rust_toolchain_version: Option<String>,

    /// Whether msvc targets should statically link the crt
    ///
    /// Defaults to true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msvc_crt_static: Option<bool>,

    /// Build only the required packages, and individually (since 0.1.0) (default: false)
    ///
    /// By default when we need to build anything in your workspace, we build your entire workspace
    /// with --workspace. This setting tells dist to instead build each app individually.
    ///
    /// On balance, the Rust experts we've consulted with find building with --workspace to
    /// be a safer/better default, as it provides some of the benefits of a more manual
    /// [workspace-hack][], without the user needing to be aware that this is a thing.
    ///
    /// TL;DR: cargo prefers building one copy of each dependency in a build, so if two apps in
    /// your workspace depend on e.g. serde with different features, building with --workspace,
    /// will build serde once with the features unioned together. However if you build each
    /// package individually it will more precisely build two copies of serde with different
    /// feature sets.
    ///
    /// The downside of using --workspace is that if your workspace has lots of example/test
    /// crates, or if you release only parts of your workspace at a time, we build a lot of
    /// gunk that's not needed, and potentially bloat up your app with unnecessary features.
    ///
    /// If that downside is big enough for you, this setting is a good idea.
    ///
    /// [workspace-hack]: https://docs.rs/cargo-hakari/latest/cargo_hakari/about/index.html
    #[serde(skip_serializing_if = "Option::is_none")]
    pub precise_builds: Option<bool>,

    /// A list of features to enable when building a package with dist
    ///
    /// (defaults to none)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<Vec<String>>,
    /// Whether to enable when building a package with dist
    ///
    /// (defaults to true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_features: Option<bool>,
    /// Whether to enable all features building a package with dist
    ///
    /// (defaults to false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all_features: Option<bool>,
    /// Whether to embed dependency information in the executable.
    ///
    /// (defaults to false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cargo_auditable: Option<bool>,
    /// Whether to run cargo-cyclonedx to generate an SBOM.
    ///
    /// (defaults to false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cargo_cyclonedx: Option<bool>,
}

impl WorkspaceCargoBuildConfig {
    /// Get defaults for the given package
    pub fn defaults_for_workspace(
        _workspaces: &WorkspaceGraph,
        _common: &CommonBuildConfig,
    ) -> Self {
        Self {
            rust_toolchain_version: None,
            precise_builds: None,
            msvc_crt_static: true,
            cargo_auditable: false,
            cargo_cyclonedx: false,
        }
    }
}

impl AppCargoBuildConfig {
    /// Get defaults for the given package
    pub fn defaults_for_package(
        _workspaces: &WorkspaceGraph,
        _pkg_idx: PackageIdx,
        common: &CommonBuildConfig,
    ) -> Self {
        Self {
            common: common.clone(),
            features: vec![],
            default_features: true,
            all_features: false,
            cargo_auditable: false,
            cargo_cyclonedx: false,
        }
    }
}

impl ApplyLayer for WorkspaceCargoBuildConfig {
    type Layer = CargoBuildLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            rust_toolchain_version,
            precise_builds,
            cargo_auditable,
            cargo_cyclonedx,
            // local-only
            common: _,
            msvc_crt_static: _,
            features: _,
            default_features: _,
            all_features: _,
        }: Self::Layer,
    ) {
        self.rust_toolchain_version
            .apply_opt(rust_toolchain_version);
        self.precise_builds.apply_opt(precise_builds);
        self.cargo_auditable.apply_val(cargo_auditable);
        self.cargo_cyclonedx.apply_val(cargo_cyclonedx);
    }
}
impl ApplyLayer for AppCargoBuildConfig {
    type Layer = CargoBuildLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            common,
            features,
            default_features,
            all_features,
            cargo_auditable,
            cargo_cyclonedx,

            // global-only
            rust_toolchain_version: _,
            precise_builds: _,
            msvc_crt_static: _,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.features.apply_val(features);
        self.default_features.apply_val(default_features);
        self.all_features.apply_val(all_features);
        self.cargo_auditable.apply_val(cargo_auditable);
        self.cargo_cyclonedx.apply_val(cargo_cyclonedx);
    }
}
impl ApplyLayer for CargoBuildLayer {
    type Layer = CargoBuildLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            common,
            rust_toolchain_version,
            precise_builds,
            msvc_crt_static,
            features,
            default_features,
            all_features,
            cargo_auditable,
            cargo_cyclonedx,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.rust_toolchain_version
            .apply_opt(rust_toolchain_version);
        self.msvc_crt_static.apply_opt(msvc_crt_static);
        self.precise_builds.apply_opt(precise_builds);
        self.features.apply_opt(features);
        self.default_features.apply_opt(default_features);
        self.all_features.apply_opt(all_features);
        self.cargo_auditable.apply_opt(cargo_auditable);
        self.cargo_cyclonedx.apply_opt(cargo_cyclonedx);
    }
}

impl std::ops::Deref for AppCargoBuildConfig {
    type Target = CommonBuildConfig;
    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
