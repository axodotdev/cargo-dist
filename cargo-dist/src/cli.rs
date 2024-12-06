//! All the clap stuff for parsing/documenting the cli

use camino::Utf8PathBuf;
use cargo_dist::announce::{TagMode, TagSettings};
use cargo_dist_schema::TripleName;
use clap::{
    builder::{PossibleValuesParser, TypedValueParser},
    Args, Parser, Subcommand, ValueEnum,
};
use tracing::level_filters::LevelFilter;

#[derive(Parser, Clone, Debug)]
#[clap(version)]
#[clap(bin_name = "dist")]
#[clap(args_conflicts_with_subcommands = true)]
/// Professional packaging and distribution for ambitious developers.
///
/// See 'init', 'build' and 'plan' for the 3 most important subcommands.
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
    /// `dist manifest --artifacts=all --output-format=json --no-local-paths`
    #[clap(long)]
    #[clap(help_heading = "GLOBAL OPTIONS", global = true)]
    pub no_local_paths: bool,

    /// Target triples we want to build
    ///
    /// If left unspecified we will use the values in [workspace.metadata.dist],
    /// except for `dist init` which will select some "good defaults" for you.
    #[clap(long, short, value_delimiter(','))]
    #[clap(help_heading = "GLOBAL OPTIONS", global = true)]
    pub target: Vec<TripleName>,

    /// Installers we want to build
    ///
    /// If left unspecified we will use the values in [workspace.metadata.dist].
    ///  `dist init` will persist the values you pass to that location.
    #[clap(long, short, value_delimiter(','))]
    #[clap(help_heading = "GLOBAL OPTIONS", global = true)]
    pub installer: Vec<InstallerStyle>,

    /// CI we want to support
    ///
    /// If left unspecified we will use the value in [workspace.metadata.dist].
    /// `dist init` will persist the values you pass to that location.
    #[clap(long, short, value_delimiter(','))]
    #[clap(help_heading = "GLOBAL OPTIONS", global = true)]
    pub ci: Vec<CiStyle>,

    /// The (git) tag to use for the Announcement that each invocation of dist is performing.
    ///
    /// This tag serves two purposes: defining which apps we are Announcing new Releases for
    /// (and therefore building binaries and installers for); and picking an id to use for
    /// certain URLs. For instance the git tag associated with a Github Release is part of the
    /// URL to fetch artifacts from that release, which needs to be known by some installers!
    ///
    /// Unified Announcement: VERSION selects all packages with the given version
    /// (v1.0.0, 0.1.0-prerelease.1, releases/1.2.3, ...)
    ///
    /// Singular Announcement: PACKAGE-VERSION or PACKAGE/VERSION selects only the given package
    /// (my-app-v1.0.0, my-app/1.0.0, release/my-app/v1.2.3-alpha, ...)
    ///
    /// If you use the singular version then we will only Announce/Release that package's apps
    /// (and return an error if that is not in fact the package's current version). This is
    /// appropriate for workspaces that have more than one app.
    ///
    /// If you use the unified version then we will assume you're Announcing/Releasing all
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
    /// Force package versions to match the tag
    #[clap(long)]
    #[clap(help_heading = "GLOBAL OPTIONS", global = true)]
    pub force_tag: bool,
    /// Allow generated files like CI scripts to be out of date
    #[clap(long)]
    #[clap(help_heading = "GLOBAL OPTIONS", global = true)]
    pub allow_dirty: bool,
}

#[derive(Subcommand, Clone, Debug)]
pub enum Commands {
    /// Build artifacts
    #[clap(disable_version_flag = true)]
    Build(BuildArgs),

    /// Print the upload files from a manifest (useful in GitHub Actions to avoid relying on jq)
    #[clap(disable_version_flag = true)]
    #[clap(hide = true)]
    PrintUploadFilesFromManifest(PrintUploadFilesFromManifestArgs),

