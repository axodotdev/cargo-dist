//! CI script generation
//!
//! In the future this may get split up into submodules.

use std::{io::Write, process::Command};

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

/// Info needed to build a docker image
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
const RUNNER_IMAGE: &str = "ubuntu:20.04";
/// What to call the temporary dockerfile we generate
const DOCKERFILE_NAME: &str = "Dockerfile";
/// The buildx builder backend we need to install + use
const DIST_BUILDER: &str = "dist-container";

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
        release
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
            .collect()
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
        eprintln!("building docker image ({})", self.target);
        // 1. check if the cargo-dist docker backend is setup
        // 2. create it if not
        // 3. do a build with the cargo-dist backend
        //
        // We do this because the kind of build+export we want to do requires a
        // "docker-container" buildx builder, which doesn't exist by default. So
        // we need to create one, and then use it. Unfortunately this is persistent
        // and non-idempotent, so we need to check if we've already done it first.
        //
        // FIXME: is there any way to do this that doesn't require persistently modifying
        // the current docker install?
        let has_dist_builder = {
            let mut cmd = Command::new(&docker.cmd);
            cmd.arg("buildx")
                .arg("inspect")
                .arg(DIST_BUILDER)
                .current_dir(&self.package_dir);
            let output = cmd.output().map_err(|cause| DistError::CommandFail {
                command_summary: "inspect your docker install with 'docker buildx inspect'"
                    .to_owned(),
                cause,
            })?;
            output.status.success()
        };
        if !has_dist_builder {
            let mut cmd = Command::new(&docker.cmd);
            cmd.arg("buildx")
                .arg("create")
                .arg("--name")
                .arg(DIST_BUILDER)
                .arg("--driver=docker-container")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::inherit())
                .current_dir(&self.package_dir);
            let output = cmd.output().map_err(|cause| DistError::CommandFail {
                command_summary: "configure docker to use the docker-container driver with 'docker buildx create'".to_owned(),
                cause,
            })?;

            if !output.stdout.is_empty() {
                eprintln!();
                eprintln!("docker stdout:");
                std::io::stderr()
                    .write_all(&output.stdout)
                    .expect("failed to write to stdout");
            }
            if !output.status.success() {
                return Err(DistError::CommandStatus {
                    command_summary: "configure docker to use the docker-container driver with 'docker buildx create'".to_owned(),
                    status: output.status,
                });
            }
        }
        {
            let mut cmd = Command::new(&docker.cmd);
            cmd.arg("buildx")
                .arg("build")
                .arg(".")
                .arg("--tag")
                .arg(&self.tag)
                .arg(format!("--builder={DIST_BUILDER}"))
                .arg("--output")
                .arg(format!("type=docker,dest={}", self.file_path))
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::inherit())
                .current_dir(&self.package_dir);
            let output = cmd.output().map_err(|cause| DistError::CommandFail {
                command_summary: "build your docker image with 'docker buildx build'".to_owned(),
                cause,
            })?;

            if !output.stdout.is_empty() {
                eprintln!();
                eprintln!("docker stdout:");
                std::io::stderr()
                    .write_all(&output.stdout)
                    .expect("failed to write to stdout");
            }
            if !output.status.success() {
                return Err(DistError::CommandStatus {
                    command_summary: "build your docker image with 'docker buildx build'"
                        .to_owned(),
                    status: output.status,
                });
            }
        }
        Ok(())
    }
}
