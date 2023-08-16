use std::ops::Not;

use axoproject::errors::AxoprojectError;
use axoproject::WorkspaceInfo;
use camino::Utf8PathBuf;
use semver::Version;
use serde::Deserialize;

use crate::{
    config::{self, CiStyle, CompressionImpl, Config, DistMetadata, InstallerStyle, ZipStyle},
    do_generate_ci,
    errors::{DistError, DistResult, Result},
    GenerateCiArgs, SortedMap, METADATA_DIST, PROFILE_DIST,
};

/// Arguments for `cargo dist init` ([`do_init`][])
#[derive(Debug)]
pub struct InitArgs {
    /// Whether to auto-accept the default values for interactive prompts
    pub yes: bool,
    /// Don't automatically generate ci
    pub no_generate_ci: bool,
    /// A path to a json file containing values to set in workspace.metadata.dist
    pub with_json_config: Option<Utf8PathBuf>,
}

/// Input for --with-json-config
///
/// Contains a DistMetadata for the workspace.metadata.dist and
/// then optionally ones for each package.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MultiDistMetadata {
    /// `[workspace.metadata.dist]`
    workspace: Option<DistMetadata>,
    /// package_name => `[package.metadata.dist]`
    #[serde(default)]
    packages: SortedMap<String, DistMetadata>,
}

/// Run 'cargo dist init'
pub fn do_init(cfg: &Config, args: &InitArgs) -> Result<()> {
    let workspace = config::get_project()?;

    // Load in the workspace toml to edit and write back
    let mut workspace_toml = config::load_cargo_toml(&workspace.manifest_path)?;

    let check = console::style("✔".to_string()).for_stderr().green();

    // Init things
    eprintln!("first let's setup your cargo build profile...");
    eprintln!();
    if init_dist_profile(cfg, &mut workspace_toml)? {
        eprintln!("{check} added [profile.dist] to your root Cargo.toml");
    } else {
        eprintln!("{check} [profile.dist] already exists");
    }
    eprintln!();

    eprintln!("next let's setup your cargo-dist config...");
    eprintln!();

    let multi_meta = if let Some(json_path) = &args.with_json_config {
        // json update path, read from a file and apply all requested updates verbatim
        let src = axoasset::SourceFile::load_local(json_path)?;
        let multi_meta: MultiDistMetadata = src.deserialize_json()?;
        multi_meta
    } else {
        // run (potentially interactive) init logic
        let meta = get_new_dist_metadata(cfg, args, &workspace)?;
        MultiDistMetadata {
            workspace: Some(meta),
            packages: SortedMap::new(),
        }
    };

    if let Some(meta) = &multi_meta.workspace {
        update_toml_metadata(&mut workspace_toml, meta, true);
    }

    // Save the workspace toml (potentially an effective no-op if we made no edits)
    eprintln!("{check} added [workspace.metadata.dist] to your root Cargo.toml");
    eprintln!();
    config::save_cargo_toml(&workspace.manifest_path, workspace_toml)?;

    // Now that we've done the stuff that's definitely part of the root Cargo.toml,
    // Optionally apply updates to packages (currently only applies with --with-json-config)
    for (package_name, meta) in &multi_meta.packages {
        for (_idx, package) in workspace.packages() {
            if &package.name == package_name {
                let mut package_toml = config::load_cargo_toml(&package.manifest_path)?;
                update_toml_metadata(&mut package_toml, meta, false);
                eprintln!("{check} added [package.metadata.dist] to {package_name}'s Cargo.toml");
                eprintln!();
                config::save_cargo_toml(&package.manifest_path, package_toml)?;
                break;
            }
        }
    }

    eprintln!("{check} cargo-dist is setup!");
    eprintln!();

    // If there's CI stuff, regenerate it
    if let Some(ci) = multi_meta.workspace.as_ref().and_then(|w| w.ci.as_ref()) {
        if !ci.is_empty() && !args.no_generate_ci {
            eprintln!("running 'cargo dist generate-ci' to apply any changes to your CI scripts");
            eprintln!();

            let ci_args = GenerateCiArgs {};
            do_generate_ci(cfg, &ci_args)?;
        }
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
        // In principle cargo-dist is targeting True Shippable Binaries and so it's
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
            .set_prefix("\n# The profile that 'cargo dist' will build with\n")
    }
    dist_profile.or_insert(new_profile);

    Ok(true)
}

