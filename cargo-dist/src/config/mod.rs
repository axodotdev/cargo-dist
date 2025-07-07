//! Config types (for workspace.metadata.dist)

use std::collections::BTreeMap;

use axoasset::{toml_edit, SourceFile};
use axoproject::local_repo::LocalRepo;
use camino::{Utf8Path, Utf8PathBuf};
use dist_schema::{
    AptPackageName, ChecksumExtensionRef, ChocolateyPackageName, GithubAttestationsFilters,
    GithubAttestationsPhase, HomebrewPackageName, PackageVersion, TripleName, TripleNameRef,
};
use serde::{Deserialize, Serialize};

use crate::announce::TagSettings;
use crate::SortedMap;
use crate::{
    errors::{DistError, DistResult},
    METADATA_DIST,
};

mod version;
pub use version::{get_version, want_v1, ConfigVersion};

pub mod v0;
pub mod v0_to_v1;
pub mod v1;

pub(crate) use v0::{parse_metadata_table_or_manifest, reject_metadata_table, DistMetadata};

/// values of the form `permission-name: read`
pub type GithubPermissionMap = SortedMap<String, GithubPermission>;

/// Possible values for a github ci permission
///
/// These are assumed to be strictly increasing in power, so admin includes write includes read.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, PartialOrd, Ord, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum GithubPermission {
    /// Read (min)
    Read,
    /// Write
    Write,
    /// Admin (max)
    Admin,
}

/// Global config for commands
#[derive(Debug, Clone)]
pub struct Config {
    /// Settings for the announcement tag
    pub tag_settings: TagSettings,
    /// Whether to actually try to side-effectfully create a hosting directory on a server
    ///
    /// this is used for compute_hosting
    pub create_hosting: bool,
    /// The subset of artifacts we want to build
    pub artifact_mode: ArtifactMode,
    /// Whether local paths to files should be in the final dist json output
    pub no_local_paths: bool,
    /// If true, override allow-dirty in the config and ignore all dirtiness
    pub allow_all_dirty: bool,
    /// Target triples we want to build for
    pub targets: Vec<TripleName>,
    /// CI kinds we want to support
    pub ci: Vec<CiStyle>,
    /// Installers we want to generate
    pub installers: Vec<InstallerStyle>,
    /// What command was being invoked here, used for SystemIds
    pub root_cmd: String,
}

/// How we should select the artifacts to build
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtifactMode {
    /// Build target-specific artifacts like archives, symbols, msi installers
    Local,
    /// Build globally unique artifacts like curl-sh installers, npm packages, metadata...
    Global,
    /// Fuzzily build "as much as possible" for the host system
    Host,
    /// Build all the artifacts; only really appropriate for `dist manifest`
    All,
    /// Fake all the artifacts; useful for testing/mocking/staging
    Lies,
}

impl std::fmt::Display for ArtifactMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            ArtifactMode::Local => "local",
            ArtifactMode::Global => "global",
            ArtifactMode::Host => "host",
            ArtifactMode::All => "all",
            ArtifactMode::Lies => "lies",
        };
        string.fmt(f)
    }
}

/// The style of CI we should generate
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum CiStyle {
    /// Generate Github CI
    Github,
}
impl CiStyle {
    /// If the CI provider provides a native release hosting system, get it
    pub(crate) fn native_hosting(&self) -> Option<HostingStyle> {
        match self {
            CiStyle::Github => Some(HostingStyle::Github),
        }
    }
}

impl std::fmt::Display for CiStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            CiStyle::Github => "github",
        };
        string.fmt(f)
    }
}

impl std::str::FromStr for CiStyle {
    type Err = DistError;
    fn from_str(val: &str) -> DistResult<Self> {
        let res = match val {
            "github" => CiStyle::Github,
            s => {
                return Err(DistError::UnrecognizedCiStyle {
                    style: s.to_string(),
                })
            }
        };
        Ok(res)
    }
}

/// Type of library to install
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum LibraryStyle {
    /// cdylib
    #[serde(rename = "cdylib")]
    CDynamic,
    /// cstaticlib
    #[serde(rename = "cstaticlib")]
    CStatic,
}

