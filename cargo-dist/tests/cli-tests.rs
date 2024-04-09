use std::{
    path::PathBuf,
    process::{Command, Output, Stdio},
};

static BIN: &str = env!("CARGO_BIN_EXE_cargo-dist");
const ENV_RUIN_ME: &str = "RUIN_MY_COMPUTER_WITH_INSTALLERS";

#[allow(dead_code)]
mod gallery;
use axoupdater::{test::helpers::RuntestArgs, ReleaseSourceType};
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

#[test]
fn test_self_update() {
    // Only do this if RUIN_MY_COMPUTER_WITH_INSTALLERS is set
    if std::env::var(ENV_RUIN_ME)
        .map(|s| !s.is_empty())
        .unwrap_or(false)
    {
        let mut args = RuntestArgs {
            app_name: "cargo-dist".to_owned(),
            package: "cargo-dist".to_owned(),
            owner: "axodotdev".to_owned(),
            bin: PathBuf::from(BIN),
            binaries: vec!["cargo-dist".to_owned()],
            args: vec![
                "dist".to_owned(),
                "update".to_owned(),
                // init includes interactive components, so we
                // can't safely run it within a noninteractive test
                "--skip-init".to_owned(),
            ],
            release_type: ReleaseSourceType::GitHub,
        };

        // First run with GitHub
        let installed_bin = axoupdater::test::helpers::perform_runtest(&args);
        assert!(installed_bin.exists());
        let status = Command::new(&installed_bin)
            .arg("--version")
            .status()
            .expect("binary didn't exist or --version returned nonzero");
        assert!(status.success());

        // Remove the installed binary before running the next test
        std::fs::remove_file(installed_bin).unwrap();

        // Then rerun with Axo; this is in one function because
        // they touch the same global files and can't happen
        // in parallel.
        args.release_type = ReleaseSourceType::Axo;
        let installed_bin = axoupdater::test::helpers::perform_runtest(&args);
        assert!(installed_bin.exists());
        let status = Command::new(&installed_bin)
            .arg("--version")
            .status()
            .expect("binary didn't exist or --version returned nonzero");
        assert!(status.success());
    }
}
