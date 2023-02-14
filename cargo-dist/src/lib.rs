#![deny(missing_docs)]
#![allow(clippy::single_match)]

//! # cargo-dist
//!
//!

use std::{collections::HashMap, fs::File, io::BufReader, ops::Not, process::Command};

use camino::{Utf8Path, Utf8PathBuf};
use cargo_dist_schema::{Asset, AssetKind, DistManifest, ExecutableAsset, Release};
use flate2::{write::ZlibEncoder, Compression, GzBuilder};
use tracing::{info, warn};
use xz2::write::XzEncoder;
use zip::ZipWriter;

use errors::*;
use miette::{miette, Context, IntoDiagnostic};
pub use tasks::*;

pub mod ci;
pub mod errors;
pub mod installer;
pub mod tasks;
#[cfg(test)]
mod tests;

/// Top level command of cargo_dist -- do everything!
pub fn do_dist(cfg: &Config) -> Result<DistManifest> {
    let dist = tasks::gather_work(cfg)?;
    if !dist.is_init {
        return Err(miette!(
            "please run 'cargo dist init' before running any other commands!"
        ));
    }

    // FIXME: parallelize this by working this like a dependency graph, so we can start
    // bundling up an executable the moment it's built! Note however that you shouldn't
    // parallelize Cargo invocations because it has global state that can get clobbered.
    // Most problematically if you do two builds with different feature flags the final
    // binaries will get copied to the same location and clobber eachother :(

    // First set up our target dirs so things don't have to race to do it later
    if !dist.dist_dir.exists() {
        std::fs::create_dir_all(&dist.dist_dir)
            .into_diagnostic()
            .wrap_err_with(|| format!("couldn't create dist target dir at {}", dist.dist_dir))?;
    }

    for artifact in &dist.artifacts {
        eprintln!("bundling {}", artifact.id);
        init_artifact_dir(&dist, artifact)?;
    }

    // Run all the build steps
    for step in &dist.build_steps {
        run_build_step(&dist, step)?;
    }

    for artifact in &dist.artifacts {
        eprintln!("bundled: {}", artifact.file_path);
    }

    Ok(build_manifest(cfg, &dist))
}

/// Just generate the manifest produced by `cargo dist build` without building
pub fn do_manifest(cfg: &Config) -> Result<DistManifest> {
    let dist = gather_work(cfg)?;
    if !dist.is_init {
        return Err(miette!(
            "please run 'cargo dist init' before running any other commands!"
        ));
    }

    Ok(build_manifest(cfg, &dist))
}

fn build_manifest(cfg: &Config, dist: &DistGraph) -> DistManifest {
    // Report the releases
    let mut releases = vec![];
    for release in &dist.releases {
        // Gather up all the local and global artifacts
        let mut artifacts = vec![];
        for &artifact_idx in &release.global_artifacts {
            artifacts.push(manifest_artifact(cfg, dist, artifact_idx));
        }
        for &variant_idx in &release.variants {
            let variant = dist.variant(variant_idx);
            for &artifact_idx in &variant.local_artifacts {
                artifacts.push(manifest_artifact(cfg, dist, artifact_idx));
            }
        }

        // And report the release
        releases.push(Release {
            app_name: release.app_name.clone(),
            app_version: release.version.to_string(),
            changelog_title: release.changelog_title.clone(),
            changelog_body: release.changelog_body.clone(),
            artifacts,
        })
    }

    let mut manifest = DistManifest::new(releases);
    manifest.dist_version = Some(env!("CARGO_PKG_VERSION").to_owned());
    manifest.announcement_tag = dist.announcement_tag.clone();
    manifest
}

