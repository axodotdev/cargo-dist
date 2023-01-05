use std::io::Write;
use std::panic;
use std::sync::Mutex;

// Import everything from the lib version of ourselves
use cargo_dist::*;
use cargo_dist_schema::DistReport;
use clap::Parser;
use cli::{Cli, Commands, FakeCli, OutputFormat};
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
        Some(Commands::Build(args)) => cmd_dist(cli, args),
        Some(Commands::Init(args)) => cmd_init(args),
        Some(Commands::GenerateCi(args)) => cmd_generate_ci(args),
        None => cmd_dist(cli, &BuildArgs::default()),
    }
}

fn print_human(_out: &mut Term, _report: &DistReport) -> Result<(), std::io::Error> {
    Ok(())
}

fn print_json(out: &mut Term, report: &DistReport) -> Result<(), std::io::Error> {
    let string = serde_json::to_string_pretty(report).unwrap();
    writeln!(out, "{string}")?;
    Ok(())
}

fn cmd_dist(cli: &Cli, _args: &BuildArgs) -> Result<(), miette::Report> {
    let report = do_dist()?;
    let mut out = Term::stdout();
    match cli.output_format {
        OutputFormat::Human => print_human(&mut out, &report).into_diagnostic()?,
        OutputFormat::Json => print_json(&mut out, &report).into_diagnostic()?,
    }
    Ok(())
}

fn cmd_init(_args: &InitArgs) -> Result<(), miette::Report> {
    do_init()
}

fn cmd_generate_ci(_args: &GenerateCiArgs) -> Result<(), miette::Report> {
    do_generate_ci()
}
