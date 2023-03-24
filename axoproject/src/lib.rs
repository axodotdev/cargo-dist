use camino::{Utf8Path, Utf8PathBuf};
use guppy::PackageId;
use miette::{Context, IntoDiagnostic};
use tracing::info;

pub type SortedMap<K, V> = std::collections::BTreeMap<K, V>;
pub type Result<T> = std::result::Result<T, miette::Report>;

#[cfg(feature = "npm-projects")]
pub mod javascript;
#[cfg(feature = "cargo-projects")]
pub mod rust;

/// Kind of workspace
#[derive(Debug, Clone, Copy)]
pub enum WorkspaceKind {
    /// cargo/rust workspace
    Rust,
    /// npm/js workspace
    Javascript,
}

/// Info on the current workspace
pub struct WorkspaceInfo {
    pub kind: WorkspaceKind,
    /// The directory where build output will go (generally `target/`)
    pub target_dir: Utf8PathBuf,
    /// The root directory of the workspace (where the root Cargo.toml is)
    pub workspace_dir: Utf8PathBuf,
    /// Computed info about the packages beyond what Guppy tells us
    ///
    /// This notably includes finding readmes and licenses even if the user didn't
    /// specify their location -- something Cargo does but Guppy (and cargo-metadata) don't.
    pub package_info: SortedMap<PackageId, PackageInfo>,
    /// Path to the Cargo.toml of the workspace (may be a package's Cargo.toml)
    pub manifest_path: Utf8PathBuf,
    /// A consensus URL for the repo according the workspace
    pub repository_url: Option<String>,
    /// If the workspace root has some auto-includeable files, here they are!
    ///
    /// This is currently what is use for top-level Announcement contents.
    pub root_auto_includes: AutoIncludes,
    /*
       /// The desired cargo-dist version for handling this project
       pub desired_cargo_dist_version: Option<Version>,
       /// The desired rust toolchain for handling this project
       pub desired_rust_toolchain: Option<String>,
       /// The desired ci backends for this project
       pub ci_kinds: Vec<CiStyle>,
       /// Contents of [profile.dist] in the root Cargo.toml
       ///
       /// This is used to determine if we expect split-debuginfo from builds.
       pub dist_profile: Option<CargoProfile>,
    */
}

/// Computed info about the packages beyond what Guppy tells us
///
/// This notably includes finding readmes and licenses even if the user didn't
/// specify their location -- something Cargo does but Guppy (and cargo-metadata) don't.
#[derive(Debug)]
pub struct PackageInfo {
    /// Name of the package
    pub name: String,
    /// Version of the package
    ///
    /// If Cargo, this is a SemVer version
    pub version: Option<String>,
    /// A brief description of the package
    pub description: Option<String>,
    /// Authors of the package (may be empty)
    pub authors: Vec<String>,
    /// The license the package is provided under
    pub license: Option<String>,
    /// False if they set publish=false, true otherwise
    pub publish: bool,
    /// URL to the repository for this package
    ///
    /// This URL can be used by various CI/Installer helpers. In the future we
    /// might also use it for auto-detecting "hey you're using github, here's the
    /// recommended github setup".
    ///
    /// i.e. `--installer=shell` uses this as the base URL for fetching from
    /// a Github Release™️.
    pub repository_url: Option<String>,
    /// URL to the homepage for this package.
    ///
    /// Currently this isn't terribly important or useful?
    pub homepage_url: Option<String>,
    /// URL to the documentation for this package.
    ///
    /// This will default to docs.rs if not specified, which is the default crates.io behaviour.
    ///
    /// Currently this isn't terribly important or useful?
    pub documentation_url: Option<String>,
    /// Path to the README file for this package.
    ///
    /// By default this should be copied into a zip containing this package's binary.
    pub readme_file: Option<Utf8PathBuf>,
    /// Paths to the LICENSE files for this package.
    ///
    /// By default these should be copied into a zip containing this package's binary.
    ///
    /// Cargo only lets you specify one such path, but that's because the license path
    /// primarily exists as an escape hatch for someone's whacky-wild custom license.
    /// But for our usecase we want to find those paths even if they're bog standard
    /// MIT/Apache, which traditionally involves two separate license files.
    pub license_files: Vec<Utf8PathBuf>,
    /// Paths to the CHANGELOG or RELEASES file for this package
    ///
    /// By default this should be copied into a zip containing this package's binary.
    ///
    /// We will *try* to parse this
    pub changelog_file: Option<Utf8PathBuf>,
    /// Names of binaries this package defines
    pub binaries: Vec<String>,
    /*
    /// DistMetadata for the package (with workspace merged and paths made absolute)
    pub config: DistMetadata,
    */
}

