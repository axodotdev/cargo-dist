#![deny(missing_docs)]
#![allow(clippy::single_match)]

//! # cargo-dist
//!
//!

use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Read},
    process::Command,
};

use camino::{Utf8Path, Utf8PathBuf};
use cargo_dist_schema::{Artifact, Asset, AssetKind, DistManifest, ExecutableAsset, Release};
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

    // TODO: parallelize this by working this like a dependency graph, so we can start
    // bundling up an executable the moment it's built!

    // First set up our target dirs so things don't have to race to do it later
    if !dist.dist_dir.exists() {
        std::fs::create_dir_all(&dist.dist_dir)
            .into_diagnostic()
            .wrap_err_with(|| format!("couldn't create dist target dir at {}", dist.dist_dir))?;
    }

    for artifact in &dist.artifacts {
        eprintln!("bundling {}", artifact.file_name);
        init_artifact_dir(&dist, artifact)?;
    }

    let mut built_assets = HashMap::new();
    if cfg.build {
        // Run all the builds
        for target in &dist.targets {
            let new_built_assets = build_target(&dist, target)?;
            // Copy the artifacts as soon as possible, future builds may clobber them!
            for (&built_asset_idx, built_asset) in &new_built_assets {
                populate_artifact_dirs_with_built_assets(&dist, built_asset_idx, built_asset)?;
            }
            built_assets.extend(new_built_assets);
        }
    }

    // Build all the bundles
    for artifact in &dist.artifacts {
        populate_artifact_dir_with_static_assets(&dist, artifact)?;
        bundle_artifact(&dist, artifact)?;
        eprintln!("bundled {}", artifact.file_path);
    }

    Ok(build_manifest(cfg, &dist))
}

/// Just generate the manifest produced by `cargo dist build` without building
pub fn do_manifest(cfg: &Config) -> Result<DistManifest> {
    let dist = gather_work(cfg)?;
    Ok(build_manifest(cfg, &dist))
}

fn build_manifest(cfg: &Config, dist: &DistGraph) -> DistManifest {
    // Report the releases
    let mut releases = vec![];
    for release in &dist.releases {
        releases.push(Release {
            app_name: release.app_name.clone(),
            app_version: release.version.to_string(),
            changelog_title: release.changelog_title.clone(),
            changelog_body: release.changelog_body.clone(),
            artifacts: release
                .artifacts
                .iter()
                .map(|artifact_idx| {
                    let artifact = &dist.artifacts[artifact_idx.0];
                    let mut assets = vec![];

                    let built_assets =
                        artifact
                            .built_assets
                            .iter()
                            .filter_map(|(asset_idx, asset_path)| {
                                let asset = &dist.built_assets[asset_idx.0];
                                match asset {
                                    BuiltAsset::Executable(exe) => {
                                        let symbols_artifact = exe.symbols_artifact.map(|a| {
                                            dist.artifacts[a.0]
                                                .file_path
                                                .file_name()
                                                .unwrap()
                                                .to_owned()
                                        });
                                        Some(Asset {
                                            name: Some(exe.exe_name.clone()),
                                            path: Some(asset_path.to_string()),
                                            kind: AssetKind::Executable(ExecutableAsset {
                                                symbols_artifact,
                                            }),
                                        })
                                    }
                                    BuiltAsset::Symbols(_sym) => {
                                        // Symbols are their own assets, so no need to report
                                        None
                                    }
                                }
                            });

                    let static_assets = artifact.static_assets.iter().map(|(kind, asset)| {
                        let kind = match kind {
                            StaticAssetKind::Changelog => AssetKind::Changelog,
                            StaticAssetKind::License => AssetKind::License,
                            StaticAssetKind::Readme => AssetKind::Readme,
                        };
                        Asset {
                            name: Some(asset.file_name().unwrap().to_owned()),
                            path: Some(asset.file_name().unwrap().to_owned()),
                            kind,
                        }
                    });

                    assets.extend(built_assets);
                    assets.extend(static_assets);
                    // Sort the assets by name to make things extra stable
                    assets.sort_by(|k1, k2| k1.name.cmp(&k2.name));

                    let mut install_hint = None;
                    let mut description = None;

                    match &artifact.bundle {
                        BundleStyle::Installer(InstallerImpl::GithubShell(info)) => {
                            install_hint = Some(info.hint.clone());
                            description = Some(info.desc.clone());
                        }
                        BundleStyle::Installer(InstallerImpl::GithubPowershell(info)) => {
                            install_hint = Some(info.hint.clone());
                            description = Some(info.desc.clone());
                        }
                        BundleStyle::Zip | BundleStyle::Tar(_) | BundleStyle::UncompressedFile => {
                            // Nothing yet
                        }
                    }

                    Artifact {
                        name: Some(artifact.file_path.file_name().unwrap().to_owned()),
                        path: if cfg.no_local_paths {
                            None
                        } else {
                            Some(artifact.file_path.to_string())
                        },
                        target_triples: artifact.target_triples.clone(),
                        install_hint,
                        description,
                        assets,
                        kind: artifact.kind.clone(),
                    }
                })
                .collect(),
        })
    }
    let dist_version = env!("CARGO_PKG_VERSION").to_owned();
    DistManifest::new(dist_version, releases)
}

