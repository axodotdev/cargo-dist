use std::{
    env::consts::EXE_SUFFIX,
    process::{Command, Output, Stdio},
};

static BIN: &str = env!("CARGO_BIN_EXE_cargo-dist");
const ENV_RUIN_ME: &str = "RUIN_MY_COMPUTER_WITH_INSTALLERS";

#[allow(dead_code)]
mod gallery;
use axoasset::LocalAsset;
use camino::Utf8PathBuf;
use gallery::*;

fn format_outputs(output: &Output) -> String {
    let stdout = std::str::from_utf8(&output.stdout).unwrap();
    let stderr = std::str::from_utf8(&output.stderr).unwrap();
    format!("stdout:\n{stdout}\nstderr:\n{stderr}")
}

#[test]
fn test_version() {
    let output = Command::new(BIN)
        .arg("dist")
        .arg("-V")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(output.status.success(), "{}", stderr);
    assert_eq!(stderr, "");

    let (name, ver) = stdout.split_once(' ').unwrap();
    assert_eq!(name, "cargo-dist");
    let mut ver_parts = ver.trim().split('.');
    ver_parts.next().unwrap().parse::<u8>().unwrap();
    ver_parts.next().unwrap().parse::<u8>().unwrap();
    let last = ver_parts.next().unwrap();
    if let Some((last, _prerelease)) = last.split_once('-') {
        last.parse::<u8>().unwrap();
        if let Some(build) = ver_parts.next() {
            build.parse::<u8>().unwrap();
        }
    } else {
        last.parse::<u8>().unwrap();
    }
    assert!(ver_parts.next().is_none());
}

#[test]
fn test_long_help() {
    let output = Command::new(BIN)
        .arg("dist")
        .arg("--help")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    snapshot_settings().bind(|| {
        insta::assert_snapshot!("long-help", format_outputs(&output));
    });
    assert!(output.status.success(), "{}", output.status);
}

#[test]
fn test_short_help() {
    let output = Command::new(BIN)
        .arg("dist")
        .arg("-h")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    snapshot_settings().bind(|| {
        insta::assert_snapshot!("short-help", format_outputs(&output));
    });
    assert!(output.status.success(), "{}", output.status);
}

#[test]
fn test_manifest() {
    let output = Command::new(BIN)
        .arg("dist")
        .arg("manifest")
        .arg("--artifacts=lies")
        .arg("--no-local-paths")
        .arg("--allow-dirty")
        .arg("--output-format=json")
        .arg("--verbose=error")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    // We don't want this to churn every time we do a version bump
    snapshot_settings_with_dist_manifest_filter().bind(|| {
        insta::assert_snapshot!(format_outputs(&output));
    });

    assert!(output.status.success(), "{}", output.status);
}

#[test]
fn test_lib_manifest() {
    let version = std::env!("CARGO_PKG_VERSION");
    let output = Command::new(BIN)
        .arg("dist")
        .arg("manifest")
        .arg("--artifacts=all")
        .arg("--no-local-paths")
        .arg("--allow-dirty")
        .arg("--output-format=json")
        .arg("--verbose=error")
        .arg("--tag")
        .arg(&format!("cargo-dist-schema-v{}", version))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    // We don't want this to churn every time we do a version bump
    snapshot_settings_with_dist_manifest_filter().bind(|| {
        insta::assert_snapshot!(format_outputs(&output));
    });

    assert!(output.status.success(), "{}", output.status);
}

#[test]
fn test_lib_manifest_slash() {
    let version = std::env!("CARGO_PKG_VERSION");
    let output = Command::new(BIN)
        .arg("dist")
        .arg("manifest")
        .arg("--artifacts=all")
        .arg("--no-local-paths")
        .arg("--allow-dirty")
        .arg("--output-format=json")
        .arg("--verbose=error")
        .arg("--tag")
        .arg(&format!("cargo-dist-schema/v{}", version))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    // We don't want this to churn every time we do a version bump
    snapshot_settings_with_dist_manifest_filter().bind(|| {
        insta::assert_snapshot!(format_outputs(&output));
    });

    assert!(output.status.success(), "{}", output.status);
}

