//! Code/artifact signing support

use axoasset::AxoClient;
use camino::Utf8Path;

use crate::{config::ProductionMode, DistResult, TargetTriple};

mod ssldotcom;

/// Code/artifact signing providers
#[derive(Debug)]
pub struct Signing {
    ssldotcom: Option<ssldotcom::CodeSignTool>,
}

impl Signing {
    /// Setup signing
    pub fn new(
        client: &AxoClient,
        host_target: &TargetTriple,
        dist_dir: &Utf8Path,
        ssldotcom_windows_sign: Option<ProductionMode>,
    ) -> DistResult<Self> {
        let ssldotcom =
            ssldotcom::CodeSignTool::new(client, host_target, dist_dir, ssldotcom_windows_sign)?;
        Ok(Self { ssldotcom })
    }

    /// Sign a file
    pub fn sign(&self, file: &Utf8Path) -> DistResult<()> {
        if let Some(signer) = &self.ssldotcom {
            let extension = file.extension().unwrap_or_default();
            if let "exe" | "msi" | "ps1" = extension {
                signer.sign(file)?;
            }
        }
        Ok(())
    }
}
