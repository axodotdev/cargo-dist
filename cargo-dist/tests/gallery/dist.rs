use std::collections::BTreeMap;
use std::sync::Mutex;

use axoasset::{toml_edit, LocalAsset, SourceFile};
use camino::{Utf8Path, Utf8PathBuf};
use miette::miette;

use super::command::CommandInfo;
use super::errors::Result;
use super::repo::{Repo, TestContext, TestContextLock, ToolsImpl};

/// Set this env-var to enable running the installer scripts in temp dirs
///
/// If everything's working right, then no problem.
/// Otherwise MEGA DANGER in messing up your computer.
#[cfg(any(target_family = "unix", target_family = "windows"))]
const ENV_RUIN_ME: &str = "RUIN_MY_COMPUTER_WITH_INSTALLERS";
/// Set this at runtime to override STATIC_CARGO_DIST_BIN
const ENV_RUNTIME_CARGO_DIST_BIN: &str = "OVERRIDE_CARGO_BIN_EXE_cargo-dist";
const STATIC_CARGO_DIST_BIN: &str = env!("CARGO_BIN_EXE_cargo-dist");
const ROOT_DIR: &str = env!("CARGO_MANIFEST_DIR");
static TOOLS: Mutex<Option<Tools>> = Mutex::new(None);

/// axolotlsay 0.1.0 is a nice simple project with shell+powershell+npm+homebrew+msi installers in its release
pub static AXOLOTLSAY: TestContextLock<Tools> = TestContextLock::new(
    &TOOLS,
    &Repo {
        repo_owner: "axodotdev",
        repo_name: "axolotlsay",
        commit_sha: "403a65095fccf77380896d0f3c85000e0a1bec69",
        app_name: "axolotlsay",
        bins: &["axolotlsay"],
    },
);
/// akaikatana-repack 0.2.0 has multiple bins!
pub static AKAIKATANA_REPACK: TestContextLock<Tools> = TestContextLock::new(
    &TOOLS,
    &Repo {
        repo_owner: "mistydemeo",
        repo_name: "akaikatana-repack",
        commit_sha: "9516f77ab81b7833e0d66de766ecf802e056f91f",
        app_name: "akaikatana-repack",
        bins: &["akextract", "akmetadata", "akrepack"],
    },
);
/// axoasset only has libraries!
pub static AXOASSET: TestContextLock<Tools> = TestContextLock::new(
    &TOOLS,
    &Repo {
        repo_owner: "axodotdev",
        repo_name: "axoasset",
        commit_sha: "5d6a531428fb645bbb1259fd401575c6c651be94",
        app_name: "axoasset",
        bins: &[],
    },
);
/// self-testing cargo-dist
pub static DIST: TestContextLock<Tools> = TestContextLock::new(
    &TOOLS,
    &Repo {
        repo_owner: "axodotdev",
        repo_name: "cargo-dist",
        commit_sha: "main",
        app_name: "cargo-dist",
        bins: &["cargo-dist"],
    },
);

pub struct Tools {
    pub git: CommandInfo,
    pub cargo_dist: CommandInfo,
    pub shellcheck: Option<CommandInfo>,
    pub psanalyzer: Option<CommandInfo>,
    pub homebrew: Option<CommandInfo>,
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
        let homebrew = CommandInfo::new("brew", None);

