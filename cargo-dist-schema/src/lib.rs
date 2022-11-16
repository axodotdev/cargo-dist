use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A manifest for a built package
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Manifest {
    /// The name of the package
    name: String,
    /// The location of the package
    path: PathBuf,
    /// The version of the package
    // FIXME: should be a Version but JsonSchema doesn't support (yet?)
    version: String,
    /// The target triple of the package
    #[serde(default)]
    target: String,
    /// The binaries the package contains
    binaries: Vec<Binary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Binary {
    /// The name of the binary
    name: String,
    /// The path of the binary relative to the root of the package
    // FIXME: should be a camino::Utf8PathBuf but JsonSchema doesn't support
    path: PathBuf,
}

#[test]
fn emit() {
    let schema = schemars::schema_for!(Manifest);
    insta::assert_snapshot!(serde_json::to_string_pretty(&schema).unwrap());
}