fn manifest_artifact(
    cfg: &Config,
    dist: &DistGraph,
    artifact_idx: ArtifactIdx,
) -> cargo_dist_schema::Artifact {
    let artifact = dist.artifact(artifact_idx);
    let mut assets = vec![];

    let built_assets = artifact
        .required_binaries
        .iter()
        .map(|(&binary_idx, exe_path)| {
            let binary = &dist.binary(binary_idx);
            let symbols_artifact = binary.symbols_artifact.map(|a| dist.artifact(a).id.clone());
            Asset {
                name: Some(binary.name.clone()),
                // Always copied to the root... for now
                path: Some(exe_path.file_name().unwrap().to_owned()),
                kind: AssetKind::Executable(ExecutableAsset { symbols_artifact }),
            }
        });

    let static_assets = match &artifact.kind {
        ArtifactKind::ExecutableZip(zip) => zip
            .static_assets
            .iter()
            .map(|(kind, asset)| {
                let kind = match kind {
                    StaticAssetKind::Changelog => AssetKind::Changelog,
                    StaticAssetKind::License => AssetKind::License,
                    StaticAssetKind::Readme => AssetKind::Readme,
                    StaticAssetKind::Other => AssetKind::Unknown,
                };
                Asset {
                    name: Some(asset.file_name().unwrap().to_owned()),
                    path: Some(asset.file_name().unwrap().to_owned()),
                    kind,
                }
            })
            .collect(),
        ArtifactKind::Installer(_) => vec![],
        ArtifactKind::Symbols(_) => vec![],
    };

    assets.extend(built_assets);
    assets.extend(static_assets);
    // Sort the assets by name to make things extra stable
    assets.sort_by(|k1, k2| k1.name.cmp(&k2.name));

    let install_hint;
    let description;
    let kind;

    match &artifact.kind {
        ArtifactKind::ExecutableZip(_) => {
            install_hint = None;
            description = None;
            kind = cargo_dist_schema::ArtifactKind::ExecutableZip;
        }
        ArtifactKind::Symbols(_) => {
            install_hint = None;
            description = None;
            kind = cargo_dist_schema::ArtifactKind::Symbols;
        }
        ArtifactKind::Installer(
            InstallerImpl::GithubPowershell(info) | InstallerImpl::GithubShell(info),
        ) => {
            install_hint = Some(info.hint.clone());
            description = Some(info.desc.clone());
            kind = cargo_dist_schema::ArtifactKind::Installer;
        }
    };

    cargo_dist_schema::Artifact {
        name: Some(artifact.id.clone()),
        path: if cfg.no_local_paths {
            None
        } else {
            Some(artifact.file_path.to_string())
        },
        target_triples: artifact.target_triples.clone(),
        install_hint,
        description,
        assets,
        kind,
    }
}

/// Run some build step
fn run_build_step(dist_graph: &DistGraph, target: &BuildStep) -> Result<()> {
    match target {
        BuildStep::Cargo(target) => build_cargo_target(dist_graph, target),
        BuildStep::CopyFile(CopyFileStep {
            src_path,
            dest_path,
        }) => copy_file(src_path, dest_path),
        BuildStep::CopyDir(CopyDirStep {
            src_path,
            dest_path,
        }) => copy_dir(src_path, dest_path),
        BuildStep::Zip(ZipDirStep {
            src_path,
            dest_path,
            zip_style,
        }) => zip_dir(src_path, dest_path, zip_style),
        BuildStep::GenerateInstaller(installer) => generate_installer(dist_graph, installer),
    }
}

