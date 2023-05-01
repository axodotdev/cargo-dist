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

#[derive(Args, Clone, Debug)]
#[clap(version)]
#[clap(bin_name = "cargo dist")]
#[clap(args_conflicts_with_subcommands = true)]
/// Shippable packaging for Rust.
///
/// When run without a subcommand, `cargo dist` will invoke the `build`
/// subcommand. See `cargo dist help build` for more details.
pub struct Cli {
    /// Subcommands ("no subcommand" defaults to `build`)
    #[clap(subcommand)]
    pub command: Commands,

    /// How verbose logging should be (log level)
    #[clap(long, short)]
    #[clap(default_value_t = LevelFilter::WARN)]
    #[clap(value_parser = PossibleValuesParser::new(["off", "error", "warn", "info", "debug", "trace"]).map(|s| s.parse::<LevelFilter>().expect("possible values are valid")))]
    #[clap(help_heading = "GLOBAL OPTIONS", global = true)]
    pub verbose: LevelFilter,

    /// The format of the output
    #[clap(long, short, value_enum)]
    #[clap(default_value_t = OutputFormat::Human)]
    #[clap(help_heading = "GLOBAL OPTIONS", global = true)]
    pub output_format: OutputFormat,

    /// Strip local paths from output (e.g. in the dist manifest json)
    ///
    /// This is useful for generating a clean "full" manifest as follows:
    ///
    /// `cargo dist manifest --artifacts=all --output-format=json --no-local-paths`
    #[clap(long)]
    #[clap(help_heading = "GLOBAL OPTIONS", global = true)]
    pub no_local_paths: bool,

    /// Target triples we want to build
    ///
    /// If left unspecified we will use the values in [workspace.metadata.dist],
    /// except for `cargo dist init` which will select some "good defaults" for you.
    #[clap(long, short)]
    #[clap(help_heading = "GLOBAL OPTIONS", global = true)]
    pub target: Vec<String>,

    /// Installers we want to build
    ///
    /// If left unspecified we will use the values in [workspace.metadata.dist].
    ///  `cargo dist init` will persist the values you pass to that location.
    #[clap(long, short)]
    #[clap(help_heading = "GLOBAL OPTIONS", global = true)]
    pub installer: Vec<InstallerStyle>,

    /// CI we want to support
    ///
    /// If left unspecified we will use the value in [workspace.metadata.dist].
    /// `cargo dist init` will persist the values you pass to that location.
    #[clap(long, short)]
    #[clap(help_heading = "GLOBAL OPTIONS", global = true)]
    pub ci: Vec<CiStyle>,

    /// The (git) tag to use for the Announcement that each invocation of cargo-dist is performing.
    ///
    /// This tag serves two purposes: defining which apps we are Announcing new Releases for
    /// (and therefore building binaries and installers for); and picking an id to use for
    /// certain URLs. For instance the git tag associated with a Github Release is part of the
    /// URL to fetch artifacts from that release, which needs to be known by some installers!
    ///
    /// The currently accepted formats are "v{VERSION}" and "{PACKAGE_NAME}-v{VERSION}"
    /// ("v1.0.0", "v0.1.0-prerelease1", "my-app-v1.0.0", etc).
    ///
    /// If you use the prefixed version then we will only Announce/Release that package's apps
    /// (and return an error if that is not in fact the package's current version). This is
    /// approp
    ///
    /// If you use the unprefixed version then we will assume you're Announcing/Releasing all
    /// packages in the workspace that have that version. This is appropriate for workspaces
    /// that only have one app, or for monorepos that version all their apps in lockstep.
    ///
    /// If you do not specify this tag we will attempt to infer it by trying to Announce/Release
    /// every app in the workspace, succeeding only if they all have the same version. The tag
    /// selected will be "v{VERSION}".
    ///
    /// In the future we may try to make this look at the current git tags or something?
    #[clap(long)]
    #[clap(help_heading = "GLOBAL OPTIONS", global = true)]
    pub tag: Option<String>,
}

