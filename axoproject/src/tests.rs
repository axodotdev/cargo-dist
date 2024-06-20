use camino::{Utf8Path, Utf8PathBuf};

use crate::{
    changelog::ChangelogInfo, errors::AxoprojectError, PackageIdx, Version, WorkspaceGraph,
    WorkspaceKind,
};

#[cfg(feature = "cargo-projects")]
#[test]
fn test_self_detect() {
    let project = crate::get_workspaces("./".into(), None).best().unwrap();
    assert_eq!(project.kind, WorkspaceKind::Rust);
    assert_eq!(project._package_info.len(), 3);

    let package = &project._package_info[0];
    assert_eq!(package.name, "axoproject");
    assert_eq!(package.binaries.len(), 0);
}

#[cfg(feature = "cargo-projects")]
#[test]
fn test_cargo_new() {
    let project = crate::get_workspaces("tests/projects/cargo-new/src/".into(), None)
        .best()
        .unwrap();
    assert_eq!(project.kind, WorkspaceKind::Rust);
    assert_eq!(project._package_info.len(), 1);

    let package = &project._package_info[0];
    assert_eq!(package.name, "cargo-new");
    assert_eq!(package.binaries.len(), 1);

    let binary = &package.binaries[0];
    assert_eq!(binary, "cargo-new");
}

#[cfg(feature = "cargo-projects")]
#[test]
fn test_rooted_cargo_wrong() {
    use camino::Utf8PathBuf;

    let project = crate::get_workspaces(
        "tests/projects/cargo-new/src/".into(),
        Some(Utf8PathBuf::from("src/")).as_deref(),
    )
    .best();
    assert!(project.is_none());
}

#[cfg(feature = "cargo-projects")]
#[test]
fn test_rooted_cargo_clamped_too_soon() {
    use camino::Utf8PathBuf;

    let project = crate::get_workspaces(
        "tests/projects/cargo-new/src/".into(),
        Some(Utf8PathBuf::from("tests/projects/cargo-new/src/")).as_deref(),
    )
    .best();
    assert!(project.is_none());
}

#[cfg(feature = "cargo-projects")]
#[test]
fn test_rooted_cargo_good() {
    use camino::Utf8PathBuf;

    let project = crate::get_workspaces(
        "tests/projects/cargo-new/src/".into(),
        Some(Utf8PathBuf::from("tests/projects/cargo-new/")).as_deref(),
    )
    .best()
    .unwrap();
    assert_eq!(project.kind, WorkspaceKind::Rust);
    assert_eq!(project._package_info.len(), 1);

    let package = &project._package_info[0];
    assert_eq!(package.name, "cargo-new");
    assert_eq!(package.binaries.len(), 1);

    let binary = &package.binaries[0];
    assert_eq!(binary, "cargo-new");
}

#[cfg(feature = "cargo-projects")]
#[test]
fn test_cargo_virtual() {
    let project = crate::get_workspaces("tests/projects/cargo-virtual/virtual/".into(), None)
        .best()
        .unwrap();
    assert_eq!(project.kind, WorkspaceKind::Rust);
    assert_eq!(project._package_info.len(), 3);

    {
        let package = &project._package_info[0];
        assert_eq!(package.name, "virtual");
        assert_eq!(&package.binaries[..], &["virtual"]);
    }

    {
        let package = &project._package_info[1];
        assert_eq!(package.name, "some-lib");
        assert!(package.binaries.is_empty());
    }

    {
        let package = &project._package_info[2];
        assert_eq!(package.name, "virtual-gui");
        assert_eq!(&package.binaries[..], &["virtual-gui"]);
    }
}

