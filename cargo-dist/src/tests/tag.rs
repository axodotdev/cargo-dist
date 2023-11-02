//! Tests for announcement tag handling
//!
//! Note that some of these should_panics are negotiable, in the sense that we might
//! one day add support for these formats, "fixing" the test. That's good as long
//! as we intended to do that!

use super::mock::*;
use semver::Version;

use crate::announce::select_tag;
use crate::{config::ArtifactMode, DistGraphBuilder};

#[test]
fn parse_one() {
    // "1.0.0" in a one package workspace
    let workspace = workspace_just_axo();
    let version: Version = BIN_AXO_VER.parse().unwrap();
    let tag = format!("{version}");

    let tools = mock_tools();
    let graph = DistGraphBuilder::new(tools, &workspace, ArtifactMode::All, true).unwrap();
    let announcing = select_tag(&graph, Some(&tag), true).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.version, Some(version));
    assert_eq!(announcing.rust_releases, vec![entry_axo_bin()]);
}

#[test]
fn parse_one_v() {
    // "v1.0.0" in a one package workspace
    let workspace = workspace_just_axo();
    let version: Version = BIN_AXO_VER.parse().unwrap();
    let tag = format!("v{version}");

    let tools = mock_tools();
    let graph = DistGraphBuilder::new(tools, &workspace, ArtifactMode::All, true).unwrap();
    let announcing = select_tag(&graph, Some(&tag), true).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.version, Some(version));
    assert_eq!(announcing.rust_releases, vec![entry_axo_bin()]);
}

#[test]
fn parse_one_package_v() {
    // "axolotlsay-v1.0.0" in a one package workspace
    let workspace = workspace_just_axo();
    let version: Version = BIN_AXO_VER.parse().unwrap();
    let tag = format!("{BIN_AXO_NAME}-v{version}");

    let tools = mock_tools();
    let graph = DistGraphBuilder::new(tools, &workspace, ArtifactMode::All, true).unwrap();
    let announcing = select_tag(&graph, Some(&tag), true).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.version, None);
    assert_eq!(announcing.rust_releases, vec![entry_axo_bin()]);
}

#[test]
fn parse_one_package() {
    // "axolotlsay-1.0.0" in a one package workspace
    let workspace = workspace_just_axo();
    let version: Version = BIN_AXO_VER.parse().unwrap();
    let tag = format!("{BIN_AXO_NAME}-{version}");

    let tools = mock_tools();
    let graph = DistGraphBuilder::new(tools, &workspace, ArtifactMode::All, true).unwrap();
    let announcing = select_tag(&graph, Some(&tag), true).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.version, None);
    assert_eq!(announcing.rust_releases, vec![entry_axo_bin()]);
}

#[test]
fn parse_one_v_alpha() {
    // "v1.0.0-prerelease.1" in a one package workspace
    let workspace = workspace_just_axo_alpha();
    let version: Version = BIN_AXO_VER_ALPHA.parse().unwrap();
    let tag = format!("v{version}");

    let tools = mock_tools();
    let graph = DistGraphBuilder::new(tools, &workspace, ArtifactMode::All, true).unwrap();
    let announcing = select_tag(&graph, Some(&tag), true).unwrap();

    assert!(announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.version, Some(version));
    assert_eq!(announcing.rust_releases, vec![entry_axo_bin()]);
}

#[test]
fn parse_one_package_v_alpha() {
    // "axolotlsay-v1.0.0-prerelease.1" in a one package workspace
    let workspace = workspace_just_axo_alpha();
    let version: Version = BIN_AXO_VER_ALPHA.parse().unwrap();
    let tag = format!("{BIN_AXO_NAME}-v{version}");

    let tools = mock_tools();
    let graph = DistGraphBuilder::new(tools, &workspace, ArtifactMode::All, true).unwrap();
    let announcing = select_tag(&graph, Some(&tag), true).unwrap();

    assert!(announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.version, None);
    assert_eq!(announcing.rust_releases, vec![entry_axo_bin()]);
}

#[test]
fn parse_one_prefix_slashv() {
    // "release/v1.0.0" in a one package workspace
    let workspace = workspace_just_axo();
    let version: Version = BIN_AXO_VER.parse().unwrap();
    let tag = format!("release/v{version}");

    let tools = mock_tools();
    let graph = DistGraphBuilder::new(tools, &workspace, ArtifactMode::All, true).unwrap();
    let announcing = select_tag(&graph, Some(&tag), true).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.version, Some(version));
    assert_eq!(announcing.rust_releases, vec![entry_axo_bin()]);
}