/// Initialize [workspace.metadata.dist] with default values based on what was passed on the CLI
///
/// Returns whether the initialization was actually done
/// and whether ci was set
fn get_new_dist_metadata(
    cfg: &Config,
    args: &InitArgs,
    workspace_info: &WorkspaceInfo,
) -> DistResult<DistMetadata> {
    use dialoguer::{Confirm, Input, MultiSelect};
    // Setup [workspace.metadata.dist]
    let has_config = workspace_info
        .cargo_metadata_table
        .as_ref()
        .and_then(|t| t.as_object())
        .map(|t| t.contains_key(METADATA_DIST))
        .unwrap_or(false);
    let mut meta = if has_config {
        config::parse_metadata_table(
            &workspace_info.manifest_path,
            workspace_info.cargo_metadata_table.as_ref(),
        )?
    } else {
        DistMetadata {
            // If they init with this version we're gonna try to stick to it!
            cargo_dist_version: Some(std::env!("CARGO_PKG_VERSION").parse().unwrap()),
            // deprecated, default to not emitting it
            rust_toolchain_version: None,
            ci: None,
            installers: None,
            targets: cfg.targets.is_empty().not().then(|| cfg.targets.clone()),
            dist: None,
            include: None,
            auto_includes: None,
            windows_archive: None,
            unix_archive: None,
            npm_scope: None,
            checksum: None,
            precise_builds: None,
            merge_tasks: None,
            fail_fast: None,
            install_path: None,
        }
    };

    // Clone this to simplify checking for settings changes
    let orig_meta = meta.clone();

    // Now prompt the user interactively to initialize these...

    // Tune the theming a bit
    let theme = dialoguer::theme::ColorfulTheme {
        checked_item_prefix: console::style("  [x]".to_string()).for_stderr().green(),
        unchecked_item_prefix: console::style("  [ ]".to_string()).for_stderr().dim(),
        active_item_style: console::Style::new().for_stderr().cyan().bold(),
        ..dialoguer::theme::ColorfulTheme::default()
    };
    // Some indicators we'll use in a few places
    let check = console::style("✔".to_string()).for_stderr().green();
    let notice = console::style("⚠️".to_string()).for_stderr().yellow();

    // Set cargo-dist-version
    let current_version: Version = std::env!("CARGO_PKG_VERSION").parse().unwrap();
    if let Some(desired_version) = &meta.cargo_dist_version {
        if desired_version != &current_version && !desired_version.pre.starts_with("github-") {
            let default = true;
            let prompt = format!(
                r#"update your project to this version of cargo-dist?
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
                meta.cargo_dist_version = Some(current_version);
            } else {
                return Err(DistError::NoUpdateVersion {
                    project_version: desired_version.clone(),
                    running_version: current_version,
                })?;
            }
        }
    } else {
        let prompt = format!(
            r#"looks like you deleted the cargo-dist-version key, add it back?
    this is the version of cargo-dist your releases should use
    (you're currently running {})"#,
            current_version
        );
        let default = true;

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
            meta.cargo_dist_version = Some(current_version);
        } else {
            // Not recommended but technically ok...
        }
    }

    // Enable CI backends
    {
        // FIXME: when there is more than one option this should be a proper
        // multiselect like the installer selector is! For now we do
        // most of the multi-select logic and then just give a prompt.
        let known = &[CiStyle::Github];
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

            // If they have a well-defined repo url and it's github, default enable it
            #[allow(irrefutable_let_patterns)]
            if let CiStyle::Github = item {
                github_key = 0;
                if let Some(repo_url) = &workspace_info.repository_url {
                    if repo_url.contains("github.com") {
                        default = true;
                    }
                }
            }
            defaults.push(default);
            // This match is here to remind you to add new CiStyles
            // to `known` above!
            keys.push(match item {
                CiStyle::Github => "github",
            });
        }

        // Prompt the user
        let prompt = r#"enable Github CI integration?
    this creates a CI action which automates creating a Github Release,
    builds all your binaries/archives, and then uploads them to the Release
    it also unlocks the ability to generate installers which fetch those artifacts"#;
        let default = defaults[github_key];

        let github_selected = if args.yes {
            default
        } else {
            let res = Confirm::with_theme(&theme)
                .with_prompt(prompt)
                .default(default)
                .interact()?;
            eprintln!();
            res
        };

        let selected = if github_selected {
            vec![github_key]
        } else {
            vec![]
        };

        // Apply the results
        let ci: Vec<_> = selected.into_iter().map(|i| known[i]).collect();
        meta.ci = if ci.is_empty() { None } else { Some(ci) };
    }

    // Enforce repository url right away
    let has_github_ci = meta
        .ci
        .as_ref()
        .map(|ci| ci.contains(&CiStyle::Github))
        .unwrap_or(false);
    if has_github_ci && workspace_info.repository_url.is_none() {
        // If axoproject complained about inconsistency, forward that
        // Massively jank manual implementation of "clone" here because lots of error types
        // (like std::io::Error) don't implement Clone and so axoproject errors can't either
        let conflict = workspace_info.warnings.iter().find_map(|w| {
            if let AxoprojectError::InconsistentRepositoryKey {
                file1,
                url1,
                file2,
                url2,
            } = w
            {
                Some(AxoprojectError::InconsistentRepositoryKey {
                    file1: file1.clone(),
                    url1: url1.clone(),
                    file2: file2.clone(),
                    url2: url2.clone(),
                })
            } else {
                None
            }
        });
        if let Some(inner) = conflict {
            return Err(DistError::CantEnableGithubUrlInconsistent { inner })?;
        } else {
            // Otherwise assume no URL
            return Err(DistError::CantEnableGithubNoUrl)?;
        }
    }

    // Enable installer backends (if they have a CI backend that can provide URLs)
    // In the future, "vendored" installers like MSIs could be enabled in this situation!
    let has_ci = meta.ci.as_ref().map(|ci| !ci.is_empty()).unwrap_or(false);
    if has_ci {
        let known = &[
            InstallerStyle::Shell,
            InstallerStyle::Powershell,
            InstallerStyle::Npm,
            InstallerStyle::Homebrew,
        ];
        let mut defaults = vec![];
        let mut keys = vec![];
        for item in known {
            // If this CI style is in their config, keep it
            // If they passed it on the CLI, flip it on
            let config_had_it = meta
                .installers
                .as_deref()
                .unwrap_or_default()
                .contains(item);
            let cli_had_it = cfg.installers.contains(item);

            let default = config_had_it || cli_had_it;
            defaults.push(default);

            // This match is here to remind you to add new InstallerStyles
            // to `known` above!
            keys.push(match item {
                InstallerStyle::Shell => "shell",
                InstallerStyle::Powershell => "powershell",
                InstallerStyle::Npm => "npm",
                InstallerStyle::Homebrew => "homebrew",
            });
        }

        // Prompt the user
        let prompt = r#"enable generating installers?
    installers streamline fetching your app's prebuilt artifacts
    see the docs for details on each one
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
        meta.installers = Some(selected.into_iter().map(|i| known[i]).collect());
    } else {
        eprintln!("{notice} no CI backends enabled, skipping installers");
        eprintln!();
    }

    // Special handling of the npm installer
    if meta
        .installers
        .as_deref()
        .unwrap_or_default()
        .contains(&InstallerStyle::Npm)
    {
        // If npm is being newly enabled here, prompt for a @scope
        let npm_is_new = !orig_meta
            .installers
            .as_deref()
            .unwrap_or_default()
            .contains(&InstallerStyle::Npm);
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
            if scope.is_empty() {
                eprintln!("{check} npm packages will be published globally");
                meta.npm_scope = None;
            } else {
                meta.npm_scope = Some(scope.to_owned());
                eprintln!("{check} npm packages will be published under {scope}");
            }
            eprintln!();
        }

        // FIXME (#226): If they have an npm installer, force on tar.gz compression
        const TAR_GZ: Option<ZipStyle> = Some(ZipStyle::Tar(CompressionImpl::Gzip));
        if meta.unix_archive != TAR_GZ || meta.windows_archive != TAR_GZ {
            let prompt = r#"the npm installer requires binaries to be distributed as .tar.gz, is that ok?
    otherwise we would distribute your binaries as .zip on windows, .tar.xz everywhere else
    (this is a hopefully temporary limitation of the npm installer's implementation)"#;
            let default = true;
            let force_targz = if args.yes {
                default
            } else {
                let res = Confirm::with_theme(&theme)
                    .with_prompt(prompt)
                    .default(default)
                    .interact()?;
                eprintln!();
                res
            };
            if force_targz {
                meta.unix_archive = TAR_GZ;
                meta.windows_archive = TAR_GZ;
            } else {
                return Err(DistError::MustEnableTarGz)?;
            }
        }
    }

    Ok(meta)
}

