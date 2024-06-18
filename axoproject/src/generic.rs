//! Support for generic projects with cargo-dist build instructions

use axoasset::SourceFile;
use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

use crate::{PackageInfo, Result, Version, WorkspaceInfo, WorkspaceSearch};

const DIST_PACKAGE_TOML: &str = "dist.toml";
const DIST_WORKSPACE_TOML: &str = "dist-workspace.toml";
const DIST_TARGET_DIR: &str = "target";

#[derive(Deserialize, Debug)]
struct WorkspaceManifest {
    workspace: Workspace,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct Workspace {
    members: Vec<Utf8PathBuf>,
    #[cfg(feature = "cargo-projects")]
    #[serde(default)]
    cargo_workspaces: Vec<Utf8PathBuf>,
}

#[derive(Deserialize, Debug)]
struct PackageManifest {
    package: Package,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
struct Package {
    name: String,
    repository: Option<String>,
    homepage: Option<String>,
    documentation: Option<String>,
    description: Option<String>,
    readme: Option<Utf8PathBuf>,
    #[serde(default = "Vec::new")]
    authors: Vec<String>,
    binaries: Vec<String>,
    license: Option<String>,
    changelog: Option<Utf8PathBuf>,
    #[serde(default = "Vec::new")]
    license_files: Vec<Utf8PathBuf>,
    #[serde(default = "Vec::new")]
    cstaticlibs: Vec<String>,
    #[serde(default = "Vec::new")]
    cdylibs: Vec<String>,
    build_command: Vec<String>,
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
    for member_reldir in &manifest.workspace.members {
        let member_dir = workspace_dir.join(member_reldir);
        let member_manifest_path = member_dir.join(DIST_PACKAGE_TOML);
        let mut package = package_from(&member_manifest_path)?;
        crate::merge_auto_includes(&mut package, &root_auto_includes);
        package_info.push(package);
    }

    let mut sub_workspaces = vec![];
    for cargo_workspace_reldir in &manifest.workspace.cargo_workspaces {
        let cargo_workspace_dir = workspace_dir.join(cargo_workspace_reldir);
        let search = crate::rust::get_workspace(&cargo_workspace_dir, Some(&cargo_workspace_dir))
            .into_result()?;
        sub_workspaces.push(search);
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

// Load and process a dist.toml
fn package_from(manifest_path: &Utf8Path) -> Result<PackageInfo> {
    let manifest = load_package_dist_toml(manifest_path)?;
    let package = manifest.package;
    let version = package.version.map(Version::Generic);

    let manifest_path = manifest_path.to_path_buf();

    let mut info = PackageInfo {
        manifest_path: manifest_path.clone(),
        package_root: manifest_path.parent().unwrap().to_owned(),
        name: package.name,
        version,
        description: package.description,
        authors: package.authors,
        license: package.license,
        publish: true,
        keywords: None,
        repository_url: package.repository.clone(),
        homepage_url: package.homepage,
        documentation_url: package.documentation,
        readme_file: package.readme,
        license_files: package.license_files,
        changelog_file: package.changelog,
        binaries: package.binaries,
        cstaticlibs: package.cstaticlibs,
        cdylibs: package.cdylibs,
        build_command: Some(package.build_command),
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
