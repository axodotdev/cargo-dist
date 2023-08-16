//! Config types (for workspace.metadata.dist)

use axoproject::WorkspaceSearch;
use camino::{Utf8Path, Utf8PathBuf};
use miette::Report;
use semver::Version;
use serde::{Deserialize, Serialize};
use tracing::log::warn;

use crate::errors::Result;
use crate::{
    errors::{DistError, DistResult},
    TargetTriple, METADATA_DIST,
};

/// Contents of METADATA_DIST in Cargo.toml files
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct DistMetadata {
    /// The intended version of cargo-dist to build with. (normal Cargo SemVer syntax)
    ///
    /// When generating full tasks graphs (such as CI scripts) we will pick this version.
    ///
    /// FIXME: Should we produce a warning if running locally with a different version? In theory
    /// it shouldn't be a problem and newer versions should just be Better... probably you
    /// Really want to have the exact version when running generate-ci to avoid generating
    /// things other cargo-dist versions can't handle!
    #[serde(rename = "cargo-dist-version")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cargo_dist_version: Option<Version>,

    /// (deprecated) The intended version of Rust/Cargo to build with (rustup toolchain syntax)
    ///
    /// When generating full tasks graphs (such as CI scripts) we will pick this version.
    #[serde(rename = "rust-toolchain-version")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rust_toolchain_version: Option<String>,

    /// Whether the package should be distributed/built by cargo-dist
    ///
    /// This mainly exists to be set to `false` to make cargo-dist ignore the existence of this
    /// package. Note that we may still build the package as a side-effect of building the
    /// workspace -- we just won't bundle it up and report it.
    ///
    /// FIXME: maybe you should also be allowed to make this a list of binary names..?
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dist: Option<bool>,

    /// CI environments you wish to target.
    ///
    /// Currently only accepts "github".
    ///
    /// When running `generate-ci` with no arguments this list will be used.
    ///
    /// This value isn't Optional because it's global, and therefore can't be overriden by packages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ci: Option<Vec<CiStyle>>,

    /// The full set of installers you would like to produce
    ///
    /// When generating full task graphs (such as CI scripts) we will try to generate these.
    ///
    /// Some installers can be generated on any platform (like shell scripts) while others
    /// may (currently) require platform-specific toolchains (like .msi installers). Some
    /// installers may also be "per release" while others are "per build". Again, shell script
    /// vs msi is a good comparison here -- you want a universal shell script that figures
    /// out which binary to install, but you might end up with an msi for each supported arch!
    ///
    /// Currently accepted values:
    ///
    /// * shell
    /// * powershell
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installers: Option<Vec<InstallerStyle>>,

    /// The full set of target triples to build for.
    ///
    /// When generating full task graphs (such as CI scripts) we will to try to generate these.
    ///
    /// The inputs should be valid rustc target triples (see `rustc --print target-list`) such
    /// as `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, or `x86_64-unknown-linux-gnu`.
    ///
    /// FIXME: We should also accept one magic target: `universal2-apple-darwin`. This will induce
    /// us to build `x86_64-apple-darwin` and `aarch64-apple-darwin` (arm64) and then combine
    /// them into a "universal" binary that can run on either arch (using apple's `lipo` tool).
    ///
    /// FIXME: Allow higher level requests like "[macos, windows, linux] x [x86_64, aarch64]"?
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<String>>,

    /// Include the following static files in bundles like executable-zips.
    ///
    /// Paths are relative to the Cargo.toml this is defined in.
    ///
    /// Files like `README*`, `(UN)LICENSE*`, `RELEASES*`, and `CHANGELOG*` are already
    /// automatically detected and included (use [`DistMetadata::auto_includes`][] to prevent this).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<Utf8PathBuf>>,

    /// Whether to auto-include files like `README*`, `(UN)LICENSE*`, `RELEASES*`, and `CHANGELOG*`
    ///
    /// Defaults to true.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "auto-includes")]
    pub auto_includes: Option<bool>,

    /// The archive format to use for windows builds (defaults .zip)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "windows-archive")]
    pub windows_archive: Option<ZipStyle>,

    /// The archive format to use for non-windows builds (defaults .tar.xz)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "unix-archive")]
    pub unix_archive: Option<ZipStyle>,

    /// A scope to prefix npm packages with (@ should be included).
    ///
    /// This is required if you're using an npm installer.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "npm-scope")]
    pub npm_scope: Option<String>,

    /// A scope to prefix npm packages with (@ should be included).
    ///
    /// This is required if you're using an npm installer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<ChecksumStyle>,

    /// Build only the required packages, and individually (since 0.1.0) (default: false)
    ///
    /// By default when we need to build anything in your workspace, we build your entire workspace
    /// with --workspace. This setting tells cargo-dist to instead build each app individually.
    ///
    /// On balance, the Rust experts we've consulted with find building with --workspace to
    /// be a safer/better default, as it provides some of the benefits of a more manual
    /// [workspace-hack][], without the user needing to be aware that this is a thing.
    ///
    /// TL;DR: cargo prefers building one copy of each dependency in a build, so if two apps in
    /// your workspace depend on e.g. serde with different features, building with --workspace,
    /// will build serde once with the features unioned together. However if you build each
    /// package individually it will more precisely build two copies of serde with different
    /// feature sets.
    ///
    /// The downside of using --workspace is that if your workspace has lots of example/test
    /// crates, or if you release only parts of your workspace at a time, we build a lot of
    /// gunk that's not needed, and potentially bloat up your app with unnecessary features.
    ///
    /// If that downside is big enough for you, this setting is a good idea.
    ///
    /// [workspace-hack]: https://docs.rs/cargo-hakari/latest/cargo_hakari/about/index.html
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "precise-builds")]
    pub precise_builds: Option<bool>,

    /// Whether we should try to merge otherwise-parallelizable tasks onto the same machine,
    /// sacrificing latency and fault-isolation for more the sake of minor effeciency gains.
    ///
    /// (defaults to false)
    ///
    /// For example, if you build for x64 macos and arm64 macos, by default we will generate ci
    /// which builds those independently on separate logical machines. With this enabled we will
    /// build both of those platforms together on the same machine, making it take twice as long
    /// as any other build and making it impossible for only one of them to succeed.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "merge-tasks")]
    pub merge_tasks: Option<bool>,

    /// Whether failing tasks should make us give up on all other tasks
    ///
    /// (defaults to false)
    ///
    /// When building a release you might discover that an obscure platform's build is broken.
    /// When this happens you have two options: give up on the release entirely (`fail-fast = true`),
    /// or keep trying to build all the other platforms anyway (`fail-fast = false`).
    ///
    /// cargo-dist was designed around the "keep trying" approach, as we create a draft Release
    /// and upload results to it over time, undrafting the release only if all tasks succeeded.
    /// The idea is that even if a platform fails to build, you can decide that's acceptable
    /// and manually undraft the release with some missing platforms.
    ///
    /// (Note that the dist-manifest.json is produced before anything else, and so it will assume
    /// that all tasks succeeded when listing out supported platforms/artifacts. This may make
    /// you sad if you do this kind of undrafting and also trust the dist-manifest to be correct.)
    ///
    /// Prior to 0.1.0 we didn't set the correct flags in our CI scripts to do this, but now we do.
    /// This flag was introduced to allow you to restore the old behaviour if you prefer.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "fail-fast")]
    pub fail_fast: Option<bool>,

    /// The strategy to use for selecting a path to install things at:
    ///
    /// * `CARGO_HOME`: (default) install as if cargo did
    ///   (try `$CARGO_HOME/bin/`, but if `$CARGO_HOME` isn't set use `$HOME/.cargo/bin/`)
    /// * `~/some/subdir/`: install to the given subdir of the user's `$HOME`
    /// * `$SOME_VAR/some/subdir`: install to the given subdir of the dir defined by `$SOME_VAR`
    ///
    /// All of these error out if the required env-vars aren't set. In the future this may
    /// allow for the input to be an array of options to try in sequence.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "install-path")]
    pub install_path: Option<InstallPathStrategy>,
}