        Self {
            git,
            cargo_dist,
            shellcheck,
            psanalyzer,
            homebrew,
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
    // Only used in some cfgs
    trust_hashes: bool,
    shell_installer_path: Option<Utf8PathBuf>,
    homebrew_installer_path: Option<Utf8PathBuf>,
    powershell_installer_path: Option<Utf8PathBuf>,
    npm_installer_package_path: Option<Utf8PathBuf>,
}

pub struct PlanResult {
    test_name: String,
    raw_json: String,
}

pub struct GenerateResult {
    test_name: String,
    github_ci_path: Option<Utf8PathBuf>,
    wxs_path: Option<Utf8PathBuf>,
}

pub struct BuildAndPlanResult {
    build: DistResult,
    plan: PlanResult,
}

pub struct Snapshots {
    settings: insta::Settings,
    name: String,
    payload: String,
}

impl<'a> TestContext<'a, Tools> {
    /// Run 'cargo dist build -alies --no-local-paths --output-format=json' and return paths to various files that were generated
    pub fn cargo_dist_build_lies(&self, test_name: &str) -> Result<BuildAndPlanResult> {
        // If the cargo-dist target dir exists, delete it to avoid cross-contamination
        let out_path = Utf8Path::new("target/distrib/");
        if out_path.exists() {
            LocalAsset::remove_dir_all(out_path)?;
        }

        // build installers
        eprintln!("running cargo dist build -aglobal...");
        let output = self.tools.cargo_dist.output_checked(|cmd| {
            cmd.arg("dist")
                .arg("build")
                .arg("-alies")
                .arg("--no-local-paths")
                .arg("--output-format=json")
        })?;

        let build = self.load_dist_results(test_name, false)?;

        let raw_json = String::from_utf8(output.stdout).expect("plan wasn't utf8!?");
        let plan = PlanResult {
            test_name: test_name.to_owned(),
            raw_json,
        };

        Ok(BuildAndPlanResult { build, plan })
    }

    /// Run `cargo_dist_plan` and `cargo_dist_build_global`
    pub fn cargo_dist_build_and_plan(&self, test_name: &str) -> Result<BuildAndPlanResult> {
        let build = self.cargo_dist_build_global(test_name)?;
        let plan = self.cargo_dist_plan(test_name)?;

        Ok(BuildAndPlanResult { build, plan })
    }

    /// Run 'cargo dist plan --output-format=json' and return dist-manifest.json
    pub fn cargo_dist_plan(&self, test_name: &str) -> Result<PlanResult> {
        let output = self
            .tools
            .cargo_dist
            .output_checked(|cmd| cmd.arg("dist").arg("plan").arg("--output-format=json"))?;
        let raw_json = String::from_utf8(output.stdout).expect("plan wasn't utf8!?");

        Ok(PlanResult {
            test_name: test_name.to_owned(),
            raw_json,
        })
    }
    /// Run 'cargo dist build -aglobal' and return paths to various files that were generated
    pub fn cargo_dist_build_global(&self, test_name: &str) -> Result<DistResult> {
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

        self.load_dist_results(test_name, true)
    }

    /// Run 'cargo dist generate' and return paths to various files that were generated
    pub fn cargo_dist_generate(&self, test_name: &str) -> Result<GenerateResult> {
        self.cargo_dist_generate_prefixed(test_name, "")
    }
    /// Run 'cargo dist generate' and return paths to various files that were generated
    /// (also apply a prefix to the github filename)
    pub fn cargo_dist_generate_prefixed(
        &self,
        test_name: &str,
        prefix: &str,
    ) -> Result<GenerateResult> {
        let ci_file_name = format!("{prefix}release.yml");
        let github_ci_path = Utf8Path::new(".github/workflows/").join(ci_file_name);
        let wxs_path = Utf8Path::new("wix/main.wxs").to_owned();
        // Delete files if they already exist
        if github_ci_path.exists() {
            LocalAsset::remove_file(&github_ci_path)?;
        }
        if wxs_path.exists() {
            LocalAsset::remove_file(&wxs_path)?;
        }

        // run generate
        eprintln!("running cargo dist generate...");
        self.tools
            .cargo_dist
            .output_checked(|cmd| cmd.arg("dist").arg("generate"))?;

        Ok(GenerateResult {
            test_name: test_name.to_owned(),
            github_ci_path: github_ci_path.exists().then_some(github_ci_path),
            wxs_path: wxs_path.exists().then_some(wxs_path),
        })
    }

