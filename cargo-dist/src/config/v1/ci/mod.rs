//! TODO

//! TODO

pub mod github;

use super::*;

use github::*;

/// TODO
#[derive(Debug, Default, Clone)]
pub struct CiConfig {
    /// TODO
    pub github: Option<GithubCiConfig>,
}

/// TODO
#[derive(Debug, Clone)]
pub struct CiConfigInheritable {
    /// TODO
    pub common: CommonCiConfig,
    /// TODO
    pub github: Option<GithubCiLayer>,
}

/// TODO
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CiLayer {
    /// TODO
    #[serde(flatten)]
    pub common: CommonCiLayer,
    /// TODO
    pub github: Option<BoolOr<GithubCiLayer>>,
}
impl CiConfigInheritable {
    /// TODO
    pub fn defaults_for_workspace(workspaces: &WorkspaceGraph) -> Self {
        Self {
            common: CommonCiConfig::defaults_for_workspace(workspaces),
            github: None,
        }
    }
    /// TODO
    pub fn apply_inheritance_for_workspace(self, workspaces: &WorkspaceGraph) -> CiConfig {
        let Self { common, github } = self;
        let github = github.map(|github| {
            let mut default = GithubCiConfig::defaults_for_workspace(workspaces, &common);
            default.apply_layer(github);
            default
        });
        CiConfig { github }
    }
}
impl ApplyLayer for CiConfigInheritable {
    type Layer = CiLayer;
    fn apply_layer(&mut self, Self::Layer { common, github }: Self::Layer) {
        self.common.apply_layer(common);
        self.github.apply_bool_layer(github);
    }
}

/// TODO
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CommonCiLayer {
    /// Whether we should try to merge otherwise-parallelizable tasks onto the same machine,
    /// sacrificing latency and fault-isolation for more the sake of minor effeciency gains.
    ///
    /// (defaults to false)
    ///
    /// For example, if you build for x64 macos and arm64 macos, by default we will generate ci
    /// which builds those independently on separate logical machines. With this enabled we will
    /// build both of those platforms together on the same machine, making it take twice as long
    /// as any other build and making it impossible for only one of them to succeed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_tasks: Option<bool>,

    /// Whether failing tasks should make us give up on all other tasks
    ///
    /// (defaults to false)
    ///
    /// When building a release you might discover that an obscure platform's build is broken.
    /// When this happens you have two options: give up on the release entirely (`fail-fast = true`),
    /// or keep trying to build all the other platforms anyway (`fail-fast = false`).
    ///
    /// cargo-dist was designed around the "keep trying" approach, as we create a draft Release
    /// and upload results to it over time, undrafting the release only if all tasks succeeded.
    /// The idea is that even if a platform fails to build, you can decide that's acceptable
    /// and manually undraft the release with some missing platforms.
    ///
    /// (Note that the dist-manifest.json is produced before anything else, and so it will assume
    /// that all tasks succeeded when listing out supported platforms/artifacts. This may make
    /// you sad if you do this kind of undrafting and also trust the dist-manifest to be correct.)
    ///
    /// Prior to 0.1.0 we didn't set the correct flags in our CI scripts to do this, but now we do.
    /// This flag was introduced to allow you to restore the old behaviour if you prefer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_fast: Option<bool>,

    /// Whether CI tasks should have build caches enabled.
    ///
    /// Defaults false because currently Cargo.toml / Cargo.lock changes
    /// invalidate the cache, making it useless for typical usage
    /// (since bumping your version changes both those files).
    ///
    /// As of this writing the two major exceptions to when it would be
    /// useful are `pr-run-mode = "upload"` and `release-branch = "my-branch"`
    /// which can run the CI action frequently and without Cargo.toml changes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_builds: Option<bool>,

    /// Whether CI should include logic to build local artifacts (default true)
    ///
    /// If false, it will be assumed that the local_artifacts_jobs will include custom
    /// jobs to build them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_local_artifacts: Option<bool>,

    /// Whether CI should trigger releases by dispatch instead of tag push (default false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispatch_releases: Option<bool>,

    /// Instead of triggering releases on tags, trigger on pushing to a specific branch
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_branch: Option<String>,

    /// Which actions to run on pull requests.
    ///
    /// "upload" will build and upload release artifacts, while "plan" will
    /// only plan out the release without running builds and "skip" will disable
    /// pull request runs entirely.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_run_mode: Option<cargo_dist_schema::PrRunMode>,

    /// a prefix to add to the release.yml and tag pattern so that
    /// cargo-dist can co-exist with other release workflows in complex workspaces
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_namespace: Option<String>,

    /// Plan jobs to run in CI
    ///
    /// The core plan job is always run, but this allows additional hooks
    /// to be added to the process to run concurrently with plan.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_jobs: Option<Vec<JobStyle>>,

    /// Local artifacts jobs to run in CI
    ///
    /// The core build job is always run, but this allows additional hooks
    /// to be added to the process to run concurrently with "upload local artifacts".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_local_jobs: Option<Vec<JobStyle>>,

    /// Global artifacts jobs to run in CI
    ///
    /// The core build job is always run, but this allows additional hooks
    /// to be added to the process to run concurrently with "upload global artifacts".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_global_jobs: Option<Vec<JobStyle>>,

    /// Host jobs to run in CI
    ///
    /// The core build job is always run, but this allows additional hooks
    /// to be added to the process to run concurrently with host.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_jobs: Option<Vec<JobStyle>>,

    /// Publish jobs to run in CI
    ///
    /// (defaults to none)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_jobs: Option<Vec<JobStyle>>,

    /// Post-announce jobs to run in CI
    ///
    /// This allows custom jobs to be configured to run after the announce job
    // runs in its entirety.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_announce_jobs: Option<Vec<JobStyle>>,
}
/// TODO
#[derive(Debug, Default, Clone)]
pub struct CommonCiConfig {
    /// Whether we should try to merge otherwise-parallelizable tasks onto the same machine,
    pub merge_tasks: bool,

