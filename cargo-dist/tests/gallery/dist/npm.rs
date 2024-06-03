use super::*;

impl DistResult {
    // Runs the installer script in the system's Homebrew installation
    #[allow(unused_variables)]
    pub fn runtest_npm_installer(&self, ctx: &TestContext<Tools>) -> Result<()> {
        if !std::env::var(ENV_RUIN_ME)
            .map(|s| s == "npm" || s == "all")
            .unwrap_or(false)
        {
            return Ok(());
        }

        // only do this if the formula exists
        let Some(package_tarball_path) = &self.npm_installer_package_path else {
            return Ok(());
        };
        // force the tarball path to be absolute
        let package_tarball_path = if package_tarball_path.is_relative() {
            let curdir =
                Utf8PathBuf::from_path_buf(std::env::current_dir().expect("couldn't get curdir"))
                    .expect("curdir not utf8");
            curdir.join(package_tarball_path)
        } else {
            package_tarball_path.to_owned()
        };
        // Only do this if npm and tar are installed
        let Some(tar) = &ctx.tools.tar else {
            return Ok(());
        };
        let app_name = ctx.repo.app_name;
        let test_name = &self.test_name;
        let bins = ctx.repo.bins;

        // Create/clobber a temp dir in target
        let repo_dir = &ctx.repo_dir;
        let repo_id = &ctx.repo_id;
        let parent = repo_dir.parent().unwrap();

        let tempdir = parent.join(format!("{repo_id}__{test_name}"));
        runtest_npm(
            &ctx.tools.npm,
            tar,
            &tempdir,
            &package_tarball_path,
            app_name,
            bins,
        )?;
        runtest_pnpm(
            &ctx.tools.npm,
            &ctx.tools.pnpm,
            tar,
            &tempdir,
            &package_tarball_path,
            app_name,
            bins,
        )?;
        runtest_yarn(
            &ctx.tools.npm,
            &ctx.tools.yarn,
            tar,
            &tempdir,
            &package_tarball_path,
            app_name,
            bins,
        )?;

        Ok(())
    }
}

fn runtest_yarn(
    npm: &Option<CommandInfo>,
    yarn: &Option<CommandInfo>,
    tar: &CommandInfo,
    tempdir: &Utf8Path,
    package_tarball_path: &Utf8Path,
    app_name: &str,
    bins: &[&str],
) -> Result<()> {
    let Some(npm) = npm else { return Ok(()) };
    let Some(yarn) = yarn else { return Ok(()) };

    clear_tempdir(tempdir);

    // Have npm install/unpack the tarball to a project
    eprintln!("running npm install...");
    let parent_package_dir = tempdir.to_owned();
    yarn_install_tarball_package(yarn, &parent_package_dir, package_tarball_path)?;

    // Run the installed app
    eprintln!("npm exec'ing installed app...");
    run_installed_package(npm, &parent_package_dir, app_name, bins)?;

    // Now let's hop into the installed package and have it lint itself
    eprintln!("linting installed app...");
    unpack_tarball_package(tar, &parent_package_dir, package_tarball_path)?;
    let package_dir = parent_package_dir.join("package");
    lint_package(npm, &package_dir, app_name)?;

    Ok(())
}

fn runtest_pnpm(
    npm: &Option<CommandInfo>,
    pnpm: &Option<CommandInfo>,
    tar: &CommandInfo,
    tempdir: &Utf8Path,
    package_tarball_path: &Utf8Path,
    app_name: &str,
    bins: &[&str],
) -> Result<()> {
    let Some(npm) = npm else { return Ok(()) };
    let Some(pnpm) = pnpm else { return Ok(()) };

    clear_tempdir(tempdir);

    // Have npm install/unpack the tarball to a project
    eprintln!("running npm install...");
    let parent_package_dir = tempdir.to_owned();
    install_tarball_package(pnpm, &parent_package_dir, package_tarball_path)?;

    // Run the installed app
    eprintln!("npm exec'ing installed app...");
    run_installed_package(npm, &parent_package_dir, app_name, bins)?;

    // Now let's hop into the installed package and have it lint itself
    eprintln!("linting installed app...");
    unpack_tarball_package(tar, &parent_package_dir, package_tarball_path)?;
    let package_dir = parent_package_dir.join("package");
    lint_package(npm, &package_dir, app_name)?;

    Ok(())
}

