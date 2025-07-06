//! Details for hosting artifacts

use crate::{
    announce::AnnouncementTag,
    check_integrity,
    config::{
        v1::{ci::CiConfig, hosts::WorkspaceHostConfig},
        CiStyle, Config, HostArgs, HostStyle, HostingStyle,
    },
    errors::DistResult,
    gather_work,
    manifest::save_manifest,
    DistError, DistGraphBuilder, HostingInfo,
};
use axoproject::WorkspaceGraph;
use dist_schema::DistManifest;

/// Do hosting
pub fn do_host(cfg: &Config, host_args: HostArgs) -> DistResult<DistManifest> {
    check_integrity(cfg)?;

    // the "create hosting" step is kinda intertwined with details of gather_work,
    // so we implement it by specifying whether hosting should be created
    let cfg = Config {
        create_hosting: host_args.steps.contains(&HostStyle::Create),
        ..cfg.clone()
    };
    let (dist, manifest) = gather_work(&cfg)?;

    // The rest of the steps are more self-contained

    if let Some(hosting) = &dist.hosting {
        for host in &hosting.hosts {
            match host {
                HostingStyle::Github => {
                    // implemented in CI backend
                }
            }
        }
    }

    // save the potentially updated dist-manifest with hosting info
    save_manifest(&dist.dist_dir.join("dist-manifest.json"), &manifest)?;

    Ok(manifest)
}

impl<'a> DistGraphBuilder<'a> {
    pub(crate) fn compute_hosting(
        &mut self,
        cfg: &Config,
        announcing: &AnnouncementTag,
    ) -> DistResult<()> {
        let mut ci = vec![];
        {
            let CiConfig { github } = &self.inner.config.ci;
            if github.is_some() {
                ci.push(CiStyle::Github);
            }
        }

        let mut hosting = vec![];
        {
            let WorkspaceHostConfig {
                github,
                force_latest: _,
            } = &self.inner.config.hosts;
            if github.is_some() {
                hosting.push(HostingStyle::Github);
            }
        }
        let hosting = if hosting.is_empty() {
            None
        } else {
            Some(hosting)
        };
        self.inner.hosting = select_hosting(self.workspaces, announcing, hosting, Some(&ci))?;
        // If we don't think we can host things, don't bother
        let Some(hosting) = &self.inner.hosting else {
            return Ok(());
        };

        let create_hosting =
            cfg.create_hosting && std::env::var("CARGO_DIST_MOCK_NETWORKING").is_err();

        let releases_without_hosting = announcing
            .rust_releases
            .iter()
            .filter_map(|release| {
                // Get the names of the apps we're releasing
                let package = self.workspaces.package(release.package_idx);
                let version = package
                    .version
                    .clone()
                    .expect("package must have version!")
                    .to_string();
                let name = package.name.clone();
                // Only update them if they don't already have hosting
                // if create_hosting is set, then consider all entries out of date
                // and needing refreshing (this is only set by `dist host create`)
                let needs_hosting = create_hosting
                    || self
                        .manifest
                        .release_by_name(&name)
                        .map(|r| r.hosting.is_empty())
                        .unwrap_or(true);
                if needs_hosting {
                    Some((name, version))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        // If everything was already provided by a merged dist-manifest, don't redo it
        if releases_without_hosting.is_empty() {
            return Ok(());
        }

        for host in &hosting.hosts {
            match host {
                HostingStyle::Github => {
                    // CI currently impls this for us, all we need to know is the URL to download from
                    let repo_path = &hosting.repo_path;
                    for (name, version) in &releases_without_hosting {
                        let tag = &announcing.tag;
                        self.manifest
                            .ensure_release(name.clone(), version.clone())
                            .hosting
                            .github = Some(dist_schema::GithubHosting {
                            artifact_base_url: hosting.domain.clone(),
                            artifact_download_path: format!("{repo_path}/releases/download/{tag}"),
                            owner: hosting.owner.clone(),
                            repo: hosting.project.clone(),
                        })
                    }
                }
            }
        }

        Ok(())
    }
}

pub(crate) fn select_hosting(
    workspaces: &WorkspaceGraph,
    announcing: &AnnouncementTag,
    hosting: Option<Vec<HostingStyle>>,
    ci: Option<&[CiStyle]>,
) -> DistResult<Option<HostingInfo>> {
    // Either use the explicit one, or default to the CI provider's native solution
    let Some(hosting_providers) = hosting
        .clone()
        .or_else(|| Some(vec![ci.as_ref()?.first()?.native_hosting()?]))
    else {
        // This is the one case where we'll tolerate hosting not existing:
        // * they don't have one set explicitly
        // * and they haven't turned on a CI provider
        // This implies early setup or using dist very "manually"
        return Ok(None);
    };

    // Get the list of packages we actually care about
    let package_list = announcing
        .rust_releases
        .iter()
        .map(|release| release.package_idx)
        .collect::<Vec<_>>();

    let raw_repository_url = match workspaces.repository_url(Some(&package_list)) {
        Ok(Some(url)) => url,
        Ok(None) => {
            let mut manifest_list = String::new();
            for pkg_idx in package_list {
                let package = workspaces.package(pkg_idx);
                manifest_list.push('\n');
                manifest_list.push_str(package.manifest_path.as_str());
            }
            return Err(DistError::CantEnableGithubNoUrl { manifest_list });
        }
        Err(e) => {
            return Err(DistError::CantEnableGithubUrlInconsistent { inner: e });
        }
    };

    // Currently there's only one supported sourcehost provider
    let repo = raw_repository_url
        .github_repo()
        .map_err(|e| DistError::CantEnableGithubUrlNotGithub { inner: e })?;
    let domain = repo.domain();
    let repo_path = repo.web_path();

    Ok(Some(HostingInfo {
        hosts: hosting_providers,
        domain,
        repo_path,
        source_host: "github".to_owned(),
        owner: repo.owner,
        project: repo.name,
    }))
}
