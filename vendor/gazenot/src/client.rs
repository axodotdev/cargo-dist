use std::{env, future::Future, str::FromStr, sync::Arc};

use crate::{
    error::*, AnnouncementKey, ArtifactSet, ArtifactSetId, Owner, PackageName, PublicRelease,
    Release, ReleaseAsset, ReleaseKey, ReleaseList, ReleaseTag, SourceHost, UnparsedUrl,
    UnparsedVersion,
};
use axoasset::reqwest::{
    self,
    header::{HeaderMap, HeaderName, HeaderValue},
    Client, Url,
};
use axoasset::LocalAsset;
use backon::{ExponentialBuilder, Retryable};
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use tokio::sync::{Semaphore, SemaphorePermit};

/// Whether we should default to production or staging
///
/// This is a convenience for easily telling our tools "go into staging mode" for reading/writing
/// test data "properly".
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Deployment {
    /// use production servers
    Production,
    /// use staging servers
    Staging,
}

/// A domain (as in part of a URL)
pub type Domain = String;

/// A client for The Abyss
///
/// This type intentionally does not implement Debug, to avoid leaking authentication secrets.
#[derive(Clone)]
pub struct Gazenot(Arc<GazenotInner>);

#[doc(hidden)]
/// Implementation detail of Gazenot
///
/// DO NOT IMPLEMENT DEBUG ON THIS TYPE, IT CONTAINS SECRET API KEYS AT RUNTIME
pub struct GazenotInner {
    /// Domain for the main abyss API
    api_server: Domain,
    /// Domain where ArtifactSet downloads are GETtable from
    hosting_server: Domain,
    /// Auth for requests
    auth_headers: HeaderMap,
    /// Owner of the project
    owner: Owner,
    /// Name of the project
    source_host: SourceHost,
    /// Are we using staging or prod?
    deployment: Deployment,
    /// reqwest client, must be accessed with the client() method
    _client: Client,
    /// semaphore the manages maximum number of requests
    _semaphore: Semaphore,
}

impl std::ops::Deref for Gazenot {
    type Target = GazenotInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Deserialize, Debug, Clone)]
struct Response<T> {
    success: bool,
    result: Option<T>,
    errors: Option<Vec<String>>,
}

#[derive(Deserialize, Debug, Clone)]
struct BasicResponse {
    success: bool,
    errors: Option<Vec<String>>,
}

#[derive(Deserialize, Debug, Clone)]
struct ArtifactSetResponse {
    public_id: ArtifactSetId,
    set_download_url: Option<UnparsedUrl>,
    upload_url: Option<UnparsedUrl>,
    release_url: Option<UnparsedUrl>,
    announce_url: Option<UnparsedUrl>,
}

#[derive(Serialize, Debug, Clone)]
struct CreateReleaseRequest {
    release: CreateReleaseRequestInner,
}

#[derive(Serialize, Debug, Clone)]
struct CreateReleaseRequestInner {
    artifact_set_id: String,
    tag: ReleaseTag,
    version: UnparsedVersion,
    is_prerelease: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ReleaseResponse {
    release_download_url: Option<UnparsedUrl>,
}

#[derive(Serialize, Debug, Clone)]
struct AnnounceReleaseKey {
    package: PackageName,
    tag: ReleaseTag,
}

#[derive(Serialize, Debug, Clone)]
struct AnnounceReleaseRequest {
    releases: Vec<AnnounceReleaseKey>,
    body: String,
}

#[derive(Deserialize, Debug, Clone)]
struct ListReleasesResponse {
    tag_name: ReleaseTag,
    version: UnparsedVersion,
    name: String,
    body: String,
    prerelease: bool,
    created_at: String,
    assets: Vec<ListReleasesResponseAsset>,
}

#[derive(Deserialize, Debug, Clone)]
struct ListReleasesResponseAsset {
    browser_download_url: String,
    name: String,
    uploaded_at: String,
}

impl Gazenot {
    /// Gaze Not Into The Abyss, Lest You Become A Release Engineer
    ///
    /// This is the vastly superior alias for [`Gazenot::new`].
    pub fn into_the_abyss(
        source_host: impl Into<SourceHost>,
        owner: impl Into<Owner>,
    ) -> Result<Self> {
        Self::new(source_host, owner)
    }

