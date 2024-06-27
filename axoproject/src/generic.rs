//! Support for generic projects with cargo-dist build instructions

use axoasset::{AxoassetError, SourceFile};
use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

use crate::{
    errors::GenericManifestParseError, PackageInfo, Result, Version, WorkspaceInfo, WorkspaceSearch,
};

const DIST_PACKAGE_TOML: &str = "dist.toml";
const DIST_WORKSPACE_TOML: &str = "dist-workspace.toml";
const DIST_TARGET_DIR: &str = "target";

const MEMBER_GENERIC: &str = "dist";
#[cfg(feature = "cargo-projects")]
const MEMBER_CARGO: &str = "cargo";
#[cfg(feature = "npm-projects")]
const MEMBER_NPM: &str = "npm";

#[derive(Deserialize, Debug)]
struct WorkspaceManifest {
    workspace: Workspace,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct Workspace {
    members: Vec<WorkspaceMember>,
}

#[derive(Debug)]
enum WorkspaceMember {
    Generic(Utf8PathBuf),
    #[cfg(feature = "cargo-projects")]
    Cargo(Utf8PathBuf),
    #[cfg(feature = "npm-projects")]
    Npm(Utf8PathBuf),
}

impl std::str::FromStr for WorkspaceMember {
    type Err = GenericManifestParseError;
    fn from_str(member: &str) -> std::result::Result<Self, GenericManifestParseError> {
        let Some((kind, path)) = member.split_once(':') else {
            return Err(GenericManifestParseError::NoPrefix {
                val: member.to_owned(),
            });
        };
        let output = match kind {
            MEMBER_GENERIC => WorkspaceMember::Generic(path.into()),
            #[cfg(feature = "cargo-projects")]
            MEMBER_CARGO => WorkspaceMember::Cargo(path.into()),
            #[cfg(feature = "npm-projects")]
            MEMBER_NPM => WorkspaceMember::Npm(path.into()),
            other => {
                return Err(GenericManifestParseError::UnknownPrefix {
                    prefix: other.to_owned(),
                    val: member.to_owned(),
                });
            }
        };
        Ok(output)
    }
}

impl std::fmt::Display for WorkspaceMember {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkspaceMember::Generic(path) => write!(f, "{MEMBER_GENERIC}:{path}"),
            #[cfg(feature = "cargo-projects")]
            WorkspaceMember::Cargo(path) => write!(f, "{MEMBER_CARGO}/{path}"),
            #[cfg(feature = "npm-projects")]
            WorkspaceMember::Npm(path) => write!(f, "${MEMBER_NPM}/{path}"),
        }
    }
}

impl serde::Serialize for WorkspaceMember {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for WorkspaceMember {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let path = String::deserialize(deserializer)?;
        path.parse().map_err(|e| D::Error::custom(format!("{e}")))
    }
}