impl DistMetadata {
    /// Apply the base path to any relative paths contained in this DistMetadata
    pub fn make_relative_to(&mut self, base_path: &Utf8Path) {
        // This is intentionally written awkwardly to make you update it
        let DistMetadata {
            cargo_dist_version: _,
            rust_toolchain_version: _,
            dist: _,
            ci: _,
            installers: _,
            targets: _,
            include,
            auto_includes: _,
            windows_archive: _,
            unix_archive: _,
            npm_scope: _,
            checksum: _,
            precise_builds: _,
            fail_fast: _,
            merge_tasks: _,
            install_path: _,
        } = self;
        if let Some(include) = include {
            for include in include {
                *include = base_path.join(&*include);
            }
        }
    }

    /// Merge a workspace config into a package config (self)
    pub fn merge_workspace_config(
        &mut self,
        workspace_config: &Self,
        package_manifest_path: &Utf8Path,
    ) {
        // This is intentionally written awkwardly to make you update it
        let DistMetadata {
            cargo_dist_version,
            rust_toolchain_version,
            dist,
            ci,
            installers,
            targets,
            include,
            auto_includes,
            windows_archive,
            unix_archive,
            npm_scope,
            checksum,
            precise_builds,
            merge_tasks,
            fail_fast,
            install_path,
        } = self;

        // Check for global settings on local packages
        if cargo_dist_version.is_some() {
            warn!("package.metadata.dist.cargo-dist-version is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if rust_toolchain_version.is_some() {
            warn!("package.metadata.dist.rust-toolchain-version is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if ci.is_some() {
            warn!("package.metadata.dist.ci is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if precise_builds.is_some() {
            warn!("package.metadata.dist.precise-builds is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if merge_tasks.is_some() {
            warn!("package.metadata.dist.merge-tasks is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }
        if fail_fast.is_some() {
            warn!("package.metadata.dist.fail-fast is set, but this is only accepted in workspace.metadata (value is being ignored): {}", package_manifest_path);
        }

        // Merge non-global settings
        if installers.is_none() {
            *installers = workspace_config.installers.clone();
        }
        if targets.is_none() {
            *targets = workspace_config.targets.clone();
        }
        if dist.is_none() {
            *dist = workspace_config.dist;
        }
        if auto_includes.is_none() {
            *auto_includes = workspace_config.auto_includes;
        }
        if windows_archive.is_none() {
            *windows_archive = workspace_config.windows_archive;
        }
        if unix_archive.is_none() {
            *unix_archive = workspace_config.unix_archive;
        }
        if npm_scope.is_none() {
            *npm_scope = workspace_config.npm_scope.clone();
        }
        if checksum.is_none() {
            *checksum = workspace_config.checksum;
        }
        if install_path.is_none() {
            *install_path = workspace_config.install_path.clone();
        }

        // This was historically implemented as extend, but I'm not convinced the
        // inconsistency is worth the inconvenience...
        if let Some(include) = include {
            if let Some(workspace_include) = &workspace_config.include {
                include.extend(workspace_include.iter().cloned());
            }
        } else {
            *include = workspace_config.include.clone();
        }
    }
}

/// Global config for commands
#[derive(Debug)]
pub struct Config {
    /// Whether we need to compute an announcement tag or if we can fudge it
    ///
    /// Commands like generate-ci and init don't need announcements, but want to run gather_work
    pub needs_coherent_announcement_tag: bool,
    /// The subset of artifacts we want to build
    pub artifact_mode: ArtifactMode,
    /// Whether local paths to files should be in the final dist json output
    pub no_local_paths: bool,
    /// Target triples we want to build for
    pub targets: Vec<TargetTriple>,
    /// CI kinds we want to support
    pub ci: Vec<CiStyle>,
    /// Installers we want to generate
    pub installers: Vec<InstallerStyle>,
    /// The (git) tag to use for this Announcement.
    pub announcement_tag: Option<String>,
}

/// How we should select the artifacts to build
#[derive(Clone, Copy, Debug)]
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

/// The style of CI we should generate
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum CiStyle {
    /// Generate Github CI
    #[serde(rename = "github")]
    Github,
}

impl std::fmt::Display for CiStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            CiStyle::Github => "github",
        };
        string.fmt(f)
    }
}

/// The style of Installer we should generate
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum InstallerStyle {
    /// Generate a shell script that fetches from [`crate::tasks::DistGraph::artifact_download_url`][]
    #[serde(rename = "shell")]
    Shell,
    /// Generate a powershell script that fetches from [`crate::tasks::DistGraph::artifact_download_url`][]
    #[serde(rename = "powershell")]
    Powershell,
    /// Generate an npm project that fetches from [`crate::tasks::DistGraph::artifact_download_url`][]
    #[serde(rename = "npm")]
    Npm,
    /// Generate a Homebrew formula that fetches from [`crate::tasks::DistGraph::artifact_download_url`][]
    #[serde(rename = "homebrew")]
    Homebrew,
}

impl std::fmt::Display for InstallerStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            InstallerStyle::Shell => "shell",
            InstallerStyle::Powershell => "powershell",
            InstallerStyle::Npm => "npm",
            InstallerStyle::Homebrew => "homebrew",
        };
        string.fmt(f)
    }
}

