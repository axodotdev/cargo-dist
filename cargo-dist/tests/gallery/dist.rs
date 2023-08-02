use axoasset::LocalAsset;
use camino::{Utf8Path, Utf8PathBuf};
use miette::miette;

use super::command::CommandInfo;
use super::errors::Result;
use super::repo::{Repo, TestContext, TestContextLock, ToolsImpl};

/// Set this env-var to enable running the installer scripts in temp dirs
///
/// If everything's working right, then no problem.
/// Otherwise MEGA DANGER in messing up your computer.
#[cfg(target_family = "unix")]
const ENV_RUIN_ME: &str = "RUIN_MY_COMPUTER_WITH_INSTALLERS";
/// Set this at runtime to override STATIC_CARGO_DIST_BIN
const ENV_RUNTIME_CARGO_DIST_BIN: &str = "OVERRIDE_CARGO_BIN_EXE_cargo-dist";
const STATIC_CARGO_DIST_BIN: &str = env!("CARGO_BIN_EXE_cargo-dist");

/// axolotlsay 0.1.0 is a nice simple project with shell+powershell+npm installers in its release
pub static AXOLOTLSAY: TestContextLock<Tools> = TestContextLock::new(&Repo {
    repo_owner: "axodotdev",
    repo_name: "axolotlsay",
    commit_sha: "6b8337fb742908e506296eab3371bb71b76283d7",
    app_name: "axolotlsay",
    bins: &["axolotlsay"],
});

pub struct Tools {
    pub git: CommandInfo,
    pub cargo_dist: CommandInfo,
    pub shellcheck: Option<CommandInfo>,
    pub psanalyzer: Option<CommandInfo>,
}