#[derive(Deserialize, Debug)]
struct PackageManifest {
    package: Package,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct Package {
    name: Option<String>,
    repository: Option<String>,
    homepage: Option<String>,
    documentation: Option<String>,
    description: Option<String>,
    readme: Option<Utf8PathBuf>,
    authors: Option<Vec<String>>,
    binaries: Option<Vec<String>>,
    license: Option<String>,
    changelog: Option<Utf8PathBuf>,
    license_files: Option<Vec<Utf8PathBuf>>,
    cstaticlibs: Option<Vec<String>>,
    cdylibs: Option<Vec<String>>,
    build_command: Option<Vec<String>>,
    version: Option<semver::Version>,
}

/// Try to find a generic workspace at the given path
///
/// See [`crate::get_workspaces`][] for the semantics.
pub fn get_workspace(start_dir: &Utf8Path, clamp_to_dir: Option<&Utf8Path>) -> WorkspaceSearch {
    // First search for a workspace file
    match crate::find_file(DIST_WORKSPACE_TOML, start_dir, clamp_to_dir) {
        // Found a workspace file, read it
        Ok(manifest_path) => match workspace_from(&manifest_path) {
            Ok(info) => WorkspaceSearch::Found(info),
            Err(e) => WorkspaceSearch::Broken {
                manifest_path,
                cause: e,
            },
        },
        // No workspace file, maybe there's just a package?
        Err(_) => match crate::find_file(DIST_PACKAGE_TOML, start_dir, clamp_to_dir) {
            // Ok, make a faux-workspace from that
            Ok(manifest_path) => match single_package_workspace_from(&manifest_path) {
                Ok(info) => WorkspaceSearch::Found(info),
                Err(e) => WorkspaceSearch::Broken {
                    manifest_path,
                    cause: e,
                },
            },
            Err(e) => WorkspaceSearch::Missing(e),
        },
    }
}

// Load and process a dist-workspace.toml, and its child packages
fn workspace_from(manifest_path: &Utf8Path) -> Result<WorkspaceInfo> {
    let manifest = load_workspace_dist_toml(manifest_path)?;
    let workspace_dir = manifest_path.parent().unwrap().to_path_buf();
    let root_auto_includes = crate::find_auto_includes(&workspace_dir)?;

    let mut package_info = vec![];
    let mut sub_workspaces = vec![];
    for member in &manifest.workspace.members {
        match member {
            WorkspaceMember::Generic(member_reldir) => {
                let member_dir = workspace_dir.join(member_reldir);
                let member_manifest_path = member_dir.join(DIST_PACKAGE_TOML);
                let mut package = package_from(&member_manifest_path)?;
                crate::merge_auto_includes(&mut package, &root_auto_includes);
                package_info.push(package);
            }
            #[cfg(feature = "cargo-projects")]
            WorkspaceMember::Cargo(member_reldir) => {
                let cargo_workspace_dir = workspace_dir.join(member_reldir);
                let search =
                    crate::rust::get_workspace(&cargo_workspace_dir, Some(&cargo_workspace_dir))
                        .into_result()?;
                sub_workspaces.push(search);
            }
            #[cfg(feature = "npm-projects")]
            WorkspaceMember::Npm(member_reldir) => {
                // First load the npm package(s)
                let npm_workspace_dir = workspace_dir.join(member_reldir);
                let search =
                    crate::javascript::get_workspace(&npm_workspace_dir, Some(&npm_workspace_dir))
                        .into_result()?;

                // Process packages
                for mut package in search._package_info {
                    // If there's a dist.toml in the same dir, load it with less validation
                    // and merge the results into the npm package
                    let paired_manifest = package.package_root.join(DIST_PACKAGE_TOML);
                    if paired_manifest.exists() {
                        let generic = raw_package_from(&paired_manifest)?;
                        merge_package_with_raw_generic(&mut package, generic);
                    }
                    // File off the serial numbers on the npm-ness (pretend it's a generic package)
                    crate::merge_auto_includes(&mut package, &root_auto_includes);
                    package_info.push(package);
                }
            }
        }
    }

    Ok(WorkspaceInfo {
        kind: crate::WorkspaceKind::Generic,
        target_dir: workspace_dir.join(DIST_TARGET_DIR),
        workspace_dir,
        _sub_workspaces: sub_workspaces,
        _package_info: package_info,
        manifest_path: manifest_path.to_owned(),
        root_auto_includes,
        warnings: vec![],
        #[cfg(feature = "cargo-projects")]
        cargo_metadata_table: None,
        #[cfg(feature = "cargo-projects")]
        cargo_profiles: crate::rust::CargoProfiles::new(),
    })
}

// Load and process a dist.toml, and treat it as an entire workspace
fn single_package_workspace_from(manifest_path: &Utf8Path) -> Result<WorkspaceInfo> {
    let package = package_from(manifest_path)?;
    let root_auto_includes = crate::find_auto_includes(&package.package_root)?;
    Ok(WorkspaceInfo {
        kind: crate::WorkspaceKind::Generic,
        target_dir: package.package_root.join(DIST_TARGET_DIR),
        workspace_dir: package.package_root.clone(),
        manifest_path: package.manifest_path.clone(),
        root_auto_includes,
        _sub_workspaces: vec![],
        _package_info: vec![package],
        warnings: vec![],
        #[cfg(feature = "cargo-projects")]
        cargo_metadata_table: None,
        #[cfg(feature = "cargo-projects")]
        cargo_profiles: Default::default(),
    })
}

fn raw_package_from(manifest_path: &Utf8Path) -> Result<Package> {
    let manifest = load_package_dist_toml(manifest_path)?;
    Ok(manifest.package)
}

// Load and process a dist.toml
fn package_from(manifest_path: &Utf8Path) -> Result<PackageInfo> {
    use serde::de::Error;
    let package = raw_package_from(manifest_path)?;
    let version = package.version.map(Version::Generic);

    let manifest_path = manifest_path.to_path_buf();

    // Create dummy src and span for missing field errors
    let source = SourceFile::new(manifest_path.as_str(), String::new());
    let span = source.span_for_line_col(1, 1);
    let Some(build_command) = package.build_command else {
        return Err(AxoassetError::Toml {
            source,
            span,
            details: axoasset::toml::de::Error::custom("missing field build-command"),
        })?;
    };
    let Some(name) = package.name else {
        return Err(AxoassetError::Toml {
            source,
            span,
            details: axoasset::toml::de::Error::custom("missing field name"),
        })?;
    };

    let mut info = PackageInfo {
        manifest_path: manifest_path.clone(),
        package_root: manifest_path.parent().unwrap().to_owned(),
        name,
        version,
        description: package.description,
        authors: package.authors.unwrap_or_default(),
        license: package.license,
        publish: true,
        keywords: None,
        repository_url: package.repository.clone(),
        homepage_url: package.homepage,
        documentation_url: package.documentation,
        readme_file: package.readme,
        license_files: package.license_files.unwrap_or_default(),
        changelog_file: package.changelog,
        binaries: package.binaries.unwrap_or_default(),
        cstaticlibs: package.cstaticlibs.unwrap_or_default(),
        cdylibs: package.cdylibs.unwrap_or_default(),
        build_command: Some(build_command),
        #[cfg(feature = "cargo-projects")]
        cargo_metadata_table: None,
        #[cfg(feature = "cargo-projects")]
        cargo_package_id: None,
    };

    // Load and apply auto-includes
    let auto_includes = crate::find_auto_includes(&info.package_root)?;
    crate::merge_auto_includes(&mut info, &auto_includes);

    Ok(info)
}

/// Load the root workspace toml
fn load_workspace_dist_toml(manifest_path: &Utf8Path) -> Result<WorkspaceManifest> {
    let manifest_src = SourceFile::load_local(manifest_path)?;
    let manifest = manifest_src.deserialize_toml()?;
    Ok(manifest)
}

/// Load the a package toml
fn load_package_dist_toml(manifest_path: &Utf8Path) -> Result<PackageManifest> {
    let manifest_src = SourceFile::load_local(manifest_path)?;
    let manifest = manifest_src.deserialize_toml()?;
    Ok(manifest)
}

fn merge_package_with_raw_generic(package: &mut PackageInfo, generic: Package) {
    let Package {
        name,
        repository,
        homepage,
        documentation,
        description,
        readme,
        authors,
        binaries,
        license,
        changelog,
        license_files,
        cstaticlibs,
        cdylibs,
        build_command,
        version,
    } = generic;
    if let Some(val) = name {
        package.name = val;
    }
    if let Some(val) = repository {
        package.repository_url = Some(val);
    }
    if let Some(val) = homepage {
        package.homepage_url = Some(val);
    }
    if let Some(val) = documentation {
        package.documentation_url = Some(val);
    }
    if let Some(val) = description {
        package.description = Some(val);
    }
    if let Some(val) = readme {
        package.readme_file = Some(val);
    }
    if let Some(val) = changelog {
        package.changelog_file = Some(val);
    }
    if let Some(val) = authors {
        package.authors = val;
    }
    if let Some(val) = binaries {
        package.binaries = val;
    }
    if let Some(val) = license {
        package.license = Some(val);
    }
    if let Some(val) = license_files {
        package.license_files = val;
    }
    if let Some(val) = cstaticlibs {
        package.cstaticlibs = val;
    }
    if let Some(val) = cdylibs {
        package.cdylibs = val;
    }
    if let Some(val) = build_command {
        package.build_command = Some(val);
    }
    if let Some(val) = version {
        package.version = Some(Version::Generic(val));
    }
}
