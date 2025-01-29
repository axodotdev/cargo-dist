pub(crate) mod v0;
pub use v0::do_init as do_init_v0;
mod apply_dist;
pub mod console_helpers;
mod dist_profile;
mod init_args;
mod interactive;

use console_helpers::theme;
use crate::{do_generate, GenerateArgs};
use crate::SortedMap;
use crate::config::{self, Config, v1::TomlLayer};
use crate::errors::DistResult;
use crate::migrate;
pub use dist_profile::init_dist_profile;
pub use init_args::InitArgs;

/// Run 'dist init'
pub fn do_init(cfg: &Config, args: &InitArgs) -> DistResult<()> {
    if !config::want_v1()? {
        return do_init_v0(cfg, args);
    }

    let ctrlc_handler = console_helpers::ctrlc_handler();

    if migrate::needs_migration()? && !args.yes {
        let prompt = r#"Would you like to opt in to the new configuration format?
    Future versions of dist will feature major changes to the configuration format."#;
        let is_migrating = dialoguer::Confirm::with_theme(&theme())
            .with_prompt(prompt)
            .default(false)
            .interact()?;

        if is_migrating {
            migrate::do_migrate()?;
            return do_init(cfg, args);
        }
    }

    eprintln!("let's setup your dist config...");
    eprintln!();

    let check = console_helpers::checkmark();

    let workspaces = config::get_project()?;

    // For each [workspace] Cargo.toml in the workspaces, initialize [profile]
    let mut did_add_profile = false;
    for workspace_idx in workspaces.all_workspace_indices() {
        let workspace = workspaces.workspace(workspace_idx);
        // TODO(migration): re-implement this.
        /*if workspace.kind == WorkspaceKind::Rust {
            let mut workspace_toml = config::load_toml(&workspace.manifest_path)?;
            did_add_profile |= init_dist_profile(cfg, &mut workspace_toml)?;
            config::write_toml(&workspace.manifest_path, workspace_toml)?;
        }*/
    }

    if did_add_profile {
        eprintln!("{check} added [profile.dist] to your workspace Cargo.toml");
    }


    // TODO(migration): should this handle JSON stuff? if so, what does that
    // look like now that DistMetadata isn't being used?
    let meta = interactive::get_new_metadata(cfg, args, &workspaces)?;
    let mut workspace = meta;
    let mut packages: SortedMap<String, TomlLayer> = SortedMap::new();

    // We're past the final dialoguer call; we can remove the ctrl-c handler.
    ctrlc_handler.abort();

    let root_workspace = workspaces.root_workspace();

    // Load in the root workspace toml to edit and write back
    let mut workspace_toml = config::load_toml(&root_workspace.manifest_path)?;

    apply_dist::apply_dist_to_workspace_toml(&mut workspace_toml, &workspace);

    eprintln!();

    let filename = root_workspace
        .manifest_path
        .file_name()
        .unwrap_or("dist-workspace.toml");
    let destination = root_workspace.manifest_path.to_owned();

    // Save the workspace toml (potentially an effective no-op if we made no edits)
    config::write_toml(&destination, workspace_toml)?;
    let key = "[dist]";
    eprintln!("{check} added {key} to your root {filename}");

    // Now that we've done the stuff that's definitely part of the root Cargo.toml,
    // Optionally apply updates to packages
    for (_idx, package) in workspaces.all_packages() {
        // Gather up all the things we'd like to be written to this file
        let meta = packages.get(&package.name);
        let needs_edit = meta.is_some();

        if needs_edit {
            // Ok we have changes to make, let's load the toml
            let mut package_toml = config::load_toml(&package.manifest_path)?;
            let metadata = config::get_toml_metadata(&mut package_toml, false);

            // Apply [package.metadata.dist]
            let mut writing_metadata = false;
            if let Some(meta) = meta {
                apply_dist::apply_dist_to_metadata(metadata, meta);
                writing_metadata = true;
            }

            // Save the result
            config::write_toml(&package.manifest_path, package_toml)?;
            if writing_metadata {
                eprintln!(
                    "{check} added [package.metadata.dist] to {}'s Cargo.toml",
                    package.name
                );
            }
        }
    }

    eprintln!("{check} dist is setup!");
    eprintln!();

    // regenerate anything that needs to be
    if !args.no_generate {
        eprintln!("running 'dist generate' to apply any changes");
        eprintln!();

        let ci_args = GenerateArgs {
            check: false,
            modes: vec![],
        };
        do_generate(cfg, &ci_args)?;
    }
    Ok(())
}