#[cfg(feature = "cargo-projects")]
#[test]
fn test_cargo_nonvirtual() {
    let project = crate::get_workspaces("tests/projects/cargo-nonvirtual/".into(), None)
        .best()
        .unwrap();
    assert_eq!(project.kind, WorkspaceKind::Rust);
    assert_eq!(project._package_info.len(), 6);

    {
        let package = &project._package_info[0];
        assert_eq!(package.name, "some-cdylib");
        assert!(package.binaries.is_empty());
    }

    {
        let package = &project._package_info[1];
        assert_eq!(package.name, "some-lib");
        assert!(package.binaries.is_empty());
    }

    {
        let package = &project._package_info[2];
        assert_eq!(package.name, "some-other-lib");
        assert!(package.binaries.is_empty());
    }

    {
        let package = &project._package_info[3];
        assert_eq!(package.name, "some-staticlib");
        assert!(package.binaries.is_empty());
    }

    {
        let package = &project._package_info[4];
        assert_eq!(package.name, "test-bin");
        assert_eq!(&package.binaries[..], &["test-bin"]);
        assert!(!package.publish);
    }

    {
        let package = &project._package_info[5];
        assert_eq!(package.name, "nonvirtual");
        assert_eq!(&package.binaries[..], &["cargo-nonvirtual", "nonvirtual"]);
        assert!(package.publish);
    }
}

#[cfg(feature = "npm-projects")]
#[test]
fn test_npm_init_legacy() {
    let project = crate::get_workspaces("tests/projects/npm-init-legacy".into(), None)
        .best()
        .unwrap();
    assert_eq!(project.kind, WorkspaceKind::Javascript);
    assert_eq!(project.package_info.len(), 1);

    let package = &project.package_info[0];
    assert_eq!(package.name, "npm-init-legacy");

    // NOTE: we provide a path for this binary that doesn't exist, so if we
    // get more rigorous this test will fail. That's fine, the test can be
    // updated. Oranda has similar tests.
    assert_eq!(package.binaries.len(), 1);
    let binary = &package.binaries[0];
    assert_eq!(binary, "npm-init-legacy");
}

#[cfg(feature = "npm-projects")]
#[test]
fn test_npm_create_react_app() {
    let project = crate::get_workspaces("tests/projects/npm-create-react-app/src/".into(), None)
        .best()
        .unwrap();
    assert_eq!(project.kind, WorkspaceKind::Javascript);
    assert_eq!(project.package_info.len(), 1);

    let package = &project.package_info[0];
    assert_eq!(package.name, "npm-create-react-app");

    // NOTE: we provide paths that exist here, but they're not proper binaries, so if we
    // get more rigorous this test will fail. That's fine, the test can be
    // updated. Oranda has similar tests.
    assert_eq!(package.binaries.len(), 2);

    let binary = &package.binaries[0];
    assert_eq!(binary, "App.js");

    let binary = &package.binaries[1];
    assert_eq!(binary, "index.js");
}

#[cfg(feature = "npm-projects")]
#[test]
fn test_rooted_npm_wrong() {
    use camino::Utf8PathBuf;

    let project = crate::get_workspaces(
        "tests/projects/npm-init-legacy/".into(),
        Some(Utf8PathBuf::from("src/")).as_deref(),
    )
    .best();
    assert!(project.is_none());
}

#[cfg(feature = "npm-projects")]
#[test]
fn test_rooted_npm_clamped_too_soon() {
    use camino::Utf8PathBuf;

    let project = crate::get_workspaces(
        "tests/projects/npm-init-legacy/src/".into(),
        Some(Utf8PathBuf::from("tests/projects/npm-init-legacy/src/")).as_deref(),
    )
    .best();
    assert!(project.is_none());
}

#[cfg(feature = "npm-projects")]
#[test]
fn test_rooted_npm_good() {
    use camino::Utf8PathBuf;

    let project = crate::get_workspaces(
        "tests/projects/npm-init-legacy/src/".into(),
        Some(Utf8PathBuf::from("tests/projects/npm-init-legacy/")).as_deref(),
    )
    .best()
    .unwrap();
    assert_eq!(project.kind, WorkspaceKind::Javascript);
    assert_eq!(project.package_info.len(), 1);

    let package = &project.package_info[0];
    assert_eq!(package.name, "npm-init-legacy");

    // NOTE: we provide a path for this binary that doesn't exist, so if we
    // get more rigorous this test will fail. That's fine, the test can be
    // updated. Oranda has similar tests.
    assert_eq!(package.binaries.len(), 1);
    let binary = &package.binaries[0];
    assert_eq!(binary, "npm-init-legacy");
}