    fn load_dist_results(&self, test_name: &str, trust_hashes: bool) -> Result<DistResult> {
        // read/analyze installers
        eprintln!("loading results...");
        let app_name = &self.repo.app_name;
        let target_dir = Utf8PathBuf::from("target/distrib");
        let ps_installer = Utf8PathBuf::from(format!("{target_dir}/{app_name}-installer.ps1"));
        let sh_installer = Utf8PathBuf::from(format!("{target_dir}/{app_name}-installer.sh"));
        let homebrew_installer = Self::load_file_with_suffix(target_dir.clone(), ".rb");
        let npm_installer =
            Utf8PathBuf::from(format!("{target_dir}/{app_name}-npm-package.tar.gz"));

        Ok(DistResult {
            test_name: test_name.to_owned(),
            trust_hashes,
            shell_installer_path: sh_installer.exists().then_some(sh_installer),
            powershell_installer_path: ps_installer.exists().then_some(ps_installer),
            homebrew_installer_path: homebrew_installer,
            npm_installer_package_path: npm_installer.exists().then_some(npm_installer),
        })
    }

    fn load_file_with_suffix(dirname: Utf8PathBuf, suffix: &str) -> Option<Utf8PathBuf> {
        let files = Self::load_files_with_suffix(dirname, suffix);
        let number_found = files.len();
        assert!(
            number_found <= 1,
            "found {} files with the suffix {}, expected 1 or 0",
            number_found,
            suffix
        );
        files.first().cloned()
    }

    fn load_files_with_suffix(dirname: Utf8PathBuf, suffix: &str) -> Vec<Utf8PathBuf> {
        // Collect all dist-manifests and fetch the appropriate Mac ones
        let mut files = vec![];
        for file in dirname
            .read_dir()
            .expect("loading target dir failed, something has gone very wrong")
        {
            let path = file.unwrap().path();
            if let Some(filename) = path.file_name() {
                if filename.to_string_lossy().ends_with(suffix) {
                    files.push(Utf8PathBuf::from_path_buf(path).unwrap())
                }
            }
        }
        files
    }

    pub fn patch_cargo_toml(&self, new_toml: String) -> Result<()> {
        eprintln!("loading Cargo.toml...");
        let toml_src = axoasset::SourceFile::load_local("Cargo.toml")?;
        let mut toml = toml_src.deserialize_toml_edit()?;
        eprintln!("editing Cargo.toml...");
        let new_table_src = axoasset::SourceFile::new("new-Cargo.toml", new_toml);
        let new_table = new_table_src.deserialize_toml_edit()?;

        // Written slightly verbosely to make it easier to isolate which failed
        let namespaces = ["workspace", "package"];
        for namespace in namespaces {
            let Some(new_meta) = new_table.get(namespace).and_then(|t| t.get("metadata")) else {
                continue;
            };
            let old_namespace = toml[namespace].or_insert(toml_edit::table());
            let old_meta = old_namespace["metadata"].or_insert(toml_edit::table());
            eprintln!("{new_table}");
            for (key, new) in new_meta.as_table().unwrap() {
                let old = &mut old_meta[key];
                *old = new.clone();
            }
        }

        let toml_out = toml.to_string();
        eprintln!("writing Cargo.toml...");
        axoasset::LocalAsset::write_new(&toml_out, "Cargo.toml")?;

        Ok(())
    }
}

impl DistResult {
    /// check_all but for when you don't expect the installers to run properly (due to hosting)
    pub fn check_all_no_ruin(
        &self,
        ctx: &TestContext<Tools>,
        _expected_bin_dir: &str,
    ) -> Result<Snapshots> {
        self.linttests(ctx)?;
        // Now that all other checks have passed, it's safe to check snapshots
        self.snapshot()
    }

    pub fn check_all(&self, ctx: &TestContext<Tools>, expected_bin_dir: &str) -> Result<Snapshots> {
        self.linttests(ctx)?;
        self.runtests(ctx, expected_bin_dir)?;
        // Now that all other checks have passed, it's safe to check snapshots
        self.snapshot()
    }

    pub fn linttests(&self, ctx: &TestContext<Tools>) -> Result<()> {
        // If we have shellcheck, check our shell script
        self.shellcheck(ctx)?;

        // If we have PsScriptAnalyzer, check our powershell script
        self.psanalyzer(ctx)?;

        Ok(())
    }

