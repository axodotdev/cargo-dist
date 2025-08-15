//! github host

use super::*;

/// github host config (raw)
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct GithubHostLayer {
    /// Common options
    #[serde(flatten)]
    pub common: CommonHostLayer,

    /// Whether we should create the Github Release for you when you push a tag.
    ///
    /// If true (default), dist will create a new Github Release and generate
    /// a title/body for it based on your changelog.
    ///
    /// If false, dist will assume a draft Github Release already exists
    /// with the title/body you want. At the end of a successful publish it will
    /// undraft the Github Release.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create: Option<bool>,

    /// Publish GitHub Releases to this repo instead of the current one
    ///
    /// The user must also set GH_RELEASES_TOKEN in their SECRETS
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo: Option<GithubRepoPair>,

    /// If `repo` is used, the commit ref to used will
    /// be read from the HEAD of the submodule at this path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submodule_path: Option<Utf8PathBuf>,

    /// Which phase to create the github release in
    #[serde(skip_serializing_if = "Option::is_none")]
    pub during: Option<GithubReleasePhase>,

    /// Whether GitHub Attestations is enabled (default false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attestations: Option<bool>,

    /// GitHub Attestation filters (default *)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attestations_filters: Option<GithubAttestationsFilters>,

    /// When to generate GitHub Attestations (default build-local-artifacts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attestations_phase: Option<GithubAttestationsPhase>,
}
/// github host config (final)
#[derive(Debug, Default, Clone)]
pub struct GithubHostConfig {
    /// Common options
    pub common: CommonHostConfig,
    /// Whether we should create the Github Release for you
    pub create: bool,
    /// Publish GitHub Releases to this repo instead of the current one
    pub repo: Option<GithubRepoPair>,
    /// If github-releases-repo is used, the commit ref to used will
    /// be read from the HEAD of the submodule at this path
    pub submodule_path: Option<Utf8PathBuf>,
    /// Which phase to create the github release in
    pub during: GithubReleasePhase,
    /// Whether GitHub Attestations is enabled (default false)
    pub attestations: bool,
    /// GitHub Attestation filters (default *)
    pub attestations_filters: GithubAttestationsFilters,
    /// When to generate GitHub Attestations (default build-local-artifacts)
    pub attestations_phase: GithubAttestationsPhase,
}

impl GithubHostConfig {
    /// Get defaults for the given package
    pub fn defaults_for_workspace(_workspaces: &WorkspaceGraph, common: &CommonHostConfig) -> Self {
        Self {
            common: common.clone(),
            create: true,
            repo: None,
            submodule_path: None,
            during: GithubReleasePhase::default(),
            attestations: false,
            attestations_filters: GithubAttestationsFilters::default(),
            attestations_phase: GithubAttestationsPhase::default(),
        }
    }
}

impl ApplyLayer for GithubHostConfig {
    type Layer = GithubHostLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            common,
            create,
            repo,
            submodule_path,
            during,
            attestations,
            attestations_filters,
            attestations_phase,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.create.apply_val(create);
        self.repo.apply_opt(repo);
        self.submodule_path.apply_opt(submodule_path);
        self.during.apply_val(during);
        self.attestations.apply_val(attestations);
        self.attestations_filters.apply_val(attestations_filters);
        self.attestations_phase.apply_val(attestations_phase);
    }
}
impl ApplyLayer for GithubHostLayer {
    type Layer = GithubHostLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            common,
            create,
            repo,
            submodule_path,
            during,
            attestations,
            attestations_filters,
            attestations_phase,
        }: Self::Layer,
    ) {
        self.common.apply_layer(common);
        self.create.apply_opt(create);
        self.repo.apply_opt(repo);
        self.submodule_path.apply_opt(submodule_path);
        self.during.apply_opt(during);
        self.attestations.apply_opt(attestations);
        self.attestations_filters.apply_opt(attestations_filters);
        self.attestations_phase.apply_opt(attestations_phase);
    }
}

impl std::ops::Deref for GithubHostConfig {
    type Target = CommonHostConfig;
    fn deref(&self) -> &Self::Target {
        &self.common
    }
}
