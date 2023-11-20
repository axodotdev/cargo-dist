#![deny(missing_docs)]

//! # cargo-dist-schema
//!
//! This crate exists to serialize and deserialize the dist-manifest.json produced
//! by cargo-dist. Ideally it should be reasonably forward and backward compatible
//! with different versions of this format.
//!
//! The root type of the schema is [`DistManifest`][].

use std::collections::BTreeMap;

use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};

/// A local system path on the machine cargo-dist was run.
///
/// This is a String because when deserializing this may be a path format from a different OS!
pub type LocalPath = String;
/// A relative path inside an artifact
///
/// This is a String because when deserializing this may be a path format from a different OS!
///
/// (Should we normalize this one?)
pub type RelPath = String;
/// The unique ID of an Artifact
pub type ArtifactId = String;

/// A report of the releases and artifacts that cargo-dist generated
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DistManifest {
    /// The version of cargo-dist that generated this
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dist_version: Option<String>,
    /// The (git) tag associated with this announcement
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub announcement_tag: Option<String>,
    /// Whether this announcement appears to be a prerelease
    #[serde(default)]
    pub announcement_is_prerelease: bool,
    /// A title for the announcement
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub announcement_title: Option<String>,
    /// A changelog for the announcement
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub announcement_changelog: Option<String>,
    /// A Github Releases body for the announcement
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub announcement_github_body: Option<String>,
    /// Info about the toolchain used to build this announcement
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_info: Option<SystemInfo>,
    /// App releases we're distributing
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub releases: Vec<Release>,
    /// The artifacts included in this Announcement, referenced by releases.
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub artifacts: BTreeMap<ArtifactId, Artifact>,
    /// Whether to publish prereleases to package managers
    #[serde(default)]
    pub publish_prereleases: bool,
    /// ci backend info
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ci: Option<CiInfo>,
    /// Data about dynamic linkage in the built libraries
    pub linkage: Vec<Linkage>,
}

/// CI backend info
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CiInfo {
    /// GitHub CI backend
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github: Option<GithubCiInfo>,
}

/// Github CI backend
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GithubCiInfo {
    /// Github CI Matrix for upload-artifacts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifacts_matrix: Option<GithubMatrix>,

    /// What kind of job to run on pull request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_run_mode: Option<PrRunMode>,
}

/// Github CI Matrix
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GithubMatrix {
    /// define each task manually rather than doing cross-product stuff
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<GithubMatrixEntry>,
}

/// Entry for a github matrix
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GithubMatrixEntry {
    /// Targets to build for
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<String>>,
    /// Github Runner to user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runner: Option<String>,
    /// Expression to execute to install cargo-dist
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_dist: Option<String>,
    /// Arguments to pass to cargo-dist
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dist_args: Option<String>,
    /// Command to run to install dependencies
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packages_install: Option<String>,
}

/// Type of job to run on pull request
#[derive(
    Debug, Copy, Clone, Serialize, Deserialize, JsonSchema, Default, PartialEq, Eq, PartialOrd, Ord,
)]
pub enum PrRunMode {
    /// Do not run on pull requests at all
    #[serde(rename = "skip")]
    Skip,
    /// Only run the plan step
    #[default]
    #[serde(rename = "plan")]
    Plan,
    /// Build and upload artifacts
    #[serde(rename = "upload")]
    Upload,
}

impl std::fmt::Display for PrRunMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrRunMode::Skip => write!(f, "skip"),
            PrRunMode::Plan => write!(f, "plan"),
            PrRunMode::Upload => write!(f, "upload"),
        }
    }
}

/// Info about the system/toolchain used to build this announcement.
///
/// Note that this is info from the machine that generated this file,
/// which *ideally* should be similar to the machines that built all the artifacts, but
/// we can't guarantee that.
///
/// dist-manifest.json is by default generated at the start of the build process,
/// and typically on a linux machine because that's usually the fastest/cheapest
/// part of CI infra.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SystemInfo {
    /// The version of Cargo used (first line of cargo -vV)
    ///
    /// Note that this is the version used on the machine that generated this file,
    /// which presumably should be the same version used on all the machines that
    /// built all the artifacts, but maybe not! It's more likely to be correct
    /// if rust-toolchain.toml is used with a specific pinned version.
    pub cargo_version_line: Option<String>,
}

/// A Release of an Application
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Release {
    /// The name of the app
    pub app_name: String,
    /// The version of the app
    // FIXME: should be a Version but JsonSchema doesn't support (yet?)
    pub app_version: String,
    /// The artifacts for this release (zips, debuginfo, metadata...)
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ArtifactId>,
    /// Hosting info
    #[serde(default)]
    #[serde(skip_serializing_if = "Hosting::is_empty")]
    pub hosting: Hosting,
}