/// Build a target
fn build_target(
    dist_graph: &DistGraph,
    target: &BuildTarget,
) -> Result<HashMap<BuiltAssetIdx, Utf8PathBuf>> {
    match target {
        BuildTarget::Cargo(target) => build_cargo_target(dist_graph, target),
    }
}

/// Build a cargo target
fn build_cargo_target(
    dist_graph: &DistGraph,
    target: &CargoBuildTarget,
) -> Result<HashMap<BuiltAssetIdx, Utf8PathBuf>> {
    eprintln!(
        "building cargo target ({}/{})",
        target.target_triple, target.profile
    );
    // Run the build
    let mut rustflags = std::env::var("RUSTFLAGS").unwrap_or_default();

    // TODO: figure out a principled way for us to add things to RUSTFLAGS
    // without breaking everything. Cargo has some builtin ways like keys
    // in [target...] tables that will get "merged" with the flags it wants
    // to set. More blunt approaches like actually setting the environment
    // variable I think can result in overwriting flags other places set
    // (which is defensible, having spaghetti flags randomly injected by
    // a dozen different tools is a build maintenance nightmare!)

    // TODO: on windows, set RUSTFLAGS="-Ctarget-feature=+crt-static"
    // See: https://rust-lang.github.io/rfcs/1721-crt-static.html
    //
    // Essentially you're *supposed* to be statically linking the windows """libc"""
    // because it's actually a wrapper around more fundamental DLLs and not
    // actually guaranteed to be on the system. This is why lots of games
    // install a C/C++ runtime in their wizards! Unclear what the cost/benefit
    // is of "install" vs "statically link", especially if you only need C
    // and not all of C++. I am however unclear on "which" "libc" you're statically
    // linking. More Research Needed.
    //
    // For similar reasons we may want to prefer targeting "linux-musl" over
    // "linux-gnu" -- the former statically links libc and makes us more portable
    // to "weird" linux setups like NixOS which apparently doesn't have like
    // /etc or /lib to try to try to force things to properly specify their deps
    // (statically linking libc says "no deps pls" (except for specific linux syscalls probably)).
    // I am however vaguely aware of issues where some system magic is hidden away
    // in the gnu libc (glibc) and musl subsequently diverges and acts wonky?
    // This is all vague folklore to me, so More Research Needed.
    //
    // Just to round things out, let's discuss macos. I've never heard of these kinds
    // of issues wrt macos! However I am vaguely aware that macos has an "sdk version"
    // system, which vaguely specifies what APIs you're allowing yourself to use so
    // you can be compatible with any system at least that new (so the older the SDK,
    // the more compatible you are). Do we need to care about that? More Research Needed.

    if target.target_triple.contains("windows-msvc") {
        rustflags.push_str(" -Ctarget-feature=+crt-static");
    }

    // TODO: maybe set RUSTFLAGS="-Cforce-frame-pointers=yes"
    //
    // On linux and macos this can make the unwind tables (debuginfo) smaller, more reliable,
    // and faster at minimal runtime cost (these days). This can be a big win for profilers
    // and crash reporters, which both want to unwind in "weird" places quickly and reliably.
    //
    // On windows this setting is unfortunately useless because Microsoft specified
    // it to be... Wrong. Specifically it points "somewhere" in the frame (instead of
    // at the start), and exists only to enable things like -Oz.
    // See: https://github.com/rust-lang/rust/issues/82333

    // TODO: maybe set RUSTFLAGS="-Csymbol-mangling-version=v0"
    // See: https://github.com/rust-lang/rust/issues/60705
    //
    // Despite the name, v0 is actually the *second* mangling format for Rust symbols.
    // The first was more unprincipled and adhoc, and is just the unnamed current
    // default. In the future v0 should become the default. Currently we're waiting
    // for as many tools as possible to add support (and then make it onto dev machines).
    //
    // The v0 scheme is bigger and contains more rich information (with its own fancy
    // compression scheme to try to compensate). Unclear on the exact pros/cons of
    // opting into it earlier.

    // TODO: is there *any* world where we can help the user use Profile Guided Optimization (PGO)?
    // See: https://doc.rust-lang.org/rustc/profile-guided-optimization.html
    // See: https://blog.rust-lang.org/inside-rust/2020/11/11/exploring-pgo-for-the-rust-compiler.html
    //
    // In essence PGO is a ~three-step process:
    //
    // 1. Build your program
    // 2. Run it on a "representative" workload and record traces of the execution ("a profile")
    // 3. Rebuild your program with the profile to Guide Optimization
    //
    // For instance the compiler might see that a certain branch (if) always goes one way
    // in the profile, and optimize the code to go faster if that holds true (by say outlining
    // the other path).
    //
    // PGO can get *huge* wins but is at the mercy of step 2, which is difficult/impossible
    // for a tool like cargo-dist to provide "automatically". But maybe we can streamline
    // some of the rough edges? This is also possibly a place where A Better Telemetry Solution
    // could do some interesting things for dev-controlled production environments.

    // TODO: can we productively use RUSTFLAGS="--remap-path-prefix"?
    // See: https://doc.rust-lang.org/rustc/command-line-arguments.html#--remap-path-prefix-remap-source-names-in-output
    // See: https://github.com/rust-lang/rust/issues/87805
    //
    // Compiler toolchains like stuffing absolute host system paths in metadata/debuginfo,
    // which can make things Bigger and also leak a modicum of private info. This flag
    // lets you specify a rewrite rule for a prefix of the path, letting you map e.g.
    // "C:\Users\Aria\checkouts\cargo-dist\src\main.rs" to ".\cargo-dist\src\main.rs".
    //
    // Unfortunately this is a VERY blunt instrument which does legit exact string matching
    // and can miss paths in places rustc doesn't Expect/See. Still it might be worth
    // setting it in case it Helps?

    let mut command = Command::new(&dist_graph.cargo);
    command
        .arg("build")
        .arg("--profile")
        .arg(&target.profile)
        .arg("--message-format=json")
        .arg("--target")
        .arg(&target.target_triple)
        .env("RUSTFLAGS", rustflags)
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
    let mut expected_exes = HashMap::<String, HashMap<String, (BuiltAssetIdx, Utf8PathBuf)>>::new();
    let mut expected_symbols =
        HashMap::<String, HashMap<String, (BuiltAssetIdx, Utf8PathBuf)>>::new();
    for asset_idx in &target.expected_assets {
        let asset = &dist_graph.built_assets[asset_idx.0];
        match asset {
            BuiltAsset::Executable(exe) => {
                let package_id = exe.package_id.to_string();
                let exe_name = exe.exe_name.clone();
                expected_exes
                    .entry(package_id)
                    .or_default()
                    .insert(exe_name, (*asset_idx, Utf8PathBuf::new()));
            }
            BuiltAsset::Symbols(sym) => {
                let package_id = sym.package_id.to_string();
                let exe_name = sym.exe_name.clone();
                expected_symbols
                    .entry(package_id)
                    .or_default()
                    .insert(exe_name, (*asset_idx, Utf8PathBuf::new()));
            }
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
                    if let Some((expected_sym_asset, sym_path)) = expected_sym {
                        let expected_sym_asset = &dist_graph.built_assets[expected_sym_asset.0];
                        let BuiltAsset::Symbols(expected_sym_asset) = expected_sym_asset else {
                            unreachable!()
                        };
                        for path in artifact.filenames {
                            let is_symbols = path
                                .extension()
                                .map(|e| e == expected_sym_asset.symbol_kind.ext())
                                .unwrap_or(false);
                            if is_symbols {
                                // These are symbols we expected! Save the path.
                                *sym_path = path;
                            }
                        }
                    }

                    // Get the exe path
                    let expected_exe = expected_exes
                        .get_mut(&package_id)
                        .and_then(|m| m.get_mut(exe_name));
                    if let Some(expected) = expected_exe {
                        // This is an exe we expected! Save the path.
                        expected.1 = new_exe;
                    }
                }
            }
            _ => {
                // Nothing else interesting?
            }
        }
    }

    // Check that we got everything we expected, and normalize to ArtifactIdx => Artifact Path
    let mut built_assets = HashMap::new();
    for (package_id, exes) in expected_exes {
        for (exe, (artifact_idx, exe_path)) in exes {
            if exe_path.as_str().is_empty() {
                return Err(miette!("failed to find bin {} for {}", exe, package_id));
            }
            built_assets.insert(artifact_idx, exe_path);
        }
    }
    for (package_id, symbols) in expected_symbols {
        for (exe, (artifact_idx, sym_path)) in symbols {
            if sym_path.as_str().is_empty() {
                return Err(miette!("failed to find symbols {} for {}", exe, package_id));
            }
            built_assets.insert(artifact_idx, sym_path);
        }
    }

    Ok(built_assets)
}

