//! Support for generating CI scripts for running dist

use dist_schema::{
    target_lexicon::{OperatingSystem, Triple},
    DashScript, GhaRunStep, PowershellScript,
};
use semver::Version;
use serde::Serialize;

use crate::config::v0::CargoDistUrlOverrideRef;

use self::github::GithubCiInfo;

pub mod github;

/// The current version of dist
const SELF_DIST_VERSION: &str = env!("CARGO_PKG_VERSION");
const BASE_DIST_FETCH_URL: &str = "https://github.com/oxidecomputer/cargo-dist/releases/download";

// NOTE: This is hard-coded to download latest.
const BASE_CARGO_AUDITABLE_FETCH_LATEST_URL: &str =
    "https://github.com/rust-secure-code/cargo-auditable/releases/latest/download";

const BASE_CARGO_CYCLONEDX_FETCH_URL: &str =
    "https://github.com/CycloneDX/cyclonedx-rust-cargo/releases/download";

// NOTE: This is hard-coded to a specific version because both cargo-cyclonedx
//       and cyclonedx-bom are released on the same repo.
//       This means the "latest" release is sometimes NOT actually cargo-cyclonedx!
const CARGO_CYCLONEDX_VERSION: &str = "0.5.5";

const BASE_OMNIBOR_FETCH_URL: &str = "https://github.com/omnibor/omnibor-rs/releases/download";

// NOTE: This is hard-coded to a specific version because omnibor-cli,
//       omnibor-rs, and gitoid are released on the same repo.
//       This means the "latest" release is sometimes NOT actually omnibor-cli!
//
// SEE ALSO: .github/workflows/ci.yml
const OMNIBOR_VERSION: &str = "0.7.0";

/// Info about all the enabled CI backends
#[derive(Debug, Default)]
pub struct CiInfo {
    /// Github CI
    pub github: Option<GithubCiInfo>,
}

/// Gives us the full information re: the version of dist we're supposed
/// to install/run in CI
struct DistInstallSettings<'a> {
    version: &'a Version,
    url_override: Option<&'a CargoDistUrlOverrideRef>,
}

/// Generates github steps to install a tool
pub trait InstallStrategy {
    /// Return a sh/dash script
    fn dash(&self) -> GhaRunStep;

    /// Return a powershell script
    fn powershell(&self) -> GhaRunStep;

    /// Return the right install method for a given set of targets
    fn for_triple(&self, triple: &Triple) -> GhaRunStep {
        match triple.operating_system {
            OperatingSystem::Linux | OperatingSystem::Darwin => self.dash(),
            OperatingSystem::Windows => self.powershell(),
            _ => panic!("unsupported host triple {triple}"),
        }
    }
}

/// The strategy used to install dist in CI
#[derive(Debug, Clone, Serialize)]
pub enum DistInstallStrategy {
    /// Download an installer and run it
    Installer {
        /// the prefix of the installer url, e.g.
        installer_url: String,
        /// the installer name, without `.sh` or `.ps1`
        installer_name: String,
    },
    /// Run `cargo install --git` (slow!)
    GitBranch {
        /// the branch to install from — from <https://github.com/axodotdev/cargo-dist>
        branch: String,
    },
}

impl DistInstallSettings<'_> {
    fn install_strategy(&self) -> DistInstallStrategy {
        if let Some(branch) = self.version.pre.strip_prefix("github-") {
            return DistInstallStrategy::GitBranch {
                branch: branch.to_owned(),
            };
        }

        if let Some(url) = self.url_override.as_ref() {
            return DistInstallStrategy::Installer {
                installer_url: url.as_str().to_owned(),
                installer_name: "cargo-dist-installer".to_owned(),
            };
        }

        let version = self.version;
        let format = dist_schema::format_of_version(version);
        let installer_name = if format.unsupported() {
            // FIXME: we should probably do this check way higher up and produce a proper err...
            panic!("requested dist v{version}, which is not supported by the this copy of dist ({SELF_DIST_VERSION})");
        } else if format.artifact_names_contain_versions() {
            format!("cargo-dist-v{version}-installer")
        } else {
            "cargo-dist-installer".to_owned()
        };

        DistInstallStrategy::Installer {
            // FIXME: it would be nice if these values were somehow using all the machinery
            // to compute these values for packages we build *BUT* it's messy and not that important
            installer_url: format!("{BASE_DIST_FETCH_URL}/v{version}"),
            installer_name,
        }
    }
}

