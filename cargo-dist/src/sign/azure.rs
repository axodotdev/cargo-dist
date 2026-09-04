//! Codesigning using Azure Artifact Signing.

use axoprocess::Cmd;
use camino::Utf8Path;
use cargo_dist_schema::TripleNameRef;
use tracing::info;

use crate::config::AzureArtifactSigningConfig;
use crate::errors::*;
use crate::platform::targets::TARGET_X64_WINDOWS;

const ENDPOINT_ENV: &str = "CARGO_DIST_AZURE_SIGNING_ENDPOINT";
const ACCOUNT_NAME_ENV: &str = "CARGO_DIST_AZURE_SIGNING_ACCOUNT_NAME";
const CERTIFICATE_PROFILE_NAME_ENV: &str = "CARGO_DIST_AZURE_SIGNING_CERTIFICATE_PROFILE_NAME";

/// An instance of Azure Artifact Signing.
#[derive(Debug)]
pub struct AzureArtifactSigning {
    config: AzureArtifactSigningConfig,
}

impl AzureArtifactSigning {
    pub fn new(
        host_target: &TripleNameRef,
        config: Option<AzureArtifactSigningConfig>,
    ) -> DistResult<Option<Self>> {
        let Some(config) = config else {
            return Ok(None);
        };

        let settings = [
            ("endpoint", &config.endpoint),
            ("account-name", &config.account_name),
            ("certificate-profile-name", &config.certificate_profile_name),
        ]
        .into_iter()
        .filter_map(|(name, value)| value.trim().is_empty().then_some(name))
        .collect::<Vec<_>>();
        if !settings.is_empty() {
            return Err(DistError::InvalidAzureArtifactSigningConfig { settings });
        }

        // Azure Artifact Signing's supported GitHub-hosted runner path is x64 Windows.
        if host_target != TARGET_X64_WINDOWS {
            return Ok(None);
        }

        Ok(Some(Self { config }))
    }

    pub fn sign(&self, file: &Utf8Path) -> DistResult<()> {
        info!("Azure Artifact Signing {file}");

        let AzureArtifactSigningConfig {
            endpoint,
            account_name,
            certificate_profile_name,
        } = &self.config;

        // Match Azure's official GitHub Action integration, which wraps this module:
        // https://github.com/Azure/artifact-signing-action
        let script = r#"
$ErrorActionPreference = 'Stop'
Import-Module ArtifactSigning
Invoke-ArtifactSigning `
    -Endpoint $env:CARGO_DIST_AZURE_SIGNING_ENDPOINT `
    -CodeSigningAccountName $env:CARGO_DIST_AZURE_SIGNING_ACCOUNT_NAME `
    -CertificateProfileName $env:CARGO_DIST_AZURE_SIGNING_CERTIFICATE_PROFILE_NAME `
    -Files $env:CARGO_DIST_SIGN_FILE `
    -FileDigest SHA256 `
    -TimestampRfc3161 'http://timestamp.acs.microsoft.com' `
    -TimestampDigest SHA256
"#;

        Cmd::new("pwsh", "sign windows artifacts")
            .arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-Command")
            .arg(script)
            .env(ENDPOINT_ENV, endpoint)
            .env(ACCOUNT_NAME_ENV, account_name)
            .env(CERTIFICATE_PROFILE_NAME_ENV, certificate_profile_name)
            .env("CARGO_DIST_SIGN_FILE", file.as_str())
            .stdout_to_stderr()
            .status()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_blank_config_fields() {
        let error = AzureArtifactSigning::new(
            TARGET_X64_WINDOWS,
            Some(AzureArtifactSigningConfig {
                endpoint: " ".into(),
                account_name: "account".into(),
                certificate_profile_name: "\t".into(),
            }),
        )
        .unwrap_err();

        let DistError::InvalidAzureArtifactSigningConfig { settings } = error else {
            panic!("unexpected error: {error}");
        };
        assert_eq!(settings, ["endpoint", "certificate-profile-name"]);
    }
}