fn kitchen_sink_changelog() -> &'static str {
    r####"
# Changelog

## Unreleased

Coming soon..!


## v3.2.5 - [CHANGEGER](https://github.com/axodotdev/fakesite)

Hope the title link also got stripped....!!!

## [3.2.3 - NEXT CHANGERATIONS](https://github.com/axodotdev/fakesite)

Hope the title link got stripped..!

## 3.2.1 - THE FINAL CHANGETIER

WOW!


## 3.2.0

Great changelog here


## v1.2.1 the BEST version

WOW CHANGLOGS!!


## v1.2.0

changelog here




## Version 1.0.1 - July 3rd, 2025

And THAT's

THE

FACTS



## Version 1.0.0

I'm changelogin' here!



## Version 0.1.0-prerelease.1+buildgunk - neato!

Wow what a first release

#### Features

some features!


"####
}

fn no_unreleased_changelog() -> &'static str {
    r##"
# v1.0.0

neat
"##
}

fn doubled_changelog() -> &'static str {
    r##"
# v1.0.0

neat

# v1.0.0

still neat
"##
}

fn ver(ver: &str) -> Version {
    Version::Cargo(ver.parse().unwrap())
}

#[test]
fn test_changelog_basic() {
    use crate::changelog::changelog_for_version_inner as test;
    let text = kitchen_sink_changelog();
    let path = Utf8PathBuf::from("CHANGELOG.md");

    // Test exact matches
    assert_eq!(
        test(&path, text, &ver("0.1.0-prerelease.1+buildgunk"))
            .unwrap()
            .unwrap(),
        ChangelogInfo {
            title: "Version 0.1.0-prerelease.1+buildgunk - neato!".to_owned(),
            body: "Wow what a first release\n\n#### Features\n\nsome features!".to_owned()
        }
    );
    assert_eq!(
        test(&path, text, &ver("1.0.0")).unwrap().unwrap(),
        ChangelogInfo {
            title: "Version 1.0.0".to_owned(),
            body: "I'm changelogin' here!".to_owned()
        }
    );
    assert_eq!(
        test(&path, text, &ver("1.0.1")).unwrap().unwrap(),
        ChangelogInfo {
            title: "Version 1.0.1 - July 3rd, 2025".to_owned(),
            body: "And THAT's\n\nTHE\n\nFACTS".to_owned()
        }
    );
    assert_eq!(
        test(&path, text, &ver("1.2.0")).unwrap().unwrap(),
        ChangelogInfo {
            title: "v1.2.0".to_owned(),
            body: "changelog here".to_owned()
        }
    );
    assert_eq!(
        test(&path, text, &ver("1.2.1")).unwrap().unwrap(),
        ChangelogInfo {
            title: "v1.2.1 the BEST version".to_owned(),
            body: "WOW CHANGLOGS!!".to_owned()
        }
    );
    assert_eq!(
        test(&path, text, &ver("3.2.0")).unwrap().unwrap(),
        ChangelogInfo {
            title: "3.2.0".to_owned(),
            body: "Great changelog here".to_owned()
        }
    );
    assert_eq!(
        test(&path, text, &ver("3.2.1")).unwrap().unwrap(),
        ChangelogInfo {
            title: "3.2.1 - THE FINAL CHANGETIER".to_owned(),
            body: "WOW!".to_owned()
        }
    );
}