fn update_toml_metadata(
    workspace_toml: &mut toml_edit::Document,
    meta: &DistMetadata,
    is_workspace: bool,
) {
    // Walk down/prepare the components...
    let root_key = if is_workspace { "workspace" } else { "package" };
    let workspace = workspace_toml[root_key].or_insert(toml_edit::table());
    if let Some(t) = workspace.as_table_mut() {
        t.set_implicit(true)
    }
    let metadata = workspace["metadata"].or_insert(toml_edit::table());
    if let Some(t) = metadata.as_table_mut() {
        t.set_implicit(true)
    }
    let dist_metadata = &mut metadata[METADATA_DIST];

    // If there's no table, make one
    if !dist_metadata.is_table() {
        *dist_metadata = toml_edit::table();
    }

    // Apply formatted/commented values
    let table = dist_metadata.as_table_mut().unwrap();

    // This is intentionally written awkwardly to make you update this
    let DistMetadata {
        cargo_dist_version,
        rust_toolchain_version,
        dist,
        ci,
        installers,
        targets,
        include,
        auto_includes,
        windows_archive,
        unix_archive,
        npm_scope,
        checksum,
        precise_builds,
        merge_tasks,
        fail_fast,
        install_path,
    } = &meta;

    apply_optional_value(
        table,
        "cargo-dist-version",
        "# The preferred cargo-dist version to use in CI (Cargo.toml SemVer syntax)\n",
        cargo_dist_version.as_ref().map(|v| v.to_string()),
    );

    apply_optional_value(
        table,
        "rust-toolchain-version",
        "# The preferred Rust toolchain to use in CI (rustup toolchain syntax)\n",
        rust_toolchain_version.as_deref(),
    );

    apply_string_list(
        table,
        "ci",
        "# CI backends to support (see 'cargo dist generate-ci')\n",
        ci.as_ref(),
    );

    apply_string_list(
        table,
        "installers",
        "# The installers to generate for each app\n",
        installers.as_ref(),
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
        "npm-scope",
        "# A namespace to use when publishing this package to the npm registry\n",
        npm_scope.as_deref(),
    );

    apply_optional_value(
        table,
        "checksum",
        "# Checksums to generate for each App\n",
        checksum.map(|c| c.ext()),
    );

    apply_optional_value(
        table,
        "precise-builds",
        "# Build only the required packages, and individually\n",
        *precise_builds,
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
        "install-path",
        "# Path that installers should place binaries in\n",
        install_path.as_ref().map(|p| p.to_string()),
    );

    // Finalize the table
    table
        .decor_mut()
        .set_prefix("\n# Config for 'cargo dist'\n");
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
        table.key_decor_mut(key).unwrap().set_prefix(desc);
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
        table.insert(key, toml_edit::Item::Value(items.into_iter().collect()));
        table.key_decor_mut(key).unwrap().set_prefix(desc);
    } else {
        table.remove(key);
    }
}
