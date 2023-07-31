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

use camino::Utf8PathBuf;
use miette::miette;
use miette::Context;
use miette::IntoDiagnostic;
use std::process::Command;
use std::sync::Mutex;

/// Set this env-var to enable running the installer scripts in temp dirs
///
/// If everything's working right, then no problem.
/// Otherwise MEGA DANGER in messing up your computer.
#[allow(dead_code)]
const ENV_RUIN_ME: &str = "RUIN_MY_COMPUTER_WITH_INSTALLERS";

#[test]
fn basic() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version.as_ref().unwrap();

        ctx.run(test_name, ".cargo/bin/", format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"   
installers = ["shell", "powershell"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#))
    })
}

#[test]
fn install_path_cargo_home() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version.as_ref().unwrap();

        ctx.run(test_name, ".cargo/bin/", format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"   
installers = ["shell", "powershell"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
install-path = "CARGO_HOME"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#))
    })
}

#[test]
fn install_path_home_subdir_min() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version.as_ref().unwrap();

        ctx.run(test_name, ".axolotlsay/", format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"   
installers = ["shell", "powershell"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
install-path = "~/.axolotlsay/"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#))
    })
}

#[test]
fn install_path_home_subdir_deeper() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version.as_ref().unwrap();

        ctx.run(test_name, ".axolotlsay/bins", format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"   
installers = ["shell", "powershell"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
install-path = "~/.axolotlsay/bins"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#))
    })
}

#[test]
fn install_path_home_subdir_space() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version.as_ref().unwrap();

        ctx.run(test_name, "My Axolotlsay Documents/", format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"   
installers = ["shell", "powershell"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
install-path = "~/My Axolotlsay Documents"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#))
    })
}

#[test]
fn install_path_home_subdir_space_deeper() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version.as_ref().unwrap();

        ctx.run(test_name, "My Axolotlsay Documents/bin/", format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"   
installers = ["shell", "powershell"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
install-path = "~/My Axolotlsay Documents/bin/"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#))
    })
}

#[test]
fn install_path_env_no_subdir() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version.as_ref().unwrap();

        ctx.run(test_name, ".axolotlsay/", format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"   
installers = ["shell", "powershell"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
install-path = "$MY_ENV_VAR/"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#))
    })
}

#[test]
fn install_path_env_subdir() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version.as_ref().unwrap();

        ctx.run(test_name, ".axolotlsay/bin/", format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"   
installers = ["shell", "powershell"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
install-path = "$MY_ENV_VAR/bin/"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#))
    })
}

#[test]
fn install_path_env_subdir_space() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version.as_ref().unwrap();

        ctx.run(test_name, ".axolotlsay/My Axolotlsay Documents/", format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"   
installers = ["shell", "powershell"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
install-path = "$MY_ENV_VAR/My Axolotlsay Documents"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#))
    })
}

#[test]
fn install_path_env_subdir_space_deeper() -> Result<(), miette::Report> {
    let test_name = _function_name!();
    run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version.as_ref().unwrap();

        ctx.run(test_name, ".axolotlsay/My Axolotlsay Documents/bin/", format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"   
installers = ["shell", "powershell"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
install-path = "$MY_ENV_VAR/My Axolotlsay Documents/bin"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#))
    })
}

#[test]
#[should_panic(expected = r#"install-path = "~/" is missing a subdirectory"#)]
fn install_path_invalid() {
    let test_name = _function_name!();
    run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version.as_ref().unwrap();

        ctx.run(test_name, "BUSTED", format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"   
installers = ["shell", "powershell"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
install-path = "~/"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#))
    }).unwrap();
}