#[test]
fn test_changelog_link_strip() {
    use crate::changelog::changelog_for_version_inner as test;
    let text = kitchen_sink_changelog();
    let path = Utf8PathBuf::from("CHANGELOG.md");

    assert_eq!(
        test(&path, text, &ver("3.2.3")).unwrap().unwrap(),
        ChangelogInfo {
            title: "3.2.3 - NEXT CHANGERATIONS".to_owned(),
            body: "Hope the title link got stripped..!".to_owned()
        }
    );

    assert_eq!(
        test(&path, text, &ver("3.2.3-prerelease.1"))
            .unwrap()
            .unwrap(),
        ChangelogInfo {
            title: "3.2.3-prerelease.1 - NEXT CHANGERATIONS".to_owned(),
            body: "Hope the title link got stripped..!".to_owned()
        }
    );

    assert_eq!(
        test(&path, text, &ver("3.2.5")).unwrap().unwrap(),
        ChangelogInfo {
            title: "v3.2.5 - CHANGEGER".to_owned(),
            body: "Hope the title link also got stripped....!!!".to_owned()
        }
    );

    assert_eq!(
        test(&path, text, &ver("3.2.5-prerelease.3"))
            .unwrap()
            .unwrap(),
        ChangelogInfo {
            title: "v3.2.5-prerelease.3 - CHANGEGER".to_owned(),
            body: "Hope the title link also got stripped....!!!".to_owned()
        }
    );
}

#[test]
fn test_changelog_normalize() {
    use crate::changelog::changelog_for_version_inner as test;
    let text = kitchen_sink_changelog();
    let path = Utf8PathBuf::from("CHANGELOG.md");

    // Test searching for prereleases when there's only a stable version,
    // which should make us do a match while rewriting the title to use that version
    assert_eq!(
        test(&path, text, &ver("1.0.0-prerelease.2"))
            .unwrap()
            .unwrap(),
        ChangelogInfo {
            title: "Version 1.0.0-prerelease.2".to_owned(),
            body: "I'm changelogin' here!".to_owned()
        }
    );
    assert_eq!(
        test(&path, text, &ver("1.0.1-alpha+buildgunk"))
            .unwrap()
            .unwrap(),
        ChangelogInfo {
            title: "Version 1.0.1-alpha+buildgunk - July 3rd, 2025".to_owned(),
            body: "And THAT's\n\nTHE\n\nFACTS".to_owned()
        }
    );
    assert_eq!(
        test(&path, text, &ver("1.2.0-beta")).unwrap().unwrap(),
        ChangelogInfo {
            title: "v1.2.0-beta".to_owned(),
            body: "changelog here".to_owned()
        }
    );
    assert_eq!(
        test(&path, text, &ver("1.2.1-preprerelease"))
            .unwrap()
            .unwrap(),
        ChangelogInfo {
            title: "v1.2.1-preprerelease the BEST version".to_owned(),
            body: "WOW CHANGLOGS!!".to_owned()
        }
    );
    assert_eq!(
        test(&path, text, &ver("3.2.0-omg")).unwrap().unwrap(),
        ChangelogInfo {
            title: "3.2.0-omg".to_owned(),
            body: "Great changelog here".to_owned()
        }
    );
    assert_eq!(
        test(&path, text, &ver("3.2.1-sadness")).unwrap().unwrap(),
        ChangelogInfo {
            title: "3.2.1-sadness - THE FINAL CHANGETIER".to_owned(),
            body: "WOW!".to_owned()
        }
    );
}

#[test]
fn test_changelog_unreleased() {
    use crate::changelog::changelog_for_version_inner as test;
    let text = kitchen_sink_changelog();
    let path = Utf8PathBuf::from("CHANGELOG.md");

    // Test searching for prereleases when there's no match, but there is an Unreleased
    // which should make us do a match while rewriting the title to use that version
    assert_eq!(
        test(&path, text, &ver("4.0.0-prerelease.2"))
            .unwrap()
            .unwrap(),
        ChangelogInfo {
            title: "Version 4.0.0-prerelease.2".to_owned(),
            body: "Coming soon..!".to_owned()
        }
    );
    assert_eq!(
        test(&path, text, &ver("4.0.0-prerelease.2+buildgunkz"))
            .unwrap()
            .unwrap(),
        ChangelogInfo {
            title: "Version 4.0.0-prerelease.2+buildgunkz".to_owned(),
            body: "Coming soon..!".to_owned()
        }
    );
}