    /// Setup or update dist
    ///
    /// This will interactively guide you through the process of selecting configuration options
    /// and will also automatically run 'dist generate' afterwards as necessary. It will
    /// also handle updating your project to a new version of dist if you're running one.
    #[clap(disable_version_flag = true)]
    Init(InitArgs),
    /// Migrate to the latest configuration variant.
    Migrate(MigrateArgs),
    /// Generate one or more pieces of configuration
    #[clap(disable_version_flag = true)]
    Generate(GenerateArgs),
    /// Generate CI scripts for orchestrating dist (deprecated in favour of generate)
    #[clap(disable_version_flag = true)]
    #[clap(hide = true)]
    GenerateCi(GenerateCiArgs),
    /// Report on the dynamic libraries used by the built artifacts.
    #[clap(disable_version_flag = true)]
    Linkage(LinkageArgs),
    /// Generate the final build manifest without running any builds.
    ///
    /// This command is designed to match the exact behaviour of
    /// 'dist build' when passed the same flags, which is nice
    /// for consistency but annoying for anyone who doesn't understand
    /// dist's design really well.
    ///
    /// Notably it will default to only talking about artifacts
    /// for the host system, and will produce paths to the build dir
    /// that may not exist (since the build wasn't run).
    ///
    /// 'dist plan' is an alias for this command that picks nicer defaults
    /// by forcing a couple flags to have specific values. You probably want that.
    #[clap(disable_version_flag = true)]
    Manifest(ManifestArgs),
    /// Print --help as markdown (for generating docs)
    ///
    /// The output of this is not stable or guaranteed.
    #[clap(disable_version_flag = true)]
    #[clap(hide = true)]
    HelpMarkdown(HelpMarkdownArgs),
    /// Print the json schema for dist-manifest.json
    #[clap(disable_version_flag = true)]
    #[clap(hide = true)]
    ManifestSchema(ManifestSchemaArgs),
    /// Get a plan of what to build (and check project status)
    ///
    /// If you want to know what running your dist CI will produce,
    /// this is the command for you! This is the exact command that CI will
    /// run to make its build plan and generate dist-manifest.json
    /// (although it adds --output-format=json so that it's machine-readable).
    ///
    /// This is an alias for the lower-level 'manifest' command with the
    /// appropriate flags forced for asking for "everything"
    ///
    ///     dist manifest --artifacts=all --no-local-paths
    ///
    #[clap(disable_version_flag = true)]
    Plan(PlanArgs),

    /// Host artifacts
    #[clap(disable_version_flag = true)]
    Host(HostArgs),

    /// Performs a self-update, if a new version is available, and then 'init'
    #[clap(disable_version_flag = true)]
    Selfupdate(UpdateArgs),
}

#[derive(Args, Clone, Debug)]
pub struct BuildArgs {
    /// Which subset of the Artifacts to build
    ///
    /// Artifacts can be broken up into two major classes: "local" ones, which are
    /// made for each target system (archives, symbols, msi installers...); and "global" ones,
    /// which are made once per app (curl-sh installers, npm package, metadata...).
    ///
    /// Having this distinction lets us run dist independently on
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

    /// What extra information to print, if anything. Currently supported:
    ///
    /// * linkage: prints information on dynamic libraries used by build artifacts
    #[clap(long, short, value_delimiter(','))]
    pub print: Vec<String>,
}

/// How we should select the artifacts to build
#[derive(ValueEnum, Copy, Clone, Debug)]
pub enum ArtifactMode {
    /// Build target-specific artifacts like archives and msi installers
    Local,
    /// Build unique artifacts like curl-sh installers and npm packages
    Global,
    /// Fuzzily build "as much as possible" for the host system
    Host,
    /// Build all the artifacts; useful for `dist manifest`
    All,
    /// Fake all the artifacts; useful for testing/mocking/staging
    Lies,
}

impl ArtifactMode {
    /// Convert the application version of this enum to the library version
    pub fn to_lib(self) -> cargo_dist::config::ArtifactMode {
        match self {
            ArtifactMode::Local => cargo_dist::config::ArtifactMode::Local,
            ArtifactMode::Global => cargo_dist::config::ArtifactMode::Global,
            ArtifactMode::Host => cargo_dist::config::ArtifactMode::Host,
            ArtifactMode::All => cargo_dist::config::ArtifactMode::All,
            ArtifactMode::Lies => cargo_dist::config::ArtifactMode::Lies,
        }
    }
}

