//! pkg installer config

use super::*;

/// Options for pkg installer
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PkgInstallerLayer {
    /// Common options
    #[serde(flatten)]
    pub common: CommonInstallerLayer,
    /// A unique identifier, in tld.domain.package format
    pub identifier: Option<String>,
    /// The location to which the software should be installed.
    /// If not specified, /usr/local will be used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_location: Option<String>,
}
/// Options for pkg installer
#[derive(Debug, Default, Clone)]
pub struct PkgInstallerConfig {
    /// Common options
    pub common: CommonInstallerConfig,
    /// A unique identifier, in tld.domain.package format
    pub identifier: String,
    /// The location to which the software should be installed.
    /// If not specified, /usr/local will be used.
    pub install_location: String,
}

impl PkgInstallerConfig {
    /// Get defaults for the given package
    pub fn defaults_for_package(
        _workspaces: &WorkspaceGraph,
        _pkg_idx: PackageIdx,
        common: &CommonInstallerConfig,
    ) -> Self {
        Self {
            common: common.clone(),
            // TODO: you *need* to provide this to make a pkg
            // installer, so it *should* be non-optional, but the
            // whole "defaults first" thing makes this messed up...
            identifier: "TODO".to_owned(),
            install_location: "/usr/local".to_owned(),
        }
    }
}

impl ApplyLayer for PkgInstallerConfig {
    type Layer = PkgInstallerLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            common,
            identifier,
            install_location,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.identifier.apply_val(identifier);
        self.install_location.apply_val(install_location);
    }
}
impl ApplyLayer for PkgInstallerLayer {
    type Layer = PkgInstallerLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            common,
            identifier,
            install_location,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.identifier.apply_opt(identifier);
        self.install_location.apply_opt(install_location);
    }
}

impl std::ops::Deref for PkgInstallerConfig {
    type Target = CommonInstallerConfig;
    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