#[derive(Subcommand, Clone, Debug)]
pub enum Commands {
    /// Build artifacts
    #[clap(disable_version_flag = true)]
    Build(BuildArgs),
    /// Setup or update cargo-dist
    ///
    /// This will interactively guide you through the process of selecting configuration options
    /// and will also automatically run 'cargo dist generate-ci' afterwards as necessary. It will
    /// also handle updating your project to a new version of cargo-dist if you're running one.
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
    /// This is most useful when paired with `--artifacts=all`.
    #[clap(disable_version_flag = true)]
    Manifest(ManifestArgs),
    /// Print --help as markdown (for generating docs)
    ///
    /// The output of this is not stable or guaranteed.
    #[clap(disable_version_flag = true)]
    #[clap(hide = true)]
    HelpMarkdown(HelpMarkdownArgs),
    /// Get a quick summary of the status of your project
    ///
    /// If you want to know what running your cargo-dist CI will produce,
    /// this is the command for you! It should run the exact same logic and do some
    /// basic integrity checks.
    ///
    /// This is roughly an alias for:
    ///
    ///     cargo dist manifest --artifacts=all --no-local-paths
    #[clap(disable_version_flag = true)]
    Status(StatusArgs),
}

#[derive(Args, Clone, Debug)]
pub struct BuildArgs {
    /// Which subset of the Artifacts to build
    ///
    /// Artifacts can be broken up into two major classes: "local" ones, which are
    /// made for each target system (executable-zips, symbols, MSIs...); and "global" ones,
    /// which are made once per app (curl-sh installers, npm package, metadata...).
    ///
    /// Having this distinction lets us run cargo-dist independently on
    /// multiple machines without collisions between the outputs.
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
    #[clap(long, short, value_enum)]
    #[clap(default_value_t = ArtifactMode::Host)]
    pub artifacts: ArtifactMode,
}

/// How we should select the artifacts to build
#[derive(ValueEnum, Copy, Clone, Debug)]
pub enum ArtifactMode {
    /// Build target-specific artifacts like executable-zips and MSIs
    Local,
    /// Build unique artifacts like curl-sh installers and npm packages
    Global,
    /// Fuzzily build "as much as possible" for the host system
    Host,
    /// Build all the artifacts; useful for `cargo dist manifest`
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

#[derive(Args, Clone, Debug)]
pub struct InitArgs {
    /// Automatically accept all recommended/default values
    ///
    /// This is equivalent to just mashing ENTER over and over
    /// during the interactive prompts.
    #[clap(long, short)]
    pub yes: bool,
    /// Don't automatically invoke 'cargo dist generate-ci' at the end
    #[clap(long)]
    pub no_generate_ci: bool,
}

#[derive(Args, Clone, Debug)]
pub struct GenerateCiArgs {}

#[derive(Args, Clone, Debug)]
pub struct HelpMarkdownArgs {}

/// A style of CI to generate
#[derive(ValueEnum, Clone, Copy, Debug)]
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
    /// Generates a shell script that fetches/installs the right build
    Shell,
    /// Generates a powershell script that fetches/installs the right build
    Powershell,
    /// Generates an npm project that fetches the right build to your node_modules
    Npm,
}

impl InstallerStyle {
    /// Convert the application version of this enum to the library version
    pub fn to_lib(self) -> cargo_dist::InstallerStyle {
        match self {
            InstallerStyle::Shell => cargo_dist::InstallerStyle::Shell,
            InstallerStyle::Powershell => cargo_dist::InstallerStyle::Powershell,
            InstallerStyle::Npm => cargo_dist::InstallerStyle::Npm,
        }
    }
}

#[derive(Args, Clone, Debug)]
pub struct ManifestArgs {
    // Add the args from the "real" build command
    #[clap(flatten)]
    pub build_args: BuildArgs,
}

#[derive(Args, Clone, Debug)]
pub struct StatusArgs {}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
}