impl std::fmt::Display for LibraryStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            Self::CDynamic => "cdylib",
            Self::CStatic => "cstaticlib",
        };
        string.fmt(f)
    }
}

impl std::str::FromStr for LibraryStyle {
    type Err = DistError;
    fn from_str(val: &str) -> DistResult<Self> {
        let res = match val {
            "cdylib" => Self::CDynamic,
            "cstaticlib" => Self::CStatic,
            s => {
                return Err(DistError::UnrecognizedLibraryStyle {
                    style: s.to_string(),
                })
            }
        };
        Ok(res)
    }
}

/// The style of Installer we should generate
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum InstallerStyle {
    /// Generate a shell script that fetches from [`dist_schema::Release::artifact_download_url`][]
    Shell,
    /// Generate a powershell script that fetches from [`dist_schema::Release::artifact_download_url`][]
    Powershell,
    /// Generate an npm project that fetches from [`dist_schema::Release::artifact_download_url`][]
    Npm,
    /// Generate a Homebrew formula that fetches from [`dist_schema::Release::artifact_download_url`][]
    Homebrew,
    /// Generate an msi installer that embeds the binary
    Msi,
    /// Generate an Apple pkg installer that embeds the binary
    Pkg,
}

impl std::fmt::Display for InstallerStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            InstallerStyle::Shell => "shell",
            InstallerStyle::Powershell => "powershell",
            InstallerStyle::Npm => "npm",
            InstallerStyle::Homebrew => "homebrew",
            InstallerStyle::Msi => "msi",
            InstallerStyle::Pkg => "pkg",
        };
        string.fmt(f)
    }
}

/// When to create GitHub releases
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum GithubReleasePhase {
    /// Release position depends on whether axo releases is enabled
    #[default]
    Auto,
    /// Create release during the "host" stage, before npm and Homebrew
    Host,
    /// Create release during the "announce" stage, after all publish jobs
    Announce,
}

impl std::fmt::Display for GithubReleasePhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            GithubReleasePhase::Auto => "auto",
            GithubReleasePhase::Host => "host",
            GithubReleasePhase::Announce => "announce",
        };
        string.fmt(f)
    }
}

/// The style of hosting we should use for artifacts
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum HostingStyle {
    /// Host on Github Releases
    Github,
}

impl std::fmt::Display for HostingStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            HostingStyle::Github => "github",
        };
        string.fmt(f)
    }
}

impl std::str::FromStr for HostingStyle {
    type Err = DistError;
    fn from_str(val: &str) -> DistResult<Self> {
        let res = match val {
            "github" => HostingStyle::Github,
            s => {
                return Err(DistError::UnrecognizedHostingStyle {
                    style: s.to_string(),
                })
            }
        };
        Ok(res)
    }
}

/// The publish jobs we should run
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PublishStyle {
    /// Publish a Homebrew formula to a tap repository
    Homebrew,
    /// Publish an npm pkg to the global npm registry
    Npm,
    /// User-supplied value
    User(String),
}

impl std::str::FromStr for PublishStyle {
    type Err = DistError;
    fn from_str(s: &str) -> DistResult<Self> {
        if let Some(slug) = s.strip_prefix("./") {
            Ok(Self::User(slug.to_owned()))
        } else if s == "homebrew" {
            Ok(Self::Homebrew)
        } else if s == "npm" {
            Ok(Self::Npm)
        } else {
            Err(DistError::UnrecognizedJobStyle {
                style: s.to_owned(),
            })
        }
    }
}

impl<'de> serde::Deserialize<'de> for PublishStyle {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let path = String::deserialize(deserializer)?;
        path.parse().map_err(|e| D::Error::custom(format!("{e}")))
    }
}

impl std::fmt::Display for PublishStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PublishStyle::Homebrew => write!(f, "homebrew"),
            PublishStyle::Npm => write!(f, "npm"),
            PublishStyle::User(s) => write!(f, "./{s}"),
        }
    }
}