impl Tools {
    fn new() -> Self {
        eprintln!("getting tools...");
        let git = CommandInfo::new("git", None).expect("git isn't installed");

        // If OVERRIDE_* is set, prefer that over the version that cargo built for us,
        // this lets us test our shippable builds.
        let cargo_dist_path = std::env::var(ENV_RUNTIME_CARGO_DIST_BIN)
            .unwrap_or_else(|_| STATIC_CARGO_DIST_BIN.to_owned());
        let cargo_dist = CommandInfo::new("cargo-dist", Some(&cargo_dist_path))
            .expect("cargo-dist isn't built!?");
        cargo_dist
            .version()
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

impl ToolsImpl for Tools {
    fn git(&self) -> &CommandInfo {
        &self.git
    }
}
impl Default for Tools {
    fn default() -> Self {
        Self::new()
    }
}

pub struct DistResult {
    test_name: String,
    shell_installer_path: Option<Utf8PathBuf>,
    powershell_installer_path: Option<Utf8PathBuf>,
    npm_installer_package_path: Option<Utf8PathBuf>,
}

impl TestContext<Tools> {
    /// Run 'cargo dist build -aglobal' with the toml patched
    /// and return paths to various files that were generated
    pub fn cargo_dist_build_global(&self, test_name: &str, new_toml: String) -> Result<DistResult> {
        eprintln!("\n=============== running test: {test_name} =================");
        // patch the Cargo.toml
        self.patch_cargo_toml(new_toml)?;

        // If the cargo-dist target dir exists, delete it to avoid cross-contamination
        let out_path = Utf8Path::new("target/distrib/");
        if out_path.exists() {
            LocalAsset::remove_dir_all(out_path)?;
        }

        // build installers
        eprintln!("running cargo dist build -aglobal...");
        self.tools
            .cargo_dist
            .output_checked(|cmd| cmd.arg("dist").arg("build").arg("-aglobal"))?;

        self.load_dist_results(test_name)
    }
    /// Run 'cargo dist generate-ci' with the toml patched
    /// and return the contents of .github/workflows/release.yml
    pub fn cargo_dist_generate_ci(&self, test_name: &str, new_toml: String) -> Result<String> {
        eprintln!("\n=============== running test: {test_name} =================");
        // patch the Cargo.toml
        self.patch_cargo_toml(new_toml)?;

        let ci_path = Utf8Path::new(".github/workflows/release.yml");
        // Delete ci.yml if it already exists
        if ci_path.exists() {
            LocalAsset::remove_file(ci_path)?;
        }

        // run generate-ci
        eprintln!("running cargo dist build -aglobal...");
        self.tools
            .cargo_dist
            .output_checked(|cmd| cmd.arg("dist").arg("generate-ci"))?;

        // load the contents
        let result = LocalAsset::load_string(ci_path)?;

        Ok(result)
    }

    fn load_dist_results(&self, test_name: &str) -> Result<DistResult> {
        // read/analyze installers
        eprintln!("loading results...");
        let app_name = &self.repo.app_name;
        let ps_installer = Utf8PathBuf::from(format!("target/distrib/{app_name}-installer.ps1"));
        let sh_installer = Utf8PathBuf::from(format!("target/distrib/{app_name}-installer.sh"));
        let npm_installer =
            Utf8PathBuf::from(format!("target/distrib/{app_name}-npm-package.tar.gz"));

        Ok(DistResult {
            test_name: test_name.to_owned(),
            shell_installer_path: sh_installer.exists().then_some(sh_installer),
            powershell_installer_path: ps_installer.exists().then_some(ps_installer),
            npm_installer_package_path: npm_installer.exists().then_some(npm_installer),
        })
    }

    fn patch_cargo_toml(&self, new_toml: String) -> Result<()> {
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
}

impl DistResult {
    pub fn check_all(&self, ctx: &TestContext<Tools>, expected_bin_dir: &str) -> Result<()> {
        // If we have shellcheck, check our shell script
        self.shellcheck(ctx)?;

        // If we have PsScriptAnalyzer, check our powershell script
        self.psanalyzer(ctx)?;

        // If we can, run the script in a temp HOME
        self.runtest_shell_installer(ctx, expected_bin_dir)?;

        // Now that all other checks have passed, it's safe to check snapshots
        self.snapshot()?;

        Ok(())
    }

    /// Run shellcheck on the shell scripts
    pub fn shellcheck(&self, ctx: &TestContext<Tools>) -> Result<()> {
        // Only do this if the script is available
        let Some(script) = &self.shell_installer_path else {
            return Ok(());
        };
        // Only do this if the tool is available
        let Some(shellcheck) = &ctx.tools.shellcheck else {
            return Ok(());
        };
        eprintln!("shellchecking {script}");
        let output = shellcheck.output(|cmd| cmd.arg(script))?;

        if !output.status.success() {
            eprintln!("{}", String::from_utf8_lossy(&output.stdout));
            return Err(miette!("shellcheck found issues"));
        }
        Ok(())
    }

    /// Run PSScriptAnalyzer on the powershell scripts
    pub fn psanalyzer(&self, ctx: &TestContext<Tools>) -> Result<()> {
        // Only do this if the script is available
        let Some(script) = &self.powershell_installer_path else {
            return Ok(());
        };
        // Only do this if the tool is available
        let Some(psanalyzer) = &ctx.tools.psanalyzer else {
            return Ok(());
        };

        eprintln!("PSScriptAnalyzing {script}");
        let output = psanalyzer.output(|cmd| cmd.arg(script).arg("-EnableExit"))?;

        if !output.status.success() {
            eprintln!("{}", String::from_utf8_lossy(&output.stdout));
            eprintln!("see https://learn.microsoft.com/en-ca/powershell/utility-modules/psscriptanalyzer/rules/readme\n");
            return Err(miette!("PsScriptAnalyzer found issues"));
        }
        Ok(())
    }

    // Runs the installer script in a temp dir, attempting to set env vars to contain it to that dir
    #[allow(unused_variables)]
    pub fn runtest_shell_installer(
        &self,
        ctx: &TestContext<Tools>,
        expected_bin_dir: &str,
    ) -> Result<()> {
        // Only do this on unix, and only do it if RUIN_MY_COMPUTER_WITH_INSTALLERS is set
        #[cfg(target_family = "unix")]
        if std::env::var(ENV_RUIN_ME)
            .map(|s| !s.is_empty())
            .unwrap_or(false)
        {
            let app_name = ctx.repo.app_name;
            let test_name = &self.test_name;

            // only do this if the script exists
            let Some(shell_path) = &self.shell_installer_path else {
                return Ok(());
            };
            eprintln!("running installer.sh...");
            // Make installer.sh executable
            use std::os::unix::prelude::PermissionsExt;
            std::fs::set_permissions(shell_path, std::fs::Permissions::from_mode(0o755)).unwrap();
            let script = CommandInfo::new_unchecked("installer.sh", Some(shell_path.as_str()));

            // Create/clobber a temp dir in target
            let cur_dir =
                Utf8PathBuf::from_path_buf(std::env::current_dir().unwrap()).expect("non-utf8 cwd");
            let parent = cur_dir.parent().unwrap();
            let tempdir = parent.join(format!("{app_name}__{test_name}"));
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
            let _output = script.output_checked(|cmd| {
                cmd.env("HOME", &tempdir)
                    .env("MY_ENV_VAR", &app_home)
                    .env_remove("CARGO_HOME")
            })?;
            // we could theoretically look at the above output and parse out the `source` line...

            // Check that the script wrote files where we expected
            let rcfile = tempdir.join(".profile");
            let expected_bin_dir = Utf8PathBuf::from(expected_bin_dir);
            let bin_dir = tempdir.join(&expected_bin_dir);
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

            // Check that all the binaries work
            for bin_name in ctx.repo.bins {
                let bin_path = bin_dir.join(bin_name);
                assert!(bin_path.exists(), "bin wasn't created");

                let bin =
                    CommandInfo::new(bin_name, Some(bin_path.as_str())).expect("failed to run bin");
                assert!(bin.version().is_some(), "failed to get app version");
                eprintln!("installer.sh worked!");

                // Check that sourcing the env script works (puts the right thing on path)
                eprintln!("checking env script..");

                // Make a test.sh script that sources the env script and then runs `which {bin_name}`
                let test_script_path = tempdir.join("test.sh");
                let test_script_text = format!(
                    r#"#!/bin/sh

                . {rcfile}
                which {bin_name}
                "#
                );
                LocalAsset::write_new(&test_script_text, &test_script_path)?;
                std::fs::set_permissions(&test_script_path, std::fs::Permissions::from_mode(0o755))
                    .unwrap();
                let sh = CommandInfo::new_unchecked("test.sh", Some(test_script_path.as_str()));

                // Run test.sh and check that the output matches
                // NOTE: we only set HOME here to make sure that the early-bound vs late-bound env-var stuff works
                // ($HOME should be kept as a variable, but $MY_ENV_VAR and $CARGO_HOME should be resolved permanently
                // at install-time, so things should work if we don't set MY_ENV_VAR anymore)
                let output = sh.output_checked(|cmd| cmd.env("HOME", &tempdir))?;
                assert_eq!(
                    String::from_utf8(output.stdout).unwrap().trim(),
                    bin_path.as_str(),
                    "bin path wasn't right"
                );
            }
        }
        Ok(())
    }

    // Run cargo-insta on everything we care to snapshot
    pub fn snapshot(&self) -> Result<()> {
        // We make a single uber-snapshot for both scripts to avoid the annoyances of having multiple snapshots
        // in one test (necessitating rerunning it multiple times or passing special flags to get all the changes)
        let mut snapshots = String::new();

        Self::append_snapshot_file(
            &mut snapshots,
            "installer.sh",
            self.shell_installer_path.as_deref(),
        )?;
        Self::append_snapshot_file(
            &mut snapshots,
            "installer.ps1",
            self.powershell_installer_path.as_deref(),
        )?;
        Self::append_snapshot_tarball(
            &mut snapshots,
            "npm-package.tar.gz",
            self.npm_installer_package_path.as_deref(),
        )?;

        let test_name = &self.test_name;
        insta::assert_snapshot!(format!("{test_name}-installers"), &snapshots);
        Ok(())
    }

    fn append_snapshot_tarball(
        out: &mut String,
        name: &str,
        src_path: Option<&Utf8Path>,
    ) -> Result<()> {
        use std::io::Read;

        // Skip snapshotting this file if absent
        let Some(src_path) = src_path else {
            return Ok(());
        };

        let file = LocalAsset::load_bytes(src_path)?;
        let gz_decoder = flate2::read::GzDecoder::new(&file[..]);
        let mut tar_decoder = tar::Archive::new(gz_decoder);
        let entries = tar_decoder.entries().expect("couldn't read tar");
        for entry in entries {
            let mut entry = entry.expect("couldn't read tar entry");
            if entry.header().entry_type() == tar::EntryType::Regular {
                let path = entry
                    .path()
                    .expect("couldn't get tarred file's path")
                    .to_string_lossy()
                    .into_owned();
                let mut val = String::new();
                entry
                    .read_to_string(&mut val)
                    .expect("couldn't read tarred file to string");
                Self::append_snapshot_string(out, &format!("{name}/{path}"), &val)?;
            }
        }
        Ok(())
    }

    fn append_snapshot_file(
        out: &mut String,
        name: &str,
        src_path: Option<&Utf8Path>,
    ) -> Result<()> {
        // Skip snapshotting this file if absent
        let Some(src_path) = src_path else {
            return Ok(());
        };

        let src = axoasset::LocalAsset::load_string(src_path)?;
        Self::append_snapshot_string(out, name, &src)
    }

    fn append_snapshot_string(out: &mut String, name: &str, val: &str) -> Result<()> {
        use std::fmt::Write;

        writeln!(out, "\n\n================ {name} ================").unwrap();
        writeln!(out, "{val}").unwrap();
        Ok(())
    }
}
