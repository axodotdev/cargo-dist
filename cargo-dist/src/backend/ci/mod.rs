//! Support for generating CI scripts for running dist

use semver::Version;

use self::github::GithubCiInfo;

pub mod github;

/// The current version of dist
const SELF_DIST_VERSION: &str = env!("CARGO_PKG_VERSION");
const BASE_DIST_FETCH_URL: &str = "https://github.com/axodotdev/cargo-dist/releases/download";

/// Info about all the enabled CI backends
#[derive(Debug, Default)]
pub struct CiInfo {
    /// Github CI
    pub github: Option<GithubCiInfo>,
}

/// Get the command to invoke to install dist via sh script
fn install_dist_sh_for_version(version: &Version) -> String {
    if let Some(git) = install_dist_git(version) {
        return git;
    }
    let format = cargo_dist_schema::format_of_version(version);
    let installer_name = if format.unsupported() {
        // FIXME: we should probably do this check way higher up and produce a proper err...
        panic!("requested dist v{version}, which is not supported by the this copy of dist ({SELF_DIST_VERSION})");
    } else if format.artifact_names_contain_versions() {
        format!("cargo-dist-v{version}-installer.sh")
    } else {
        "cargo-dist-installer.sh".to_owned()
    };

    // FIXME: it would be nice if these values were somehow using all the machinery
    // to compute these values for packages we build *BUT* it's messy and not that important
    let installer_url = format!("{BASE_DIST_FETCH_URL}/v{version}/{installer_name}");
    format!("curl --proto '=https' --tlsv1.2 -LsSf {installer_url} | sh")
}

/// Get the command to invoke to install dist via ps1 script
fn install_dist_ps1_for_version(version: &Version) -> String {
    if let Some(git) = install_dist_git(version) {
        return git;
    }
    let format = cargo_dist_schema::format_of_version(version);
    let installer_name = if format.unsupported() {
        // FIXME: we should probably do this check way higher up and produce a proper err...
        panic!("requested dist v{version}, which is not supported by the this copy of dist ({SELF_DIST_VERSION})");
    } else if format.artifact_names_contain_versions() {
        format!("cargo-dist-v{version}-installer.ps1")
    } else {
        "cargo-dist-installer.ps1".to_owned()
    };

    // FIXME: it would be nice if these values were somehow using all the machinery
    // to compute these values for packages we build *BUT* it's messy and not that important
    let installer_url = format!("{BASE_DIST_FETCH_URL}/v{version}/{installer_name}");
    format!(r#"powershell -c "irm {installer_url} | iex""#)
}

/// Cute little hack for developing dist itself: if we see a version like "0.0.3-github-config"
/// then install from the main github repo with branch=config!
fn install_dist_git(version: &Version) -> Option<String> {
    version.pre.strip_prefix("github-").map(|branch| {
        format!("cargo install --git https://github.com/axodotdev/cargo-dist/ --branch={branch} cargo-dist")
    })
}