    /// Whether failing tasks should make us give up on all other tasks
    pub fail_fast: bool,

    /// Whether CI tasks should have build caches enabled.
    pub cache_builds: Option<bool>,

    /// Whether CI should include logic to build local artifacts (default true)
    ///
    /// If false, it will be assumed that the local_artifacts_jobs will include custom
    /// jobs to build them.
    pub build_local_artifacts: bool,

    /// Whether CI should trigger releases by dispatch instead of tag push (default false)
    pub dispatch_releases: bool,

    /// Instead of triggering releases on tags, trigger on pushing to a specific branch
    pub release_branch: Option<String>,

    /// Which actions to run on pull requests.
    pub pr_run_mode: cargo_dist_schema::PrRunMode,

    /// a prefix to add to the release.yml and tag pattern so that
    /// cargo-dist can co-exist with other release workflows in complex workspaces
    pub tag_namespace: Option<String>,

    /// Plan jobs to run in CI
    pub plan_jobs: Vec<JobStyle>,

    /// Local artifacts jobs to run in CI
    pub build_local_jobs: Vec<JobStyle>,

    /// Global artifacts jobs to run in CI
    pub build_global_jobs: Vec<JobStyle>,

    /// Host jobs to run in CI
    pub host_jobs: Vec<JobStyle>,

    /// Publish jobs to run in CI
    pub publish_jobs: Vec<JobStyle>,

    /// Post-announce jobs to run in CI
    pub post_announce_jobs: Vec<JobStyle>,
}
impl CommonCiConfig {
    /// TODO
    pub fn defaults_for_workspace(_workspaces: &WorkspaceGraph) -> Self {
        Self {
            merge_tasks: false,
            fail_fast: false,
            cache_builds: None,
            build_local_artifacts: true,
            dispatch_releases: false,
            release_branch: None,
            pr_run_mode: cargo_dist_schema::PrRunMode::default(),
            tag_namespace: None,
            plan_jobs: vec![],
            build_local_jobs: vec![],
            build_global_jobs: vec![],
            host_jobs: vec![],
            publish_jobs: vec![],
            post_announce_jobs: vec![],
        }
    }
}
impl ApplyLayer for CommonCiConfig {
    type Layer = CommonCiLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            merge_tasks,
            fail_fast,
            cache_builds,
            build_local_artifacts,
            dispatch_releases,
            release_branch,
            pr_run_mode,
            tag_namespace,
            plan_jobs,
            build_local_jobs,
            build_global_jobs,
            host_jobs,
            publish_jobs,
            post_announce_jobs,
        }: Self::Layer,
    ) {
        self.merge_tasks.apply_val(merge_tasks);
        self.fail_fast.apply_val(fail_fast);
        self.cache_builds.apply_opt(cache_builds);
        self.build_local_artifacts.apply_val(build_local_artifacts);
        self.dispatch_releases.apply_val(dispatch_releases);
        self.release_branch.apply_opt(release_branch);
        self.pr_run_mode.apply_val(pr_run_mode);
        self.tag_namespace.apply_opt(tag_namespace);
        self.plan_jobs.apply_val(plan_jobs);
        self.build_local_jobs.apply_val(build_local_jobs);
        self.build_global_jobs.apply_val(build_global_jobs);
        self.host_jobs.apply_val(host_jobs);
        self.publish_jobs.apply_val(publish_jobs);
        self.post_announce_jobs.apply_val(post_announce_jobs);
    }
}
impl ApplyLayer for CommonCiLayer {
    type Layer = CommonCiLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            merge_tasks,
            fail_fast,
            cache_builds,
            build_local_artifacts,
            dispatch_releases,
            release_branch,
            pr_run_mode,
            tag_namespace,
            plan_jobs,
            build_local_jobs,
            build_global_jobs,
            host_jobs,
            publish_jobs,
            post_announce_jobs,
        }: Self::Layer,
    ) {
        self.merge_tasks.apply_opt(merge_tasks);
        self.fail_fast.apply_opt(fail_fast);
        self.cache_builds.apply_opt(cache_builds);
        self.build_local_artifacts.apply_opt(build_local_artifacts);
        self.dispatch_releases.apply_opt(dispatch_releases);
        self.release_branch.apply_opt(release_branch);
        self.pr_run_mode.apply_opt(pr_run_mode);
        self.tag_namespace.apply_opt(tag_namespace);
        self.plan_jobs.apply_opt(plan_jobs);
        self.build_local_jobs.apply_opt(build_local_jobs);
        self.build_global_jobs.apply_opt(build_global_jobs);
        self.host_jobs.apply_opt(host_jobs);
        self.publish_jobs.apply_opt(publish_jobs);
        self.post_announce_jobs.apply_opt(post_announce_jobs);
    }
}
