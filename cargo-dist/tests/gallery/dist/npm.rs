use super::*;

impl DistResult {
    // Runs the installer script in the system's Homebrew installation
    #[allow(unused_variables)]
    pub fn runtest_npm_installer(&self, ctx: &TestContext<Tools>) -> Result<()> {
        if !std::env::var(ENV_RUIN_ME)
            .map(|s| !s.is_empty())
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

        // Only do this if npm is installed
        let Some(npm) = &ctx.tools.npm else {
            return Ok(());
        };
        let app_name = ctx.repo.app_name;
        let test_name = &self.test_name;

        // Create/clobber a temp dir in target
        let repo_dir = &ctx.repo_dir;
        let repo_id = &ctx.repo_id;
        let parent = repo_dir.parent().unwrap();
        let tempdir = parent.join(format!("{repo_id}__{test_name}"));
        if tempdir.exists() {
            std::fs::remove_dir_all(&tempdir).unwrap();
        }
        std::fs::create_dir_all(&tempdir).unwrap();

        // Have npm install/unpack the tarball to a project
        eprintln!("running npm install...");
        let parent_package_dir = tempdir.clone();
        install_tarball_package(npm, &parent_package_dir, &package_tarball_path)?;

        // Run the installed app
        eprintln!("npm exec'ing installed app...");
        run_installed_package(npm, &parent_package_dir, app_name)?;

        // Now let's hop into the installed package and have it lint itself
        lint_installed_package(npm, &parent_package_dir, app_name)?;
        Ok(())
    }
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

fn run_installed_package(
    npm: &CommandInfo,
    in_project: &Utf8Path,
    package_name: &str,
) -> Result<()> {
    let version_out = npm.output_checked(|cmd| {
        cmd.current_dir(&in_project)
            .arg("exec")
            .arg(package_name)
            .arg("--")
            .arg("--version")
    })?;

    let version_string = String::from_utf8(version_out.stdout).unwrap();
    let (v_name, v_version) = version_string
        .split_once(' ')
        .expect("could not parse version of npm");
    assert_eq!(v_name, package_name);
    assert!(v_version.contains('.'));
    Ok(())
}

fn lint_installed_package(
    npm: &CommandInfo,
    in_project: &Utf8Path,
    package_name: &str,
) -> Result<()> {
    let package_dir = in_project.join("node_modules").join(package_name);
    // Setup its deps
    npm.output_checked(|cmd| cmd.current_dir(&package_dir).arg("install"))?;
    // Lint check it
    npm.output_checked(|cmd| cmd.current_dir(&package_dir).arg("run").arg("fmt:check"))?;

    Ok(())
}
