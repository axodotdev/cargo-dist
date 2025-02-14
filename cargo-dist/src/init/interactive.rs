use axoproject::WorkspaceGraph;
use crate::{config, migrate};
use crate::config::v1::layer::BoolOr;
use crate::config::v1::TomlLayer;
//use crate::config::{CiStyle, InstallerStyle, PublishStyle};
use crate::config::Config;
use crate::errors::{DistError, DistResult};
use crate::platform::triple_to_display_name;
use dialoguer::{Confirm, Input, MultiSelect};
use dist_schema::TripleNameRef;
use semver::Version;
use super::console_helpers::{self, theme};
use super::InitArgs;
use crate::config::v1::layer::BoolOrOptExt;
use crate::config::InstallerStyle;
use crate::config::v1::installers::InstallerLayer;

/// Initialize [dist] with values based on what was passed on the CLI
pub fn get_new_metadata(
    cfg: &Config,
    args: &InitArgs,
    workspaces: &WorkspaceGraph,
) -> DistResult<TomlLayer> {
    let root_workspace = workspaces.root_workspace();
    let has_config = migrate::has_metadata_table(root_workspace);

    let mut meta = if has_config {
        config::v1::load_dist(&root_workspace.manifest_path)?
    } else {
        TomlLayer::default().with_current_dist_version()
    };

    // Clone this to simplify checking for settings changes
    let orig_meta = meta.clone();

    // Now prompt the user interactively to initialize these...

    /*
    // Tune the theming a bit
    let theme = theme();
    // Some indicators we'll use in a few places
    let check = console_helpers::checkmark();
    let notice = console_helpers::notice();
    */

    if !args.host.is_empty() {
        // FIXME(v1): IMPLEMENT THIS
        // --hosting?
        println!("args.host = {:#?}", args.host);
        //meta.hosting = Some(args.host.clone());
    }

    update_dist_version(&cfg, &args, &mut meta)?;
    update_platforms(&cfg, &args, &mut meta)?;
    update_ci_backends(&cfg, &args, &mut meta)?;
    update_installers(&cfg, &args, &mut meta)?;
    update_publishers(&cfg, &args, &orig_meta, &mut meta)?;

    Ok(meta)
}

fn update_dist_version(_cfg: &Config, args: &InitArgs, meta: &mut TomlLayer) -> DistResult<()> {
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
                let res = Confirm::with_theme(&theme())
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

    Ok(())
}

fn update_platforms(cfg: &Config, args: &InitArgs, meta: &mut TomlLayer) -> DistResult<()> {
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
        let res = MultiSelect::with_theme(&theme())
            .items(&keys)
            .defaults(&defaults)
            .with_prompt(prompt)
            .interact()?;
        eprintln!();
        res
    };

    // Apply the results
    meta.targets = Some(selected.into_iter().map(|i| known[i].clone()).collect());

    Ok(())
}

fn update_ci_backends(_cfg: &Config, args: &InitArgs, meta: &mut TomlLayer) -> DistResult<()> {
    // Enable CI backends
    // FIXME: when there is more than one option we maybe shouldn't hide this
    // once the user has any one enabled, right now it's just annoying to always
    // prompt for Github CI support.
    if meta.ci.is_none() {
        // Prompt the user
        let prompt = r#"enable Github CI and Releases?"#;

        let github_selected = if args.yes {
            true
        } else {
            let res = Confirm::with_theme(&theme())
                .with_prompt(prompt)
                .default(true)
                .interact()?;
            eprintln!();
            res
        };

        if github_selected {
            meta.ci = Some(config::v1::ci::CiLayer {
                common: Default::default(),
                github: Some(BoolOr::Bool(true)),
            });
        }
    }

    Ok(())
}