/// Build a cargo target
fn build_cargo_target(dist_graph: &DistGraph, target: &CargoBuildStep) -> Result<()> {
    eprintln!(
        "building cargo target ({}/{})",
        target.target_triple, target.profile
    );

    let mut command = Command::new(&dist_graph.cargo);
    command
        .arg("build")
        .arg("--profile")
        .arg(&target.profile)
        .arg("--message-format=json")
        .arg("--target")
        .arg(&target.target_triple)
        .env("RUSTFLAGS", &target.rustflags)
        .stdout(std::process::Stdio::piped());
    if target.features.no_default_features {
        command.arg("--no-default-features");
    }
    match &target.features.features {
        CargoTargetFeatureList::All => {
            command.arg("--all-features");
        }
        CargoTargetFeatureList::List(features) => {
            if !features.is_empty() {
                command.arg("--features");
                for feature in features {
                    command.arg(feature);
                }
            }
        }
    }
    match &target.package {
        CargoTargetPackages::Workspace => {
            command.arg("--workspace");
        }
        CargoTargetPackages::Package(package) => {
            command.arg("--package").arg(package.to_string());
        }
    }
    info!("exec: {:?}", command);
    let mut task = command
        .spawn()
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to exec cargo build: {command:?}"))?;

    // Create entries for all the binaries we expect to find with empty paths
    // we'll fail if any are still empty at the end!
    let mut expected_exes = HashMap::<String, HashMap<String, (Utf8PathBuf, Utf8PathBuf)>>::new();
    let mut expected_symbols =
        HashMap::<String, HashMap<String, (Utf8PathBuf, Utf8PathBuf)>>::new();
    for &binary_idx in &target.expected_binaries {
        let binary = &dist_graph.binary(binary_idx);
        let package_id = binary.pkg_id.to_string();
        let exe_name = binary.name.clone();
        for exe_dest in &binary.copy_exe_to {
            expected_exes
                .entry(package_id.clone())
                .or_default()
                .insert(exe_name.clone(), (Utf8PathBuf::new(), exe_dest.clone()));
        }
        for sym_dest in &binary.copy_symbols_to {
            expected_symbols
                .entry(package_id.clone())
                .or_default()
                .insert(exe_name.clone(), (Utf8PathBuf::new(), sym_dest.clone()));
        }
    }

    // Collect up the compiler messages to find out where binaries ended up
    let reader = std::io::BufReader::new(task.stdout.take().unwrap());
    for message in cargo_metadata::Message::parse_stream(reader) {
        let Ok(message) = message.into_diagnostic().wrap_err("failed to parse cargo json message").map_err(|e| warn!("{:?}", e)) else {
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
                    if let Some((src_sym_path, _)) = expected_sym {
                        for path in artifact.filenames {
                            // FIXME: unhardcode this when we add support for other symbol kinds!
                            let is_symbols = path.extension().map(|e| e == "pdb").unwrap_or(false);
                            if is_symbols {
                                // These are symbols we expected! Save the path.
                                *src_sym_path = path;
                            }
                        }
                    }

                    // Get the exe path
                    let expected_exe = expected_exes
                        .get_mut(&package_id)
                        .and_then(|m| m.get_mut(exe_name));
                    if let Some(expected) = expected_exe {
                        // This is an exe we expected! Save the path.
                        expected.0 = new_exe;
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
        for (exe_name, (src_path, dest_path)) in &exes {
            if src_path.as_str().is_empty() {
                return Err(miette!("failed to find bin {} ({})", exe_name, package_id));
            }
            copy_file(src_path, dest_path)?;
        }
    }
    for (package_id, symbols) in expected_symbols {
        for (exe, (src_path, dest_path)) in &symbols {
            if src_path.as_str().is_empty() {
                return Err(miette!(
                    "failed to find symbols for bin {} ({})",
                    exe,
                    package_id
                ));
            }
            copy_file(src_path, dest_path)?;
        }
    }

    Ok(())
}

/// Initialize the dir for an artifact (and delete the old artifact file).
fn init_artifact_dir(_dist: &DistGraph, artifact: &Artifact) -> Result<()> {
    // Delete any existing bundle
    if artifact.file_path.exists() {
        std::fs::remove_file(&artifact.file_path)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to delete old artifact {}", artifact.file_path))?;
    }

    let Some(artifact_dir_path) = &artifact.dir_path else {
        // If there's no dir than we're done
        return Ok(());
    };
    info!("recreating artifact dir: {artifact_dir_path}");

    // Clear out the dir we'll build the bundle up in
    if artifact_dir_path.exists() {
        std::fs::remove_dir_all(artifact_dir_path)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to delete old artifact dir {artifact_dir_path}"))?;
    }
    std::fs::create_dir(artifact_dir_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to create artifact dir {artifact_dir_path}"))?;

    Ok(())
}

fn copy_file(src_path: &Utf8Path, dest_path: &Utf8Path) -> Result<()> {
    let _bytes_written = std::fs::copy(src_path, dest_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to copy file {src_path} => {dest_path}"))?;
    Ok(())
}

fn copy_dir(_src_path: &Utf8Path, _dest_path: &Utf8Path) -> Result<()> {
    todo!("copy_dir isn't implemented yet")
}

fn zip_dir(src_path: &Utf8Path, dest_path: &Utf8Path, zip_style: &ZipStyle) -> Result<()> {
    match zip_style {
        ZipStyle::Zip => really_zip_dir(src_path, dest_path),
        ZipStyle::Tar(compression) => tar_dir(src_path, dest_path, compression),
    }
}

fn tar_dir(src_path: &Utf8Path, dest_path: &Utf8Path, compression: &CompressionImpl) -> Result<()> {
    // Set up the archive/compression
    // The contents of the zip (e.g. a tar)
    let dir_name = src_path.file_name().unwrap();
    let zip_contents_name = format!("{dir_name}.tar");
    let final_zip_file = File::create(dest_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to create file for artifact: {dest_path}"))?;

    match compression {
        CompressionImpl::Gzip => {
            // Wrap our file in compression
            let zip_output = GzBuilder::new()
                .filename(zip_contents_name)
                .write(final_zip_file, Compression::default());

            // Write the tar to the compression stream
            let mut tar = tar::Builder::new(zip_output);

            // Add the whole dir to the tar
            tar.append_dir_all(dir_name, src_path)
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!("failed to copy directory into tar: {src_path} => {dir_name}",)
                })?;
            // Finish up the tarring
            let zip_output = tar
                .into_inner()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write tar: {dest_path}"))?;
            // Finish up the compression
            let _zip_file = zip_output
                .finish()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write archive: {dest_path}"))?;
            // Drop the file to close it
        }
        CompressionImpl::Xzip => {
            let zip_output = XzEncoder::new(final_zip_file, 9);
            // Write the tar to the compression stream
            let mut tar = tar::Builder::new(zip_output);

            // Add the whole dir to the tar
            tar.append_dir_all(dir_name, src_path)
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!("failed to copy directory into tar: {src_path} => {dir_name}",)
                })?;
            // Finish up the tarring
            let zip_output = tar
                .into_inner()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write tar: {dest_path}"))?;
            // Finish up the compression
            let _zip_file = zip_output
                .finish()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write archive: {dest_path}"))?;
            // Drop the file to close it
        }
        CompressionImpl::Zstd => {
            // Wrap our file in compression
            let zip_output = ZlibEncoder::new(final_zip_file, Compression::default());

            // Write the tar to the compression stream
            let mut tar = tar::Builder::new(zip_output);

            // Add the whole dir to the tar
            tar.append_dir_all(dir_name, src_path)
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!("failed to copy directory into tar: {src_path} => {dir_name}",)
                })?;
            // Finish up the tarring
            let zip_output = tar
                .into_inner()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write tar: {dest_path}"))?;
            // Finish up the compression
            let _zip_file = zip_output
                .finish()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write archive: {dest_path}"))?;
            // Drop the file to close it
        }
    }

    info!("artifact created at: {}", dest_path);
    Ok(())
}

