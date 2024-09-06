use std::{path::PathBuf, process::Output};

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

            // Only do this if Homebrew is installed
            let Some(homebrew) = &ctx.tools.homebrew else {
                return Ok(());
            };

            // Homebrew fails to guess that this is a formula
            // file if it's not in a path named Formula,
            // so we need to put the formula in a temp path
            // to hint it correctly.
            // (We could also skip individual lints via
            // --except-cop on the `brew style` CLI, but that's
            // a bit too much of a game of whack a mole.)
            let temp_root = temp_dir::TempDir::new().unwrap();
            let formula_temp_path = create_formula_copy(&temp_root, formula_path).unwrap();

            // We perform linting here too because we want to both
            // lint and runtest the `brew style --fix`ed version.
            // We're unable to check the fixed version into the
            // snapshots since it doesn't work cross-platform, so
            // doing them both in one place means we don't have to
            // run it twice.
            let output = brew_style(homebrew, &formula_temp_path)?;
            if !output.status.success() {
                eprintln!("{}", String::from_utf8_lossy(&output.stdout));
                return Err(miette!("brew style found issues"));
            }

            eprintln!("running brew install...");
            homebrew.output_checked(|cmd| cmd.arg("install").arg(&formula_temp_path))?;
            let prefix_output =
                homebrew.output_checked(|cmd| cmd.arg("--prefix").arg(&formula_temp_path))?;
            let prefix_raw = String::from_utf8(prefix_output.stdout).unwrap();
            let prefix = prefix_raw.strip_suffix('\n').unwrap();
            let bin = Utf8PathBuf::from(&prefix).join("bin");

            for bin_name in ctx.options.bins_with_aliases(&self.app_name, &self.bins) {
                let bin_path = bin.join(bin_name);
                assert!(bin_path.exists(), "bin wasn't created");
            }

            homebrew.output_checked(|cmd| cmd.arg("uninstall").arg(formula_temp_path))?;
        }
        Ok(())
    }
}

fn create_formula_copy(
    temp_root: &temp_dir::TempDir,
    formula_path: &Utf8PathBuf,
) -> std::io::Result<PathBuf> {
    let formula_temp_root = temp_root.path().join("Formula");
    std::fs::create_dir(&formula_temp_root)?;
    let formula_temp_path = formula_temp_root.join(formula_path.file_name().unwrap());
    std::fs::copy(formula_path, &formula_temp_path)?;

    Ok(formula_temp_path)
}

fn brew_style(homebrew: &CommandInfo, path: &PathBuf) -> Result<Output> {
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
            .arg("FormulaAudit/Homepage,FormulaAudit/Desc,FormulaAuditStrict")
            // Applying --fix will ensure that fixable
            // style issues won't be treated as errors.
            .arg("--fix")
            .arg(path)
    })
}