/// Extra CI jobs we should run
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobStyle {
    /// User-supplied value
    User(String),
}

impl std::str::FromStr for JobStyle {
    type Err = DistError;
    fn from_str(s: &str) -> DistResult<Self> {
        if let Some(slug) = s.strip_prefix("./") {
            Ok(Self::User(slug.to_owned()))
        } else {
            Err(DistError::UnrecognizedJobStyle {
                style: s.to_owned(),
            })
        }
    }
}

impl serde::Serialize for JobStyle {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = self.to_string();
        s.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for JobStyle {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let path = String::deserialize(deserializer)?;
        path.parse().map_err(|e| D::Error::custom(format!("{e}")))
    }
}

impl std::fmt::Display for JobStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStyle::User(s) => write!(f, "./{s}"),
        }
    }
}

/// The style of zip/tarball to make
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipStyle {
    /// `.zip`
    Zip,
    /// `.tar.<compression>`
    Tar(CompressionImpl),
    /// Don't bundle/compress this, it's just a temp dir
    TempDir,
}

/// Compression impls (used by [`ZipStyle::Tar`][])
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CompressionImpl {
    /// `.gz`
    Gzip,
    /// `.xz`
    Xzip,
    /// `.zst`
    Zstd,
}
impl ZipStyle {
    /// Get the extension used for this kind of zip
    pub fn ext(&self) -> &'static str {
        match self {
            ZipStyle::TempDir => "",
            ZipStyle::Zip => ".zip",
            ZipStyle::Tar(compression) => match compression {
                CompressionImpl::Gzip => ".tar.gz",
                CompressionImpl::Xzip => ".tar.xz",
                CompressionImpl::Zstd => ".tar.zst",
            },
        }
    }
}

impl Serialize for ZipStyle {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.ext())
    }
}

impl<'de> Deserialize<'de> for ZipStyle {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let ext = String::deserialize(deserializer)?;
        match &*ext {
            ".zip" => Ok(ZipStyle::Zip),
            ".tar.gz" => Ok(ZipStyle::Tar(CompressionImpl::Gzip)),
            ".tar.xz" => Ok(ZipStyle::Tar(CompressionImpl::Xzip)),
            ".tar.zstd" | ".tar.zst" => Ok(ZipStyle::Tar(CompressionImpl::Zstd)),
            _ => Err(D::Error::custom(format!(
                "unknown archive format {ext}, expected one of: .zip, .tar.gz, .tar.xz, .tar.zstd, .tar.zst"
            ))),
        }
    }
}

/// key for the install-path config that selects [`InstallPathStrategyCargoHome`][]
const CARGO_HOME_INSTALL_PATH: &str = "CARGO_HOME";

/// Strategy for install binaries
#[derive(Debug, Clone, PartialEq)]
pub enum InstallPathStrategy {
    /// install to $CARGO_HOME, falling back to ~/.cargo/
    CargoHome,
    /// install to this subdir of the user's home
    ///
    /// syntax: `~/subdir`
    HomeSubdir {
        /// The subdir of home to install to
        subdir: String,
    },
    /// install to this subdir of this env var
    ///
    /// syntax: `$ENV_VAR/subdir`
    EnvSubdir {
        /// The env var to get the base of the path from
        env_key: String,
        /// The subdir to install to
        subdir: String,
    },
}

impl InstallPathStrategy {
    /// Returns the default set of install paths
    pub fn default_list() -> Vec<Self> {
        vec![InstallPathStrategy::CargoHome]
    }
}

