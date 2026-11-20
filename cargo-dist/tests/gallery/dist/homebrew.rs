use std::process::Output;

use super::*;

impl AppResult {
    // Runs the installer script in the system's Homebrew installation
    #[allow(unused_variables)]
    pub fn runtest_homebrew_installer(&self, ctx: &TestContext<Tools>) -> Result<()> {
        // Only do this if we trust hashes (outside cfg so the compiler knows we use this)
        if !self.trust_hashes {
            return Ok(());
        }

        // Only do this on macOS, and only do it if RUIN_MY_COMPUTER_WITH_INSTALLERS is set
        if std::env::var(ENV_RUIN_ME)
            .map(|s| s == "homebrew" || s == "all")
            .unwrap_or(false)
        {
            // only do this if the formula exists
            let Some(formula_path) = &self.homebrew_installer_path else {
                return Ok(());
            };

            // don't do this if the test asked not to do it,
            // cf. https://github.com/axodotdev/cargo-dist/issues/1525
            if self.homebrew_skip_install {
                return Ok(());
            }

            // Only do this if Homebrew is installed
            let Some(homebrew) = &ctx.tools.homebrew else {
                return Ok(());
            };

            // Homebrew insists every single formula has to be in a tap,
            // so we have to set this up as a temporary tap for testing
            // before we can start working with it.
            let brew_prefix = brew_prefix(homebrew)?;
            let tap_path = Utf8PathBuf::from(&brew_prefix)
                .join("Library")
                .join("Taps")
                .join("axodotdev")
                .join("homebrew-citesttap");
            // If this exists from a previous test run, remove it
            // before running.
            if tap_path.exists() {
                std::fs::remove_dir_all(&tap_path).unwrap();
            }

            let tap_formula_path = create_formula_copy(&tap_path, formula_path).unwrap();
            let tap_name = "axodotdev/citesttap";
            // Formula name is the same as the filename minus the .rb extension
            let formula_basename = formula_path.file_prefix().unwrap();
            let formula_name = format!("{tap_name}/{formula_basename}");

            // We perform linting here too because we want to both
            // lint and runtest the `brew style --fix`ed version.
            // We're unable to check the fixed version into the
            // snapshots since it doesn't work cross-platform, so
            // doing them both in one place means we don't have to
            // run it twice.
            let output = brew_style(homebrew, &formula_name)?;
            if !output.status.success() {
                eprintln!("{}", String::from_utf8_lossy(&output.stdout));
                return Err(miette!("brew style found issues"));
            }

            eprintln!("running brew install...");
            homebrew.output_checked(|cmd| {
                // Set HOMEBREW_TESTS and HOMEBREW_DEVELOPER
                // so we can install from a file path
                // https://github.com/Homebrew/brew/blob/a1f112f3fea3a47c689317da7dced8818917b03d/Library/Homebrew/env_config.rb#L677-L679
                cmd.env("HOMEBREW_TESTS", "1")
                    .env("HOMEBREW_DEVELOPER", "1")
                    .arg("install")
                    .arg(&formula_name)
            })?;
            let prefix_output = homebrew.output_checked(|cmd| {
                cmd.env("HOMEBREW_TESTS", "1")
                    .env("HOMEBREW_DEVELOPER", "1")
                    .arg("--prefix")
                    .arg(&formula_name)
            })?;
            let prefix_raw = String::from_utf8(prefix_output.stdout).unwrap();
            let prefix = prefix_raw.strip_suffix('\n').unwrap();
            let bin = Utf8PathBuf::from(&prefix).join("bin");

            for bin_name in ctx.options.bins_with_aliases(&self.app_name, &self.bins) {
                let bin_path = bin.join(bin_name);
                assert!(bin_path.exists(), "bin wasn't created");
            }

            homebrew.output_checked(|cmd| {
                cmd.env("HOMEBREW_TESTS", "1")
                    .env("HOMEBREW_DEVELOPER", "1")
                    .arg("uninstall")
                    .arg(formula_name)
            })?;

            // Finally, remove the tap entirely
            homebrew.output_checked(|cmd| cmd.arg("untap").arg(tap_name))?;
        }
        Ok(())
    }
}

fn create_formula_copy(
    temp_root: &Utf8PathBuf,
    formula_path: &Utf8PathBuf,
) -> std::io::Result<Utf8PathBuf> {
    let formula_temp_root = temp_root.join("Formula");
    std::fs::create_dir_all(&formula_temp_root)?;
    let formula_temp_path = formula_temp_root.join(formula_path.file_name().unwrap());
    std::fs::copy(formula_path, &formula_temp_path)?;

    Ok(formula_temp_path)
}

fn brew_prefix(homebrew: &CommandInfo) -> Result<String> {
    let output = homebrew.output(|cmd| cmd.arg("--prefix"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    Ok(stdout.trim().to_string())
}

fn brew_style(homebrew: &CommandInfo, path: &str) -> Result<Output> {
    homebrew.output(|cmd| {
        cmd.arg("style")
            // We ignore audits for user-supplied metadata,
            // since we avoid rewriting those on behalf of
            // the user. We also avoid the homepage nit,
            // because if the user doesn't supply a homepage
            // it's correct that we don't generate one.
            // We add FormulaAuditStrict because that's the
            // default exclusion, and adding anything to
            // --except-cops overrides it.
            .arg("--except-cops")
            .arg("FormulaAudit/Homepage,FormulaAudit/Desc,FormulaAudit/Test,FormulaAuditStrict")
            // Applying --fix will ensure that fixable
            // style issues won't be treated as errors.
            .arg("--fix")
            .arg(path)
    })
}
