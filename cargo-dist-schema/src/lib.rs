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
    /// The distributable bundles for this release
    pub distributables: Vec<Distributable>,
}

/// A distributable bundle that's part of a Release
///
/// i.e. a zip or installer
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Distributable {
    /// The kind of distributable (e.g. "zip")
    pub kind: DistributableKind,
    /// The target triple of the bundle
    pub target_triple: String,
    /// The location of the distributable bundle
    pub path: PathBuf,
    /// Artifacts included in the bundle (like executables)
    pub artifacts: Vec<Artifact>,
}

/// An artifact included in a Distributable
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind")]
pub enum Artifact {
    /// An executable artifact
    #[serde(rename = "executable")]
    Executable(ExecutableArtifact),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum DistributableKind {
    /// A zip or a tarball
    #[serde(rename = "zip")]
    Zip,
}

/// An executable artifact (exe/binary)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExecutableArtifact {
    /// The name of the executable
    pub name: String,
    /// The path of the executable relative to the root of the package
    pub path: PathBuf,
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
