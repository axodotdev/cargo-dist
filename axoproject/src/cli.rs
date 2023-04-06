use camino::Utf8PathBuf;
use clap::{
    builder::{PossibleValuesParser, TypedValueParser},
    Parser, ValueEnum,
};
use tracing::level_filters::LevelFilter;

#[derive(Parser)]
#[clap(version, about, long_about = None)]
#[clap(args_conflicts_with_subcommands = true)]

/// Get info about projects/workspaces
pub struct Cli {
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

    /// A specific dir to treat as the "root" directory for the purposes of
    /// limiting how high we're willing to climb up the file system, and what
    /// all other paths should be relative to.
    ///
    /// This is useful for situations where you want to find projects in a given
    /// git repo (by passing the root dir of the git repo here).
    ///
    /// If unspecified, returned paths will be absolute.
    #[clap(long)]
    pub root: Option<Utf8PathBuf>,

    /// A path to search for projects from (including all its ancestors)
    ///
    /// If unspecified, we will use the current working directory.
    /// How relative paths are interpretted depends on whether `--root` is specified.
    /// If it is, then this path will be assumed to be relative to `--root`.
    /// If it isn't, then this path will be assumed to be relative to the current working directory.
    pub search_path: Option<Utf8PathBuf>,
}

/// Style of output we should produce
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable output
    Human,
    /// Machine-readable JSON output
    Json,
}
