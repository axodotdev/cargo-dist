use crate::WorkspaceKind;

#[cfg(feature = "cargo-projects")]
#[test]
fn test_self_detect() {
    let project = crate::get_project("./".into()).unwrap();
    assert_eq!(project.kind, WorkspaceKind::Rust);
    assert_eq!(project.package_info.len(), 1);

    let package = &project.package_info[0];
    assert_eq!(package.name, "axoproject");
    assert_eq!(package.binaries.len(), 1);

    let binary = &package.binaries[0];
    assert_eq!(binary, "axoproject");
}

#[cfg(feature = "cargo-projects")]
#[test]
fn test_cargo_new() {
    let project = crate::get_project("tests/projects/cargo-new/src/".into()).unwrap();
    assert_eq!(project.kind, WorkspaceKind::Rust);
    assert_eq!(project.package_info.len(), 1);

    let package = &project.package_info[0];
    assert_eq!(package.name, "cargo-new");
    assert_eq!(package.binaries.len(), 1);

    let binary = &package.binaries[0];
    assert_eq!(binary, "cargo-new");
}

#[cfg(feature = "npm-projects")]
#[test]
fn test_npm_init_legacy() {
    let project = crate::get_project("tests/projects/npm-init-legacy".into()).unwrap();
    assert_eq!(project.kind, WorkspaceKind::Javascript);
    assert_eq!(project.package_info.len(), 1);

    let package = &project.package_info[0];
    assert_eq!(package.name, "npm-init-legacy");
    assert_eq!(package.binaries.len(), 0);
}

#[cfg(feature = "npm-projects")]
#[test]
fn test_npm_create_react_app() {
    let project = crate::get_project("tests/projects/npm-create-react-app/src/".into()).unwrap();
    assert_eq!(project.kind, WorkspaceKind::Javascript);
    assert_eq!(project.package_info.len(), 1);

    let package = &project.package_info[0];
    assert_eq!(package.name, "npm-create-react-app");
    assert_eq!(package.binaries.len(), 0);
}
