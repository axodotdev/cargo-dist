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
            let xdg_data_home = tempdir.join(".local/share");
            let test_legacy_env_migration = ctx.options.shell_legacy_env_migration(app_name);
            let test_user_owned_env = ctx.options.shell_user_owned_env(app_name);
            assert!(
                !(test_legacy_env_migration && test_user_owned_env),
                "shell migration fixtures are mutually exclusive"
            );
            let seeded_path = if test_legacy_env_migration {
                Some(seed_legacy_env_install(
                    &tempdir,
                    app_name,
                    expected_bin_dir,
                )?)
            } else {
                if test_user_owned_env {
                    seed_user_owned_env_install(&tempdir, app_name, expected_bin_dir)?;
                }
                None
            };

            let _output = script.output_checked(|cmd| {
                let cmd = cmd
                    .env("HOME", &tempdir)
                    .env("ZDOTDIR", &tempdir)
                    .env("MY_ENV_VAR", &app_home)
                    .env("XDG_DATA_HOME", &xdg_data_home)
                    .env_remove("XDG_BIN_HOME")
                    .env_remove("CARGO_HOME")
                    .env_remove("XDG_CONFIG_HOME");
                if let Some(path) = &seeded_path {
                    cmd.env("PATH", path);
                }
                cmd
            })?;
            // we could theoretically look at the above output and parse out the `source` line...

            // Check that the script wrote files where we expected
            let rcfiles = &[
                // .profile is shared between POSIX and Bash as the default
                tempdir.join(".profile"),
                tempdir.join(".zshrc"),
            ];
            let receipt_file = tempdir.join(format!(".config/{app_name}/{app_name}-receipt.json"));
            let expected_bin_dir = Utf8PathBuf::from(expected_bin_dir);
            let bin_dir = tempdir.join(&expected_bin_dir);
            let env_script = if expected_bin_dir
                .components()
                .any(|d| d.as_str() == ".cargo")
            {
                bin_dir.parent().unwrap().join("env")
            } else {
                tempdir.join(format!(".config/{app_name}/env.sh"))
            };
            let fish_env_script = if expected_bin_dir
                .components()
                .any(|d| d.as_str() == ".cargo")
            {
                bin_dir.parent().unwrap().join("env.fish")
            } else {
                tempdir.join(format!(".config/{app_name}/env.fish"))
            };

            assert!(bin_dir.exists(), "bin dir wasn't created");
            for rcfile in rcfiles {
                assert!(rcfile.exists(), "{} wasn't created", rcfile);
            }
            assert!(env_script.exists(), "env script wasn't created");
            assert!(fish_env_script.exists(), "fish env script wasn't created");

            if test_legacy_env_migration {
                assert_legacy_env_migrated(
                    &tempdir,
                    app_name,
                    expected_bin_dir.as_str(),
                    &bin_dir,
                    rcfiles,
                );
            }
            if test_user_owned_env {
                assert_user_owned_env_preserved(
                    &tempdir,
                    app_name,
                    expected_bin_dir.as_str(),
                    &bin_dir,
                    rcfiles,
                );
            }

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

#[cfg(target_family = "unix")]
fn seed_legacy_env_install(
    tempdir: &Utf8Path,
    app_name: &str,
    expected_bin_dir: &str,
) -> Result<String> {
    let bin_dir = tempdir.join(expected_bin_dir);
    let bin_dir_expr = format!("$HOME/{expected_bin_dir}");
    let legacy_env_script = bin_dir.join("env");
    let legacy_fish_env_script = bin_dir.join("env.fish");
    let fish_conf_dir = tempdir.join(".config/fish/conf.d");

    std::fs::create_dir_all(&bin_dir).unwrap();
    std::fs::create_dir_all(&fish_conf_dir).unwrap();
    LocalAsset::write_new(&legacy_env_script_sh(&bin_dir_expr), &legacy_env_script)?;
    LocalAsset::write_new(
        &legacy_env_script_fish(&bin_dir_expr),
        &legacy_fish_env_script,
    )?;
    seed_legacy_source_lines(tempdir, app_name, &bin_dir_expr)?;

    Ok(format!(
        "{}:{}",
        bin_dir,
        std::env::var("PATH").unwrap_or_default()
    ))
}

#[cfg(target_family = "unix")]
fn seed_user_owned_env_install(
    tempdir: &Utf8Path,
    app_name: &str,
    expected_bin_dir: &str,
) -> Result<()> {
    let bin_dir = tempdir.join(expected_bin_dir);
    let bin_dir_expr = format!("$HOME/{expected_bin_dir}");
    let legacy_env_script = bin_dir.join("env");
    let legacy_fish_env_script = bin_dir.join("env.fish");
    let fish_conf_dir = tempdir.join(".config/fish/conf.d");

    std::fs::create_dir_all(&bin_dir).unwrap();
    std::fs::create_dir_all(&fish_conf_dir).unwrap();
    LocalAsset::write_new(
        &format!(
            "{}# user customization\n",
            legacy_env_script_sh(&bin_dir_expr)
        ),
        &legacy_env_script,
    )?;
    LocalAsset::write_new(
        &format!(
            "{}# user customization\n",
            legacy_env_script_fish(&bin_dir_expr)
        ),
        &legacy_fish_env_script,
    )?;
    seed_legacy_source_lines(tempdir, app_name, &bin_dir_expr)
}

