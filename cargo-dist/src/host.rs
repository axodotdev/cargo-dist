//! Details for hosting artifacts

use crate::{
    announce::{announcement_axodotdev, announcement_github, AnnouncementTag},
    check_integrity,
    config::{CiStyle, Config, HostArgs, HostStyle, HostingStyle},
    errors::DistResult,
    gather_work,
    manifest::save_manifest,
    net::create_gazenot_client,
    DistGraph, DistGraphBuilder, HostingInfo,
};
use axoproject::WorkspaceInfo;
use cargo_dist_schema::{DistManifest, Hosting};
use gazenot::{AnnouncementKey, Gazenot};
use tracing::warn;

/// Do hosting
pub fn do_host(cfg: &Config, host_args: HostArgs) -> DistResult<DistManifest> {
    check_integrity(cfg)?;

    // the "create hosting" step is kinda intertwined with details of gather_work,
    // so we implement it by specifying whether hosting should be created
    let cfg = Config {
        create_hosting: host_args.steps.contains(&HostStyle::Create),
        ..cfg.clone()
    };
    let (dist, mut manifest) = gather_work(&cfg)?;

    // The rest of the steps are more self-contained

    if let Some(hosting) = &dist.hosting {
        for host in &hosting.hosts {
            match host {
                HostingStyle::Axodotdev => {
                    let abyss = create_gazenot_client(
                        &dist.client_settings,
                        &hosting.source_host,
                        &hosting.owner,
                    )?;
                    if host_args.steps.contains(&HostStyle::Check) {
                        check_hosting(&dist, &manifest, &abyss)?;
                    }
                    if host_args.steps.contains(&HostStyle::Upload) {
                        // pre-save the hosting info so that it will be found on the FS to upload.
                        // The currently in-memory manifest has more information about stuff that
                        // the original 'plan' couldn't: system info, linkage, hashes, symbols, ...
                        save_manifest(&dist.dist_dir.join("dist-manifest.json"), &manifest)?;
                        upload_to_hosting(&dist, &manifest, &abyss)?;
                    }
                    if host_args.steps.contains(&HostStyle::Release) {
                        // note that this mutates the manifest with new Release URLs, to be written back!
                        // this makes Axo Releases and Github Releases diverge on the dist-manifest.json
                        // uploaded to them, differing specifically in the URLs in install-hints. This is OK.
                        release_hosting(&dist, &mut manifest, &abyss)?;
                    }
                    if host_args.steps.contains(&HostStyle::Announce) {
                        announce_hosting(&dist, &manifest, &abyss)?;
                    }
                }
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
        hosting: Option<Vec<HostingStyle>>,
        ci: Option<Vec<CiStyle>>,
    ) -> DistResult<()> {
        self.inner.hosting = select_hosting(self.workspace, announcing, hosting, ci.as_deref());
        // If we don't think we can host things, don't bother
        let Some(hosting) = &self.inner.hosting else {
            return Ok(());
        };

        let create_hosting =
            cfg.create_hosting && std::env::var("CARGO_DIST_MOCK_NETWORKING").is_err();

        let releases_without_hosting = announcing
            .rust_releases
            .iter()
            .filter_map(|(package, _)| {
                // Get the names of the apps we're releasing
                let package = self.workspace.package(*package);
                let version = package
                    .version
                    .clone()
                    .expect("package must have version!")
                    .to_string();
                let name = package.name.clone();
                // Only update them if they don't already have hosting
                // if create_hosting is set, then consider all entries out of date
                // and needing refreshing (this is only set by `cargo dist host create`)
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
                HostingStyle::Axodotdev => {
                    // Ask The Abyss For Hosting, or mock the result
                    let packages = releases_without_hosting
                        .iter()
                        .map(|(name, _version)| name.clone());

                    let artifact_sets = if create_hosting {
                        let abyss = create_gazenot_client(
                            &self.inner.client_settings,
                            &hosting.source_host,
                            &hosting.owner,
                        )?;
                        tokio::runtime::Handle::current()
                            .block_on(abyss.create_artifact_sets(packages))?
                    } else {
                        packages.map(gazenot::ArtifactSet::mock).collect()
                    };

                    // Store the results so other machines can use it
                    for ((name, version), set) in releases_without_hosting.iter().zip(artifact_sets)
                    {
                        assert_eq!(
                            *name, set.package,
                            "gazenot got confused about package names..."
                        );
                        self.manifest
                            .ensure_release(name.clone(), version.clone())
                            .hosting
                            .axodotdev = Some(set);
                    }
                }
                HostingStyle::Github => {
                    // CI currently impls this for us, all we need to know is the URL to download from
                    let repo_url = &hosting.repo_url;
                    for (name, version) in &releases_without_hosting {
                        let tag = &announcing.tag;
                        self.manifest
                            .ensure_release(name.clone(), version.clone())
                            .hosting
                            .github = Some(cargo_dist_schema::GithubHosting {
                            artifact_download_url: format!("{repo_url}/releases/download/{tag}"),
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

fn check_hosting(_dist: &DistGraph, _manifest: &DistManifest, _abyss: &Gazenot) -> DistResult<()> {
    // FIXME: implement a ping/whoami API to check the Abyss client is working

    Ok(())
}

fn upload_to_hosting(dist: &DistGraph, manifest: &DistManifest, abyss: &Gazenot) -> DistResult<()> {
    // Gather up the files to upload for each release
    let files = manifest.releases.iter().filter_map(|release| {
        // Github Releases only has semantics on Announce
        let Hosting {
            axodotdev,
            github: _,
        } = &release.hosting;
        if let Some(set) = axodotdev {
            // Upload all files associated with this Release, plus the dist-manifest.json
            let files = manifest
                .artifacts_for_release(release)
                .filter_map(|(_id, artifact)| artifact.name.as_deref())
                .chain(Some("dist-manifest.json"))
                .map(|name| dist.dist_dir.join(name))
                .collect::<Vec<_>>();
            Some((set, files))
        } else {
            None
        }
    });

    tokio::runtime::Handle::current().block_on(abyss.upload_files(files))?;
    eprintln!("all artifacts hosted!");
    Ok(())
}

fn release_hosting(
    _dist: &DistGraph,
    manifest: &mut DistManifest,
    abyss: &Gazenot,
) -> DistResult<()> {
    // Gather up the releases
    let releases = manifest.releases.iter().filter_map(|release| {
        // Github Releases only has semantics on Announce
        let Hosting {
            axodotdev,
            github: _,
        } = &release.hosting;
        if let Some(set) = axodotdev {
            let release = gazenot::ReleaseKey {
                version: release.app_version.clone(),
                tag: manifest.announcement_tag.clone().unwrap(),
                is_prerelease: manifest.announcement_is_prerelease,
            };
            Some((set, release))
        } else {
            None
        }
    });

    // Tell The Abyss To Release
    let new_releases =
        tokio::runtime::Handle::current().block_on(abyss.create_releases(releases))?;

    // Update artifact download URLs with release results
    for new_release in new_releases {
        if let Some(new_url) = new_release.release_download_url {
            manifest.update_release_axodotdev_artifact_download_url(&new_release.package, new_url);
        }
    }

    // Update Github Announcement body with new URLs
    announcement_github(manifest);

    eprintln!("release published!");
    Ok(())
}

fn announce_hosting(_dist: &DistGraph, manifest: &DistManifest, abyss: &Gazenot) -> DistResult<()> {
    // Perform the announcement
    let releases = manifest
        .releases
        .iter()
        .filter_map(|release| {
            // FIXME: implement native github releases support? (currently exists in github ci logic)
            let Hosting {
                axodotdev,
                github: _,
            } = &release.hosting;
            axodotdev
                .as_ref()
                .map(|set| set.to_release(manifest.announcement_tag.clone().unwrap()))
        })
        .collect::<Vec<_>>();

    let announcement = AnnouncementKey {
        body: announcement_axodotdev(manifest),
    };
    tokio::runtime::Handle::current()
        .block_on(abyss.create_announcements(&releases, announcement))?;
    eprintln!("release announced!");
    Ok(())
}

pub(crate) fn select_hosting(
    workspace: &WorkspaceInfo,
    announcing: &AnnouncementTag,
    hosting: Option<Vec<HostingStyle>>,
    ci: Option<&[CiStyle]>,
) -> Option<HostingInfo> {
    let package_list = announcing
        .rust_releases
        .iter()
        .map(|(idx, _)| *idx)
        .collect::<Vec<_>>();
    // Either use the explicit one, or default to the CI provider's native solution
    let hosting_providers = hosting
        .clone()
        .or_else(|| Some(vec![ci.as_ref()?.first()?.native_hosting()?]))?;
    // Check that there's a consistent repository URL, and if not, warn
    if let Err(warning) = workspace.repository_url(Some(&package_list)) {
        let report = miette::Report::new(warning);
        warn!("{:?}", report);
    };
    let repo_url = workspace.web_url(Some(&package_list)).unwrap_or_default()?;
    // Currently there's only one supported sourcehost provider
    let repo = workspace
        .github_repo(Some(&package_list))
        .unwrap_or_default()?;

    Some(HostingInfo {
        hosts: hosting_providers,
        repo_url: repo_url.clone(),
        source_host: "github".to_owned(),
        owner: repo.owner,
        project: repo.name,
    })
}
