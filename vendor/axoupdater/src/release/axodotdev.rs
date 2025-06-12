//! Fetching and processing from axo Releases

use super::{Asset, Release};
use crate::errors::*;
use axotag::Version;
use gazenot::Gazenot;

pub(crate) async fn get_specific_axo_version(
    name: &str,
    owner: &str,
    app_name: &str,
    version: &Version,
) -> AxoupdateResult<Release> {
    let releases = get_axo_releases(name, owner, app_name).await?;
    let release = releases.into_iter().find(|r| &r.version == version);

    if let Some(release) = release {
        Ok(release)
    } else {
        Err(AxoupdateError::ReleaseNotFound {
            name: name.to_owned(),
            app_name: app_name.to_owned(),
        })
    }
}

pub(crate) async fn get_specific_axo_tag(
    name: &str,
    owner: &str,
    app_name: &str,
    tag: &str,
) -> AxoupdateResult<Release> {
    let releases = get_axo_releases(name, owner, app_name).await?;
    let release = releases.into_iter().find(|r| r.tag_name == tag);

    if let Some(release) = release {
        Ok(release)
    } else {
        Err(AxoupdateError::ReleaseNotFound {
            name: name.to_owned(),
            app_name: app_name.to_owned(),
        })
    }
}

pub(crate) async fn get_axo_releases(
    name: &str,
    owner: &str,
    app_name: &str,
) -> AxoupdateResult<Vec<Release>> {
    let abyss = Gazenot::new_unauthed("github".to_string(), owner)?;
    let release_lists = abyss.list_releases_many(vec![app_name.to_owned()]).await?;
    let Some(our_release) = release_lists
        .into_iter()
        .find(|rl| rl.package_name == app_name)
    else {
        return Err(AxoupdateError::ReleaseNotFound {
            name: name.to_owned(),
            app_name: app_name.to_owned(),
        });
    };

    let releases: Vec<Release> = our_release
        .releases
        .into_iter()
        .filter_map(|r| Release::try_from_gazenot(r).ok())
        .collect();

    Ok(releases)
}

impl Release {
    /// Constructs a release from Axo Releases data fetched via gazenot.
    pub(crate) fn try_from_gazenot(release: gazenot::PublicRelease) -> AxoupdateResult<Release> {
        Ok(Release {
            tag_name: release.tag_name,
            version: release.version.parse()?,
            name: release.name,
            url: String::new(),
            assets: release
                .assets
                .into_iter()
                .map(|asset| Asset {
                    url: asset.browser_download_url.clone(),
                    browser_download_url: asset.browser_download_url,
                    name: asset.name,
                })
                .collect(),
            prerelease: release.prerelease,
        })
    }
}
