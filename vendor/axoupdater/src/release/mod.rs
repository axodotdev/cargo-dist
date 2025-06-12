use std::fmt;

use serde::Deserialize;

use crate::{errors::*, AuthorizationTokens, AxoUpdater, UpdateRequest, Version};

#[cfg(feature = "axo_releases")]
pub(crate) mod axodotdev;
#[cfg(feature = "github_releases")]
pub(crate) mod github;

/// A struct representing a specific release, either from GitHub or Axo Releases.
#[derive(Clone, Debug)]
pub struct Release {
    /// The tag this release represents
    pub tag_name: String,
    /// The version this release represents
    pub version: Version,
    /// The name of the release
    pub name: String,
    /// The URL at which this release lists
    pub url: String,
    /// All assets associated with this release
    pub assets: Vec<Asset>,
    /// Whether or not this release is a prerelease
    pub prerelease: bool,
}

/// Represents a specific asset inside a release.
#[derive(Clone, Debug)]
pub struct Asset {
    /// The URL at which this asset can be found
    pub url: String,
    /// The URL at which this asset can be downloaded
    pub browser_download_url: String,
    /// This asset's name
    pub name: String,
}

/// Where service this app's releases are hosted on
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseSourceType {
    /// GitHub Releases
    GitHub,
    /// Axo Releases
    Axo,
}

impl fmt::Display for ReleaseSourceType {
    /// Returns a string representation of this ReleaseSourceType.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::GitHub => write!(f, "github"),
            Self::Axo => write!(f, "axodotdev"),
        }
    }
}

/// Information about the source of this app's releases
#[derive(Clone, Debug, Deserialize)]
pub struct ReleaseSource {
    /// Which hosting service to query for new releases
    pub release_type: ReleaseSourceType,
    /// Owner, in GitHub name-with-owner format
    pub owner: String,
    /// Name, in GitHub name-with-owner format
    pub name: String,
    /// The app's name; this can be distinct from the repository name above
    pub app_name: String,
}

impl AxoUpdater {
    /// Configures AxoUpdater to use a specific GitHub token when performing requests.
    /// This is useful in circumstances where the user may encounter rate
    /// limits, and is necessary to access private repositories.
    /// This must have the `repo` scope enabled.
    pub fn set_github_token(&mut self, token: &str) -> &mut AxoUpdater {
        self.tokens.github = Some(token.to_owned());

        self
    }

    /// Configures AxoUpdater to use a specific Axo Releases token when performing requests.
    pub fn set_axo_token(&mut self, token: &str) -> &mut AxoUpdater {
        self.tokens.axodotdev = Some(token.to_owned());

        self
    }

    pub(crate) async fn fetch_release(&mut self) -> AxoupdateResult<()> {
        let Some(app_name) = &self.name else {
            return Err(AxoupdateError::NotConfigured {
                missing_field: "app_name".to_owned(),
            });
        };
        let Some(source) = &self.source else {
            return Err(AxoupdateError::NotConfigured {
                missing_field: "source".to_owned(),
            });
        };

        let release = match self.version_specifier.to_owned() {
            UpdateRequest::Latest => {
                get_latest_stable_release(
                    &source.name,
                    &source.owner,
                    &source.app_name,
                    &source.release_type,
                    &self.tokens,
                )
                .await?
            }
            UpdateRequest::LatestMaybePrerelease => {
                get_latest_maybe_prerelease(
                    &source.name,
                    &source.owner,
                    &source.app_name,
                    &source.release_type,
                    &self.tokens,
                )
                .await?
            }
            UpdateRequest::SpecificTag(version) => {
                get_specific_tag(
                    &source.name,
                    &source.owner,
                    &source.app_name,
                    &source.release_type,
                    &version,
                    &self.tokens,
                )
                .await?
            }
            UpdateRequest::SpecificVersion(version) => {
                get_specific_version(
                    &source.name,
                    &source.owner,
                    &source.app_name,
                    &source.release_type,
                    &version.parse::<Version>()?,
                    &self.tokens,
                )
                .await?
            }
        };

        let Some(release) = release else {
            return Err(AxoupdateError::NoStableReleases {
                app_name: app_name.to_owned(),
            });
        };

        self.requested_release = Some(release);

        Ok(())
    }
}