/// A distributable artifact that's part of a Release
///
/// i.e. a zip or installer
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Artifact {
    /// The unique name of the artifact (e.g. `myapp-v1.0.0-x86_64-pc-windows-msvc.zip`)
    ///
    /// If this is missing then that indicates the artifact is purely informative and has
    /// no physical files associated with it. This may be used (in the future) to e.g.
    /// indicate you can install the application with `cargo install` or `npm install`.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub name: Option<String>,
    /// The kind of artifact this is (e.g. "executable-zip")
    #[serde(flatten)]
    pub kind: ArtifactKind,
    /// The target triple of the bundle
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub target_triples: Vec<String>,
    /// The location of the artifact on the local system
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub path: Option<LocalPath>,
    /// Assets included in the bundle (like executables and READMEs)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub assets: Vec<Asset>,
    /// A string describing how to install this
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub install_hint: Option<String>,
    /// A brief description of what this artifact is
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub description: Option<String>,
    /// id of an that contains the checksum for this artifact
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub checksum: Option<String>,
}

/// An asset contained in an artifact (executable, license, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Asset {
    /// The high-level name of the asset
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The path of the asset relative to the root of the artifact
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<RelPath>,
    /// The kind of asset this is
    #[serde(flatten)]
    pub kind: AssetKind,
}

/// An artifact included in a Distributable
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind")]
#[non_exhaustive]
pub enum AssetKind {
    /// An executable artifact
    #[serde(rename = "executable")]
    Executable(ExecutableAsset),
    /// A README file
    #[serde(rename = "readme")]
    Readme,
    /// A LICENSE file
    #[serde(rename = "license")]
    License,
    /// A CHANGELOG or RELEASES file
    #[serde(rename = "changelog")]
    Changelog,
    /// Unknown to this version of cargo-dist-schema
    ///
    /// This is a fallback for forward/backward-compat
    #[serde(other)]
    #[serde(rename = "unknown")]
    Unknown,
}

/// A kind of Artifact
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind")]
#[non_exhaustive]
pub enum ArtifactKind {
    /// A zip or a tarball
    #[serde(rename = "executable-zip")]
    ExecutableZip,
    /// Standalone Symbols/Debuginfo for a build
    #[serde(rename = "symbols")]
    Symbols,
    /// Installer
    #[serde(rename = "installer")]
    Installer,
    /// A checksum of another artifact
    #[serde(rename = "checksum")]
    Checksum,
    /// Unknown to this version of cargo-dist-schema
    ///
    /// This is a fallback for forward/backward-compat
    #[serde(other)]
    #[serde(rename = "unknown")]
    Unknown,
}

/// An executable artifact (exe/binary)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExecutableAsset {
    /// The name of the Artifact containing symbols for this executable
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub symbols_artifact: Option<String>,
}

/// Info about a manifest version
pub struct VersionInfo {
    /// The version
    pub version: Version,
    /// The rough epoch of the format
    pub format: Format,
}

/// The current version of cargo-dist-schema
pub const SELF_VERSION: &str = env!("CARGO_PKG_VERSION");
/// The first epoch of cargo-dist, after this version a bunch of things changed
/// and we don't support that design anymore!
pub const DIST_EPOCH_1_MAX: &str = "0.0.3-prerelease8";
/// Second epoch of cargo-dist, after this we stopped putting versions in artifact ids.
/// This changes the download URL, but everything else works the same.
pub const DIST_EPOCH_2_MAX: &str = "0.0.6-prerelease6";

/// More coarse-grained version info, indicating periods when significant changes were made
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Format {
    /// THE BEFORE TIMES -- Unsupported
    Epoch1,
    /// First stable versions; during this epoch artifact names/ids contained their version numbers.
    Epoch2,
    /// Same as Epoch2, but now artifact names/ids don't include the version number,
    /// making /latest/ a stable path/url you can perma-link. This only affects download URLs.
    Epoch3,
    /// The version is newer than this version of cargo-dist-schema, so we don't know. Most
    /// likely it's compatible/readable, but maybe a breaking change was made?
    Future,
}

impl Format {
    /// Whether this format is too old to be supported
    pub fn unsupported(&self) -> bool {
        self <= &Format::Epoch1
    }
    /// Whether this format has version numbers in artifact names
    pub fn artifact_names_contain_versions(&self) -> bool {
        self <= &Format::Epoch2
    }
}

impl DistManifest {
    /// Create a new DistManifest
    pub fn new(releases: Vec<Release>, artifacts: BTreeMap<String, Artifact>) -> Self {
        Self {
            dist_version: None,
            announcement_tag: None,
            announcement_is_prerelease: false,
            announcement_title: None,
            announcement_changelog: None,
            announcement_github_body: None,
            system_info: None,
            releases,
            artifacts,
            publish_prereleases: false,
            ci: None,
            linkage: vec![],
        }
    }

    /// Get the JSON Schema for a DistManifest
    pub fn json_schema() -> schemars::schema::RootSchema {
        schemars::schema_for!(DistManifest)
    }