#[test]
fn parse_one_prefix_slash() {
    // "release/1.0.0" in a one package workspace
    let workspace = workspace_just_axo();
    let version: Version = BIN_AXO_VER.parse().unwrap();
    let tag = format!("release/{version}");

    let tools = mock_tools();
    let graph = DistGraphBuilder::new(tools, &workspace, ArtifactMode::All, true).unwrap();
    let announcing = select_tag(&graph, Some(&tag), true).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.version, Some(version));
    assert_eq!(announcing.rust_releases, vec![entry_axo_bin()]);
}

#[test]
fn parse_one_prefix_slash_package_v() {
    // "release/axolotlsay-v1.0.0" in a one package workspace
    let workspace = workspace_just_axo();
    let version: Version = BIN_AXO_VER.parse().unwrap();
    let tag = format!("release/{BIN_AXO_NAME}-v{version}");

    let tools = mock_tools();
    let graph = DistGraphBuilder::new(tools, &workspace, ArtifactMode::All, true).unwrap();
    let announcing = select_tag(&graph, Some(&tag), true).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.version, None);
    assert_eq!(announcing.rust_releases, vec![entry_axo_bin()]);
}

#[test]
fn parse_one_prefix_slash_package_slashv() {
    // "releases/axolotlsay/v1.0.0" in a one package workspace
    let workspace = workspace_just_axo();
    let version: Version = BIN_AXO_VER.parse().unwrap();
    let tag = format!("releases/{BIN_AXO_NAME}/v{version}");

    let tools = mock_tools();
    let graph = DistGraphBuilder::new(tools, &workspace, ArtifactMode::All, true).unwrap();
    let announcing = select_tag(&graph, Some(&tag), true).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.version, None);
    assert_eq!(announcing.rust_releases, vec![entry_axo_bin()]);
}

#[test]
fn parse_one_package_slashv() {
    // "releases/axolotlsay/v1.0.0" in a one package workspace
    let workspace = workspace_just_axo();
    let version: Version = BIN_AXO_VER.parse().unwrap();
    let tag = format!("{BIN_AXO_NAME}/v{version}");

    let tools = mock_tools();
    let graph = DistGraphBuilder::new(tools, &workspace, ArtifactMode::All, true).unwrap();
    let announcing = select_tag(&graph, Some(&tag), true).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.version, None);
    assert_eq!(announcing.rust_releases, vec![entry_axo_bin()]);
}

#[test]
fn parse_one_prefix_slash_package_slash() {
    // "releases/axolotlsay/v1.0.0" in a one package workspace
    let workspace = workspace_just_axo();
    let version: Version = BIN_AXO_VER.parse().unwrap();
    let tag = format!("releases/{BIN_AXO_NAME}/{version}");

    let tools = mock_tools();
    let graph = DistGraphBuilder::new(tools, &workspace, ArtifactMode::All, true).unwrap();
    let announcing = select_tag(&graph, Some(&tag), true).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.version, None);
    assert_eq!(announcing.rust_releases, vec![entry_axo_bin()]);
}

#[test]
fn parse_one_prefix_many_slash_package_slash() {
    // "releases/axolotlsay/v1.0.0" in a one package workspace
    let workspace = workspace_just_axo();
    let version: Version = BIN_AXO_VER.parse().unwrap();
    let tag = format!("blah/blah/releases/{BIN_AXO_NAME}/{version}");

    let tools = mock_tools();
    let graph = DistGraphBuilder::new(tools, &workspace, ArtifactMode::All, true).unwrap();
    let announcing = select_tag(&graph, Some(&tag), true).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.version, None);
    assert_eq!(announcing.rust_releases, vec![entry_axo_bin()]);
}

#[test]
fn parse_one_package_slash() {
    // "releases/axolotlsay/v1.0.0" in a one package workspace
    let workspace = workspace_just_axo();
    let version: Version = BIN_AXO_VER.parse().unwrap();
    let tag = format!("{BIN_AXO_NAME}/{version}");

    let tools = mock_tools();
    let graph = DistGraphBuilder::new(tools, &workspace, ArtifactMode::All, true).unwrap();
    let announcing = select_tag(&graph, Some(&tag), true).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.version, None);
    assert_eq!(announcing.rust_releases, vec![entry_axo_bin()]);
}

#[test]
fn parse_one_infer() {
    // Provide no explicit tag in a one package workspace
    let workspace = workspace_just_axo();
    let version: Version = BIN_AXO_VER.parse().unwrap();
    let tag = format!("v{version}");

    let tools = mock_tools();
    let graph = DistGraphBuilder::new(tools, &workspace, ArtifactMode::All, true).unwrap();
    let announcing = select_tag(&graph, None, true).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.version, Some(version));
    assert_eq!(announcing.rust_releases, vec![entry_axo_bin()]);
}

