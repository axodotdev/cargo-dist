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

            // The ./ at the start ensures Homebrew sees this as a path
            // reference and doesn't misinrepret it as a reference to a
            // formula in a tap.
            let relative_formula_path = format!("./{formula_path}");

            eprintln!("running brew install...");
            homebrew.output_checked(|cmd| cmd.arg("install").arg(&relative_formula_path))?;
            let prefix_output =
                homebrew.output_checked(|cmd| cmd.arg("--prefix").arg(&relative_formula_path))?;
            let prefix_raw = String::from_utf8(prefix_output.stdout).unwrap();
            let prefix = prefix_raw.strip_suffix('\n').unwrap();
            let bin = Utf8PathBuf::from(&prefix).join("bin");

            for bin_name in ctx.options.bins_with_aliases(&self.app_name, &self.bins) {
                let bin_path = bin.join(bin_name);
                assert!(bin_path.exists(), "bin wasn't created");
            }

            homebrew.output_checked(|cmd| cmd.arg("uninstall").arg(relative_formula_path))?;
        }
        Ok(())
    }
}