/// Initialize the dir for an artifact (and delete the old artifact file).
fn init_artifact_dir(_dist: &DistGraph, artifact: &ArtifactTarget) -> Result<()> {
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

fn populate_artifact_dirs_with_built_assets(
    dist: &DistGraph,
    built_asset_idx: BuiltAssetIdx,
    built_asset_path: &Utf8Path,
) -> Result<()> {
    for artifact in &dist.artifacts {
        if let Some(rel_asset_path) = artifact.built_assets.get(&built_asset_idx) {
            let bundled_asset = if let BundleStyle::UncompressedFile = artifact.bundle {
                // If the asset is a single uncompressed file, we can just copy it to its final dest
                info!("  copying {built_asset_path} to {}", artifact.file_path);
                artifact.file_path.clone()
            } else {
                let artifact_dir_path = artifact
                    .dir_path
                    .as_ref()
                    .expect("compressed bundle didn't have a dir path?!");
                info!("  adding {built_asset_path} to {}", artifact_dir_path);
                artifact_dir_path.join(rel_asset_path)
            };

            std::fs::copy(built_asset_path, &bundled_asset)
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!(
                        "failed to copy built asset to artifact: {built_asset_path} => {bundled_asset}"
                    )
                })?;
        }
    }
    Ok(())
}