    /// Gaze Not Into My Personal Very Comfortable Abyss
    ///
    /// This is the vastly superior alias for [`Gazenot::new_with_custom_servers`].
    pub fn into_my_custom_abyss(
        source_host: impl Into<SourceHost>,
        owner: impl Into<Owner>,
        api_server: impl Into<Domain>,
        hosting_server: impl Into<Domain>,
    ) -> Result<Self> {
        Self::new_with_custom_servers(source_host, owner, api_server, hosting_server)
    }

    /// Create a new authenticated client for The Abyss
    ///
    /// Authentication requires an Axo Releases Token, whose value
    /// is currently sourced from an AXO_RELEASES_TOKEN environment variable.
    /// It's an error for that variable to not be properly set.
    ///
    /// This is the vastly inferior alias for [`Gazenot::into_the_abyss`].
    ///
    /// See also, [`Gazenot::new_unauthed`].
    pub fn new(source_host: impl Into<SourceHost>, owner: impl Into<Owner>) -> Result<Self> {
        let source_host = source_host.into();
        let owner = owner.into();

        let deployment = deployment();
        let auth_headers = auth_headers(&source_host, &owner, deployment)
            .map_err(|e| GazenotError::new("initializing Abyss authentication", e))?;

        Self::new_with_auth_headers(source_host, owner, deployment, auth_headers, None, None)
    }

    /// Create a new authenticated client for a specific installation of The Abyss
    ///
    /// This is similar to the above, but allows specifying the API and hosting
    /// servers to connect to via parameters. This takes priority over any
    /// servers specified via the environment and over the defaults.
    ///
    /// This is the vastly inferior alias for [`Gazenot::into_my_custom_abyss`].
    ///
    /// See also, [`Gazenot::new_unauthed`].
    pub fn new_with_custom_servers(
        source_host: impl Into<SourceHost>,
        owner: impl Into<Owner>,
        api_server: impl Into<Domain>,
        hosting_server: impl Into<Domain>,
    ) -> Result<Self> {
        let source_host = source_host.into();
        let owner = owner.into();

        let deployment = deployment();
        let auth_headers = auth_headers(&source_host, &owner, deployment)
            .map_err(|e| GazenotError::new("initializing Abyss authentication", e))?;

        Self::new_with_auth_headers(
            source_host,
            owner,
            deployment,
            auth_headers,
            Some(api_server.into()),
            Some(hosting_server.into()),
        )
    }

    /// Create a new client for The Abyss with no authentication
    ///
    /// This creates a client that is only suitable for accessing certain kinds of endpoint, such as:
    ///
    /// * [`Gazenot::list_releases_many``][]
    /// * [`Gazenot::download_artifact_set_url``][]
    pub fn new_unauthed(
        source_host: impl Into<SourceHost>,
        owner: impl Into<Owner>,
    ) -> Result<Self> {
        let deployment = deployment();
        let auth_headers = HeaderMap::new();

        Self::new_with_auth_headers(
            source_host.into(),
            owner.into(),
            deployment,
            auth_headers,
            None,
            None,
        )
    }

    fn new_with_auth_headers(
        source_host: SourceHost,
        owner: Owner,
        deployment: Deployment,
        auth_headers: HeaderMap,
        api_server: Option<String>,
        hosting_server: Option<String>,
    ) -> Result<Self> {
        const DESC: &str = "create http client for axodotdev hosting (abyss)";

        let default_api_server;
        let default_hosting_server;
        let env_api_server;
        let env_hosting_server;
        match deployment {
            Deployment::Production => {
                env_api_server = "GAZENOT_API_SERVER";
                env_hosting_server = "GAZENOT_HOSTING_SERVER";
                default_api_server = "releases.axo.dev";
                default_hosting_server = "artifacts.axodotdev.host";
            }
            Deployment::Staging => {
                env_api_server = "GAZENOT_STAGING_API_SERVER";
                env_hosting_server = "GAZENOT_STAGING_HOSTING_SERVER";
                default_api_server = "staging-axo-abyss.fly.dev";
                // same hosting server, staging affects the url schema
                default_hosting_server = "artifacts.axodotdev.host";
            }
        }

        // Order of preference:
        // 1. specified via args
        // 2. from environment
        // 3. constants
        let api_server = if let Some(server) = api_server {
            server
        } else {
            env::var(env_api_server).unwrap_or(default_api_server.to_owned())
        };
        let hosting_server = if let Some(server) = hosting_server {
            server
        } else {
            env::var(env_hosting_server).unwrap_or(default_hosting_server.to_owned())
        };

        let timeout = std::time::Duration::from_secs(10);
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| GazenotError::new(DESC, e))?;
        // 10 is a reasonable number of maximum concurrent connections
        let semaphore = Semaphore::new(10);

