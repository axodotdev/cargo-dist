use axoasset::{toml, toml_edit, LocalAsset};
use axoproject::{WorkspaceGraph, WorkspaceInfo, WorkspaceKind};
use camino::Utf8PathBuf;
use cargo_dist_schema::TripleNameRef;
use semver::Version;
use serde::Deserialize;

use crate::{
    config::{
        self, CiStyle, Config, DistMetadata, HostingStyle, InstallPathStrategy, InstallerStyle,
        MacPkgConfig, PublishStyle,
        v1::{
            builds::BuildLayer,
            layer::BoolOr,
            TomlLayer,
        },
    },
    do_generate,
    errors::{DistError, DistResult},
    platform::{triple_to_display_name, MinGlibcVersion},
    GenerateArgs, SortedMap, METADATA_DIST, PROFILE_DIST,
};

/// Arguments for `dist init` ([`do_init`][])
#[derive(Debug)]
pub struct InitArgs {
    /// Whether to auto-accept the default values for interactive prompts
    pub yes: bool,
    /// Don't automatically generate ci
    pub no_generate: bool,
    /// A path to a json file containing values to set in workspace.metadata.dist
    pub with_json_config: Option<Utf8PathBuf>,
    /// Hosts to enable
    pub host: Vec<HostingStyle>,
}

/// Input for --with-json-config
///
/// Contains a DistMetadata for the workspace.metadata.dist and
/// then optionally ones for each package.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MultiDistMetadata {
    /// `[workspace.metadata.dist]`
    workspace: Option<TomlLayer>,
    /// package_name => `[package.metadata.dist]`
    #[serde(default)]
    packages: SortedMap<String, TomlLayer>,
}

