use std::panic::{RefUnwindSafe, UnwindSafe};

use miette::Context;
use tracing::level_filters::LevelFilter;

use panic::Panic;

mod panic;

pub fn json_diagnostic(diagnostic: &miette::Report) -> serde_json::Value {
    let mut output = Vec::new();
    write_json_diagnostic(&mut output, diagnostic);
    let output = String::from_utf8(output).unwrap();
    serde_json::from_str(&output).unwrap()
}

pub fn write_json_diagnostic<W: std::io::Write>(mut f: W, diagnostic: &miette::Report) {
    let mut report = String::new();
    miette::JSONReportHandler::new()
        .render_report(&mut report, diagnostic.as_ref())
        .unwrap();

    // We wrap the result in a json object with a "diagnostic" field to
    // avoid weird collisions between the success schema and error schema.
    writeln!(f, r#"{{"diagnostic": {report}}}"#).unwrap();
}

fn report_error(error: &miette::Report, json_errors: bool) {
    use std::io::Write;

    // If json_errors are enabled, emit the error to stdout in json format.
    // We explicitly use a JSONReportHandler here because we still want the
    // human-friendly one to be live.
    if json_errors {
        write_json_diagnostic(&mut std::io::stdout(), error);
    }

    // Regardless of whether we want to emit json errors, we should emit a human-friendly
    // version of the error to stderr for usability reasons.
    writeln!(&mut std::io::stderr(), "{error:?}").unwrap();
}

pub struct CliAppBuilder {
    app_name: &'static str,
    force_color: Option<bool>,
    verbose: LevelFilter,
    json_errors: bool,
}
pub struct CliApp<C> {
    pub config: C,
}

impl CliAppBuilder {
    pub fn new(app_name: &'static str) -> Self {
        Self {
            app_name,
            force_color: None,
            verbose: LevelFilter::WARN,
            json_errors: false,
        }
    }
    pub fn color(mut self, color: bool) -> Self {
        self.force_color = Some(color);
        self
    }
    pub fn verbose(mut self, verbose: LevelFilter) -> Self {
        self.verbose = verbose;
        self
    }
    pub fn json_errors(mut self, json_errors: bool) -> Self {
        self.json_errors = json_errors;
        self
    }
    pub fn start<C: RefUnwindSafe>(
        self,
        config: C,
        real_main: impl FnOnce(&CliApp<C>) -> Result<(), miette::Report> + UnwindSafe,
    ) {
        self.init_miette();
        self.init_panic_hook();
        self.init_tracing();

        // Wrap everything in a block so that after this we can run
        // std::process::exit without forgetting any important shutdown code
        let panic_result = {
            // This is where we should setup any scoped state to be shutdown on exit,
            // like a tokio runtime. It should then be stored (by reference if need be)
            // in the CliApp so that `real_main` can use it.
            let app = CliApp { config };

            // Create a wrapper around the real main function that passes CliApp
            // down, because catch_unwind wants us to give it a function that takes no args
            let stub_main = || real_main(&app);

            // Run main, and catch any unwinds (panics)
            std::panic::catch_unwind(stub_main)
        };

        // We now have effectively a `Result<Result<(), MainError>, PanicError>`.
        // First let's peel back the first layer and check if we panicked.
        let main_result = match panic_result {
            Ok(main_result) => main_result,
            Err(_e) => {
                // Main panicked, the panic hook already handled reporting this,
                // so shut down immediately, there's nothing more to do!
                std::process::exit(-1);
            }
        };

        // Ok we didn't panic, now handle any error the main app might have returned
        if let Err(e) = main_result {
            report_error(&e, self.json_errors);
            std::process::exit(-1);
        }

        // Everything succeeded here, so we can just return happily
    }

    fn init_miette(&self) {
        let force_color = self.force_color;
        miette::set_hook(Box::new(move |_| {
            let mut builder = miette::MietteHandlerOpts::new();
            // Miette's default 80-column width for line-wrapping errors is too
            // aggressive. Miette *does* "need" a linewrap threshold because it
            // pretty-renders with indentation, which a terminal's builtin wrap
            // won't respect, producing uglier output than if miette handled it
            //
            // This Comment Is In Memoriam To cargo-dist's error_manifest test,
            // which snapshot-tested a miette error. This test would "randomly"
            // break all the time but we soon realized it was breaking because:
            //
            // cargo-dist sometimes has clean Cargo SemVer Versions like v1.0.0
            // but usually cargo-dist likes messier SemVer Versions like v1.0.0-prerelease.1
            //
            // Every line of this comment is line-wrapped to 80 columns, except
            // one line. And although our snapshot tests strip versions strings
            // from the output, it's as a post-process that comes after miette.
            // Hopefully you see why I think this value should be more than 80.
            builder = builder.width(120);
            if let Some(force_color) = force_color {
                builder = builder.color(force_color);
            }
            Box::new(builder.build())
        }))
        .expect("failed to initialize error handler");
    }

    fn init_panic_hook(&self) {
        let app_name = self.app_name;
        let json_errors = self.json_errors;
        std::panic::set_hook(Box::new(move |info| {
            let mut message = "Something went wrong".to_string();
            let payload = info.payload();
            if let Some(msg) = payload.downcast_ref::<&str>() {
                message = msg.to_string();
            }
            if let Some(msg) = payload.downcast_ref::<String>() {
                message = msg.clone();
            }
            let mut report: Result<(), miette::Report> = Err(Panic(message).into());
            if let Some(loc) = info.location() {
                report = report
                    .with_context(|| format!("at {}:{}:{}", loc.file(), loc.line(), loc.column()));
            }
            if let Err(err) = report.with_context(|| format!("{app_name} panicked.")) {
                // Report the error, the top-level catch_unwind will do the rest
                report_error(&err, json_errors);
            }
        }));
    }

    fn init_tracing(&self) {
        tracing_subscriber::fmt::fmt()
            .with_max_level(self.verbose)
            .with_target(false)
            .without_time()
            .with_writer(std::io::stderr)
            .with_ansi(console::colors_enabled_stderr())
            .init();
    }
}
