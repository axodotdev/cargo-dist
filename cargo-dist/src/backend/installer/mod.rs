//! Installer Generation
//!
//! In the future this might get split up into submodules.

use std::collections::BTreeMap;

use camino::Utf8PathBuf;
use serde::Serialize;

use crate::{
    config::{JinjaInstallPathStrategy, ZipStyle},
    InstallReceipt, TargetTriple,
};

use self::homebrew::HomebrewInstallerInfo;
use self::msi::MsiInstallerInfo;
use self::npm::NpmInstallerInfo;

pub mod homebrew;
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
}

/// Generic info about an installer
#[derive(Debug, Clone, Serialize)]
pub struct InstallerInfo {
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
    pub bin_aliases: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    /// Whether to install generated C dynamic libraries
    pub install_cdylibs: bool,
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
}

/// A fake fragment of an Updater artifact for installers
#[derive(Debug, Clone, Serialize)]
pub struct UpdaterFragment {
    /// The id of the artifact
    pub id: String,
    /// The binary the artifact contains (name, assumed at root)
    pub binary: String,
}
