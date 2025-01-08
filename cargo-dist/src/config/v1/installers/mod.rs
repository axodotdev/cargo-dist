//! installer config

pub mod homebrew;
pub mod msi;
pub mod npm;
pub mod pkg;
pub mod powershell;
pub mod shell;

use super::*;

use homebrew::*;
use msi::*;
use npm::*;
use pkg::*;
use powershell::*;
use shell::*;

/// workspace installer config (final)
#[derive(Debug, Default, Clone)]
pub struct WorkspaceInstallerConfig {
    /// Whether to install an updater program alongside the software
    pub updater: bool,
    /// Whether to always use the latest version instead of a known-good version
    pub always_use_latest_updater: bool,
}
/// package installer config (final)
#[derive(Debug, Default, Clone)]
pub struct AppInstallerConfig {
    /// homebrew installer
    pub homebrew: Option<HomebrewInstallerConfig>,
    /// msi installer
    pub msi: Option<MsiInstallerConfig>,
    /// npm installer
    pub npm: Option<NpmInstallerConfig>,
    /// powershell installer
    pub powershell: Option<PowershellInstallerConfig>,
    /// shell installer
    pub shell: Option<ShellInstallerConfig>,
    /// shell installer
    pub pkg: Option<PkgInstallerConfig>,
}

/// installer config (inheritance not yet applied)
#[derive(Debug, Clone)]
pub struct InstallerConfigInheritable {
    /// inheritable fields
    pub common: CommonInstallerConfig,
    /// homebrew installer
    pub homebrew: Option<HomebrewInstallerLayer>,
    /// msi installer
    pub msi: Option<MsiInstallerLayer>,
    /// npm installer
    pub npm: Option<NpmInstallerLayer>,
    /// powershell installer
    pub powershell: Option<PowershellInstallerLayer>,
    /// shell installer
    pub shell: Option<ShellInstallerLayer>,
    /// pkg installer
    pub pkg: Option<PkgInstallerLayer>,
    /// Whether to install an updater program alongside the software
    pub updater: bool,
    /// Whether to always use the latest version instead of a fixed version
    pub always_use_latest_updater: bool,
}

/// installer config (raw from file)
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct InstallerLayer {
    /// inheritable fields
    #[serde(flatten)]
    pub common: CommonInstallerLayer,
    /// homebrew installer
    pub homebrew: Option<BoolOr<HomebrewInstallerLayer>>,
    /// msi installer
    pub msi: Option<BoolOr<MsiInstallerLayer>>,
    /// npm installer
    pub npm: Option<BoolOr<NpmInstallerLayer>>,
    /// powershell installer
    pub powershell: Option<BoolOr<PowershellInstallerLayer>>,
    /// shell installer
    pub shell: Option<BoolOr<ShellInstallerLayer>>,
    /// pkg installer
    pub pkg: Option<BoolOr<PkgInstallerLayer>>,
    /// Whether to install an updater program alongside the software
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updater: Option<bool>,
    /// Whether to always use the latest updater version instead of a fixed version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub always_use_latest_updater: Option<bool>,
}
impl InstallerConfigInheritable {
    /// defaults for a workspace
    pub fn defaults_for_workspace(_workspaces: &WorkspaceGraph) -> Self {
        Self::defaults()
    }
    /// defaults for a package
    pub fn defaults_for_package(_workspaces: &WorkspaceGraph, _pkg_idx: PackageIdx) -> Self {
        Self::defaults()
    }
    /// defaults
    pub fn defaults() -> Self {
        Self {
            common: CommonInstallerConfig::defaults(),
            homebrew: None,
            msi: None,
            npm: None,
            powershell: None,
            shell: None,
            pkg: None,
            updater: false,
            always_use_latest_updater: false,
        }
    }
    /// apply inheritance to and get final workspace config
    pub fn apply_inheritance_for_workspace(
        self,
        _workspaces: &WorkspaceGraph,
    ) -> WorkspaceInstallerConfig {
        let Self {
            // global
            updater,
            always_use_latest_updater,
            // local-only
            common: _,
            homebrew: _,
            msi: _,
            npm: _,
            powershell: _,
            shell: _,
            pkg: _,
        } = self;

        WorkspaceInstallerConfig {
            updater,
            always_use_latest_updater,
        }
    }
    /// apply inheritance to get final package config
    pub fn apply_inheritance_for_package(
        self,
        workspaces: &WorkspaceGraph,
        pkg_idx: PackageIdx,
    ) -> AppInstallerConfig {
        let Self {
            common,
            homebrew,
            msi,
            npm,
            powershell,
            shell,
            pkg,
            // global-only
            updater: _,
            always_use_latest_updater: _,
        } = self;
        let homebrew = homebrew.map(|homebrew| {
            let mut default =
                HomebrewInstallerConfig::defaults_for_package(workspaces, pkg_idx, &common);
            default.apply_layer(homebrew);
            default
        });
        let msi = msi.map(|msi| {
            let mut default =
                MsiInstallerConfig::defaults_for_package(workspaces, pkg_idx, &common);
            default.apply_layer(msi);
            default
        });
        let npm = npm.map(|npm| {
            let mut default =
                NpmInstallerConfig::defaults_for_package(workspaces, pkg_idx, &common);
            default.apply_layer(npm);
            default
        });
        let powershell = powershell.map(|powershell| {
            let mut default =
                PowershellInstallerConfig::defaults_for_package(workspaces, pkg_idx, &common);
            default.apply_layer(powershell);
            default
        });
        let shell = shell.map(|shell| {
            let mut default =
                ShellInstallerConfig::defaults_for_package(workspaces, pkg_idx, &common);
            default.apply_layer(shell);
            default
        });
        let pkg = pkg.map(|pkg| {
            let mut default =
                PkgInstallerConfig::defaults_for_package(workspaces, pkg_idx, &common);
            default.apply_layer(pkg);
            default
        });
        AppInstallerConfig {
            homebrew,
            msi,
            npm,
            powershell,
            shell,
            pkg,
        }
    }
}
impl ApplyLayer for InstallerConfigInheritable {
    type Layer = InstallerLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            common,
            homebrew,
            msi,
            npm,
            powershell,
            shell,
            pkg,
            updater,
            always_use_latest_updater,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.homebrew.apply_bool_layer(homebrew);
        self.msi.apply_bool_layer(msi);
        self.npm.apply_bool_layer(npm);
        self.powershell.apply_bool_layer(powershell);
        self.shell.apply_bool_layer(shell);
        self.pkg.apply_bool_layer(pkg);
        self.updater.apply_val(updater);
        self.always_use_latest_updater
            .apply_val(always_use_latest_updater);
    }
}