/// Various files we might want to auto-include
#[derive(Debug, Clone)]
pub struct AutoIncludes {
    /// README
    pub readme: Option<Utf8PathBuf>,
    /// LICENSE/UNLICENSE
    pub licenses: Vec<Utf8PathBuf>,
    /// CHANGELOG/RELEASES
    pub changelog: Option<Utf8PathBuf>,
}

pub fn get_project() -> Option<WorkspaceInfo> {
    #[cfg(feature = "cargo-projects")]
    if let Ok(project) = rust::get_project() {
        return Some(project);
    }
    #[cfg(feature = "npm-projects")]
    if let Ok(project) = javascript::get_project() {
        return Some(project);
    }
    None
}

/// Find auto-includeable files in a dir
pub fn find_auto_includes(dir: &Utf8Path) -> Result<AutoIncludes> {
    let entries = dir
        .read_dir_utf8()
        .into_diagnostic()
        .wrap_err("Failed to read workspace dir")?;

    let mut includes = AutoIncludes {
        readme: None,
        licenses: vec![],
        changelog: None,
    };

    for entry in entries {
        let entry = entry
            .into_diagnostic()
            .wrap_err("Failed to read workspace dir entry")?;
        let meta = entry
            .file_type()
            .into_diagnostic()
            .wrap_err("Failed to read workspace dir entry's metadata")?;
        if !meta.is_file() {
            continue;
        }
        let file_name = entry.file_name();
        if file_name.starts_with("README") {
            if includes.readme.is_none() {
                let path = entry.path().to_owned();
                info!("Found README at {}", path);
                includes.readme = Some(path);
            } else {
                info!("Ignoring duplicate candidate README at {}", entry.path());
            }
        } else if file_name.starts_with("LICENSE") || file_name.starts_with("UNLICENSE") {
            let path = entry.path().to_owned();
            info!("Found LICENSE at {}", path);
            includes.licenses.push(path);
        } else if file_name.starts_with("CHANGELOG") || file_name.starts_with("RELEASES") {
            if includes.changelog.is_none() {
                let path = entry.path().to_owned();
                info!("Found CHANGELOG at {}", path);
                includes.changelog = Some(path);
            } else {
                info!("Ignoring duplicate candidate CHANGELOG at {}", entry.path());
            }
        }
    }

    Ok(includes)
}

pub fn merge_auto_includes(info: &mut PackageInfo, auto_includes: &AutoIncludes) {
    if info.readme_file.is_none() {
        info.readme_file = auto_includes.readme.clone();
    }
    if info.changelog_file.is_none() {
        info.changelog_file = auto_includes.changelog.clone();
    }
    if info.license_files.is_empty() {
        info.license_files = auto_includes.licenses.clone();
    }
}

/*
/// Load a changelog to a string
fn try_load_changelog(changelog_path: &Utf8Path) -> Result<String> {
    let file = File::open(changelog_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to open changelog at {changelog_path}"))?;
    let mut data = BufReader::new(file);
    let mut changelog_str = String::new();
    data.read_to_string(&mut changelog_str)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to read changelog at {changelog_path}"))?;
    Ok(changelog_str)
}

/// Tries to find a changelog entry with the exact version given and returns its title and notes.
fn try_extract_changelog_exact(
    changelogs: &parse_changelog::Changelog,
    version: &Version,
) -> Option<(String, String)> {
    let version_string = format!("{}", version);

    changelogs.get(&*version_string).map(|release_notes| {
        (
            release_notes.title.to_string(),
            release_notes.notes.to_string(),
        )
    })
}

/// Tries to find a changelog entry that matches the given version's normalized form. That is, just
/// the `major.minor.patch` part. If successful, the entry's title is modified to include the
/// version's prerelease part before it is returned together with the notes.
///
/// Noop if the given version is already normalized.
fn try_extract_changelog_normalized(
    changelogs: &parse_changelog::Changelog,
    version: &Version,
) -> Option<(String, String)> {
    if version.pre.is_empty() {
        return None;
    }

    let version_normalized = Version::new(version.major, version.minor, version.patch);
    let version_normalized_string = format!("{}", version_normalized);

    let release_notes = changelogs.get(&*version_normalized_string)?;

    // title looks something like '<prefix><version><freeform>'
    // prefix could be 'v' or 'Version ' for example
    let (prefix_and_version, freeform) = release_notes.title.split_at(
        release_notes
            .title
            .find(&*version_normalized_string)
            .unwrap() // impossible that this version string is not present in the header
            + version_normalized_string.len(),
    );

    // insert prerelease suffix into the title
    let title = format!(
        "{}-{} {}",
        prefix_and_version.trim(),
        version.pre,
        freeform.trim()
    );

    Some((title.trim().to_string(), release_notes.notes.to_string()))
}
 */
