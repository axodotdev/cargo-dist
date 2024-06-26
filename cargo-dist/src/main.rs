#![deny(missing_docs)]

//! CLI binary interface for cargo-dist

use std::io::Write;

use axoasset::LocalAsset;
use axoprocess::Cmd;
use axoupdater::AxoUpdater;
use camino::Utf8PathBuf;
// Import everything from the lib version of ourselves
use cargo_dist::{linkage::LinkageDisplay, *};
use cargo_dist_schema::{AssetKind, DistManifest};
use clap::Parser;
use cli::{
    Cli, Commands, FakeCli, GenerateMode, HelpMarkdownArgs, HostArgs, ManifestArgs, OutputFormat,
    PlanArgs,
};
use console::Term;
use miette::{miette, IntoDiagnostic};
use net::ClientSettings;

use crate::cli::{BuildArgs, GenerateArgs, GenerateCiArgs, InitArgs, LinkageArgs};

mod cli;

fn main() {
    let FakeCli::Dist(config) = FakeCli::parse();
    axocli::CliAppBuilder::new("cargo dist")
        .verbose(config.verbose)
        .json_errors(config.output_format == OutputFormat::Json)
        .start(config, real_main);
}

fn real_main(cli: &axocli::CliApp<Cli>) -> Result<(), miette::Report> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(128)
        .enable_all()
        .build()
        .expect("Initializing tokio runtime failed");
    let _guard = runtime.enter();

    let config = &cli.config;
    match &config.command {
        Commands::Init(args) => cmd_init(config, args),
        Commands::Generate(args) => cmd_generate(config, args),
        Commands::GenerateCi(args) => cmd_generate_ci(config, args),
        Commands::Linkage(args) => cmd_linkage(config, args),
        Commands::Manifest(args) => cmd_manifest(config, args),
        Commands::Plan(args) => cmd_plan(config, args),
        Commands::HelpMarkdown(args) => cmd_help_md(config, args),
        Commands::ManifestSchema(args) => cmd_manifest_schema(config, args),
        Commands::Build(args) => cmd_build(config, args),
        Commands::Host(args) => cmd_host(config, args),
        Commands::Selfupdate(args) => runtime.block_on(cmd_update(config, args)),
    }
}

fn print(
    cli: &Cli,
    report: &DistManifest,
    print_linkage: bool,
    warn_cmd: Option<&str>,
) -> Result<(), miette::Report> {
    let mut out = Term::stdout();
    match cli.output_format {
        OutputFormat::Human => {
            print_human(&mut out, report).into_diagnostic()?;

            // Add some context if we're printing predicted paths
            if let Some(name) = warn_cmd {
                if !cli.no_local_paths {
                    let message = format!("\nNOTE: 'cargo dist {name}' does not perform builds, these paths may not exist yet!");
                    writeln!(out, "{}", out.style().yellow().apply_to(message))
                        .into_diagnostic()?;
                }
            }
        }
        OutputFormat::Json => print_json(&mut out, report).into_diagnostic()?,
    }

    let mut err = Term::stderr();
    if print_linkage {
        print_human_linkage(&mut err, report).into_diagnostic()?;
    }

    Ok(())
}

fn print_human(out: &mut Term, manifest: &DistManifest) -> Result<(), std::io::Error> {
    // First say what the announcement would be
    writeln!(
        out,
        "announcing {}",
        manifest.announcement_tag.as_ref().unwrap()
    )?;

    // Now list off all releases
    for release in &manifest.releases {
        writeln!(
            out,
            "{}",
            out.style()
                .blue()
                .apply_to(format!("  {} {}", release.app_name, release.app_version))
        )?;
        for artifact_id in &release.artifacts {
            let artifact = &manifest.artifacts[artifact_id];
            if let cargo_dist_schema::ArtifactKind::Checksum = &artifact.kind {
                // Don't print shasums at top-level
                continue;
            }

            write!(out, "    ")?;
            print_human_artifact_path(out, artifact)?;

            // Print out all the binaries first, those are the money!
            for asset in &artifact.assets {
                if let Some(path) = &asset.path {
                    if let AssetKind::Executable(exe) = &asset.kind {
                        writeln!(out, "      [bin] {}", path)?;
                        if let Some(syms) = &exe.symbols_artifact {
                            writeln!(out, "        (symbols artifact: {syms})")?;
                        }
                    }
                }
            }

            // Provide a more compact printout of less interesting files
            // (We have more specific labels than "misc" here, but we don't care)
            let mut printed_asset = false;
            for asset in &artifact.assets {
                if !matches!(&asset.kind, AssetKind::Executable(_)) {
                    if let Some(path) = &asset.path {
                        if printed_asset {
                            write!(out, ", ")?;
                        } else {
                            printed_asset = true;
                            write!(out, "      [misc] ")?;
                        }
                        write!(out, "{path}")?;
                    }
                }
            }
            if printed_asset {
                writeln!(out)?;
            }

            // Mention the presence of a checksum if it exists
            if let Some(checksum_id) = &artifact.checksum {
                let checksum_artifact = &manifest.artifacts[checksum_id];
                write!(out, "      [checksum] ")?;
                print_human_artifact_path(out, checksum_artifact)?;
            }
        }
    }
    Ok(())
}