impl InstallStrategy for DistInstallStrategy {
    /// Returns a bit of sh/dash to install the requested version of dist
    fn dash(&self) -> GhaRunStep {
        DashScript::new(match self {
            DistInstallStrategy::Installer { installer_url, installer_name } => format!(
                "curl --proto '=https' --tlsv1.2 -LsSf {installer_url}/{installer_name}.sh | sh"
            ),
            DistInstallStrategy::GitBranch { branch } => format!(
                "cargo install --git https://github.com/oxidecomputer/cargo-dist/ --branch={branch} cargo-dist"
            ),
        }).into()
    }

    /// Returns a bit of powershell to install the requested version of dist
    fn powershell(&self) -> GhaRunStep {
        PowershellScript::new(match self {
            DistInstallStrategy::Installer { installer_url, installer_name } => format!(
                "irm {installer_url}/{installer_name}.ps1 | iex"
            ),
            DistInstallStrategy::GitBranch { branch } => format!(
                "cargo install --git https://github.com/oxidecomputer/cargo-dist/ --branch={branch} cargo-dist"
            ),
        }).into()
    }
}

struct CargoAuditableInstallStrategy;

impl InstallStrategy for CargoAuditableInstallStrategy {
    /// Return a sh/dash script
    fn dash(&self) -> GhaRunStep {
        let installer_url =
            format!("{BASE_CARGO_AUDITABLE_FETCH_LATEST_URL}/cargo-auditable-installer.sh");
        DashScript::new(format!(
            "curl --proto '=https' --tlsv1.2 -LsSf {installer_url} | sh"
        ))
        .into()
    }

    /// Return a powershell script
    fn powershell(&self) -> GhaRunStep {
        let installer_url =
            format!("{BASE_CARGO_AUDITABLE_FETCH_LATEST_URL}/cargo-auditable-installer.ps1");
        PowershellScript::new(format!(r#"powershell -c "irm {installer_url} | iex""#)).into()
    }
}

struct CargoCyclonedxInstallStrategy;

impl InstallStrategy for CargoCyclonedxInstallStrategy {
    /// Return an sh/dash script to install cargo-cyclonedx
    fn dash(&self) -> GhaRunStep {
        let installer_url =
            format!("{BASE_CARGO_CYCLONEDX_FETCH_URL}/cargo-cyclonedx-{CARGO_CYCLONEDX_VERSION}/cargo-cyclonedx-installer.sh");
        DashScript::new(format!(
            "curl --proto '=https' --tlsv1.2 -LsSf {installer_url} | sh"
        ))
        .into()
    }

    /// Return a powershell script to install cargo-cyclonedx.
    /// This probably isn't being used.
    fn powershell(&self) -> GhaRunStep {
        let installer_url =
            format!("{BASE_CARGO_CYCLONEDX_FETCH_URL}/cargo-cyclonedx-{CARGO_CYCLONEDX_VERSION}/cargo-cyclonedx-installer.ps1");
        PowershellScript::new(format!(r#"powershell -c "irm {installer_url} | iex""#)).into()
    }
}

struct OmniborInstallStrategy;

impl InstallStrategy for OmniborInstallStrategy {
    /// Return an sh/dash script to install cargo-cyclonedx
    fn dash(&self) -> GhaRunStep {
        let installer_url = format!(
            "{BASE_OMNIBOR_FETCH_URL}/omnibor-cli-v{OMNIBOR_VERSION}/omnibor-cli-installer.sh"
        );
        DashScript::new(format!(
            "curl --proto '=https' --tlsv1.2 -LsSf {installer_url} | sh"
        ))
        .into()
    }

    /// Return a powershell script to install cargo-cyclonedx.
    /// This probably isn't being used.
    fn powershell(&self) -> GhaRunStep {
        let installer_url = format!(
            "{BASE_OMNIBOR_FETCH_URL}/omnibor-cli-v{OMNIBOR_VERSION}/omnibor-cli-installer.ps1"
        );
        PowershellScript::new(format!(r#"powershell -c "irm {installer_url} | iex""#)).into()
    }
}
