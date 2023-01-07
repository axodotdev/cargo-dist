use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
// FIXME: ideally these would be UTF8PathBufs but JsonSchema doesn't support (yet?)
use std::path::PathBuf;

/// The final report of cargo-dist
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DistReport {
    /// App releases we're distributing
    pub releases: Vec<Release>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Release {
    /// The name of the app
    pub app_name: String,
    /// The version of the app
    // FIXME: should be a Version but JsonSchema doesn't support (yet?)
    pub app_version: String,
    /// The artifacts for this release (zips, debuginfo, metadata...)
    pub artifacts: Vec<Artifact>,
}

/// A distributable bundle that's part of a Release
///
/// i.e. a zip or installer
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Artifact {
    /// The unique name of the artifact (e.g. `myapp-v1.0.0-x86_64-pc-windows-msvc.zip`)
    pub name: String,
    /// The kind of artifact this is (e.g. "exectuable-zip")
    #[serde(flatten)]
    pub kind: ArtifactKind,
    /// The target triple of the bundle
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_triple: Option<String>,
    /// The location of the artifact on the local system
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    /// Assets included in the bundle (like executables)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<Asset>,
}

/// An asset contained in an artifact (executable, license, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Asset {
    /// The high-level name of the asset
    pub name: String,
    /// The path of the asset relative to the root of the artifact
    pub path: PathBuf,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind")]
pub enum ArtifactKind {
    /// A zip or a tarball
    #[serde(rename = "executable-zip")]
    ExecutableZip,
    /// Standalone Symbols/Debuginfo for a build
    #[serde(rename = "symbols")]
    Symbols,
    /// Machine-readable metadata
    #[serde(rename = "dist-metadata")]
    DistMetadata,
}

/// An executable artifact (exe/binary)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExecutableAsset {
    /// The name of the Artifact containing symbols for this executable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbols_artifact: Option<String>,
}

impl DistReport {
    pub fn new(releases: Vec<Release>) -> Self {
        Self { releases }
    }
}

#[test]
fn emit() {
    let schema = schemars::schema_for!(DistReport);
    insta::assert_snapshot!(serde_json::to_string_pretty(&schema).unwrap());
}
