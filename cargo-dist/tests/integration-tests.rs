//! Serious Integration Tests!
//!
//! These tests:
//!
//! * Fetch a specific commit of axolotlsay
//! * Overlay a new value for [workspace.metadata.dist]
//! * Run `cargo-dist dist build -aglobal` on it (building installers)
//!     * set `OVERRIDE_CARGO_BIN_EXE_cargo-dist=path/to/some/cargo-dist` to not use the current checkout
//! * Run shellcheck on installer.sh (only if detected on the system)
//! * Run PSScriptAnalyzer on installer.ps1 (only if detected on the system)
//! * Run installer.sh and check the results
//!    * linux/macos only, must also set RUIN_MY_COMPUTER_WITH_INSTALLERS to opt in
//!    * HOME, CARGO_HOME, and MY_ENV_VAR overriden to keep it scoped to a temp dir
//!        * CARGO_HOME currently always deleted, should probably have a test where we set it
//! * insta.rs snapshot the installers
//!
//! Also note that the "run installer.sh" step forces us to be coherent with the actual published
//! releases. (i.e. axolotlsay 0.1.0 has .tar.gz archives, so we need to always set that!)
//! In the future we may unblock that (and deepen the coverage of our integration-testing)
//! by actually running `cargo dist build -alocal` and hosting the binaries on a local
//! static file server.
//!
//! In the future we may also further generalize this into a "gallery" of test projects
//! with support for testing other tools like oranda.

mod gallery;
use gallery::*;

#[test]
fn axolotlsay_basic() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew", "npm"]
tap = "axodotdev/homebrew-packages"
publish-jobs = ["homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
allow-dirty = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
scope = "@axodotdev"

"#
        ))?;

        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(ctx, ".cargo/bin/")?;
        // Check generate-ci
        let ci_result = ctx.cargo_dist_generate_ci(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_no_homebrew_publish() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew", "npm"]
tap = "axodotdev/homebrew-packages"
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
allow-dirty = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
scope = "@axodotdev"

"#
        ))?;

        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(ctx, ".cargo/bin/")?;
        // Check generate-ci
        let ci_result = ctx.cargo_dist_generate_ci(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_edit_existing() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew", "npm"]
tap = "axodotdev/homebrew-packages"
publish-jobs = ["homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
allow-dirty = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
scope = "@axodotdev"
create-release = false

"#
        ))?;

        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(ctx, ".cargo/bin/")?;
        // Check generate-ci
        let ci_result = ctx.cargo_dist_generate_ci(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn akaikatana_basic() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AKAIKATANA_REPACK.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();

        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
rust-toolchain-version = "1.67.1"
ci = ["github"]
allow-dirty = ["github"]
installers = ["shell", "powershell", "homebrew"]
tap = "mistydemeo/homebrew-formulae"
publish-jobs = ["homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]

"#
        ))?;

        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(ctx, ".cargo/bin/")?;
        // Check generate-ci
        let ci_result = ctx.cargo_dist_generate_ci(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn akaikatana_repo_with_dot_git() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AKAIKATANA_REPACK.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();

        // Same as above, except we set a repository path with .git.
        // We rely on the snapshot to test that the output looks right.
        ctx.patch_cargo_toml(format!(r#"
[package]
repository = "https://github.com/mistydemeo/akaikatana-repack.git"

[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
rust-toolchain-version = "1.67.1"
ci = ["github"]
allow-dirty = ["github"]
installers = ["shell", "powershell", "homebrew"]
tap = "mistydemeo/homebrew-formulae"
publish-jobs = ["homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]

"#
        ))?;

        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(ctx, ".cargo/bin/")?;
        // Check generate-ci
        let ci_result = ctx.cargo_dist_generate_ci(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn install_path_cargo_home() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();

        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
allow-dirty = ["github"]
install-path = "CARGO_HOME"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;

        let results = ctx.cargo_dist_build_and_plan(test_name)?;
        results.check_all(ctx, ".cargo/bin/")?.snap();

        Ok(())
    })
}

#[test]
fn install_path_home_subdir_min() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();

        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
allow-dirty = ["github"]
install-path = "~/.axolotlsay/"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;

        let results = ctx.cargo_dist_build_and_plan(test_name)?;
        results.check_all(ctx, ".axolotlsay/")?.snap();

        Ok(())
    })
}

#[test]
fn install_path_home_subdir_deeper() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();

        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
allow-dirty = ["github"]
install-path = "~/.axolotlsay/bins"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;

        let results = ctx.cargo_dist_build_and_plan(test_name)?;
        results.check_all(ctx, ".axolotlsay/bins")?.snap();

        Ok(())
    })
}

#[test]
fn install_path_home_subdir_space() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();

        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
allow-dirty = ["github"]
install-path = "~/My Axolotlsay Documents"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;

        let results = ctx.cargo_dist_build_and_plan(test_name)?;
        results.check_all(ctx, "My Axolotlsay Documents/")?.snap();

        Ok(())
    })
}

#[test]
fn install_path_home_subdir_space_deeper() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
allow-dirty = ["github"]
install-path = "~/My Axolotlsay Documents/bin/"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;

        let results = ctx.cargo_dist_build_and_plan(test_name)?;
        results.check_all(ctx, "My Axolotlsay Documents/bin/")?.snap();

        Ok(())
    })
}

#[test]
fn install_path_env_no_subdir() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();

        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
allow-dirty = ["github"]
install-path = "$MY_ENV_VAR/"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;

        let results = ctx.cargo_dist_build_and_plan(test_name)?;
        results.check_all(ctx, ".axolotlsay/")?.snap();

        Ok(())
    })
}

#[test]
fn install_path_env_subdir() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();

        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
allow-dirty = ["github"]
install-path = "$MY_ENV_VAR/bin/"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;

        let results = ctx.cargo_dist_build_and_plan(test_name)?;
        results.check_all(ctx, ".axolotlsay/bin/")?.snap();

        Ok(())
    })
}

#[test]
fn install_path_env_subdir_space() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();

        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
allow-dirty = ["github"]
install-path = "$MY_ENV_VAR/My Axolotlsay Documents"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;

        let results = ctx.cargo_dist_build_and_plan(test_name)?;
        results.check_all(ctx, ".axolotlsay/My Axolotlsay Documents/")?.snap();

        Ok(())
    })
}

#[test]
fn install_path_env_subdir_space_deeper() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();

        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
allow-dirty = ["github"]
install-path = "$MY_ENV_VAR/My Axolotlsay Documents/bin"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;

        let results = ctx.cargo_dist_build_and_plan(test_name)?;
        results.check_all(ctx, ".axolotlsay/My Axolotlsay Documents/bin/")?.snap();

        Ok(())
    })
}

#[test]
#[should_panic(expected = r#"install-path = "~/" is missing a subdirectory"#)]
fn install_path_invalid() {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();

        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
allow-dirty = ["github"]
install-path = "~/"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;
        ctx.cargo_dist_build_and_plan(test_name).unwrap();

        Ok(())
    }).unwrap();
}

#[test]
#[should_panic(expected = r#"install-path = "$MY_ENV" is missing a subdirectory"#)]
fn env_path_invalid() {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();

        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
allow-dirty = ["github"]
install-path = "$MY_ENV"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;
        ctx.cargo_dist_build_and_plan(test_name).unwrap();

        Ok(())
    }).unwrap();
}