fn update_installers(cfg: &Config, args: &InitArgs, meta: &mut TomlLayer) -> DistResult<()> {
    let notice = console_helpers::notice();

    // Enable installer backends (if they have a CI backend that can provide URLs)
    // FIXME: "vendored" installers like msi could be enabled without any CI...
    let has_ci = meta.ci.clone().map(|ci| ci.github.is_some_and_not_false()).unwrap_or(false);

    // If they have CI, then they can use fetching installers,
    // otherwise they can only do vendored installers.
    let known: &[InstallerStyle] = if has_ci {
        &[
            InstallerStyle::Shell,
            InstallerStyle::Powershell,
            InstallerStyle::Npm,
            InstallerStyle::Homebrew,
            InstallerStyle::Msi,
        ]
    } else {
        eprintln!("{notice} no CI backends enabled, most installers have been hidden");
        &[InstallerStyle::Msi]
    };
    let mut defaults = vec![];
    let mut keys = vec![];
    for item in known {
        // If this CI style is in their config, keep it
        // If they passed it on the CLI, flip it on
        let config_had_it = match item {
            InstallerStyle::Shell => meta.installers.clone().map(|ins| ins.shell.is_some_and_not_false()).unwrap_or(false),
            InstallerStyle::Powershell => meta.installers.clone().map(|ins| ins.powershell.is_some_and_not_false()).unwrap_or(false),
            InstallerStyle::Npm => meta.installers.clone().map(|ins| ins.npm.is_some_and_not_false()).unwrap_or(false),
            InstallerStyle::Homebrew => meta.installers.clone().map(|ins| ins.homebrew.is_some_and_not_false()).unwrap_or(false),
            InstallerStyle::Msi => meta.installers.clone().map(|ins| ins.msi.is_some_and_not_false()).unwrap_or(false),
            InstallerStyle::Pkg => meta.installers.clone().map(|ins| ins.pkg.is_some_and_not_false()).unwrap_or(false),
        };
        let cli_had_it = cfg.installers.contains(item);

        let default = config_had_it || cli_had_it;
        defaults.push(default);

        // This match is here to remind you to add new InstallerStyles
        // to `known` above!
        keys.push(item.to_string());
    }

    // Prompt the user
    let prompt = r#"what installers do you want to build?
(select with arrow keys and space, submit with enter)"#;
    let selected = if args.yes {
        defaults
            .iter()
            .enumerate()
            .filter_map(|(idx, enabled)| enabled.then_some(idx))
            .collect()
    } else {
        let res = MultiSelect::with_theme(&theme())
            .items(&keys)
            .defaults(&defaults)
            .with_prompt(prompt)
            .interact()?;
        eprintln!();
        res
    };

    // Apply the results
    if !selected.is_empty() {
        meta.installers = Some(InstallerLayer {
            shell: installer_wanted(&selected, known, "shell"),
            powershell: installer_wanted(&selected, known, "powershell"),
            npm: installer_wanted(&selected, known, "npm"),
            homebrew: installer_wanted(&selected, known, "homebrew"),
            msi: installer_wanted(&selected, known, "msi"),
            pkg: installer_wanted(&selected, known, "pkg"),
            ..Default::default()
        });
    }

    Ok(())
}

fn installer_wanted<T>(selected: &std::vec::Vec<usize>, known: &[InstallerStyle], name: &str) -> Option<BoolOr<T>> {
    let Some(idx) = known.iter().position(|&r| r.to_string() == name) else {
        return None
    };
    let wanted = selected.contains(&idx);

    if wanted {
        Some(BoolOr::Bool(true))
    } else {
        None
    }
}

fn update_publishers(cfg: &Config, args: &InitArgs, orig_meta: &TomlLayer, meta: &mut TomlLayer) -> DistResult<()> {
    let mut publishers = meta.publishers.clone().unwrap_or_default();

    // FIXME(v1): IMPLEMENT THIS

    return unimplemented!();

/*
    // Special handling of the Homebrew installers
    if let Some(&installers) = meta.installers.as_ref() {
        if installers.homebrew.is_none_or_false() {
            // If we don't have a homebrew config, there's nothing to do.
            return Ok(());
        }

        let homebrew_is_new = !orig_meta.installers.is_some_and(|ins| ins.homebrew.is_some_and_not_false());

        if homebrew_is_new {
            if installers.homebrew.is_none() {
                installers.homebrew = Some(BoolOr::Val(Default::default()));
            }

            let prompt = r#"you've enabled Homebrew support; if you want dist
    to automatically push package updates to a tap (repository) for you,
    please enter the tap name (in GitHub owner/name format)"#;
            let default = "".to_string();

            let tap: String = if args.yes {
                default
            } else {
                let res = Input::with_theme(&theme())
                    .with_prompt(prompt)
                    .allow_empty(true)
                    .interact_text()?;
                eprintln!();
                res
            };
            let tap = tap.trim();
            if tap.is_empty() {
                eprintln!("Homebrew packages will not be automatically published");
                meta.tap = None;
            } else {
                meta.tap = Some(tap.to_owned());
                publish_jobs.push(PublishStyle::Homebrew);

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
        let homebrew_toggled_off = orig_meta
            .installers
            .as_deref()
            .unwrap_or_default()
            .contains(&InstallerStyle::Homebrew);
        if homebrew_toggled_off {
            meta.tap = None;
            publish_jobs.retain(|job| job != &PublishStyle::Homebrew);
        }
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
            if scope.is_empty() {
                eprintln!("{check} npm packages will be published globally");
                meta.npm_scope = None;
            } else {
                meta.npm_scope = Some(scope.to_owned());
                eprintln!("{check} npm packages will be published under {scope}");
            }
            eprintln!();
        }
    } else {
        let npm_toggled_off = orig_meta
            .installers
            .as_deref()
            .unwrap_or_default()
            .contains(&InstallerStyle::Npm);
        if npm_toggled_off {
            meta.npm_scope = None;
            publish_jobs.retain(|job| job != &PublishStyle::Npm);
        }
    }

    meta.publish_jobs = if publish_jobs.is_empty() {
        None
    } else {
        Some(publish_jobs)
    };

    if orig_meta.install_updater.is_none()
        && meta
            .installers
            .as_deref()
            .unwrap_or_default()
            .iter()
            .any(|installer| {
                installer == &InstallerStyle::Shell || installer == &InstallerStyle::Powershell
            })
    {
        let default = false;
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

        meta.install_updater = Some(install_updater);
    }
*/
    Ok(())
}