fn populate_artifact_dir_with_static_assets(
    _dist: &DistGraph,
    artifact: &ArtifactTarget,
) -> Result<()> {
    let Some(artifact_dir_path) = &artifact.dir_path else {
        assert!(artifact.static_assets.is_empty(), "had static assets but didn't have a dir path?!");
        // If there's no dir than we're done
        return Ok(());
    };

    info!("populating artifact dir: {}", artifact_dir_path);
    // Copy assets
    for (_kind, asset) in &artifact.static_assets {
        let asset_file_name = asset.file_name().unwrap();
        let bundled_asset = artifact_dir_path.join(asset_file_name);
        info!("  adding {bundled_asset}");
        std::fs::copy(asset, &bundled_asset)
            .into_diagnostic()
            .wrap_err_with(|| {
                format!("failed to copy bundled asset to artifact: {asset} => {bundled_asset}")
            })?;
    }

    Ok(())
}

fn bundle_artifact(dist_graph: &DistGraph, artifact: &ArtifactTarget) -> Result<()> {
    info!("bundling artifact: {}", artifact.file_path);
    match &artifact.bundle {
        BundleStyle::Zip => zip_artifact(dist_graph, artifact),
        BundleStyle::Tar(compression) => tar_artifact(dist_graph, artifact, compression),
        BundleStyle::Installer(installer) => generate_installer(dist_graph, artifact, installer),
        BundleStyle::UncompressedFile => {
            // Already handled by populate_artifact_dirs_with_built_assets
            info!("artifact created at: {}", artifact.file_path);
            Ok(())
        }
    }
}