#[test]
fn test_error_manifest() {
    let output = Command::new(BIN)
        .arg("dist")
        .arg("manifest")
        .arg("--artifacts=all")
        .arg("--no-local-paths")
        .arg("--allow-dirty")
        .arg("--output-format=json")
        .arg("--verbose=error")
        .arg("--tag=v0.0.0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    // We don't want this to churn every time we do a version bump
    snapshot_settings_with_dist_manifest_filter().bind(|| {
        insta::assert_snapshot!(format_outputs(&output));
    });

    assert!(!output.status.success(), "{}", output.status);
}

#[test]
fn test_markdown_help() {
    let output = Command::new(BIN)
        .arg("dist")
        .arg("help-markdown")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    snapshot_settings().bind(|| {
        insta::assert_snapshot!("markdown-help", format_outputs(&output));
    });
    assert!(output.status.success(), "{}", output.status);
}

static RECEIPT_TEMPLATE: &str = r#"{"binaries":["cargo-dist"],"install_prefix":"INSTALL_PREFIX","provider":{"source":"cargo-dist","version":"0.10.0-prerelease.1"},"source":{"app_name":"cargo-dist","name":"cargo-dist","owner":"axodotdev","release_type":"github"},"version":"VERSION"}"#;

fn install_receipt(version: &str, prefix: &Utf8PathBuf) -> String {
    RECEIPT_TEMPLATE
        .replace("INSTALL_PREFIX", &prefix.to_string().replace('\\', "\\\\"))
        .replace("VERSION", version)
}

fn write_receipt(version: &str, prefix: &Utf8PathBuf, config_path: &Utf8PathBuf) {
    let contents = install_receipt(version, prefix);
    let receipt_name = config_path.join("cargo-dist-receipt.json");
    LocalAsset::write_new_all(&contents, receipt_name).unwrap();
}

#[test]
fn test_self_update() {
    // Only do this if RUIN_MY_COMPUTER_WITH_INSTALLERS is set
    if std::env::var(ENV_RUIN_ME)
        .map(|s| !s.is_empty())
        .unwrap_or(false)
    {
        let dist_home = Utf8PathBuf::from_path_buf(
            homedir::get_my_home()
                .unwrap()
                .unwrap()
                .join(".cargo")
                .join("bin"),
        )
        .unwrap();
        let dist_path = &dist_home.join(format!("cargo-dist{}", EXE_SUFFIX));

        #[cfg(target_family = "unix")]
        let config_path = Utf8PathBuf::from_path_buf(
            homedir::get_my_home()
                .unwrap()
                .unwrap()
                .join(".config")
                .join("cargo-dist"),
        )
        .unwrap();
        #[cfg(target_family = "windows")]
        let config_path = Utf8PathBuf::from_path_buf(
            std::env::var("LOCALAPPDATA")
                .map(std::path::PathBuf::from)
                .unwrap()
                .join("cargo-dist"),
        )
        .unwrap();

        // Ensure we delete any previous copy that may exist
        // at this path before we copy in our version.
        if dist_path.exists() {
            std::fs::remove_file(dist_path).unwrap();
        }
        assert!(!dist_path.exists());

        // Install to the home directory
        std::fs::copy(BIN, dist_path).unwrap();

        // Create a fake install receipt
        // We lie about being a very old version so we always
        // consider upgrading to something.
        write_receipt("0.5.0", &dist_home, &config_path);

        let output = Command::new(dist_path)
            .arg("dist")
            .arg("update")
            // init includes interactive components, so we
            // can't safely run it within a noninteractive test
            .arg("--skip-init")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap();

        let out_str = String::from_utf8_lossy(&output.stdout);
        let err_str = String::from_utf8_lossy(&output.stderr);

        assert!(
            output.status.success(),
            "status code: {}, stdout: {out_str}; stderr: {err_str}",
            output.status
        );
    }
}