    pub fn runtests(&self, ctx: &TestContext<Tools>, expected_bin_dir: &str) -> Result<()> {
        // If we can, run the shell script in a temp HOME
        self.runtest_shell_installer(ctx, expected_bin_dir)?;

        // If we can, run the powershell script in a temp HOME
        self.runtest_powershell_installer(ctx, expected_bin_dir)?;

        // If we can, run the homebrew script in a temp HOME
        self.runtest_homebrew_installer(ctx)?;

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

    #[cfg(any(target_family = "unix", target_family = "windows"))]
    fn check_install_receipt(
        &self,
        ctx: &TestContext<Tools>,
        bin_dir: &Utf8Path,
        receipt_file: &Utf8Path,
        bin_ext: &str,
    ) {
        // Check that the install receipt works
        use serde::Deserialize;

        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct InstallReceipt {
            binaries: Vec<String>,
            install_prefix: String,
            provider: InstallReceiptProvider,
            source: InstallReceiptSource,
            version: String,
        }
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct InstallReceiptProvider {
            source: String,
            version: String,
        }
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct InstallReceiptSource {
            app_name: String,
            name: String,
            owner: String,
            release_type: String,
        }

        assert!(receipt_file.exists());
        let receipt_src = SourceFile::load_local(receipt_file).expect("couldn't load receipt file");
        let receipt: InstallReceipt = receipt_src.deserialize_json().unwrap();
        assert_eq!(receipt.source.app_name, ctx.repo.app_name);
        assert_eq!(
            receipt.binaries,
            ctx.repo
                .bins
                .iter()
                .map(|s| format!("{s}{bin_ext}"))
                .collect::<Vec<_>>()
        );
        let receipt_bin_dir = receipt
            .install_prefix
            .trim_end_matches('/')
            .trim_end_matches('\\')
            .to_owned();
        let expected_bin_dir = bin_dir
            .to_string()
            .trim_end_matches('/')
            .trim_end_matches('\\')
            .to_owned();
        assert_eq!(receipt_bin_dir, expected_bin_dir);
    }

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
            .map(|s| !s.is_empty())
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
                        .env("UserProfile", &tempdir)
                        .env_remove("PsModulePath")
                })?;
                eprintln!("{}", String::from_utf8(output.stderr).unwrap());
                Ok(String::from_utf8(output.stdout).unwrap().trim().to_owned())
            }

            let app_name = ctx.repo.app_name;
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
                    run_ps1_script(&self.powershell, &self.tempdir, "restorepath.ps1", &format!(r#"
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
            let bin_dir = tempdir.join(&expected_bin_dir);

            assert!(bin_dir.exists(), "bin dir wasn't created");

            // Check that all the binaries work
            for bin_name in ctx.repo.bins {
                let bin_path = bin_dir.join(format!("{bin_name}.exe"));
                assert!(bin_path.exists(), "{bin_name} wasn't created");

                let bin =
                    CommandInfo::new(bin_name, Some(bin_path.as_str())).expect("failed to run bin");
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

    // Runs the installer script in the system's Homebrew installation
    #[allow(unused_variables)]
    pub fn runtest_homebrew_installer(&self, ctx: &TestContext<Tools>) -> Result<()> {
        // Only do this if we trust hashes (outside cfg so the compiler knows we use this)
        if !self.trust_hashes {
            return Ok(());
        }

        // Only do this on macOS, and only do it if RUIN_MY_COMPUTER_WITH_INSTALLERS is set
        #[cfg(target_os = "macos")]
        if std::env::var(ENV_RUIN_ME)
            .map(|s| !s.is_empty())
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

            for bin_name in ctx.repo.bins {
                let bin_path = bin.join(bin_name);
                assert!(bin_path.exists(), "bin wasn't created");
            }

            homebrew.output_checked(|cmd| cmd.arg("uninstall").arg(relative_formula_path))?;
        }
        Ok(())
    }

    // Run cargo-insta on everything we care to snapshot
    pub fn snapshot(&self) -> Result<Snapshots> {
        // We make a single uber-snapshot for both scripts to avoid the annoyances of having multiple snapshots
        // in one test (necessitating rerunning it multiple times or passing special flags to get all the changes)
        let mut snapshots = String::new();

        append_snapshot_file(
            &mut snapshots,
            "installer.sh",
            self.shell_installer_path.as_deref(),
        )?;
        append_snapshot_file(
            &mut snapshots,
            "formula.rb",
            self.homebrew_installer_path.as_deref(),
        )?;
        append_snapshot_file(
            &mut snapshots,
            "installer.ps1",
            self.powershell_installer_path.as_deref(),
        )?;
        append_snapshot_tarball(
            &mut snapshots,
            "npm-package.tar.gz",
            self.npm_installer_package_path.as_deref(),
        )?;

        Ok(Snapshots {
            settings: snapshot_settings_with_gallery_filter(),
            name: self.test_name.to_owned(),
            payload: snapshots,
        })
    }
}

impl PlanResult {
    pub fn check_all(&self) -> Result<Snapshots> {
        self.parse()?;
        self.snapshot()
    }

    pub fn parse(&self) -> Result<cargo_dist_schema::DistManifest> {
        let src = SourceFile::new("dist-manifest.json", self.raw_json.clone());
        let val = src.deserialize_json()?;
        Ok(val)
    }

    // Run cargo-insta on everything we care to snapshot
    pub fn snapshot(&self) -> Result<Snapshots> {
        // We make a single uber-snapshot for both scripts to avoid the annoyances of having multiple snapshots
        // in one test (necessitating rerunning it multiple times or passing special flags to get all the changes)
        let mut snapshots = String::new();

        append_snapshot_string(&mut snapshots, "dist-manifest.json", &self.raw_json)?;

        Ok(Snapshots {
            settings: snapshot_settings_with_gallery_filter(),
            name: self.test_name.to_owned(),
            payload: snapshots,
        })
    }
}

impl BuildAndPlanResult {
    pub fn check_all_no_ruin(
        &self,
        ctx: &TestContext<Tools>,
        expected_bin_dir: &str,
    ) -> Result<Snapshots> {
        let build_snaps = self.build.check_all_no_ruin(ctx, expected_bin_dir)?;
        let plan_snaps = self.plan.check_all()?;

        // Merge snapshots
        let snaps = build_snaps.join(plan_snaps);
        Ok(snaps)
    }
    pub fn check_all(&self, ctx: &TestContext<Tools>, expected_bin_dir: &str) -> Result<Snapshots> {
        let build_snaps = self.build.check_all(ctx, expected_bin_dir)?;
        let plan_snaps = self.plan.check_all()?;

        // Merge snapshots
        let snaps = build_snaps.join(plan_snaps);
        Ok(snaps)
    }
}

impl GenerateResult {
    pub fn check_all(&self) -> Result<Snapshots> {
        self.snapshot()
    }

    // Run cargo-insta on everything we care to snapshot
    pub fn snapshot(&self) -> Result<Snapshots> {
        // We make a single uber-snapshot for both scripts to avoid the annoyances of having multiple snapshots
        // in one test (necessitating rerunning it multiple times or passing special flags to get all the changes)
        let mut snapshots = String::new();

        eprintln!("{:?}", self.github_ci_path);
        append_snapshot_file(
            &mut snapshots,
            self.github_ci_path
                .as_deref()
                .and_then(|p| p.file_name())
                .unwrap_or_default(),
            self.github_ci_path.as_deref(),
        )?;

        append_snapshot_file(&mut snapshots, "main.wxs", self.wxs_path.as_deref())?;

        Ok(Snapshots {
            settings: snapshot_settings_with_gallery_filter(),
            name: self.test_name.to_owned(),
            payload: snapshots,
        })
    }
}

impl Snapshots {
    pub fn snap(self) {
        self.settings.bind(|| {
            insta::assert_snapshot!(self.name, self.payload);
        })
    }

    pub fn join(mut self, other: Self) -> Self {
        self.payload.push_str(&other.payload);
        self
    }
}

pub fn snapshot_settings() -> insta::Settings {
    let mut settings = insta::Settings::clone_current();
    let snapshot_dir = Utf8Path::new(ROOT_DIR).join("tests").join("snapshots");
    settings.set_snapshot_path(snapshot_dir);
    settings.set_prepend_module_to_snapshot(false);
    settings
}

pub fn snapshot_settings_with_version_filter() -> insta::Settings {
    let mut settings = snapshot_settings();
    settings.add_filter(
        r"\d+\.\d+\.\d+(\-prerelease\d*)?(\.\d+)?",
        "1.0.0-FAKEVERSION",
    );
    settings
}

/// Only filter parts that are specific to the toolchains being used to build the result
///
/// This is used for checking gallery entries
pub fn snapshot_settings_with_gallery_filter() -> insta::Settings {
    let mut settings = snapshot_settings();
    settings.add_filter(r#""dist_version": .*"#, r#""dist_version": "CENSORED","#);
    settings.add_filter(
        r#""cargo_version_line": .*"#,
        r#""cargo_version_line": "CENSORED""#,
    );
    settings.add_filter(
        r"cargo-dist/releases/download/v\d+\.\d+\.\d+(\-prerelease\d*)?(\.\d+)?/",
        "cargo-dist/releases/download/vSOME_VERSION/",
    );
    settings.add_filter(r#"sha256 ".*""#, r#"sha256 "CENSORED""#);
    settings.add_filter(r#""sha256": .*"#, r#""sha256": "CENSORED""#);
    settings.add_filter(r#""sha512": .*"#, r#""sha512": "CENSORED""#);
    settings.add_filter(r#""version":"[a-zA-Z\.0-9\-]*""#, r#""version":"CENSORED""#);
    settings
}

/// Filter anything that will regularly change in the process of a release
///
/// This is used for checking `main` against itself.
#[allow(dead_code)]
pub fn snapshot_settings_with_dist_manifest_filter() -> insta::Settings {
    let mut settings = snapshot_settings_with_version_filter();
    settings.add_filter(
        r#""announcement_tag": .*"#,
        r#""announcement_tag": "CENSORED","#,
    );
    settings.add_filter(
        r#""announcement_title": .*"#,
        r#""announcement_title": "CENSORED""#,
    );
    settings.add_filter(
        r#""announcement_changelog": .*"#,
        r#""announcement_changelog": "CENSORED""#,
    );
    settings.add_filter(
        r#""announcement_github_body": .*"#,
        r#""announcement_github_body": "CENSORED""#,
    );
    settings.add_filter(
        r#""announcement_is_prerelease": .*"#,
        r#""announcement_is_prerelease": "CENSORED""#,
    );
    settings.add_filter(
        r#""cargo_version_line": .*"#,
        r#""cargo_version_line": "CENSORED""#,
    );
    settings.add_filter(r#""sha256": .*"#, r#""sha256": "CENSORED""#);
    settings.add_filter(r#""sha512": .*"#, r#""sha512": "CENSORED""#);

    settings
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

    // We shove everything in a BTreeMap to keep ordering stable
    let mut results = BTreeMap::new();

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
            results.insert(path, val);
        }
    }

    for (path, val) in &results {
        append_snapshot_string(out, &format!("{name}/{path}"), val)?;
    }
    Ok(())
}

fn append_snapshot_file(out: &mut String, name: &str, src_path: Option<&Utf8Path>) -> Result<()> {
    // Skip snapshotting this file if absent
    let Some(src_path) = src_path else {
        return Ok(());
    };

    let src = axoasset::LocalAsset::load_string(src_path)?;
    append_snapshot_string(out, name, &src)
}

fn append_snapshot_string(out: &mut String, name: &str, val: &str) -> Result<()> {
    use std::fmt::Write;

    writeln!(out, "================ {name} ================").unwrap();
    writeln!(out, "{val}").unwrap();
    Ok(())
}