        Ok(Self(Arc::new(GazenotInner {
            api_server,
            hosting_server,
            owner,
            source_host,
            deployment,
            auth_headers,
            _client: client,
            _semaphore: semaphore,
        })))
    }

    /// Ask The Abyss to create new ArtifactSets for the given packages
    pub async fn create_artifact_sets(
        &self,
        packages: impl IntoIterator<Item = PackageName>,
    ) -> Result<Vec<ArtifactSet>> {
        // Spawn all the queries in parallel...
        let mut queries = Vec::new();
        for package in packages {
            // Abyss is just an Arc wrapper around the real client, so Cloning is fine
            let handle = self.clone();
            let desc = format!(
                "create hosting for {}/{}/{}",
                self.source_host, self.owner, package
            );
            let url = self
                .create_artifact_set_url(&package)
                .map_err(|e| GazenotError::new(&desc, e))?;
            queries.push((
                desc,
                url.clone(),
                tokio::spawn(async move { handle.create_artifact_set(url, package).await }),
            ));
        }

        // Then join on them all
        join_all(queries).await
    }

    /// Ask The Abyss to create a new ArtifactSets for the given package
    async fn create_artifact_set(
        &self,
        url: Url,
        package: PackageName,
    ) -> ResultInner<ArtifactSet> {
        let req = || async {
            // No body
            let (_permit, client) = self.client().await;
            let res = client
                .post(url.clone())
                .headers(self.auth_headers.clone())
                .send()
                .await?;
            ResultInner::Ok(res)
        };

        // Send the request, potentially retrying a few times
        let response = retry_request(req).await?;

        // Process the response
        let ArtifactSetResponse {
            public_id,
            set_download_url,
            upload_url,
            release_url,
            announce_url,
        } = process_response(response).await?;

        // Add extra context to make the response more useful in code
        Ok(ArtifactSet {
            package,
            public_id,
            set_download_url,
            upload_url,
            release_url,
            announce_url,
        })
    }

    /// Upload files to several ArtifactSets
    ///
    /// The input is a list of files to upload, but with each file parented
    /// to the ArtifactSet it should be uploaded to.
    ///
    /// This is a bit of an awkward signature, but it lets us handle all the parallelism for you!
    pub async fn upload_files(
        &self,
        files: impl IntoIterator<Item = (&ArtifactSet, Vec<Utf8PathBuf>)>,
    ) -> Result<()> {
        // Spawn all the queries in parallel...
        let mut queries = vec![];
        for (set, sub_files) in files {
            for file in sub_files {
                let handle = self.clone();
                let filename = file.file_name().unwrap();
                let desc = format!(
                    "upload {filename} to hosting for {}/{}/{}",
                    self.source_host, self.owner, set.package
                );
                reject_mock(set).map_err(|e| GazenotError::new(&desc, e))?;
                let url = self
                    .upload_artifact_set_url(set, filename)
                    .map_err(|e| GazenotError::new(&desc, e))?;

                // See comment about serial connections above.
                // Just run one query at a time to be safe.
                queries.push((
                    desc,
                    url.clone(),
                    tokio::spawn(async move { handle.upload_file(url, file).await }),
                ));
            }
        }

        // Then join on them all
        join_all(queries).await?;

        Ok(())
    }