// !!!!!!!!!!!!
// HEY HEY YOU
// !!!!!!!!!!!!
// IF YOU ADD NEW FIELDS TO THIS MIRROR THEM TO UpdateArgs!!!
#[derive(Args, Clone, Debug)]
pub struct InitArgs {
    /// Automatically accept all recommended/default values
    ///
    /// This is equivalent to just mashing ENTER over and over
    /// during the interactive prompts.
    #[clap(long, short)]
    pub yes: bool,
    /// Skip running 'dist generate' at the end
    #[clap(long, alias = "no-generate-ci", alias = "no-generate")]
    pub skip_generate: bool,
    /// A path to a json file containing values to set in workspace.metadata.dist
    /// and package.metadata.dist, for building tools that edit these configs.
    ///
    /// This is the same toml => json format that `cargo metadata` produces
    /// when reporting `workspace.metadata.dist`. There is some additional
    /// hierarchy for specifying which values go to which packages, but this
    /// is currently intentionally undocumented to give us some flexibility to change it.
    #[clap(long)]
    pub with_json_config: Option<Utf8PathBuf>,
    /// releases hosting backends we want to support
    ///
    /// If left unspecified we will use the value in [workspace.metadata.dist].
    /// (If no such value exists we will use the one "native" to your CI provider)
    /// `dist init` will persist the values you pass to that location.
    #[clap(long, value_delimiter(','))]
    pub hosting: Vec<HostingStyle>,
}

#[derive(Args, Clone, Debug)]
pub struct MigrateArgs {
}

/// Which style(s) of configuration to generate
#[derive(ValueEnum, Copy, Clone, Debug)]
pub enum GenerateMode {
    /// Generate CI scripts for orchestrating dist
    Ci,
    /// Generate .wxs templates for msi installers
    Msi,
}

impl GenerateMode {
    /// Convert the application version of this enum to the library version
    pub fn to_lib(self) -> cargo_dist::config::GenerateMode {
        match self {
            GenerateMode::Ci => cargo_dist::config::GenerateMode::Ci,
            GenerateMode::Msi => cargo_dist::config::GenerateMode::Msi,
        }
    }
}

#[derive(Args, Clone, Debug)]
pub struct GenerateArgs {
    /// Which type of configuration to generate
    #[clap(long, value_delimiter(','))]
    pub mode: Vec<GenerateMode>,

    /// Check if the generated output differs from on-disk config without writing it
    #[clap(long)]
    #[clap(default_value_t = false)]
    pub check: bool,
}

#[derive(Args, Clone, Debug)]
pub struct GenerateCiArgs {
    /// Check if the generated output differs from on-disk config without writing it
    #[clap(long)]
    #[clap(default_value_t = false)]
    pub check: bool,
}
#[derive(Args, Clone, Debug)]
pub struct LinkageArgs {
    /// Print human-readable output
    #[clap(long)]
    #[clap(default_value_t = false)]
    pub print_output: bool,
    /// Print output as JSON
    #[clap(long)]
    #[clap(default_value_t = false)]
    pub print_json: bool,
    #[clap(long)]
    #[clap(hide = true)]
    #[clap(default_value = "")]
    pub artifacts: String,
    /// Read linkage data from JSON rather than parsing from binaries
    #[clap(long)]
    pub from_json: Option<String>,
}

#[derive(Args, Clone, Debug)]
pub struct HelpMarkdownArgs {}

// !!!!!!!!!!!!
// HEY HEY YOU
// !!!!!!!!!!!!
// IF YOU ADD NEW FIELDS TO THIS CONSIDER MIRRORING THEM TO InitArgs!!!
#[derive(Args, Clone, Debug)]
pub struct UpdateArgs {
    /// Upgrade to a specific version, instead of "latest"
    #[clap(long)]
    pub version: Option<String>,
    /// Allow upgrading to prereleases when picking "latest"
    #[clap(long)]
    pub prerelease: bool,
    /// Automatically accept all recommended/default values
    ///
    /// This is equivalent to just mashing ENTER over and over
    /// during the interactive prompts.
    #[clap(long, short)]
    pub yes: bool,
    /// Skip running 'dist init' after performing an upgrade
    #[clap(long)]
    pub skip_init: bool,
    /// Skip running 'dist generate' at the end
    #[clap(long, alias = "no-generate-ci", alias = "no-generate")]
    pub skip_generate: bool,
    /// A path to a json file containing values to set in workspace.metadata.dist
    /// and package.metadata.dist, for building tools that edit these configs.
    ///
    /// This is the same toml => json format that `cargo metadata` produces
    /// when reporting `workspace.metadata.dist`. There is some additional
    /// hierarchy for specifying which values go to which packages, but this
    /// is currently intentionally undocumented to give us some flexibility to change it.
    #[clap(long)]
    pub with_json_config: Option<Utf8PathBuf>,
    /// releases hosting backends we want to support
    ///
    /// If left unspecified we will use the value in [workspace.metadata.dist].
    /// (If no such value exists we will use the one "native" to your CI provider)
    /// `dist init` will persist the values you pass to that location.
    #[clap(long, value_delimiter(','))]
    pub hosting: Vec<HostingStyle>,
}

