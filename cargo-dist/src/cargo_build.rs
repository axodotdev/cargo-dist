//! Functionality required to invoke `cargo build` properly

use std::env;
use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};
use miette::{miette, Context, IntoDiagnostic};
use tracing::{info, warn};

use crate::{
    copy_file, CargoBuildStep, CargoTargetFeatureList, CargoTargetPackages, DistGraph, FastMap,
    RustupStep, SortedMap,
};
use crate::{errors::*, BinaryIdx, BuildStep, DistGraphBuilder, TargetTriple, PROFILE_DIST};

impl<'a> DistGraphBuilder<'a> {
    pub(crate) fn compute_cargo_builds(&mut self) -> Vec<BuildStep> {
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
            if self.inner.msvc_crt_static && target.contains("windows-msvc") {
                rustflags.push_str(" -Ctarget-feature=+crt-static");
            }

            // Likewise, the default for musl will change in the future, so
            // we can future-proof this by adding the flag now
            // See: https://github.com/axodotdev/cargo-dist/issues/486
            if target.ends_with("linux-musl") {
                rustflags.push_str(" -Ctarget-feature=+crt-static -Clink-self-contained=yes");
            }

            // If we're trying to cross-compile on macOS, ensure the rustup toolchain
            // is setup!
            if target.ends_with("apple-darwin")
                && self.inner.tools.cargo.host_target.ends_with("apple-darwin")
                && target != self.inner.tools.cargo.host_target
            {
                if let Some(rustup) = self.inner.tools.rustup.clone() {
                    builds.push(BuildStep::Rustup(RustupStep {
                        rustup,
                        target: target.clone(),
                    }));
                } else {
                    warn!("You're trying to cross-compile on macOS, but I can't find rustup to ensure you have the rust toolchains for it!")
                }
            }

            if target.ends_with("linux-musl")
                && self.inner.tools.cargo.host_target.ends_with("linux-gnu")
            {
                if let Some(rustup) = self.inner.tools.rustup.clone() {
                    builds.push(BuildStep::Rustup(RustupStep {
                        rustup,
                        target: target.clone(),
                    }));
                } else {
                    warn!("You're trying to cross-compile for musl from glibc, but I can't find rustup to ensure you have the rust toolchains for it!")
                }
            }

            if self.inner.precise_builds {
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
                        target_triple: target.clone(),
                        package: CargoTargetPackages::Package(pkg_spec),
                        features,
                        rustflags: rustflags.clone(),
                        profile: String::from(PROFILE_DIST),
                        expected_binaries,
                    }));
                }
            } else {
                // If we think a workspace build is possible, every binary agrees on the features, so take an arbitrary one
                let features = binaries
                    .first()
                    .map(|&idx| self.binary(idx).features.clone())
                    .unwrap_or_default();
                builds.push(BuildStep::Cargo(CargoBuildStep {
                    target_triple: target.clone(),
                    package: CargoTargetPackages::Workspace,
                    features,
                    rustflags,
                    profile: String::from(PROFILE_DIST),
                    expected_binaries: binaries,
                }));
            }
        }
        builds
    }
}

