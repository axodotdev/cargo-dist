//! Support for generating CI scripts for running dist

use semver::Version;

use crate::config::v0::CargoDistUrlOverrideRef;

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
fn install_dist_sh_for_version(ver_info: &FullVersionInfo<'_>) -> String {
    if let Some(git) = install_dist_special(ver_info, ShellFlavor::Dash) {
        return git;
    }
    let version = ver_info.version;
    let format = cargo_dist_schema::format_of_version(version);
    let installer_name = if format.unsupported() {
        // FIXME: we should probably do this check way higher up and produce a proper err...
        panic!("requested dist v{version}, which is not supported by this copy of dist ({SELF_DIST_VERSION})");
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
fn install_dist_ps1_for_version(ver_info: &FullVersionInfo<'_>) -> String {
    if let Some(git) = install_dist_special(ver_info, ShellFlavor::Powershell) {
        return git;
    }
    let version = ver_info.version;
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

#[derive(Debug, Clone, Copy)]
enum ShellFlavor {
    Dash,
    Powershell,
}

/// Cute little hack for developing dist itself: if we see a version like "0.0.3-github-config"
/// then install from the main github repo with branch=config!
fn install_dist_special(
    ver_info: &FullVersionInfo<'_>,
    shell_flavor: ShellFlavor,
) -> Option<String> {
    // FIXME: this can be invoked from a "shell" or a "ps1" context: `cargo install` works
    // both ways, but `curl | sh` doesn't.

    if let Some(base_url) = ver_info.url_override.as_ref() {
        // Versions like `0.0.0-dist-https://dl.bearcove.cloud/dump/dist-cross` result in us
        // installing from that server. This is another fast way to iterate on dist itself
        // without having to wait for it to build from source in CI.
        return Some(match shell_flavor {
            ShellFlavor::Dash => format!(
                // note: `INSTALLER_DOWNLOAD_URL` needs to be set for `sh`, _not_ for `curl`
                r#"curl --proto '=https' --tlsv1.2 -LsSf {base_url}/cargo-dist-installer.sh | INSTALLER_DOWNLOAD_URL="{base_url}" sh"#
            ),
            ShellFlavor::Powershell => format!(
                r#"powershell -c "$env:INSTALLER_DOWNLOAD_URL = '{base_url}'; irm {base_url}/cargo-dist-installer.ps1 | iex""#
            ),
        });
    }

    if let Some(branch) = ver_info.version.pre.strip_prefix("github-") {
        return Some(format!("cargo install --git https://github.com/axodotdev/cargo-dist/ --branch={branch} cargo-dist"));
    }
    None
}

/// Gives us the full information re: the version of cargo-dist we're supposed to run
struct FullVersionInfo<'a> {
    version: &'a Version,
    url_override: Option<&'a CargoDistUrlOverrideRef>,
}
