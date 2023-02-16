use std::process::{Command, Output, Stdio};

static BIN: &str = env!("CARGO_BIN_EXE_cargo-dist");

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

    insta::assert_snapshot!("long-help", format_outputs(&output));
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

    insta::assert_snapshot!("short-help", format_outputs(&output));
    assert!(output.status.success(), "{}", output.status);
}

#[test]
fn test_manifest() {
    let output = Command::new(BIN)
        .arg("dist")
        .arg("manifest")
        .arg("--artifacts=all")
        .arg("--no-local-paths")
        .arg("--output-format=json")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    // We don't want this to churn every time we do a version bump
    insta::with_settings!({filters => vec![
        (r"\d+\.\d+\.\d+(\-prerelease\d+)?", "1.0.0-FAKEVERSION"),
        (r#""announcement_tag": .*"#, r#""announcement_tag": "CENSORED","#),
        (r#""announcement_title": .*"#, r#""announcement_title": "CENSORED""#),
        (r#""announcement_changelog": .*"#, r#""announcement_changelog": "CENSORED""#),
        (r#""announcement_github_body": .*"#, r#""announcement_github_body": "CENSORED""#),
        (r#""announcement_is_prerelease": .*"#, r#""announcement_is_prerelease": "CENSORED""#),
    ]}, {
        insta::assert_snapshot!(format_outputs(&output));
    });

    assert!(output.status.success(), "{}", output.status);
}
