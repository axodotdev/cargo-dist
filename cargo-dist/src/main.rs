#![deny(missing_docs)]

//! CLI binary interface for cargo-dist

use std::io::Write;

use camino::Utf8PathBuf;
// Import everything from the lib version of ourselves
use cargo_dist::*;
use cargo_dist_schema::{AssetKind, DistManifest};
use clap::Parser;
use cli::{
    Cli, Commands, FakeCli, GenerateMode, HelpMarkdownArgs, ManifestArgs, OutputFormat, PlanArgs,
};
use console::Term;
use miette::IntoDiagnostic;

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
        Commands::Build(args) => cmd_dist(config, args),
    }
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

fn cmd_dist(cli: &Cli, args: &BuildArgs) -> Result<(), miette::Report> {
    let config = cargo_dist::config::Config {
        needs_coherent_announcement_tag: true,
        artifact_mode: args.artifacts.to_lib(),
        no_local_paths: cli.no_local_paths,
        allow_all_dirty: cli.allow_dirty,
        targets: cli.target.clone(),
        ci: cli.ci.iter().map(|ci| ci.to_lib()).collect(),
        installers: cli.installer.iter().map(|ins| ins.to_lib()).collect(),
        announcement_tag: cli.tag.clone(),
    };
    let report = do_build(&config)?;
    let mut out = Term::stdout();
    match cli.output_format {
        OutputFormat::Human => print_human(&mut out, &report).into_diagnostic()?,
        OutputFormat::Json => print_json(&mut out, &report).into_diagnostic()?,
    }
    Ok(())
}

fn cmd_manifest(cli: &Cli, args: &ManifestArgs) -> Result<(), miette::Report> {
    let config = cargo_dist::config::Config {
        needs_coherent_announcement_tag: true,
        artifact_mode: args.build_args.artifacts.to_lib(),
        no_local_paths: cli.no_local_paths,
        allow_all_dirty: cli.allow_dirty,
        targets: cli.target.clone(),
        ci: cli.ci.iter().map(|ci| ci.to_lib()).collect(),
        installers: cli.installer.iter().map(|ins| ins.to_lib()).collect(),
        announcement_tag: cli.tag.clone(),
    };
    let report = do_manifest(&config)?;
    let mut out = Term::stdout();
    match cli.output_format {
        OutputFormat::Human => {
            print_human(&mut out, &report).into_diagnostic()?;

            // Add some context if we're printing predicted paths
            if !cli.no_local_paths {
                let message = "\nNOTE: 'cargo dist manifest' does not perform builds, these paths may not exist yet!";
                writeln!(out, "{}", out.style().yellow().apply_to(message)).into_diagnostic()?;
            }
        }
        OutputFormat::Json => print_json(&mut out, &report).into_diagnostic()?,
    }
    Ok(())
}

fn cmd_plan(cli: &Cli, _args: &PlanArgs) -> Result<(), miette::Report> {
    // Force --no-local-paths and --artifacts=all
    // No need to force --output-format=human
    let mut new_cli = cli.clone();
    new_cli.no_local_paths = true;
    let args = &ManifestArgs {
        build_args: BuildArgs {
            artifacts: cli::ArtifactMode::All,
        },
    };

    cmd_manifest(&new_cli, args)
}

fn cmd_init(cli: &Cli, args: &InitArgs) -> Result<(), miette::Report> {
    let config = cargo_dist::config::Config {
        needs_coherent_announcement_tag: false,
        artifact_mode: cargo_dist::config::ArtifactMode::All,
        no_local_paths: cli.no_local_paths,
        allow_all_dirty: cli.allow_dirty,
        targets: cli.target.clone(),
        ci: cli.ci.iter().map(|ci| ci.to_lib()).collect(),
        installers: cli.installer.iter().map(|ins| ins.to_lib()).collect(),
        announcement_tag: cli.tag.clone(),
    };
    let args = cargo_dist::InitArgs {
        yes: args.yes,
        no_generate: args.no_generate,
        with_json_config: args.with_json_config.clone(),
    };
    do_init(&config, &args)
}

fn cmd_generate(cli: &Cli, args: &GenerateArgs) -> Result<(), miette::Report> {
    let config = cargo_dist::config::Config {
        needs_coherent_announcement_tag: false,
        artifact_mode: cargo_dist::config::ArtifactMode::All,
        no_local_paths: cli.no_local_paths,
        allow_all_dirty: cli.allow_dirty,
        targets: cli.target.clone(),
        ci: cli.ci.iter().map(|ci| ci.to_lib()).collect(),
        installers: cli.installer.iter().map(|ins| ins.to_lib()).collect(),
        announcement_tag: cli.tag.clone(),
    };
    let args = cargo_dist::GenerateArgs {
        check: args.check,
        modes: args.mode.iter().map(|m| m.to_lib()).collect(),
    };
    do_generate(&config, &args)
}

fn cmd_linkage(cli: &Cli, args: &LinkageArgs) -> Result<(), miette::Report> {
    let config = cargo_dist::config::Config {
        needs_coherent_announcement_tag: false,
        artifact_mode: cargo_dist::config::ArtifactMode::All,
        no_local_paths: cli.no_local_paths,
        allow_all_dirty: cli.allow_dirty,
        targets: cli.target.clone(),
        ci: cli.ci.iter().map(|ci| ci.to_lib()).collect(),
        installers: cli.installer.iter().map(|ins| ins.to_lib()).collect(),
        announcement_tag: cli.tag.clone(),
    };
    let mut options = cargo_dist::LinkageArgs {
        print_output: args.print_output,
        print_json: args.print_json,
    };
    if !args.print_output && !args.print_json {
        options.print_output = true;
    }
    do_linkage(&config, &options)
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
    _args: &cli::ManifestSchemaArgs,
) -> Result<(), miette::ErrReport> {
    let schema = cargo_dist_schema::DistManifest::json_schema();
    let json_schema = serde_json::to_string_pretty(&schema).expect("failed to stringify schema!?");
    println!("{json_schema}");
    Ok(())
}