fn tar_artifact(
    _dist_graph: &DistGraph,
    artifact: &ArtifactTarget,
    compression: &CompressionImpl,
) -> Result<()> {
    // Set up the archive/compression
    // The contents of the zip (e.g. a tar)
    let artifact_dir_path = artifact.dir_path.as_ref().unwrap();
    let artifact_dir_name = &artifact.dir_name.as_ref().unwrap();
    let zip_contents_name = format!("{artifact_dir_name}.tar");
    let final_zip_path = &artifact.file_path;
    let final_zip_file = File::create(final_zip_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to create file for artifact: {final_zip_path}"))?;

    match compression {
        CompressionImpl::Gzip => {
            // Wrap our file in compression
            let zip_output = GzBuilder::new()
                .filename(zip_contents_name)
                .write(final_zip_file, Compression::default());

            // Write the tar to the compression stream
            let mut tar = tar::Builder::new(zip_output);

            // Add the whole dir to the tar
            tar.append_dir_all(artifact_dir_name, artifact_dir_path)
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!(
                        "failed to copy directory into tar: {artifact_dir_path} => {artifact_dir_name}",
                    )
                })?;
            // Finish up the tarring
            let zip_output = tar
                .into_inner()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write tar: {final_zip_path}"))?;
            // Finish up the compression
            let _zip_file = zip_output
                .finish()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write archive: {final_zip_path}"))?;
            // Drop the file to close it
        }
        CompressionImpl::Xzip => {
            let zip_output = XzEncoder::new(final_zip_file, 9);
            // Write the tar to the compression stream
            let mut tar = tar::Builder::new(zip_output);

            // Add the whole dir to the tar
            tar.append_dir_all(artifact_dir_name, artifact_dir_path)
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!(
                        "failed to copy directory into tar: {artifact_dir_path} => {artifact_dir_name}",
                    )
                })?;
            // Finish up the tarring
            let zip_output = tar
                .into_inner()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write tar: {final_zip_path}"))?;
            // Finish up the compression
            let _zip_file = zip_output
                .finish()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write archive: {final_zip_path}"))?;
            // Drop the file to close it
        }
        CompressionImpl::Zstd => {
            // Wrap our file in compression
            let zip_output = ZlibEncoder::new(final_zip_file, Compression::default());

            // Write the tar to the compression stream
            let mut tar = tar::Builder::new(zip_output);

            // Add the whole dir to the tar
            tar.append_dir_all(artifact_dir_name, artifact_dir_path)
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!(
                        "failed to copy directory into tar: {artifact_dir_path} => {artifact_dir_name}",
                    )
                })?;
            // Finish up the tarring
            let zip_output = tar
                .into_inner()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write tar: {final_zip_path}"))?;
            // Finish up the compression
            let _zip_file = zip_output
                .finish()
                .into_diagnostic()
                .wrap_err_with(|| format!("failed to write archive: {final_zip_path}"))?;
            // Drop the file to close it
        }
    }

    info!("artifact created at: {}", final_zip_path);
    Ok(())
}

