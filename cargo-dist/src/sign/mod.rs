//! Code/artifact signing support

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use axoasset::AxoClient;
use camino::Utf8Path;

use crate::{config::ProductionMode, DistResult, TargetTriple};

pub mod macos;
mod ssldotcom;

/// Code/artifact signing providers
#[derive(Debug)]
pub struct Signing {
    macos: Option<macos::Codesign>,
    ssldotcom: Option<ssldotcom::CodeSignTool>,
}

impl Signing {
    /// Setup signing
    pub fn new(
        client: &AxoClient,
        host_target: &TargetTriple,
        dist_dir: &Utf8Path,
        ssldotcom_windows_sign: Option<ProductionMode>,
        macos_sign: bool,
    ) -> DistResult<Self> {
        let ssldotcom =
            ssldotcom::CodeSignTool::new(client, host_target, dist_dir, ssldotcom_windows_sign)?;
        let macos = if macos_sign {
            macos::Codesign::new(host_target)?
        } else {
            None
        };
        Ok(Self { macos, ssldotcom })
    }

    /// Sign a file
    pub fn sign(&self, file: &Utf8Path) -> DistResult<()> {
        if let Some(signer) = &self.ssldotcom {
            let extension = file.extension().unwrap_or_default();
            if let "exe" | "msi" | "ps1" = extension {
                signer.sign(file)?;
            }
        }
        if let Some(signer) = &self.macos {
            // TODO: restructure, this is just to keep Windows
            // from flagging dead code
            #[cfg(unix)]
            let is_executable = file.metadata()?.permissions().mode() & 0o111 != 0;
            #[cfg(windows)]
            let is_executable = true;

            // At the moment, we're exclusively signing executables.
            // In the future, we may need to sign app bundles (which are
            // directories) or certain other metadata files.
            if file.is_file() && is_executable {
                signer.sign(file)?;
            }
        }
        Ok(())
    }
}