/// A style of CI to generate
#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum CiStyle {
    /// Generate github CI that uploads to github releases
    Github,
}

impl CiStyle {
    /// Convert the application version of this enum to the library version
    pub fn to_lib(self) -> cargo_dist::config::CiStyle {
        match self {
            CiStyle::Github => cargo_dist::config::CiStyle::Github,
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
    /// Generates a Homebrew formula
    Homebrew,
    /// Generates an msi for each windows platform
    Msi,
}

impl InstallerStyle {
    /// Convert the application version of this enum to the library version
    pub fn to_lib(self) -> cargo_dist::config::InstallerStyle {
        match self {
            InstallerStyle::Shell => cargo_dist::config::InstallerStyle::Shell,
            InstallerStyle::Powershell => cargo_dist::config::InstallerStyle::Powershell,
            InstallerStyle::Npm => cargo_dist::config::InstallerStyle::Npm,
            InstallerStyle::Homebrew => cargo_dist::config::InstallerStyle::Homebrew,
            InstallerStyle::Msi => cargo_dist::config::InstallerStyle::Msi,
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
pub struct PlanArgs {}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
}

#[derive(Args, Clone, Debug)]
pub struct ManifestSchemaArgs {
    /// Write the manifest schema to the named file instead of stdout
    #[clap(long)]
    pub output: Option<String>,
}

#[derive(Args, Clone, Debug)]
pub struct PrintUploadFilesFromManifestArgs {
    /// The manifest to print upload files from
    #[clap(long)]
    pub manifest: String,
}

#[derive(Args, Clone, Debug)]
pub struct HostArgs {
    /// The hosting steps to perform
    #[clap(long, value_delimiter(','))]
    pub steps: Vec<HostStyle>,
}

impl HostStyle {
    /// Convert the application version of this enum to the library version
    pub fn to_lib(self) -> cargo_dist::config::HostStyle {
        match self {
            HostStyle::Check => cargo_dist::config::HostStyle::Check,
            HostStyle::Create => cargo_dist::config::HostStyle::Create,
            HostStyle::Upload => cargo_dist::config::HostStyle::Upload,
            HostStyle::Release => cargo_dist::config::HostStyle::Release,
            HostStyle::Announce => cargo_dist::config::HostStyle::Announce,
        }
    }
}

/// What parts of hosting to perform
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum HostStyle {
    /// Check that hosting is properly setup without doing other effects
    Check,
    /// Create a location to host artifacts
    Create,
    /// Upload artifacts
    Upload,
    /// Release artifacts
    Release,
    /// Announce artifacts
    Announce,
}

impl HostingStyle {
    /// Convert the application version of this enum to the library version
    pub fn to_lib(self) -> cargo_dist::config::HostingStyle {
        match self {
            HostingStyle::Github => cargo_dist::config::HostingStyle::Github,
            HostingStyle::Axodotdev => cargo_dist::config::HostingStyle::Axodotdev,
        }
    }
}

/// Hosting Providers
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum HostingStyle {
    /// Host on Github Releases
    Github,
    /// Host on Axo Releases ("Abyss")
    Axodotdev,
}

impl std::fmt::Display for HostingStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            HostingStyle::Github => "github",
            HostingStyle::Axodotdev => "axodotdev",
        };
        string.fmt(f)
    }
}

impl Cli {
    pub fn tag_settings(&self, needs_coherence: bool) -> TagSettings {
        TagSettings {
            needs_coherence,
            tag: if let Some(tag) = &self.tag {
                if tag == "timestamp" {
                    assert!(
                        self.force_tag,
                        "--tag=timestamp currently requires --force-tag"
                    );
                    TagMode::ForceMaxAndTimestamp
                } else if self.force_tag {
                    TagMode::Force(tag.clone())
                } else {
                    TagMode::Select(tag.clone())
                }
            } else {
                TagMode::Infer
            },
        }
    }
}