fn really_zip_dir(src_path: &Utf8Path, dest_path: &Utf8Path) -> Result<()> {
    // Set up the archive/compression
    let final_zip_file = File::create(dest_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to create file for artifact: {dest_path}"))?;

    // Wrap our file in compression
    let mut zip = ZipWriter::new(final_zip_file);

    let dir = std::fs::read_dir(src_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to read artifact dir: {src_path}"))?;
    for entry in dir {
        let entry = entry.into_diagnostic()?;
        if entry.file_type().into_diagnostic()?.is_file() {
            let options = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            let file = File::open(entry.path()).into_diagnostic()?;
            let mut buf = BufReader::new(file);
            let file_name = entry.file_name();
            // FIXME: ...don't do this lossy conversion?
            let utf8_file_name = file_name.to_string_lossy();
            zip.start_file(utf8_file_name.clone(), options)
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!("failed to create file {utf8_file_name} in zip: {dest_path}")
                })?;
            std::io::copy(&mut buf, &mut zip).into_diagnostic()?;
        } else {
            todo!("implement zip subdirs! (or was this a symlink?)");
        }
    }

    // Finish up the compression
    let _zip_file = zip
        .finish()
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to write archive: {dest_path}"))?;
    // Drop the file to close it
    info!("artifact created at: {}", dest_path);
    Ok(())
}