impl std::str::FromStr for InstallPathStrategy {
    type Err = DistError;
    fn from_str(path: &str) -> DistResult<Self> {
        if path == CARGO_HOME_INSTALL_PATH {
            Ok(InstallPathStrategy::CargoHome)
        } else if let Some(subdir) = path.strip_prefix("~/") {
            if subdir.is_empty() {
                Err(DistError::InstallPathHomeSubdir {
                    path: path.to_owned(),
                })
            } else {
                Ok(InstallPathStrategy::HomeSubdir {
                    // If there's a trailing slash, strip it to be uniform
                    subdir: subdir.strip_suffix('/').unwrap_or(subdir).to_owned(),
                })
            }
        } else if let Some(val) = path.strip_prefix('$') {
            if let Some((env_key, subdir)) = val.split_once('/') {
                Ok(InstallPathStrategy::EnvSubdir {
                    env_key: env_key.to_owned(),
                    // If there's a trailing slash, strip it to be uniform
                    subdir: subdir.strip_suffix('/').unwrap_or(subdir).to_owned(),
                })
            } else {
                Err(DistError::InstallPathEnvSlash {
                    path: path.to_owned(),
                })
            }
        } else {
            Err(DistError::InstallPathInvalid {
                path: path.to_owned(),
            })
        }
    }
}

impl std::fmt::Display for InstallPathStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallPathStrategy::CargoHome => write!(f, "{}", CARGO_HOME_INSTALL_PATH),
            InstallPathStrategy::HomeSubdir { subdir } => write!(f, "~/{subdir}"),
            InstallPathStrategy::EnvSubdir { env_key, subdir } => write!(f, "${env_key}/{subdir}"),
        }
    }
}

impl serde::Serialize for InstallPathStrategy {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for InstallPathStrategy {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let path = String::deserialize(deserializer)?;
        path.parse().map_err(|e| D::Error::custom(format!("{e}")))
    }
}

/// A GitHub repo like 'axodotdev/axolotlsay'
#[derive(Debug, Clone, PartialEq)]
pub struct GithubRepoPair {
    /// owner (axodotdev)
    pub owner: String,
    /// repo (axolotlsay)
    pub repo: String,
}

impl std::str::FromStr for GithubRepoPair {
    type Err = DistError;
    fn from_str(pair: &str) -> DistResult<Self> {
        let Some((owner, repo)) = pair.split_once('/') else {
            return Err(DistError::GithubRepoPairParse {
                pair: pair.to_owned(),
            });
        };
        Ok(GithubRepoPair {
            owner: owner.to_owned(),
            repo: repo.to_owned(),
        })
    }
}

impl std::fmt::Display for GithubRepoPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.owner, self.repo)
    }
}

impl serde::Serialize for GithubRepoPair {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for GithubRepoPair {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let path = String::deserialize(deserializer)?;
        path.parse().map_err(|e| D::Error::custom(format!("{e}")))
    }
}

impl GithubRepoPair {
    /// Convert this into a jinja-friendly form
    pub fn into_jinja(self) -> JinjaGithubRepoPair {
        JinjaGithubRepoPair {
            owner: self.owner,
            repo: self.repo,
        }
    }
}

/// Jinja-friendly version of [`GithubRepoPair`][]
#[derive(Debug, Clone, Serialize)]
pub struct JinjaGithubRepoPair {
    /// owner
    pub owner: String,
    /// repo
    pub repo: String,
}

/// Strategy for install binaries (replica to have different Serialize for jinja)
///
/// The serialize/deserialize impls are already required for loading/saving the config
/// from toml/json, and that serialize impl just creates a plain string again. To allow
/// jinja templates to have richer context we have use duplicate type with a more
/// conventional derived serialize.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum JinjaInstallPathStrategy {
    /// install to $CARGO_HOME, falling back to ~/.cargo/
    CargoHome,
    /// install to this subdir of the user's home
    ///
    /// syntax: `~/subdir`
    HomeSubdir {
        /// The subdir of home to install to
        subdir: String,
    },
    /// install to this subdir of this env var
    ///
    /// syntax: `$ENV_VAR/subdir`
    EnvSubdir {
        /// The env var to get the base of the path from
        env_key: String,
        /// The subdir to install to
        subdir: String,
    },
}

