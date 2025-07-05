use axoasset::{toml_edit, LocalAsset};
use axoproject::{WorkspaceInfo, WorkspaceKind};
use tracing::debug;

use crate::{config, errors::DistResult, METADATA_DIST};

mod from_v0;
use from_v0::do_migrate_from_v0;

pub fn needs_migration() -> DistResult<bool> {
    let workspaces = config::get_project()?;
    let root_workspace = workspaces.root_workspace();
    let initted = has_metadata_table(root_workspace);

    let using_dist_toml = root_workspace.kind == WorkspaceKind::Generic
        && initted
        && root_workspace.manifest_path.file_name() == Some("dist.toml");

    let using_cargo_toml = root_workspace.kind == WorkspaceKind::Rust && initted;

    Ok(using_dist_toml || using_cargo_toml)
}

/// Copy [workspace.metadata.dist] from one workspace to [dist] in another.
fn copy_cargo_workspace_metadata_dist(
    new_workspace: &mut toml_edit::DocumentMut,
    workspace_toml: toml_edit::DocumentMut,
) {
    if let Some(dist) = workspace_toml
        .get("workspace")
        .and_then(|t| t.get("metadata"))
        .and_then(|t| t.get("dist"))
    {
        new_workspace.insert("dist", dist.to_owned());
    }
}

/// Remove [workspace.metadata.dist], if it exists.
fn prune_cargo_workspace_metadata_dist(workspace: &mut toml_edit::DocumentMut) {
    workspace
        .get_mut("workspace")
        .and_then(|ws| ws.get_mut("metadata"))
        .and_then(|metadata_item| metadata_item.as_table_mut())
        .and_then(|table| table.remove("dist"));
}

/// Create a toml-edit document set up for a cargo workspace.
pub(crate) fn new_cargo_workspace() -> toml_edit::DocumentMut {
    let mut new_workspace = toml_edit::DocumentMut::new();

    // Write generic workspace config
    let mut table = toml_edit::table();
    if let Some(t) = table.as_table_mut() {
        let mut array = toml_edit::Array::new();
        array.push("cargo:.");
        t["members"] = toml_edit::value(array);
    }
    new_workspace.insert("workspace", table);

    new_workspace
}

/// Create a toml-edit document set up for a cargo workspace.
fn new_generic_workspace() -> toml_edit::DocumentMut {
    let mut new_workspace = toml_edit::DocumentMut::new();

    // Write generic workspace config
    let mut table = toml_edit::table();
    if let Some(t) = table.as_table_mut() {
        let mut array = toml_edit::Array::new();
        array.push("dist:.");
        t["members"] = toml_edit::value(array);
    }
    new_workspace.insert("workspace", table);

    new_workspace
}

fn do_migrate_from_rust_workspace() -> DistResult<()> {
    let workspaces = config::get_project()?;
    let root_workspace = workspaces.root_workspace();
    let initted = has_metadata_table(root_workspace);

    if root_workspace.kind != WorkspaceKind::Rust {
        // we're not using a Rust workspace, so no migration needed.
        return Ok(());
    }

    if !initted {
        // the workspace hasn't been initialized, so no migration needed.
        return Ok(());
    }

    eprintln!("migrating dist config from Cargo.toml to dist-workspace.toml...");

    // Load in the root workspace toml to edit and write back
    let workspace_toml = config::load_toml(&root_workspace.manifest_path)?;
    let mut original_workspace_toml = workspace_toml.clone();

    // Generate a new workspace, then populate it using config from Cargo.toml.
    let mut new_workspace_toml = new_cargo_workspace();
    copy_cargo_workspace_metadata_dist(&mut new_workspace_toml, workspace_toml);

    // Determine config file location.
    let filename = "dist-workspace.toml";
    let destination = root_workspace.workspace_dir.join(filename);

    // Write new config file.
    config::write_toml(&destination, new_workspace_toml)?;

    // We've been asked to migrate away from Cargo.toml; delete what
    // we've added after writing the new config
    prune_cargo_workspace_metadata_dist(&mut original_workspace_toml);
    config::write_toml(&root_workspace.manifest_path, original_workspace_toml)?;

    Ok(())
}

fn do_migrate_from_dist_toml() -> DistResult<()> {
    let workspaces = config::get_project()?;
    let root_workspace = workspaces.root_workspace();
    let initted = has_metadata_table(root_workspace);

    if !initted {
        return Ok(());
    }

    if root_workspace.kind != WorkspaceKind::Generic
        || root_workspace.manifest_path.file_name() != Some("dist.toml")
    {
        return Ok(());
    }

    eprintln!("migrating dist config from dist.toml to dist-workspace.toml...");

    // OK, now we know we have a root-level dist.toml. Time to fix that.
    let workspace_toml = config::load_toml(&root_workspace.manifest_path)?;

    // Init a generic workspace with the appropriate members
    let mut new_workspace_toml = new_generic_workspace();
    // First copy the [package] section
    if let Some(package) = workspace_toml.get("package") {
        let mut package = package.clone();
        // Ensures we have whitespace between the end of [workspace] and
        // the start of [package]
        if let Some(table) = package.as_table_mut() {
            let decor = table.decor_mut();
            // Try to keep existing comments if we can
            if let Some(desc) = decor.prefix().and_then(|p| p.as_str()) {
                if !desc.starts_with('\n') {
                    decor.set_prefix(format!("\n{desc}"));
                }
            } else {
                decor.set_prefix("\n");
            }
        }
        new_workspace_toml.insert("package", package.to_owned());
    }
    // ...then copy the [dist] section
    if let Some(dist) = workspace_toml.get("dist") {
        new_workspace_toml.insert("dist", dist.to_owned());
    }

    // Finally, write out the new config...
    let filename = "dist-workspace.toml";
    let destination = root_workspace.workspace_dir.join(filename);
    config::write_toml(&destination, new_workspace_toml)?;
    // ...and delete the old config
    LocalAsset::remove_file(&root_workspace.manifest_path)?;

    Ok(())
}

/// Run `dist migrate`
pub fn do_migrate() -> DistResult<()> {
    do_migrate_from_rust_workspace()?;
    do_migrate_from_dist_toml()?;
    debug!("dist.config-version = {}", config::get_version()?);
    if config::want_v1()? {
        do_migrate_from_v0()?;
    }
    Ok(())
}

pub fn has_metadata_table(workspace_info: &WorkspaceInfo) -> bool {
    if workspace_info.kind == WorkspaceKind::Rust {
        // Setup [workspace.metadata.dist]
        workspace_info
            .cargo_metadata_table
            .as_ref()
            .and_then(|t| t.as_object())
            .map(|t| t.contains_key(METADATA_DIST))
            .unwrap_or(false)
    } else {
        config::parse_metadata_table_or_manifest(
            &workspace_info.manifest_path,
            workspace_info.dist_manifest_path.as_deref(),
            workspace_info.cargo_metadata_table.as_ref(),
        )
        .is_ok()
    }
}