/// inheritable installer fields (raw from file)
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CommonInstallerLayer {
    /// The strategy to use for selecting a path to install things at:
    ///
    /// * `CARGO_HOME`: (default) install as if cargo did
    ///   (try `$CARGO_HOME/bin/`, but if `$CARGO_HOME` isn't set use `$HOME/.cargo/bin/`)
    /// * `~/some/subdir/`: install to the given subdir of the user's `$HOME`
    /// * `$SOME_VAR/some/subdir`: install to the given subdir of the dir defined by `$SOME_VAR`
    ///
    /// All of these error out if the required env-vars aren't set. In the future this may
    /// allow for the input to be an array of options to try in sequence.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, with = "opt_string_or_vec")]
    pub install_path: Option<Vec<InstallPathStrategy>>,

    /// Custom success message for installers
    ///
    /// When an shell or powershell installer succeeds at installing your app it
    /// will out put a message to the user. This config allows a user to specify
    /// a custom message as opposed to the default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_success_msg: Option<String>,

    /// Whether installers should install libraries from the release archive
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, with = "opt_string_or_vec")]
    pub install_libraries: Option<Vec<LibraryStyle>>,

    /// Aliases to install binaries as
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin_aliases: Option<SortedMap<String, Vec<String>>>,
}
/// inheritable installer fields (final)
#[derive(Debug, Default, Clone)]
pub struct CommonInstallerConfig {
    /// The strategy to use for selecting a path to install things at:
    ///
    /// * `CARGO_HOME`: (default) install as if cargo did
    ///   (try `$CARGO_HOME/bin/`, but if `$CARGO_HOME` isn't set use `$HOME/.cargo/bin/`)
    /// * `~/some/subdir/`: install to the given subdir of the user's `$HOME`
    /// * `$SOME_VAR/some/subdir`: install to the given subdir of the dir defined by `$SOME_VAR`
    ///
    /// All of these error out if the required env-vars aren't set. In the future this may
    /// allow for the input to be an array of options to try in sequence.
    pub install_path: Vec<InstallPathStrategy>,

    /// Custom success message for installers
    ///
    /// When an shell or powershell installer succeeds at installing your app it
    /// will out put a message to the user. This config allows a user to specify
    /// a custom message as opposed to the default.
    pub install_success_msg: String,

    /// Whether installers should install libraries from the release archive
    pub install_libraries: Vec<LibraryStyle>,

    /// Aliases to install binaries as
    pub bin_aliases: SortedMap<String, Vec<String>>,

    /// Whether to install an updater program alongside the software
    pub install_updater: bool,
}
impl CommonInstallerConfig {
    /// defaults
    pub fn defaults() -> Self {
        Self {
            install_path: InstallPathStrategy::default_list(),
            install_success_msg: "everything's installed!".to_owned(),
            install_libraries: Default::default(),
            bin_aliases: Default::default(),
            install_updater: false,
        }
    }
}
impl ApplyLayer for CommonInstallerConfig {
    type Layer = CommonInstallerLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            install_path,
            install_success_msg,
            install_libraries,
            bin_aliases,
        }: Self::Layer,
    ) {
        self.install_path.apply_val(install_path);
        self.install_success_msg.apply_val(install_success_msg);
        self.install_libraries.apply_val(install_libraries);
        self.bin_aliases.apply_val(bin_aliases);
    }
}
impl ApplyLayer for CommonInstallerLayer {
    type Layer = CommonInstallerLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            install_path,
            install_success_msg,
            install_libraries,
            bin_aliases,
        }: Self::Layer,
    ) {
        self.install_path.apply_opt(install_path);
        self.install_success_msg.apply_opt(install_success_msg);
        self.install_libraries.apply_opt(install_libraries);
        self.bin_aliases.apply_opt(bin_aliases);
    }
}