fn theme() -> dialoguer::theme::ColorfulTheme {
    dialoguer::theme::ColorfulTheme {
        checked_item_prefix: console::style("  [x]".to_string()).for_stderr().green(),
        unchecked_item_prefix: console::style("  [ ]".to_string()).for_stderr().dim(),
        active_item_style: console::Style::new().for_stderr().cyan().bold(),
        ..dialoguer::theme::ColorfulTheme::default()
    }
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
fn new_cargo_workspace() -> toml_edit::DocumentMut {
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

    // OK, now we know we have a root-level dist.toml. Time to fix that.
    let workspace_toml = config::load_toml(&root_workspace.manifest_path)?;

    eprintln!("Migrating tables");
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
                    decor.set_prefix(&format!("\n{desc}"));
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

fn do_migrate_from_v0() -> DistResult<()> {
    let workspaces = config::get_project()?;
    let root_workspace = workspaces.root_workspace();
    let manifest_path = &root_workspace.manifest_path;

    if config::load_config(manifest_path).is_ok() {
        // We're already on a V1 config, no need to migrate!
        return Ok(());
    }

    // Load in the root workspace toml to edit and write back
    let Ok(old_config) = config::load_v0_config(manifest_path) else {
        // We don't have a valid v0 _or_ v1 config. No migration can be done.
        // It feels weird to return Ok(()) here, but I think it's right?
        return Ok(());
    };

    let Some(dist_metadata) = &old_config.dist else {
        // We don't have a valid v0 config. No migration can be done.
        return Ok(());
    };

    let dist = dist_metadata.to_toml_layer(true);

    let workspace = old_config.workspace;
    let package = None;

    let config = config::v1::DistWorkspaceConfig {
        dist,
        workspace,
        package,
    };

    let workspace_toml_text = toml::to_string(&config)?;

    // Write new config file.
    axoasset::LocalAsset::write_new(&workspace_toml_text, manifest_path)?;

    Ok(())
}

/// Run `dist migrate`
pub fn do_migrate() -> DistResult<()> {
    do_migrate_from_rust_workspace()?;
    do_migrate_from_dist_toml()?;
    do_migrate_from_v0()?;
    Ok(())
}

/// Run 'dist init'
pub fn do_init(cfg: &Config, args: &InitArgs) -> DistResult<()> {
    // on ctrl-c,  dialoguer/console will clean up the rest of its
    // formatting, but the cursor will remain hidden unless we
    // explicitly go in and show it again
    // See: https://github.com/console-rs/dialoguer/issues/294
    let ctrlc_handler = tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();

        let term = console::Term::stdout();
        // Ignore the error here if there is any, this is best effort
        let _ = term.show_cursor();

        // Immediately re-exit the process with the same
        // exit code the unhandled ctrl-c would have used
        let exitstatus = if cfg!(windows) {
            0xc000013a_u32 as i32
        } else {
            130
        };
        std::process::exit(exitstatus);
    });

    let workspaces = config::get_project()?;
    let root_workspace = workspaces.root_workspace();
    let check = console::style("✔".to_string()).for_stderr().green();

    eprintln!("let's setup your dist config...");
    eprintln!();

    // For each [workspace] Cargo.toml in the workspaces, initialize [profile]
    let mut did_add_profile = false;
    for workspace_idx in workspaces.all_workspace_indices() {
        let workspace = workspaces.workspace(workspace_idx);
        if workspace.kind == WorkspaceKind::Rust {
            let mut workspace_toml = config::load_toml(&workspace.manifest_path)?;
            did_add_profile |= init_dist_profile(cfg, &mut workspace_toml)?;
            config::write_toml(&workspace.manifest_path, workspace_toml)?;
        }
    }

    if did_add_profile {
        eprintln!("{check} added [profile.dist] to your workspace Cargo.toml");
    }

    // Load in the root workspace toml to edit and write back
    let workspace_toml = config::load_toml(&root_workspace.manifest_path)?;
    let initted = has_metadata_table(root_workspace);

    if root_workspace.kind == WorkspaceKind::Generic
        && initted
        && root_workspace.manifest_path.file_name() == Some("dist.toml")
    {
        do_migrate()?;
        return do_init(cfg, args);
    }

    // Already-initted users should be asked whether to migrate.
    if root_workspace.kind == WorkspaceKind::Rust && initted && !args.yes {
        let prompt = r#"Would you like to opt in to the new configuration format?
    Future versions of dist will feature major changes to the
    configuration format, including a new dist-specific configuration file."#;
        let is_migrating = dialoguer::Confirm::with_theme(&theme())
            .with_prompt(prompt)
            .default(false)
            .interact()?;

        if is_migrating {
            do_migrate()?;
            return do_init(cfg, args);
        }
    }
//=====================
    // If this is a Cargo.toml, offer to either write their config to
    // a dist-workspace.toml, or migrate existing config there
    let mut newly_initted_generic = false;
    // Users who haven't initted yet should be opted into the
    // new config format by default.
    let desired_workspace_kind = if root_workspace.kind == WorkspaceKind::Rust && !initted {
        newly_initted_generic = true;
        WorkspaceKind::Generic
    } else {
        root_workspace.kind
    };

    let multi_meta = if let Some(json_path) = &args.with_json_config {
        // json update path, read from a file and apply all requested updates verbatim
        let src = axoasset::SourceFile::load_local(json_path)?;
        let multi_meta: MultiDistMetadata = src.deserialize_json()?;
        multi_meta
    } else {
        // run (potentially interactive) init logic
        let meta = get_new_dist_metadata(cfg, args, &workspaces)?;
        MultiDistMetadata {
            workspace: Some(meta),
            packages: SortedMap::new(),
        }
    };

    // We're past the final dialoguer call; we can remove the
    // ctrl-c handler.
    ctrlc_handler.abort();

    // If we're migrating, the configuration will be missing the
    // generic workspace specification, and will have some
    // extraneous cargo-specific stuff that we don't want.
    let mut workspace_toml = if newly_initted_generic {
        new_cargo_workspace()
    } else {
        workspace_toml
    };

    if let Some(meta) = &multi_meta.workspace {
        apply_dist_to_workspace_toml(&mut workspace_toml, desired_workspace_kind, meta);
    }

    eprintln!();

    let filename;
    let destination;
    if newly_initted_generic {
        // Migrations and newly-initted setups always use dist-workspace.toml.
        filename = "dist-workspace.toml";
        destination = root_workspace.workspace_dir.join(filename);
    } else {
        filename = root_workspace
            .manifest_path
            .file_name()
            .expect("no filename!?");
        destination = root_workspace.manifest_path.to_owned();
    };

    // Save the workspace toml (potentially an effective no-op if we made no edits)
    config::write_toml(&destination, workspace_toml)?;
    let key = if desired_workspace_kind == WorkspaceKind::Rust {
        "[workspace.metadata.dist]"
    } else {
        "[dist]"
    };
    eprintln!("{check} added {key} to your root {filename}");

    // Now that we've done the stuff that's definitely part of the root Cargo.toml,
    // Optionally apply updates to packages
    for (_idx, package) in workspaces.all_packages() {
        // Gather up all the things we'd like to be written to this file
        let meta = multi_meta.packages.get(&package.name);
        let needs_edit = meta.is_some();

        if needs_edit {
            // Ok we have changes to make, let's load the toml
            let mut package_toml = config::load_toml(&package.manifest_path)?;
            let metadata = config::get_toml_metadata(&mut package_toml, false);

            // Apply [package.metadata.dist]
            let mut writing_metadata = false;
            if let Some(meta) = meta {
                apply_dist_to_metadata(metadata, meta);
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

fn init_dist_profile(
    _cfg: &Config,
    workspace_toml: &mut toml_edit::DocumentMut,
) -> DistResult<bool> {
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
        // For some detailed discussion, see: https://github.com/axodotdev/cargo-dist/issues/118
        let new_profile = new_profile.as_table_mut().unwrap();
        // We're building for release, so this is a good base!
        new_profile.insert("inherits", toml_edit::value("release"));
        // We're building for SUPER DUPER release, so lto is a good idea to enable!
        //
        // There's a decent argument for lto=true (aka "fat") here but the cost-benefit
        // is a bit complex. Fat LTO can be way more expensive to compute (to the extent
        // that enormous applications like chromium can become unbuildable), but definitely
        // eeks out a bit more from your binaries.
        //
        // In principle dist is targeting True Shippable Binaries and so it's
        // worth it to go nuts getting every last drop out of your binaries... but a lot
        // of people are going to build binaries that might never even be used, so really
        // we're just burning a bunch of CI time for nothing.
        //
        // The user has the freedom to crank this up higher (and/or set codegen-units=1)
        // if they think it's worth it, but we otherwise probably shouldn't set the planet
        // on fire just because Number Theoretically Go Up.
        new_profile.insert("lto", toml_edit::value("thin"));
        new_profile
            .decor_mut()
            .set_prefix("\n# The profile that 'dist' will build with\n")
    }
    dist_profile.or_insert(new_profile);

    Ok(true)
}

fn has_metadata_table(workspace_info: &WorkspaceInfo) -> bool {
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

/// Initialize [workspace.metadata.dist] with default values based on what was passed on the CLI
///
/// Returns whether the initialization was actually done
/// and whether ci was set
fn get_new_dist_metadata(
    cfg: &Config,
    args: &InitArgs,
    workspaces: &WorkspaceGraph,
) -> DistResult<TomlLayer> {
    use dialoguer::{Confirm, Input, MultiSelect};
    let root_workspace = workspaces.root_workspace();
    let has_config = has_metadata_table(root_workspace);

    let mut meta = if has_config {
        config::load_config(&root_workspace.manifest_path)?.dist
    } else {
        TomlLayer {
            // If they init with this version we're gonna try to stick to it!
            dist_version: Some(std::env!("CARGO_PKG_VERSION").parse().unwrap()),
            dist_url_override: None,
            dist: None,
            allow_dirty: None,
            targets: None,
            artifacts: None,
            builds: None,
            ci: None,
            hosts: None,
            installers: None,
            publishers: None,
        }
    };

    // Clone this to simplify checking for settings changes
    let orig_meta = meta.clone();

    // Now prompt the user interactively to initialize these...

    // Tune the theming a bit
    let theme = theme();
    // Some indicators we'll use in a few places
    let check = console::style("✔".to_string()).for_stderr().green();
    let notice = console::style("⚠️".to_string()).for_stderr().yellow();

    let github_hosting = !args.host.is_empty() && args.host.contains(&HostingStyle::Github);
    let axo_hosting = !args.host.is_empty() && args.host.contains(&HostingStyle::Axodotdev);

    // Set cargo-dist-version
    let current_version: Version = std::env!("CARGO_PKG_VERSION").parse().unwrap();
    if let Some(desired_version) = &meta.dist_version {
        if desired_version != &current_version && !desired_version.pre.starts_with("github-") {
            let default = true;
            let prompt = format!(
                r#"update your project to this version of dist?
    {} => {}"#,
                desired_version, current_version
            );
            let response = if args.yes {
                default
            } else {
                let res = Confirm::with_theme(&theme)
                    .with_prompt(prompt)
                    .default(default)
                    .interact()?;
                eprintln!();
                res
            };

            if response {
                meta.dist_version = Some(current_version);
            } else {
                Err(DistError::NoUpdateVersion {
                    project_version: desired_version.clone(),
                    running_version: current_version,
                })?;
            }
        }
    } else {
        // Really not allowed, so just force them onto the current version
        meta.dist_version = Some(current_version);
    }

    {
        // Start with builtin targets
        let default_platforms = crate::default_desktop_targets();
        let mut known = crate::known_desktop_targets();
        // If the config doesn't have targets at all, generate them
        let config_vals = meta.targets.as_deref().unwrap_or(&default_platforms);
        let cli_vals = cfg.targets.as_slice();
        // Add anything custom they did to the list (this will do some reordering if they hand-edited)
        for val in config_vals.iter().chain(cli_vals) {
            if !known.contains(val) {
                known.push(val.clone());
            }
        }

        // Prettify/sort things
        let desc = move |triple: &TripleNameRef| -> String {
            let pretty = triple_to_display_name(triple).unwrap_or("[unknown]");
            format!("{pretty} ({triple})")
        };
        known.sort_by_cached_key(|k| desc(k).to_uppercase());

        let mut defaults = vec![];
        let mut keys = vec![];
        for item in &known {
            // If this target is in their config, keep it
            // If they passed it on the CLI, flip it on
            let config_had_it = config_vals.contains(item);
            let cli_had_it = cli_vals.contains(item);

            let default = config_had_it || cli_had_it;
            defaults.push(default);

            keys.push(desc(item));
        }

        // Prompt the user
        let prompt = r#"what platforms do you want to build for?
    (select with arrow keys and space, submit with enter)"#;
        let selected = if args.yes {
            defaults
                .iter()
                .enumerate()
                .filter_map(|(idx, enabled)| enabled.then_some(idx))
                .collect()
        } else {
            let res = MultiSelect::with_theme(&theme)
                .items(&keys)
                .defaults(&defaults)
                .with_prompt(prompt)
                .interact()?;
            eprintln!();
            res
        };

        // Apply the results
        meta.targets = Some(selected.into_iter().map(|i| known[i].clone()).collect());
    }

    // Enable CI backends
    // FIXME: when there is more than one option we maybe shouldn't hide this
    // once the user has any one enabled, right now it's just annoying to always
    // prompt for Github CI support.
    if meta.ci.is_none() {


        // FIXME: when there is more than one option this should be a proper
        // multiselect like the installer selector is! For now we do
        // most of the multi-select logic and then just give a prompt.
        /*let known = &[CiStyle::Github];
        let mut defaults = vec![];
        let mut keys = vec![];
        let mut github_key = 0;
        for item in known {
            // If this CI style is in their config, keep it
            // If they passed it on the CLI, flip it on
            let mut default = meta
                .ci
                .as_ref()
                .map(|ci| ci.contains(item))
                .unwrap_or(false)
                || cfg.ci.contains(item);

            // Currently default to enabling github CI because we don't
            // support anything else and we can give a good error later
            #[allow(irrefutable_let_patterns)]
            if let CiStyle::Github = item {
                github_key = 0;
                default = true;
            }
            defaults.push(default);
            // This match is here to remind you to add new CiStyles
            // to `known` above!
            keys.push(match item {
                CiStyle::Github => "github",
            });
        }*/

        // Prompt the user
        let prompt = r#"enable Github CI and Releases?"#;
        let default_value = true;

        let github_selected = if args.yes {
            default_value
        } else {
            let res = Confirm::with_theme(&theme)
                .with_prompt(prompt)
                .default(default_value)
                .interact()?;
            eprintln!();
            res
        };

        if github_selected {
            meta.ci.as_ref().map(|ci| ci.github = Some(BoolOr::Bool(true)));
        }
    }

    // Enable installer backends (if they have a CI backend that can provide URLs)
    // FIXME: "vendored" installers like msi could be enabled without any CI...
    //let has_ci = meta.ci.as_ref().map(|ci| !ci.is_empty()).unwrap_or(false);
    let has_ci = meta.ci.is_some_and(|ci|
        ci.github.is_some_and(|gh| gh.truthy())
    );

    let existing_shell_config = meta.installers.is_some_and(|ins| ins.shell.is_some_and(|sh| sh.truthy()));
    let existing_powershell_config = meta.installers.is_some_and(|ins| ins.powershell.is_some_and(|ps| ps.truthy()));
    let existing_npm_config = meta.installers.is_some_and(|ins| ins.npm.is_some_and(|npm| npm.truthy()));
    let existing_homebrew_config = meta.installers.is_some_and(|ins| ins.homebrew.is_some_and(|hb| hb.truthy()));
    let existing_msi_config = meta.installers.is_some_and(|ins| ins.msi.is_some_and(|msi| msi.truthy()));
    let existing_pkg_config = meta.installers.is_some_and(|ins| ins.pkg.is_some_and(|pkg| pkg.truthy()));

    {

        // If they have CI, then they can use fetching installers,
        // otherwise they can only do vendored installers.
        let known: &[InstallerStyle] = if has_ci {
            &[
                InstallerStyle::Shell,
                InstallerStyle::Powershell,
                InstallerStyle::Npm,
                InstallerStyle::Homebrew,
                InstallerStyle::Msi,
                // Pkg intentionally left out because it's currently opt-in only.
            ]
        } else {
            eprintln!("{notice} no CI backends enabled, most installers have been hidden");
            &[InstallerStyle::Msi]
        };

        let mut defaults: SortedMap<&str, bool> = SortedMap::new();
        defaults.insert("shell",
            existing_shell_config || cfg.installers.contains(&InstallerStyle::Shell)
        );
        defaults.insert("powershell",
            existing_powershell_config || cfg.installers.contains(&InstallerStyle::Powershell)
        );
        defaults.insert("npm",
            existing_npm_config || cfg.installers.contains(&InstallerStyle::Npm)
        );
        defaults.insert("homebrew",
            existing_homebrew_config || cfg.installers.contains(&InstallerStyle::Homebrew)
        );
        defaults.insert("msi",
            existing_msi_config || cfg.installers.contains(&InstallerStyle::Msi)
        );
        defaults.insert("pkg",
            existing_pkg_config || cfg.installers.contains(&InstallerStyle::Pkg)
        );

        let keys: Vec<&str> = defaults.keys().cloned().collect();

        // Prompt the user
        let prompt = r#"what installers do you want to build?
    (select with arrow keys and space, submit with enter)"#;
        let selected = if args.yes {
            defaults
                .iter()
                .enumerate()
                .filter_map(|(idx, (_, enabled))| enabled.then_some(idx))
                .collect()
        } else {
            let default_values: Vec<bool> = defaults.values().cloned().collect();

            let res = MultiSelect::with_theme(&theme)
                .items(&keys)
                .defaults(&default_values)
                .with_prompt(prompt)
                .interact()?;
            eprintln!();
            res
        };

        // Apply the results
        meta.installers = Some(meta.installers.unwrap_or_default());

        meta.installers.map(|mut installers| {
            for item in selected {
                match keys[item] {
                    "shell" => {
                        installers.shell = installers.shell.or(Some(BoolOr::Bool(true)));
                    }
                    "powershell" => {
                        installers.powershell = installers.powershell.or(Some(BoolOr::Bool(true)));
                    }
                    "npm" => {
                        installers.npm = installers.npm.or(Some(BoolOr::Bool(true)));
                    }
                    "homebrew" => {
                        installers.homebrew = installers.homebrew.or(Some(BoolOr::Bool(true)));
                    }
                    "msi" => {
                        installers.msi = installers.msi.or(Some(BoolOr::Bool(true)));
                    }
                    "pkg" => {
                        installers.pkg = installers.pkg.or(Some(BoolOr::Bool(true)));
                    }
                    _ => {
                        // This should be enforced at the type level, ideally.
                        unreachable!("got an unknown installer type -- this is a dist bug, please report it");
                    }
                }
            }
        });
    }

    // Special handling of the Homebrew installer
    if meta.installers.is_some_and(|ins| ins.homebrew.is_some_and(|hb| hb.truthy())) {
        let homebrew_is_new = !existing_homebrew_config;

        if homebrew_is_new {
            let prompt = r#"you've enabled Homebrew support; if you want dist
    to automatically push package updates to a tap (repository) for you,
    please enter the tap name (in GitHub owner/name format)"#;
            let default = "".to_string();

            let tap: String = if args.yes {
                default
            } else {
                let res = Input::with_theme(&theme)
                    .with_prompt(prompt)
                    .allow_empty(true)
                    .interact_text()?;
                eprintln!();
                res
            };
            let tap = tap.trim();
            if tap.is_empty() {
                eprintln!("Homebrew packages will not be automatically published");
                meta.installers.map(|mut ins| ins.homebrew = None);
            } else {
                let installers = meta.installers.unwrap_or_default();
                let homebrew = match installers.homebrew.unwrap_or(BoolOr::Bool(true)) {
                    BoolOr::Val(v) => v,
                    // The hb.truthy() condition above means this should never be false.
                    BoolOr::Bool(_b) => Default::default(),
                };

                homebrew.tap = Some(tap.to_owned());

                installers.homebrew = Some(BoolOr::Val(homebrew));
                meta.installers = Some(installers);

                eprintln!("{check} Homebrew package will be published to {tap}");

                eprintln!(
                    r#"{check} You must provision a GitHub token and expose it as a secret named
    HOMEBREW_TAP_TOKEN in GitHub Actions. For more information,
    see the documentation:
    https://opensource.axo.dev/cargo-dist/book/installers/homebrew.html"#
                );
            }
        }
    } else {
        let homebrew_toggled_off = existing_homebrew_config;

        if homebrew_toggled_off {
            meta.installers.map(|mut ins| ins.homebrew = None);
        }
    }

    // Special handling of the npm installer
    if meta.installers.is_some_and(|ins| ins.npm.is_some_and(|npm| npm.truthy())) {
        // If npm is being newly enabled here, prompt for a @scope
        let npm_is_new = !existing_npm_config;
        if npm_is_new {
            let prompt = r#"you've enabled npm support, please enter the @scope you want to use
    this is the "namespace" the package will be published under
    (leave blank to publish globally)"#;
            let default = "".to_string();

            let scope: String = if args.yes {
                default
            } else {
                let res = Input::with_theme(&theme)
                    .with_prompt(prompt)
                    .allow_empty(true)
                    .validate_with(|v: &String| {
                        let v = v.trim();
                        if v.is_empty() {
                            Ok(())
                        } else if v != v.to_ascii_lowercase() {
                            Err("npm scopes must be lowercase")
                        } else if let Some(v) = v.strip_prefix('@') {
                            if v.is_empty() {
                                Err("@ must be followed by something")
                            } else {
                                Ok(())
                            }
                        } else {
                            Err("npm scopes must start with @")
                        }
                    })
                    .interact_text()?;
                eprintln!();
                res
            };
            let scope = scope.trim();

            meta.installers = Some(meta.installers.unwrap_or_default());

            meta.installers.map(|mut installers| {
                // unwrap() is okay because we use .unwrap_or_default() immediately above.
                let mut npm = match installers.npm.unwrap_or(BoolOr::Bool(true)) {
                    BoolOr::Val(v) => v,
                    // The npm.truthy() condition above means this should never be false.
                    BoolOr::Bool(_b) => Default::default(),
                };

                if scope.is_empty() {
                    eprintln!("{check} npm packages will be published globally");
                    npm.scope = None;
                } else {
                    npm.scope = Some(scope.to_owned());
                    eprintln!("{check} npm packages will be published under {scope}");
                }

                installers.npm = Some(BoolOr::Val(npm));
            });

            eprintln!();
        }
    } else {
        // Remove the npm installer configuration.
        meta.installers.map(|mut ins| ins.npm = None);

        // Remove the npm publisher configuration.
        meta.publishers.map(|mut pubs| pubs.npm = None);
    }

    meta.publishers =
        if meta.publishers.is_some_and(|p| p.homebrew.is_some() || p.npm.is_some()) {
            meta.publishers
        } else {
            None
        };

    if let Some(installers) = &meta.installers {
        if installers.shell.is_some() || installers.powershell.is_some() {
            // default to the current value if there is one, or false otherwise.
            let default = installers.updater.unwrap_or(false);
            let install_updater = if args.yes {
                default
            } else {
                let prompt = r#"Would you like to include an updater program with your binaries?"#;
                let res = Confirm::with_theme(&theme)
                    .with_prompt(prompt)
                    .default(default)
                    .interact()?;
                eprintln!();

                res
            };

            installers.updater = Some(install_updater);
        }
    }

    Ok(meta)
}

/// Update a workspace toml-edit document with the current DistMetadata value
pub(crate) fn apply_dist_to_workspace_toml(
    workspace_toml: &mut toml_edit::DocumentMut,
    _workspace_kind: WorkspaceKind,
    meta: &TomlLayer,
) {
    let metadata = workspace_toml.as_item_mut();
    apply_dist_to_metadata(metadata, meta);
}

/// Ensure [dist] has the given values
fn apply_dist_to_metadata(metadata: &mut toml_edit::Item, meta: &TomlLayer) {
    let dist_metadata = &mut metadata[METADATA_DIST];

    // If there's no table, make one
    if !dist_metadata.is_table() {
        *dist_metadata = toml_edit::table();
    }

    // Apply formatted/commented values
    let table = dist_metadata.as_table_mut().unwrap();

    // This is intentionally written awkwardly to make you update this
    let TomlLayer {
        dist_version,
        dist_url_override,
        dist,
        allow_dirty,
        targets,
        artifacts,
        builds,
        ci,
        hosts,
        installers,
        publishers,
    } = &meta;

/*
    // Forcibly inline the default install_path if not specified,
    // and if we've specified a shell or powershell installer
    let install_path = if install_path.is_none()
        && installers
            .as_ref()
            .map(|i| {
                i.iter()
                    .any(|el| matches!(el, InstallerStyle::Shell | InstallerStyle::Powershell))
            })
            .unwrap_or(false)
    {
        Some(InstallPathStrategy::default_list())
    } else {
        install_path.clone()
    };
*/

    apply_optional_value(
        table,
        "dist-version",
        "# The preferred dist version to use in CI (Cargo.toml SemVer syntax)\n",
        dist_version.as_ref().map(|v| v.to_string()),
    );

    apply_optional_value(
        table,
        "dist-url-override",
        "# A URL to use to install `cargo-dist` (with the installer script)\n",
        dist_url_override.as_ref().map(|v| v.to_string()),
    );

    apply_optional_value(
        table,
        "dist",
        "# Whether the package should be distributed/built by dist (defaults to true)\n",
        dist.clone(),
    );

    apply_string_list(
        table,
        "allow-dirty",
        "# Skip checking whether the specified configuration files are up to date\n",
        allow_dirty.as_ref(),
    );

    //apply_targets(table, targets);
    //apply_artifacts(table, artifacts);
    apply_builds(table, builds);
    //apply_ci(table, ci);
    //apply_hosts(table, hosts);
    //apply_installers(table, installers);
    //apply_publishers(table, publishers);

    if let Some(installers) = installers {
        // InstallerLayer
/*
        if let Some(homebrew) = &installers.homebrew {
            match homebrew {
                BoolOr::Bool(b) => {
                    apply_optional_value(
                        installers_table,
                        "homebrew",
                        "# Whether to build a Homebrew installer",
                        installers.updater.clone(),
                    );
                }
                BoolOr::Val(v) => {
                    // HomebrewInstallerLayer

                }
            }
        }

        if let Some(msi) = &installers.msi {
            match msi {
                BoolOr::Bool(b) => {
                    /* handle bool */
                }
                BoolOr::Val(v) => {
                    /* handle MsiInstallerLayer */
                }
            }
        }

        if let Some(npm) = &installers.npm {
            match npm {
                BoolOr::Bool(b) => {
                    /* handle bool */
                }
                BoolOr::Val(v) => {
                    /* handle NpmInstallerLayer */
                }
            }
        }

        if let Some(powershell) = &installers.powershell {
            match powershell {
                BoolOr::Bool(b) => {
                    // handle bool
                }
                BoolOr::Val(v) => {
                    // PowershellInstallerLayer
                }
            }
        }

        if let Some(shell) = &installers.shell {
            match shell {
                BoolOr::Bool(b) => {
                    // handle bool
                }
                BoolOr::Val(v) => {
                    // ShellInstallerLayer
                }
            }
        }

        if let Some(pkg) = &installers.pkg {
            match pkg {
                BoolOr::Bool(b) => {
                    apply_optional_value(
                        installers_table,
                        "pkg",
                        "\n# Configuration for the Mac .pkg installer\n",
                        Some(b),
                    );
                }
                BoolOr::Val(v) => {
                    // PkgInstallerLayer
                    apply_optional_mac_pkg(
                        installers_table,
                        "pkg",
                        "\n# Configuration for the Mac .pkg installer\n",
                        Some(v).as_ref(),
                    );
                }
            }
        }

        // installer.updater: Option<Bool>
        // installer.always_use_latest_updater: Option<bool>
        apply_optional_value(
            installers_table,
            "updater",
            "# Whether to install an updater program alongside the software",
            installers.updater.clone(),
        );

        apply_optional_value(
            installers_table,
            "always-use-latest-updater",
            "# Whether to always use the latest updater version instead of a fixed version",
            installers.always_use_latest_updater.clone(),
        );
*/
    }


/*
    apply_string_or_list(table, "ci", "# CI backends to support\n", ci.as_ref());

    apply_string_list(
        table,
        "installers",
        "# The installers to generate for each app\n",
        installers.as_ref(),
    );

    apply_optional_value(
        table,
        "tap",
        "# A GitHub repo to push Homebrew formulas to\n",
        tap.clone(),
    );

    apply_optional_value(
        table,
        "formula",
        "# Customize the Homebrew formula name\n",
        formula.clone(),
    );

    apply_string_list(
        table,
        "targets",
        "# Target platforms to build apps for (Rust target-triple syntax)\n",
        targets.as_ref(),
    );

    apply_optional_value(
        table,
        "dist",
        "# Whether to consider the binaries in a package for distribution (defaults true)\n",
        *dist,
    );

    apply_string_list(
        table,
        "include",
        "# Extra static files to include in each App (path relative to this Cargo.toml's dir)\n",
        include.as_ref(),
    );

    apply_optional_value(
        table,
        "auto-includes",
        "# Whether to auto-include files like READMEs, LICENSEs, and CHANGELOGs (default true)\n",
        *auto_includes,
    );

    apply_optional_value(
        table,
        "windows-archive",
        "# The archive format to use for windows builds (defaults .zip)\n",
        windows_archive.map(|a| a.ext()),
    );

    apply_optional_value(
        table,
        "unix-archive",
        "# The archive format to use for non-windows builds (defaults .tar.xz)\n",
        unix_archive.map(|a| a.ext()),
    );

    apply_optional_value(
        table,
        "npm-package",
        "# The npm package should have this name\n",
        npm_package.as_deref(),
    );

    apply_optional_value(
        table,
        "install-success-msg",
        "# Custom message to display on successful install\n",
        install_success_msg.as_deref(),
    );

    apply_optional_value(
        table,
        "npm-scope",
        "# A namespace to use when publishing this package to the npm registry\n",
        npm_scope.as_deref(),
    );

    apply_optional_value(
        table,
        "checksum",
        "# Checksums to generate for each App\n",
        checksum.map(|c| c.ext().as_str()),
    );

    apply_optional_value(
        table,
        "merge-tasks",
        "# Whether to run otherwise-parallelizable tasks on the same machine\n",
        *merge_tasks,
    );

    apply_optional_value(
        table,
        "fail-fast",
        "# Whether failing tasks should make us give up on all other tasks\n",
        *fail_fast,
    );

    apply_optional_value(
        table,
        "cache-builds",
        "# Whether builds should try to be cached in CI\n",
        *cache_builds,
    );

    apply_optional_value(
        table,
        "build-local-artifacts",
        "# Whether CI should include auto-generated code to build local artifacts\n",
        *build_local_artifacts,
    );

    apply_optional_value(
        table,
        "dispatch-releases",
        "# Whether CI should trigger releases with dispatches instead of tag pushes\n",
        *dispatch_releases,
    );

    apply_optional_value(
        table,
        "release-branch",
        "# Trigger releases on pushes to this branch instead of tag pushes\n",
        release_branch.as_ref(),
    );

    apply_optional_value(
        table,
        "create-release",
        "# Whether dist should create a Github Release or use an existing draft\n",
        *create_release,
    );

    apply_optional_value(
        table,
        "github-release",
        "# Which phase dist should use to create the GitHub release\n",
        github_release.as_ref().map(|a| a.to_string()),
    );

    apply_optional_value(
        table,
        "github-releases-repo",
        "# Publish GitHub Releases to this repo instead\n",
        github_releases_repo.as_ref().map(|a| a.to_string()),
    );

    apply_optional_value(
        table,
        "github-releases-submodule-path",
        "# Read the commit to be tagged from the submodule at this path\n",
        github_releases_submodule_path
            .as_ref()
            .map(|a| a.to_string()),
    );

    apply_string_or_list(
        table,
        "install-path",
        "# Path that installers should place binaries in\n",
        install_path.as_ref(),
    );

    apply_string_list(
        table,
        "plan-jobs",
        "# Plan jobs to run in CI\n",
        plan_jobs.as_ref(),
    );

    apply_string_list(
        table,
        "local-artifacts-jobs",
        "# Local artifacts jobs to run in CI\n",
        local_artifacts_jobs.as_ref(),
    );

    apply_string_list(
        table,
        "global-artifacts-jobs",
        "# Global artifacts jobs to run in CI\n",
        global_artifacts_jobs.as_ref(),
    );

    apply_optional_value(
        table,
        "source-tarball",
        "# Generate and dist a source tarball\n",
        *source_tarball,
    );

    apply_string_list(
        table,
        "host-jobs",
        "# Host jobs to run in CI\n",
        host_jobs.as_ref(),
    );

    apply_string_list(
        table,
        "publish-jobs",
        "# Publish jobs to run in CI\n",
        publish_jobs.as_ref(),
    );

    apply_string_list(
        table,
        "post-announce-jobs",
        "# Post-announce jobs to run in CI\n",
        post_announce_jobs.as_ref(),
    );

    apply_optional_value(
        table,
        "publish-prereleases",
        "# Whether to publish prereleases to package managers\n",
        *publish_prereleases,
    );

    apply_optional_value(
        table,
        "force-latest",
        "# Always mark releases as latest, ignoring semver semantics\n",
        *force_latest,
    );

    apply_optional_value(
        table,
        "pr-run-mode",
        "# Which actions to run on pull requests\n",
        pr_run_mode.as_ref().map(|m| m.to_string()),
    );

    apply_optional_value(
        table,
        "ssldotcom-windows-sign",
        "",
        ssldotcom_windows_sign.as_ref().map(|p| p.to_string()),
    );

    apply_optional_value(
        table,
        "macos-sign",
        "# Whether to sign macOS executables\n",
        *macos_sign,
    );

    apply_optional_value(
        table,
        "github-attestations",
        "# Whether to enable GitHub Attestations\n",
        *github_attestations,
    );

    apply_string_or_list(
        table,
        "hosting",
        "# Where to host releases\n",
        hosting.as_ref(),
    );

    apply_optional_value(
        table,
        "tag-namespace",
        "# A prefix git tags must include for dist to care about them\n",
        tag_namespace.as_ref(),
    );

    apply_optional_value(
        table,
        "install-updater",
        "# Whether to install an updater program\n",
        *install_updater,
    );

    apply_optional_value(
        table,
        "always-use-latest-updater",
        "# Whether to always use the latest updater instead of a specific known-good version\n",
        *always_use_latest_updater,
    );

    apply_optional_value(
        table,
        "display",
        "# Whether to display this app's installers/artifacts in release bodies\n",
        *display,
    );

    apply_optional_value(
        table,
        "display-name",
        "# Custom display name to use for this app in release bodies\n",
        display_name.as_ref(),
    );

    apply_string_or_list(
        table,
        "package-libraries",
        "# Which kinds of built libraries to include in the final archives\n",
        package_libraries.as_ref(),
    );

    apply_string_or_list(
        table,
        "install-libraries",
        "# Which kinds of packaged libraries to install\n",
        install_libraries.as_ref(),
    );
*/

    // Finalize the table
    table.decor_mut().set_prefix("\n# Config for 'dist'\n");
}

fn apply_builds(toplevel_table: &mut toml_edit::Table, builds: &Option<BuildLayer>) {
    let Some(builds) = builds
        else {
            return
        };

    let mut possible_table = toml_edit::table();
    let table = toplevel_table
        .get_mut("builds")
        .unwrap_or_else(|| &mut possible_table);

    let toml_edit::Item::Table(table) = table
        else { panic!("Expected [dist.builds] to be a table") };

    // / inheritable fields
    //common: CommonBuildLayer,

    // / Whether we should sign windows binaries with ssl.com
    //ssldotcom_windows_sign: Option<ProductionMode>,

    // / whether to sign macos binaries with apple
    //macos_sign: Option<bool>,

    apply_cargo_builds(table, builds);
    // / cargo builds
    //cargo: Option<BoolOr<CargoBuildLayer>>,
    // / generic builds
    //generic: Option<BoolOr<GenericBuildLayer>>,
    // / A set of packages to install before building
    //#[serde(rename = "dependencies")]
    //system_dependencies: Option<SystemDependencies>,

        /*
    apply_optional_min_glibc_version(
        table,
        "min-glibc-version",
        "# The minimum glibc version supported by the package (overrides auto-detection)\n",
        min_glibc_version.as_ref(),
    );

    apply_optional_value(
        table,
        "omnibor",
        "# Whether to use omnibor-cli to generate OmniBOR Artifact IDs\n",
        *omnibor,
    );
        */
}

fn apply_cargo_builds(builds_table: &mut toml_edit::Table, builds: &BuildLayer) {
    let Some(BoolOr::Val(ref cargo_builds)) = builds.cargo
        else {
            return;
        };

    let mut possible_table = toml_edit::table();
    let table = builds_table
        .get_mut("cargo")
        .unwrap_or_else(|| &mut possible_table);

    let toml_edit::Item::Table(table) = table
        else { panic!("Expected [dist.builds] to be a table") };

    apply_optional_value(
        table,
        "rust-toolchain-version",
        "# The preferred Rust toolchain to use in CI (rustup toolchain syntax)\n",
        cargo_builds.rust_toolchain_version.as_deref(),
    );

    apply_optional_value(
        table,
        "msvc-crt-static",
        "# Whether +crt-static should be used on msvc\n",
        cargo_builds.msvc_crt_static.clone(),
    );

    apply_optional_value(
        table,
        "precise-builds",
        "# Build only the required packages, and individually\n",
        cargo_builds.precise_builds.clone(),
    );

    apply_string_list(
        table,
        "features",
        "# Features to pass to cargo build\n",
        cargo_builds.features.as_ref(),
    );

    apply_optional_value(
        table,
        "default-features",
        "# Whether default-features should be enabled with cargo build\n",
        cargo_builds.default_features.clone(),
    );

    apply_optional_value(
        table,
        "all-features",
        "# Whether to pass --all-features to cargo build\n",
        cargo_builds.all_features.clone(),
    );

    apply_optional_value(
        table,
        "cargo-auditable",
        "# Whether to embed dependency information using cargo-auditable\n",
        cargo_builds.cargo_auditable.clone(),
    );

    apply_optional_value(
        table,
        "cargo-cyclonedx",
        "# Whether to use cargo-cyclonedx to generate an SBOM\n",
        cargo_builds.cargo_cyclonedx.clone(),
    );
}

/// Update the toml table to add/remove this value
///
/// If the value is Some we will set the value and hang a description comment off of it.
/// If the given key already existed in the table, this will update it in place and overwrite
/// whatever comment was above it. If the given key is new, it will appear at the end of the
/// table.
///
/// If the value is None, we delete it (and any comment above it).
fn apply_optional_value<I>(table: &mut toml_edit::Table, key: &str, desc: &str, val: Option<I>)
where
    I: Into<toml_edit::Value>,
{
    if let Some(val) = val {
        table.insert(key, toml_edit::value(val));
        if let Some(mut key) = table.key_mut(key) {
            key.leaf_decor_mut().set_prefix(desc)
        }
    } else {
        table.remove(key);
    }
}

/// Same as [`apply_optional_value`][] but with a list of items to `.to_string()`
fn apply_string_list<I>(table: &mut toml_edit::Table, key: &str, desc: &str, list: Option<I>)
where
    I: IntoIterator,
    I::Item: std::fmt::Display,
{
    if let Some(list) = list {
        let items = list.into_iter().map(|i| i.to_string()).collect::<Vec<_>>();
        let array: toml_edit::Array = items.into_iter().collect();
        // FIXME: Break the array up into multiple lines with pretty formatting
        // if the list is "too long". Alternatively, more precisely toml-edit
        // the existing value so that we can preserve the user's formatting and comments.
        table.insert(key, toml_edit::Item::Value(toml_edit::Value::Array(array)));
        if let Some(mut key) = table.key_mut(key) {
            key.leaf_decor_mut().set_prefix(desc)
        }
    } else {
        table.remove(key);
    }
}

/// Same as [`apply_string_list`][] but when the list can be shorthanded as a string
fn apply_string_or_list<I>(table: &mut toml_edit::Table, key: &str, desc: &str, list: Option<I>)
where
    I: IntoIterator,
    I::Item: std::fmt::Display,
{
    if let Some(list) = list {
        let items = list.into_iter().map(|i| i.to_string()).collect::<Vec<_>>();
        if items.len() == 1 {
            apply_optional_value(table, key, desc, items.into_iter().next())
        } else {
            apply_string_list(table, key, desc, Some(items))
        }
    } else {
        table.remove(key);
    }
}

/// Similar to [`apply_optional_value`][] but specialized to `MacPkgConfig`, since we're not able to work with structs dynamically
fn apply_optional_mac_pkg(
    table: &mut toml_edit::Table,
    key: &str,
    desc: &str,
    val: Option<&MacPkgConfig>,
) {
    if let Some(mac_pkg_config) = val {
        let MacPkgConfig {
            identifier,
            install_location,
        } = mac_pkg_config;

        let new_item = &mut table[key];
        let mut new_table = toml_edit::table();
        if let Some(new_table) = new_table.as_table_mut() {
            apply_optional_value(
                new_table,
                "identifier",
                "# A unique identifier, in tld.domain.package format\n",
                identifier.as_ref(),
            );
            apply_optional_value(
                new_table,
                "install-location",
                "# The location to which the software should be installed\n",
                install_location.as_ref(),
            );
            new_table.decor_mut().set_prefix(desc);
        }
        new_item.or_insert(new_table);
    } else {
        table.remove(key);
    }
}

/// Similar to [`apply_optional_value`][] but specialized to `MinGlibcVersion`, since we're not able to work with structs dynamically
fn apply_optional_min_glibc_version(
    table: &mut toml_edit::Table,
    key: &str,
    desc: &str,
    val: Option<&MinGlibcVersion>,
) {
    if let Some(min_glibc_version) = val {
        let new_item = &mut table[key];
        let mut new_table = toml_edit::table();
        if let Some(new_table) = new_table.as_table_mut() {
            for (target, version) in min_glibc_version {
                new_table.insert(target, toml_edit::Item::Value(version.to_string().into()));
            }
            new_table.decor_mut().set_prefix(desc);
        }
        new_item.or_insert(new_table);
    } else {
        table.remove(key);
    }
}
