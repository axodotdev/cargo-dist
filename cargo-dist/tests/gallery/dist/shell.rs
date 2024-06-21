use super::*;

impl AppResult {
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
            .map(|s| s == "shell" || s == "all")
            .unwrap_or(false)
        {
            let app_name = &self.app_name;
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
            let repo_dir = &ctx.repo_dir;
            let repo_id = &ctx.repo_id;
            let parent = repo_dir.parent().unwrap();
            let tempdir = parent.join(format!("{repo_id}__{test_name}"));
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
                    .env("ZDOTDIR", &tempdir)
                    .env("MY_ENV_VAR", &app_home)
                    .env_remove("CARGO_HOME")
            })?;
            // we could theoretically look at the above output and parse out the `source` line...

            // Check that the script wrote files where we expected
            let rcfiles = &[
                tempdir.join(".profile"),
                tempdir.join(".bash_profile"),
                tempdir.join(".zshrc"),
            ];
            let receipt_file = tempdir.join(format!(".config/{app_name}/{app_name}-receipt.json"));
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
            for rcfile in rcfiles {
                assert!(rcfile.exists(), "{} wasn't created", rcfile);
            }
            assert!(env_script.exists(), "env script wasn't created");

            // Check that all the binaries work
            for bin_name in ctx.options.bins_with_aliases(&self.app_name, &self.bins) {
                let bin_path = bin_dir.join(&bin_name);
                assert!(bin_path.exists(), "bin wasn't created");

                let bin = CommandInfo::new(&bin_name, Some(bin_path.as_str()))
                    .expect("failed to run bin");
                assert!(bin.version().is_some(), "failed to get app version");
                eprintln!("installer.sh worked!");

                // Check that sourcing the env script works (puts the right thing on path)
                eprintln!("checking env script..");

                // Make a test.sh script that sources the env script and then runs `which {bin_name}`
                let test_script_path = tempdir.join("test.sh");
                let test_script_text = format!(
                    r#"#!/bin/sh

                . {}
                which {bin_name}
                "#,
                    rcfiles.first().expect("rcfiles was empty?!")
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

            // Check the install receipts
            self.check_install_receipt(ctx, &bin_dir, &receipt_file, "");
        }
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
}
