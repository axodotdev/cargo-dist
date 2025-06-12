use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::{receipt::get_config_paths, ReleaseSourceType};

static RECEIPT_TEMPLATE: &str = r#"{"binaries":[BINARIES],"install_prefix":"INSTALL_PREFIX","provider":{"source":"cargo-dist","version":"0.10.0-prerelease.1"},"source":{"app_name":"APP_NAME","name":"PACKAGE","owner":"OWNER","release_type":"RELEASE_TYPE"},"version":"VERSION"}"#;

/// Generates an install receipt given the specified fields and returns it as a string.
fn install_receipt(
    app_name: &str,
    package: &str,
    owner: &str,
    binaries: &[String],
    version: &str,
    prefix: &str,
    release_type: &ReleaseSourceType,
) -> String {
    let binaries = binaries
        .iter()
        .map(|name| format!(r#""{name}""#))
        .collect::<Vec<String>>()
        .join(", ");
    RECEIPT_TEMPLATE
        .replace("BINARIES", &binaries)
        .replace("PACKAGE", package)
        .replace("OWNER", owner)
        .replace("APP_NAME", app_name)
        .replace("INSTALL_PREFIX", &prefix.replace('\\', "\\\\"))
        .replace("VERSION", version)
        .replace("RELEASE_TYPE", &release_type.to_string())
}

/// Generates an install receipt given the specified fields and writes it to disk at the appropriate location in `config_path`. The path to the new file is returned.
#[allow(clippy::too_many_arguments)]
fn write_receipt(
    app_name: &str,
    package: &str,
    owner: &str,
    binaries: &[String],
    version: &str,
    prefix: &Path,
    config_path: &PathBuf,
    release_type: &ReleaseSourceType,
) -> std::io::Result<PathBuf> {
    let contents = install_receipt(
        app_name,
        package,
        owner,
        binaries,
        version,
        &prefix.to_string_lossy(),
        release_type,
    );
    let receipt_name = config_path.join(format!("{package}-receipt.json"));
    std::fs::create_dir_all(config_path)?;
    std::fs::write(&receipt_name, contents)?;

    Ok(receipt_name)
}

/// The arguments used for `perform_runtest`.
pub struct RuntestArgs {
    /// The name of the app being tested.
    pub app_name: String,
    /// The name of the package/workspace being tested. In GitHub terms, this is the "name" of the owner/name repo format.
    pub package: String,
    /// The owner of the package being tested. In GitHub terms, this is the "owner" of the owner/name repo format.
    pub owner: String,
    /// The path to the executable being tested, usually the one from the `CARGO_BIN_EXE_<name>` environment variable.
    pub bin: PathBuf,
    /// A list of all binaries installed by the app being tested.
    pub binaries: Vec<String>,
    /// The arguments taken by the binary being tested.
    ///
    /// For example, for cargo dist, it's called as `cargo dist selfupdate --skip-init`, so this value is `vec!["dist", "selfupdate", "--skip-init"]`.
    pub args: Vec<String>,
    /// The type of release to test, either GitHub or Axo Releases.
    pub release_type: ReleaseSourceType,
}

/// Actually installs your app and runs its updater.
/// For detailed information on the arguments, see [`RuntestArgs`][].
///
/// This function performs several assertions of its own, then returns the path to which the binary was expected to have been installed in order to allow the caller to perform additional tests or assertions.
/// Because it writes to real files outside a temporary directory, it's highly recommended that this only becalled within CI builds.
///
/// Note that, at the moment, this always attempts to install to CARGO_HOME (~/.cargo/bin).
pub fn perform_runtest(runtest_args: &RuntestArgs) -> PathBuf {
    let RuntestArgs {
        app_name,
        package,
        owner,
        bin,
        binaries,
        args,
        release_type,
    } = runtest_args;

    let basename = bin.file_name().unwrap();
    let home = homedir::my_home().unwrap().unwrap();

    let app_home = &home.join(".cargo").join("bin");
    let app_path = &app_home.join(basename);

    let config_path = get_config_paths(app_name)
        .unwrap()
        // Accept whichever path comes first; it doesn't matter to us.
        .first()
        .expect("no possible legal config paths found!?")
        .to_owned()
        .into_std_path_buf();

    // Ensure we delete any previous copy that may exist
    // at this path before we copy in our version.
    if app_path.exists() {
        std::fs::remove_file(app_path).unwrap();
    }
    assert!(!app_path.exists());

    // Install to the home directory
    std::fs::copy(bin, app_path).unwrap();

    // Create a fake install receipt
    // We lie about being a very old version so we always
    // consider upgrading to something.
    write_receipt(
        app_name,
        package,
        owner,
        binaries.as_slice(),
        "0.0.1",
        app_home,
        &config_path,
        release_type,
    )
    .unwrap();

    let output = Command::new(app_path)
        .args(args)
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

    app_path.to_owned()
}

// Who tests the testers........
#[test]
fn test_receipt_generation() {
    let expected = r#"{"binaries":["cargo-dist"],"install_prefix":"/tmp/prefix","provider":{"source":"cargo-dist","version":"0.10.0-prerelease.1"},"source":{"app_name":"cargo-dist","name":"cargo-dist","owner":"axodotdev","release_type":"github"},"version":"0.5.0"}"#;

    let actual = install_receipt(
        "cargo-dist",
        "cargo-dist",
        "axodotdev",
        &["cargo-dist".to_owned()],
        "0.5.0",
        "/tmp/prefix",
        &ReleaseSourceType::GitHub,
    );
    assert_eq!(expected, actual);
}

#[test]
fn test_receipt_different_app_package() {
    let expected = r#"{"binaries":["axolotlsay"],"install_prefix":"/tmp/prefix","provider":{"source":"cargo-dist","version":"0.10.0-prerelease.1"},"source":{"app_name":"axolotlsay","name":"cargodisttest","owner":"mistydemeo","release_type":"github"},"version":"0.5.0"}"#;

    let actual = install_receipt(
        "axolotlsay",
        "cargodisttest",
        "mistydemeo",
        &["axolotlsay".to_owned()],
        "0.5.0",
        "/tmp/prefix",
        &ReleaseSourceType::GitHub,
    );
    assert_eq!(expected, actual);
}

#[test]
fn test_receipt_multiple_binaries() {
    let expected = r#"{"binaries":["bin1", "bin2"],"install_prefix":"/tmp/prefix","provider":{"source":"cargo-dist","version":"0.10.0-prerelease.1"},"source":{"app_name":"axolotlsay","name":"cargodisttest","owner":"mistydemeo","release_type":"github"},"version":"0.5.0"}"#;

    let actual = install_receipt(
        "axolotlsay",
        "cargodisttest",
        "mistydemeo",
        &["bin1".to_owned(), "bin2".to_owned()],
        "0.5.0",
        "/tmp/prefix",
        &ReleaseSourceType::GitHub,
    );
    assert_eq!(expected, actual);
}

#[test]
fn test_receipt_different_alternate_release_type() {
    let expected = r#"{"binaries":["axolotlsay"],"install_prefix":"/tmp/prefix","provider":{"source":"cargo-dist","version":"0.10.0-prerelease.1"},"source":{"app_name":"axolotlsay","name":"cargodisttest","owner":"mistydemeo","release_type":"axodotdev"},"version":"0.5.0"}"#;

    let actual = install_receipt(
        "axolotlsay",
        "cargodisttest",
        "mistydemeo",
        &["axolotlsay".to_owned()],
        "0.5.0",
        "/tmp/prefix",
        &ReleaseSourceType::Axo,
    );
    assert_eq!(expected, actual);
}
