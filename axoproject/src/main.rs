use std::borrow::Cow;

use axoasset::{serde_json, LocalAsset};
use axoproject::{WorkspaceInfo, WorkspaceKind, WorkspaceSearch, Workspaces};
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use miette::Report;

use cli::{Cli, OutputFormat};
use serde::{Deserialize, Serialize};

mod cli;

fn main() {
    let config = Cli::parse();
    axocli::CliAppBuilder::new("axoproject")
        .verbose(config.verbose)
        .json_errors(config.output_format == OutputFormat::Json)
        .start(config, real_main);
}

fn real_main(app: &axocli::CliApp<Cli>) -> Result<(), Report> {
    // FIXME: use the root_dir
    let root_dir = app.config.root.clone();
    let start_dir = app
        .config
        .search_path
        .clone()
        .unwrap_or_else(|| LocalAsset::current_dir().unwrap());

    let workspaces = axoproject::get_workspaces(&start_dir, root_dir.as_deref());

    match app.config.output_format {
        OutputFormat::Human => print_searches(workspaces),
        OutputFormat::Json => print_searches_json(root_dir.as_deref(), workspaces),
    }

    Ok(())
}

fn print_searches(workspaces: Workspaces) {
    #[cfg(feature = "generic-projects")]
    print_search(workspaces.generic, "generic");
    #[cfg(feature = "cargo-projects")]
    print_search(workspaces.rust, "rust");
    #[cfg(feature = "npm-projects")]
    print_search(workspaces.javascript, "javascript");
}

fn print_search(search: WorkspaceSearch, name: &str) {
    eprintln!("searching for {name} project...");
    match search {
        WorkspaceSearch::Found(w) => print_workspace(&w),
        WorkspaceSearch::Missing(e) => {
            let err = Report::new(e).wrap_err(format!("Couldn't find a {name} project"));
            eprintln!("{:?}", err);
        }
        WorkspaceSearch::Broken {
            manifest_path,
            cause,
        } => {
            let err = Report::new(cause).wrap_err(format!(
                "We found a potential {name} project at {manifest_path}, but there was an issue"
            ));
            eprintln!("{:?}", err);
        }
    }
}

fn print_workspace(project: &WorkspaceInfo) {
    let disabled_sty = console::Style::new().dim();
    let enabled_sty = console::Style::new();
    eprintln!("manifest: {}", project.manifest_path);
    eprintln!("target: {}", project.target_dir);

    for (_, pkg) in project.packages() {
        let pkg_name = &pkg.name;
        let pkg_version = &pkg.version;

        // Determine if this package's binaries should be Released
        let mut disabled_reason = None;
        if pkg.binaries.is_empty() {
            // Nothing to publish if there's no binaries!
            disabled_reason = Some("no binaries".to_owned());
        /*
        } else if let Some(do_dist) = pkg.config.dist {
            // If [metadata.dist].dist is explicitly set, respect it!
            if !do_dist {
                disabled_reason = Some("dist = false".to_owned());
            }
         */
        } else if !pkg.publish {
            // Otherwise defer to Cargo's `publish = false`
            disabled_reason = Some("publish = false".to_owned());
            /*
            } else if let Some(id) = &announcing_package {
                // If we're announcing a package, reject every other package
                if pkg_id != id {
                    disabled_reason = Some(format!(
                        "didn't match tag {}",
                        announcement_tag.as_ref().unwrap()
                    ));
                }
            } else if let Some(ver) = &announcing_version {
                if pkg_version != ver {
                    disabled_reason = Some(format!(
                        "didn't match tag {}",
                        announcement_tag.as_ref().unwrap()
                    ));
                }
             */
        }

        // Report our conclusion/discoveries
        let sty;
        if let Some(reason) = &disabled_reason {
            sty = &disabled_sty;
            if let Some(version) = pkg_version {
                eprintln!(
                    "  {}",
                    sty.apply_to(format!("{pkg_name}@{version} ({reason})"))
                );
            } else {
                eprintln!("  {}", sty.apply_to(format!("{pkg_name} ({reason})")));
            }
        } else {
            sty = &enabled_sty;
            if let Some(version) = pkg_version {
                eprintln!("  {}@{version}", sty.apply_to(pkg_name));
            } else {
                eprintln!("  {}", sty.apply_to(pkg_name));
            }
        }
        eprintln!("    manifest: {}", pkg.manifest_path);

        // Report each binary and potentially add it to the Release for this package
        let mut rust_binaries = vec![];
        for binary in &pkg.binaries {
            eprintln!("    {}", sty.apply_to(format!("[bin] {}", binary)));
            // In the future might want to allow this to be granular for each binary
            if disabled_reason.is_none() {
                rust_binaries.push(binary);
            }
        }
        for lib in &pkg.cdylibs {
            eprintln!("    {}", sty.apply_to(format!("[cdylib] {}", lib)));
        }
        for lib in &pkg.cstaticlibs {
            eprintln!("    {}", sty.apply_to(format!("[cstaticlib] {}", lib)));
        }

        // If any binaries were accepted for this package, it's a Release!
        if !rust_binaries.is_empty() {
            // rust_releases.push((*pkg_id, rust_binaries));
        }
    }
    eprintln!();
}