#[test]
#[should_panic(expected = r#"install-path = "$MY_ENV" is missing a subdirectory"#)]
fn env_path_invalid() {
    let test_name = _function_name!();
    run_test(|ctx| {
        let dist_version = ctx.tools.cargo_dist.version.as_ref().unwrap();

        ctx.run(test_name, "BUSTED", format!(r#"
[workspace.metadata.dist]
cargo-dist-version = "{dist_version}"   
installers = ["shell", "powershell"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-apple-darwin"]
ci = ["github"]
install-path = "$MY_ENV"
unix-archive = ".tar.gz"
windows-archive = ".tar.gz"

"#))
    }).unwrap();
}

const STATIC_CARGO_DIST_BIN: &str = env!("CARGO_BIN_EXE_cargo-dist");
const TARGET_DIR: &str = env!("CARGO_TARGET_TMPDIR");
static TEST_CONTEXT: Mutex<Option<TestContext>> = Mutex::new(None);

struct TestContext {
    tools: Tools,
    app_name: String,
}

fn run_test(
    f: impl FnOnce(&TestContext) -> Result<(), miette::Report>,
) -> Result<(), miette::Report> {
    let maybe_guard = TEST_CONTEXT.lock();
    // It's fine for the mutex to be poisoned once the value is Some because none of the tests
    // are allowed to mutate the TestContext. But if it's poisoned while None that means we
    // encountered an error while setting up TestContext and should just abort everything
    // instead of retrying over and over.
    let mut guard = match maybe_guard {
        Ok(guard) => guard,
        Err(poison) => {
            let guard = poison.into_inner();
            if guard.is_none() {
                panic!("aborting all tests: failed fetch");
            }
            guard
        }
    };
    if guard.is_none() {
        // Intentionally unwrapping here to poison the mutex if we can't fetch
        let ctx = init_context().unwrap();
        *guard = Some(ctx);
    }

    let ctx = guard.as_ref().unwrap();

    f(ctx)
}

fn init_context() -> Result<TestContext, miette::Report> {
    let repo_owner = "axodotdev";
    let repo_name = "axolotlsay";
    let repo_url = format!("https://github.com/{repo_owner}/{repo_name}");
    let commit_sha = "6b8337fb742908e506296eab3371bb71b76283d7";
    let app_name = repo_name;

    // Get the tools we'll invoke
    let tools = Tools::new();

    // Clone the repo we're interested in and cd into it
    fetch_repo(&tools.git, repo_name, &repo_url, commit_sha)?;

    // Run tests
    let ctx = TestContext {
        tools,
        app_name: app_name.to_owned(),
    };
    Ok(ctx)
}

impl TestContext {
    fn run(
        &self,
        name: &str,
        expected_bin_dir: &str,
        new_toml: String,
    ) -> Result<(), miette::Report> {
        eprintln!("\n=============== running test: {name} =================");
        // patch the Cargo.toml
        self.patch_cargo_toml(new_toml)?;

        // build installers
        eprintln!("running cargo dist build...");
        self.tools
            .cargo_dist
            .run(|cmd| cmd.arg("dist").arg("build").arg("-aglobal"))?;

        // read/analyze installers
        eprintln!("loading results...");
        let app_name = &self.app_name;
        let powershell_path = format!("target/distrib/{app_name}-installer.ps1");
        let shell_path = format!("target/distrib/{app_name}-installer.sh");

        // If we have shellcheck, check our shell script
        self.shellcheck(&shell_path)?;

        // If we have PsScriptAnalyzer, check our powershell script
        self.psanalyzer(&powershell_path)?;

        // If we can, run the script in a temp HOME
        self.runtest_shell(name, expected_bin_dir, &shell_path)?;

        // Now that all other checks have passed, it's safe to check snapshots
        self.snapshot_scripts(name, &shell_path, &powershell_path)?;

        eprintln!("ok!");
        Ok(())
    }

    fn patch_cargo_toml(&self, new_toml: String) -> Result<(), miette::Report> {
        eprintln!("loading Cargo.toml...");
        let toml_src = axoasset::SourceFile::load_local("Cargo.toml")?;
        let mut toml = toml_src.deserialize_toml_edit()?;
        eprintln!("editing Cargo.toml...");
        let new_table_src = axoasset::SourceFile::new("new-Cargo.toml", new_toml);
        let new_table = new_table_src.deserialize_toml_edit()?;

        // Written slightly verbosely to make it easier to isolate which failed
        eprintln!("{new_table}");
        let old = &mut toml["workspace"]["metadata"]["dist"];
        let new = &new_table["workspace"]["metadata"]["dist"];
        *old = new.clone();
        let toml_out = toml.to_string();
        eprintln!("writing Cargo.toml...");
        axoasset::LocalAsset::write_new(&toml_out, "Cargo.toml")?;

        Ok(())
    }

    /// Run shellcheck on the given script
    fn shellcheck(&self, shell_path: &str) -> Result<(), miette::Report> {
        // Only do this if the tool is available
        if let Some(shellcheck) = &self.tools.shellcheck {
            eprintln!("shellchecking {shell_path}");
            let output = shellcheck.output(|cmd| cmd.arg(shell_path))?;

            if !output.status.success() {
                eprintln!("{}", String::from_utf8_lossy(&output.stdout));
                return Err(miette!("shellcheck found issues"));
            }
        }
        Ok(())
    }

    /// Run PSScriptAnalyzer on the given script
    fn psanalyzer(&self, powershell_path: &str) -> Result<(), miette::Report> {
        // Only do this if the tool is available
        if let Some(psanalyzer) = &self.tools.psanalyzer {
            eprintln!("PSScriptAnalyzing {powershell_path}");
            let output = psanalyzer.output(|cmd| cmd.arg(powershell_path).arg("-EnableExit"))?;

            if !output.status.success() {
                eprintln!("{}", String::from_utf8_lossy(&output.stdout));
                eprintln!("see https://learn.microsoft.com/en-ca/powershell/utility-modules/psscriptanalyzer/rules/readme\n");
                return Err(miette!("PsScriptAnalyzer found issues"));
            }
        }
        Ok(())
    }

    // Runs the installer script in a temp dir, attempting to set env vars to contain it to that dir
    #[allow(unused_variables)]
    fn runtest_shell(
        &self,
        name: &str,
        expected_bin_dir: &str,
        shell_path: &str,
    ) -> Result<(), miette::Report> {
        let app_name = &self.app_name;

        // Only do this on unix, and only do it if RUIN_MY_COMPUTER_WITH_INSTALLERS is set
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        if std::env::var(ENV_RUIN_ME)
            .map(|s| !s.is_empty())
            .unwrap_or(false)
        {
            use axoasset::LocalAsset;

            eprintln!("running installer.sh...");
            // Make installer.sh executable
            use std::os::unix::prelude::PermissionsExt;
            std::fs::set_permissions(shell_path, std::fs::Permissions::from_mode(0o777)).unwrap();
            let script = CommandInfo::new_unchecked("installer.sh", Some(shell_path));

            // Create/clobber a temp dir in target
            let cur_dir =
                Utf8PathBuf::from_path_buf(std::env::current_dir().unwrap()).expect("non-utf8 cwd");
            let parent = cur_dir.parent().unwrap();
            let tempdir = parent.join(format!("{app_name}__{name}"));
            if tempdir.exists() {
                std::fs::remove_dir_all(&tempdir).unwrap();
            }
            std::fs::create_dir_all(&tempdir).unwrap();

            // Run the installer script with:
            //
            // HOME="{tempdir}"            (for install-path=~/... and install-path=CARGO_HOME)
            // MY_ENV_VAR=".{app_name}"    (for install-path=$MY_ENV_VAR/...)
            // CARGO_HOME=null             (cargo test sets this so we have to clear it)
            let app_home = tempdir.join(format!(".{app_name}"));
            let _output = script.run(|cmd| {
                cmd.env("HOME", &tempdir)
                    .env("MY_ENV_VAR", &app_home)
                    .env_remove("CARGO_HOME")
            })?;
            // we could theoretically look at the above output and parse out the `source` line...

            // Check that the script wrote files where we expected
            let rcfile = tempdir.join(".profile");
            let expected_bin_dir = Utf8PathBuf::from(expected_bin_dir);
            let bin_dir = tempdir.join(&expected_bin_dir);
            let bin = bin_dir.join(app_name);
            let env_dir = if expected_bin_dir
                .components()
                .any(|d| d.as_str() == ".cargo")
            {
                bin_dir.parent().unwrap()
            } else {
                &bin_dir
            };
            let env_script = env_dir.join("env");

            assert!(bin_dir.exists(), "bin dir wasn't created");
            assert!(rcfile.exists(), ".profile wasn't created");
            assert!(env_script.exists(), "env script wasn't created");
            assert!(bin.exists(), "bin wasn't created");

            // Check that the binary works
            let app = CommandInfo::new(app_name, Some(bin.as_str())).expect("failed to run app");
            assert!(app.version.is_some(), "failed to get app version");
            eprintln!("installer.sh worked!");

            // Check that sourcing the env script works (puts the right thing on path)
            eprintln!("checking env script..");

            // Make a test.sh script that sources the env script and then runs `which {app_name}`
            let test_script_path = tempdir.join("test.sh");
            let test_script_text = format!(
                r#"#!/bin/sh

. {rcfile}
which {app_name}
"#
            );
            LocalAsset::write_new(&test_script_text, &test_script_path)?;
            std::fs::set_permissions(&test_script_path, std::fs::Permissions::from_mode(0o777))
                .unwrap();
            let sh = CommandInfo::new_unchecked("test.sh", Some(test_script_path.as_str()));

            // Run test.sh and check that the output matches
            // NOTE: we only set HOME here to make sure that the early-bound vs late-bound env-var stuff works
            // ($HOME should be kept as a variable, but $MY_ENV_VAR and $CARGO_HOME should be resolved permanently
            // at install-time, so things should work if we don't set MY_ENV_VAR anymore)
            let output = sh.run(|cmd| cmd.env("HOME", &tempdir))?;
            assert_eq!(
                String::from_utf8(output.stdout).unwrap().trim(),
                bin.as_str(),
                "bin path wasn't right"
            );
        }
        Ok(())
    }

    // Run cargo-insta on the script bodies, so we can track any changes made
    fn snapshot_scripts(
        &self,
        name: &str,
        shell_path: &str,
        powershell_path: &str,
    ) -> Result<(), miette::Report> {
        // We make a single uber-snapshot for both scripts to avoid the annoyances of having multiple snapshots
        // in one test (necessitating rerunning it multiple times or passing special flags to get all the changes)
        let powershell_src = axoasset::SourceFile::load_local(powershell_path)?;
        let shell_src = axoasset::SourceFile::load_local(shell_path)?;
        let mut shell_snapshots = String::new();
        shell_snapshots.push_str("================ installer.sh ================\n");
        shell_snapshots.push_str(shell_src.contents());
        shell_snapshots.push_str("\n\n\n================ installer.ps1 ================\n");
        shell_snapshots.push_str(powershell_src.contents());

        insta::assert_snapshot!(format!("{name}-installers"), &shell_snapshots);
        Ok(())
    }
}

struct Tools {
    git: CommandInfo,
    cargo_dist: CommandInfo,
    shellcheck: Option<CommandInfo>,
    psanalyzer: Option<CommandInfo>,
}

impl Tools {
    fn new() -> Self {
        eprintln!("getting tools...");
        let git = CommandInfo::new("git", None).expect("git isn't installed");

        // If OVERRIDE_* is set, prefer that over the version that cargo built for us,
        // this lets us test our shippable builds.
        let cargo_dist_path = std::env::var("OVERRIDE_CARGO_BIN_EXE_cargo-dist")
            .unwrap_or_else(|_| STATIC_CARGO_DIST_BIN.to_owned());
        let cargo_dist = CommandInfo::new("cargo-dist", Some(&cargo_dist_path))
            .expect("cargo-dist isn't built!?");
        cargo_dist
            .version
            .as_ref()
            .expect("couldn't parse cargo-dist version!?");
        let shellcheck = CommandInfo::new("shellcheck", None);
        let psanalyzer = CommandInfo::new_powershell_command("Invoke-ScriptAnalyzer");

        Self {
            git,
            cargo_dist,
            shellcheck,
            psanalyzer,
        }
    }
}

struct CommandInfo {
    name: String,
    cmd: String,
    args: Vec<String>,
    version: Option<String>,
}

impl CommandInfo {
    /// Create a new command, checking that it works by running it with `--version`
    fn new(name: &str, path: Option<&str>) -> Option<Self> {
        let cmd = path.unwrap_or(name).to_owned();
        let output = Command::new(&cmd).arg("--version").output().ok()?;

        Some(CommandInfo {
            name: name.to_owned(),
            cmd,
            args: vec![],
            version: parse_version(output),
        })
    }

    /// Create a new command, don't check that it works
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn new_unchecked(name: &str, path: Option<&str>) -> Self {
        let cmd = path.unwrap_or(name).to_owned();

        CommandInfo {
            name: name.to_owned(),
            cmd,
            args: vec![],
            version: None,
        }
    }

    /// Create a new powershell command (for running things like powershell modules)
    fn new_powershell_command(name: &str) -> Option<Self> {
        let output = Command::new("powershell")
            .arg("-Command")
            .arg("Get-Command")
            .arg(name)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        Some(CommandInfo {
            name: name.to_owned(),
            cmd: "powershell".to_owned(),
            args: vec!["-Command".to_owned(), name.to_owned()],
            version: parse_version(output),
        })
    }

    /// Run with `.output` and check for errors/status
    fn run(
        &self,
        builder: impl FnOnce(&mut Command) -> &mut Command,
    ) -> Result<std::process::Output, miette::Report> {
        let mut command = Command::new(&self.cmd);
        command.args(&self.args);
        builder(&mut command);
        let output = command
            .output()
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to run \"{}\"", pretty_cmd(&self.name, &command)))?;
        if output.status.success() {
            Ok(output)
        } else {
            let mut out = String::new();
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            out.push_str("\nstdout:\n");
            out.push_str(&stdout);
            out.push_str("\nstderr:\n");
            out.push_str(&stderr);
            Err(miette!("{out}")).wrap_err_with(|| {
                format!(
                    "\"{}\" failed ({})",
                    pretty_cmd(&self.name, &command),
                    output.status
                )
            })
        }
    }

    /// Run with `.output` and only check for errors, DON'T check status
    fn output(
        &self,
        builder: impl FnOnce(&mut Command) -> &mut Command,
    ) -> Result<std::process::Output, miette::Report> {
        let mut command = Command::new(&self.cmd);
        command.args(&self.args);
        builder(&mut command);
        let output = command
            .output()
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to run \"{}\"", pretty_cmd(&self.name, &command)))?;
        Ok(output)
    }
}

