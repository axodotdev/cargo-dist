//! All the clap stuff for parsing/documenting the cli

use clap::{
    builder::{PossibleValuesParser, TypedValueParser},
    Args, Parser, Subcommand, ValueEnum,
};
use tracing::level_filters::LevelFilter;

#[derive(Parser)]
#[clap(version, about, long_about = None)]
#[clap(propagate_version = true)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
pub enum FakeCli {
    Dist(Cli),
}

#[derive(Args)]
#[clap(version)]
#[clap(bin_name = "cargo vet")]
#[clap(args_conflicts_with_subcommands = true)]
/// Shippable packaging for Rust.
///
/// When run without a subcommand, `cargo dist` will invoke the `build`
/// subcommand. See `cargo dist help build` for more details.
pub struct Cli {
    /// Subcommands ("no subcommand" defaults to `build`)
    #[clap(subcommand)]
    pub command: Option<Commands>,

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

    /// Target triples we want to build
    #[clap(long)]
    #[clap(help_heading = "GLOBAL OPTIONS", global = true)]
    pub target: Vec<String>,

    // Add the args from the "real" build command
    #[clap(flatten)]
    pub build_args: BuildArgs,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Build distributables for the current platform
    #[clap(disable_version_flag = true)]
    Build(BuildArgs),
    /// Initialize default settings in your Cargo.toml
    #[clap(disable_version_flag = true)]
    Init(InitArgs),
    /// Generate CI scripts for orchestrating cargo-dist
    #[clap(disable_version_flag = true)]
    GenerateCi(GenerateCiArgs),
    /// Generate the final build manifest without running any builds.
    ///
    /// Everything will be computed based on what cargo-dist *expects*
    /// the output of a build to be, so this may produce several paths
    /// to nowhere without the actual build to populate them!
    ///
    /// Currently for uniformity this still requires --output-format=json
    /// to actually produce any output -- should it?
    #[clap(disable_version_flag = true)]
    Manifest(ManifestArgs),
}

#[derive(Args)]
pub struct BuildArgs {}

#[derive(Args)]
pub struct InitArgs {
    /// What styles of ci to generate
    #[clap(long)]
    pub ci: Vec<CiStyle>,
}

#[derive(Args)]
pub struct GenerateCiArgs {
    /// What styles of ci to generate
    pub style: Vec<CiStyle>,
}

/// A style of CI to generate
#[derive(ValueEnum, Clone, Copy)]
pub enum CiStyle {
    /// Generate github CI that uploads to github releases
    Github,
}

impl CiStyle {
    /// Convert the application version of this enum to the library version
    pub fn to_lib(self) -> cargo_dist::CiStyle {
        match self {
            CiStyle::Github => cargo_dist::CiStyle::Github,
        }
    }
}

#[derive(Args)]
pub struct ManifestArgs {}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
}