#[test]
fn parse_unified_v() {
    // "v1.0.0" in a unified workspace
    let workspace = workspace_unified();
    let version: Version = BIN_AXO_VER.parse().unwrap();
    let tag = format!("v{version}");

    let tools = mock_tools();
    let graph = DistGraphBuilder::new(tools, &workspace, ArtifactMode::All, true).unwrap();
    let announcing = select_tag(&graph, Some(&tag), true).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.version, Some(version));
    assert_eq!(
        announcing.rust_releases,
        vec![entry_axo_bin(), entry_helper_bin()]
    );
}

#[test]
fn parse_unified_infer() {
    // Provide no explicit tag in a unified workspace
    let workspace = workspace_unified();
    let version: Version = BIN_AXO_VER.parse().unwrap();
    let tag = format!("v{version}");

    let tools = mock_tools();
    let graph = DistGraphBuilder::new(tools, &workspace, ArtifactMode::All, true).unwrap();
    let announcing = select_tag(&graph, None, true).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.version, Some(version));
    assert_eq!(
        announcing.rust_releases,
        vec![entry_axo_bin(), entry_helper_bin()]
    );
}

#[test]
fn parse_unified_lib() {
    // trying to explicitly publish a library in a unified workspace
    let workspace = workspace_unified();
    let version: Version = LIB_SOME_VER.parse().unwrap();
    let tag = format!("{LIB_SOME_NAME}-v{version}");

    let tools = mock_tools();
    let graph = DistGraphBuilder::new(tools, &workspace, ArtifactMode::All, true).unwrap();
    let announcing = select_tag(&graph, Some(&tag), true).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.version, None);
    assert_eq!(announcing.rust_releases, vec![]);
}

#[test]
fn parse_disjoint_v() {
    // selecting the bulk packages in a disjoint workspace
    let workspace = workspace_disjoint();
    let version: Version = BIN_AXO_VER.parse().unwrap();
    let tag = format!("v{version}");

    let tools = mock_tools();
    let graph = DistGraphBuilder::new(tools, &workspace, ArtifactMode::All, true).unwrap();
    let announcing = select_tag(&graph, Some(&tag), true).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.version, Some(version));
    assert_eq!(
        announcing.rust_releases,
        vec![entry_axo_bin(), entry_helper_bin(), entry_forced_bin()]
    );
}

#[test]
#[should_panic = "TooManyUnrelatedApps"]
fn parse_disjoint_infer() {
    // Provide no explicit tag in a disjoint workspace (SHOULD FAIL!)
    let workspace = workspace_disjoint();
    let version: Version = BIN_AXO_VER.parse().unwrap();
    let tag = format!("v{version}");

    let tools = mock_tools();
    let graph = DistGraphBuilder::new(tools, &workspace, ArtifactMode::All, true).unwrap();
    let announcing = select_tag(&graph, None, true).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.version, Some(version));
    assert_eq!(
        announcing.rust_releases,
        vec![entry_axo_bin(), entry_helper_bin(), entry_forced_bin()]
    );
}

#[test]
fn parse_disjoint_v_oddball() {
    // selecting the oddball package in a disjoint workspace
    let workspace = workspace_disjoint();
    let version: Version = BIN_ODDBALL_VER.parse().unwrap();
    let tag = format!("v{version}");

    let tools = mock_tools();
    let graph = DistGraphBuilder::new(tools, &workspace, ArtifactMode::All, true).unwrap();
    let announcing = select_tag(&graph, Some(&tag), true).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.version, Some(version));
    assert_eq!(announcing.rust_releases, vec![entry_oddball_bin()]);
}

#[test]
fn parse_disjoint_lib() {
    // trying to explicitly publish a library in a disjoint workspace
    let workspace = workspace_disjoint();
    let version: Version = LIB_OTHER_VER.parse().unwrap();
    let tag = format!("{LIB_OTHER_NAME}-v{version}");

    let tools = mock_tools();
    let graph = DistGraphBuilder::new(tools, &workspace, ArtifactMode::All, true).unwrap();
    let announcing = select_tag(&graph, Some(&tag), true).unwrap();

    assert!(!announcing.prerelease);
    assert_eq!(announcing.tag, tag);
    assert_eq!(announcing.version, None);
    assert_eq!(announcing.rust_releases, vec![]);
}