/// The style of zip/tarball to make
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipStyle {
    /// `.zip`
    Zip,
    /// `.tar.<compression>`
    Tar(CompressionImpl),
}

/// Compression impls (used by [`ZipStyle::Tar`][])
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CompressionImpl {
    /// `.gz`
    Gzip,
    /// `.xz`
    Xzip,
    /// `.zstd`
    Zstd,
}
impl ZipStyle {
    /// Get the extension used for this kind of zip
    pub fn ext(&self) -> &'static str {
        match self {
            ZipStyle::Zip => ".zip",
            ZipStyle::Tar(compression) => match compression {
                CompressionImpl::Gzip => ".tar.gz",
                CompressionImpl::Xzip => ".tar.xz",
                CompressionImpl::Zstd => ".tar.zstd",
            },
        }
    }
}

impl Serialize for ZipStyle {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.ext())
    }
}

impl<'de> Deserialize<'de> for ZipStyle {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let ext = String::deserialize(deserializer)?;
        match &*ext {
            ".zip" => Ok(ZipStyle::Zip),
            ".tar.gz" => Ok(ZipStyle::Tar(CompressionImpl::Gzip)),
            ".tar.xz" => Ok(ZipStyle::Tar(CompressionImpl::Xzip)),
            ".tar.zstd" => Ok(ZipStyle::Tar(CompressionImpl::Zstd)),
            _ => Err(D::Error::custom(format!(
                "unknown archive format {ext}, expected one of: .zip, .tar.gz, .tar.xz, .tar.zstd"
            ))),
        }
    }
}