/// Arguments for `cargo dist init` ([`do_init`][])
#[derive(Debug)]
pub struct InitArgs {}

/// Run 'cargo dist init'
pub fn do_init(cfg: &Config, _args: &InitArgs) -> Result<()> {
    let cargo = tasks::cargo()?;
    let pkg_graph = tasks::package_graph(&cargo)?;
    let workspace = tasks::workspace_info(&pkg_graph)?;

    // Load in the workspace toml to edit and write back
    let mut workspace_toml = tasks::load_root_cargo_toml(&workspace.manifest_path)?;

    // Init things
    let mut did_anything = false;
    if init_dist_profile(cfg, &mut workspace_toml)? {
        eprintln!("added [profile.dist] to your root Cargo.toml");
        did_anything = true;
    } else {
        eprintln!("[profile.dist] already exists, nothing to do");
    }
    if init_dist_metadata(cfg, &mut workspace_toml)? {
        eprintln!("added [workspace.metadata.dist] to your root Cargo.toml");
        did_anything = true;
    } else {
        eprintln!("[workspace.metadata.dist] already exists, nothing to do");
    }

    if did_anything {
        use std::io::Write;
        let mut workspace_toml_file = File::options()
            .write(true)
            .open(&workspace.manifest_path)
            .into_diagnostic()
            .wrap_err("couldn't load root workspace Cargo.toml")?;
        write!(&mut workspace_toml_file, "{workspace_toml}")
            .into_diagnostic()
            .wrap_err("failed to write to Cargo.toml")?;
    }
    if !cfg.ci.is_empty() {
        let ci_args = GenerateCiArgs {};
        do_generate_ci(cfg, &ci_args)?;
    }
    Ok(())
}