fn print_human_artifact_path(
    out: &mut Term,
    artifact: &cargo_dist_schema::Artifact,
) -> Result<(), std::io::Error> {
    // Print out the name or path of the artifact (path is more useful by noisier)
    if let Some(path) = &artifact.path {
        // Try to highlight the actual filename for easier scanning
        let path = Utf8PathBuf::from(path);
        let file = path.file_name().unwrap();
        let parent = path.as_str().strip_suffix(file);
        if let Some(parent) = parent {
            write!(out, "{}", parent)?;
            writeln!(out, "{}", out.style().green().apply_to(file))?;
        } else {
            write!(out, "{}", out.style().green().apply_to(path))?;
        }
    } else if let Some(name) = &artifact.name {
        writeln!(out, "{}", out.style().green().apply_to(name))?;
    }
    Ok(())
}

fn print_json(out: &mut Term, report: &DistManifest) -> Result<(), std::io::Error> {
    let string = serde_json::to_string_pretty(report).unwrap();
    writeln!(out, "{string}")?;
    Ok(())
}

fn print_human_linkage(out: &mut Term, report: &DistManifest) -> Result<(), std::io::Error> {
    writeln!(out, "{}", LinkageDisplay(report))
}

fn cmd_build(cli: &Cli, args: &BuildArgs) -> Result<(), miette::Report> {
    let config = cargo_dist::config::Config {
        tag_settings: cli.tag_settings(true),
        create_hosting: false,
        artifact_mode: args.artifacts.to_lib(),
        no_local_paths: cli.no_local_paths,
        allow_all_dirty: cli.allow_dirty,
        targets: cli.target.clone(),
        ci: cli.ci.iter().map(|ci| ci.to_lib()).collect(),
        installers: cli.installer.iter().map(|ins| ins.to_lib()).collect(),
        root_cmd: "build".to_owned(),
    };
    let report = do_build(&config)?;
    print(
        cli,
        &report,
        args.print.contains(&"linkage".to_owned()),
        None,
    )
}

fn cmd_host(cli: &Cli, args: &HostArgs) -> Result<(), miette::Report> {
    let args = cargo_dist::config::HostArgs {
        steps: args.steps.iter().map(|m| m.to_lib()).collect(),
    };
    // host can be invoked on multiple machines, so use arg keys to disambiguate
    let arg_key = args
        .steps
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let config = cargo_dist::config::Config {
        tag_settings: cli.tag_settings(true),
        create_hosting: false,
        artifact_mode: config::ArtifactMode::All,
        no_local_paths: true,
        allow_all_dirty: cli.allow_dirty,
        targets: cli.target.clone(),
        ci: cli.ci.iter().map(|ci| ci.to_lib()).collect(),
        installers: cli.installer.iter().map(|ins| ins.to_lib()).collect(),
        root_cmd: format!("host:{arg_key}"),
    };

    let report = cargo_dist::host::do_host(&config, args)?;
    print(cli, &report, false, Some("host"))
}

fn cmd_manifest(cli: &Cli, args: &ManifestArgs) -> Result<(), miette::Report> {
    let config = cargo_dist::config::Config {
        tag_settings: cli.tag_settings(true),
        create_hosting: false,
        artifact_mode: args.build_args.artifacts.to_lib(),
        no_local_paths: cli.no_local_paths,
        allow_all_dirty: cli.allow_dirty,
        targets: cli.target.clone(),
        ci: cli.ci.iter().map(|ci| ci.to_lib()).collect(),
        installers: cli.installer.iter().map(|ins| ins.to_lib()).collect(),
        root_cmd: "plan".to_owned(),
    };
    let report = do_manifest(&config)?;
    print(cli, &report, false, Some("manifest"))
}

