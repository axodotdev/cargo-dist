#![deny(missing_docs)]

//! # cargo-dist-schema
//!
//! This crate exists to serialize and deserialize the dist-manifest.json produced
//! by cargo-dist. Ideally it should be reasonably forward and backward compatible
//! with different versions of this format.
//!
//! The root type of the schema is [`DistManifest`][].

use schemars::JsonSchema;
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

/// A report of the releases and artifacts that cargo-dist generated
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DistManifest {
    /// The version of cargo-dist that generated this
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dist_version: Option<String>,
    /// App releases we're distributing
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub releases: Vec<Release>,
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
    pub artifacts: Vec<Artifact>,
    /// The title of the changelog for this release
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changelog_title: Option<String>,
    /// The body of the changelog for this release
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changelog_body: Option<String>,
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
    /// The kind of artifact this is (e.g. "exectuable-zip")
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
    /// Machine-readable metadata
    #[serde(rename = "dist-metadata")]
    DistMetadata,
    /// Installer
    #[serde(rename = "installer")]
    Installer,
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

impl DistManifest {
    /// Create a new DistManifest
    pub fn new(dist_version: String, releases: Vec<Release>) -> Self {
        Self {
            dist_version: Some(dist_version),
            releases,
        }
    }

    /// Get the JSON Schema for a DistManifest
    pub fn json_schema() -> schemars::schema::RootSchema {
        schemars::schema_for!(DistManifest)
    }
}

#[test]
fn emit() {
    use std::fs::File;
    use std::io::BufWriter;
    use std::io::Write;
    use std::path::PathBuf;

    let schema = DistManifest::json_schema();
    let json_schema = serde_json::to_string_pretty(&schema).unwrap();
    insta::assert_snapshot!(json_schema);

    // FIXME: (?) we should use something like xtask to update the schema, but this works ok.
    let root = std::env!("CARGO_MANIFEST_DIR");
    let schema = PathBuf::from(root).join("cargo-dist-json-schema.json");
    let file = File::options()
        .create(true)
        .write(true)
        .truncate(true)
        .open(schema)
        .unwrap();
    let mut file = BufWriter::new(file);
    writeln!(&mut file, "{json_schema}").unwrap();
}
