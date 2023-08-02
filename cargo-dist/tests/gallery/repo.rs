use std::sync::Mutex;

use camino::Utf8PathBuf;
use miette::IntoDiagnostic;

use super::command::CommandInfo;
use super::errors::Result;

const TARGET_TEMP_DIR: &str = env!("CARGO_TARGET_TMPDIR");

pub struct TestContextLock<ToolsImpl> {
    repo: &'static Repo,
    ctx: Mutex<Option<TestContext<ToolsImpl>>>,
}

pub struct TestContext<ToolsImpl> {
    pub repo: &'static Repo,
    pub tools: ToolsImpl,
}

pub struct Repo {
    pub repo_owner: &'static str,
    pub repo_name: &'static str,
    pub commit_sha: &'static str,
    pub app_name: &'static str,
    pub bins: &'static [&'static str],
}

pub trait ToolsImpl: Default {
    fn git(&self) -> &CommandInfo;
}

impl<Tools> TestContextLock<Tools>
where
    Tools: ToolsImpl,
{
    pub const fn new(repo: &'static Repo) -> Self {
        Self {
            repo,
            ctx: Mutex::new(None),
        }
    }
    pub fn run_test(&self, f: impl FnOnce(&TestContext<Tools>) -> Result<()>) -> Result<()> {
        let maybe_guard = self.ctx.lock();
        // It's fine for the mutex to be poisoned once the value is Some because none of the tests
        // are allowed to mutate the TestContext. But if it's poisoned while None that means we
        // encountered an error while setting up TestContext and should just abort everything
        // instead of retrying over and over.
        let mut guard = match maybe_guard {
            Ok(guard) => guard,
            Err(poison) => {
                let guard = poison.into_inner();
                if guard.is_none() {
                    panic!("aborting all tests: failed fetch");
                }
                guard
            }
        };

        if guard.is_none() {
            // Intentionally unwrapping here to poison the mutex if we can't fetch
            let ctx = self.init_context().unwrap();
            *guard = Some(ctx);
        }

        let ctx = guard.as_ref().unwrap();

        f(ctx)
    }

    fn init_context(&self) -> Result<TestContext<Tools>> {
        let Repo {
            repo_owner,
            repo_name,
            commit_sha,
            ..
        } = self.repo;
        let repo_url = format!("https://github.com/{repo_owner}/{repo_name}");

        // Get the tools we'll invoke
        let tools = Tools::default();

        // Clone the repo we're interested in and cd into it
        Self::fetch_repo(tools.git(), repo_name, &repo_url, commit_sha)?;

        // Run tests
        let ctx = TestContext {
            repo: self.repo,
            tools,
        };
        Ok(ctx)
    }

    /// Fetch/update a repo to the given commit_sha
    fn fetch_repo(
        git: &CommandInfo,
        repo_name: &str,
        repo_url: &str,
        commit_sha: &str,
    ) -> Result<()> {
        std::env::set_current_dir(TARGET_TEMP_DIR).into_diagnostic()?;
        if Utf8PathBuf::from(repo_name).exists() {
            eprintln!("repo already cloned, updating it...");
            std::env::set_current_dir(repo_name).into_diagnostic()?;
            git.output_checked(|c| c.arg("remote").arg("set-url").arg("origin").arg(repo_url))?;
            git.output_checked(|c| c.arg("fetch").arg("origin").arg(commit_sha))?;
            git.output_checked(|c| c.arg("reset").arg("--hard").arg("FETCH_HEAD"))?;
        } else {
            eprintln!("fetching {repo_url}");
            axoasset::LocalAsset::create_dir(repo_name)?;
            std::env::set_current_dir(repo_name).into_diagnostic()?;
            git.output_checked(|c| c.arg("init"))?;
            git.output_checked(|c| c.arg("remote").arg("add").arg("origin").arg(repo_url))?;
            git.output_checked(|c| c.arg("fetch").arg("origin").arg(commit_sha))?;
            git.output_checked(|c| c.arg("reset").arg("--hard").arg("FETCH_HEAD"))?;
        }

        Ok(())
    }
}
