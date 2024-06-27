//! Mock testing utils, mostly you want the `workspace_*` functions,
//! but other functions/consts will help you assert the results

use crate::{CargoInfo, Tools};
use axoproject::{AutoIncludes, PackageIdx, PackageInfo, WorkspaceGraph, WorkspaceInfo};
use serde_json::json;

pub const REPO_URL: &str = "https://github.com/axodotdev/axolotlsay";
pub const REPO_DESC: &str = "💬 a CLI for learning to distribute CLIs in rust";

pub const BIN_AXO_NAME: &str = "axolotlsay";
pub const BIN_AXO_VER: &str = "1.0.0";
pub const BIN_AXO_VER_ALPHA: &str = "1.0.0-prerelease.1";
pub const BIN_AXO_IDX: PackageIdx = PackageIdx(0);

pub const LIB_SOME_NAME: &str = "some-lib";
pub const LIB_SOME_VER: &str = BIN_AXO_VER;
pub const LIB_SOME_IDX: PackageIdx = PackageIdx(1);

pub const BIN_HELPER_NAME: &str = "helper-bin";
pub const BIN_HELPER_NAME2: &str = "helper-bin-utils";
pub const BIN_HELPER_VER: &str = BIN_AXO_VER;
pub const BIN_HELPER_IDX: PackageIdx = PackageIdx(2);

pub const LIB_OTHER_NAME: &str = "other-lib";
pub const LIB_OTHER_VER: &str = "0.5.0";
pub const LIB_OTHER_IDX: PackageIdx = PackageIdx(3);

pub const BIN_ODDBALL_NAME: &str = "oddball-bin";
pub const BIN_ODDBALL_VER: &str = "0.1.0";
pub const BIN_ODDBALL_IDX: PackageIdx = PackageIdx(4);

pub const BIN_FORCED_NAME: &str = "forced-bin";
pub const BIN_FORCED_VER: &str = BIN_AXO_VER;
pub const BIN_FORCED_IDX: PackageIdx = PackageIdx(5);

pub const BIN_TEST1_NAME: &str = "test-bin1";
pub const BIN_TEST1_VER: &str = BIN_AXO_VER;

pub const BIN_TEST2_NAME: &str = "test-bin2";
pub const BIN_TEST2_VER: &str = BIN_AXO_VER;

pub fn mock_tools() -> Tools {
    Tools {
        cargo: CargoInfo {
            cmd: String::new(),
            version_line: None,
            host_target: "x86_64-unknown-linux-gnu".to_owned(),
        },
        rustup: None,
        brew: None,
        git: None,
        code_sign_tool: None,
    }
}

pub fn mock_package(name: &str, ver: &str) -> PackageInfo {
    PackageInfo {
        name: name.to_owned(),
        version: Some(axoproject::Version::Cargo(ver.parse().unwrap())),
        manifest_path: Default::default(),
        package_root: Default::default(),
        description: Some(REPO_DESC.to_owned()),
        authors: vec![],
        license: None,
        publish: true,
        keywords: None,
        repository_url: Some(REPO_URL.to_owned()),
        homepage_url: None,
        documentation_url: None,
        readme_file: None,
        license_files: vec![],
        changelog_file: None,
        binaries: vec![],
        cstaticlibs: vec![],
        cdylibs: vec![],
        cargo_metadata_table: None,
        cargo_package_id: None,
        build_command: None,
    }
}

pub fn mock_workspace(packages: Vec<PackageInfo>) -> WorkspaceGraph {
    let mut workspaces = WorkspaceGraph::default();
    let workspace = WorkspaceInfo {
        kind: axoproject::WorkspaceKind::Rust,
        target_dir: Default::default(),
        workspace_dir: Default::default(),
        _sub_workspaces: vec![],
        _package_info: packages,
        manifest_path: Default::default(),
        root_auto_includes: AutoIncludes {
            readme: None,
            licenses: vec![],
            changelog: None,
        },
        warnings: vec![],
        cargo_metadata_table: None,
        cargo_profiles: Default::default(),
    };
    workspaces.add_workspace(workspace, None);
    workspaces
}

/// axolotlsay 1.0.0
pub fn pkg_axo_bin() -> PackageInfo {
    PackageInfo {
        binaries: vec![BIN_AXO_NAME.to_owned()],
        ..mock_package(BIN_AXO_NAME, BIN_AXO_VER)
    }
}
/// axolotlsay 1.0.0-prerelease.1
pub fn pkg_axo_bin_alpha() -> PackageInfo {
    PackageInfo {
        binaries: vec![BIN_AXO_NAME.to_owned()],
        ..mock_package(BIN_AXO_NAME, BIN_AXO_VER_ALPHA)
    }
}
pub fn entry_axo_bin() -> (PackageIdx, Vec<String>) {
    (BIN_AXO_IDX, vec![BIN_AXO_NAME.to_owned()])
}