/// key for the install-path config that selects [`InstallPathStrategyCargoHome`][]
const CARGO_HOME_INSTALL_PATH: &str = "CARGO_HOME";

/// Strategy for install binaries
#[derive(Debug, Clone)]
pub enum InstallPathStrategy {
    /// install to $CARGO_HOME, falling back to ~/.cargo/
    CargoHome,
    /// install to this subdir of the user's home
    ///
    /// syntax: `~/subdir`
    HomeSubdir {
        /// The subdir of home to install to
        subdir: String,
    },
    /// install to this subdir of this env var
    ///
    /// syntax: `$ENV_VAR/subdir`
    EnvSubdir {
        /// The env var to get the base of the path from
        env_key: String,
        /// The subdir to install to
        subdir: String,
    },
}

impl std::str::FromStr for InstallPathStrategy {
    type Err = DistError;
    fn from_str(path: &str) -> DistResult<Self> {
        if path == CARGO_HOME_INSTALL_PATH {
            Ok(InstallPathStrategy::CargoHome)
        } else if let Some(subdir) = path.strip_prefix("~/") {
            if subdir.is_empty() {
                Err(DistError::InstallPathHomeSubdir {
                    path: path.to_owned(),
                })
            } else {
                Ok(InstallPathStrategy::HomeSubdir {
                    // If there's a trailing slash, strip it to be uniform
                    subdir: subdir.strip_suffix('/').unwrap_or(subdir).to_owned(),
                })
            }
        } else if let Some(val) = path.strip_prefix('$') {
            if let Some((env_key, subdir)) = val.split_once('/') {
                Ok(InstallPathStrategy::EnvSubdir {
                    env_key: env_key.to_owned(),
                    // If there's a trailing slash, strip it to be uniform
                    subdir: subdir.strip_suffix('/').unwrap_or(subdir).to_owned(),
                })
            } else {
                Err(DistError::InstallPathEnvSlash {
                    path: path.to_owned(),
                })
            }
        } else {
            Err(DistError::InstallPathInvalid {
                path: path.to_owned(),
            })
        }
    }
}

impl std::fmt::Display for InstallPathStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallPathStrategy::CargoHome => write!(f, "{}", CARGO_HOME_INSTALL_PATH),
            InstallPathStrategy::HomeSubdir { subdir } => write!(f, "~/{subdir}"),
            InstallPathStrategy::EnvSubdir { env_key, subdir } => write!(f, "${env_key}/{subdir}"),
        }
    }
}

impl serde::Serialize for InstallPathStrategy {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for InstallPathStrategy {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let path = String::deserialize(deserializer)?;
        path.parse().map_err(|e| D::Error::custom(format!("{e}")))
    }
}

