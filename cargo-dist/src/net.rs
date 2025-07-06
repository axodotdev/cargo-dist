//! Centralized logic for initializing http clients to
//! ensure uniform configuration.

use crate::errors::DistResult;
use axoasset::reqwest;

/// Settings for http clients
///
/// Any settings that should apply to all http requests should
/// be stored here, to avoid different configurations.
#[derive(Debug, Clone, Default)]
pub struct ClientSettings {}

impl ClientSettings {
    /// Create new ClientSettings using all necessary values
    pub fn new() -> Self {
        Self::default()
    }
}

/// Create a raw reqwest client
///
/// As of this writing this shouldn't be used/exposed, as we'd prefer
/// to avoid proliferating random http clients. For now AxoClient
/// is sufficient.
fn create_reqwest_client(ClientSettings {}: &ClientSettings) -> DistResult<reqwest::Client> {
    let client = reqwest::Client::builder()
        .build()
        .expect("failed to initialize http client");
    Ok(client)
}

/// Create an AxoClient
///
/// Ideally this should be called only once and reused!
pub fn create_axoasset_client(settings: &ClientSettings) -> DistResult<axoasset::AxoClient> {
    let client = create_reqwest_client(settings)?;
    Ok(axoasset::AxoClient::with_reqwest(client))
}
