#![deny(missing_docs)]

//! CLI binary interface for cargo-dist

use std::io::Write;
use std::panic;
use std::sync::Mutex;

// Import everything from the lib version of ourselves
use cargo_dist::*;
use cargo_dist_schema::DistManifest;
use clap::Parser;
use cli::{Cli, Commands, FakeCli, ManifestArgs, OutputFormat};
use console::Term;
use lazy_static::lazy_static;
use miette::{Diagnostic, IntoDiagnostic};
use thiserror::Error;
use tracing::error;

use crate::cli::{BuildArgs, GenerateCiArgs, InitArgs};

mod cli;

type ReportErrorFunc = dyn Fn(&miette::Report) + Send + Sync + 'static;

// XXX: We might be able to get rid of this `lazy_static` after 1.63 due to
// `const Mutex::new` being stabilized.
lazy_static! {
    static ref REPORT_ERROR: Mutex<Option<Box<ReportErrorFunc>>> = Mutex::new(None);
}

fn set_report_errors_as_json() {
    *REPORT_ERROR.lock().unwrap() = Some(Box::new(move |error| {
        // Manually invoke JSONReportHandler to format the error as a report
        // to out_.
        let mut report = String::new();
        miette::JSONReportHandler::new()
            .render_report(&mut report, error.as_ref())
            .unwrap();
        writeln!(&mut Term::stdout(), r#"{{"error": {report}}}"#).unwrap();
    }));
}

fn report_error(error: &miette::Report) {
    {
        let guard = REPORT_ERROR.lock().unwrap();
        if let Some(do_report) = &*guard {
            do_report(error);
            return;
        }
    }
    error!("{:?}", error);
}

fn main() {
    let FakeCli::Dist(cli) = FakeCli::parse();
    // Init the logger
    tracing_subscriber::fmt::fmt()
        .with_max_level(cli.verbose)
        .with_target(false)
        .without_time()
        .with_writer(std::io::stderr)
        .with_ansi(console::colors_enabled_stderr())
        .init();

    // Control how errors are formatted by setting the miette hook. This will
    // only be used for errors presented to humans, when formatting an error as
    // JSON, it will be handled by a custom `report_error` override, bypassing
    // the hook.
    let using_log_file = false;
    miette::set_hook(Box::new(move |_| {
        let graphical_theme = if console::colors_enabled_stderr() && !using_log_file {
            miette::GraphicalTheme::unicode()
        } else {
            miette::GraphicalTheme::unicode_nocolor()
        };
        Box::new(
            miette::MietteHandlerOpts::new()
                .graphical_theme(graphical_theme)
                .build(),
        )
    }))
    .expect("failed to initialize error handler");

    // Now that miette is set up, use it to format panics.
    panic::set_hook(Box::new(move |panic_info| {
        let payload = panic_info.payload();
        let message = if let Some(msg) = payload.downcast_ref::<&str>() {
            msg
        } else if let Some(msg) = payload.downcast_ref::<String>() {
            &msg[..]
        } else {
            "something went wrong"
        };

        #[derive(Debug, Error, Diagnostic)]
        #[error("{message}")]
        pub struct PanicError {
            pub message: String,
            #[help]
            pub help: Option<String>,
        }

        report_error(
            &miette::Report::from(PanicError {
                message: message.to_owned(),
                help: panic_info
                    .location()
                    .map(|loc| format!("at {}:{}:{}", loc.file(), loc.line(), loc.column())),
            })
            .wrap_err("cargo vet panicked"),
        );
    }));

    // If we're outputting JSON, replace the error report method such that it
    // writes errors out to the normal output stream as JSON.
    if cli.output_format == OutputFormat::Json {
        set_report_errors_as_json();
    }

    let main_result = real_main(&cli);

    let _ = main_result.map_err(|e| {
        report_error(&e);
        std::process::exit(-1);
    });
}

fn real_main(cli: &Cli) -> Result<(), miette::Report> {
    match &cli.command {
        Some(Commands::Init(args)) => cmd_init(cli, args),
        Some(Commands::GenerateCi(args)) => cmd_generate_ci(cli, args),
        Some(Commands::Manifest(args)) => cmd_manifest(cli, args),
        Some(Commands::Build(args)) => cmd_dist(cli, args),
        None => cmd_dist(cli, &cli.build_args),
    }
}

fn print_human(_out: &mut Term, _report: &DistManifest) -> Result<(), std::io::Error> {
    Ok(())
}

fn print_json(out: &mut Term, report: &DistManifest) -> Result<(), std::io::Error> {
    let string = serde_json::to_string_pretty(report).unwrap();
    writeln!(out, "{string}")?;
    Ok(())
}

fn cmd_dist(cli: &Cli, args: &BuildArgs) -> Result<(), miette::Report> {
    let config = cargo_dist::Config {
        artifact_mode: args.artifacts.to_lib(),
        no_local_paths: cli.no_local_paths,
        targets: cli.target.clone(),
        ci: cli.ci.iter().map(|ci| ci.to_lib()).collect(),
        installers: cli.installer.iter().map(|ins| ins.to_lib()).collect(),
    };
    let report = do_dist(&config)?;
    let mut out = Term::stdout();
    match cli.output_format {
        OutputFormat::Human => print_human(&mut out, &report).into_diagnostic()?,
        OutputFormat::Json => print_json(&mut out, &report).into_diagnostic()?,
    }
    Ok(())
}

fn cmd_manifest(cli: &Cli, args: &ManifestArgs) -> Result<(), miette::Report> {
    let config = cargo_dist::Config {
        artifact_mode: args.build_args.artifacts.to_lib(),
        no_local_paths: cli.no_local_paths,
        targets: cli.target.clone(),
        ci: cli.ci.iter().map(|ci| ci.to_lib()).collect(),
        installers: cli.installer.iter().map(|ins| ins.to_lib()).collect(),
    };
    let report = do_manifest(&config)?;
    let mut out = Term::stdout();
    match cli.output_format {
        OutputFormat::Human => print_human(&mut out, &report).into_diagnostic()?,
        OutputFormat::Json => print_json(&mut out, &report).into_diagnostic()?,
    }
    Ok(())
}

fn cmd_init(cli: &Cli, _args: &InitArgs) -> Result<(), miette::Report> {
    // This command is more automagic, so provide default targets if none are chosen
    let targets = if cli.target.is_empty() {
        default_desktop_targets()
    } else {
        cli.target.clone()
    };
    let config = cargo_dist::Config {
        artifact_mode: cargo_dist::ArtifactMode::All,
        no_local_paths: cli.no_local_paths,
        targets,
        ci: cli.ci.iter().map(|ci| ci.to_lib()).collect(),
        installers: cli.installer.iter().map(|ins| ins.to_lib()).collect(),
    };
    let args = cargo_dist::InitArgs {};
    do_init(&config, &args)
}

fn cmd_generate_ci(cli: &Cli, _args: &GenerateCiArgs) -> Result<(), miette::Report> {
    // This command is more automagic, so provide default targets if none are chosen
    let targets = if cli.target.is_empty() {
        default_desktop_targets()
    } else {
        cli.target.clone()
    };
    let config = cargo_dist::Config {
        artifact_mode: cargo_dist::ArtifactMode::All,
        no_local_paths: cli.no_local_paths,
        targets,
        ci: cli.ci.iter().map(|ci| ci.to_lib()).collect(),
        installers: cli.installer.iter().map(|ins| ins.to_lib()).collect(),
    };
    let args = cargo_dist::GenerateCiArgs {};
    do_generate_ci(&config, &args)
}

fn default_desktop_targets() -> Vec<String> {
    vec![
        "x86_64-unknown-linux-gnu".to_owned(),
        "x86_64-apple-darwin".to_owned(),
        "x86_64-pc-windows-msvc".to_owned(),
        // cross-compiles not yet supported
        // "aarch64-gnu-unknown-linux".to_owned(),
        // "aarch64-pc-windows-msvc".to_owned(),
        // "aarch64-apple-darwin".to_owned(),
    ]
}
