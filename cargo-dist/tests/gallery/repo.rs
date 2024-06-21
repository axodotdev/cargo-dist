use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use camino::{Utf8Path, Utf8PathBuf};
use miette::IntoDiagnostic;

use super::command::CommandInfo;
use super::errors::Result;

/// A subdir of `target/` that cargo helpfully defines for us to scribble in during tests.
/// We are 100% responsible for its contents.
const TARGET_TEMP_DIR: &str = env!("CARGO_TARGET_TMPDIR");

/// Top-level type that should be used to declare `statics` that define test repos
pub struct TestContextLock<Tools: 'static> {
    repo: &'static Repo,
    tools: &'static Mutex<Option<Tools>>,
    ctx: Mutex<Option<RawTestContext>>,
}
/// Inner state of a TestContext
pub struct RawTestContext {
    pub repo: &'static Repo,
    pub repo_id: String,
    pub repo_dir: Utf8PathBuf,
}
/// Context passed down to test runs
pub struct TestContext<'a, Tools> {
    raw_ctx: &'a RawTestContext,
    pub tools: &'a Tools,
    pub options: TestOptions,
}

#[derive(Debug, Default)]
pub struct TestOptions {
    pub apps: HashMap<String, AppOverrides>,
}

impl TestOptions {
    pub fn set_options(&mut self, app_name: &str) -> &mut AppOverrides {
        self.apps.entry(app_name.to_owned()).or_default()
    }

    pub fn options(&self, app_name: &str) -> &AppOverrides {
        self.apps.get(app_name).unwrap_or(&EMPTY_OVERRIDES)
    }

    pub fn npm_scope(&self, app_name: &str) -> &str {
        self.options(app_name)
            .npm_scope
            .as_deref()
            .unwrap_or("axodotdev")
    }
    pub fn npm_package_name<'a>(&'a self, app_name: &'a str) -> &'a str {
        self.options(app_name)
            .npm_package_name
            .as_deref()
            .unwrap_or(app_name)
    }
    #[allow(dead_code)]
    pub fn homebrew_tap(&self, app_name: &str) -> &str {
        self.options(app_name)
            .homebrew_tap
            .as_deref()
            .unwrap_or("axodotdev/homebrew-tap")
    }
    pub fn homebrew_package_name<'a>(&'a self, app_name: &'a str) -> &'a str {
        self.options(app_name)
            .homebrew_package_name
            .as_deref()
            .unwrap_or(app_name)
    }
    pub fn bins_with_aliases(&self, app_name: &str, bins: &[String]) -> Vec<String> {
        self.options(app_name)
            .bin_aliases
            .clone()
            .unwrap_or_default()
            .into_iter()
            .chain(bins.to_owned())
            .collect()
    }
}

#[derive(Debug, Default)]
pub struct AppOverrides {
    pub npm_scope: Option<String>,
    pub npm_package_name: Option<String>,
    #[allow(dead_code)]
    pub homebrew_tap: Option<String>,
    pub homebrew_package_name: Option<String>,
    pub bin_aliases: Option<Vec<String>>,
}

static EMPTY_OVERRIDES: AppOverrides = AppOverrides {
    npm_package_name: None,
    npm_scope: None,
    homebrew_tap: None,
    homebrew_package_name: None,
    bin_aliases: None,
};

impl<'a, Tools> std::ops::Deref for TestContext<'a, Tools> {
    type Target = RawTestContext;
    fn deref(&self) -> &Self::Target {
        self.raw_ctx
    }
}
/// Info about a repo (assumed to be a github repo)
pub struct Repo {
    pub repo_owner: &'static str,
    pub repo_name: &'static str,
    pub commit_sha: &'static str,
    /// Apps included
    pub apps: &'static [App],
}

pub struct App {
    pub name: &'static str,
    pub bins: &'static [&'static str],
}

pub trait ToolsImpl: Default {
    /// Get an implementation of `git`
    fn git(&self) -> &CommandInfo;
}