fn print_searches_json(root: Option<&Utf8Path>, workspaces: Workspaces) {
    let output = JsonOutput {
        root: root.map(|p| p.to_owned()),
        #[cfg(feature = "generic-projects")]
        generic: JsonWorkspaceSearch::from_real(root, workspaces.generic),
        #[cfg(feature = "cargo-projects")]
        rust: JsonWorkspaceSearch::from_real(root, workspaces.rust),
        #[cfg(feature = "npm-projects")]
        javascript: JsonWorkspaceSearch::from_real(root, workspaces.javascript),
    };

    serde_json::to_writer_pretty(std::io::stdout(), &output).unwrap();
}

#[derive(Serialize, Deserialize)]
struct JsonOutput {
    root: Option<Utf8PathBuf>,
    #[cfg(feature = "generic-projects")]
    generic: JsonWorkspaceSearch,
    #[cfg(feature = "cargo-projects")]
    rust: JsonWorkspaceSearch,
    #[cfg(feature = "npm-projects")]
    javascript: JsonWorkspaceSearch,
}

#[derive(Serialize, Deserialize)]
enum JsonWorkspaceSearch {
    #[serde(rename = "found")]
    Found(JsonWorkspaceInfo),
    #[serde(rename = "missing")]
    Missing { cause: serde_json::Value },
    #[serde(rename = "broken")]
    Broken {
        manifest_path: JsonRelPath,
        cause: serde_json::Value,
    },
}

impl JsonWorkspaceSearch {
    fn from_real(root: Option<&Utf8Path>, val: WorkspaceSearch) -> Self {
        match val {
            WorkspaceSearch::Found(info) => {
                JsonWorkspaceSearch::Found(JsonWorkspaceInfo::from_real(root, info))
            }
            WorkspaceSearch::Broken {
                manifest_path,
                cause,
            } => JsonWorkspaceSearch::Broken {
                manifest_path: JsonRelPath::from_real(root, manifest_path),
                cause: json_diagnostic_from_real(root, cause),
            },
            WorkspaceSearch::Missing(e) => JsonWorkspaceSearch::Missing {
                cause: json_diagnostic_from_real(root, e),
            },
        }
    }
}

