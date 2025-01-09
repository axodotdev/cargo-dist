//! Errors!
//!
//! This module is kind of pointless and stubbed out right now,
//! because the crate is currently opting for a "typeless" approach
//! (where everything gets folded into miette::Report right away).
//!
//! If we ever change this decision, this will be a lot more important!

use axoproject::errors::AxoprojectError;
use backtrace::Backtrace;
use camino::Utf8PathBuf;
use cargo_dist_schema::{target_lexicon::Triple, ArtifactId, TripleName};
use color_backtrace::BacktracePrinter;
use console::style;
use miette::{Diagnostic, SourceOffset, SourceSpan};
use thiserror::Error;

/// An alias for the common Result type for this crate
pub type DistResult<T> = std::result::Result<T, DistError>;

/// Errors dist can have
#[derive(Debug, Error, Diagnostic)]
pub enum DistError {
    /// random i/o error
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// random axoasset error
    #[error(transparent)]
    #[diagnostic(transparent)]
    Asset(#[from] axoasset::AxoassetError),

    /// random axoprocess error
    #[error(transparent)]
    #[diagnostic(transparent)]
    Cmd(#[from] axoprocess::AxoprocessError),

    /// random gazenot error
    #[error(transparent)]
    #[diagnostic(transparent)]
    Gazenot(#[from] gazenot::error::GazenotError),

    /// random gazenot error
    #[error(transparent)]
    #[diagnostic(transparent)]
    Project(#[from] axoproject::errors::ProjectError),

    /// random string error
    #[error(transparent)]
    FromUtf8Error(#[from] std::string::FromUtf8Error),

    /// random i/o error
    #[error(transparent)]
    Goblin(#[from] goblin::error::Error),

    /// random camino conversion error
    #[error(transparent)]
    FromPathBufError(#[from] camino::FromPathBufError),

    /// random dialoguer error
    #[error(transparent)]
    DialoguerError(#[from] dialoguer::Error),

    /// random axotag error
    #[error(transparent)]
    #[diagnostic(transparent)]
    AxotagError(#[from] axotag::errors::TagError),

    /// random parseint error
    #[error(transparent)]
    ParseIntError(#[from] std::num::ParseIntError),

    /// random triple parse error
    #[error(transparent)]
    TripleError(#[from] cargo_dist_schema::target_lexicon::ParseError),

    /// error when using axoasset::toml::to_string() or similar
    #[error(transparent)]
    AxoassetTomlSerErr(#[from] axoasset::toml::ser::Error),

    /// A problem with a jinja template, which is always a dist bug
    #[error("Failed to render template")]
    #[diagnostic(
        help("this is a bug in dist, let us know and we'll fix it: https://github.com/axodotdev/cargo-dist/issues/new")
    )]
    Jinja {
        /// The SourceFile we were try to parse
        #[source_code]
        source: String,
        /// The range the error was found on
        #[label]
        span: Option<miette::SourceSpan>,
        /// Details of the error
        #[source]
        backtrace: JinjaErrorWithBacktrace,
    },

    /// Error from (cargo-)wix
    #[error("WiX returned an error while building {msi}")]
    Wix {
        /// The msi we were trying to build
        msi: String,
        /// The underlying wix error
        #[source]
        details: wix::Error,
    },

    /// Error from (cargo-)wix init
    #[error("Couldn't generate main.wxs for {package}'s msi installer")]
    WixInit {
        /// The package
        package: String,
        /// The underlying wix error
        #[source]
        details: wix::Error,
    },

    /// Error parsing metadata in Cargo.toml (json because it's from cargo-metadata)
    #[error("Malformed metadata.dist in\n{manifest_path}")]
    #[diagnostic(help("you can find a reference for the configuration schema at https://opensource.axo.dev/cargo-dist/book/reference/config.html"))]
    CargoTomlParse {
        /// path to file
        manifest_path: Utf8PathBuf,
        /// Inner error
        #[source]
        cause: serde_json::Error,
    },

    /// User declined to update dist, refuse to make progress
    #[error("to update your dist config you must use the version your project is configured for")]
    #[diagnostic(help(
        "you're running {running_version} but the project is configured for {project_version}"
    ))]
    NoUpdateVersion {
        /// Version the config had
        project_version: semver::Version,
        /// Version they're running
        running_version: semver::Version,
    },

    /// User tried to enable Github CI support but had inconsistent urls for the repo
    #[error("Github CI support requires your crates to agree on the URL of your repository")]
    CantEnableGithubUrlInconsistent {
        /// inner error that caught this
        #[diagnostic_source]
        inner: AxoprojectError,
    },

    /// User tried to enable Github CI support but no url for the repo
    #[error("Github CI support requires you to specify the URL of your repository")]
    #[diagnostic(help(
        r#"Set the repository = "https://github.com/..." key in these manifests: {manifest_list}"#
    ))]
    CantEnableGithubNoUrl {
        /// List of manifests to change
        manifest_list: String,
    },

    /// We got a repository URL but couldn't interpret it as a GitHub repo
    #[error("GitHub CI support requires a GitHub repository")]
    CantEnableGithubUrlNotGithub {
        /// inner error that caught this
        #[diagnostic_source]
        inner: AxoprojectError,
    },

    /// User supplied an illegal npm scope
    #[error("The npm-scope field must be an all-lowercase value; the supplied value was {scope}")]
    ScopeMustBeLowercase {
        /// The incorrectly-formatted scope
        scope: String,
    },

    /// Completely unknown format to install-path
    ///
    /// NOTE: we can't use `diagnostic(help)` here because this will get crammed into
    /// a serde_json error, reducing it to a String. So we inline the help!
    #[error(r#"install-path = "{path}" has an unknown format (it can either be "CARGO_HOME", "~/subdir/", or "$ENV_VAR/subdir/")"#)]
    InstallPathInvalid {
        /// The full value passed to install-path
        path: String,
    },

    /// Being pedantic about the env-var mode of install-path to be consistent
    ///
    /// NOTE: we can't use `diagnostic(help)` here because this will get crammed into
    /// a serde_json error, reducing it to a String. So we inline the help!
    #[error(r#"install-path = "{path}" is missing a subdirectory (add a trailing slash if you want no subdirectory)"#)]
    InstallPathEnvSlash {
        /// The full value passed to install-path
        path: String,
    },

    /// Being pedantic about the home mode of install-path to be consistent
    ///
    /// NOTE: we can't use `diagnostic(help)` here because this will get crammed into
    /// a serde_json error, reducing it to a String. So we inline the help!
    #[error(r#"install-path = "{path}" is missing a subdirectory (installing directly to home isn't allowed)"#)]
    InstallPathHomeSubdir {
        /// The full value passed to install-path
        path: String,
    },

    /// explicitly requested workspace builds, but had packages with custom feature settings
    #[error("precise-builds = false was set, but some packages have custom build features, making it impossible")]
    #[diagnostic(help("these packages customized either features, no-default-features, or all-features:\n{packages:#?}"))]
    PreciseImpossible {
        /// paths of problem manifests
        packages: Vec<camino::Utf8PathBuf>,
    },

    /// packages disagreed on homebrew taps
    #[error("different homebrew taps were set in your workspace, this is currently unsupported")]
    #[diagnostic(help("these packages disagree:\n{packages:#?}"))]
    MismatchedTaps {
        /// paths of problem manifests
        packages: Vec<camino::Utf8PathBuf>,
    },

    /// packages disagreed on publishers
    #[error("different publisher settings were in your workspace, this is currently unuspported")]
    #[diagnostic(help("these packages disagree:\n{packages:#?}"))]
    MismatchedPublishers {
        /// paths of problem manifests
        packages: Vec<camino::Utf8PathBuf>,
    },

    /// publishers disagreed on prereleases
    #[error("different publisher 'prereleases' settings were in your workspace, this is currently unsupported")]
    MismatchedPrereleases,

    /// parse_tag concluded there was nothing to release
    #[error("This workspace doesn't have anything for dist to Release!")]
    NothingToRelease {
        /// full help printout (very dynamic)
        #[help]
        help: String,
    },

    /// parse_tag concluded there are too many unrelated things for a single tag
    #[error("There are too many unrelated apps in your workspace to coherently Announce!")]
    TooManyUnrelatedApps {
        /// full help printout (very dynamic)
        #[help]
        help: String,
    },

    /// Not an error; indicates that a file's contents differ via --check
    #[error("{} has out of date contents and needs to be regenerated:\n{diff}", file.origin_path())]
    #[diagnostic(help("run 'dist init' to update the file\n('allow-dirty' in Cargo.toml to ignore out of date contents)"))]
    CheckFileMismatch {
        /// The file whose contents differ
        file: axoasset::SourceFile,
        /// The diff
        diff: String,
    },

    /// `dist generate` was passed an explicit GenerateMode but the config in their Cargo.toml
    /// has that mode set to allow-dirty, a contradiction!
    #[error("'{generate_mode}' is marked as allow-dirty in your dist config, refusing to run")]
    ContradictoryGenerateModes {
        /// The problematic mode
        generate_mode: crate::config::GenerateMode,
    },

    /// msi/pkg with too many packages
    #[error("{artifact_name} depends on multiple packages, which isn't yet supported")]
    #[diagnostic(help("depends on {spec1} and {spec2}"))]
    MultiPackage {
        /// Name of the artifact
        artifact_name: ArtifactId,
        /// One of the packages
        spec1: String,
        /// A different package
        spec2: String,
    },

    /// msi/pkg with too few packages
    #[error("{artifact_name} has no binaries")]
    #[diagnostic(help("This should be impossible, you did nothing wrong, please file an issue!"))]
    NoPackage {
        /// Name of the msi
        artifact_name: ArtifactId,
    },

    /// These GUIDs for msi's are required and enforced by `dist generate --check`
    #[error("missing WiX GUIDs in {manifest_path}: {keys:?}")]
    #[diagnostic(help("run 'dist init' to generate them"))]
    MissingWixGuids {
        /// The Cargo.toml missing them
        manifest_path: Utf8PathBuf,
        /// The missing keys
        keys: &'static [&'static str],
    },

    /// unrecognized job style
    #[error("{style} is not a recognized job value")]
    #[diagnostic(help("Jobs that do not come with dist should be prefixed with ./"))]
    UnrecognizedJobStyle {
        /// value provided
        style: String,
    },

    /// unrecognized hosting style
    #[error("{style} is not a recognized release host")]
    UnrecognizedHostingStyle {
        /// value provided
        style: String,
    },

    /// unrecognized hosting style
    #[error("No GitHub hosting is defined!")]
    #[diagnostic(help("Releases must have at least GitHub hosting for updates to be supported."))]
    NoGitHubHosting {},

    /// unrecognized ci style
    #[error("{style} is not a recognized ci provider")]
    UnrecognizedCiStyle {
        /// value provided
        style: String,
    },

    /// unrecognized library style
    #[error("{style} is not a recognized type of library")]
    UnrecognizedLibraryStyle {
        /// value provided
        style: String,
    },

    /// Linkage report can't be run for this combination of OS and target
    #[error("unable to run linkage report for {target} on {host}")]
    LinkageCheckInvalidOS {
        /// The OS the check was run on
        host: String,
        /// The OS being checked
        target: TripleName,
    },

    /// Linkage report can't be run for this target
    #[error("unable to run linkage report for this type of binary")]
    LinkageCheckUnsupportedBinary,

    /// Error parsing a string containing an environment variable
    /// in VAR=value syntax
    #[error("Unable to parse environment variable as a key/value pair: {line}")]
    #[diagnostic(help("This should be impossible, you did nothing wrong, please file an issue!"))]
    EnvParseError {
        /// The line of text that couldn't be parsed
        line: String,
    },

    /// An error running `git archive`
    #[error("We failed to generate a source tarball for your project")]
    #[diagnostic(help("This is probably not your fault, please file an issue!"))]
    GitArchiveError {},

    /// An error running `git -C path rev-parse HEAD`
    #[error("We failed to query information about the git submodule at\n{path}")]
    #[diagnostic(help("Does a submodule exist at that path? Has it been fetched with `git submodule update --init`?"))]
    GitSubmoduleCommitError {
        /// The path we failed to fetch
        path: String,
    },

    /// A required tool is missing
    #[error("{tool}, required to run this task, is missing")]
    #[diagnostic(help("Ensure {tool} is installed"))]
    ToolMissing {
        /// the name of the missing tool
        tool: String,
    },

    /// One or more required tools are missing.
    #[error("The following tools are required to run this task, but are missing:\n- {}", tools.join("\n- "))]
    #[diagnostic(help("Please install the tools mentioned above and try again."))]
    EnvToolsMissing {
        /// the names of the missing tools
        tools: Vec<String>,
    },

    /// Unknown target requested
    #[error(
        "A build was requested for {target}, but the standalone updater isn't available for it."
    )]
    #[diagnostic(help("At the moment, we can only provide updater binaries for the core set of most common target triples. Please set `install-updater = false` in your config."))]
    NoAxoupdaterForTarget {
        /// The target triple being built for
        target: String,
    },

    /// reqwest returned non-2xx/404 when checking axoupdater releases
    #[error("Failed to check the latest release of axoupdater")]
    #[diagnostic(help(
        "Is your internet connection working? If not, this may be a bug; please file an issue!"
    ))]
    AxoupdaterReleaseCheckFailed {},

    /// Failed to determine how to uncompress something
    #[error("Failed to determine compression format")]
    #[diagnostic(help("File extension of unrecognized file was {extension}"))]
    UnrecognizedCompression {
        /// The file extension of the unrecognized file
        extension: String,
    },

    /// Binaries were missing
    #[error("failed to find bin {bin_name} for {pkg_name}")]
    #[diagnostic(help("did the above build fail?"))]
    MissingBinaries {
        /// Name of package
        pkg_name: String,
        /// Name of binary
        bin_name: String,
    },

    /// Error during `dist selfupdate`
    #[error("`dist selfupdate` failed; the new version isn't in the place we expected")]
    #[diagnostic(help("This is probably not your fault, please file an issue!"))]
    UpdateFailed {},

    /// Trying to run dist selfupdate in a random dir
    #[error("`dist selfupdate` needs to be run in a project")]
    #[diagnostic(help("If you just want to update dist and not your project, pass --skip-init"))]
    UpdateNotInWorkspace {
        /// The report about the missing workspace
        #[diagnostic_source]
        cause: axoproject::errors::ProjectError,
    },

    /// Trying to include CargoHome with other install paths
    #[error("Incompatible install paths configured in Cargo.toml")]
    #[diagnostic(help("The CargoHome `install-path` configuration can't be combined with other install path strategies."))]
    IncompatibleInstallPathConfiguration,

    /// Passed --artifacts but no --target
    #[error("You specified --artifacts, disabling host mode, but specified no targets to build!")]
    #[diagnostic(help("try adding --target={host_target}"))]
    CliMissingTargets {
        /// Current host target
        host_target: TripleName,
    },

    /// Workspace isn't init
    #[error("please run 'dist init' before running any other commands!")]
    NeedsInit,

    /// Running different version from config
    #[error("You're running dist {running_version}, but 'cargo-dist-version = {config_version}' is set in your Cargo.toml")]
    #[diagnostic(help("Rerun 'dist init' to update to this version."))]
    MismatchedDistVersion {
        /// config version
        config_version: String,
        /// running version
        running_version: String,
    },

    /// Failed to make sense of 'cargo -vV'
    #[error("Failed to get get toolchain version from 'cargo -vV'")]
    FailedCargoVersion,

    /// Failed to parse Github repo pair
    #[error("Failed to parse github repo: {pair}")]
    #[diagnostic(help("should be 'owner/repo' format"))]
    GithubRepoPairParse {
        /// The input
        pair: String,
    },

    /// Unknown permission specified in GitHub Actions config
    #[error("One or more unrecognized permissions levels were specified: {levels:?}")]
    #[diagnostic(help("recognized values are: admin, write, read"))]
    GithubUnknownPermission {
        /// The input
        levels: Vec<String>,
    },

    /// An unknown target was found
    #[error("Unrecognized target: {target}")]
    #[diagnostic(help("The full list of supported targets can be found here: https://opensource.axo.dev/cargo-dist/book/reference/config.html#targets"))]
    UnrecognizedTarget {
        /// The target in question
        target: TripleName,
    },

    /// Installers requested despite having nothing to install
    #[error("Installers were requested, but app contains no installable binaries")]
    #[diagnostic(help(
        "The only installable files are libraries, but `install-libraries` isn't enabled."
    ))]
    EmptyInstaller {},

    /// Configuration value for github-build-setup defined but not found
    #[error("failed to load github-build-setup file")]
    GithubBuildSetupNotFound {
        /// Inner i/o error with file path
        #[diagnostic_source]
        details: axoasset::AxoassetError,
    },

    /// Configuration value for github-build-setup defined but not found
    #[error("github-build-setup file wasn't valid yaml")]
    GithubBuildSetupParse {
        /// Inner parse error with path and spans
        #[diagnostic_source]
        details: axoasset::AxoassetError,
    },

    /// github-build-setup file contents are invalid
    #[error("github-build-setup file at {file_path} was invalid: {message}")]
    #[diagnostic(help(
        "For more details about writing build steps see: https://docs.github.com/en/actions/using-workflows/workflow-syntax-for-github-actions#jobsjob_idstepsid"
    ))]
    GithubBuildSetupNotValid {
        /// The value from the configuration file
        file_path: Utf8PathBuf,
        /// Error message details
        message: String,
    },

    /// Something has metadata.dist but shouldn't
    #[error("The metadata.dist entry in this Cargo.toml isn't being used:\n{manifest_path}")]
    #[diagnostic(help(
        "You probably want to move them to the [dist] section in\n{dist_manifest_path}"
    ))]
    UnusedMetadata {
        /// The manifest that had the metadata.dist
        manifest_path: Utf8PathBuf,
        /// The dist.toml/dist-workspace.toml that means it's ignored
        dist_manifest_path: Utf8PathBuf,
    },

    /// Build command looks like they put args in same string as command
    #[error("Your build-command's arguments need to be split up\n{manifest}\ncommand was: [\"{command}\"]")]
    #[diagnostic(help("the command should be split [\"like\", \"--this\", \"--array=here\"]"))]
    SusBuildCommand {
        /// path to manifest
        manifest: Utf8PathBuf,
        /// what the command was
        command: String,
    },

    /// missing "dist" script in a package.json
    #[error("package.json was missing a \"dist\" script\n{manifest}")]
    #[diagnostic(help(
        "https://opensource.axo.dev/cargo-dist/book/quickstart/javascript.html#adding-a-dist-script"
    ))]
    NoDistScript {
        /// path to package.json
        manifest: Utf8PathBuf,
    },

    /// Unsupported cross-compilation host/target combination
    #[error("Cross-compilation from {host} to {target} is not supported")]
    #[diagnostic(help("{details}"))]
    UnsupportedCrossCompile {
        /// The host system
        host: Triple,
        /// The target system
        target: Triple,
        /// Additional details about why this combination is unsupported
        details: String,
    },

    /// Cannot use cross-compilation with cargo-auditable
    #[error(
        "Cross-compilation builds from {host} to {target} cannot be used with cargo-auditable"
    )]
    #[diagnostic(help("set cargo-auditable to false or don't do cross-compilation"))]
    CannotDoCargoAuditableAndCrossCompile {
        /// The host system
        host: Triple,
        /// The target system
        target: Triple,
    },

    /// Generic build with Cargo-only build options
    #[error("You're building a generic package but have a Cargo-only option enabled")]
    #[diagnostic(help("Please disable the following from your configuration: {}", options.join(", ")))]
    CargoOnlyBuildOptions {
        /// The names of the invalid options
        options: Vec<String>,
    },

    /// missing "build-command" for a package that needs one
    #[error("dist package was missing a build-command\n{manifest}")]
    #[diagnostic(help(
        "https://opensource.axo.dev/cargo-dist/book/quickstart/everyone-else.html#setup"
    ))]
    NoBuildCommand {
        /// path to manifest
        manifest: Utf8PathBuf,
    },

    /// cargo package with build-command
    #[error(
        "cargo package was overridden with a build-command, which isn't supported yet\n{manifest}"
    )]
    UnexpectedBuildCommand {
        /// path to manifest
        manifest: Utf8PathBuf,
    },

    /// Failure to decode base64-encoded certificate
    #[error("We failed to decode the certificate stored in the CODESIGN_CERTIFICATE environment variable.")]
    #[diagnostic(help("Is the value of this environment variable valid base64?"))]
    CertificateDecodeError {},

    /// Missing configuration for a .pkg
    #[error("A Mac .pkg installer was requested, but the config is missing")]
    #[diagnostic(help("Please ensure a dist.mac-pkg-config section is present in your config. For more details see: https://example.com"))]
    MacPkgConfigMissing {},

    /// User left identifier empty in init
    #[error("No bundle identifier was specified")]
    #[diagnostic(help("Please either enter a bundle identifier, or disable the Mac .pkg"))]
    MacPkgBundleIdentifierMissing {},

    /// Project depends on a too-old axoupdater
    #[error("Your project ({package_name}) uses axoupdater as a library, but the version specified ({your_version}) is older than the minimum supported version ({minimum}). (The dependency comes via {source_name} in the dependency tree.)")]
    #[diagnostic(help(
        "https://opensource.axo.dev/cargo-dist/book/installers/updater.html#minimum-supported-version-checking"
    ))]
    AxoupdaterTooOld {
        /// Name of the package
        package_name: String,
        /// The package that depended on axoupdater
        source_name: String,
        /// Minimum supported version
        minimum: semver::Version,
        /// Version the project uses
        your_version: semver::Version,
    },

    /// Project is using a v0 config
    #[error("Your project is using an old configuration format. Please run `dist init` to migrate to the new format.")]
    //#[diagnostic(help("???? probably provide a link ???")]
    OldConfigFormat {},

    /// No dist manifest
    #[error("No manifest path specified")]
    NoDistManifest {},
}

