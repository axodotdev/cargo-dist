use super::*;
use crate::gallery::command::CommandInfo;

// Allowing dead code because some of these are platform-specific
#[allow(dead_code)]
pub struct Tools {
    pub git: CommandInfo,
    pub cargo_dist: CommandInfo,
    pub shellcheck: Option<CommandInfo>,
    pub psanalyzer: Option<CommandInfo>,
    pub homebrew: Option<CommandInfo>,
    pub npm: Option<CommandInfo>,
    pub pnpm: Option<CommandInfo>,
    pub yarn: Option<CommandInfo>,
    pub tar: Option<CommandInfo>,
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
        let tar = CommandInfo::new("tar", None);
        let npm = CommandInfo::new_js("npm", None);
        let pnpm = CommandInfo::new_js("pnpm", None);
        let yarn = CommandInfo::new_js("yarn", None);
        assert!(tar.is_some());
        Self {
            git,
            cargo_dist,
            shellcheck,
            psanalyzer,
            homebrew,
            npm,
            pnpm,
            yarn,
            tar,
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