fn json_diagnostic_from_real(
    _root: Option<&Utf8Path>,
    e: axoproject::errors::AxoprojectError,
) -> serde_json::Value {
    axocli::json_diagnostic(&Report::new(e))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonWorkspaceInfo {
    pub kind: JsonWorkspaceKind,
    pub target_dir: JsonRelPath,
    pub workspace_dir: JsonRelPath,
    pub package_info: Vec<JsonPackageInfo>,
    pub manifest_path: JsonRelPath,
    pub repository_url: Option<String>,
    pub root_auto_includes: JsonAutoIncludes,
    pub warnings: Vec<serde_json::Value>,
    #[cfg(feature = "cargo-projects")]
    pub cargo_metadata_table: Option<serde_json::Value>,
    #[cfg(feature = "cargo-projects")]
    pub cargo_profiles: std::collections::BTreeMap<String, JsonCargoProfile>,
}

impl JsonWorkspaceInfo {
    fn from_real(root: Option<&Utf8Path>, ws: WorkspaceInfo) -> Self {
        Self {
            workspace_dir: JsonRelPath::from_real(root, ws.workspace_dir),
            kind: JsonWorkspaceKind::from_real(ws.kind),
            target_dir: JsonRelPath::from_real(root, ws.target_dir),
            package_info: ws
                .package_info
                .into_iter()
                .map(|p| JsonPackageInfo::from_real(root, p))
                .collect(),
            manifest_path: JsonRelPath::from_real(root, ws.manifest_path),
            repository_url: ws.repository_url,
            root_auto_includes: JsonAutoIncludes::from_real(root, ws.root_auto_includes),
            warnings: ws
                .warnings
                .into_iter()
                .map(|w| json_diagnostic_from_real(root, w))
                .collect(),
            cargo_metadata_table: ws.cargo_metadata_table,
            cargo_profiles: ws
                .cargo_profiles
                .into_iter()
                .map(|(k, v)| (k, JsonCargoProfile::from_real(root, v)))
                .collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonPackageInfo {
    pub manifest_path: JsonRelPath,
    pub package_root: JsonRelPath,
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub license: Option<String>,
    pub publish: bool,
    pub repository_url: Option<String>,
    pub homepage_url: Option<String>,
    pub documentation_url: Option<String>,
    pub readme_file: Option<JsonRelPath>,
    pub license_files: Vec<JsonRelPath>,
    pub changelog_file: Option<JsonRelPath>,
    pub binaries: Vec<String>,
    pub cstaticlibs: Vec<String>,
    pub cdylibs: Vec<String>,
    #[cfg(feature = "cargo-projects")]
    pub cargo_metadata_table: Option<serde_json::Value>,
    #[cfg(feature = "cargo-projects")]
    pub cargo_package_id: Option<String>,
}
impl JsonPackageInfo {
    fn from_real(root: Option<&Utf8Path>, p: axoproject::PackageInfo) -> Self {
        Self {
            manifest_path: JsonRelPath::from_real(root, p.manifest_path),
            package_root: JsonRelPath::from_real(root, p.package_root),
            name: p.name,
            version: p.version.map(|v| v.to_string()),
            description: p.description,
            authors: p.authors,
            license: p.license,
            publish: p.publish,
            repository_url: p.repository_url,
            homepage_url: p.homepage_url,
            documentation_url: p.documentation_url,
            readme_file: p.readme_file.map(|f| JsonRelPath::from_real(root, f)),
            license_files: p
                .license_files
                .into_iter()
                .map(|f| JsonRelPath::from_real(root, f))
                .collect(),
            changelog_file: p.changelog_file.map(|f| JsonRelPath::from_real(root, f)),
            binaries: p.binaries,
            cstaticlibs: p.cstaticlibs,
            cdylibs: p.cdylibs,
            cargo_metadata_table: p.cargo_metadata_table,
            cargo_package_id: p.cargo_package_id.map(|v| v.to_string()),
        }
    }
}

/// Parts of a [profile.*] entry in a Cargo.toml we care about
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonCargoProfile {
    pub inherits: Option<String>,
    pub debug: Option<i64>,
    pub split_debuginfo: Option<String>,
}
impl JsonCargoProfile {
    fn from_real(_root: Option<&Utf8Path>, v: axoproject::rust::CargoProfile) -> Self {
        Self {
            inherits: v.inherits,
            debug: v.debug,
            split_debuginfo: v.split_debuginfo,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JsonRelPath(String);

impl JsonRelPath {
    fn from_real(root: Option<&Utf8Path>, real: Utf8PathBuf) -> Self {
        let Some(root) = root else {
            return Self(real.to_string());
        };

        let path_diff = pathdiff::diff_utf8_paths(real, root).expect("workspace root is absolute");
        // On Windows, the directory name and the workspace root might be on different drives,
        // in which case the path can't be relative.
        let path_diff = if path_diff.is_absolute() {
            path_diff
        } else {
            convert_forward_slashes(path_diff)
        };
        Self(path_diff.to_string())
    }
}

/// Kind of workspace
#[derive(Debug, Serialize, Deserialize)]
pub enum JsonWorkspaceKind {
    /// generic workspace
    #[cfg(feature = "generic-projects")]
    #[serde(rename = "generic")]
    Generic,
    /// cargo/rust workspace
    #[cfg(feature = "cargo-projects")]
    #[serde(rename = "rust")]
    Rust,
    /// npm/js workspace
    #[cfg(feature = "npm-projects")]
    #[serde(rename = "javascript")]
    Javascript,
}

impl JsonWorkspaceKind {
    fn from_real(real: WorkspaceKind) -> Self {
        match real {
            #[cfg(feature = "generic-projects")]
            WorkspaceKind::Generic => Self::Generic,
            #[cfg(feature = "cargo-projects")]
            WorkspaceKind::Rust => Self::Rust,
            #[cfg(feature = "npm-projects")]
            WorkspaceKind::Javascript => Self::Javascript,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonAutoIncludes {
    /// README
    pub readme: Option<JsonRelPath>,
    /// LICENSE/UNLICENSE
    pub licenses: Vec<JsonRelPath>,
    /// CHANGELOG/RELEASES
    pub changelog: Option<JsonRelPath>,
}
impl JsonAutoIncludes {
    fn from_real(root: Option<&Utf8Path>, v: axoproject::AutoIncludes) -> Self {
        Self {
            readme: v.readme.map(|p| JsonRelPath::from_real(root, p)),
            licenses: v
                .licenses
                .into_iter()
                .map(|p| JsonRelPath::from_real(root, p))
                .collect(),
            changelog: v.changelog.map(|p| JsonRelPath::from_real(root, p)),
        }
    }
}

#[track_caller]
fn convert_forward_slashes<'a>(rel_path: impl Into<Cow<'a, Utf8Path>>) -> Utf8PathBuf {
    let rel_path = rel_path.into();
    debug_assert!(
        rel_path.is_relative(),
        "path {} should be relative",
        rel_path,
    );

    if cfg!(windows) {
        rel_path.as_str().replace('\\', "/").into()
    } else {
        rel_path.into_owned()
    }
}