    /// Single file portion of upload_file
    ///
    /// Not exposed as a public because you shouldn't use this directly,
    /// and we might want to rework it.
    async fn upload_file(&self, url: Url, path: Utf8PathBuf) -> ResultInner<()> {
        let req = || async {
            // Load the bytes from disk
            //
            // FIXME: this should be streamed to the request as it's loaded to disk
            let data = LocalAsset::load_bytes(&path)?;

            // Send the bytes
            let (_permit, client) = self.client().await;
            let res = client
                .post(url.clone())
                // Give file uploads a way beefier timeout
                .timeout(std::time::Duration::from_secs(60 * 3))
                .headers(self.auth_headers.clone())
                // FIXME: properly compute the mime-type!
                .header("content-type", "application/octet-stream")
                .body(data)
                .send()
                .await?;
            ResultInner::Ok(res)
        };

        // Send the request, potentially retrying a few times
        let response = retry_request(req).await?;

        process_response_basic(response).await?;

        Ok(())
    }

    /// Create Releases for all the given ArtifactSets
    pub async fn create_releases(
        &self,
        releases: impl IntoIterator<Item = (&ArtifactSet, ReleaseKey)>,
    ) -> Result<Vec<Release>> {
        // Spawn all the queries in parallel...
        let mut queries = Vec::new();
        for (set, key) in releases {
            // Abyss is just an Arc wrapper around the real client, so Cloning is fine
            let handle = self.clone();
            let package = set.package.clone();
            let announce_url = set.announce_url.clone();
            let set_id = set.public_id.clone();
            let desc = format!(
                "create release for {}/{}/{}",
                self.source_host, self.owner, set.package
            );
            reject_mock(set).map_err(|e| GazenotError::new(&desc, e))?;
            let url = self
                .create_release_url(set)
                .map_err(|e| GazenotError::new(&desc, e))?;
            queries.push((
                desc,
                url.clone(),
                tokio::spawn(async move {
                    handle
                        .create_release(url, set_id, package, announce_url, key)
                        .await
                }),
            ));
        }

        // Then join on them all
        join_all(queries).await
    }

    async fn create_release(
        &self,
        url: Url,
        set_id: ArtifactSetId,
        package: PackageName,
        announce_url: Option<UnparsedUrl>,
        release: ReleaseKey,
    ) -> ResultInner<Release> {
        let request = CreateReleaseRequest {
            release: CreateReleaseRequestInner {
                artifact_set_id: set_id,
                tag: release.tag.clone(),
                version: release.version.clone(),
                is_prerelease: release.is_prerelease,
            },
        };

        let req = || async {
            let (_permit, client) = self.client().await;
            let res = client
                .post(url.clone())
                .headers(self.auth_headers.clone())
                .json(&request)
                .send()
                .await?;
            ResultInner::Ok(res)
        };

        // Send the request, potentially retrying a few times
        let response = retry_request(req).await?;

        // Parse the result
        let ReleaseResponse {
            release_download_url,
        } = process_response(response).await?;
        Ok(Release {
            package,
            tag: release.tag,
            release_download_url,
            announce_url,
        })
    }

    pub async fn create_announcements(
        &self,
        releases: impl IntoIterator<Item = &Release>,
        announcement: AnnouncementKey,
    ) -> Result<()> {
        // Sort the releases by owner (this should always select one owner, but hey why not...)
        let releases = releases.into_iter().collect::<Vec<_>>();
        let Some(some_release) = releases.first() else {
            return Ok(());
        };
        let desc = format!(
            "create announcement for {}/{}/{}",
            self.source_host, self.owner, some_release.tag
        );
        let url = self
            .create_announcement_url(some_release)
            .map_err(|e| GazenotError::new(&desc, e))?;

        // Spawn all the queries in parallel... (there's only one lol)
        let mut queries = Vec::new();
        {
            let handle = self.clone();
            let releases = releases
                .iter()
                .map(|r| AnnounceReleaseKey {
                    package: r.package.clone(),
                    tag: r.tag.clone(),
                })
                .collect();
            let announcement = announcement.clone();
            queries.push((
                desc,
                url.clone(),
                tokio::spawn(async move {
                    handle
                        .create_announcement(url, releases, announcement)
                        .await
                }),
            ));
        }

        // Then join on them all
        join_all(queries).await?;
        Ok(())
    }

