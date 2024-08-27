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
installers = ["shell", "powershell", "homebrew", "npm", "msi", "pkg"]
tap = "axodotdev/homebrew-packages"
publish-jobs = ["homebrew", "npm"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
install-success-msg = ">o_o< everything's installed!"
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
npm-scope ="@axodotdev"

[package.metadata.wix]
upgrade-guid = "B36177BE-EA4D-44FB-B05C-EDDABDAA95CA"
path-guid = "BFD25009-65A4-4D1E-97F1-0030465D90D6"

[package.metadata.dist.mac-pkg-config]
identifier = "dev.axo.axolotsay"

"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_basic_lies() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew", "npm", "msi", "pkg"]
tap = "axodotdev/homebrew-packages"
publish-jobs = ["homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
npm-scope ="@axodotdev"

[package.metadata.wix]
upgrade-guid = "B36177BE-EA4D-44FB-B05C-EDDABDAA95CA"
path-guid = "BFD25009-65A4-4D1E-97F1-0030465D90D6"

[package.metadata.dist.mac-pkg-config]
identifier = "dev.axo.axolotsay"
install-location = "/opt/axolotlsay"

"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_lies(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_custom_formula() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|mut ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["homebrew"]
tap = "axodotdev/homebrew-packages"
# https://rubydoc.brew.sh/Formula.html naming rules for Formulae
# providing this config will make an AxolotlBrew formula
formula = "axolotl-brew"
publish-jobs = ["homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
release-branch = "production"

[package.metadata.wix]
upgrade-guid = "B36177BE-EA4D-44FB-B05C-EDDABDAA95CA"
path-guid = "BFD25009-65A4-4D1E-97F1-0030465D90D6"

"#
        ))?;

        ctx.options.set_options("axolotlsay").homebrew_package_name = Some("axolotl-brew".to_owned());

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_abyss() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew", "npm", "msi", "pkg"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
npm-scope ="@axodotdev"
hosting = ["axodotdev", "github"]
github-attestations = true

[package.metadata.wix]
upgrade-guid = "B36177BE-EA4D-44FB-B05C-EDDABDAA95CA"
path-guid = "BFD25009-65A4-4D1E-97F1-0030465D90D6"

[package.metadata.dist.mac-pkg-config]
identifier = "dev.axo.axolotsay"

