//! CI script generation
//!
//! In the future this may get split up into submodules.

use std::process::Command;

use axoasset::LocalAsset;
use camino::Utf8PathBuf;
use cargo_dist_schema::DistManifest;
use serde::Serialize;

use crate::{
    backend::templates::{Templates, TEMPLATE_INSTALLER_DOCKER},
    config::DependencyKind,
    errors::{DistError, DistResult},
    DistGraph, ReleaseIdx,
};

/// Info needed to build an msi
#[derive(Debug, Clone)]
pub struct DockerInstallerInfo {
    /// Target triple this image is for
    pub target: String,
    /// Release this is for
    pub release_idx: ReleaseIdx,
    /// What to tag the image as
    pub tag: String,
    /// Binaries we'll be baking into the docker image
    pub bins: Vec<String>,
    /// Final file path of the docker image
    pub file_path: Utf8PathBuf,
    /// Dir stuff goes to
    pub package_dir: Utf8PathBuf,
    /// Additional dirs/files to include
    pub includes: Vec<Utf8PathBuf>,
}

/// Info about running cargo-dist in Github CI
#[derive(Debug, Serialize)]
pub struct DockerfileInfo {
    /// List of binaries this docker image should include
    bins: Vec<Bin>,
    /// Info about the runner image
    runner: RunnerImage,
}

/// A binary
#[derive(Debug, Serialize)]
struct Bin {
    /// Path to where the binary should be copied from, on the host system
    source: String,
    /// Name of the binary, as exposed to users of the image
    name: String,
}

/// Info about the runner image
#[derive(Debug, Serialize)]
struct RunnerImage {
    /// Base image (e.g. some debian distro)
    image: String,
    /// Apt deps to install
    apt_deps: Vec<String>,
    /// Additional files/dirs to copy into the image (e.g. examples)
    includes: Vec<Utf8PathBuf>,
}

/// As of this rewriting this is the latest debian release
const RUNNER_IMAGE: &str = "debian:bookworm-slim";
/// What to call the temporary dockerfile we generate
const DOCKERFILE_NAME: &str = "Dockerfile";

impl DockerInstallerInfo {
    /// Build the docker image
    pub fn build(
        &self,
        templates: &Templates,
        dist: &DistGraph,
        manifest: &DistManifest,
    ) -> DistResult<()> {
        let info = self.compute_dockerfile(dist, manifest)?;
        self.render_dockerfile(templates, &info)?;
        self.build_and_export_image(dist)?;
        Ok(())
    }

    /// Compute the apt runner deps
    ///
    /// We want this to be late-bound and to reference DistManifest
    /// so linkage data can be incorporated.
    fn apt_runner_deps(&self, dist: &DistGraph, _manifest: &DistManifest) -> Vec<String> {
        // FIXME: incorporate linkage info to only include Explicit runtime deps
        // and to also auto-include runtime deps they forgot to mention!
        // (soft-blocked on doing a sysdeps refactor)
        let release = dist.release(self.release_idx);
        let packages: Vec<String> = release
            .system_dependencies
            .apt
            .clone()
            .into_iter()
            .filter(|(_, package)| package.0.wanted_for_target(&self.target))
            .filter(|(_, package)| package.0.stage_wanted(&DependencyKind::Run))
            .map(|(name, spec)| {
                if let Some(version) = spec.0.version {
                    format!("{name}={version}")
                } else {
                    name
                }
            })
            .collect();
        packages
    }

    /// Compute the data for the dockerfile
    ///
    /// Split off from rendering in case it's useful for testing.
    fn compute_dockerfile(
        &self,
        dist: &DistGraph,
        manifest: &DistManifest,
    ) -> DistResult<DockerfileInfo> {
        let apt_deps = self.apt_runner_deps(dist, manifest);

        let bins = self
            .bins
            .iter()
            .map(|file_name| Bin {
                source: format!("./{file_name}"),
                name: file_name.clone(),
            })
            .collect();
        let runner = RunnerImage {
            image: RUNNER_IMAGE.to_owned(),
            apt_deps,
            includes: self.includes.clone(),
        };
        Ok(DockerfileInfo { bins, runner })
    }

    /// Render the dockerfile with the given data
    fn render_dockerfile(&self, templates: &Templates, info: &DockerfileInfo) -> DistResult<()> {
        let contents = templates.render_file_to_clean_string(TEMPLATE_INSTALLER_DOCKER, &info)?;
        let dockerfile_path = self.package_dir.join(DOCKERFILE_NAME);
        LocalAsset::write_new(&contents, dockerfile_path)?;
        Ok(())
    }

    /// Build and export the docker image to a tarball with appropriate metadata,
    /// assuming `render_dockerfile` has already run.
    fn build_and_export_image(&self, dist: &DistGraph) -> DistResult<()> {
        let Some(docker) = &dist.tools.docker else {
            unreachable!("docker didn't exist, tasks.rs was supposed to catch that!");
        };
        // FIXME: in theory we can do "build" and "save" in one shot with
        //
        //    docker buildx build --output type=docker,dest=/path/to/output.tar
        //
        // which is what the Github Action seems to do, but locally (in WSL) i get:
        //
        // > ERROR: Docker exporter feature is currently not supported for docker driver.
        // > Please switch to a different driver (eg. "docker buildx create --use")
        //
        // So for now just use `save` which Does work
        {
            let mut cmd = Command::new(&docker.cmd);
            cmd.arg("build")
                .arg(".")
                .arg("-t")
                .arg(&self.tag)
                .current_dir(&self.package_dir);
            let status = cmd.status().map_err(|cause| DistError::CommandFail {
                command_summary: "export your docker image with 'docker build'".to_owned(),
                cause,
            })?;

            if !status.success() {
                return Err(DistError::CommandStatus {
                    command_summary: "build your docker image with 'docker build'".to_owned(),
                    status,
                });
            }
        }
        {
            let mut cmd = Command::new(&docker.cmd);
            cmd.arg("save")
                .arg("--output")
                .arg(&self.file_path)
                .arg(&self.tag)
                .current_dir(&self.package_dir);
            let status = cmd.status().map_err(|cause| DistError::CommandFail {
                command_summary: "export your docker image with 'docker save'".to_owned(),
                cause,
            })?;

            if !status.success() {
                return Err(DistError::CommandStatus {
                    command_summary: "export your docker image with 'docker save'".to_owned(),
                    status,
                });
            }
        }
        Ok(())
    }
}
