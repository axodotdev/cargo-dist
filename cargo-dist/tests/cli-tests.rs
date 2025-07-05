use std::{
    path::PathBuf,
    process::{Command, Output, Stdio},
};

#[cfg(unix)]
use std::{fs::File, os::unix::fs::PermissionsExt};

static BIN: &str = env!("CARGO_BIN_EXE_dist");
const ENV_RUIN_ME: &str = "RUIN_MY_COMPUTER_WITH_INSTALLERS";

#[allow(dead_code)]
mod gallery;
use axoasset::LocalAsset;
use axoprocess::Cmd;
use axoupdater::{test::helpers::RuntestArgs, AxoUpdater, ReleaseSourceType};
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
    assert_eq!(name, "dist");
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
        .arg(format!("dist-schema-v{}", version))
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
        .arg(format!("dist-schema/v{}", version))
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

fn generate_installer(version: &axotag::Version, release_type: ReleaseSourceType) -> Utf8PathBuf {
    let tools = Tools::default();

    // On Windows, we need a debug build of cargo-dist present
    // in order to be able to build the global artifacts. This is
    // due to the extra-artifacts, which we don't yet have a CLI
    // option to turn off during `cargo dist build`.
    Cmd::new("cargo", "run debug build")
        .arg("build")
        .status()
        .unwrap();

    // First update the cargo-dist version so we can safely `cargo dist build`
    tools
        .cargo_dist
        .output_checked(|cmd| cmd.arg("dist").arg("init").arg("--yes"))
        .unwrap();

    // Now we run cargo-dist to generate an installer for the current version
    tools
        .cargo_dist
        .output_checked(|cmd| cmd.arg("dist").arg("build").arg("--artifacts=global"))
        .unwrap();

    let ext = if cfg!(windows) { ".ps1" } else { ".sh" };
    let root = env!("CARGO_MANIFEST_DIR");
    let installer_path = Utf8PathBuf::from(root)
        .parent()
        .unwrap()
        .join("target")
        .join("distrib")
        .join(format!("cargo-dist-installer{ext}"));
    let installer_string = std::fs::read_to_string(&installer_path).unwrap();

    let installer_url = match release_type {
        ReleaseSourceType::Axo => {
            format!("https://axodotdev.artifacts.axodotdev.host/cargo-dist/v{version}",)
        }
        ReleaseSourceType::GitHub => {
            format!("https://github.com/axodotdev/cargo-dist/releases/download/v{version}",)
        }
    };

    let installer_string = installer_string
        .replace(env!("CARGO_PKG_VERSION"), &version.to_string())
        .replace(
            "https://fake.axo.dev/faker/cargo-dist/fake-id-do-not-upload",
            &installer_url,
        );

    #[cfg(unix)]
    {
        let installer_file = File::create(&installer_path).unwrap();
        let mut perms = installer_file.metadata().unwrap().permissions();
        perms.set_mode(0o744);
        installer_file.set_permissions(perms).unwrap();
    }

    LocalAsset::write_new(&installer_string, &installer_path).unwrap();

    installer_path
}

#[test]
#[ignore = "can't be reenabled until after the rename"]
fn test_self_update() {
    // Only do this if RUIN_MY_COMPUTER_WITH_INSTALLERS is set
    if std::env::var(ENV_RUIN_ME)
        .map(|s| s == "selfupdate" || s == "all")
        .unwrap_or(false)
    {
        std::env::remove_var("XDG_CONFIG_HOME");

        let mut args = RuntestArgs {
            app_name: "cargo-dist".to_owned(),
            package: "cargo-dist".to_owned(),
            owner: "axodotdev".to_owned(),
            bin: PathBuf::from(BIN),
            binaries: vec!["dist".to_owned()],
            args: vec![
                "dist".to_owned(),
                "selfupdate".to_owned(),
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

        // Next two runtests: like the above, but we produce
        // new installers so that we test against the latest installer
        // code in this PR instead of the installers that were generated
        // for the releases we're fetching.
        let tokio = tokio::runtime::Builder::new_current_thread()
            .worker_threads(1)
            .max_blocking_threads(128)
            .enable_all()
            .build()
            .expect("Initializing tokio runtime failed");

        let mut updater = AxoUpdater::new_for("cargo-dist");
        updater.set_release_source(axoupdater::ReleaseSource {
            release_type: ReleaseSourceType::Axo,
            owner: "axodotdev".to_owned(),
            name: "cargo-dist".to_owned(),
            app_name: "cargo-dist".to_owned(),
        });
        // This is the new version that we'll create alternate installers for.
        let new_version = tokio
            .block_on(updater.query_new_version())
            .unwrap()
            .unwrap();

        let installer_path = generate_installer(new_version, ReleaseSourceType::GitHub);

        // OK now, finally, we have an installer at `installer_path`
        // with URLs pointing at the exact version we want to test.
        // Point cargo-dist at it.
        std::env::set_var("CARGO_DIST_USE_INSTALLER_AT_PATH", installer_path);

        args.release_type = ReleaseSourceType::GitHub;
        let installed_bin = axoupdater::test::helpers::perform_runtest(&args);
        assert!(installed_bin.exists());
        let status = Command::new(&installed_bin)
            .arg("--version")
            .status()
            .expect("binary didn't exist or --version returned nonzero");
        assert!(status.success());

        // And once more, with Axo
        generate_installer(new_version, ReleaseSourceType::Axo);

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