    /// Get the format of the manifest
    ///
    /// If anything goes wrong we'll default to Format::Future
    pub fn format(&self) -> Format {
        self.dist_version
            .as_ref()
            .and_then(|v| v.parse().ok())
            .map(|v| format_of_version(&v))
            .unwrap_or(Format::Future)
    }

    /// Convenience for iterating artifacts
    pub fn artifacts_for_release<'a>(
        &'a self,
        release: &'a Release,
    ) -> impl Iterator<Item = (&'a str, &'a Artifact)> {
        release
            .artifacts
            .iter()
            .filter_map(|k| Some((&**k, self.artifacts.get(k)?)))
    }

    /// Look up a release by its name
    pub fn release_by_name(&self, name: &str) -> Option<&Release> {
        self.releases.iter().find(|r| r.app_name == name)
    }

    /// Either get the release with the given name, or make a minimal one
    /// with no hosting/artifacts (to be populated)
    pub fn ensure_release(&mut self, name: String, version: String) -> &mut Release {
        // Written slightly awkwardly to make the borrowchecker happy :/
        if let Some(position) = self.releases.iter().position(|r| r.app_name == name) {
            &mut self.releases[position]
        } else {
            self.releases.push(Release {
                app_name: name,
                app_version: version,
                artifacts: vec![],
                hosting: Hosting::default(),
            });
            self.releases.last_mut().unwrap()
        }
    }
}

impl Release {
    /// Get the base URL that artifacts should be downloaded from (append the artifact name to the URL)
    pub fn artifact_download_url(&self) -> Option<&str> {
        self.hosting.artifact_download_url()
    }
}

/// Possible hosting providers
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub struct Hosting {
    /// Hosted on Github Releases
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github: Option<GithubHosting>,
    /// Hosted on Axo Releases
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axodotdev: Option<gazenot::ArtifactSet>,
}

/// Github Hosting
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct GithubHosting {
    /// The URL of the Github Release's artifact downloads
    pub artifact_download_url: String,
}

impl Hosting {
    /// Get the base URL that artifacts should be downloaded from (append the artifact name to the URL)
    pub fn artifact_download_url(&self) -> Option<&str> {
        let Hosting { axodotdev, github } = &self;
        // Prefer axodotdev is present, otherwise github
        if let Some(host) = &axodotdev {
            return host.set_download_url.as_deref();
        }
        if let Some(host) = &github {
            return Some(&host.artifact_download_url);
        }
        None
    }
    /// Gets whether there's no hosting
    pub fn is_empty(&self) -> bool {
        let Hosting { axodotdev, github } = &self;
        axodotdev.is_none() && github.is_none()
    }
}

/// Information about dynamic libraries used by a binary
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct Linkage {
    /// The filename of the binary
    pub binary: String,
    /// The target triple for which the binary was built
    pub target: String,
    /// Libraries included with the operating system
    pub system: Vec<Library>,
    /// Libraries provided by the Homebrew package manager
    pub homebrew: Vec<Library>,
    /// Public libraries not provided by the system and not managed by any package manager
    pub public_unmanaged: Vec<Library>,
    /// Libraries which don't fall into any other categories
    pub other: Vec<Library>,
    /// Frameworks, only used on macOS
    pub frameworks: Vec<Library>,
}

/// Represents a dynamic library located somewhere on the system
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct Library {
    /// The path to the library; on platforms without that information, it will be a basename instead
    pub path: String,
    /// The package from which a library comes, if relevant
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Helper to read the raw version from serialized json
fn dist_version(input: &str) -> Option<Version> {
    #[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
    struct PartialDistManifest {
        /// The version of cargo-dist that generated this
        #[serde(default)]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub dist_version: Option<String>,
    }

    let manifest: PartialDistManifest = serde_json::from_str(input).ok()?;
    let version: Version = manifest.dist_version?.parse().ok()?;
    Some(version)
}

/// Take serialized json and minimally parse out version info
pub fn check_version(input: &str) -> Option<VersionInfo> {
    let version = dist_version(input)?;
    let format = format_of_version(&version);
    Some(VersionInfo { version, format })
}

/// Get the format for a given version
pub fn format_of_version(version: &Version) -> Format {
    let epoch1 = Version::parse(DIST_EPOCH_1_MAX).unwrap();
    let epoch2 = Version::parse(DIST_EPOCH_2_MAX).unwrap();
    let self_ver = Version::parse(SELF_VERSION).unwrap();
    if version > &self_ver {
        Format::Future
    } else if version > &epoch2 {
        Format::Epoch3
    } else if version > &epoch1 {
        Format::Epoch2
    } else {
        Format::Epoch1
    }
}

#[test]
fn emit() {
    let schema = DistManifest::json_schema();
    let json_schema = serde_json::to_string_pretty(&schema).unwrap();
    insta::assert_snapshot!(json_schema);
}