impl InstallPathStrategy {
    /// Convert this into a jinja-friendly form
    pub fn into_jinja(self) -> JinjaInstallPathStrategy {
        match self {
            InstallPathStrategy::CargoHome => JinjaInstallPathStrategy::CargoHome,
            InstallPathStrategy::HomeSubdir { subdir } => {
                JinjaInstallPathStrategy::HomeSubdir { subdir }
            }
            InstallPathStrategy::EnvSubdir { env_key, subdir } => {
                JinjaInstallPathStrategy::EnvSubdir { env_key, subdir }
            }
        }
    }
}

/// A checksumming algorithm
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChecksumStyle {
    /// sha256sum (using the sha2 crate)
    Sha256,
    /// sha512sum (using the sha2 crate)
    Sha512,
    /// sha3-256sum (using the sha3 crate)
    Sha3_256,
    /// sha3-512sum (using the sha3 crate)
    Sha3_512,
    /// b2sum (using the blake2 crate)
    Blake2s,
    /// b2sum (using the blake2 crate)
    Blake2b,
    /// Do not checksum
    False,
}

impl ChecksumStyle {
    /// Get the extension of a checksum
    pub fn ext(self) -> &'static ChecksumExtensionRef {
        ChecksumExtensionRef::from_str(match self {
            ChecksumStyle::Sha256 => "sha256",
            ChecksumStyle::Sha512 => "sha512",
            ChecksumStyle::Sha3_256 => "sha3-256",
            ChecksumStyle::Sha3_512 => "sha3-512",
            ChecksumStyle::Blake2s => "blake2s",
            ChecksumStyle::Blake2b => "blake2b",
            ChecksumStyle::False => "false",
        })
    }
}

impl std::fmt::Display for ChecksumStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.ext())
    }
}

/// Which style(s) of configuration to generate
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GenerateMode {
    /// Generate CI scripts for orchestrating dist
    #[serde(rename = "ci")]
    Ci,
    /// Generate wsx (WiX) templates for msi installers
    #[serde(rename = "msi")]
    Msi,
}

impl std::fmt::Display for GenerateMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GenerateMode::Ci => "ci".fmt(f),
            GenerateMode::Msi => "msi".fmt(f),
        }
    }
}

/// Arguments to `dist host`
#[derive(Clone, Debug)]
pub struct HostArgs {
    /// Which hosting steps to run
    pub steps: Vec<HostStyle>,
}

/// What parts of hosting to perform
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HostStyle {
    /// Check that hosting API keys are working
    Check,
    /// Create a location to host artifacts
    Create,
    /// Upload artifacts
    Upload,
    /// Release artifacts
    Release,
    /// Announce artifacts
    Announce,
}

impl std::fmt::Display for HostStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            HostStyle::Check => "check",
            HostStyle::Create => "create",
            HostStyle::Upload => "upload",
            HostStyle::Release => "release",
            HostStyle::Announce => "announce",
        };
        string.fmt(f)
    }
}

/// Configuration for Mac .pkg installers
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct MacPkgConfig {
    /// A unique identifier, in tld.domain.package format
    pub identifier: Option<String>,
    /// The location to which the software should be installed.
    /// If not specified, /usr/local will be used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_location: Option<String>,
}

/// Packages to install before build from the system package manager
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SystemDependencies {
    /// Packages to install in Homebrew
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub homebrew: BTreeMap<HomebrewPackageName, SystemDependency>,

    /// Packages to install in apt
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub apt: BTreeMap<AptPackageName, SystemDependency>,

    /// Package to install in Chocolatey
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub chocolatey: BTreeMap<ChocolateyPackageName, SystemDependency>,
}

impl SystemDependencies {
    /// Extends `self` with the elements of `other`.
    pub fn append(&mut self, other: &mut Self) {
        self.homebrew.append(&mut other.homebrew);
        self.apt.append(&mut other.apt);
        self.chocolatey.append(&mut other.chocolatey);
    }
}

/// Represents a package from a system package manager
// newtype wrapper to hang a manual derive impl off of
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct SystemDependency(pub SystemDependencyComplex);

