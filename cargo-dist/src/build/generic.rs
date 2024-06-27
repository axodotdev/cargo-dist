//! Functionality required to invoke a generic build's `build-command`

use std::{env, process::ExitStatus};

use axoprocess::Cmd;
use axoproject::WorkspaceIdx;
use camino::{Utf8Path, Utf8PathBuf};
use cargo_dist_schema::DistManifest;

use crate::{
    build::{package_id_string, BuildExpectations},
    copy_file,
    env::{calculate_cflags, calculate_ldflags, fetch_brew_env, parse_env, select_brew_env},
    ArtifactKind, BinaryIdx, BuildStep, DistError, DistGraph, DistGraphBuilder, DistResult,
    ExtraBuildStep, GenericBuildStep, SortedMap, TargetTriple,
};

impl<'a> DistGraphBuilder<'a> {
    pub(crate) fn compute_generic_builds(&mut self, workspace_idx: WorkspaceIdx) -> Vec<BuildStep> {
        // For now we can be really simplistic and just do a workspace build for every
        // target-triple we have a binary-that-needs-a-real-build for.
        let mut targets = SortedMap::<TargetTriple, Vec<BinaryIdx>>::new();
        for (binary_idx, binary) in self.inner.binaries.iter().enumerate() {
            // Only bother with binaries owned by this workspace
            if self.workspaces.workspace_for_package(binary.pkg_idx) != workspace_idx {
                continue;
            }
            if !binary.copy_exe_to.is_empty() || !binary.copy_symbols_to.is_empty() {
                targets
                    .entry(binary.target.clone())
                    .or_default()
                    .push(BinaryIdx(binary_idx));
            }
        }

        let mut builds = vec![];
        for (target, binaries) in targets {
            // `(target, pkg_idx)` uniquely identifies a build we need to do,
            // so group all the binaries under those buckets and add a build for each one
            // (targets is handled by the loop we're in)
            let mut builds_by_pkg_idx = SortedMap::new();
            for bin_idx in binaries {
                let bin = self.binary(bin_idx);
                builds_by_pkg_idx
                    .entry(bin.pkg_idx)
                    .or_insert(vec![])
                    .push(bin_idx);
            }
            for (pkg_idx, expected_binaries) in builds_by_pkg_idx {
                let package = self.workspaces.package(pkg_idx);
                builds.push(BuildStep::Generic(GenericBuildStep {
                    target_triple: target.clone(),
                    expected_binaries,
                    working_dir: package.package_root.clone(),
                    out_dir: package.package_root.clone(),
                    build_command: package
                        .build_command
                        .clone()
                        .expect("A build command is mandatory for generic builds"),
                }));
            }
        }
        builds
    }

    pub(crate) fn compute_extra_builds(&mut self) -> Vec<BuildStep> {
        // Get all the extra artifacts
        let extra_artifacts = self.inner.artifacts.iter().filter_map(|artifact| {
            if let ArtifactKind::ExtraArtifact(extra) = &artifact.kind {
                Some(extra)
            } else {
                None
            }
        });

        // Gather up and dedupe extra builds
        let mut by_command = SortedMap::<(Utf8PathBuf, Vec<String>), Vec<Utf8PathBuf>>::new();
        for extra in extra_artifacts {
            by_command
                .entry((extra.working_dir.clone(), extra.command.clone()))
                .or_default()
                .push(extra.artifact_relpath.clone())
        }

        by_command
            .into_iter()
            .map(|((working_dir, build_command), expected_artifacts)| {
                BuildStep::Extra(ExtraBuildStep {
                    working_dir,
                    build_command,
                    artifact_relpaths: expected_artifacts,
                })
            })
            .collect()
    }
}

fn platform_appropriate_cc(target: &str) -> &str {
    if target.contains("darwin") {
        "clang"
    } else if target.contains("linux") {
        "gcc"
    } else if target.contains("windows") {
        "cl.exe"
    } else {
        "cc"
    }
}

fn platform_appropriate_cxx(target: &str) -> &str {
    if target.contains("darwin") {
        "clang++"
    } else if target.contains("linux") {
        "g++"
    } else if target.contains("windows") {
        "cl.exe"
    } else {
        "c++"
    }
}