#[test]
fn test_changelog_errors() {
    use crate::changelog::changelog_for_version_inner as test;
    let changelog = kitchen_sink_changelog();
    let no_unreleased_changelog = no_unreleased_changelog();
    let doubled_changelog = doubled_changelog();
    let path = Utf8PathBuf::from("CHANGELOG.md");

    // Searching for a stable version that doesn't exist
    assert!(matches!(
        test(&path, changelog, &ver("4.0.0")),
        Err(AxoprojectError::ChangelogVersionNotFound { .. })
    ));

    // Searching for an unstable version without unreleased fallback
    assert!(matches!(
        test(&path, no_unreleased_changelog, &ver("4.0.0-prerelease.2")),
        Err(AxoprojectError::ChangelogVersionNotFound { .. })
    ));

    // Parse-changelog doesn't like changelogs with repeated entries
    assert!(matches!(
        test(&path, doubled_changelog, &ver("1.0.0")),
        Err(AxoprojectError::ParseChangelog(..))
    ));
}

#[test]
fn test_generic_c() {
    let project = crate::get_workspaces("tests/projects/generic-c/".into(), None)
        .best()
        .unwrap();
    assert_eq!(project.kind, WorkspaceKind::Generic);
    assert_eq!(project._package_info.len(), 1);
    assert!(project.manifest_path.exists());

    let package = &project._package_info[0];
    assert_eq!(package.name, "testprog");
    assert_eq!(package.binaries.len(), 1);
    assert!(project.manifest_path.exists());

    let binary = &package.binaries[0];
    assert_eq!(binary, "main");
}

#[test]
fn test_generic_workspace_root() {
    generic_workspace_check("tests/projects/generic-workspace")
}

#[test]
fn test_generic_workspace_subdir() {
    generic_workspace_check("tests/projects/generic-workspace/generic1/")
}

#[test]
fn test_shared_workspace_root() {
    shared_workspace_check("tests/projects/shared-workspace")
}

fn generic_workspace_check<'a>(path: impl Into<&'a Utf8Path>) {
    let workspaces = WorkspaceGraph::find(path.into(), None).unwrap();
    let project = workspaces.root_workspace();
    let packages = workspaces
        .direct_packages(workspaces.root_workspace_idx())
        .collect::<Vec<_>>();
    assert_eq!(project.kind, WorkspaceKind::Generic);
    assert_eq!(packages.len(), 2);
    assert!(project.manifest_path.exists());
    check_file(
        project.root_auto_includes.readme.as_deref().unwrap(),
        "root fake readme!",
    );
    check_file(
        &project.root_auto_includes.licenses[0],
        "root fake license!",
    );
    check_file(
        project.root_auto_includes.changelog.as_deref().unwrap(),
        "root fake changelog!",
    );
    // repository is inconsistent, so this should be Err
    assert!(workspaces.repository_url(None).is_err());

    {
        let package = &packages[0].1;
        assert_eq!(package.name, "generic1");
        assert_eq!(package.binaries.len(), 1);
        let binary = &package.binaries[0];
        assert_eq!(binary, "main");
        assert!(package.manifest_path.exists());
        assert!(package.manifest_path != project.manifest_path);
        // Inner package defines its own auto_includes
        check_file(
            package.readme_file.as_deref().unwrap(),
            "inner fake readme!",
        );
        check_file(&package.license_files[0], "inner fake license!");
        check_file(
            package.changelog_file.as_deref().unwrap(),
            "inner fake changelog!",
        );
        // repository should yield this one, so this should faile
        assert_eq!(
            workspaces
                .repository_url(Some(&[PackageIdx(0)]))
                .unwrap()
                .unwrap()
                .0,
            "https://github.com/mistydemeo/testprog1"
        );
    }

    {
        let package = &packages[1].1;
        assert_eq!(package.name, "generic2");
        assert_eq!(package.binaries.len(), 1);
        let binary = &package.binaries[0];
        assert_eq!(binary, "main");
        assert!(package.manifest_path.exists());
        assert!(package.manifest_path != project.manifest_path);
        // Inner package inherits root auto_includes
        check_file(package.readme_file.as_deref().unwrap(), "root fake readme!");
        check_file(&package.license_files[0], "root fake license!");
        check_file(
            package.changelog_file.as_deref().unwrap(),
            "root fake changelog!",
        );
        assert_eq!(
            workspaces
                .repository_url(Some(&[PackageIdx(1)]))
                .unwrap()
                .unwrap()
                .0,
            "https://github.com/mistydemeo/testprog2"
        );
    }
}