/// Build a cargo target
pub fn build_cargo_target(dist_graph: &DistGraph, target: &CargoBuildStep) -> Result<()> {
    eprint!(
        "building cargo target ({}/{}",
        target.target_triple, target.profile
    );

    let mut rustflags = target.rustflags.clone();
    let mut desired_extra_env = vec![];
    let skip_brewfile = env::var("DO_NOT_USE_BREWFILE").is_ok();
    if let Some(brew) = &dist_graph.tools.brew {
        if Utf8Path::new("Brewfile").exists() && !skip_brewfile {
            // Uses `brew bundle exec` to just print its own environment,
            // allowing us to capture what it generated and decide what
            // to do with it.
            let result = Command::new(&brew.cmd)
                .arg("bundle")
                .arg("exec")
                .arg("--")
                .arg("/usr/bin/env")
                .output()
                .into_diagnostic()
                .wrap_err_with(|| "failed to exec brew bundle exec".to_string())?;

            let env_output = String::from_utf8_lossy(&result.stdout).to_string();

            let brew_env = parse_env(&env_output)?;
            desired_extra_env = select_brew_env(&brew_env);
            rustflags = determine_brew_rustflags(&rustflags, &brew_env);
        }
    }

    let mut command = Command::new(&dist_graph.tools.cargo.cmd);
    command
        .arg("build")
        .arg("--profile")
        .arg(&target.profile)
        .arg("--message-format=json-render-diagnostics")
        .arg("--target")
        .arg(&target.target_triple)
        .env("RUSTFLAGS", &rustflags)
        .stdout(std::process::Stdio::piped());
    if !target.features.default_features {
        command.arg("--no-default-features");
    }
    match &target.features.features {
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
    match &target.package {
        CargoTargetPackages::Workspace => {
            command.arg("--workspace");
            eprintln!(" --workspace)");
        }
        CargoTargetPackages::Package(package) => {
            command.arg("--package").arg(package);
            eprintln!(" --package={})", package);
        }
    }
    // If we generated any extra environment variables to
    // inject into the environment, apply them now.
    command.envs(desired_extra_env);
    info!("exec: {:?}", command);
    let mut task = command
        .spawn()
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to exec cargo build: {command:?}"))?;

    // Create entries for all the binaries we expect to find, and the paths they should
    // be copied to (according to the copy_exe_to subscribers list).
    //
    // Structure is:
    //
    // package-id (key)
    //    binary-name (key)
    //       subscribers (list)
    //          src-path (initially blank, must be filled in by rustc)
    //          dest-path (where to copy the file to)
    let mut expected_exes =
        FastMap::<String, FastMap<String, Vec<(Utf8PathBuf, Utf8PathBuf)>>>::new();
    let mut expected_symbols =
        FastMap::<String, FastMap<String, Vec<(Utf8PathBuf, Utf8PathBuf)>>>::new();
    for &binary_idx in &target.expected_binaries {
        let binary = &dist_graph.binary(binary_idx);
        let package_id = binary
            .pkg_id
            .clone()
            .expect("pkg_id is mandatory for cargo builds")
            .to_string();
        let exe_name = binary.name.clone();
        for exe_dest in &binary.copy_exe_to {
            expected_exes
                .entry(package_id.clone())
                .or_default()
                .entry(exe_name.clone())
                .or_default()
                .push((Utf8PathBuf::new(), exe_dest.clone()));
        }
        for sym_dest in &binary.copy_symbols_to {
            expected_symbols
                .entry(package_id.clone())
                .or_default()
                .entry(exe_name.clone())
                .or_default()
                .push((Utf8PathBuf::new(), sym_dest.clone()));
        }
    }

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
                // Hey we got an executable, is it one we wanted?
                if let Some(new_exe) = artifact.executable {
                    info!("got a new exe: {}", new_exe);
                    let package_id = artifact.package_id.to_string();
                    let exe_name = new_exe.file_stem().unwrap();

                    // If we expected some symbols, pull them out of the paths of this executable
                    let expected_sym = expected_symbols
                        .get_mut(&package_id)
                        .and_then(|m| m.get_mut(exe_name));
                    if let Some(expected) = expected_sym {
                        for (src_sym_path, _) in expected {
                            for path in &artifact.filenames {
                                // FIXME: unhardcode this when we add support for other symbol kinds!
                                let is_symbols =
                                    path.extension().map(|e| e == "pdb").unwrap_or(false);
                                if is_symbols {
                                    // These are symbols we expected! Save the path.
                                    *src_sym_path = path.to_owned();
                                }
                            }
                        }
                    }

                    // Get the exe path
                    let expected_exe = expected_exes
                        .get_mut(&package_id)
                        .and_then(|m| m.get_mut(exe_name));
                    if let Some(expected) = expected_exe {
                        for (src_bin_path, _) in expected {
                            // This is an exe we expected! Save the path.
                            *src_bin_path = new_exe.clone();
                        }
                    }
                }
            }
            _ => {
                // Nothing else interesting?
            }
        }
    }

    // Check that we got everything we expected, and normalize to ArtifactIdx => Artifact Path
    for (package_id, exes) in expected_exes {
        for (exe_name, to_copy) in &exes {
            for (src_path, dest_path) in to_copy {
                if src_path.as_str().is_empty() {
                    return Err(miette!(
                        "failed to find bin {} ({}) -- did the cargo build above have errors?",
                        exe_name,
                        package_id
                    ));
                }
                copy_file(src_path, dest_path)?;
            }
        }
    }
    for (package_id, symbols) in expected_symbols {
        for (exe, to_copy) in &symbols {
            for (src_path, dest_path) in to_copy {
                if src_path.as_str().is_empty() {
                    return Err(miette!(
                        "failed to find symbols for bin {} ({}) -- did the cargo build above have errors?",
                        exe,
                        package_id
                    ));
                }
                copy_file(src_path, dest_path)?;
            }
        }
    }

    Ok(())
}

/// Build a cargo target
pub fn rustup_toolchain(_dist_graph: &DistGraph, cmd: &RustupStep) -> Result<()> {
    eprintln!("running rustup to ensure you have {} installed", cmd.target);
    let status = Command::new(&cmd.rustup.cmd)
        .arg("target")
        .arg("add")
        .arg(&cmd.target)
        .status()
        .into_diagnostic()
        .wrap_err("Failed to install rustup toolchain")?;

    if !status.success() {
        return Err(miette!("Failed to install rustup toolchain"));
    }
    Ok(())
}