/// Parse out the version from `--version` assuming the standard `app-name 0.1.0` format
fn parse_version(output: std::process::Output) -> Option<String> {
    let version_bytes = output.stdout;
    let version_full = String::from_utf8(version_bytes).ok()?;
    let version_line = version_full.lines().next()?;
    let version_suffix = version_line.split_once(' ')?.1.trim().to_owned();
    Some(version_suffix)
}

/// Fetch/update a repo to the given commit_sha
fn fetch_repo(
    git: &CommandInfo,
    repo_name: &str,
    repo_url: &str,
    commit_sha: &str,
) -> Result<(), miette::Report> {
    std::env::set_current_dir(TARGET_DIR).into_diagnostic()?;
    if Utf8PathBuf::from(repo_name).exists() {
        eprintln!("repo already cloned, updating it...");
        std::env::set_current_dir(repo_name).into_diagnostic()?;
        git.run(|c| c.arg("remote").arg("set-url").arg("origin").arg(repo_url))?;
        git.run(|c| c.arg("fetch").arg("origin").arg(commit_sha))?;
        git.run(|c| c.arg("reset").arg("--hard").arg("FETCH_HEAD"))?;
    } else {
        eprintln!("fetching {repo_url}");
        axoasset::LocalAsset::create_dir(repo_name)?;
        std::env::set_current_dir(repo_name).into_diagnostic()?;
        git.run(|c| c.arg("init"))?;
        git.run(|c| c.arg("remote").arg("add").arg("origin").arg(repo_url))?;
        git.run(|c| c.arg("fetch").arg("origin").arg(commit_sha))?;
        git.run(|c| c.arg("reset").arg("--hard").arg("FETCH_HEAD"))?;
    }

    Ok(())
}

/// Pretty print a command invocation
fn pretty_cmd(name: &str, cmd: &Command) -> String {
    let mut out = String::new();
    out.push_str(name);
    for arg in cmd.get_args() {
        out.push(' ');
        out.push_str(&arg.to_string_lossy())
    }
    out
}

/// Taken from cargo-insta to avoid copy-paste errors
///
/// Gets the ~name of the function running this macro
#[macro_export]
macro_rules! _function_name {
    () => {{
        fn f() {}
        fn type_name_of_val<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let mut name = type_name_of_val(f).strip_suffix("::f").unwrap_or("");
        while let Some(rest) = name.strip_suffix("::{{closure}}") {
            name = rest;
        }
        name.split_once("::")
            .map(|(_module, func)| func)
            .unwrap_or(name)
    }};
}
