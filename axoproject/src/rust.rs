//! Support for Cargo-based Rust projects

use std::{collections::BTreeMap, fs::File, io::Read};

use crate::{PackageInfo, Result, Version, WorkspaceInfo, WorkspaceKind};
use camino::Utf8Path;
use guppy::{
    graph::{BuildTargetId, DependencyDirection, PackageGraph, PackageMetadata},
    MetadataCommand,
};
use miette::{miette, Context, IntoDiagnostic};
use tracing::warn;

/// All the [profile] entries we found in the root Cargo.toml
pub type CargoProfiles = BTreeMap<String, CargoProfile>;

/// Try to find a Cargo/Rust project at the given path
///
/// This relies on `cargo metadata` so will only work if you have `cargo` installed.
pub fn get_project(start_dir: &Utf8Path) -> Result<WorkspaceInfo> {
    let graph = package_graph(start_dir)?;
    workspace_info(&graph)
}

/// Get the PackageGraph for the current workspace
fn package_graph(start_dir: &Utf8Path) -> Result<PackageGraph> {
    let mut metadata_cmd = MetadataCommand::new();

    // We don't care about dependency information, and disabling it makes things much faster!
    metadata_cmd.no_deps();
    metadata_cmd.current_dir(start_dir);

    let pkg_graph = metadata_cmd
        .build_graph()
        .into_diagnostic()
        .wrap_err("failed to read 'cargo metadata'")?;

    Ok(pkg_graph)
}

/// Computes [`WorkspaceInfo`][] for the current workspace.
fn workspace_info(pkg_graph: &PackageGraph) -> Result<WorkspaceInfo> {
    let workspace = pkg_graph.workspace();
    let members = pkg_graph.resolve_workspace();

    let manifest_path = workspace.root().join("Cargo.toml");
    if !manifest_path.exists() {
        return Err(miette!("couldn't find root workspace Cargo.toml"));
    }

    let cargo_profiles = get_profiles(&manifest_path)?;

    let cargo_metadata_table = Some(workspace.metadata_table().clone());
    let workspace_root = workspace.root();
    let root_auto_includes = crate::find_auto_includes(workspace_root)?;

    let mut repo_url_conflicted = false;
    let mut repo_url = None;
    let mut all_package_info = vec![];
    for package in members.packages(DependencyDirection::Forward) {
        let mut info = package_info(workspace_root, &package)?;

        // Apply root workspace's auto-includes
        crate::merge_auto_includes(&mut info, &root_auto_includes);

        // Try to find repo URL consensus
        if !repo_url_conflicted {
            if let Some(new_url) = &info.repository_url {
                if let Some(cur_url) = &repo_url {
                    if new_url == cur_url {
                        // great! consensus!
                    } else {
                        warn!("your workspace has inconsistent values for 'repository', refusing to select one:\n  {}\n  {}", new_url, cur_url);
                        repo_url_conflicted = true;
                        repo_url = None;
                    }
                } else {
                    repo_url = info.repository_url.clone();
                }
            }
        }

        all_package_info.push(info);
    }

    // Normalize trailing `/` on the repo URL
    if let Some(repo_url) = &mut repo_url {
        if repo_url.ends_with('/') {
            repo_url.pop();
        }
    }

    let target_dir = workspace.target_directory().to_owned();
    let workspace_dir = workspace.root().to_owned();

    Ok(WorkspaceInfo {
        kind: WorkspaceKind::Rust,
        target_dir,
        workspace_dir,
        package_info: all_package_info,
        manifest_path,

        repository_url: repo_url,
        root_auto_includes,
        cargo_metadata_table,
        cargo_profiles,
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
    for target in package.build_targets() {
        let build_id = target.id();
        if let BuildTargetId::Binary(name) = build_id {
            binaries.push(name.to_owned());
        }
    }

    let mut info = PackageInfo {
        name: package.name().to_owned(),
        version: Some(Version::Cargo(package.version().clone())),
        manifest_path,
        package_root: package_root.clone(),
        description: package.description().map(ToOwned::to_owned),
        authors: package.authors().to_vec(),
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
        cargo_metadata_table,
        cargo_package_id,
    };

    // Find files we might want to auto-include
    //
    // This is kinda expensive so only bother doing it for things we MIGHT care about
    if !info.binaries.is_empty() {
        let auto_includes = crate::find_auto_includes(&package_root)?;
        crate::merge_auto_includes(&mut info, &auto_includes);
    }

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

/// Load the root workspace toml into toml-edit form
pub fn load_root_cargo_toml(manifest_path: &Utf8Path) -> Result<toml_edit::Document> {
    // FIXME: this should really be factored out into some sort of i/o module
    let mut workspace_toml_file = File::open(manifest_path)
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
        .wrap_err("couldn't parse root workspace Cargo.toml")
}

fn get_profiles(manifest_path: &Utf8Path) -> Result<BTreeMap<String, CargoProfile>> {
    let mut profiles = CargoProfiles::new();
    let workspace_toml = load_root_cargo_toml(manifest_path)?;
    let Some(profiles_table) = &workspace_toml.get("profile").and_then(|t| t.as_table()) else {
        // No table, empty return
        return Ok(profiles)
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