impl<Tools> TestContextLock<Tools>
where
    Tools: ToolsImpl,
{
    /// Create a new test with the given tools/repo
    ///
    /// Note that you should only have one Tools instance in your test suite, as it serves as a global
    /// lock for global mutable state like `set_current_dir`.
    pub const fn new(tools: &'static Mutex<Option<Tools>>, repo: &'static Repo) -> Self {
        Self {
            repo,
            tools,
            ctx: Mutex::new(None),
        }
    }

    /// Run a test on this repo
    pub fn run_test(&self, test: impl FnOnce(TestContext<Tools>) -> Result<()>) -> Result<()> {
        std::env::set_var("CARGO_DIST_MOCK_NETWORKING", "1");
        let maybe_tools = self.tools.lock();
        let maybe_repo = self.ctx.lock();
        // Intentionally unwrapping here to poison the mutexes if we can't fetch
        let tools_guard = Self::init_mutex(maybe_tools, || Tools::default());
        let tools = tools_guard.as_ref().unwrap();
        let raw_ctx_guard = Self::init_mutex(maybe_repo, || self.init_context(tools).unwrap());
        let raw_ctx = raw_ctx_guard.as_ref().unwrap();

        let ctx = TestContext {
            raw_ctx,
            tools,
            options: TestOptions::default(),
        };
        // Ensure we're in the right dir before running the test
        std::env::set_current_dir(&ctx.repo_dir).into_diagnostic()?;

        test(ctx)
    }

    /// Create the RawTestContext for this Repo by git fetching it to a sufficient temp dir
    fn init_context(&self, tools: &Tools) -> Result<RawTestContext> {
        let Repo {
            repo_owner,
            repo_name,
            commit_sha,
            ..
        } = self.repo;
        let repo_url: String = format!("https://github.com/{repo_owner}/{repo_name}");
        let repo_id: String = format!("{repo_owner}_{repo_name}_{commit_sha}");
        let repo_dir = Utf8Path::new(TARGET_TEMP_DIR).join(&repo_id);

        // Clone the repo we're interested in and cd into it
        Self::fetch_repo(tools.git(), &repo_dir, &repo_url, commit_sha)?;

        // Run tests
        let ctx = RawTestContext {
            repo: self.repo,
            repo_id,
            repo_dir,
        };
        Ok(ctx)
    }

    /// Take a potentially-poisoned, potentially-unintializeed `MutexGuard<Option<T>>` and
    /// handle the poison and initialization of it.
    ///
    /// It's fine for the mutex to be poisoned once the value is Some because none of the tests
    /// are allowed to mutate the TestContext. But if it's poisoned while None that means we
    /// encountered an error while setting up the TestContext and should just abort everything
    /// instead of retrying over and over. (e.g. git fetch failed, finding tools failed, etc.)
    fn init_mutex<T>(
        maybe_guard: std::sync::LockResult<MutexGuard<'_, Option<T>>>,
        init: impl FnOnce() -> T,
    ) -> MutexGuard<'_, Option<T>> {
        let mut guard = match maybe_guard {
            Ok(guard) => guard,
            Err(poison) => {
                let guard = poison.into_inner();
                if guard.is_none() {
                    panic!("aborting all tests: failed test harness initialization");
                }
                guard
            }
        };

        if guard.is_none() {
            let ctx = init();
            *guard = Some(ctx);
        }
        guard
    }

    /// Fetch/update a repo to the given commit_sha
    fn fetch_repo(
        git: &CommandInfo,
        repo_dir: &Utf8Path,
        repo_url: &str,
        commit_sha: &str,
    ) -> Result<()> {
        if repo_dir.exists() {
            eprintln!("repo already cloned, updating it...");
            std::env::set_current_dir(repo_dir).into_diagnostic()?;
            git.output_checked(|c| c.arg("remote").arg("set-url").arg("origin").arg(repo_url))?;
            git.output_checked(|c| c.arg("fetch").arg("origin").arg(commit_sha).arg("--tags"))?;
            git.output_checked(|c| c.arg("reset").arg("--hard").arg("FETCH_HEAD"))?;
        } else {
            eprintln!("fetching {repo_url}");
            axoasset::LocalAsset::create_dir(repo_dir)?;
            std::env::set_current_dir(repo_dir).into_diagnostic()?;
            git.output_checked(|c| c.arg("init"))?;
            git.output_checked(|c| c.arg("remote").arg("add").arg("origin").arg(repo_url))?;
            git.output_checked(|c| c.arg("fetch").arg("origin").arg(commit_sha).arg("--tags"))?;
            git.output_checked(|c| c.arg("reset").arg("--hard").arg("FETCH_HEAD"))?;
        }

        Ok(())
    }
}
