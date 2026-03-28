//! Functionality required to invoke `cargo build` properly

use std::env;

use axoprocess::Cmd;
use axoproject::WorkspaceIdx;
use cargo_dist_schema::target_lexicon::{Architecture, Environment, Triple};
use cargo_dist_schema::{DistManifest, TripleName};
use miette::{Context, IntoDiagnostic};
use tracing::warn;

use crate::build::BuildExpectations;
use crate::env::{calculate_ldflags, fetch_brew_env, parse_env, select_brew_env};
use crate::{
    build_wrapper_for_cross, errors::*, BinaryIdx, BuildStep, CargoBuildWrapper, DistGraphBuilder,
    AXOUPDATER_MINIMUM_VERSION, PROFILE_DIST,
};
use crate::{
    CargoBuildStep, CargoTargetFeatureList, CargoTargetPackages, DistGraph, RustupStep, SortedMap,
};

impl<'a> DistGraphBuilder<'a> {
    pub(crate) fn compute_cargo_builds(
        &mut self,
        workspace_idx: WorkspaceIdx,
    ) -> DistResult<Vec<BuildStep>> {
        let cargo = self.inner.tools.cargo()?;
        // For now we can be really simplistic and just do a workspace build for every
        // target-triple we have a binary-that-needs-a-real-build for.
        let mut targets = SortedMap::<TripleName, Vec<BinaryIdx>>::new();
        let working_dir = self
            .workspaces
            .workspace(workspace_idx)
            .workspace_dir
            .clone();

        for (binary_idx, binary) in self.inner.binaries.iter().enumerate() {
            let package = self.workspaces.package(binary.pkg_idx);

            let oldest = package
                .axoupdater_versions
                .iter()
                .min_by(|a, b| a.1.cmp(&b.1));
            if let Some((source, axoproject::Version::Cargo(version))) = oldest {
                let axoupdater_min_version = semver::Version::parse(AXOUPDATER_MINIMUM_VERSION)
                    .expect("invalid axoupdater const?!");

                if *version < axoupdater_min_version {
                    return Err(DistError::AxoupdaterTooOld {
                        package_name: package.name.to_owned(),
                        source_name: source.to_owned(),
                        minimum: axoupdater_min_version,
                        your_version: version.to_owned(),
                    });
                }
            }

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
        for (target_triple, binaries) in targets {
            let target = target_triple.parse()?;
            let mut rustflags = std::env::var("RUSTFLAGS").unwrap_or_default();

            // FIXME: is there a more principled way for us to add things to RUSTFLAGS
            // without breaking everything. Cargo has some builtin ways like keys
            // in [target...] tables that will get "merged" with the flags it wants
            // to set. More blunt approaches like actually setting the environment
            // variable I think can result in overwriting flags other places set
            // (which is defensible, having spaghetti flags randomly injected by
            // a dozen different tools is a build maintenance nightmare!)

            // You're *supposed* to link libc statically on windows but Rust has a bad default.
            // See: https://rust-lang.github.io/rfcs/1721-crt-static.html
            //
            // ... well ok it's actually more complicated than that. Most rust applications
            // don't dynamically link anything non-trivial, so statically linking libc is fine.
            // However if you need to dynamically link stuff there starts to be issues about
            // agreeing to the crt in play. At that point you should ship a
            // Visual C(++) Redistributable that installs the version of the runtime you depend
            // on. Not doing that is basically rolling some dice and hoping the user already
            // has it installed, which isn't great. We should support redists eventually,
            // but for now this hacky global flag is here to let you roll dice.
            if self.inner.config.builds.cargo.msvc_crt_static
                && target.environment == Environment::Msvc
            {
                rustflags.push_str(" -Ctarget-feature=+crt-static");
            }

            // Likewise, the default for musl will change in the future, so
            // we can future-proof this by adding the flag now
            // See: https://github.com/axodotdev/cargo-dist/issues/486
            if target.environment == Environment::Musl {
                rustflags.push_str(" -Ctarget-feature=+crt-static -Clink-self-contained=yes");
            }

            let host = cargo.host_target.parse()?;

            // If we're trying to cross-compile, ensure the rustup toolchain is set up!
            if target != host {
                if let Some(rustup) = self.inner.tools.rustup.clone() {
                    builds.push(BuildStep::Rustup(RustupStep {
                        rustup,
                        target: target_triple.clone(),
                    }));
                } else {
                    warn!("You're trying to cross-compile, but I can't find rustup to ensure you have the rust toolchains for it!")
                }
            }

            if self.inner.precise_cargo_builds {
                // `(target, package, features)` uniquely identifies a build we need to do,
                // so group all the binaries under those buckets and add a build for each one
                // (targets is handled by the loop we're in)
                let mut builds_by_pkg_spec = SortedMap::new();
                for bin_idx in binaries {
                    let bin = self.binary(bin_idx);
                    builds_by_pkg_spec
                        .entry((bin.pkg_spec.clone(), bin.features.clone()))
                        .or_insert(vec![])
                        .push(bin_idx);
                }
                for ((pkg_spec, features), expected_binaries) in builds_by_pkg_spec {
                    builds.push(BuildStep::Cargo(CargoBuildStep {
                        target_triple: target_triple.clone(),
                        package: CargoTargetPackages::Package(pkg_spec),
                        features,
                        rustflags: rustflags.clone(),
                        profile: String::from(PROFILE_DIST),
                        expected_binaries,
                        working_dir: working_dir.clone(),
                    }));
                }
            } else {
                // If we think a workspace build is possible, every binary agrees on the features, so take an arbitrary one
                let features = binaries
                    .first()
                    .map(|&idx| self.binary(idx).features.clone())
                    .unwrap_or_default();
                builds.push(BuildStep::Cargo(CargoBuildStep {
                    target_triple: target_triple.clone(),
                    package: CargoTargetPackages::Workspace,
                    features,
                    rustflags,
                    profile: String::from(PROFILE_DIST),
                    expected_binaries: binaries,
                    working_dir: working_dir.clone(),
                }));
            }
        }
        Ok(builds)
    }
}

/// Generate a `cargo build` command
pub fn make_build_cargo_target_command(
    host: &Triple,
    cargo_cmd: &str,
    rustflags: &str,
    step: &CargoBuildStep,
    auditable: bool,
) -> DistResult<Cmd> {
    let target: Triple = step.target_triple.parse()?;

    eprint!("building {target} target");
    let wrapper = build_wrapper_for_cross(host, &target)?;
    if &target != host {
        eprint!(", from {host} host");
        if let Some(wrapper) = wrapper.as_ref() {
            eprint!(", via {wrapper}");
        }
    }
    eprint!(", using cargo profile {}", step.profile);

    let mut command = Cmd::new(cargo_cmd, "build your app with Cargo");
    if auditable {
        command.arg("auditable");
    }
    match wrapper {
        None => {
            command.arg("build");
        }
        Some(CargoBuildWrapper::ZigBuild) => {
            command.arg("zigbuild");
        }
        Some(CargoBuildWrapper::Xwin) => {
            // cf. <https://github.com/rust-cross/cargo-xwin?tab=readme-ov-file#customization>
            let arch = match target.architecture {
                Architecture::X86_32(_) => "x86",
                Architecture::X86_64 => "x86_64",
                Architecture::Aarch64(_) => "aarch64",
                _ => panic!("cargo-xwin doesn't support {target} because of its architecture",),
            };
            command.env("XWIN_ARCH", arch);
            command.arg("xwin").arg("build");
        }
    }
    command
        .arg("--profile")
        .arg(&step.profile)
        .arg("--message-format=json-render-diagnostics")
        .arg("--target")
        .arg(step.target_triple.as_str())
        .env("RUSTFLAGS", rustflags)
        .current_dir(&step.working_dir)
        .stdout(std::process::Stdio::piped());
    if !step.features.default_features {
        command.arg("--no-default-features");
    }
    match &step.features.features {
        CargoTargetFeatureList::All => {
            command.arg("--all-features");
        }
        CargoTargetFeatureList::List(features) => {
            if !features.is_empty() {
                // The way we pass these, Cargo wants us to use --features
                // once for each arg, idk why exactly (might be a windows quirk).
                for feature in features {
                    command.arg("--features");
                    command.arg(feature);
                }
            }
        }
    }
    match &step.package {
        CargoTargetPackages::Workspace => {
            command.arg("--workspace");
            eprintln!(" --workspace)");
        }
        CargoTargetPackages::Package(package) => {
            command.arg("--package").arg(package);
            eprintln!(" --package={})", package);
        }
    }

    Ok(command)
}

/// Build a cargo target
pub fn build_cargo_target(
    dist_graph: &DistGraph,
    manifest: &mut DistManifest,
    step: &CargoBuildStep,
) -> DistResult<()> {
    let cargo = dist_graph.tools.cargo()?;

    let mut rustflags = step.rustflags.clone();
    let mut desired_extra_env = vec![];
    let skip_brewfile = env::var("DO_NOT_USE_BREWFILE").is_ok();
    if !skip_brewfile {
        if let Some(env_output) = fetch_brew_env(dist_graph, &step.working_dir)? {
            let brew_env = parse_env(&env_output)?;
            desired_extra_env = select_brew_env(&brew_env);
            rustflags = determine_brew_rustflags(&rustflags, &brew_env);
        }
    }

    let auditable = dist_graph.config.builds.cargo.cargo_auditable;
    let host = cargo_dist_schema::target_lexicon::HOST;
    let mut command =
        make_build_cargo_target_command(&host, &cargo.cmd, &rustflags, step, auditable)?;

    // If we generated any extra environment variables to
    // inject into the environment, apply them now.
    command.envs(desired_extra_env);
    let mut task = command.spawn()?;

    let mut expected = BuildExpectations::new(dist_graph, &step.expected_binaries);

    // Collect up the compiler messages to find out where binaries ended up
    let reader = std::io::BufReader::new(task.stdout.take().unwrap());
    for message in cargo_metadata::Message::parse_stream(reader) {
        let Ok(message) = message
            .into_diagnostic()
            .wrap_err("failed to parse cargo json message")
            .map_err(|e| warn!("{:?}", e))
        else {
            // It's ok for there to be messages we don't understand if we don't care about them.
            // At the end we'll check if we got the messages we *do* need.
            continue;
        };
        match message {
            cargo_metadata::Message::CompilerArtifact(artifact) => {
                // Hey we got some files, record that fact
                expected.found_bins(artifact.package_id.to_string(), artifact.filenames);
            }
            _ => {
                // Nothing else interesting?
            }
        }
    }

    // Process all the resulting binaries
    expected.process_bins(dist_graph, manifest)?;

    Ok(())
}

/// Run rustup to setup a cargo target
pub fn rustup_toolchain(dist_graph: &DistGraph, cmd: &RustupStep) -> DistResult<()> {
    eprintln!("running rustup to ensure you have {} installed", cmd.target);
    Cmd::new(&cmd.rustup.cmd, "install rustup toolchain")
        .arg("target")
        .arg("add")
        .arg(cmd.target.as_str())
        .current_dir(&dist_graph.workspace_dir)
        .run()?;
    Ok(())
}

/// Similar to the above, we read Homebrew's recursive dependency tree and
/// then append link flags to dist's rustflags.
/// These ensure that Rust can find C libraries that may exist within
/// each package's prefix.
fn determine_brew_rustflags(base_rustflags: &str, environment: &SortedMap<&str, &str>) -> String {
    format!("{base_rustflags} {}", calculate_ldflags(environment))
}

#[cfg(test)]
mod tests {

