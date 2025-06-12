//! Example CLI application based on axocli
//!
//! The crux of the example is `fn main`, but the rest of the example shows off
//! typical stuff you would do in an application with this framework.

// third-party deps
use clap::Parser;
use miette::Report;

// implementation details of our app
use cli_args::{CliArgs, OutputFormat};
use errors::AxoAppError;

fn main() {
    // Step 1: Parse our clap args like normal
    //
    // It's ok for nothing to be setup when this can fail, because CLI arg errors are kind of
    // different from every other kind of error?
    let config = CliArgs::parse();

    // Build our app, forwarding relevant CLI args to the builder
    axocli::CliAppBuilder::new("axoapp")
        // Set logger verbosity
        .verbose(config.verbose)
        // Set whether errors should be printed to stdout as JSON
        // (in addition to the human errors on stderr)
        .json_errors(config.output_format == OutputFormat::Json)
        // Forward our cli args into our "real" main function and run it
        // This will handle printing errors, setting up loggers, catching panics, and so on.
        .start(config, run);
}

/// At this point everything should be properly setup
fn run(app: &axocli::CliApp<CliArgs>) -> Result<(), Report> {
    // Here we do the bulk of the logic in our app, very complex!

    // Some example error conditions, both manual and panic
    assert!(
        app.config.exclaim_count > 0,
        "i have no exclamation marks but i must scream"
    );
    if app.config.exclaim_count < 3 {
        return Err(AxoAppError::NotExcitedEnough)?;
    }
    let message = "hello axoapp";

    // Now that we've done some complex computation, decide how to output the result
    match app.config.output_format {
        OutputFormat::Human => the_impl::print(message, app.config.exclaim_count)?,
        OutputFormat::Json => the_impl::print_json(message, app.config.exclaim_count)?,
    }

    Ok(())
}

/// These details of our app will vary wildly between different applications
mod the_impl {
    use crate::errors::Result;
    use serde::{Deserialize, Serialize};

    /// Print a human-readable output to stdout
    pub fn print(message: &str, num_exclaims: u64) -> Result<()> {
        print!("{message}");
        for _ in 0..num_exclaims {
            print!("!");
        }
        println!();

        Ok(())
    }

    /// Print a machine-readable output (JSON) to stdout
    pub fn print_json(message: &str, num_exclaims: u64) -> Result<()> {
        /// The output
        #[derive(Serialize, Deserialize)]
        struct JsonOutput {
            /// The base message
            message: String,
            /// How many exclamation marks should be added to it
            num_exclaims: u64,
        }

        // Build the output
        let output = JsonOutput {
            message: message.to_owned(),
            num_exclaims,
        };

        // Serialize it to stdout
        serde_json::to_writer_pretty(std::io::stdout(), &output)?;

        Ok(())
    }
}

/// Error types for our application
mod errors {
    use miette::Diagnostic;
    use thiserror::Error;

    /// This is a useful alias if basically every function is going to return your custom error type
    pub type Result<T> = std::result::Result<T, AxoAppError>;

    /// An axoapp Error
    #[derive(Debug, Error, Diagnostic)]
    pub enum AxoAppError {
        /// The user didn't demand enough exclamation marks
        #[error("you're not excited enough!!!!")]
        #[diagnostic(help("pass at least 3 to the CLI"))]
        NotExcitedEnough,

        /// Some random serde error occured
        #[error(transparent)]
        SerdeJson(#[from] serde_json::Error),
    }
}

/// The clap-based CLI Args (derive-style)
mod cli_args {
    use clap::{
        builder::{PossibleValuesParser, TypedValueParser},
        Parser, ValueEnum,
    };
    use tracing::level_filters::LevelFilter;

    /// My perfect little example CLI
    #[derive(Parser)]
    #[clap(version, about, long_about = None)]
    #[clap(args_conflicts_with_subcommands = true)]
    pub struct CliArgs {
        /// How verbose logging should be (log level)
        #[clap(long)]
        #[clap(default_value_t = LevelFilter::WARN)]
        #[clap(value_parser = PossibleValuesParser::new(["off", "error", "warn", "info", "debug", "trace"]).map(|s| s.parse::<LevelFilter>().expect("possible values are valid")))]
        #[clap(help_heading = "GLOBAL OPTIONS", global = true)]
        pub verbose: LevelFilter,

        /// The format of the output
        #[clap(long, value_enum)]
        #[clap(default_value_t = OutputFormat::Human)]
        #[clap(help_heading = "GLOBAL OPTIONS", global = true)]
        pub output_format: OutputFormat,

        /// Number of exclamation marks to add
        pub exclaim_count: u64,
    }

    /// Style of output we should produce
    #[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
    pub enum OutputFormat {
        /// Human-readable output
        Human,
        /// Machine-readable JSON output
        Json,
    }
}
