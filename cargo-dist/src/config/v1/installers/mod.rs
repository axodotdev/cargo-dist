//! TODO

pub mod homebrew;
pub mod msi;
pub mod npm;
pub mod powershell;
pub mod shell;

use super::*;

use homebrew::*;
use msi::*;
use npm::*;
use powershell::*;
use shell::*;

/// TODO
#[derive(Debug, Default, Clone)]
pub struct InstallerConfig {
    /// TODO
    pub homebrew: Option<HomebrewInstallerConfig>,
    /// TODO
    pub msi: Option<MsiInstallerConfig>,
    /// TODO
    pub npm: Option<NpmInstallerConfig>,
    /// TODO
    pub powershell: Option<PowershellInstallerConfig>,
    /// TODO
    pub shell: Option<ShellInstallerConfig>,
}

/// TODO
#[derive(Debug, Clone)]
pub struct InstallerConfigInheritable {
    /// TODO
    pub common: CommonInstallerConfig,
    /// TODO
    pub homebrew: Option<HomebrewInstallerLayer>,
    /// TODO
    pub msi: Option<MsiInstallerLayer>,
    /// TODO
    pub npm: Option<NpmInstallerLayer>,
    /// TODO
    pub powershell: Option<PowershellInstallerLayer>,
    /// TODO
    pub shell: Option<ShellInstallerLayer>,
}

/// TODO
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct InstallerLayer {
    /// TODO
    #[serde(flatten)]
    pub common: CommonInstallerLayer,
    /// TODO
    pub homebrew: Option<BoolOr<HomebrewInstallerLayer>>,
    /// TODO
    pub msi: Option<BoolOr<MsiInstallerLayer>>,
    /// TODO
    pub npm: Option<BoolOr<NpmInstallerLayer>>,
    /// TODO
    pub powershell: Option<BoolOr<PowershellInstallerLayer>>,
    /// TODO
    pub shell: Option<BoolOr<ShellInstallerLayer>>,
}
impl InstallerConfigInheritable {
    /// TODO
    pub fn defaults_for_package(workspaces: &WorkspaceGraph, pkg_idx: PackageIdx) -> Self {
        Self {
            common: CommonInstallerConfig::defaults_for_package(workspaces, pkg_idx),
            homebrew: None,
            msi: None,
            npm: None,
            powershell: None,
            shell: None,
        }
    }
    /// TODO
    pub fn apply_inheritance_for_package(
        self,
        workspaces: &WorkspaceGraph,
        pkg_idx: PackageIdx,
    ) -> InstallerConfig {
        let Self {
            common,
            homebrew,
            msi,
            npm,
            powershell,
            shell,
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
        InstallerConfig {
            homebrew,
            msi,
            npm,
            powershell,
            shell,
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
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.homebrew.apply_bool_layer(homebrew);
        self.msi.apply_bool_layer(msi);
        self.npm.apply_bool_layer(npm);
        self.powershell.apply_bool_layer(powershell);
        self.shell.apply_bool_layer(shell);
    }
}

/// TODO
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

    /// Custom sucess message for installers
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

    /// Whether to install an updater program alongside the software
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_updater: Option<bool>,
}
/// TODO
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

    /// Custom sucess message for installers
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
    /// TODO
    pub fn defaults_for_package(_workspaces: &WorkspaceGraph, _pkg_idx: PackageIdx) -> Self {
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
            install_updater,
        }: Self::Layer,
    ) {
        self.install_path.apply_val(install_path);
        self.install_success_msg.apply_val(install_success_msg);
        self.install_libraries.apply_val(install_libraries);
        self.bin_aliases.apply_val(bin_aliases);
        self.install_updater.apply_val(install_updater);
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
            install_updater,
        }: Self::Layer,
    ) {
        self.install_path.apply_opt(install_path);
        self.install_success_msg.apply_opt(install_success_msg);
        self.install_libraries.apply_opt(install_libraries);
        self.bin_aliases.apply_opt(bin_aliases);
        self.install_updater.apply_opt(install_updater);
    }
}
