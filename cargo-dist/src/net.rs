//! Centralized logic for initializing http clients to
//! ensure uniform configuration.

use crate::errors::DistResult;

/// Settings for http clients
///
/// Any settings that should apply to all http requests should
/// be stored here, to avoid different configurations.
#[derive(Debug, Clone)]
pub struct ClientSettings {
    /// Whether to prefer native certs over webpki ones
    pub prefer_native_tls: bool,
}

impl ClientSettings {
    /// Create new ClientSettings using all necessary values
    pub fn new(prefer_native_tls: bool) -> Self {
        // TODO: load env-vars here?
        Self { prefer_native_tls }
    }
}

/// Create a raw reqwest client
///
/// As of this writing this shouldn't be used/exposed, as we'd prefer
/// to avoid proliferating random http clients. For now AxoClient
/// is sufficient.
fn create_reqwest_client(
    ClientSettings { prefer_native_tls }: &ClientSettings,
) -> DistResult<reqwest::Client> {
    // TODO: add a proper error instead of calling `expect`?
    let client = reqwest::Client::builder()
        .tls_built_in_webpki_certs(!prefer_native_tls)
        .tls_built_in_native_certs(*prefer_native_tls)
        .build()
        .expect("failed to intitalize http client");
    Ok(client)
}

/// Create an AxoClient
///
/// Ideally this should be called only once and reused!
pub fn create_axoasset_client(settings: &ClientSettings) -> DistResult<axoasset::AxoClient> {
    let client = create_reqwest_client(settings)?;
    Ok(axoasset::AxoClient::with_reqwest(client))
}

/// Create a Gazenot client
///
/// Gazenot clients are configured to particular sourcehosts, and creating
/// one will error out if certain environment variables aren't set. As such,
/// this should be called in a fairly lazy/latebound way -- only when we know
/// for sure we HAVE to do gazenot http requests.
pub fn create_gazenot_client(
    ClientSettings {
        prefer_native_tls: _,
    }: &ClientSettings,
    source_host: &str,
    owner: &str,
) -> DistResult<gazenot::Gazenot> {
    // TODO: add an API to gazenot for passing this setting in!
    let client = gazenot::Gazenot::into_the_abyss(source_host, owner)?;
    Ok(client)
}