fn runtest_npm(
    npm: &Option<CommandInfo>,
    tar: &CommandInfo,
    tempdir: &Utf8Path,
    package_tarball_path: &Utf8Path,
    app_name: &str,
    bins: &[&str],
) -> Result<()> {
    let Some(npm) = npm else { return Ok(()) };

    clear_tempdir(tempdir);

    // Have npm install/unpack the tarball to a project
    eprintln!("running npm install...");
    let parent_package_dir = tempdir.to_owned();
    install_tarball_package(npm, &parent_package_dir, package_tarball_path)?;

    // Run the installed app
    eprintln!("npm exec'ing installed app...");
    run_installed_package(npm, &parent_package_dir, app_name, bins)?;

    // Now let's hop into the installed package and have it lint itself
    eprintln!("linting installed app...");
    unpack_tarball_package(tar, &parent_package_dir, package_tarball_path)?;
    let package_dir = parent_package_dir.join("package");
    lint_package(npm, &package_dir, app_name)?;

    Ok(())
}

fn clear_tempdir(tempdir: &Utf8Path) {
    if tempdir.exists() {
        std::fs::remove_dir_all(tempdir).unwrap();
    }
    std::fs::create_dir_all(tempdir).unwrap();
}

fn install_tarball_package(
    npm: &CommandInfo,
    to_project: &Utf8Path,
    package_tarball_path: &Utf8Path,
) -> Result<()> {
    // Install the npm package to a project (this will automatically create one)
    npm.output_checked(|cmd| {
        cmd.current_dir(to_project)
            .arg("install")
            .arg(package_tarball_path)
    })?;
    Ok(())
}

fn yarn_install_tarball_package(
    yarn: &CommandInfo,
    to_project: &Utf8Path,
    package_tarball_path: &Utf8Path,
) -> Result<()> {
    // Install the npm package to a project (this will automatically create one)
    yarn.output_checked(|cmd| {
        cmd.current_dir(to_project)
            .arg("add")
            .arg(package_tarball_path)
    })?;
    Ok(())
}

fn unpack_tarball_package(
    tar: &CommandInfo,
    to_project: &Utf8Path,
    package_tarball_path: &Utf8Path,
) -> Result<()> {
    // Install the npm package to a project (this will automatically create one)
    tar.output_checked(|cmd| {
        cmd.current_dir(to_project)
            .arg("-xvf")
            .arg(package_tarball_path)
    })?;
    Ok(())
}

fn run_installed_package(
    npm: &CommandInfo,
    in_project: &Utf8Path,
    package_name: &str,
    bins: &[&str],
) -> Result<()> {
    // Explicitly run each binary
    for bin in bins {
        let _version_out = npm.output_checked(|cmd| {
            cmd.current_dir(in_project)
                .arg("exec")
                .arg(format!("--package=@axodotdev/{package_name}"))
                .arg("-c")
                .arg(format!("{bin} --version"))
        })?;
    }

    // If common npx special cases apply where "just run the package" is unambiguous
    // then also check that that mode also works fine.
    if bins.len() == 1 || bins.contains(&package_name) {
        let _version_out = npm.output_checked(|cmd| {
            cmd.current_dir(in_project)
                .arg("exec")
                .arg(format!("@axodotdev/{package_name}"))
                .arg("--")
                .arg("--version")
        })?;
    }

    // Check that the test harness is actually working by running a nonsense binary
    let test = npm.output_checked(|cmd| {
        cmd.current_dir(in_project)
            .arg("exec")
            .arg(format!("--package=@axodotdev/{package_name}"))
            .arg("-c")
            .arg("asdasdadfakebin --version")
    });
    assert!(test.is_err());

    Ok(())
}

fn lint_package(npm: &CommandInfo, package_dir: &Utf8Path, _package_name: &str) -> Result<()> {
    // Setup its deps
    npm.output_checked(|cmd| cmd.current_dir(package_dir).arg("ci"))?;
    // Lint check it
    // FIXME: DISABLED FOR HAUNTED
    // npm.output_checked(|cmd| cmd.current_dir(package_dir).arg("run").arg("fmt:check"))?;

    Ok(())
}