fn run_build(
    dist_graph: &DistGraph,
    build_command: &[String],
    working_dir: &Utf8Path,
    target: Option<&TargetTriple>,
) -> DistResult<ExitStatus> {
    let mut command_string = build_command.to_owned();

    let mut desired_extra_env = vec![];
    let mut cflags = None;
    let mut ldflags = None;
    let skip_brewfile = env::var("DO_NOT_USE_BREWFILE").is_ok();
    if !skip_brewfile {
        if let Some(env_output) = fetch_brew_env(dist_graph, working_dir)? {
            let brew_env = parse_env(&env_output)?;
            desired_extra_env = select_brew_env(&brew_env);
            cflags = Some(calculate_cflags(&brew_env));
            ldflags = Some(calculate_ldflags(&brew_env));
        }
    }

    let args = command_string.split_off(1);
    let command_name = command_string
        .first()
        .expect("The build command must contain at least one entry");
    let mut command = Cmd::new(command_name, format!("exec generic build: {command_name}"));
    command.current_dir(working_dir);
    command.stdout_to_stderr();
    for arg in args {
        command.arg(arg);
    }
    // If we generated any extra environment variables to
    // inject into the environment, apply them now.
    command.envs(desired_extra_env);

    if let Some(target) = target {
        // Ensure we inform the build what architecture and platform
        // it's building for.
        command.env("CARGO_DIST_TARGET", target);

        let cc = std::env::var("CC").unwrap_or(platform_appropriate_cc(target).to_owned());
        command.env("CC", cc);
        let cxx = std::env::var("CXX").unwrap_or(platform_appropriate_cxx(target).to_owned());
        command.env("CXX", cxx);
    }

    // Pass CFLAGS/LDFLAGS for C builds
    if let Some(cflags) = cflags {
        // These typically contain the same values as each other.
        // Properly speaking, CPPFLAGS is for C++ software and CFLAGS is for
        // C software, but many buildsystems treat them as interchangeable.
        command.env("CFLAGS", &cflags);
        command.env("CPPFLAGS", &cflags);
    }
    if let Some(ldflags) = ldflags {
        command.env("LDFLAGS", &ldflags);
    }

    Ok(command.status()?)
}

/// Build a generic targets
pub fn build_generic_target(
    dist_graph: &DistGraph,
    manifest: &mut DistManifest,
    target: &GenericBuildStep,
) -> DistResult<()> {
    eprintln!(
        "building generic target ({} via {})",
        target.target_triple,
        target.build_command.join(" ")
    );

    let result = run_build(
        dist_graph,
        &target.build_command,
        &target.working_dir,
        Some(&target.target_triple),
    )?;

    if !result.success() {
        eprintln!("Build exited non-zero: {}", result);
    }

    let mut expected = BuildExpectations::new(dist_graph, &target.expected_binaries);

    // Since generic builds provide no feedback, blindly assume we got what
    // we expected, BuildExpectations will check for us
    for binary_idx in &target.expected_binaries {
        let binary = dist_graph.binary(*binary_idx);
        let src_path = target.out_dir.join(&binary.file_name);
        expected.found_bin(package_id_string(binary.pkg_id.as_ref()), src_path, vec![]);
    }

    // Check and process the binaries
    expected.process_bins(dist_graph, manifest)?;

    Ok(())
}

/// Similar to the above, but with slightly different signatures since
/// it's not based around axoproject-identified binaries
pub fn run_extra_artifacts_build(dist: &DistGraph, build: &ExtraBuildStep) -> DistResult<()> {
    eprintln!(
        "building extra artifacts target (via {})",
        build.build_command.join(" ")
    );

    let result = run_build(dist, &build.build_command, &build.working_dir, None)?;

    if !result.success() {
        eprintln!("Build exited non-zero: {}", result);
    }

    // Check that we got everything we expected, and copy into the distribution path
    for artifact_relpath in &build.artifact_relpaths {
        let artifact_name = artifact_relpath.file_name().unwrap();
        let src_path = build.working_dir.join(artifact_relpath);
        let dest_path = dist.dist_dir.join(artifact_name);
        if src_path.exists() {
            copy_file(&src_path, &dest_path)?;
        } else {
            return Err(DistError::MissingBinaries {
                pkg_name: "extra build".to_owned(),
                bin_name: artifact_name.to_owned(),
            });
        }
    }

    Ok(())
}