fn init_dist_profile(_cfg: &Config, workspace_toml: &mut toml_edit::Document) -> Result<bool> {
    let profiles = workspace_toml["profile"].or_insert(toml_edit::table());
    if let Some(t) = profiles.as_table_mut() {
        t.set_implicit(true)
    }
    let dist_profile = &mut profiles[PROFILE_DIST];
    if !dist_profile.is_none() {
        return Ok(false);
    }
    let mut new_profile = toml_edit::table();
    {
        let new_profile = new_profile.as_table_mut().unwrap();
        // We're building for release, so this is a good base!
        new_profile.insert("inherits", toml_edit::value("release"));
        // We want *full* debuginfo for good crashreporting/profiling
        // This doesn't bloat the final binary because we use split-debuginfo below
        // new_profile.insert("debug", toml_edit::value(true));

        // Ensure that all debuginfo is pulled out of the binary and tossed
        // into a separate file from the final binary. This should ideally be
        // uploaded to something like a symbol server to be fetched on demand.
        // This is already the default on windows (.pdb) and macos (.dsym) but
        // is rather bleeding on other platforms (.dwp) -- it requires Rust 1.65,
        // which as of this writing in the latest stable rust release! If anyone
        // ever makes a big deal with building final binaries with an older MSRV
        // we may need to more intelligently select this.
        // new_profile.insert("split-debuginfo", toml_edit::value("packed"));

        // TODO: set codegen-units=1? (Probably Not!)
        //
        // Ok so there's an inherent tradeoff in compilers where if the compiler does
        // everything in a very serial/global way, it can discover more places where
        // optimizations can be done and theoretically make things faster/smaller
        // using all the information at its fingertips... at the cost of your builds
        // taking forever. Compiler optimizations generally take super-linear time,
        // so if you let the compiler see and think about EVERYTHING your builds
        // can literally take *days* for codebases on the order of LLVM itself.
        //
        // To keep compile times tractable, we generally break up the program
        // into "codegen units" (AKA "translation units") that get compiled
        // independently and then combined by the linker. This keeps the super-linear
        // scaling under control, but prevents optimizations like inlining across
        // units. (This process is why we have things like "object files" and "rlibs",
        // those are the intermediate artifacts fed to the linker.)
        //
        // Compared to C, Rust codegen units are quite monolithic. Where each C
        // *file* might gets its own codegen unit, Rust prefers scoping them to
        // an entire *crate*.  As it turns out, neither of these answers is right in
        // all cases, and being able to tune the unit size is useful.
        //
        // Large C++ codebases like Firefox have "unified" builds where they basically
        // concatenate files together to get bigger units. Rust provides the
        // opposite: the codegen-units=N option tells rustc that it should try to
        // break up a crate into at most N different units. This is done with some
        // heuristics and constraints to try to still get the most out of each unit
        // (i.e. try to keep functions that call eachother together for inlining).
        //
        // In the --release profile, codegen-units is set to 16, which attempts
        // to strike a balance between The Best Binaries and Ever Finishing Compiles.
        // In principle, tuning this down to 1 could be profitable, but LTO
        // (see the next TODO) does most of that work for us. As such we can probably
        // leave this alone to keep compile times reasonable.

        // TODO: set lto="thin" (or "fat")? (Probably "fat"!)
        //
        // LTO, Link Time Optimization, is basically hijacking the step where we
        // would link together everything and going back to the compiler (LLVM) to
        // do global optimizations across codegen-units (see the previous TODO).
        // Better Binaries, Slower Build Times.
        //
        // LTO can be "fat" (or "full") or "thin".
        //
        // Fat LTO is the "obvious" implementation: once you're done individually
        // optimizing the LLVM bitcode (IR) for each compilation unit, you concatenate
        // all the units and optimize it all together. Extremely serial, extremely
        // slow, but thorough as hell. For *enormous* codebases (millions of lines)
        // this can become intractably expensive and crash the compiler.
        //
        // Thin LTO is newer and more complicated: instead of unconditionally putting
        // everything together, we want to optimize each unit with other "useful" units
        // pulled in for inlining and other analysis. This grouping is done with
        // similar heuristics that rustc uses to break crates into codegen-units.
        // This is much faster than Fat LTO and can scale to arbitrarily big
        // codebases, but does produce slightly worse results.
        //
        // Release builds currently default to lto=false, which, despite the name,
        // actually still does LTO (lto="off" *really* turns it off)! Specifically it
        // does Thin LTO but *only* between the codegen units for a single crate.
        // This theoretically negates the disadvantages of codegen-units=16 while
        // still getting most of the advantages! Neat!
        //
        // Since most users will have codebases significantly smaller than An Entire
        // Browser, we can probably go all the way to default lto="fat", and they
        // can tune that down if it's problematic. If a user has "nightly" and "stable"
        // builds, it might be the case that they want lto="thin" for the nightlies
        // to keep them timely.
        //
        // > Aside: you may be wondering "why still have codegen units at all if using
        // > Fat LTO" and the best answer I can give you is "doing things in parallel
        // > at first lets you throw out a lot of junk and trim down the input before
        // > starting the really expensive super-linear global analysis, without losing
        // > too much of the important information". The less charitable answer is that
        // > compiler infra is built around codegen units and so this is a pragmatic hack.
        // >
        // > Thin LTO of course *really* benefits from still having codegen units.

        // TODO: set panic="abort"?
        //
        // PROBABLY NOT, but here's the discussion anyway!
        //
        // The default is panic="unwind", and things can be relying on unwinding
        // for correctness. Unwinding support bloats up the binary and can make
        // code run slower (because each place that *can* unwind is essentially
        // an early-return the compiler needs to be cautious of).
        //
        // panic="abort" immediately crashes the program if you panic,
        // but does still run the panic handler, so you *can* get things like
        // backtraces/crashreports out at that point.
        //
        // See RUSTFLAGS="-Cforce-unwind-tables" for the semi-orthogonal flag
        // that adjusts whether unwinding tables are emitted at all.
        //
        // Major C++ applications like Firefox already build with this flag,
        // the Rust ecosystem largely works fine with either.

        new_profile
            .decor_mut()
            .set_prefix("\n# The profile that 'cargo dist' will build with\n")
    }
    dist_profile.or_insert(new_profile);

    Ok(true)
}