pub(crate) async fn get_specific_version(
    name: &str,
    owner: &str,
    app_name: &str,
    release_type: &ReleaseSourceType,
    version: &Version,
    tokens: &AuthorizationTokens,
) -> AxoupdateResult<Option<Release>> {
    let release = match release_type {
        #[cfg(feature = "github_releases")]
        ReleaseSourceType::GitHub => {
            github::get_specific_github_version(name, owner, app_name, version, &tokens.github)
                .await?
        }
        #[cfg(not(feature = "github_releases"))]
        ReleaseSourceType::GitHub => {
            return Err(AxoupdateError::BackendDisabled {
                backend: "github".to_owned(),
            })
        }
        #[cfg(feature = "axo_releases")]
        ReleaseSourceType::Axo => {
            axodotdev::get_specific_axo_version(name, owner, app_name, version).await?
        }
        #[cfg(not(feature = "axo_releases"))]
        ReleaseSourceType::Axo => {
            return Err(AxoupdateError::BackendDisabled {
                backend: "axodotdev".to_owned(),
            })
        }
    };

    Ok(Some(release))
}

pub(crate) async fn get_specific_tag(
    name: &str,
    owner: &str,
    app_name: &str,
    release_type: &ReleaseSourceType,
    tag: &str,
    tokens: &AuthorizationTokens,
) -> AxoupdateResult<Option<Release>> {
    let release = match release_type {
        #[cfg(feature = "github_releases")]
        ReleaseSourceType::GitHub => {
            github::get_specific_github_tag(name, owner, app_name, tag, &tokens.github).await?
        }
        #[cfg(not(feature = "github_releases"))]
        ReleaseSourceType::GitHub => {
            return Err(AxoupdateError::BackendDisabled {
                backend: "github".to_owned(),
            })
        }
        #[cfg(feature = "axo_releases")]
        ReleaseSourceType::Axo => {
            axodotdev::get_specific_axo_tag(name, owner, app_name, tag).await?
        }
        #[cfg(not(feature = "axo_releases"))]
        ReleaseSourceType::Axo => {
            return Err(AxoupdateError::BackendDisabled {
                backend: "axodotdev".to_owned(),
            })
        }
    };

    Ok(Some(release))
}

pub(crate) async fn get_release_list(
    name: &str,
    owner: &str,
    app_name: &str,
    release_type: &ReleaseSourceType,
    tokens: &AuthorizationTokens,
) -> AxoupdateResult<Vec<Release>> {
    let releases = match release_type {
        #[cfg(feature = "github_releases")]
        ReleaseSourceType::GitHub => {
            github::get_github_releases(name, owner, app_name, &tokens.github).await?
        }
        #[cfg(not(feature = "github_releases"))]
        ReleaseSourceType::GitHub => {
            return Err(AxoupdateError::BackendDisabled {
                backend: "github".to_owned(),
            })
        }
        #[cfg(feature = "axo_releases")]
        ReleaseSourceType::Axo => axodotdev::get_axo_releases(name, owner, app_name).await?,
        #[cfg(not(feature = "axo_releases"))]
        ReleaseSourceType::Axo => {
            return Err(AxoupdateError::BackendDisabled {
                backend: "axodotdev".to_owned(),
            })
        }
    };
    Ok(releases)
}

/// Get the latest stable release
pub(crate) async fn get_latest_stable_release(
    name: &str,
    owner: &str,
    app_name: &str,
    release_type: &ReleaseSourceType,
    tokens: &AuthorizationTokens,
) -> AxoupdateResult<Option<Release>> {
    // GitHub has an API to request the latest stable release.
    // If we're looking up a GitHub release, we can use that.
    // This cuts down on our API requests compared to the paginated release list
    // we do below.
    // Note that abyss has an API for this, but gazenot doesn't expose it yet;
    // we can expand this pattern to Axo Releases in a later release.
    // It's less critical for that path because the rate limits are less of a
    // blocker.
    #[cfg(feature = "github_releases")]
    if release_type == &ReleaseSourceType::GitHub {
        if let Ok(Some(release)) =
            github::get_latest_github_release(name, owner, app_name, &tokens.github).await
        {
            return Ok(Some(release));
        }
    }

    let releases = get_release_list(name, owner, app_name, release_type, tokens).await?;
    Ok(releases
        .into_iter()
        .filter(|r| !r.prerelease)
        .max_by_key(|r| r.version.clone()))
}

/// Get the latest release, allowing for prereleases
pub(crate) async fn get_latest_maybe_prerelease(
    name: &str,
    owner: &str,
    app_name: &str,
    release_type: &ReleaseSourceType,
    tokens: &AuthorizationTokens,
) -> AxoupdateResult<Option<Release>> {
    let releases = get_release_list(name, owner, app_name, release_type, tokens).await?;
    Ok(releases.into_iter().max_by_key(|r| r.version.clone()))
}