/// some-lib 1.0.0
pub fn pkg_some_lib() -> PackageInfo {
    PackageInfo {
        ..mock_package(LIB_SOME_NAME, LIB_SOME_VER)
    }
}
pub fn entry_some_lib() -> (PackageIdx, Vec<String>) {
    (LIB_SOME_IDX, vec![])
}

/// helper-bin 1.0.0 (has 2 binaries)
pub fn pkg_helper_bin() -> PackageInfo {
    PackageInfo {
        binaries: vec![BIN_HELPER_NAME.to_owned(), BIN_HELPER_NAME2.to_owned()],
        ..mock_package(BIN_HELPER_NAME, BIN_HELPER_VER)
    }
}
pub fn entry_helper_bin() -> (PackageIdx, Vec<String>) {
    (
        BIN_HELPER_IDX,
        vec![BIN_HELPER_NAME.to_owned(), BIN_HELPER_NAME2.to_owned()],
    )
}

/// other-lib 0.5.0 (non-harmonious version)
pub fn pkg_other_lib() -> PackageInfo {
    PackageInfo {
        ..mock_package(LIB_OTHER_NAME, LIB_OTHER_VER)
    }
}
pub fn entry_other_lib() -> (PackageIdx, Vec<String>) {
    (LIB_OTHER_IDX, vec![])
}

/// oddball-bin 0.1.0 (non-harmonious version)
pub fn pkg_oddball_bin() -> PackageInfo {
    PackageInfo {
        binaries: vec![BIN_ODDBALL_NAME.to_owned()],
        ..mock_package(BIN_ODDBALL_NAME, BIN_ODDBALL_VER)
    }
}
pub fn entry_oddball_bin() -> (PackageIdx, Vec<String>) {
    (BIN_ODDBALL_IDX, vec![BIN_ODDBALL_NAME.to_owned()])
}

/// forced-bin 1.0.0
///
/// has publish=false and dist=true set
pub fn pkg_forced_bin() -> PackageInfo {
    PackageInfo {
        publish: false,
        cargo_metadata_table: Some(json!({
            "dist": {
                "dist": true
            }
        })),
        binaries: vec![BIN_FORCED_NAME.to_owned()],
        ..mock_package(BIN_FORCED_NAME, BIN_FORCED_VER)
    }
}
pub fn entry_forced_bin() -> (PackageIdx, Vec<String>) {
    (BIN_FORCED_IDX, vec![BIN_FORCED_NAME.to_owned()])
}

/// test-bin1 1.0.0
///
/// has publish=false set
pub fn pkg_test_bin1() -> PackageInfo {
    PackageInfo {
        publish: false,
        binaries: vec![BIN_TEST1_NAME.to_owned()],
        ..mock_package(BIN_TEST1_NAME, BIN_TEST1_VER)
    }
}
/// test-bin2 1.0.0
///
/// has dist=false set
pub fn pkg_test_bin2() -> PackageInfo {
    PackageInfo {
        cargo_metadata_table: Some(json!({
            "dist": {
                "dist": false
            }
        })),
        binaries: vec![BIN_TEST2_NAME.to_owned()],
        ..mock_package(BIN_TEST2_NAME, BIN_TEST2_VER)
    }
}
/// axolotlsay
pub fn workspace_just_axo() -> WorkspaceGraph {
    mock_workspace(vec![pkg_axo_bin()])
}

/// axolotlsay (alpha release)
pub fn workspace_just_axo_alpha() -> WorkspaceGraph {
    mock_workspace(vec![pkg_axo_bin_alpha()])
}

/// axolotlsay, some-lib, helper-bin -- all same version
pub fn workspace_unified() -> WorkspaceGraph {
    mock_workspace(vec![pkg_axo_bin(), pkg_some_lib(), pkg_helper_bin()])
}

/// axolotlsay, some-lib, helper-bin, other-lib, oddball-bin, forced-bin, test-bin1, test-bin2
///
/// oddball-bin and other-lib have different version
/// forced-bin has publish=false AND dist=true, so should be included
/// test-bin1 has publish=false, so should be ignored
/// test-bin2 has dist=false, so should be ignored
pub fn workspace_disjoint() -> WorkspaceGraph {
    // axolotlsay, a lib
    mock_workspace(vec![
        pkg_axo_bin(),
        pkg_some_lib(),
        pkg_helper_bin(),
        pkg_other_lib(),
        pkg_oddball_bin(),
        pkg_forced_bin(),
        pkg_test_bin1(),
        pkg_test_bin2(),
    ])
}