/// Initialize [workspace.metadata.dist] with default values based on what was passed on the CLI
///
/// Returns whether the initialization was actually done
fn init_dist_metadata(cfg: &Config, workspace_toml: &mut toml_edit::Document) -> Result<bool> {
    use toml_edit::{value, Item};
    // Setup [workspace.metadata.dist]
    let workspace = workspace_toml["workspace"].or_insert(toml_edit::table());
    if let Some(t) = workspace.as_table_mut() {
        t.set_implicit(true)
    }
    let metadata = workspace["metadata"].or_insert(toml_edit::table());
    if let Some(t) = metadata.as_table_mut() {
        t.set_implicit(true)
    }
    let dist_metadata = &mut metadata[METADATA_DIST];
    if !dist_metadata.is_none() {
        return Ok(false);
    }

    // We "pointlessly" make this struct so you remember to consider what init should
    // do with any new config values!
    let meta = DistMetadata {
        // If they init with this version we're gonna try to stick to it!
        cargo_dist_version: Some(std::env!("CARGO_PKG_VERSION").parse().unwrap()),
        // latest stable release at this precise moment
        // maybe there's something more clever we can do here, but, *shrug*
        rust_toolchain_version: Some("1.67.1".to_owned()),
        ci: cfg.ci.clone(),
        installers: cfg
            .installers
            .is_empty()
            .not()
            .then(|| cfg.installers.clone()),
        targets: cfg.targets.is_empty().not().then(|| cfg.targets.clone()),
        dist: None,
        include: vec![],
        auto_includes: None,
    };

    const KEY_RUST_VERSION: &str = "rust-toolchain-version";
    const DESC_RUST_VERSION: &str =
        "# The preferred Rust toolchain to use in CI (rustup toolchain syntax)\n";

    const KEY_DIST_VERSION: &str = "cargo-dist-version";
    const DESC_DIST_VERSION: &str =
        "# The preferred cargo-dist version to use in CI (Cargo.toml SemVer syntax)\n";

    const KEY_CI: &str = "ci";
    const DESC_CI: &str = "# CI backends to support (see 'cargo dist generate-ci')\n";

    const KEY_INSTALLERS: &str = "installers";
    const DESC_INSTALLERS: &str = "# The installers to generate for each app\n";

    const KEY_TARGETS: &str = "targets";
    const DESC_TARGETS: &str = "# Target platforms to build apps for (Rust target-triple syntax)\n";

    const KEY_DIST: &str = "dist";
    const DESC_DIST: &str =
        "# Whether to consider the binaries in a package for distribution (defaults true)\n";

    const KEY_INCLUDE: &str = "include";
    const DESC_INCLUDE: &str =
        "# Extra static files to include in each App (path relative to this Cargo.toml's dir)\n";

    const KEY_AUTO_INCLUDE: &str = "auto-includes";
    const DESC_AUTO_INCLUDE: &str =
        "# Whether to auto-include files like READMEs, LICENSEs, and CHANGELOGs (default true)\n";

    let mut new_metadata = toml_edit::table();
    let table = new_metadata.as_table_mut().unwrap();
    if let Some(val) = meta.cargo_dist_version {
        table.insert(KEY_DIST_VERSION, value(val.to_string()));
        table
            .key_decor_mut(KEY_DIST_VERSION)
            .unwrap()
            .set_prefix(DESC_DIST_VERSION);
    }
    if let Some(val) = meta.rust_toolchain_version {
        table.insert(KEY_RUST_VERSION, value(val));
        table
            .key_decor_mut(KEY_RUST_VERSION)
            .unwrap()
            .set_prefix(DESC_RUST_VERSION);
    }
    if !meta.ci.is_empty() {
        table.insert(
            KEY_CI,
            Item::Value(
                meta.ci
                    .iter()
                    .map(|ci| match ci {
                        CiStyle::Github => "github",
                    })
                    .collect(),
            ),
        );
        table.key_decor_mut(KEY_CI).unwrap().set_prefix(DESC_CI);
    }
    if let Some(val) = meta.installers {
        table.insert(
            KEY_INSTALLERS,
            Item::Value(
                val.iter()
                    .map(|installer| match installer {
                        InstallerStyle::GithubPowershell => "github-powershell",
                        InstallerStyle::GithubShell => "github-shell",
                    })
                    .collect(),
            ),
        );
        table
            .key_decor_mut(KEY_INSTALLERS)
            .unwrap()
            .set_prefix(DESC_INSTALLERS);
    }
    if let Some(val) = meta.targets {
        table.insert(KEY_TARGETS, Item::Value(val.into_iter().collect()));
        table
            .key_decor_mut(KEY_TARGETS)
            .unwrap()
            .set_prefix(DESC_TARGETS);
    }
    if let Some(val) = meta.dist {
        table.insert(KEY_DIST, value(val));
        table.key_decor_mut(KEY_DIST).unwrap().set_prefix(DESC_DIST);
    }
    if !meta.include.is_empty() {
        table.insert(
            KEY_INCLUDE,
            Item::Value(meta.include.iter().map(ToString::to_string).collect()),
        );
        table
            .key_decor_mut(KEY_INCLUDE)
            .unwrap()
            .set_prefix(DESC_INCLUDE);
    }
    if let Some(val) = meta.auto_includes {
        table.insert(KEY_AUTO_INCLUDE, value(val));
        table
            .key_decor_mut(KEY_AUTO_INCLUDE)
            .unwrap()
            .set_prefix(DESC_AUTO_INCLUDE);
    }
    table
        .decor_mut()
        .set_prefix("\n# Config for 'cargo dist'\n");

    dist_metadata.or_insert(new_metadata);

    Ok(true)
}

/// Arguments for `cargo dist generate-ci` ([`do_generate_ci][])
#[derive(Debug)]
pub struct GenerateCiArgs {}

/// Generate CI scripts (impl of `cargo dist generate-ci`)
pub fn do_generate_ci(cfg: &Config, _args: &GenerateCiArgs) -> Result<()> {
    let dist = gather_work(cfg)?;
    if !dist.is_init {
        return Err(miette!(
            "please run 'cargo dist init' before running any other commands!"
        ));
    }

    for style in &cfg.ci {
        match style {
            CiStyle::Github => {
                ci::generate_github_ci(&dist.workspace_dir, &cfg.targets, &cfg.installers)?
            }
        }
    }
    Ok(())
}

/// Build a cargo target
fn generate_installer(_dist_graph: &DistGraph, style: &InstallerImpl) -> Result<()> {
    match style {
        InstallerImpl::GithubShell(info) => installer::generate_github_install_sh_script(info),
        InstallerImpl::GithubPowershell(info) => installer::generate_github_install_ps_script(info),
    }
}