fn cmd_plan(cli: &Cli, _args: &PlanArgs) -> Result<(), miette::Report> {
    // Force --no-local-paths and --artifacts=all
    // No need to force --output-format=human
    let mut new_cli = cli.clone();
    new_cli.no_local_paths = true;
    let args = &ManifestArgs {
        build_args: BuildArgs {
            artifacts: cli::ArtifactMode::All,
            print: vec![],
        },
    };

    cmd_manifest(&new_cli, args)
}

fn cmd_init(cli: &Cli, args: &InitArgs) -> Result<(), miette::Report> {
    let config = cargo_dist::config::Config {
        tag_settings: cli.tag_settings(false),
        create_hosting: false,
        artifact_mode: cargo_dist::config::ArtifactMode::All,
        no_local_paths: cli.no_local_paths,
        allow_all_dirty: cli.allow_dirty,
        targets: cli.target.clone(),
        ci: cli.ci.iter().map(|ci| ci.to_lib()).collect(),
        installers: cli.installer.iter().map(|ins| ins.to_lib()).collect(),
        root_cmd: "init".to_owned(),
    };
    let args = cargo_dist::InitArgs {
        yes: args.yes,
        no_generate: args.skip_generate,
        with_json_config: args.with_json_config.clone(),
        host: args.hosting.iter().map(|host| host.to_lib()).collect(),
    };
    do_init(&config, &args)?;
    Ok(())
}

fn cmd_generate(cli: &Cli, args: &GenerateArgs) -> Result<(), miette::Report> {
    let config = cargo_dist::config::Config {
        tag_settings: cli.tag_settings(false),
        create_hosting: false,
        artifact_mode: cargo_dist::config::ArtifactMode::All,
        no_local_paths: cli.no_local_paths,
        allow_all_dirty: cli.allow_dirty,
        targets: cli.target.clone(),
        ci: cli.ci.iter().map(|ci| ci.to_lib()).collect(),
        installers: cli.installer.iter().map(|ins| ins.to_lib()).collect(),
        root_cmd: "generate".to_owned(),
    };
    let args = cargo_dist::GenerateArgs {
        check: args.check,
        modes: args.mode.iter().map(|m| m.to_lib()).collect(),
    };
    do_generate(&config, &args)?;
    Ok(())
}

fn cmd_linkage(cli: &Cli, args: &LinkageArgs) -> Result<(), miette::Report> {
    let config = cargo_dist::config::Config {
        tag_settings: cli.tag_settings(false),
        create_hosting: false,
        artifact_mode: cargo_dist::config::ArtifactMode::All,
        no_local_paths: cli.no_local_paths,
        allow_all_dirty: cli.allow_dirty,
        targets: cli.target.clone(),
        ci: cli.ci.iter().map(|ci| ci.to_lib()).collect(),
        installers: cli.installer.iter().map(|ins| ins.to_lib()).collect(),
        root_cmd: "linkage".to_owned(),
    };
    let mut options = cargo_dist::linkage::LinkageArgs {
        print_output: args.print_output,
        print_json: args.print_json,
        from_json: args.from_json.clone(),
    };
    if !args.print_output && !args.print_json {
        options.print_output = true;
    }
    cargo_dist::linkage::do_linkage(&config, &options)?;
    Ok(())
}

fn cmd_generate_ci(cli: &Cli, args: &GenerateCiArgs) -> Result<(), miette::Report> {
    cmd_generate(
        cli,
        &GenerateArgs {
            check: args.check,
            mode: vec![GenerateMode::Ci],
        },
    )
}

fn cmd_help_md(_args: &Cli, _sub_args: &HelpMarkdownArgs) -> Result<(), miette::Report> {
    let mut out = Term::stdout();
    print_help_markdown(&mut out).into_diagnostic()
}