"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        // !!! this hosting doesn't exist, do not ruin my computer with installers!!!
        let main_snap = main_result.check_all_no_ruin(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_abyss_only() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew", "npm", "msi", "pkg"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
npm-scope ="@axodotdev"
hosting = ["axodotdev"]

[package.metadata.wix]
upgrade-guid = "B36177BE-EA4D-44FB-B05C-EDDABDAA95CA"
path-guid = "BFD25009-65A4-4D1E-97F1-0030465D90D6"

[package.metadata.dist.mac-pkg-config]
identifier = "dev.axo.axolotsay"
install-location = "/opt/axolotlsay"

"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        // !!! this hosting doesn't exist, do not ruin my computer with installers!!!
        let main_snap = main_result.check_all_no_ruin(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_dispatch() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = []
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
dispatch-releases = true
"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_tag_namespace() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = []
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
tag-namespace = "owo"
"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate_prefixed(test_name, "owo-")?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_dispatch_abyss() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = []
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
hosting = ["axodotdev", "github"]
dispatch-releases = true
"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        // !!! this hosting doesn't exist, do not ruin my computer with installers!!!
        let main_snap = main_result.check_all_no_ruin(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_dispatch_abyss_only() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = []
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
hosting = ["axodotdev"]
dispatch-releases = true
"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        // !!! this hosting doesn't exist, do not ruin my computer with installers!!!
        let main_snap = main_result.check_all_no_ruin(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_no_locals() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = []
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
build-local-artifacts = false
github-releases-repo = "custom-owner/cool-repo"
"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        // !!! this hosting doesn't exist, do not ruin my computer with installers!!!
        let main_snap = main_result.check_all_no_ruin(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_no_locals_but_custom() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = []
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
build-local-artifacts = false
local-artifacts-jobs = ["./local-artifacts"]
github-custom-job-permissions = {{ local-artifacts = {{ packages = "write" }} }}
release-branch = "production"

"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        // !!! this hosting doesn't exist, do not ruin my computer with installers!!!
        let main_snap = main_result.check_all_no_ruin(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_no_homebrew_publish() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|mut ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew", "npm"]
tap = "axodotdev/homebrew-packages"
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
npm-scope = "@axodotdev"
npm-package = "coolbeans"
cache-builds = true

"#
        ))?;
        ctx.options.set_options("axolotlsay").npm_package_name = Some("coolbeans".to_owned());

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
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
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
npm-scope ="@axodotdev"
create-release = false

"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_ssldotcom_windows_sign() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "msi", "pkg"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
ssldotcom-windows-sign = "test"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

[package.metadata.wix]
upgrade-guid = "B36177BE-EA4D-44FB-B05C-EDDABDAA95CA"
path-guid = "BFD25009-65A4-4D1E-97F1-0030465D90D6"

[package.metadata.dist.mac-pkg-config]
identifier = "dev.axo.axolotsay"
install-location = "/opt/axolotlsay"

"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_ssldotcom_windows_sign_prod() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "msi", "pkg"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
ssldotcom-windows-sign = "prod"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

[package.metadata.wix]
upgrade-guid = "B36177BE-EA4D-44FB-B05C-EDDABDAA95CA"
path-guid = "BFD25009-65A4-4D1E-97F1-0030465D90D6"

[package.metadata.dist.mac-pkg-config]
identifier = "dev.axo.axolotsay"
install-location = "/opt/axolotlsay"

"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_user_plan_job() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew", "npm"]
tap = "axodotdev/homebrew-packages"
plan-jobs = ["./my-plan-job-1", "./my-plan-job-2"]
publish-jobs = ["homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
npm-scope ="@axodotdev"
github-create-release-phase = "host"
pr-run-mode = "upload"

"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_user_local_build_job() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew", "npm"]
tap = "axodotdev/homebrew-packages"
local-artifacts-jobs = ["./my-plan-job-1", "./my-plan-job-2"]
publish-jobs = ["homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
npm-scope ="@axodotdev"

"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_user_global_build_job() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew", "npm"]
tap = "axodotdev/homebrew-packages"
global-artifacts-jobs = ["./my-plan-job-1", "./my-plan-job-2"]
publish-jobs = ["homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
npm-scope ="@axodotdev"
"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_user_host_job() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew", "npm"]
tap = "axodotdev/homebrew-packages"
host-jobs = ["./my-plan-job-1", "./my-plan-job-2"]
publish-jobs = ["homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
npm-scope ="@axodotdev"
"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_user_publish_job() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew", "npm"]
tap = "axodotdev/homebrew-packages"
publish-jobs = ["homebrew", "./custom-task-1", "./custom-task-2"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
npm-scope ="@axodotdev"
"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_musl() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(
            r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "npm"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl", "aarch64-apple-darwin", "x86_64-apple-darwin"]
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
npm-scope ="@axodotdev"

"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_musl_no_gnu() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(
            r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "npm"]
targets = ["x86_64-unknown-linux-musl", "aarch64-apple-darwin", "x86_64-apple-darwin"]
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
npm-scope ="@axodotdev"

"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_custom_github_runners() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = []
targets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu", "x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl"]
ci = ["github"]

[workspace.metadata.dist.github-custom-runners]
global = "buildjet-8vcpu-ubuntu-2204"
x86_64-unknown-linux-gnu = "buildjet-8vcpu-ubuntu-2204"
x86_64-unknown-linux-musl = "buildjet-8vcpu-ubuntu-2204"
aarch64-unknown-linux-gnu = "buildjet-8vcpu-ubuntu-2204-arm"
aarch64-unknown-linux-musl = "buildjet-8vcpu-ubuntu-2204-arm"
"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_updaters() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew", "npm", "msi", "pkg"]
tap = "axodotdev/homebrew-packages"
publish-jobs = ["homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
npm-scope ="@axodotdev"
install-updater = true

[package.metadata.wix]
upgrade-guid = "B36177BE-EA4D-44FB-B05C-EDDABDAA95CA"
path-guid = "BFD25009-65A4-4D1E-97F1-0030465D90D6"

[package.metadata.dist.mac-pkg-config]
identifier = "dev.axo.axolotsay"

"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        // Ruin won't work because we don't have a release with actual updaters yet
        let main_snap = main_result.check_all_no_ruin(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_homebrew_packages() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(
            r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew", "npm", "msi", "pkg"]
tap = "axodotdev/homebrew-packages"
publish-jobs = ["homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
npm-scope ="@axodotdev"

[workspace.metadata.dist.dependencies.homebrew]
"homebrew/cask/macfuse" = "*"
libcue = "2.3.0"

[package.metadata.dist.mac-pkg-config]
identifier = "dev.axo.axolotsay"

"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_alias() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew", "npm", "msi", "pkg"]
tap = "axodotdev/homebrew-packages"
publish-jobs = ["homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
npm-scope ="@axodotdev"

[package.metadata.wix]
upgrade-guid = "B36177BE-EA4D-44FB-B05C-EDDABDAA95CA"
path-guid = "BFD25009-65A4-4D1E-97F1-0030465D90D6"

[workspace.metadata.dist.bin-aliases]
axolotlsay = ["axolotlsay-link"]

[package.metadata.dist.mac-pkg-config]
identifier = "dev.axo.axolotsay"
install-location = "/opt/axolotlsay"

"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_several_aliases() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew", "npm", "msi", "pkg"]
tap = "axodotdev/homebrew-packages"
publish-jobs = ["homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
npm-scope ="@axodotdev"

[package.metadata.wix]
upgrade-guid = "B36177BE-EA4D-44FB-B05C-EDDABDAA95CA"
path-guid = "BFD25009-65A4-4D1E-97F1-0030465D90D6"

[workspace.metadata.dist.bin-aliases]
axolotlsay = ["axolotlsay-link1", "axolotlsay-link2"]

[package.metadata.dist.mac-pkg-config]
identifier = "dev.axo.axolotsay"

"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_alias_ignores_missing_bins() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew", "npm", "msi", "pkg"]
tap = "axodotdev/homebrew-packages"
publish-jobs = ["homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
npm-scope ="@axodotdev"

[package.metadata.wix]
upgrade-guid = "B36177BE-EA4D-44FB-B05C-EDDABDAA95CA"
path-guid = "BFD25009-65A4-4D1E-97F1-0030465D90D6"

[workspace.metadata.dist.bin-aliases]
nosuchbin = ["axolotlsay-link1", "axolotlsay-link2"]

[package.metadata.dist.mac-pkg-config]
identifier = "dev.axo.axolotsay"

"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
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
installers = ["shell", "powershell", "homebrew"]
tap = "mistydemeo/homebrew-formulae"
publish-jobs = ["homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
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
allow-dirty = ["ci"]
install-path = "CARGO_HOME"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;

        let results = ctx.cargo_dist_build_and_plan(test_name)?;
        results.check_all(&ctx, ".cargo/bin/")?.snap();

        Ok(())
    })
}

#[test]
fn akaikatana_musl() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AKAIKATANA_REPACK.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();

        ctx.patch_cargo_toml(format!(
            r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
rust-toolchain-version = "1.67.1"
ci = ["github"]
installers = ["shell"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl", "aarch64-apple-darwin", "x86_64-apple-darwin"]

"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn akaikatana_updaters() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AKAIKATANA_REPACK.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();

        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
rust-toolchain-version = "1.67.1"
ci = ["github"]
installers = ["shell", "powershell", "homebrew"]
tap = "mistydemeo/homebrew-formulae"
publish-jobs = ["homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
install-updater = true

"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        // Ruin won't work because we don't have a release with actual updaters yet
        let main_snap = main_result.check_all_no_ruin(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn akaikatana_one_alias_among_many_binaries() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AKAIKATANA_REPACK.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();

        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
rust-toolchain-version = "1.67.1"
ci = ["github"]
installers = ["shell", "powershell", "homebrew"]
tap = "mistydemeo/homebrew-formulae"
publish-jobs = ["homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]

[workspace.metadata.dist.bin-aliases]
akextract = ["akextract-link"]

"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn akaikatana_two_bin_aliases() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AKAIKATANA_REPACK.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();

        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
rust-toolchain-version = "1.67.1"
ci = ["github"]
installers = ["shell", "powershell", "homebrew"]
tap = "mistydemeo/homebrew-formulae"
publish-jobs = ["homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]

[workspace.metadata.dist.bin-aliases]
akextract = ["akextract-link"]
akmetadata = ["akmetadata-link"]


"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
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
allow-dirty = ["ci"]
install-path = "~/.axolotlsay/"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;

        let results = ctx.cargo_dist_build_and_plan(test_name)?;
        results.check_all(&ctx, ".axolotlsay/")?.snap();

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
allow-dirty = ["ci"]
install-path = "~/.axolotlsay/bins"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;

        let results = ctx.cargo_dist_build_and_plan(test_name)?;
        results.check_all(&ctx, ".axolotlsay/bins")?.snap();

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
allow-dirty = ["ci"]
install-path = "~/My Axolotlsay Documents"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;

        let results = ctx.cargo_dist_build_and_plan(test_name)?;
        results.check_all(&ctx, "My Axolotlsay Documents/")?.snap();

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
allow-dirty = ["ci"]
install-path = "~/My Axolotlsay Documents/bin/"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;

        let results = ctx.cargo_dist_build_and_plan(test_name)?;
        results.check_all(&ctx, "My Axolotlsay Documents/bin/")?.snap();

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
allow-dirty = ["ci"]
install-path = "$MY_ENV_VAR/"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;

        let results = ctx.cargo_dist_build_and_plan(test_name)?;
        results.check_all(&ctx, ".axolotlsay/")?.snap();

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
allow-dirty = ["ci"]
install-path = "$MY_ENV_VAR/bin/"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;

        let results = ctx.cargo_dist_build_and_plan(test_name)?;
        results.check_all(&ctx, ".axolotlsay/bin/")?.snap();

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
allow-dirty = ["ci"]
install-path = "$MY_ENV_VAR/My Axolotlsay Documents"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;

        let results = ctx.cargo_dist_build_and_plan(test_name)?;
        results.check_all(&ctx, ".axolotlsay/My Axolotlsay Documents/")?.snap();

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
allow-dirty = ["ci"]
install-path = "$MY_ENV_VAR/My Axolotlsay Documents/bin"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;

        let results = ctx.cargo_dist_build_and_plan(test_name)?;
        results.check_all(&ctx, ".axolotlsay/My Axolotlsay Documents/bin/")?.snap();

        Ok(())
    })
}

#[test]
fn install_path_fallback_no_env_var_set() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();

        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
allow-dirty = ["ci"]
install-path = ["$NO_SUCH_ENV_VAR/My Nonexistent Documents", "$MY_ENV_VAR/My Axolotlsay Documents"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;

        let results = ctx.cargo_dist_build_and_plan(test_name)?;
        results.check_all(&ctx, ".axolotlsay/My Axolotlsay Documents/")?.snap();

        Ok(())
    })
}

#[test]
#[should_panic(expected = r#"Incompatible install paths configured in Cargo.toml"#)]
fn install_path_fallback_to_cargo_home() {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();

        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
allow-dirty = ["ci"]
install-path = ["$NO_SUCH_ENV_VAR/My Nonexistent Documents", "CARGO_HOME"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;
        ctx.cargo_dist_build_and_plan(test_name).unwrap();

        Ok(())
    }).unwrap();
}

#[test]
fn install_path_no_fallback_taken() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();

        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
allow-dirty = ["ci"]
install-path = ["~/.axolotlsay/", "$MY_ENV_VAR/My Axolotlsay Documents/bin"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;

        let results = ctx.cargo_dist_build_and_plan(test_name)?;
        results.check_all(&ctx, ".axolotlsay/")?.snap();

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
allow-dirty = ["ci"]
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
allow-dirty = ["ci"]
install-path = "$MY_ENV"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#
        ))?;
        ctx.cargo_dist_build_and_plan(test_name).unwrap();

        Ok(())
    }).unwrap();
}

#[test]
#[should_panic(expected = r#"no packages"#)]
fn axoasset_basic() {
    // This is just a library so we should error with a helpful message
    let test_name = _function_name!();
    AXOASSET
        .run_test(|ctx| {
            let dist_version = ctx.tools.cargo_dist.version().unwrap();

            ctx.patch_cargo_toml(format!(
                r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
ci = ["github"]
targets = ["x86_64-pc-windows-msvc"]
"#
            ))?;

            // Do usual build+plan checks
            let main_result = ctx.cargo_dist_build_and_plan(test_name).unwrap();
            let main_snap = main_result.check_all(&ctx, ".cargo/bin/").unwrap();
            // snapshot all
            main_snap.snap();
            Ok(())
        })
        .unwrap();
}

#[test]
fn axolotlsay_disable_source_tarball() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew", "npm", "msi", "pkg"]
tap = "axodotdev/homebrew-packages"
publish-jobs = ["homebrew"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
npm-scope ="@axodotdev"
source-tarball = false

[package.metadata.wix]
upgrade-guid = "B36177BE-EA4D-44FB-B05C-EDDABDAA95CA"
path-guid = "BFD25009-65A4-4D1E-97F1-0030465D90D6"

[package.metadata.dist.mac-pkg-config]
identifier = "dev.axo.axolotsay"
install-location = "/opt/axolotlsay"

"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_checksum_sha3_256() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
checksum = "sha3-256"
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_checksum_sha3_512() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
checksum = "sha3-512"
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_checksum_blake2s() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
checksum = "blake2s"
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_checksum_blake2b() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
checksum = "blake2b"
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_generic_workspace_basic() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY_HYBRID.run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_dist_workspace(format!(
            r#"
        [dist]
        cargo-dist-version = "{dist_version}"
        installers = ["shell", "powershell", "homebrew"]
        tap = "axodotdev/homebrew-packages"
        publish-jobs = ["homebrew"]
        targets = ["x86_64-apple-darwin", "x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
        install-success-msg = ">o_o< everything's installed!"
        ci = ["github"]
        unix-archive = ".tar.xz"
        windows-archive = ".zip"

        "#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}

#[test]
fn axolotlsay_build_setup_steps() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    AXOLOTLSAY
        .run_test(|ctx| {
            ctx.workspace_write_file(".github/workflows/build_setup.yml",
        include_str!("../../cargo-dist/tests/build_setup.yml"))?;
        let dist_version = ctx.tools.cargo_dist.version().unwrap();
        ctx.patch_cargo_toml(format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"
installers = ["shell", "powershell", "homebrew", "npm", "msi", "pkg"]
tap = "axodotdev/homebrew-packages"
publish-jobs = ["homebrew", "npm"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
install-success-msg = ">o_o< everything's installed!"
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"
npm-scope ="@axodotdev"
github-build-setup = "build_setup.yml"

[package.metadata.wix]
upgrade-guid = "B36177BE-EA4D-44FB-B05C-EDDABDAA95CA"
path-guid = "BFD25009-65A4-4D1E-97F1-0030465D90D6"

[package.metadata.dist.mac-pkg-config]
identifier = "dev.axo.axolotsay"

"#
        ))?;

        // Run generate to make sure stuff is up to date before running other commands
        let ci_result = ctx.cargo_dist_generate(test_name)?;
        let ci_snap = ci_result.check_all()?;
        // Do usual build+plan checks
        let main_result = ctx.cargo_dist_build_and_plan(test_name)?;
        let main_snap = main_result.check_all(&ctx, ".cargo/bin/")?;
        // snapshot all
        main_snap.join(ci_snap).snap();
        Ok(())
    })
}