    async fn create_announcement(
        &self,
        url: Url,
        releases: Vec<AnnounceReleaseKey>,
        announcement: AnnouncementKey,
    ) -> ResultInner<()> {
        let request = AnnounceReleaseRequest {
            releases,
            body: announcement.body,
        };
        let req = || async {
            let (_permit, client) = self.client().await;
            let res = client
                .post(url.clone())
                .headers(self.auth_headers.clone())
                .json(&request)
                .send()
                .await?;
            ResultInner::Ok(res)
        };

        // Send the request, potentially retrying a few times
        let response = retry_request(req).await?;

        process_response_basic(response).await
    }

    /// Ask The Abyss about releases for several packages
    pub async fn list_releases_many(
        &self,
        packages: impl IntoIterator<Item = PackageName>,
    ) -> Result<Vec<ReleaseList>> {
        // Spawn all the queries in parallel...
        let mut queries = Vec::new();
        for package in packages {
            // Abyss is just an Arc wrapper around the real client, so Cloning is fine
            let handle = self.clone();
            let desc = format!(
                "get releases for {}/{}/{}",
                self.source_host, self.owner, package
            );
            let url = self
                .list_releases_url(&package)
                .map_err(|e| GazenotError::new(&desc, e))?;
            queries.push((
                desc,
                url.clone(),
                tokio::spawn(async move { handle.list_releases(url, package).await }),
            ));
        }

        // Then join on them all
        join_all(queries).await
    }

    /// Ask The Abyss about releases
    async fn list_releases(&self, url: Url, package: PackageName) -> ResultInner<ReleaseList> {
        let req = || async {
            // No body
            let (_permit, client) = self.client().await;
            let res = client
                .get(url.clone())
                .headers(self.auth_headers.clone())
                .send()
                .await?;
            ResultInner::Ok(res)
        };

        // Send the request, retrying a few times for server errors
        let response = retry_request(req).await?;

        // Process the response
        let releases: Vec<ListReleasesResponse> = process_response(response).await?;
        let releases = releases
            .into_iter()
            .map(|release| {
                let ListReleasesResponse {
                    tag_name,
                    name,
                    body,
                    prerelease,
                    created_at,
                    assets,
                    version,
                } = release;

                let assets: Vec<ReleaseAsset> = assets
                    .into_iter()
                    .map(|a| ReleaseAsset {
                        name: a.name,
                        uploaded_at: a.uploaded_at,
                        browser_download_url: a.browser_download_url,
                    })
                    .collect();

                PublicRelease {
                    version,
                    tag_name,
                    name,
                    prerelease,
                    created_at,
                    body,
                    assets,
                }
            })
            .collect();

        // Add extra context to make the response more useful in code
        Ok(ReleaseList {
            package_name: package,
            releases,
        })
    }

    pub fn create_artifact_set_url(&self, package: &PackageName) -> ResultInner<Url> {
        // POST /:sourcehost/:owner/:package/artifacts
        let server = &self.api_server;
        let source_host = &self.source_host;
        let owner = &self.owner;
        let url = Url::from_str(&format!(
            "https://{server}/{source_host}/{owner}/{package}/artifacts"
        ))?;
        Ok(url)
    }

    pub fn download_artifact_set_url(&self, set: &ArtifactSet, filename: &str) -> ResultInner<Url> {
        // GET :owner.:hosting_server/:package/:public_id/

        // We don't need a seperate staging server for hosting since we already have production
        // broken up into tenants and it's just a simple CDN. So staging is just distinguished
        // by prefixing usernames thusly.
        let prefix = match self.deployment {
            Deployment::Production => "",
            Deployment::Staging => "staging--",
        };

        let base = set.set_download_url.clone().unwrap_or_else(|| {
            let server = &self.hosting_server;
            let owner = &self.owner;
            let ArtifactSet {
                package, public_id, ..
            } = set;
            format!("https://{prefix}{owner}.{server}/{package}/{public_id}")
        });
        let url = Url::from_str(&format!("{base}/{filename}"))?;
        Ok(url)
    }