/// Perform crimes on clap long_help to generate markdown docs
fn print_help_markdown(out: &mut dyn Write) -> std::io::Result<()> {
    use clap::CommandFactory;

    let app_name = "cargo-dist";
    let pretty_app_name = "cargo dist";
    // Make a new App to get the help message this time.

    writeln!(out, "# {pretty_app_name} CLI manual")?;
    writeln!(out)?;
    writeln!(
        out,
        "> This manual can be regenerated with `{pretty_app_name} help-markdown`"
    )?;
    writeln!(out)?;

    let mut fake_cli = FakeCli::command().term_width(0);
    let full_command = fake_cli.get_subcommands_mut().next().unwrap();
    full_command.build();
    let mut work_stack = vec![full_command];
    let mut is_full_command = true;

    while let Some(command) = work_stack.pop() {
        let mut help_buf = Vec::new();
        command.write_long_help(&mut help_buf)?;
        let help = String::from_utf8(help_buf).unwrap();

        // First line is --version
        let lines = help.lines();
        // let version_line = lines.next().unwrap();
        let subcommand_name = command.get_name();

        if is_full_command {
            // writeln!(out, "Version: `{version_line}`")?;
            // writeln!(out)?;
        } else {
            // Give subcommands some breathing room
            writeln!(out, "<br><br><br>")?;
            writeln!(out, "## {pretty_app_name} {subcommand_name}")?;
        }

        let mut in_subcommands_listing = false;
        let mut in_global_options = false;
        let mut in_normal_options = false;
        for line in lines {
            if let Some(usage) = line.strip_prefix("Usage: ") {
                writeln!(out, "### Usage")?;
                writeln!(out)?;
                writeln!(out, "```text")?;
                writeln!(out, "{usage}")?;
                writeln!(out, "```")?;
                continue;
            }

            // Use a trailing colon to indicate a heading
            if let Some(heading) = line.strip_suffix(':') {
                if !line.starts_with(' ') {
                    in_subcommands_listing = heading == "Commands";

                    in_global_options = heading == "GLOBAL OPTIONS";
                    in_normal_options = heading == "Options";

                    writeln!(out, "### {heading}")?;

                    if in_global_options && !is_full_command {
                        writeln!(
                            out,
                            "This subcommand accepts all the [global options](#global-options)"
                        )?;
                    }
                    continue;
                }
            }

            if in_normal_options && is_full_command {
                // Skip normal options for the primary command
                continue;
            }
            if in_global_options && !is_full_command {
                // Skip global options for non-primary commands
                continue;
            }

            if in_subcommands_listing && !line.starts_with("     ") {
                // subcommand names are list items
                let subcommand_line = line.trim();
                if let Some((first, rest)) = subcommand_line.split_once(' ') {
                    let own_subcommand_name = first.trim();
                    let desc = rest.trim();
                    if !own_subcommand_name.is_empty() {
                        writeln!(
                            out,
                            "* [{own_subcommand_name}](#{app_name}-{own_subcommand_name}): {desc}"
                        )?;
                        continue;
                    }
                } else {
                    let own_subcommand_name = subcommand_line;
                    if !own_subcommand_name.is_empty() {
                        write!(
                            out,
                            "* [{own_subcommand_name}](#{app_name}-{own_subcommand_name}): "
                        )?;
                        continue;
                    }
                }
            }
            // The rest is indented, get rid of that
            let line = line.trim();

            // argument names are subheadings
            if line.starts_with("- ") {
                // Do nothing it's a bullet
            } else if line.starts_with('-') || line.starts_with('<') {
                writeln!(out, "#### `{line}`")?;
                continue;
            }
            if line == "[SYMBOLS_PATH_LEGACY]..." {
                writeln!(out, "#### `{line}`")?;
                continue;
            }

            // escape default/value strings
            if line.starts_with('[') {
                writeln!(out, "\\{line}  ")?;
                continue;
            }

            // Normal paragraph text
            writeln!(out, "{line}")?;
        }
        writeln!(out)?;

        // The work_stack is necessarily processed in reverse-order, so append
        // these commands to the end in reverse-order so the first command is
        // processed first (i.e. at the end of the list).
        if subcommand_name != "help" {
            work_stack.extend(
                command
                    .get_subcommands_mut()
                    .filter(|cmd| !cmd.is_hide_set())
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev(),
            );
            is_full_command = false;
        }
    }

    Ok(())
}

fn cmd_manifest_schema(
    _config: &Cli,
    args: &cli::ManifestSchemaArgs,
) -> Result<(), miette::ErrReport> {
    let schema = cargo_dist_schema::DistManifest::json_schema();
    let json_schema = serde_json::to_string_pretty(&schema).expect("failed to stringify schema!?");

    if let Some(destination) = args.output.to_owned() {
        let contents = json_schema + "\n";
        LocalAsset::write_new(&contents, destination)?;
    } else {
        println!("{json_schema}");
    }
    Ok(())
}

fn this_cargo_dist_provided_by_brew() -> bool {
    if cfg!(target_family = "windows") {
        return false;
    }

    if let Ok(path) = std::env::current_exe() {
        // The cargo-dist being a symlink that points to a copy that
        // lives in Homebrew's "Cellar", *or* that file directly,
        // suggests that this file is from Homebrew.
        let realpath;
        if let Ok(resolved) = path.read_link() {
            realpath = resolved;
        } else {
            realpath = path;
        }
        realpath.starts_with("/usr/local/Cellar") || realpath.starts_with("/opt/homebrew/Cellar")
    } else {
        false
    }
}

