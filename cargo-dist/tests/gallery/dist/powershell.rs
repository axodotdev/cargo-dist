use super::*;

impl AppResult {
    // Runs the installer script in a temp dir, attempting to set env vars to contain it to that dir
    #[allow(unused_variables)]
    pub fn runtest_powershell_installer(
        &self,
        ctx: &TestContext<Tools>,
        expected_bin_dir: &str,
    ) -> Result<()> {
        // Only do this on windows, and only do it if RUIN_MY_COMPUTER_WITH_INSTALLERS is set
        #[cfg(target_family = "windows")]
        if std::env::var(ENV_RUIN_ME)
            .map(|s| s == "powershell" || s == "all")
            .unwrap_or(false)
        {
            fn run_ps1_script(
                powershell: &CommandInfo,
                tempdir: &Utf8Path,
                script_name: &str,
                script_body: &str,
            ) -> Result<String> {
                let script_path = tempdir.join("test.ps1");
                LocalAsset::write_new(script_body, &script_path)?;
                let output = powershell.output_checked(|cmd| {
                    cmd.arg("-c")
                        .arg(script_path)
                        .env("UserProfile", tempdir)
                        .env_remove("PsModulePath")
                })?;
                eprintln!("{}", String::from_utf8(output.stderr).unwrap());
                Ok(String::from_utf8(output.stdout).unwrap().trim().to_owned())
            }

            let app_name = &self.app_name;
            let test_name = &self.test_name;

            // only do this if the script exists
            let Some(shell_path) = &self.powershell_installer_path else {
                return Ok(());
            };
            eprintln!("running installer.ps1...");
            let powershell = CommandInfo::new_unchecked("powershell", None);

            // Create/clobber a temp dir in target
            let repo_dir = &ctx.repo_dir;
            let repo_id = &ctx.repo_id;
            let parent = repo_dir.parent().unwrap();
            let tempdir = parent.join(format!("{repo_id}__{test_name}"));
            let appdata = tempdir.join("AppData/Local");
            if appdata.exists() {
                std::fs::remove_dir_all(&appdata).unwrap();
            }
            std::fs::create_dir_all(&appdata).unwrap();

            // save the current PATH in the registry
            let saved_path = run_ps1_script(
                &powershell,
                &tempdir,
                "savepath.ps1",
                r#"
            $Item = Get-Item -Path "HKCU:\Environment"
            $RegPath = $Item | Get-ItemPropertyValue -Name "Path"
            return $RegPath
            "#,
            )?;
            assert!(!saved_path.trim().is_empty(), "failed to load path");
            eprintln!("backing up PATH: {saved_path}\n");

            // on exit, retore the current PATH in the registry, even if we panic
            struct RestorePath<'a> {
                powershell: &'a CommandInfo,
                tempdir: &'a Utf8Path,
                saved_path: String,
            }
            impl Drop for RestorePath<'_> {
                fn drop(&mut self) {
                    let saved_path = &self.saved_path;
                    eprintln!("restoring PATH: {saved_path}\n");
                    run_ps1_script(self.powershell, self.tempdir, "restorepath.ps1", &format!(r#"
                        $Item = Get-Item -Path "HKCU:\Environment"
                        $Item | New-ItemProperty -Name "Path" -Value "{saved_path}" -PropertyType String -Force | Out-Null
                    "#)).unwrap();
                }
            }
            let _restore = RestorePath {
                powershell: &powershell,
                tempdir: &tempdir,
                saved_path,
            };

            // Run the installer script with:
            //
            // UserProfile="{tempdir}"     (for install-path=~/... and install-path=CARGO_HOME)
            // LOCALAPPDATA="{tempdir}/AppData/Local" (for install receipts)
            // MY_ENV_VAR=".{app_name}"    (for install-path=$MY_ENV_VAR/...)
            // CARGO_HOME=null             (cargo test sets this so we have to clear it)
            // PSModulePath=null           (https://github.com/PowerShell/PowerShell/issues/18530)
            let app_home = tempdir.join(format!(".{app_name}"));
            let output = powershell.output_checked(|cmd| {
                cmd.arg("-c")
                    .arg(shell_path)
                    .arg("-Verbose")
                    .env("UserProfile", &tempdir)
                    .env("LOCALAPPDATA", &appdata)
                    .env("MY_ENV_VAR", &app_home)
                    .env_remove("CARGO_HOME")
                    .env_remove("PSModulePath")
            })?;
            eprintln!(
                "installer.ps1 stdout:\n{}",
                String::from_utf8(output.stdout).unwrap()
            );
            eprintln!(
                "installer.ps1 stderr:\n{}",
                String::from_utf8(output.stderr).unwrap()
            );
            // log the current PATH in the registry
            let new_path = run_ps1_script(
                &powershell,
                &tempdir,
                "savepath.ps1",
                r#"
            $Item = Get-Item -Path "HKCU:\Environment"
            $RegPath = $Item | Get-ItemPropertyValue -Name "Path"
            return $RegPath
            "#,
            )?;
            assert!(!new_path.trim().is_empty(), "failed to load path");
            eprintln!("PATH updated to: {new_path}\n");

            // Check that the script wrote files where we expected
            let receipt_file = appdata.join(format!("{app_name}\\{app_name}-receipt.json"));
            let expected_bin_dir = Utf8PathBuf::from(expected_bin_dir.replace('/', "\\"));
            let bin_dir = tempdir.join(expected_bin_dir);

            assert!(bin_dir.exists(), "bin dir wasn't created");

            // Check that all the binaries work
            for bin_name in ctx.options.bins_with_aliases(app_name, &self.bins) {
                let bin_path = bin_dir.join(format!("{bin_name}.exe"));
                assert!(bin_path.exists(), "{bin_name} wasn't created");

                let bin = CommandInfo::new(&bin_name, Some(bin_path.as_str()))
                    .expect("failed to run bin");
                assert!(bin.version().is_some(), "failed to get app version");

                // checking path...
                // Make a test.ps1 script that runs `where.exe {bin_name}`
                //
                // (note that "where" and "where.exe" are completely different things...)
                //
                // also note that HKCU:\Environment\PATH is not actually the full PATH
                // a shell will have, so preprend it to the current PATH (if we don't do
                // this then where.exe won't be on PATH anymore!)
                let empirical_path = run_ps1_script(
                    &powershell,
                    &tempdir,
                    "test.ps1",
                    &format!(
                        r#"
                $Item = Get-Item -Path "HKCU:\Environment"
                $RegPath = $Item | Get-ItemPropertyValue -Name "Path"
                $env:PATH = "$RegPath;$env:PATH"
                $Res = where.exe {bin_name}
                return $Res
                "#
                    ),
                )?;
                // where.exe will return every matching result, but the one we
                // want, the one selected by PATH, should appear first.
                assert_eq!(
                    empirical_path.lines().next().unwrap_or_default(),
                    bin_path.as_str(),
                    "{bin_name} path wasn't right"
                );
            }
            // check the install receipts
            self.check_install_receipt(ctx, &bin_dir, &receipt_file, ".exe");
            eprintln!("installer.ps1 worked!");
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
}
