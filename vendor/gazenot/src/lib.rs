//! Gaze Not Into The Abyss, Lest You Become A Release Engineer.
//!
//! Gazenot is a client library for accessing the Abyss service,
//! which hosts Releases of various Packages (apps).
//!
#![cfg_attr(feature = "client_lib", doc = include_str!("../example.md"))]
#[cfg(feature = "client_lib")]
mod client;
#[cfg(feature = "client_lib")]
pub mod error;
#[cfg(feature = "client_lib")]
pub use client::Gazenot;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The public_id of an ArtifactSet
pub type ArtifactSetId = String;
/// The owner of a package
pub type Owner = String;
/// The source hosting provider (e.g. "github")
pub type SourceHost = String;
/// The name of a package
pub type PackageName = String;
/// The tag for a Release
pub type ReleaseTag = String;
/// An unparsed URL
pub type UnparsedUrl = String;
/// An unparsed SemVer Version
pub type UnparsedVersion = String;

/// A handle for talking about ArtifactSets
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct ArtifactSet {
    pub package: PackageName,
    pub public_id: ArtifactSetId,

    pub set_download_url: Option<UnparsedUrl>,
    pub upload_url: Option<UnparsedUrl>,
    pub release_url: Option<UnparsedUrl>,
    pub announce_url: Option<UnparsedUrl>,
}

pub const MOCK_ARTIFACT_SET_PUBLIC_ID: &str = "fake-id-do-not-upload";

impl ArtifactSet {
    pub fn new(package: String, public_id: ArtifactSetId) -> Self {
        Self {
            package,
            public_id,
            set_download_url: None,
            upload_url: None,
            release_url: None,
            announce_url: None,
        }
    }

    /// Create a mock ArtifactSet that can be used for internal consistency checks
    /// without hitting the server.
    ///
    /// Also can be used for tests.
    pub fn mock(package: String) -> Self {
        // This URL is gibberish but it needs to exist for some things
        let set_download_url = Some(format!(
            "https://fake.axo.dev/faker/{package}/{MOCK_ARTIFACT_SET_PUBLIC_ID}"
        ));
        Self {
            package,
            public_id: MOCK_ARTIFACT_SET_PUBLIC_ID.to_owned(),
            set_download_url,
            upload_url: None,
            release_url: None,
            announce_url: None,
        }
    }

    pub fn is_mock(&self) -> bool {
        self.public_id == MOCK_ARTIFACT_SET_PUBLIC_ID
    }

    pub fn to_release(&self, tag: ReleaseTag) -> Release {
        Release {
            package: self.package.clone(),
            tag,
            announce_url: self.announce_url.clone(),
            release_download_url: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct Release {
    pub package: PackageName,
    pub tag: ReleaseTag,
    pub release_download_url: Option<UnparsedUrl>,
    pub announce_url: Option<UnparsedUrl>,
}

impl Release {
    pub fn new(package: String, tag: ReleaseTag) -> Self {
        Self {
            package,
            tag,
            release_download_url: None,
            announce_url: None,
        }
    }
}

/// Info needed to create a release
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone)]
pub struct ReleaseKey {
    /// Git tag for the release, this is the "primary key" of the release
    ///
    /// Strictly speaking this is the only value that needs to be sent,
    /// the other fields can be computed for it. *However* we require clients
    /// to send the redundant fields (and have the sever check them) to avoid
    /// issues where clients and servers desync on semantics.
    ///
    /// cargo-dist and The Abyss are ideally kept in sync by both using axotag, but
    /// it's a REST API so nothing prevents other people from using it!
    pub tag: ReleaseTag,
    /// Version of the package
    ///
    /// This must agree with the tag, the server will check it with axotag.
    pub version: UnparsedVersion,

    /// Whether this release should be considered a prerelease
    ///
    /// This must agree with the tag, the server will check it with axotag.
    pub is_prerelease: bool,
}

/// Info needed to create an announement
#[derive(Debug, Clone)]
pub struct AnnouncementKey {
    /// Markdown to be rendered for the announcement.
    ///
    /// It should start with the title of the announcement (e.g. "# My Project v1.0.0")
    pub body: String,
}

/// A listing of the releases for a package
#[derive(Debug, Clone)]
pub struct ReleaseList {
    /// Name of the package
    pub package_name: PackageName,
    /// The list of releases
    pub releases: Vec<PublicRelease>,
}

/// A release that has been announced.
#[derive(Debug, Clone)]
pub struct PublicRelease {
    /// Name of the release
    pub name: String,
    /// Tag name used for the release
    pub tag_name: ReleaseTag,
    /// Version of the release
    pub version: UnparsedVersion,
    /// Body of the release announcement
    pub body: String,
    /// Whether the release is considered a prerelease
    pub prerelease: bool,
    /// Timestamp when the release was announced
    /// TODO: Verify this is actually the _announcement_ timestamp
    pub created_at: String,
    /// List of assets associated with the release
    pub assets: Vec<ReleaseAsset>,
}

/// A single release artifact/asset
#[derive(Debug, Clone)]
pub struct ReleaseAsset {
    /// The URL that can be used to download this package
    pub browser_download_url: String,
    /// The filename of the asset
    pub name: String,
    /// The date it was uploaded and attached to the artifact set
    pub uploaded_at: String,
}