fn zip_artifact(_dist_graph: &DistGraph, artifact: &ArtifactTarget) -> Result<()> {
    // Set up the archive/compression
    let artifact_dir_path = artifact.dir_path.as_ref().unwrap();
    let final_zip_path = &artifact.file_path;
    let final_zip_file = File::create(final_zip_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to create file for artifact: {final_zip_path}"))?;

    // Wrap our file in compression
    let mut zip = ZipWriter::new(final_zip_file);

    let dir = std::fs::read_dir(artifact_dir_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to read artifact dir: {artifact_dir_path}"))?;
    for entry in dir {
        let entry = entry.into_diagnostic()?;
        if entry.file_type().into_diagnostic()?.is_file() {
            let options = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            let file = File::open(entry.path()).into_diagnostic()?;
            let mut buf = BufReader::new(file);
            let file_name = entry.file_name();
            // TODO: ...don't do this lossy conversion?
            let utf8_file_name = file_name.to_string_lossy();
            zip.start_file(utf8_file_name.clone(), options)
                .into_diagnostic()
                .wrap_err_with(|| {
                    format!("failed to create file {utf8_file_name} in zip: {final_zip_path}")
                })?;
            std::io::copy(&mut buf, &mut zip).into_diagnostic()?;
        } else {
            panic!("TODO: implement zip subdirs! (or was this a symlink?)");
        }
    }

    // Finish up the compression
    let _zip_file = zip
        .finish()
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to write archive: {final_zip_path}"))?;
    // Drop the file to close it
    info!("artifact created at: {}", final_zip_path);
    Ok(())
}

/// Arguments for `cargo dist init` ([`do_init`][])
#[derive(Debug)]
pub struct InitArgs {
    /// The styles of CI we should generate
    pub ci_styles: Vec<CiStyle>,
}

/// Run 'cargo dist init'
pub fn do_init(cfg: &Config, args: &InitArgs) -> Result<()> {
    let cargo = tasks::cargo()?;
    let pkg_graph = tasks::package_graph(&cargo)?;
    let workspace = tasks::workspace_info(&pkg_graph)?;

    // Load in the workspace toml to edit and write back
    let mut workspace_toml = {
        let mut workspace_toml_file = File::open(&workspace.manifest_path)
            .into_diagnostic()
            .wrap_err("couldn't load root workspace Cargo.toml")?;
        let mut workspace_toml_str = String::new();
        workspace_toml_file
            .read_to_string(&mut workspace_toml_str)
            .into_diagnostic()
            .wrap_err("couldn't read root workspace Cargo.toml")?;
        workspace_toml_str
            .parse::<toml_edit::Document>()
            .into_diagnostic()
            .wrap_err("couldn't parse root workspace Cargo.toml")?
    };

    // Setup the [profile.dist]
    {
        let profiles = workspace_toml["profile"].or_insert(toml_edit::table());
        if let Some(t) = profiles.as_table_mut() {
            t.set_implicit(true)
        }
        let dist_profile = &mut profiles[PROFILE_DIST];
        if !dist_profile.is_none() {
            return Err(miette!(
                "already init! (based on [profile.dist] existing in your Cargo.toml)"
            ));
        }
        let mut new_profile = toml_edit::table();
        {
            let new_profile = new_profile.as_table_mut().unwrap();
            // We're building for release, so this is a good base!
            new_profile.insert("inherits", toml_edit::value("release"));
            // We want *full* debuginfo for good crashreporting/profiling
            // This doesn't bloat the final binary because we use split-debuginfo below
            new_profile.insert("debug", toml_edit::value(true));
            // Ensure that all debuginfo is pulled out of the binary and tossed
            // into a separate file from the final binary. This should ideally be
            // uploaded to something like a symbol server to be fetched on demand.
            // This is already the default on windows (.pdb) and macos (.dsym) but
            // is rather bleeding on other platforms (.dwp) -- it requires Rust 1.65,
            // which as of this writing in the latest stable rust release! If anyone
            // ever makes a big deal with building final binaries with an older MSRV
            // we may need to more intelligently select this.
            new_profile.insert("split-debuginfo", toml_edit::value("packed"));

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
                .set_prefix("\n# generated by 'cargo dist init'\n")
        }
        dist_profile.or_insert(new_profile);
    }
    // Setup [workspace.metadata.dist] or [package.metadata.dist]
    /* temporarily disabled until we have a real config to write here
    {
        let metadata_pre_key = if workspace.root_package.is_some() {
            "package"
        } else {
            "workspace"
        };
        let workspace = workspace_toml[metadata_pre_key].or_insert(toml_edit::table());
        if let Some(t) = workspace.as_table_mut() {
            t.set_implicit(true)
        }
        let metadata = workspace["metadata"].or_insert(toml_edit::table());
        if let Some(t) = metadata.as_table_mut() {
            t.set_implicit(true)
        }
        let dist_metadata = &mut metadata[METADATA_DIST];
        if !dist_metadata.is_none() {
            return Err(miette!(
                "already init! (based on [workspace.metadata.dist] existing in your Cargo.toml)"
            ));
        }
        let mut new_metadata = toml_edit::table();
        {
            let new_metadata = new_metadata.as_table_mut().unwrap();
            new_metadata.insert(
                "os",
                toml_edit::Item::Value([OS_WINDOWS, OS_MACOS, OS_LINUX].into_iter().collect()),
            );
            new_metadata.insert(
                "cpu",
                toml_edit::Item::Value([CPU_X64, CPU_ARM64].into_iter().collect()),
            );
            new_metadata.decor_mut().set_prefix(
                "\n# These keys are generated by 'cargo dist init' and are fake placeholders\n",
            );
        }

        dist_metadata.or_insert(new_metadata);
    }
    */
    {
        use std::io::Write;
        let mut workspace_toml_file = File::options()
            .write(true)
            .open(&workspace.manifest_path)
            .into_diagnostic()
            .wrap_err("couldn't load root workspace Cargo.toml")?;
        writeln!(&mut workspace_toml_file, "{workspace_toml}")
            .into_diagnostic()
            .wrap_err("failed to write to Cargo.toml")?;
    }
    if !args.ci_styles.is_empty() {
        let ci_args = GenerateCiArgs {
            ci_styles: args.ci_styles.clone(),
        };
        do_generate_ci(cfg, &ci_args)?;
    }
    Ok(())
}

/// Arguments for `cargo dist generate-ci` ([`do_generate_ci][])
#[derive(Debug)]
pub struct GenerateCiArgs {
    /// Styles of CI to generate
    pub ci_styles: Vec<CiStyle>,
}

/// Generate CI scripts (impl of `cargo dist generate-ci`)
pub fn do_generate_ci(cfg: &Config, args: &GenerateCiArgs) -> Result<()> {
    let graph = gather_work(cfg)?;
    for style in &args.ci_styles {
        match style {
            CiStyle::Github => ci::generate_github_ci(
                &graph.workspace_dir,
                &cfg.targets,
                cfg.exe_bundle_style.as_ref(),
                &cfg.installers,
            )?,
        }
    }
    Ok(())
}

/// Build a cargo target
fn generate_installer(
    _dist_graph: &DistGraph,
    target: &ArtifactTarget,
    style: &InstallerImpl,
) -> Result<()> {
    match style {
        InstallerImpl::GithubShell(info) => {
            installer::generate_github_install_sh_script(target, info)
        }
        InstallerImpl::GithubPowershell(info) => {
            installer::generate_github_install_ps_script(target, info)
        }
    }
}