    use super::make_build_cargo_target_command;
    use crate::platform::targets;
    use crate::tasks::{CargoTargetFeatureList, CargoTargetFeatures, CargoTargetPackages};
    use crate::CargoBuildStep;

    #[test]
    fn build_command_not_auditable() {
        let cargo_cmd = "cargo".to_string();
        let rustflags = "--some-rust-flag".to_string();
        let auditable = false;

        let features = CargoTargetFeatures {
            default_features: true,
            features: CargoTargetFeatureList::default(),
        };
        let step = CargoBuildStep {
            target_triple: targets::TARGET_X64_LINUX_GNU.to_owned(),
            features,
            package: CargoTargetPackages::Workspace,
            profile: "release".to_string(),
            rustflags: "--this-rust-flag-gets-ignored".to_string(),
            expected_binaries: vec![],
            working_dir: ".".to_string().into(), // this feels mildly cursed -duckinator.
        };

        let cmd = make_build_cargo_target_command(
            &step.target_triple.parse().unwrap(),
            &cargo_cmd,
            &rustflags,
            &step,
            auditable,
        )
        .unwrap();

        let mut args = cmd.inner.get_args();

        let arg1 = args.next().unwrap().to_str().unwrap();
        assert_eq!(arg1, "build");
    }

    #[test]
    fn build_command_auditable() {
        let cargo_cmd = "cargo".to_string();
        let rustflags = "--some-rust-flag".to_string();
        let auditable = true;

        let features = CargoTargetFeatures {
            default_features: true,
            features: CargoTargetFeatureList::default(),
        };
        let step = CargoBuildStep {
            target_triple: targets::TARGET_X64_LINUX_GNU.to_owned(),
            features,
            package: CargoTargetPackages::Workspace,
            profile: "release".to_string(),
            rustflags: "--this-rust-flag-gets-ignored".to_string(),
            expected_binaries: vec![],
            working_dir: ".".to_string().into(), // this feels mildly cursed -duckinator.
        };

        let cmd = make_build_cargo_target_command(
            &step.target_triple.parse().unwrap(),
            &cargo_cmd,
            &rustflags,
            &step,
            auditable,
        )
        .unwrap();

        let mut args = cmd.inner.get_args();

        let arg1 = args.next().unwrap().to_str().unwrap();
        assert_eq!(arg1, "auditable");

        let arg2 = args.next().unwrap().to_str().unwrap();
        assert_eq!(arg2, "build");
    }
}