/// Strategy for install binaries (replica to have different Serialize for jinja)
///
/// The serialize/deserialize impls are already required for loading/saving the config
/// from toml/json, and that serialize impl just creates a plain string again. To allow
/// jinja templates to have richer context we have use duplicate type with a more
/// conventional derived serialize.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum JinjaInstallPathStrategy {
    /// install to $CARGO_HOME, falling back to ~/.cargo/
    CargoHome,
    /// install to this subdir of the user's home
    ///
    /// syntax: `~/subdir`
    HomeSubdir {
        /// The subdir of home to install to
        subdir: String,
    },
    /// install to this subdir of this env var
    ///
    /// syntax: `$ENV_VAR/subdir`
    EnvSubdir {
        /// The env var to get the base of the path from
        env_key: String,
        /// The subdir to install to
        subdir: String,
    },
}

impl InstallPathStrategy {
    /// Convert this into a jinja-friendly form
    pub fn into_jinja(self) -> JinjaInstallPathStrategy {
        match self {
            InstallPathStrategy::CargoHome => JinjaInstallPathStrategy::CargoHome,
            InstallPathStrategy::HomeSubdir { subdir } => {
                JinjaInstallPathStrategy::HomeSubdir { subdir }
            }
            InstallPathStrategy::EnvSubdir { env_key, subdir } => {
                JinjaInstallPathStrategy::EnvSubdir { env_key, subdir }
            }
        }
    }
}

/// A checksumming algorithm
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ChecksumStyle {
    /// sha256sum (using the sha2 crate)
    #[serde(rename = "sha256")]
    Sha256,
    /// sha512sum (using the sha2 crate)
    #[serde(rename = "sha512")]
    Sha512,
    /// Do not checksum
    #[serde(rename = "false")]
    False,
}

impl ChecksumStyle {
    /// Get the extension of a checksum
    pub fn ext(self) -> &'static str {
        match self {
            ChecksumStyle::Sha256 => "sha256",
            ChecksumStyle::Sha512 => "sha512",
            ChecksumStyle::False => "false",
        }
    }
}

pub(crate) fn parse_metadata_table(
    manifest_path: &Utf8Path,
    metadata_table: Option<&serde_json::Value>,
) -> DistResult<DistMetadata> {
    Ok(metadata_table
        .and_then(|t| t.get(METADATA_DIST))
        .map(DistMetadata::deserialize)
        .transpose()
        .map_err(|cause| DistError::CargoTomlParse {
            manifest_path: manifest_path.to_owned(),
            cause,
        })?
        .unwrap_or_default())
}

/// Get the general info about the project (via axo-project)
pub fn get_project() -> Result<axoproject::WorkspaceInfo> {
    let start_dir = std::env::current_dir().expect("couldn't get current working dir!?");
    let start_dir = Utf8PathBuf::from_path_buf(start_dir).expect("project path isn't utf8!?");
    match axoproject::rust::get_workspace(None, &start_dir) {
        WorkspaceSearch::Found(mut workspace) => {
            // This is a goofy as heck workaround for two facts:
            //   * the convenient Report approach requires us to provide an Error by-val
            //   * many error types (like std::io::Error) don't impl Clone, so we can't
            //     clone axoproject Errors.
            //
            // So we temporarily take ownership of the warnings and then pull them back
            // out of the Report with runtime reflection to put them back in :)
            let mut warnings = std::mem::take(&mut workspace.warnings);
            for warning in warnings.drain(..) {
                let report = Report::new(warning);
                warn!("{:?}", report);
                workspace.warnings.push(report.downcast().unwrap());
            }
            Ok(workspace)
        }
        WorkspaceSearch::Missing(e) => Err(Report::new(e).wrap_err("no cargo workspace found")),
        WorkspaceSearch::Broken(e) => {
            Err(Report::new(e).wrap_err("your cargo workspace has an issue"))
        }
    }
}

/// Load a Cargo.toml into toml-edit form
pub fn load_cargo_toml(manifest_path: &Utf8Path) -> Result<toml_edit::Document> {
    let src = axoasset::SourceFile::load_local(manifest_path)?;
    let toml = src.deserialize_toml_edit()?;
    Ok(toml)
}

/// Save a Cargo.toml from toml-edit form
pub fn save_cargo_toml(manifest_path: &Utf8Path, toml: toml_edit::Document) -> Result<()> {
    let toml_text = toml.to_string();
    axoasset::LocalAsset::write_new(&toml_text, manifest_path)?;
    Ok(())
}
