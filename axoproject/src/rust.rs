//! Support for Cargo-based Rust projects

use std::collections::BTreeMap;

use crate::{PackageInfo, Result, Version, WorkspaceInfo, WorkspaceKind, WorkspaceSearch};
use axoasset::SourceFile;
use camino::{Utf8Path, Utf8PathBuf};
use guppy::{
    graph::{BuildTargetId, BuildTargetKind, DependencyDirection, PackageGraph, PackageMetadata},
    MetadataCommand,
};
use itertools::{concat, Itertools};

pub use axoasset::toml_edit::DocumentMut;

/// All the `[profile]` entries we found in the root Cargo.toml
pub type CargoProfiles = BTreeMap<String, CargoProfile>;

/// Try to find a Cargo/Rust workspace at the given path
///
/// See [`crate::get_workspaces`][] for the semantics.
///
/// This relies on `cargo metadata` so will only work if you have `cargo` installed.
pub fn get_workspace(start_dir: &Utf8Path, clamp_to_dir: Option<&Utf8Path>) -> WorkspaceSearch {
    // The call to `workspace_manifest` here is technically redundant with what cargo-metadata will
    // do, but doing it ourselves makes it really easy to distinguish between
    // "no workspace at all" and "workspace is busted", and to provide better context in the latter.
    let manifest_path = match workspace_manifest(start_dir, clamp_to_dir) {
        Ok(path) => path,
        Err(e) => {
            return WorkspaceSearch::Missing(e);
        }
    };

    // There's definitely some kind of Cargo workspace, now try to make sense of it
    let workspace = package_graph(start_dir).and_then(|graph| workspace_info(&graph));
    match workspace {
        Ok(workspace) => WorkspaceSearch::Found(workspace),
        Err(e) => WorkspaceSearch::Broken {
            manifest_path,
            cause: e,
        },
    }
}

/// Get the PackageGraph for the current workspace
fn package_graph(start_dir: &Utf8Path) -> Result<PackageGraph> {
    let mut metadata_cmd = MetadataCommand::new();

    // We don't care about dependency information, and disabling it makes things much faster!
    metadata_cmd.no_deps();
    metadata_cmd.current_dir(start_dir);

    let pkg_graph = metadata_cmd.build_graph()?;

    Ok(pkg_graph)
}

/// Computes [`WorkspaceInfo`][] for the current workspace.
fn workspace_info(pkg_graph: &PackageGraph) -> Result<WorkspaceInfo> {
    let workspace = pkg_graph.workspace();
    let members = pkg_graph.resolve_workspace();
    let warnings = vec![];

    let manifest_path = workspace.root().join("Cargo.toml");
    // I originally had this as a proper Error but honestly this would be MADNESS and
    // I want someone to tell me about this if they ever encounter it, so blow everything up
    assert!(
        manifest_path.exists(),
        "cargo metadata returned a workspace without a Cargo.toml!?"
    );

    let cargo_profiles = get_profiles(&manifest_path)?;

    let cargo_metadata_table = Some(workspace.metadata_table().clone());
    let workspace_root = workspace.root();
    let root_auto_includes = crate::find_auto_includes(workspace_root)?;
    let mut all_package_info = vec![];
    for package in members.packages(DependencyDirection::Forward) {
        let mut info = package_info(workspace_root, &package)?;
        crate::merge_auto_includes(&mut info, &root_auto_includes);
        all_package_info.push(info);
    }

    let target_dir = workspace.target_directory().to_owned();
    let workspace_dir = workspace.root().to_owned();

    Ok(WorkspaceInfo {
        kind: WorkspaceKind::Rust,
        target_dir,
        workspace_dir,
        _sub_workspaces: vec![],
        _package_info: all_package_info,
        manifest_path,
        root_auto_includes,
        cargo_metadata_table,
        cargo_profiles,

        warnings,
    })
}