/// Backing type for SystemDependency
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub struct SystemDependencyComplex {
    /// The version to install, as expected by the underlying package manager
    pub version: Option<PackageVersion>,
    /// Stages at which the dependency is required
    #[serde(default)]
    pub stage: Vec<DependencyKind>,
    /// One or more targets this package should be installed on; defaults to all targets if not specified
    #[serde(default)]
    pub targets: Vec<TripleName>,
}

impl SystemDependencyComplex {
    /// Checks if this dependency should be installed on the specified target.
    pub fn wanted_for_target(&self, target: &TripleNameRef) -> bool {
        if self.targets.is_empty() {
            true
        } else {
            self.targets.iter().any(|t| t == target)
        }
    }

    /// Checks if this dependency should used in the specified stage.
    pub fn stage_wanted(&self, stage: &DependencyKind) -> bool {
        if self.stage.is_empty() {
            match stage {
                DependencyKind::Build => true,
                DependencyKind::Run => false,
            }
        } else {
            self.stage.contains(stage)
        }
    }
}

/// Definition for a single package
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SystemDependencyKind {
    /// Simple specification format, parsed as cmake = 'version'
    /// The special string "*" is parsed as a None version
    Untagged(String),
    /// Complex specification format
    Tagged(SystemDependencyComplex),
}

/// Provides detail on when a specific dependency is required
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DependencyKind {
    /// A dependency that must be present when the software is being built
    Build,
    /// A dependency that must be present when the software is being used
    Run,
}

impl std::fmt::Display for DependencyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DependencyKind::Build => "build".fmt(f),
            DependencyKind::Run => "run".fmt(f),
        }
    }
}

impl<'de> Deserialize<'de> for SystemDependency {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let kind: SystemDependencyKind = SystemDependencyKind::deserialize(deserializer)?;

        let res = match kind {
            SystemDependencyKind::Untagged(version) => {
                let v = if version == "*" { None } else { Some(version) };
                SystemDependencyComplex {
                    version: v.map(PackageVersion::new),
                    stage: vec![],
                    targets: vec![],
                }
            }
            SystemDependencyKind::Tagged(dep) => dep,
        };

        Ok(SystemDependency(res))
    }
}

/// Settings for which Generate targets can be dirty
#[derive(Debug, Clone)]
pub enum DirtyMode {
    /// Allow only these targets
    AllowList(Vec<GenerateMode>),
    /// Allow all targets
    AllowAll,
}

impl DirtyMode {
    /// Do we need to run this Generate Mode
    pub fn should_run(&self, mode: GenerateMode) -> bool {
        match self {
            DirtyMode::AllowAll => false,
            DirtyMode::AllowList(list) => !list.contains(&mode),
        }
    }
}

/// For features that can be generated in "test" or "production" mode
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProductionMode {
    /// test mode
    Test,
    /// production mode
    Prod,
}

/// An extra artifact to upload alongside the release tarballs,
/// and the build command which produces it.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ExtraArtifact {
    /// The working dir to run the command in
    ///
    /// If blank, the directory of the manifest that defines this is used.
    #[serde(default)]
    #[serde(skip_serializing_if = "path_is_empty")]
    pub working_dir: Utf8PathBuf,
    /// The build command to invoke in the working_dir
    #[serde(rename = "build")]
    pub command: Vec<String>,
    /// Relative paths (from the working_dir) to artifacts that should be included
    #[serde(rename = "artifacts")]
    pub artifact_relpaths: Vec<Utf8PathBuf>,
}

/// Why doesn't this exist omg
fn path_is_empty(p: &Utf8PathBuf) -> bool {
    p.as_str().is_empty()
}

impl std::fmt::Display for ProductionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProductionMode::Test => "test".fmt(f),
            ProductionMode::Prod => "prod".fmt(f),
        }
    }
}

/// Find the dist workspaces relative to the current directory
pub fn get_project() -> Result<axoproject::WorkspaceGraph, axoproject::errors::ProjectError> {
    let start_dir = std::env::current_dir().expect("couldn't get current working dir!?");
    let start_dir = Utf8PathBuf::from_path_buf(start_dir).expect("project path isn't utf8!?");
    let repo = LocalRepo::new("git", &start_dir).ok();
    let workspaces = axoproject::WorkspaceGraph::find_from_git(&start_dir, repo)?;
    Ok(workspaces)
}

