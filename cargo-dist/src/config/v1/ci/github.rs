//! github ci config

use cargo_dist_schema::{
    ContainerConfig, GithubRunner, GithubRunnerConfig, GithubRunnerConfigInput, StringLikeOr,
    TripleName,
};

use crate::platform::{github_runners::target_for_github_runner, targets};
use crate::SortedSet;

use super::*;

/// github ci config (raw from file)
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct GithubCiLayer {
    /// Common options
    #[serde(flatten)]
    pub common: CommonCiLayer,

    /// Custom GitHub runners, mapped by triple target
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runners: Option<SortedMap<TripleName, StringLikeOr<GithubRunner, GithubRunnerConfigInput>>>,

    /// Custom permissions for jobs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<SortedMap<String, GithubPermissionMap>>,

    /// Custom secrets for jobs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<SortedMap<String, GithubSecretSpec>>,

    /// Custom build setup for jobs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_setup: Option<String>,

    /// Use these commits for actions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_commits: Option<SortedMap<String, String>>,
}

/// github ci config (final)
#[derive(Debug, Default, Clone)]
pub struct GithubCiConfig {
    /// Common options
    pub common: CommonCiConfig,

    /// Custom GitHub runners, mapped by triple target
    pub runners: SortedMap<TripleName, GithubRunnerConfig>,

    /// Custom permissions for jobs
    pub permissions: SortedMap<String, GithubPermissionMap>,

    /// Custom secrets for jobs
    pub secrets: SortedMap<String, GithubSecretMap>,

    /// Jobs which had secrets configured (used to distinguish unset vs empty maps)
    pub configured_secret_jobs: SortedSet<String>,

    /// Custom build setup for jobs
    pub build_setup: Option<String>,

    /// Use these commits for github actions
    pub action_commits: SortedMap<String, String>,
}

impl GithubCiConfig {
    /// Get defaults for the given package
    pub fn defaults_for_workspace(_workspaces: &WorkspaceGraph, common: &CommonCiConfig) -> Self {
        Self {
            common: common.clone(),
            runners: Default::default(),
            permissions: Default::default(),
            secrets: Default::default(),
            configured_secret_jobs: Default::default(),
            action_commits: Default::default(),
            build_setup: None,
        }
    }
}

impl ApplyLayer for GithubCiConfig {
    type Layer = GithubCiLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            common,
            runners,
            permissions,
            secrets,
            build_setup,
            action_commits,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);

        let mk_default_github_runner = || GithubRunner::new("ubuntu-22.04".to_owned());
        self.runners.apply_val(runners.map(|runners| {
            runners
                .into_iter()
                .map(|(target_triple, runner)| {
                    (
                        target_triple.clone(),
                        match runner {
                            StringLikeOr::StringLike(runner) => {
                                let host = target_for_github_runner(&runner)
                                    .map(|t| t.to_owned())
                                    .unwrap_or_else(|| target_triple.clone());
                                GithubRunnerConfig {
                                    host,
                                    runner,
                                    container: None,
                                }
                            }
                            StringLikeOr::Val(runner_config) => {
                                let runner = runner_config
                                    .runner
                                    .unwrap_or_else(mk_default_github_runner);
                                let host = runner_config
                                    .host
                                    .or_else(|| {
                                        target_for_github_runner(&runner).map(|t| t.to_owned())
                                    })
                                    .unwrap_or_else(|| {
                                        // if not specified, then assume the custom github runner is
                                        // the right platform (host == target)
                                        target_triple.clone()
                                    });
                                let container =
                                    runner_config.container.map(|container| match container {
                                        StringLikeOr::StringLike(image_name) => {
                                            ContainerConfig {
                                                image: image_name,
                                                // assume x86_64-unknown-linux-musl if not specified
                                                host: targets::TARGET_X64_LINUX_MUSL.to_owned(),
                                                package_manager: None,
                                            }
                                        }
                                        StringLikeOr::Val(container_config) => ContainerConfig {
                                            image: container_config.image,
                                            host: container_config.host.unwrap_or_else(|| {
                                                targets::TARGET_X64_LINUX_MUSL.to_owned()
                                            }),
                                            package_manager: container_config.package_manager,
                                        },
                                    });
                                GithubRunnerConfig {
                                    runner,
                                    host,
                                    container,
                                }
                            }
                        },
                    )
                })
                .collect()
        }));
        self.permissions.apply_val(permissions);
        if let Some(secrets) = secrets {
            self.configured_secret_jobs
                .apply_val(Some(secrets.keys().cloned().collect()));
            self.secrets = secrets
                .into_iter()
                .map(|(job_name, spec)| (job_name, spec.into_map()))
                .collect();
        }
        self.build_setup.apply_opt(build_setup);
        self.action_commits.apply_val(action_commits);
    }
}
impl ApplyLayer for GithubCiLayer {
    type Layer = GithubCiLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            common,
            runners,
            permissions,
            secrets,
            build_setup,
            action_commits,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.runners.apply_opt(runners);
        self.permissions.apply_opt(permissions);
        self.secrets.apply_opt(secrets);
        self.build_setup.apply_opt(build_setup);
        self.action_commits.apply_opt(action_commits);
    }
}

impl std::ops::Deref for GithubCiConfig {
    type Target = CommonCiConfig;
    fn deref(&self) -> &Self::Target {
        &self.common
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_ci_layer_normalizes_secret_specs() {
        let mut config = GithubCiConfig::default();
        config.apply_layer(GithubCiLayer {
            common: CommonCiLayer::default(),
            runners: None,
            permissions: None,
            secrets: Some(SortedMap::from_iter([
                (
                    "job-list".to_owned(),
                    GithubSecretSpec::List(vec!["TOKEN_A".to_owned(), "TOKEN_B".to_owned()]),
                ),
                (
                    "job-map".to_owned(),
                    GithubSecretSpec::Map(SortedMap::from_iter([(
                        "NPM_TOKEN".to_owned(),
                        "ORG_NPM_TOKEN".to_owned(),
                    )])),
                ),
                ("job-empty-list".to_owned(), GithubSecretSpec::List(vec![])),
                (
                    "job-empty-map".to_owned(),
                    GithubSecretSpec::Map(SortedMap::new()),
                ),
            ])),
            build_setup: None,
            action_commits: None,
        });

        assert_eq!(
            config.secrets.get("job-list"),
            Some(&SortedMap::from_iter([
                ("TOKEN_A".to_owned(), "TOKEN_A".to_owned()),
                ("TOKEN_B".to_owned(), "TOKEN_B".to_owned()),
            ]))
        );
        assert_eq!(
            config.secrets.get("job-map"),
            Some(&SortedMap::from_iter([(
                "NPM_TOKEN".to_owned(),
                "ORG_NPM_TOKEN".to_owned(),
            )]))
        );
        assert_eq!(
            config.secrets.get("job-empty-list"),
            Some(&SortedMap::new())
        );
        assert_eq!(config.secrets.get("job-empty-map"), Some(&SortedMap::new()));

        assert!(config.configured_secret_jobs.contains("job-list"));
        assert!(config.configured_secret_jobs.contains("job-map"));
        assert!(config.configured_secret_jobs.contains("job-empty-list"));
        assert!(config.configured_secret_jobs.contains("job-empty-map"));
    }
}
