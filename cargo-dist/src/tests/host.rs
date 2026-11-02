use super::mock::*;
use crate::announce::{select_tag, AnnouncementTag, TagMode, TagSettings};
use crate::config::{CiStyle, HostingStyle};
use crate::host::select_hosting;
use crate::DistError;
use crate::{config::ArtifactMode, DistGraphBuilder};
use axoproject::errors::AxoprojectError;
use axoproject::{PackageIdx, WorkspaceGraph};
use semver::Version;

fn mock_announce(workspaces: &mut WorkspaceGraph) -> (DistGraphBuilder<'_>, AnnouncementTag) {
    let version: Version = BIN_AXO_VER.parse().unwrap();
    let tag = format!("{version}");

    let tools = mock_tools();
    let mut graph = DistGraphBuilder::new(
        "a".to_owned(),
        tools,
        workspaces,
        ArtifactMode::All,
        true,
        false,
    )
    .unwrap();
    let settings = TagSettings {
        needs_coherence: true,
        tag: TagMode::Select(tag.clone()),
    };
    let announcing = select_tag(&mut graph, &settings).unwrap();
    (graph, announcing)
}

#[test]
fn github_simple() {
    // ci = "github" and hosting = "github"
    let mut workspaces = workspace_unified();
    let hosting = Some(vec![HostingStyle::Github]);
    let ci = Some(vec![CiStyle::Github]);

    let (_graph, announcing) = mock_announce(&mut workspaces);
    let hosting = select_hosting(&workspaces, &announcing, hosting, ci.as_deref());

    let hosting = hosting.unwrap().unwrap();
    assert_eq!(hosting.hosts, &[HostingStyle::Github]);
    assert_eq!(hosting.owner, REPO_OWNER);
    assert_eq!(hosting.project, REPO_PROJECT);
    assert_eq!(hosting.source_host, "github");
}

#[test]
fn github_implicit() {
    // ci = "github" and hosting = None
    let mut workspaces = workspace_unified();
    let hosting = None;
    let ci = Some(vec![CiStyle::Github]);

    let (_graph, announcing) = mock_announce(&mut workspaces);
    let hosting = select_hosting(&workspaces, &announcing, hosting, ci.as_deref());

    let hosting = hosting.unwrap().unwrap();
    assert_eq!(hosting.hosts, &[HostingStyle::Github]);
    assert_eq!(hosting.owner, REPO_OWNER);
    assert_eq!(hosting.project, REPO_PROJECT);
    assert_eq!(hosting.source_host, "github");
}

#[test]
fn github_diff_repository_on_non_distables() {
    // DIFFERENT repository keys for each non-distable package
    const OTHER_REPO_URL: &str = "https://github.com/mycoolorg/radproj";
    let mut workspaces = workspace_unified();
    let num_packages = workspaces.all_packages().count();
    for pkg_idx in 0..num_packages {
        let package = workspaces.package_mut(PackageIdx(pkg_idx));
        if package.binaries.is_empty() {
            package.repository_url = Some(OTHER_REPO_URL.to_owned());
        }
    }
    let hosting = None;
    let ci = Some(vec![CiStyle::Github]);

    let (_graph, announcing) = mock_announce(&mut workspaces);
    let hosting = select_hosting(&workspaces, &announcing, hosting, ci.as_deref());

    let hosting = hosting.unwrap().unwrap();
    assert_eq!(hosting.hosts, &[HostingStyle::Github]);
    assert_eq!(hosting.owner, REPO_OWNER);
    assert_eq!(hosting.project, REPO_PROJECT);
    assert_eq!(hosting.source_host, "github");
}

#[test]
fn github_no_repository() {
    // no repository key in any packages
    let mut workspaces = workspace_unified();
    let num_packages = workspaces.all_packages().count();
    for pkg_idx in 0..num_packages {
        let package = workspaces.package_mut(PackageIdx(pkg_idx));
        package.repository_url = None;
    }

    let hosting = None;
    let ci = Some(vec![CiStyle::Github]);

    let (_graph, announcing) = mock_announce(&mut workspaces);
    let hosting = select_hosting(&workspaces, &announcing, hosting, ci.as_deref());

    if let Err(DistError::CantEnableGithubNoUrl { manifest_list }) = &hosting {
        assert!(manifest_list.contains(".toml"));
    } else {
        panic!("unexpected result: {hosting:?}");
    }
}