// Takes a string in KEY=value environment variable format and
// parses it into a BTreeMap. The string syntax is sh-compatible, and also the
// format returned by `env`.
// Note that we trust the parsed string to contain a given key only once;
// if specified more than once, only the final occurrence will be included.
fn parse_env(env_string: &str) -> DistResult<SortedMap<&str, &str>> {
    let mut parsed = SortedMap::new();
    for line in env_string.trim_end().split('\n') {
        let Some((key, value)) = line.split_once('=') else {
            return Err(DistError::EnvParseError {
                line: line.to_owned(),
            });
        };
        parsed.insert(key, value);
    }

    Ok(parsed)
}

/// Given the environment captured from `brew bundle exec -- env`, returns
/// a list of all dependencies from that environment and the opt prefixes
/// to those packages.
fn formulas_from_env(environment: &SortedMap<&str, &str>) -> Vec<(String, String)> {
    let mut packages = vec![];

    // Set by Homebrew/brew bundle - a comma-separated list of all
    // dependencies in the recursive tree calculated from the dependencies
    // in the Brewfile.
    if let Some(formulastring) = environment.get("HOMEBREW_DEPENDENCIES") {
        // Set by Homebrew/brew bundle - the path to Homebrew's "opt"
        // directory, which is where links to the private cellar of every
        // installed package lives.
        // Usually /opt/homebrew/opt or /usr/local/opt.
        if let Some(opt_prefix) = environment.get("HOMEBREW_OPT") {
            for dep in formulastring.split(',') {
                // Unwrap here is safe because `split` will always return
                // a collection of at least one item.
                let short_name = dep.split('/').last().unwrap();
                let pkg_opt = format!("{opt_prefix}/{short_name}");
                packages.push((dep.to_owned(), pkg_opt));
            }
        }
    }

    packages
}

/// Takes a BTreeMap of key/value environment variables produced by
/// `brew bundle exec` and decides which ones we want to keep for our own builds.
/// Returns a Vec containing (KEY, value) tuples.
fn select_brew_env(environment: &SortedMap<&str, &str>) -> Vec<(String, String)> {
    let mut desired_env = vec![];

    // Several of Homebrew's environment variables are safe for us to use
    // unconditionally, so pick those in their entirety.
    if let Some(value) = environment.get("PKG_CONFIG_PATH") {
        desired_env.push(("PKG_CONFIG_PATH".to_owned(), value.to_string()))
    }
    if let Some(value) = environment.get("PKG_CONFIG_LIBDIR") {
        desired_env.push(("PKG_CONFIG_LIBDIR".to_owned(), value.to_string()))
    }
    if let Some(value) = environment.get("CMAKE_INCLUDE_PATH") {
        desired_env.push(("CMAKE_INCLUDE_PATH".to_owned(), value.to_string()))
    }
    if let Some(value) = environment.get("CMAKE_LIBRARY_PATH") {
        desired_env.push(("CMAKE_LIBRARY_PATH".to_owned(), value.to_string()))
    }
    let mut paths = vec![];

    // For each listed dependency, add it to the PATH
    for (_, pkg_opt) in formulas_from_env(environment) {
        // Not every package will have a /bin or /sbin directory,
        // but it's safe to add both to the PATH just in case.
        paths.push(format!("{pkg_opt}/bin"));
        paths.push(format!("{pkg_opt}/sbin"));
    }

    if !paths.is_empty() {
        let our_path = env!("PATH");
        let desired_path = format!("{our_path}:{}", paths.join(":"));

        desired_env.insert(0, ("PATH".to_owned(), desired_path));
    }

    desired_env
}

/// Similar to the above, we read Homebrew's recursive dependency tree and
/// then append link flags to cargo-dist's rustflags.
/// These ensure that Rust can find C libraries that may exist within
/// each package's prefix.
fn determine_brew_rustflags(base_rustflags: &str, environment: &SortedMap<&str, &str>) -> String {
    let mut rustflags = base_rustflags.to_owned();
    // For each listed dependency, add it to CFLAGS/LDFLAGS
    for (_, pkg_opt) in formulas_from_env(environment) {
        // Note that this path might not actually exist; not every
        // package contains libraries. However, it's safe to
        // append this flag anyway; Rust passes it on to the
        // compiler/linker, which tolerate missing directories
        // just fine.
        rustflags = format!("{rustflags} -L{pkg_opt}/lib");
    }

    rustflags
}