#[cfg(target_family = "unix")]
fn seed_legacy_source_lines(tempdir: &Utf8Path, app_name: &str, bin_dir_expr: &str) -> Result<()> {
    let fish_conf_dir = tempdir.join(".config/fish/conf.d");
    std::fs::create_dir_all(&fish_conf_dir).unwrap();
    LocalAsset::write_new(
        &format!(". \"{bin_dir_expr}/env\"\n"),
        tempdir.join(".profile"),
    )?;
    LocalAsset::write_new(
        &format!("source \"{bin_dir_expr}/env\"\n"),
        tempdir.join(".zshrc"),
    )?;
    LocalAsset::write_new(
        &format!("source \"{bin_dir_expr}/env.fish\"\n"),
        fish_conf_dir.join(format!("{app_name}.env.fish")),
    )?;
    Ok(())
}

#[cfg(target_family = "unix")]
fn legacy_env_script_sh(bin_dir_expr: &str) -> String {
    format!(
        r#"#!/bin/sh
# add binaries to PATH if they aren't added yet
# affix colons on either side of $PATH to simplify matching
case ":${{PATH}}:" in
    *:"{bin_dir_expr}":*)
        ;;
    *)
        # Prepending path in case a system-installed binary needs to be overridden
        export PATH="{bin_dir_expr}:$PATH"
        ;;
esac
"#
    )
}

#[cfg(target_family = "unix")]
fn legacy_env_script_fish(bin_dir_expr: &str) -> String {
    format!(
        r#"if not contains "{bin_dir_expr}" $PATH
    # Prepending path in case a system-installed binary needs to be overridden
    set -x PATH "{bin_dir_expr}" $PATH
end
"#
    )
}

#[cfg(target_family = "unix")]
fn assert_legacy_env_migrated(
    tempdir: &Utf8Path,
    app_name: &str,
    expected_bin_dir: &str,
    bin_dir: &Utf8Path,
    rcfiles: &[Utf8PathBuf],
) {
    assert!(
        !bin_dir.join("env").exists(),
        "legacy env script was left in the binary directory"
    );
    assert!(
        !bin_dir.join("env.fish").exists(),
        "legacy fish env script was left in the binary directory"
    );

    let legacy_env_script = format!("$HOME/{expected_bin_dir}/env");
    let env_script = format!("$HOME/.config/{app_name}/env.sh");
    for rcfile in rcfiles {
        let contents = std::fs::read_to_string(rcfile).unwrap();
        assert!(
            contents.contains(&format!(". \"{env_script}\""))
                || contents.contains(&format!("source \"{env_script}\"")),
            "{} wasn't migrated to the new env script",
            rcfile
        );
        assert!(
            !contents.contains(&legacy_env_script),
            "{} still sources the legacy env script",
            rcfile
        );
    }

    let fish_rcfile = tempdir.join(format!(".config/fish/conf.d/{app_name}.env.fish"));
    let contents = std::fs::read_to_string(&fish_rcfile).unwrap();
    let legacy_fish_env_script = format!("$HOME/{expected_bin_dir}/env.fish");
    let fish_env_script = format!("$HOME/.config/{app_name}/env.fish");
    assert!(
        contents.contains(&format!("source \"{fish_env_script}\"")),
        "{} wasn't migrated to the new fish env script",
        fish_rcfile
    );
    assert!(
        !contents.contains(&legacy_fish_env_script),
        "{} still sources the legacy fish env script",
        fish_rcfile
    );
}

#[cfg(target_family = "unix")]
fn assert_user_owned_env_preserved(
    tempdir: &Utf8Path,
    app_name: &str,
    expected_bin_dir: &str,
    bin_dir: &Utf8Path,
    rcfiles: &[Utf8PathBuf],
) {
    let bin_dir_expr = format!("$HOME/{expected_bin_dir}");
    assert_eq!(
        std::fs::read_to_string(bin_dir.join("env")).unwrap(),
        format!(
            "{}# user customization\n",
            legacy_env_script_sh(&bin_dir_expr)
        ),
        "user-owned env script was modified"
    );
    assert_eq!(
        std::fs::read_to_string(bin_dir.join("env.fish")).unwrap(),
        format!(
            "{}# user customization\n",
            legacy_env_script_fish(&bin_dir_expr)
        ),
        "user-owned fish env script was modified"
    );

    let legacy_env_script = format!("$HOME/{expected_bin_dir}/env");
    for rcfile in rcfiles {
        let contents = std::fs::read_to_string(rcfile).unwrap();
        assert!(
            contents.contains(&legacy_env_script),
            "{} no longer sources the user-owned env script",
            rcfile
        );
    }

    let fish_rcfile = tempdir.join(format!(".config/fish/conf.d/{app_name}.env.fish"));
    let contents = std::fs::read_to_string(&fish_rcfile).unwrap();
    assert!(
        contents.contains(&format!("$HOME/{expected_bin_dir}/env.fish")),
        "{} no longer sources the user-owned fish env script",
        fish_rcfile
    );
}
