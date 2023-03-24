use crate::{PackageInfo, Result, SortedMap, WorkspaceInfo, WorkspaceKind};
use camino::Utf8Path;
use guppy::{
    graph::{BuildTargetId, DependencyDirection, PackageGraph, PackageMetadata},
    MetadataCommand,
};
use miette::{miette, Context, IntoDiagnostic};
use tracing::warn;

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

    /*
    // Get the [workspace.metadata.dist] table, which can be set either in a virtual
    // manifest or a root package (this code handles them uniformly).

       let mut workspace_config = workspace
           .metadata_table()
           .get(METADATA_DIST)
           .map(DistMetadata::deserialize)
           .transpose()
           .into_diagnostic()
           .wrap_err("couldn't parse [workspace.metadata.dist]")?
           .unwrap_or_default();

       let dist_profile = get_dist_profile(&manifest_path)
           .map_err(|e| {
               let err = e.wrap_err("failed to load [profile.dist] from toml");
               info!("{:?}", err);
           })
           .ok();
    */
    let workspace_root = workspace.root();
    /*
       workspace_config.make_relative_to(workspace_root);
    */
    let root_auto_includes = crate::find_auto_includes(workspace_root)?;

    let mut repo_url_conflicted = false;
    let mut repo_url = None;
    let mut all_package_info = SortedMap::new();
    for package in members.packages(DependencyDirection::Forward) {
        let mut info = package_info(workspace_root, /*&workspace_config,*/ &package)?;

        // Check for global settings on local packages
        /*
               if info.config.cargo_dist_version.is_some() {
                   warn!("package.metadata.dist.cargo-dist-version is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package.manifest_path());
               }
               if info.config.cargo_dist_version.is_some() {
                   warn!("package.metadata.dist.rust-toolchain-version is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package.manifest_path());
               }
               if !info.config.ci.is_empty() {
                   warn!("package.metadata.dist.ci is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package.manifest_path());
               }
        */
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

        all_package_info.insert(package.id().clone(), info);
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
        /*

        dist_profile,
        desired_cargo_dist_version: workspace_config.cargo_dist_version,
        desired_rust_toolchain: workspace_config.rust_toolchain_version,
        ci_kinds: workspace_config.ci,
        */
    })
}

fn package_info(
    _workspace_root: &Utf8Path,
    /*workspace_config: &DistMetadata,*/
    package: &PackageMetadata,
) -> Result<PackageInfo> {
    // Is there a better way to get the path to sniff?
    // Should we spider more than just package_root and workspace_root?
    // Should we more carefully prevent grabbing LICENSES from both dirs?
    // Should we not spider the workspace root for README since Cargo has a proper field for this?
    // Should we check for a "readme=..." on the workspace root Cargo.toml?
    let manifest_path = package.manifest_path().to_owned();
    let package_root = manifest_path
        .parent()
        .expect("package manifest had no parent!?")
        .to_owned();
    /*
       let mut package_config = package
           .metadata_table()
           .get(METADATA_DIST)
           .map(DistMetadata::deserialize)
           .transpose()
           .into_diagnostic()
           .wrap_err("couldn't parse [package.metadata.dist]")?
           .unwrap_or_default();
       package_config.make_relative_to(package_root);
       package_config.merge_workspace_config(workspace_config);
    */
    let mut binaries = vec![];
    for target in package.build_targets() {
        let build_id = target.id();
        if let BuildTargetId::Binary(name) = build_id {
            binaries.push(name.to_owned());
        }
    }

    let mut info = PackageInfo {
        name: package.name().to_owned(),
        version: Some(package.version().to_string()),
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
        /*config: package_config,*/
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
