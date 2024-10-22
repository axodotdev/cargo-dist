//! Installer Generation
//!
//! In the future this might get split up into submodules.

use std::collections::BTreeMap;

use camino::Utf8PathBuf;
use cargo_dist_schema::TargetTriple;
use macpkg::PkgInstallerInfo;
use serde::Serialize;

use crate::{
    config::{JinjaInstallPathStrategy, LibraryStyle, ZipStyle},
    platform::{PlatformSupport, RuntimeConditions},
    InstallReceipt, ReleaseIdx,
};

use self::homebrew::HomebrewInstallerInfo;
use self::msi::MsiInstallerInfo;
use self::npm::NpmInstallerInfo;

pub mod homebrew;
pub mod macpkg;
pub mod msi;
pub mod npm;
pub mod powershell;
pub mod shell;

/// A kind of an installer
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum InstallerImpl {
    /// shell installer script
    Shell(InstallerInfo),
    /// powershell installer script
    Powershell(InstallerInfo),
    /// npm installer package
    Npm(NpmInstallerInfo),
    /// Homebrew formula
    Homebrew(HomebrewInstallerInfo),
    /// Windows msi installer
    Msi(MsiInstallerInfo),
    /// Mac pkg installer
    Pkg(PkgInstallerInfo),
}

/// Generic info about an installer
#[derive(Debug, Clone, Serialize)]
pub struct InstallerInfo {
    /// The parent release
    #[serde(skip)]
    pub release: ReleaseIdx,
    /// The path to generate the installer at
    pub dest_path: Utf8PathBuf,
    /// App name to use (display only)
    pub app_name: String,
    /// App version to use (display only)
    pub app_version: String,
    /// URL of the directory where artifacts can be fetched from
    pub base_url: String,
    /// Artifacts this installer can fetch
    pub artifacts: Vec<ExecutableZipFragment>,
    /// Description of the installer (a good heading)
    pub desc: String,
    /// Hint for how to run the installer
    pub hint: String,
    /// Where to install binaries
    pub install_paths: Vec<JinjaInstallPathStrategy>,
    /// Custom message to display on install success
    pub install_success_msg: String,
    /// Install receipt to write, if any
    pub receipt: Option<InstallReceipt>,
    /// Aliases to install binaries under
    pub bin_aliases: BTreeMap<TargetTriple, BTreeMap<String, Vec<String>>>,
    /// Whether to install generated C dynamic libraries
    pub install_libraries: Vec<LibraryStyle>,
    /// Platform-specific runtime conditions
    pub runtime_conditions: RuntimeConditions,
    /// platform support matrix
    pub platform_support: Option<PlatformSupport>,
    /// Environment variable to force an install location
    pub install_dir_env_var: String,
    /// Like the above, but for unmanaged installs
    pub unmanaged_dir_env_var: String,
    /// Environment variable to disable self-update features
    pub disable_update_env_var: String,
    /// Environment variable to disable modifying the path
    pub no_modify_path_env_var: String,
}

/// A fake fragment of an ExecutableZip artifact for installers
#[derive(Debug, Clone, Serialize)]
pub struct ExecutableZipFragment {
    /// The id of the artifact
    pub id: String,
    /// The target the artifact supports
    pub target_triple: TargetTriple,
    /// The executables the artifact contains (name, assumed at root)
    pub executables: Vec<String>,
    /// The dynamic libraries the artifact contains (name, assumed at root)
    pub cdylibs: Vec<String>,
    /// The static libraries the artifact contains (name, assumed at root)
    pub cstaticlibs: Vec<String>,
    /// The style of zip this is
    pub zip_style: ZipStyle,
    /// The updater associated with this platform
    pub updater: Option<UpdaterFragment>,
    /// Conditions the system being installed to should ideally satisfy to install this
    pub runtime_conditions: RuntimeConditions,
}

/// A fake fragment of an Updater artifact for installers
#[derive(Debug, Clone, Serialize)]
pub struct UpdaterFragment {
    /// The id of the artifact
    pub id: String,
    /// The binary the artifact contains (name, assumed at root)
    pub binary: String,
}