#[test]
fn github_diff_repository() {
    // DIFFERENT repository keys for each package
    const OTHER_REPO_URL: &str = "https://github.com/mycoolorg/radproj";
    let mut workspaces = workspace_unified();
    let num_packages = workspaces.all_packages().count();
    for pkg_idx in 0..num_packages {
        let package = workspaces.package_mut(PackageIdx(pkg_idx));
        if !package.binaries.is_empty() {
            package.repository_url = Some(OTHER_REPO_URL.to_owned());
            break;
        }
    }

    let hosting = None;
    let ci = Some(vec![CiStyle::Github]);

    let (_graph, announcing) = mock_announce(&mut workspaces);
    let hosting = select_hosting(&workspaces, &announcing, hosting, ci.as_deref());

    if let Err(DistError::CantEnableGithubUrlInconsistent {
        inner:
            AxoprojectError::InconsistentRepositoryKey {
                file1: _,
                url1,
                file2: _,
                url2,
            },
    }) = &hosting
    {
        assert_eq!(url1, OTHER_REPO_URL);
        assert_eq!(url2, REPO_URL);
    } else {
        panic!("unexpected result: {hosting:?}");
    }
}

#[test]
fn github_not_github_repository() {
    // repo isn't github, but hosting enabled
    const NOT_GH_REPO_URL: &str = "https://mysourcehost.com/mycoolorg/radproj";

    let mut workspaces = workspace_unified();
    let num_packages = workspaces.all_packages().count();
    for pkg_idx in 0..num_packages {
        let package = workspaces.package_mut(PackageIdx(pkg_idx));
        package.repository_url = Some(NOT_GH_REPO_URL.to_owned());
    }

    let hosting = None;
    let ci = Some(vec![CiStyle::Github]);

    let (_graph, announcing) = mock_announce(&mut workspaces);
    let hosting = select_hosting(&workspaces, &announcing, hosting, ci.as_deref());

    if let Err(DistError::CantEnableGithubUrlNotGithub {
        inner: AxoprojectError::NotGitHubError { url },
    }) = &hosting
    {
        assert_eq!(url, NOT_GH_REPO_URL);
    } else {
        panic!("unexpected result: {hosting:?}");
    }
}

#[test]
fn no_ci_no_problem() {
    // no repository key in any packages, but ci is disabled
    let mut workspaces = workspace_unified();
    let num_packages = workspaces.all_packages().count();
    for pkg_idx in 0..num_packages {
        let package = workspaces.package_mut(PackageIdx(pkg_idx));
        package.repository_url = None;
    }

    let hosting = None;
    let ci = None;

    let (_graph, announcing) = mock_announce(&mut workspaces);
    let hosting = select_hosting(&workspaces, &announcing, hosting, ci);

    assert!(matches!(hosting, Ok(None)))
}

#[test]
fn github_dot_git() {
    // passed in a .git url
    let mut workspaces = workspace_unified();
    let num_packages = workspaces.all_packages().count();
    for pkg_idx in 0..num_packages {
        let package = workspaces.package_mut(PackageIdx(pkg_idx));
        package.repository_url = Some(format!("{REPO_URL}.git"));
    }
    let hosting = Some(vec![HostingStyle::Github]);
    let ci = Some(vec![CiStyle::Github]);

    let (_graph, announcing) = mock_announce(&mut workspaces);
    let hosting = select_hosting(&workspaces, &announcing, hosting, ci.as_deref());

    let hosting = hosting.unwrap().unwrap();
    assert_eq!(hosting.hosts, &[HostingStyle::Github]);
    assert_eq!(hosting.owner, REPO_OWNER);
    assert_eq!(hosting.project, REPO_PROJECT);
    assert_eq!(hosting.source_host, "github");
}

#[test]
fn github_trail_slash() {
    // repo urls only differ by trailing slash
    let mut workspaces = workspace_unified();
    let num_packages = workspaces.all_packages().count();
    for pkg_idx in 0..num_packages {
        let package = workspaces.package_mut(PackageIdx(pkg_idx));
        if !package.binaries.is_empty() {
            package.repository_url = Some(format!("{REPO_URL}/"));
            break;
        }
    }
    let hosting = Some(vec![HostingStyle::Github]);
    let ci = Some(vec![CiStyle::Github]);

    let (_graph, announcing) = mock_announce(&mut workspaces);
    let hosting = select_hosting(&workspaces, &announcing, hosting, ci.as_deref());

    let hosting = hosting.unwrap().unwrap();
    assert_eq!(hosting.hosts, &[HostingStyle::Github]);
    assert_eq!(hosting.owner, REPO_OWNER);
    assert_eq!(hosting.project, REPO_PROJECT);
    assert_eq!(hosting.source_host, "github");
}