fn shared_workspace_check<'a>(path: impl Into<&'a Utf8Path>) {
    let workspaces = WorkspaceGraph::find(path.into(), None).unwrap();
    let project = workspaces.root_workspace();
    let direct_packages = workspaces
        .direct_packages(workspaces.root_workspace_idx())
        .collect::<Vec<_>>();
    assert_eq!(project.kind, WorkspaceKind::Generic);
    assert_eq!(direct_packages.len(), 2);
    assert!(project.manifest_path.exists());
    check_file(
        project.root_auto_includes.readme.as_deref().unwrap(),
        "root fake readme!",
    );
    check_file(
        &project.root_auto_includes.licenses[0],
        "root fake license!",
    );
    check_file(
        project.root_auto_includes.changelog.as_deref().unwrap(),
        "root fake changelog!",
    );
    // repository is inconsistent, so this should be Err
    assert!(workspaces.repository_url(None).is_err());

    {
        let package = &direct_packages[0].1;
        assert_eq!(package.name, "generic1");
        assert_eq!(package.binaries.len(), 1);
        let binary = &package.binaries[0];
        assert_eq!(binary, "main");
        assert!(package.manifest_path.exists());
        assert!(package.manifest_path != project.manifest_path);
        // Inner package defines its own auto_includes
        check_file(
            package.readme_file.as_deref().unwrap(),
            "inner fake readme!",
        );
        check_file(&package.license_files[0], "inner fake license!");
        check_file(
            package.changelog_file.as_deref().unwrap(),
            "inner fake changelog!",
        );
        // repository should yield this one, so this should faile
        assert_eq!(
            workspaces
                .repository_url(Some(&[PackageIdx(0)]))
                .unwrap()
                .unwrap()
                .0,
            "https://github.com/mistydemeo/testprog1"
        );
    }

    {
        let package = &direct_packages[1].1;
        assert_eq!(package.name, "generic2");
        assert_eq!(package.binaries.len(), 1);
        let binary = &package.binaries[0];
        assert_eq!(binary, "main");
        assert!(package.manifest_path.exists());
        assert!(package.manifest_path != project.manifest_path);
        // Inner package inherits root auto_includes
        check_file(package.readme_file.as_deref().unwrap(), "root fake readme!");
        check_file(&package.license_files[0], "root fake license!");
        check_file(
            package.changelog_file.as_deref().unwrap(),
            "root fake changelog!",
        );
        assert_eq!(
            workspaces
                .repository_url(Some(&[PackageIdx(1)]))
                .unwrap()
                .unwrap()
                .0,
            "https://github.com/mistydemeo/testprog2"
        );
    }

    // TODO: write checks for the cargo parts
}

#[track_caller]
fn check_file(file: &Utf8Path, val: &str) {
    assert!(axoasset::LocalAsset::load_string(file).unwrap().trim() == val)
}
