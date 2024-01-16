//! Installer Generation
//!
//! In the future this might get split up into submodules.

use camino::Utf8PathBuf;
use serde::Serialize;

use crate::{
    config::ZipStyle,
    TargetTriple, ReleaseIdx,
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
    /// URL of the directory where artifacts can be fetched from
    pub base_url: String,
    /// Description of the installer (a good heading)
    pub desc: String,
    /// Hint for how to run the installer
    pub hint: String,
    /// The release this is installing
    #[serde(skip)]
    pub release: ReleaseIdx,
}

/// A fake fragment of an ExecutableZip artifact for installers
#[derive(Debug, Clone, Serialize)]
pub struct ExecutableZipFragment {
    /// The id of the artifact
    pub id: String,
    /// The targets the artifact supports
    pub target_triples: Vec<TargetTriple>,
    /// The binaries the artifact contains (name, assumed at root)
    pub binaries: Vec<String>,
    /// The style of zip this is
    pub zip_style: ZipStyle,
}