impl From<minijinja::Error> for DistError {
    fn from(details: minijinja::Error) -> Self {
        let source: String = details.template_source().unwrap_or_default().to_owned();
        let span = details.range().map(|r| r.into()).or_else(|| {
            details.line().map(|line| {
                // some minijinja errors only have a line, not a range, so let's just highlight the whole line
                let start = SourceOffset::from_location(&source, line, 0);
                let end = SourceOffset::from_location(&source, line + 1, 0);
                let len = (end.offset() - start.offset()).wrapping_sub(1);
                SourceSpan::from((start, len))
            })
        });

        DistError::Jinja {
            source,
            span,
            backtrace: JinjaErrorWithBacktrace::new(details),
        }
    }
}
/// A struct that implements `std::error::Error` so it can be added as "related" to
/// a miette diagnostic, and it'll show the backtrace.
#[derive(Debug)]
pub struct JinjaErrorWithBacktrace {
    /// The original minijinja error
    pub error: minijinja::Error,
    /// The captured backtrace
    backtrace: Backtrace,
}

impl JinjaErrorWithBacktrace {
    fn new(error: minijinja::Error) -> Self {
        Self {
            error,
            backtrace: Backtrace::new_unresolved(),
        }
    }
}

impl std::error::Error for JinjaErrorWithBacktrace {}

impl std::fmt::Display for JinjaErrorWithBacktrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut bt = self.backtrace.clone();
        bt.resolve();
        let backtrace = BacktracePrinter::new()
            .add_frame_filter(Box::new(|frames| {
                // try to find index of a frame that says `dist::real_main` and remove everything after it
                if let Some(real_main_idx) = frames.iter().position(|f| {
                    f.name
                        .as_ref()
                        .map(|n| n.contains("real_main"))
                        .unwrap_or(false)
                }) {
                    frames.splice(real_main_idx + 1.., vec![]);
                }
            }))
            .format_trace_to_string(&bt)
            .unwrap();
        write!(
            f,
            "Backtrace:\n{}\n{}",
            style(&backtrace).dim(),
            style(&self.error).red()
        )
    }
}
