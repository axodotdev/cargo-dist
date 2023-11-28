//! Functionality required to invoke a generic build's `build-command`

use std::{
    env,
    io::{stderr, Write},
    process::Command,
};

use camino::Utf8Path;
use miette::{miette, Context, IntoDiagnostic};
use tracing::info;

use crate::{
    copy_file,
    env::{calculate_cflags, calculate_ldflags, fetch_brew_env, parse_env, select_brew_env},
    errors::Result,
    BinaryIdx, BuildStep, DistGraph, DistGraphBuilder, GenericBuildStep, SortedMap, TargetTriple,
};

impl<'a> DistGraphBuilder<'a> {
    pub(crate) fn compute_generic_builds(&mut self) -> Vec<BuildStep> {
        // For now we can be really simplistic and just do a workspace build for every
        // target-triple we have a binary-that-needs-a-real-build for.
        let mut targets = SortedMap::<TargetTriple, Vec<BinaryIdx>>::new();
        for (binary_idx, binary) in self.inner.binaries.iter().enumerate() {
            if !binary.copy_exe_to.is_empty() || !binary.copy_symbols_to.is_empty() {
                targets
                    .entry(binary.target.clone())
                    .or_default()
                    .push(BinaryIdx(binary_idx));
            }
        }

        let mut builds = vec![];
        for (target, binaries) in targets {
            builds.push(BuildStep::Generic(GenericBuildStep {
                target_triple: target.clone(),
                expected_binaries: binaries,
                build_command: self
                    .workspace
                    .build_command
                    .clone()
                    .expect("A build command is mandatory for generic builds"),
            }));
        }

        builds
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

/// Build a generic target
pub fn build_generic_target(dist_graph: &DistGraph, target: &GenericBuildStep) -> Result<()> {
    let mut command_string = target.build_command.clone();
    eprintln!(
        "building generic target ({} via {})",
        target.target_triple,
        command_string.join(" ")
    );

    let mut desired_extra_env = vec![];
    let mut cflags = None;
    let mut ldflags = None;
    let skip_brewfile = env::var("DO_NOT_USE_BREWFILE").is_ok();
    if !skip_brewfile {
        if let Some(env_output) = fetch_brew_env(dist_graph)? {
            let brew_env = parse_env(&env_output)?;
            desired_extra_env = select_brew_env(&brew_env);
            cflags = Some(calculate_cflags(&brew_env));
            ldflags = Some(calculate_ldflags(&brew_env));
        }
    }

    let args = command_string.split_off(1);
    let mut command = Command::new(
        command_string
            .first()
            .expect("The build command must contain at least one entry"),
    );
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::inherit());
    for arg in args {
        command.arg(arg);
    }
    // If we generated any extra environment variables to
    // inject into the environment, apply them now.
    command.envs(desired_extra_env);

    // Ensure we inform the build what architecture and platform
    // it's building for.
    command.env("CARGO_DIST_TARGET", &target.target_triple);

    let cc =
        std::env::var("CC").unwrap_or(platform_appropriate_cc(&target.target_triple).to_owned());
    command.env("CC", cc);
    let cxx =
        std::env::var("CXX").unwrap_or(platform_appropriate_cxx(&target.target_triple).to_owned());
    command.env("CXX", cxx);

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

    info!("exec: {:?}", command);
    let result = command
        .output()
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to exec generic build: {command:?}"))?;

    if !result.status.success() {
        println!("Build exited non-zero: {}", result.status);
    }
    eprintln!();
    eprintln!("stdout:");
    stderr().write_all(&result.stdout).into_diagnostic()?;

    // Check that we got everything we expected, and normalize to ArtifactIdx => Artifact Path
    for binary_idx in &target.expected_binaries {
        let binary = dist_graph.binary(*binary_idx);
        let binary_path = Utf8Path::new(&binary.file_name);
        if binary_path.exists() {
            for dest in &binary.copy_exe_to {
                copy_file(binary_path, dest)?;
            }
        } else {
            return Err(miette!(
                "failed to find bin {} -- did the build above have errors?",
                binary_path
            ));
        }
    }

    Ok(())
}
