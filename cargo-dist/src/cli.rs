//! All the clap stuff for parsing/documenting the cli

use clap::{
    builder::{PossibleValuesParser, TypedValueParser},
    Args, Parser, Subcommand, ValueEnum,
};
use semver::Version;
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

    /// Strip local paths from output (e.g. in the dist manifest json)
    #[clap(long)]
    #[clap(help_heading = "GLOBAL OPTIONS", global = true)]
    pub no_local_paths: bool,

    /// Target triples we want to build
    #[clap(long)]
    #[clap(help_heading = "GLOBAL OPTIONS", global = true)]
    pub target: Vec<String>,

    /// Installers we want to build
    #[clap(long)]
    #[clap(help_heading = "GLOBAL OPTIONS", global = true)]
    pub installer: Vec<InstallerStyle>,

    /// CI we want to support
    #[clap(long)]
    #[clap(help_heading = "GLOBAL OPTIONS", global = true)]
    pub ci: Vec<CiStyle>,

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
pub struct ReleaseNotesArgs {
    /// Get release notes for a specific version.
    ///
    /// Otherwise the app's current version will be used.
    #[clap(long)]
    pub version: Version,
}

#[derive(Args)]
pub struct BuildArgs {
    /// Which subset of the Artifacts to build
    ///
    /// Artifacts can be broken up into two major classes:
    ///
    /// * local: made for each target system (executable-zips, symbols, MSIs...)
    /// * global: made once (curl-sh installers, npm package, metadata...)
    ///
    /// Having this distinction lets us run cargo-dist independently on
    /// multiple machines without collisions between the outputs by spinning
    /// up machines that run something like:
    ///
    /// * linux-runner1: cargo-dist --artifacts=global
    /// * linux-runner2: cargo-dist --artifacts=local --target=x86_64-unknown-linux-gnu
    /// * windows-runner: cargo-dist --artifacts=local --target=x86_64-pc-windows-msvc
    ///  
    /// If let unspecified, we will pick a fuzzier "host" mode that builds "as much as possible"
    /// for the local system. This mode is appropriate for local testing/debugging/demoing.
    /// If no --target flags are passed on the CLI then "host" mode will try to intelligently
    /// guess which targets to build for, which may include building targets that aren't
    /// defined in your metadata.dist config (since that config may exclude the current machine!).
    ///
    /// The specifics of "host" mode are intentionally unspecified to enable us to provider better
    /// out-of-the-box UX for local usage. In CI environments you should always specify "global"
    /// or "local" to get consistent behaviour!
    #[clap(long, value_enum)]
    #[clap(default_value_t = ArtifactMode::Host)]
    pub artifacts: ArtifactMode,
}

/// How we should select the artifacts to build
#[derive(ValueEnum, Copy, Clone, Debug)]
pub enum ArtifactMode {
    /// Build target-specific artifacts like executable-zips, symbols, MSIs...
    Local,
    /// Build globally unique artifacts like curl-sh installers, npm packages, metadata...
    Global,
    /// Fuzzily build "as much as possible" for the host system
    Host,
    /// Build all the artifacts; only really appropriate for `cargo-dist manifest`
    All,
}

impl ArtifactMode {
    /// Convert the application version of this enum to the library version
    pub fn to_lib(self) -> cargo_dist::ArtifactMode {
        match self {
            ArtifactMode::Local => cargo_dist::ArtifactMode::Local,
            ArtifactMode::Global => cargo_dist::ArtifactMode::Global,
            ArtifactMode::Host => cargo_dist::ArtifactMode::Host,
            ArtifactMode::All => cargo_dist::ArtifactMode::All,
        }
    }
}

#[derive(Args)]
pub struct InitArgs {}

#[derive(Args)]
pub struct GenerateCiArgs {}

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

/// A style of installer to generate
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum InstallerStyle {
    /// Generates a shell script that fetches from github ci
    GithubShell,
    /// Generates a powershell script that fetches from github ci
    GithubPowershell,
}

impl InstallerStyle {
    /// Convert the application version of this enum to the library version
    pub fn to_lib(self) -> cargo_dist::InstallerStyle {
        match self {
            InstallerStyle::GithubShell => cargo_dist::InstallerStyle::GithubShell,
            InstallerStyle::GithubPowershell => cargo_dist::InstallerStyle::GithubPowershell,
        }
    }
}

#[derive(Args)]
pub struct ManifestArgs {
    // Add the args from the "real" build command
    #[clap(flatten)]
    pub build_args: BuildArgs,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
}
