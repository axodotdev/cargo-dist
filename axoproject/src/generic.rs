//! Support for generic projects with cargo-dist build instructions

use axoasset::SourceFile;
use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

use crate::{PackageInfo, Result, Version, WorkspaceInfo, WorkspaceSearch};

#[derive(Deserialize)]
struct Manifest {
    package: Package,
}

#[derive(Deserialize)]
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
    let manifest_path = match crate::find_file("dist.toml", start_dir, clamp_to_dir) {
        Ok(path) => path,
        Err(e) => return WorkspaceSearch::Missing(e),
    };

    match workspace_from(&manifest_path) {
        Ok(info) => WorkspaceSearch::Found(info),
        Err(e) => WorkspaceSearch::Broken {
            manifest_path,
            cause: e,
        },
    }
}

fn workspace_from(manifest_path: &Utf8Path) -> Result<WorkspaceInfo> {
    let workspace_dir = manifest_path.parent().unwrap().to_path_buf();
    let root_auto_includes = crate::find_auto_includes(&workspace_dir)?;

    let manifest: Manifest = load_root_dist_toml(manifest_path)?;
    let package = manifest.package;
    let version = package.version.map(Version::Generic);

    let manifest_path = manifest_path.to_path_buf();

    let package_info = PackageInfo {
        manifest_path: manifest_path.clone(),
        package_root: manifest_path.clone(),
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
        #[cfg(feature = "cargo-projects")]
        cargo_metadata_table: None,
        #[cfg(feature = "cargo-projects")]
        cargo_package_id: None,
    };

    Ok(WorkspaceInfo {
        kind: crate::WorkspaceKind::Generic,
        target_dir: workspace_dir.join("target"),
        workspace_dir,
        package_info: vec![package_info],
        manifest_path,
        repository_url: package.repository,
        root_auto_includes,
        warnings: vec![],
        build_command: Some(package.build_command),
        #[cfg(feature = "cargo-projects")]
        cargo_metadata_table: None,
        #[cfg(feature = "cargo-projects")]
        cargo_profiles: crate::rust::CargoProfiles::new(),
    })
}

/// Load the root workspace toml
fn load_root_dist_toml(manifest_path: &Utf8Path) -> Result<Manifest> {
    let manifest_src = SourceFile::load_local(manifest_path)?;
    let manifest = manifest_src.deserialize_toml()?;
    Ok(manifest)
}