    pub fn upload_artifact_set_url(&self, set: &ArtifactSet, filename: &str) -> ResultInner<Url> {
        // POST /:sourcehost/:owner/:package/artifacts/:id/
        let base = set.upload_url.clone().unwrap_or_else(|| {
            let server = &self.api_server;
            let source_host = &self.source_host;
            let owner = &self.owner;
            let ArtifactSet {
                package, public_id, ..
            } = set;
            format!("https://{server}/{source_host}/{owner}/{package}/artifacts/{public_id}/upload")
        });
        let url = Url::from_str(&format!("{base}/{filename}"))?;
        Ok(url)
    }

    pub fn create_release_url(&self, set: &ArtifactSet) -> ResultInner<Url> {
        // POST /:sourcehost/:owner/:package/releases
        let url = set.release_url.clone().unwrap_or_else(|| {
            let server = &self.api_server;
            let source_host = &self.source_host;
            let owner = &self.owner;
            let package = &set.package;
            format!("https://{server}/{source_host}/{owner}/{package}/releases")
        });
        let url = Url::from_str(&url)?;
        Ok(url)
    }

    pub fn create_announcement_url(&self, release: &Release) -> ResultInner<Url> {
        // POST /:sourcehost/:owner/announcements
        let url = release.announce_url.clone().unwrap_or_else(|| {
            let server = &self.api_server;
            let source_host = &self.source_host;
            let owner = &self.owner;
            format!("https://{server}/{source_host}/{owner}/announcements")
        });
        let url = Url::from_str(&url)?;
        Ok(url)
    }

    pub fn list_releases_url(&self, package: &PackageName) -> ResultInner<Url> {
        // GET /:sourcehost/:owner/:projects/releases
        let server = &self.api_server;
        let source_host = &self.source_host;
        let owner = &self.owner;
        let package = &package;
        let url = Url::from_str(&format!(
            "https://{server}/{source_host}/{owner}/{package}/releases"
        ))?;
        Ok(url)
    }
}

impl GazenotInner {
    async fn client(&self) -> (SemaphorePermit, &Client) {
        let permit = self
            ._semaphore
            .acquire()
            .await
            .expect("Gazenot client semaphore closed!?");
        (permit, &self._client)
    }
}

async fn join_all<T>(
    queries: impl IntoIterator<Item = (String, Url, tokio::task::JoinHandle<ResultInner<T>>)>,
) -> Result<Vec<T>> {
    let mut results = Vec::new();
    for (desc, url, query) in queries {
        let result = query
            .await
            .map_err(|e| GazenotError::with_url(&desc, url.to_string(), e))?
            .map_err(|e| GazenotError::with_url(&desc, url.to_string(), e))?;
        results.push(result);
    }
    Ok(results)
}

fn deployment() -> Deployment {
    let prod = env::var("STAGE_INTO_THE_ABYSS")
        .unwrap_or_default()
        .is_empty();
    if prod {
        Deployment::Production
    } else {
        Deployment::Staging
    }
}

fn auth_headers(
    source: &SourceHost,
    owner: &Owner,
    deployment: Deployment,
) -> ResultInner<HeaderMap> {
    // extra-awkard code so you're on your toes and properly treat this like radioactive waste
    // DO NOT UNDER ANY CIRCUMSTANCES PRINT THIS VALUE.
    // DO NOT IMPLEMENT DEBUG ON Abyss OR AbyssInner!!
    let auth = {
        // Intentionally hidden so we only do this here
        let env_var = match deployment {
            Deployment::Production => "AXO_RELEASES_TOKEN",
            Deployment::Staging => "AXO_RELEASES_STAGING_TOKEN",
        };
        // Load from env-var
        let Ok(auth_key) = std::env::var(env_var) else {
            return Err(GazenotErrorInner::AuthKey {
                reason: "could not load env var",
                env_var_name: env_var,
            });
        };
        if auth_key.is_empty() {
            return Err(GazenotErrorInner::AuthKey {
                reason: "no value in env var",
                env_var_name: env_var,
            });
        }
        // Create http header
        let Ok(auth) = HeaderValue::from_str(&format!("Bearer {auth_key}")) else {
            return Err(GazenotErrorInner::AuthKey {
                reason: "had invalid characters for an http header",
                env_var_name: env_var,
            });
        };
        auth
    };

    let id = HeaderValue::from_str(&format!("{source}/{owner}"))?;
    let auth_headers = HeaderMap::from_iter([
        (HeaderName::from_static("authorization"), auth),
        (HeaderName::from_static("x-axo-identifier"), id),
    ]);
    Ok(auth_headers)
}