fn package_info(_workspace_root: &Utf8Path, package: &PackageMetadata) -> Result<PackageInfo> {
    let manifest_path = package.manifest_path().to_owned();
    let package_root = manifest_path
        .parent()
        .expect("package manifest had no parent!?")
        .to_owned();
    let cargo_package_id = Some(package.id().clone());
    let cargo_metadata_table = Some(package.metadata_table().clone());

    let mut binaries = vec![];
    let mut cdylibs = vec![];
    let mut cstaticlibs = vec![];
    for target in package.build_targets() {
        let build_id = target.id();
        match build_id {
            BuildTargetId::Binary(name) => {
                // Hooray it's a proper binary
                binaries.push(name.to_owned());
            }
            BuildTargetId::Library => {
                // This is the ONE AND ONLY "true" library target, now that we've confirmed
                // that we can trust BuildTargetKind::LibraryOrExample to only be non-examples.
                // All the different kinds of library outputs like cdylibs and staticlibs are
                // shoved into this one build target, making it impossible to build only one
                // at a time (which is really unfortunate because cargo can produce conflicting
                // names for some of the outputs on some platforms).
                //
                // crate-types is a messy field with weird naming and history. The outputs are
                // roughly broken into two families (by me). See rustc's docs for details:
                //
                // https://doc.rust-lang.org/nightly/reference/linkage.html
                //
                //
                // # rust-only / intermediates
                //
                // * proc-macro: a target to build the proc-macros *defined* by this crate
                // * rlib: a rust-only staticlib
                // * dylib: a rust-only dynamic library
                // * lib: the fuzzy default library target that lets cargo/rustc pick
                //   the "right" choice. this enables things like -Cprefer-dynamic
                //   which override all libs to the desired result.
                //
                // The rust-only outputs are mostly things rust developers don't have to care
                // about, and mostly exist as intermediate.temporary results (the main exception
                // is the stdlib is shipped in this form, because it's released in lockstep with
                // the rustc that understands it)
                //
                //
                // # final outputs
                //
                // * staticlib: a C-style static library
                // * cdylib: a C-style dynamic library
                // * bin: a binary (not relevant here)
                //
                // Grouping a C-style static library here is kinda dubious but at very least
                // it's something meaningful outside of cargo/rustc itself (I super don't care
                // that rlibs are a thin veneer over staticlibs and that you got things to link,
                // you're not "supposed" to do that.)
                if let BuildTargetKind::LibraryOrExample(crate_types) = target.kind() {
                    for crate_type in crate_types {
                        match &**crate_type {
                            "cdylib" => {
                                cdylibs.push(target.name().to_owned());
                            }
                            "staticlib" => {
                                cstaticlibs.push(target.name().to_owned());
                            }
                            _ => {
                                // Don't care about these
                            }
                        }
                    }
                }
            }
            _ => {
                // Don't care about these
            }
        }
    }

    let keywords_and_categories: Option<Vec<String>> =
        if package.keywords().is_empty() && package.categories().is_empty() {
            None
        } else {
            let categories = package.categories().to_vec();
            let keywords = package.keywords().to_vec();
            Some(
                concat(vec![categories, keywords])
                    .into_iter()
                    .unique()
                    .collect::<Vec<String>>(),
            )
        };

    let mut info = PackageInfo {
        name: package.name().to_owned(),
        version: Some(Version::Cargo(package.version().clone())),
        manifest_path,
        package_root: package_root.clone(),
        description: package.description().map(ToOwned::to_owned),
        authors: package.authors().to_vec(),
        keywords: keywords_and_categories,
        license: package.license().map(ToOwned::to_owned),
        publish: !package.publish().is_never(),
        repository_url: package.repository().map(ToOwned::to_owned),
        homepage_url: package.homepage().map(ToOwned::to_owned),
        documentation_url: package.documentation().map(ToOwned::to_owned),
        readme_file: package.readme().map(|readme| package_root.join(readme)),
        license_files: package
            .license_file()
            .map(ToOwned::to_owned)
            .into_iter()
            .collect(),
        changelog_file: None,
        binaries,
        cdylibs,
        cstaticlibs,
        cargo_metadata_table,
        cargo_package_id,
        #[cfg(feature = "generic-projects")]
        build_command: None,
    };

    // Find files we might want to auto-include
    // It's kind of unfortunate that we do this unconditionally for every
    // package, even if we'll never care about the result, but that's how
    // separation of concerns gets ya.
    let auto_includes = crate::find_auto_includes(&package_root)?;
    crate::merge_auto_includes(&mut info, &auto_includes);

    // If there's no documentation URL provided, default assume it's docs.rs like crates.io does
    if info.documentation_url.is_none() {
        info.documentation_url = Some(format!(
            "https://docs.rs/{}/{}",
            info.name,
            info.version.as_ref().unwrap()
        ));
    }

    Ok(info)
}

/// Find a Cargo.toml, starting at the given dir and walking up to ancestor dirs,
/// optionally clamped to a given ancestor dir
fn workspace_manifest(
    start_dir: &Utf8Path,
    clamp_to_dir: Option<&Utf8Path>,
) -> Result<Utf8PathBuf> {
    crate::find_file("Cargo.toml", start_dir, clamp_to_dir)
}

/// Load the root workspace toml into toml-edit form
pub fn load_root_cargo_toml(manifest_path: &Utf8Path) -> Result<DocumentMut> {
    let manifest_src = SourceFile::load_local(manifest_path)?;
    let manifest = manifest_src.deserialize_toml_edit()?;
    Ok(manifest)
}

fn get_profiles(manifest_path: &Utf8Path) -> Result<BTreeMap<String, CargoProfile>> {
    let mut profiles = CargoProfiles::new();
    let workspace_toml = load_root_cargo_toml(manifest_path)?;
    let Some(profiles_table) = &workspace_toml.get("profile").and_then(|t| t.as_table()) else {
        // No table, empty return
        return Ok(profiles);
    };

    for (profile_name, profile) in profiles_table.iter() {
        // Get the fields we care about
        let debug = profile.get("debug");
        let split_debuginfo = profile.get("split-debuginfo");
        let inherits = profile.get("inherits");

        // clean up the true/false sugar for "debug"
        let debug = debug.and_then(|debug| {
            debug
                .as_bool()
                .map(|val| if val { 2 } else { 0 })
                .or_else(|| debug.as_integer())
        });

        // Just capture these directly
        let split_debuginfo = split_debuginfo
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned);
        let inherits = inherits.and_then(|v| v.as_str()).map(ToOwned::to_owned);

        let entry = CargoProfile {
            inherits,
            debug,
            split_debuginfo,
        };
        profiles.insert(profile_name.to_owned(), entry);
    }

    Ok(profiles)
}

/// Parts of a [profile.*] entry in a Cargo.toml we care about
#[derive(Debug, Clone)]
pub struct CargoProfile {
    /// What profile a custom profile inherits from
    pub inherits: Option<String>,
    /// Whether debuginfo is enabled.
    ///
    /// can be 0, 1, 2, true (=2), false (=0).
    pub debug: Option<i64>,
    /// Whether split-debuginfo is enabled.
    ///
    /// Can be "off", "packed", or "unpacked".
    ///
    /// If "packed" then we expect a pdb/dsym/dwp artifact.
    pub split_debuginfo: Option<String>,
}