/// Load a TOML file to a toml-edit document.
pub fn load_toml(manifest_path: &Utf8Path) -> DistResult<toml_edit::DocumentMut> {
    let src = axoasset::SourceFile::load_local(manifest_path)?;
    let toml = src.deserialize_toml_edit()?;
    Ok(toml)
}

/// Save a toml-edit document to a TOML file.
pub fn write_toml(manifest_path: &Utf8Path, toml: toml_edit::DocumentMut) -> DistResult<()> {
    let toml_text = toml.to_string();
    axoasset::LocalAsset::write_new(&toml_text, manifest_path)?;
    Ok(())
}

/// Get the `[workspace.metadata]` or `[package.metadata]` (based on `is_workspace`)
pub fn get_toml_metadata(
    toml: &mut toml_edit::DocumentMut,
    is_workspace: bool,
) -> &mut toml_edit::Item {
    // Walk down/prepare the components...
    let root_key = if is_workspace { "workspace" } else { "package" };
    let workspace = toml[root_key].or_insert(toml_edit::table());
    if let Some(t) = workspace.as_table_mut() {
        t.set_implicit(true)
    }
    let metadata = workspace["metadata"].or_insert(toml_edit::table());
    if let Some(t) = metadata.as_table_mut() {
        t.set_implicit(true)
    }

    metadata
}

/// This module implements support for serializing and deserializing
/// `Option<Vec<T>>> where T: Display + FromStr`
/// when we want both of these syntaxes to be valid:
///
/// * install-path = "~/.mycompany"
/// * install-path = ["$MY_COMPANY", "~/.mycompany"]
///
/// Notable corners of roundtripping:
///
/// * `["one_elem"]`` will be force-rewritten as `"one_elem"` (totally equivalent and prettier)
/// * `[]` will be preserved as `[]` (it's semantically distinct from None when cascading config)
///
/// This is a variation on a documented serde idiom for "string or struct":
/// <https://serde.rs/string-or-struct.html>
mod opt_string_or_vec {
    use super::*;
    use serde::de::Error;

    pub fn serialize<S, T>(v: &Option<Vec<T>>, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
        T: std::fmt::Display,
    {
        // If none, do none
        let Some(vec) = v else {
            return s.serialize_none();
        };
        // If one item, make it a string
        if vec.len() == 1 {
            s.serialize_str(&vec[0].to_string())
        // If many items (or zero), make it a list
        } else {
            let string_vec = Vec::from_iter(vec.iter().map(ToString::to_string));
            string_vec.serialize(s)
        }
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<Vec<T>>, D::Error>
    where
        D: serde::Deserializer<'de>,
        T: std::str::FromStr,
        T::Err: std::fmt::Display,
    {
        struct StringOrVec<T>(std::marker::PhantomData<T>);

        impl<'de, T> serde::de::Visitor<'de> for StringOrVec<T>
        where
            T: std::str::FromStr,
            T::Err: std::fmt::Display,
        {
            type Value = Option<Vec<T>>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("string or list of strings")
            }

            // if none, return none
            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(None)
            }

            // if string, parse it and make a single-element list
            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(Some(vec![s
                    .parse()
                    .map_err(|e| E::custom(format!("{e}")))?]))
            }

            // if a sequence, parse the whole thing
            fn visit_seq<S>(self, seq: S) -> Result<Self::Value, S::Error>
            where
                S: serde::de::SeqAccess<'de>,
            {
                let vec: Vec<String> =
                    Deserialize::deserialize(serde::de::value::SeqAccessDeserializer::new(seq))?;
                let parsed: Result<Vec<T>, S::Error> = vec
                    .iter()
                    .map(|s| s.parse::<T>().map_err(|e| S::Error::custom(format!("{e}"))))
                    .collect();
                Ok(Some(parsed?))
            }
        }

        deserializer.deserialize_any(StringOrVec::<T>(std::marker::PhantomData))
    }
}