/// Take some code that builds up and performs a Request
/// and retry it a few times if it's a server error.
async fn retry_request<Fut, FutureFn, R>(request: R) -> ResultInner<reqwest::Response>
where
    Fut: Future<Output = ResultInner<reqwest::Response>>,
    FutureFn: FnMut() -> Fut,
    R: Retryable<ExponentialBuilder, reqwest::Response, GazenotErrorInner, Fut, FutureFn>,
{
    // Defaults to:
    //
    // * jitter: false
    // * factor: 2
    // * min_delay: 1s
    // * max_delay: 60s
    // * max_times: 3
    //
    // (If I understand this correctly, the actual max delay is 8s as a result of the other values,
    // the default is there presumably in case you mess with other params and Do Something Bad).
    let policy = ExponentialBuilder::default();

    let resp = request
        .retry(&policy)
        .when(|e| {
            // Only retry if the server thinks the error is its own fault (500 range)
            let GazenotErrorInner::Reqwest(e) = e else {
                return false;
            };
            let Some(status) = e.status() else {
                return false;
            };
            status.is_server_error()
        })
        .await?;
    Ok(resp)
}

async fn process_response<T: for<'a> Deserialize<'a>>(
    response: reqwest::Response,
) -> ResultInner<T> {
    // don't use status_for_error, we want to try to parse errors!
    let status = response.status();

    // Load the text of the response
    let text = response.text().await?;

    // Try to parse the response as json
    let Ok(parsed): std::result::Result<Response<T>, _> = axoasset::serde_json::de::from_str(&text)
    else {
        // Failed to parse response as json, error out and display whatever text as an error
        let errors = if text.is_empty() {
            vec![]
        } else {
            vec![SimpleError(text.clone())]
        };
        return Err(GazenotErrorInner::ResponseError { status, errors });
    };

    // Only return success if everything agrees
    if parsed.success && status.is_success() {
        if let Some(result) = parsed.result {
            return Ok(result);
        }
    }

    // Otherwise return an error

    // Add extra context if the server is sending us gibberish
    let has_cohesion =
        parsed.success == status.is_success() && parsed.success == parsed.result.is_some();
    let extra_error = if !has_cohesion {
        Some(format!("server response inconsistently reported success -- status: {}, .success: {}, .result.is_some(): {}", status, parsed.success, parsed.result.is_some()))
    } else {
        None
    };

    Err(GazenotErrorInner::ResponseError {
        status,
        errors: parsed
            .errors
            .unwrap_or_default()
            .into_iter()
            .chain(extra_error)
            .map(SimpleError)
            .collect(),
    })
}

async fn process_response_basic(response: reqwest::Response) -> ResultInner<()> {
    // don't use status_for_error, we want to try to parse errors!
    let status = response.status();

    // Load the text of the response
    let text = response.text().await?;

    // Try to parse the response as json
    let Ok(parsed): std::result::Result<BasicResponse, _> =
        axoasset::serde_json::de::from_str(&text)
    else {
        // Failed to parse response as json, error out and display whatever text as an error
        let errors = if text.is_empty() {
            vec![]
        } else {
            vec![SimpleError(text.clone())]
        };
        return Err(GazenotErrorInner::ResponseError { status, errors });
    };

    // Only return success if everything agrees
    if parsed.success && status.is_success() {
        return Ok(());
    }

    // Otherwise return an error

    // Add extra context if the server is sending us gibberish
    let has_cohesion = parsed.success == status.is_success();
    let extra_error = if !has_cohesion {
        Some(format!(
            "server response inconsistently reported success -- status: {}, .success: {}",
            status, parsed.success
        ))
    } else {
        None
    };

    Err(GazenotErrorInner::ResponseError {
        status,
        errors: parsed
            .errors
            .unwrap_or_default()
            .into_iter()
            .chain(extra_error)
            .map(SimpleError)
            .collect(),
    })
}

fn reject_mock(artifact_set: &ArtifactSet) -> ResultInner<()> {
    if artifact_set.is_mock() {
        Err(GazenotErrorInner::IsMocked)
    } else {
        Ok(())
    }
}