fn perform_init(path: &Utf8PathBuf, args: &cli::UpdateArgs) -> Result<(), miette::ErrReport> {
    let mut cmd = Cmd::new(path, "cargo dist init");
    cmd.arg("dist").arg("init");
    // Forward shared arguments as necessary
    if args.yes {
        cmd.arg("--yes");
    }
    if args.skip_generate {
        cmd.arg("--skip-generate");
    }
    if let Some(path) = &args.with_json_config {
        cmd.arg(format!("--with-json-config={path}"));
    }
    for host in &args.hosting {
        cmd.arg(format!("--hosting={host}"));
    }
    cmd.run()?;

    Ok(())
}

async fn cmd_update(_config: &Cli, args: &cli::UpdateArgs) -> Result<(), miette::ErrReport> {
    // If the user is asking us to run init, but it doesn't look like we can, error
    // out immediately to avoid the user getting confused and thinking the update didn't work!
    if !args.skip_init {
        config::get_project()
            .map_err(|cause| cargo_dist::errors::DistError::UpdateNotInWorkspace { cause })?;
    }

    if this_cargo_dist_provided_by_brew() {
        eprintln!("Your copy of `cargo-dist` seems to have been installed via Homebrew.");
        eprintln!("Please run `brew upgrade cargo-dist` to update this copy.");
        return Ok(());
    }

    let mut updater = AxoUpdater::new_for("cargo-dist");

    // If there's a specific version needed, random-access query it by tag,
    // because we always use the same tag format and this is fastest while
    // axoupdater needs to look over all releases to find the one.
    let specifier = if let Some(version) = &args.version {
        axoupdater::UpdateRequest::SpecificTag(format!("v{version}"))
    } else if args.prerelease {
        axoupdater::UpdateRequest::LatestMaybePrerelease
    } else {
        axoupdater::UpdateRequest::Latest
    };
    updater.configure_version_specifier(specifier);

    // TODO: pass cli flags..?
    let ClientSettings {
        prefer_native_tls: _,
    } = ClientSettings::new(false);
    // TODO: add API to axoupdater

    // This uses debug assertions because we want to avoid this
    // being compiled into the release build; this is purely for
    // testing.
    #[cfg(debug_assertions)]
    if let Ok(installer_path) = std::env::var("CARGO_DIST_USE_INSTALLER_AT_PATH") {
        let path = Utf8PathBuf::from(installer_path);
        updater.configure_installer_path(path);
    }

    if let Ok(token) = std::env::var("CARGO_DIST_GITHUB_TOKEN") {
        updater.set_github_token(&token);
    }

    // Do we want to treat this as an error?
    // Or do we want to sniff if this was a Homebrew installation?
    if updater.load_receipt().is_err() {
        eprintln!("Unable to load install receipt to check for updates.");
        eprintln!("If you installed this via `brew`, please `brew upgrade cargo-dist`!");
        return Ok(());
    }

    if !updater.check_receipt_is_for_this_executable()? {
        eprintln!("This installation of cargo-dist wasn't installed via a method that `cargo dist selfupdate` supports.");
        eprintln!("Please update manually.");
        return Ok(());
    }

    if let Some(result) = updater.run().await? {
        eprintln!(
            "Update performed: {} => {}",
            env!("CARGO_PKG_VERSION"),
            result.new_version
        );

        // Check that the binary was actually created
        let bin_name = format!("cargo-dist{}", std::env::consts::EXE_SUFFIX);
        let mut new_path = result.install_prefix.join("bin").join(&bin_name);

        // Install prefix could be a flat prefix with no "bin";
        // try that next
        if !new_path.exists() {
            new_path = result.install_prefix.join(&bin_name);
            // Well crap, nothing got installed in the path
            // we wanted it to go. Error out instead of
            // proceeding.
            if !new_path.exists() {
                return Err(errors::DistError::UpdateFailed {}).into_diagnostic();
            }
        }

        // At this point, we've either updated or bailed out;
        // we can proceed with the init if the user would like us to.
        if !args.skip_init {
            perform_init(&new_path, args)?;

            return Ok(());
        }
    } else {
        eprintln!(
            "No update necessary; {} is up to date.",
            env!("CARGO_PKG_VERSION")
        );
    }

    // We didn't update, but we can still check if an init
    // is appropriate.
    if !args.skip_init {
        let my_path = Utf8PathBuf::from_path_buf(std::env::current_exe().into_diagnostic()?)
            .map_err(|_| miette!("Unable to decode the path to cargo-dist itself"))?;
        perform_init(&my_path, args)?;

        return Ok(());
    }

    Ok(())
}
