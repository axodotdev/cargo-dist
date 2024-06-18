//! Shared code for gathering up information about a workspace, used by various axo.dev tools
//! like cargo-dist and oranda.
//!
//! The main entry point is [`get_workspaces`][].

#![deny(missing_docs)]
#![allow(clippy::result_large_err)]

use std::fmt::Display;

#[cfg(feature = "cargo-projects")]
use axoasset::serde_json;
use axoasset::{AxoassetError, LocalAsset};
use camino::{Utf8Path, Utf8PathBuf};
use errors::{AxoprojectError, Result};
use tracing::info;

#[cfg(feature = "cargo-projects")]
pub use guppy::PackageId;

pub mod changelog;
pub mod errors;
#[cfg(feature = "generic-projects")]
pub mod generic;
#[cfg(feature = "npm-projects")]
pub mod javascript;
pub mod platforms;
mod repo;
#[cfg(feature = "cargo-projects")]
pub mod rust;
#[cfg(test)]
mod tests;

pub use crate::repo::GithubRepo;
use crate::repo::GithubRepoInput;
/// A sorted map impl
pub type SortedMap<K, V> = std::collections::BTreeMap<K, V>;

/// Workspaces
#[derive(Debug, Default)]
pub struct WorkspaceGraph {
    /// All workspaces
    workspaces: Vec<WorkspaceInfo>,
    /// All packages
    packages: Vec<PackageInfo>,
    workspace_workspace_children: SortedMap<WorkspaceIdx, Vec<WorkspaceIdx>>,
    workspace_package_children: SortedMap<WorkspaceIdx, Vec<PackageIdx>>,
    workspace_parents: SortedMap<WorkspaceIdx, WorkspaceIdx>,
    package_parents: SortedMap<PackageIdx, WorkspaceIdx>,
}

impl WorkspaceGraph {
    /// Create WorkspaceGraph
    pub fn find(start_dir: &Utf8Path, clamp_to_dir: Option<&Utf8Path>) -> Self {
        // TODO: add proper logic and error handling here!
        let mut workspaces = Self::default();
        let generic = generic::get_workspace(start_dir, clamp_to_dir);
        if let WorkspaceSearch::Found(ws) = generic {
            workspaces.add_workspace(ws, None);
            return workspaces;
        }

        let rust = rust::get_workspace(start_dir, clamp_to_dir);
        if let WorkspaceSearch::Found(ws) = rust {
            workspaces.add_workspace(ws, None);
            return workspaces;
        }

        workspaces
    }

    /// Add workspaces
    pub fn add_workspace(
        &mut self,
        mut workspace: WorkspaceInfo,
        parent_workspace: Option<WorkspaceIdx>,
    ) {
        let sub_workspaces = std::mem::take(&mut workspace._sub_workspaces);
        let packages = std::mem::take(&mut workspace._package_info);

        // Add the workspace and edges to parent
        let workspace_idx: WorkspaceIdx = WorkspaceIdx(self.workspaces.len());
        self.workspaces.push(workspace);
        if let Some(parent_workspace) = parent_workspace {
            self.workspace_workspace_children
                .entry(parent_workspace)
                .or_default()
                .push(workspace_idx);
            self.workspace_parents
                .insert(workspace_idx, parent_workspace);
        }

        // Add the packages and edges to the workspace
        for package in packages {
            let package_idx = PackageIdx(self.packages.len());
            self.packages.push(package);
            self.workspace_package_children
                .entry(workspace_idx)
                .or_default()
                .push(package_idx);
            self.package_parents.insert(package_idx, workspace_idx);
        }

        // Recursively add subworkspaces
        for sub_workspace in sub_workspaces {
            self.add_workspace(sub_workspace, Some(workspace_idx));
        }
    }

    /// Get the root workspace's index
    pub fn root_workspace_idx(&self) -> WorkspaceIdx {
        WorkspaceIdx(0)
    }
    /// Get the root workspace
    pub fn root_workspace(&self) -> &WorkspaceInfo {
        self.workspace(self.root_workspace_idx())
    }

    /// Get a workspace
    pub fn workspace(&self, idx: WorkspaceIdx) -> &WorkspaceInfo {
        &self.workspaces[idx.0]
    }
    /// Get a package
    pub fn package(&self, idx: PackageIdx) -> &PackageInfo {
        &self.packages[idx.0]
    }
    /// Get a mutable workspace
    pub fn workspace_mut(&mut self, idx: WorkspaceIdx) -> &mut WorkspaceInfo {
        &mut self.workspaces[idx.0]
    }
    /// Get a mutable package
    pub fn package_mut(&mut self, idx: PackageIdx) -> &mut PackageInfo {
        &mut self.packages[idx.0]
    }
    /// Get the parent workspace of a package
    pub fn workspace_for_package(&self, idx: PackageIdx) -> WorkspaceIdx {
        self.package_parents[&idx]
    }

    /// Get a workspace's packages (only direct children)
    pub fn direct_packages(
        &self,
        idx: WorkspaceIdx,
    ) -> impl Iterator<Item = (PackageIdx, &PackageInfo)> {
        self.workspace_package_children
            .get(&idx)
            .map(|pkgs| &**pkgs)
            .unwrap_or_default()
            .iter()
            .map(|p_idx| (*p_idx, &self.packages[p_idx.0]))
    }

    /// Get a workspace's packages (recursively getting children of subworkspaces)
    pub fn recursive_packages(
        &self,
        idx: WorkspaceIdx,
    ) -> impl Iterator<Item = (PackageIdx, &PackageInfo)> {
        let mut working_set = vec![idx];
        let mut package_indices = vec![];
        while let Some(workspace_idx) = working_set.pop() {
            // Add own packages
            if let Some(packages) = self.workspace_package_children.get(&workspace_idx) {
                package_indices.extend(packages.iter().copied());
            }
            // Add child workspaces to working set
            if let Some(sub_workspaces) = self.workspace_workspace_children.get(&workspace_idx) {
                working_set.extend(sub_workspaces.iter().copied());
            }
        }
        // Return result
        package_indices
            .into_iter()
            .map(|idx| (idx, self.package(idx)))
    }

    /// Get all packages
    pub fn all_packages(&self) -> impl Iterator<Item = (PackageIdx, &PackageInfo)> {
        self.packages
            .iter()
            .enumerate()
            .map(|(idx, pkg)| (PackageIdx(idx), pkg))
    }

    /// Get all workspaces
    pub fn all_workspace_indices(&self) -> impl Iterator<Item = WorkspaceIdx> {
        (0..self.workspaces.len()).map(WorkspaceIdx)
    }

    /// Try to get a consensus repository_url for the given packages (or all if None given)
    pub fn repository_url(&self, packages: Option<&[PackageIdx]>) -> Result<Option<RepositoryUrl>> {
        let package_list = if let Some(packages) = packages {
            packages
                .iter()
                .map(|idx| self.package(*idx))
                .collect::<Vec<_>>()
        } else {
            self.packages.iter().collect::<Vec<_>>()
        };
        RepositoryUrl::from_packages(package_list)
    }
}

/// Information about various kinds of workspaces
pub struct Workspaces {
    /// Info about the generic workspace
    #[cfg(feature = "generic-projects")]
    pub generic: WorkspaceSearch,
    /// Info about the cargo/rust workspace
    #[cfg(feature = "cargo-projects")]
    pub rust: WorkspaceSearch,
    /// Info about the npm/js workspace
    #[cfg(feature = "npm-projects")]
    pub javascript: WorkspaceSearch,
}

impl Workspaces {
    #[cfg(test)]
    pub(crate) fn best(self) -> Option<WorkspaceInfo> {
        #![allow(clippy::vec_init_then_push)]

        let mut best_project = None;
        let mut max_depth = 0;
        let mut projects = vec![];

        #[cfg(feature = "generic-projects")]
        projects.push(self.generic);

        // FIXME: should we provide feedback/logging here?
        #[cfg(feature = "cargo-projects")]
        projects.push(self.rust);

        #[cfg(feature = "npm-projects")]
        projects.push(self.javascript);

        // If we find multiple projects, prefer the one deeper in the file system
        // (the one closer to the start_dir).
        for project in projects {
            let WorkspaceSearch::Found(project) = project else {
                continue;
            };
            let depth = project.workspace_dir.ancestors().count();
            if depth > max_depth {
                best_project = Some(project);
                max_depth = depth;
            }
        }

        best_project
    }
}

/// Result of searching for a particular kind of workspace
pub enum WorkspaceSearch {
    /// We found it
    Found(WorkspaceInfo),
    /// We found something that looks like a workspace but there's something wrong with it
    Broken {
        /// Path to the closest manifest we found.
        ///
        /// Note that for workspaces we may have had a parsing error with a different file,
        /// but at least this is the file we found that made us discover that workspace!
        manifest_path: Utf8PathBuf,
        /// The error we encountered
        cause: AxoprojectError,
    },
    /// We found no hint of this kind of workspace
    Missing(AxoprojectError),
}

impl WorkspaceSearch {
    /// Simplify the search into a Result
    pub fn into_result(self) -> Result<WorkspaceInfo> {
        match self {
            WorkspaceSearch::Found(val) => Ok(val),
            WorkspaceSearch::Missing(val) => Err(val),
            WorkspaceSearch::Broken {
                manifest_path: _,
                cause,
            } => Err(cause),
        }
    }
}

/// Kind of workspace
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WorkspaceKind {
    /// generic cargo-dist compatible workspace
    #[cfg(feature = "generic-projects")]
    Generic,
    /// cargo/rust workspace
    #[cfg(feature = "cargo-projects")]
    Rust,
    /// npm/js workspace
    #[cfg(feature = "npm-projects")]
    Javascript,
}

/// Info on the current workspace
///
/// This can either be a cargo workspace or an npm workspace, the concepts
/// are conflated to let users of axoproject handle things more uniformly.
#[derive(Debug)]
pub struct WorkspaceInfo {
    /// The kind of workspace this is (Rust or Javascript)
    pub kind: WorkspaceKind,
    /// The directory where build output will go (generally `target/`)
    pub target_dir: Utf8PathBuf,
    /// The root directory of the workspace (where the root Cargo.toml is)
    pub workspace_dir: Utf8PathBuf,
    /// Nested workspaces (exists temp, do not use outside axoproject)
    pub _sub_workspaces: Vec<WorkspaceInfo>,
    /// Computed info about the packages (exists temp, do not use outside axoproject)
    ///
    /// This notably includes finding readmes and licenses even if the user didn't
    /// specify their location -- something Cargo does but Guppy (and cargo-metadata) don't.
    pub _package_info: Vec<PackageInfo>,
    /// Path to the root manifest of the workspace
    ///
    /// This can be either a Cargo.toml or package.json. In either case this manifest
    /// may or may not represent a "real" package. Both systems have some notion of
    /// "virtual" manifest which exists only to list the actual packages in the workspace.
    pub manifest_path: Utf8PathBuf,
    /// If the workspace root has some auto-includeable files, here they are!
    ///
    /// This is currently what is use for top-level Announcement contents.
    pub root_auto_includes: AutoIncludes,
    /// Non-fatal issues that were encountered and should probably be reported
    pub warnings: Vec<AxoprojectError>,
    /// Raw cargo `[workspace.metadata]` table
    #[cfg(feature = "cargo-projects")]
    pub cargo_metadata_table: Option<serde_json::Value>,
    /// Any [profile.*] entries we found in the root Cargo.toml
    #[cfg(feature = "cargo-projects")]
    pub cargo_profiles: rust::CargoProfiles,
}

/// A URL to a repository, with some normalization applied
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct RepositoryUrl(pub String);

impl std::ops::Deref for RepositoryUrl {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl RepositoryUrl {
    /// Construct a RepositoryUrl from a String
    pub fn from_string(url: impl Into<String>) -> Self {
        let mut url = url.into();
        // Normalize away trailing `/` stuff
        if url.ends_with('/') {
            url.pop();
        }
        Self(url)
    }

    /// Returns a consensus package URL for the given packages, if any exists
    pub fn from_packages<'a>(
        packages: impl IntoIterator<Item = &'a PackageInfo>,
    ) -> Result<Option<Self>> {
        let mut repo_url = None::<RepositoryUrl>;
        let mut repo_url_origin = None::<Utf8PathBuf>;

        for info in packages {
            if let Some(new_url) = &info.repository_url {
                let normalized_new_url = RepositoryUrl::from_string(new_url);
                if let Some(cur_url) = &repo_url {
                    if &normalized_new_url == cur_url {
                        // great! consensus!
                    } else {
                        return Err(AxoprojectError::InconsistentRepositoryKey {
                            file1: repo_url_origin.as_ref().unwrap().to_owned(),
                            url1: cur_url.0.clone(),
                            file2: info.manifest_path.clone(),
                            url2: normalized_new_url.0,
                        });
                    }
                } else {
                    repo_url = Some(normalized_new_url);
                    repo_url_origin = Some(info.manifest_path.clone());
                }
            }
        }
        Ok(repo_url)
    }

    /// Returns a struct which contains the repository's owner and name.
    pub fn github_repo(&self) -> Result<GithubRepo> {
        GithubRepoInput::new(self.0.clone())?.parse()
    }
}

impl WorkspaceInfo {
    /// Get a package
    pub fn package(&self, idx: PackageIdx) -> &PackageInfo {
        &self._package_info[idx.0]
    }
    /// Get a mutable package
    pub fn package_mut(&mut self, idx: PackageIdx) -> &mut PackageInfo {
        &mut self._package_info[idx.0]
    }
    /// Iterate over packages
    pub fn packages(&self) -> impl Iterator<Item = (PackageIdx, &PackageInfo)> {
        self._package_info
            .iter()
            .enumerate()
            .map(|(i, k)| (PackageIdx(i), k))
    }
}

/// Computed info about a package
///
/// This notably includes finding readmes and licenses even if the user didn't
/// specify their location -- something Cargo does but Guppy (and cargo-metadata) don't.
#[derive(Debug)]
pub struct PackageInfo {
    /// Path to the manifest for this package
    pub manifest_path: Utf8PathBuf,
    /// Path to the root dir for this package
    pub package_root: Utf8PathBuf,
    /// Name of the package
    ///
    /// This can actually be missing for JS packages, but in that case it's basically
    /// the same thing as a "virtual manifest" in Cargo. PackageInfo is only for concrete
    /// packages so we don't need to allow for that.
    pub name: String,
    /// Version of the package
    ///
    /// Both cargo and npm use SemVer but they disagree slightly on what that means:
    ///
    /// * cargo: <https://crates.io/crates/semver>
    /// * npm: <https://crates.io/crates/node-semver>
    ///
    /// Cargo requires this field at all times, npm only requires it to publish.
    /// Probably we could get away with making it non-optional but allowing this
    /// theoretically lets npm users "kick the tires" even when they're not ready
    /// to publish.
    pub version: Option<Version>,
    /// A brief description of the package
    pub description: Option<String>,
    /// Authors of the package (may be empty)
    pub authors: Vec<String>,
    /// The license the package is provided under
    pub license: Option<String>,
    /// False if they set publish=false, true otherwise
    ///
    /// Currently always true for npm packages.
    pub publish: bool,
    /// Package keywords AND/OR categories.
    ///
    /// Specifically, Cargo has both the notion
    /// of a "package keyword" (free-form text) and a "package category" (one of circa 70
    /// predefined categories accepted by crates.io). We don't really care about validating
    /// these, though, and just squash them together with the keywords.
    pub keywords: Option<Vec<String>>,
    /// URL to the repository for this package
    ///
    /// This URL can be used by various CI/Installer helpers. In the future we
    /// might also use it for auto-detecting "hey you're using github, here's the
    /// recommended github setup".
    ///
    /// i.e. `cargo dist init --installer=shell` uses this as the base URL for fetching from
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
    /// If the user specifies where this is, we'll respect it. Otherwise we'll try to find
    /// this in the workspace using AutoIncludes.
    pub readme_file: Option<Utf8PathBuf>,
    /// Paths to the LICENSE files for this package.
    ///
    /// By default these should be copied into a zip containing this package's binary.
    ///
    /// If the user specifies where this is, we'll respect it. Otherwise we'll try to find
    /// this in the workspace using AutoIncludes.
    ///
    /// Cargo only lets you specify one such path, but that's because its license-path
    /// key primarily exists as an escape hatch for someone's whacky-wild custom license.
    /// Ultimately Cargo's license-path is inadequate for Normal Licenses because it
    /// can't handle the standard pattern of dual licensing MIT/Apache and having two
    /// license files. AutoIncludes properly handles dual licensing.
    pub license_files: Vec<Utf8PathBuf>,
    /// Paths to the CHANGELOG or RELEASES file for this package
    ///
    /// By default this should be copied into a zip containing this package's binary.
    ///
    /// We will *try* to parse this
    pub changelog_file: Option<Utf8PathBuf>,
    /// Names of binaries this package defines
    ///
    /// For Cargo this is currently properly computed in all its complexity.
    /// For JS I *think* this is computed in its full complexity but Tests Needed
    /// and also there's so many ways to define things who can ever be sure.
    pub binaries: Vec<String>,
    /// Names of C-style staticlibs (.a) this library defines.
    ///
    /// For Cargo this is currently properly computed in all its complexity.
    /// For JS we don't compute this at all.
    pub cstaticlibs: Vec<String>,
    /// Names of C-style dylibs (.dll, .so, ...) this package defines
    ///
    /// For Cargo this is currently properly computed in all its complexity.
    /// For JS we don't compute this at all.
    pub cdylibs: Vec<String>,
    /// Raw cargo `[package.metadata]` table
    #[cfg(feature = "cargo-projects")]
    pub cargo_metadata_table: Option<serde_json::Value>,
    /// A unique id used by Cargo to refer to the package
    #[cfg(feature = "cargo-projects")]
    pub cargo_package_id: Option<PackageId>,
    /// Command to run to build this package
    #[cfg(feature = "generic-projects")]
    pub build_command: Option<Vec<String>>,
}

impl PackageInfo {
    /// Returns a struct which contains the repository's owner and name.
    pub fn github_repo(&self) -> Result<Option<GithubRepo>> {
        match self.repository_url.clone() {
            None => Ok(None),
            Some(url) => Ok(Some(GithubRepoInput::new(url)?.parse()?)),
        }
    }

    /// Returns a web version of the repository URL,
    /// converted from SSH if necessary, with .git suffix trimmed.
    pub fn web_url(&self) -> Result<Option<String>> {
        Ok(self.github_repo()?.map(|repo| repo.web_url()))
    }
}

/// An id for a [`PackageInfo`][] entry in a [`WorkspaceInfo`][] or [`WorkspaceGraph`][].
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageIdx(pub usize);

/// An id for a [`WorkspaceInfo`][] entry in a [`WorkspaceGraph`][].
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkspaceIdx(pub usize);

/// A Version abstracted over project type
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Version {
    /// generic version (assumed to be semver)
    #[cfg(feature = "generic-projects")]
    Generic(semver::Version),
    /// cargo version
    #[cfg(feature = "cargo-projects")]
    Cargo(semver::Version),
    /// npm version
    #[cfg(feature = "npm-projects")]
    Npm(node_semver::Version),
}

impl Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(feature = "generic-projects")]
            Version::Generic(v) => v.fmt(f),
            #[cfg(feature = "cargo-projects")]
            Version::Cargo(v) => v.fmt(f),
            #[cfg(feature = "npm-projects")]
            Version::Npm(v) => v.fmt(f),
        }
    }
}

impl Version {
    /// Assume it's a cargo Version
    #[cfg(feature = "cargo-projects")]
    pub fn cargo(&self) -> &semver::Version {
        #[allow(irrefutable_let_patterns)]
        if let Version::Cargo(v) = self {
            v
        } else {
            panic!("Version wasn't in the cargo format")
        }
    }

    /// Returns a semver-based Version
    #[cfg(any(feature = "generic-projects", feature = "cargo-projects"))]
    pub fn semver(&self) -> &semver::Version {
        #[allow(unreachable_patterns)]
        match self {
            #[cfg(feature = "generic-projects")]
            Version::Generic(v) => v,
            #[cfg(feature = "cargo-projects")]
            Version::Cargo(v) => v,
            _ => panic!("Version wasn't in semver format"),
        }
    }

    /// Assume it's an npm Version
    #[cfg(feature = "npm-projects")]
    pub fn npm(&self) -> &node_semver::Version {
        #[allow(irrefutable_let_patterns)]
        if let Version::Npm(v) = self {
            v
        } else {
            panic!("Version wasn't in the npm format")
        }
    }

    /// Returns whether the version is stable (no pre/build component)
    pub fn is_stable(&self) -> bool {
        match self {
            #[cfg(feature = "generic-projects")]
            Version::Generic(v) => v.pre.is_empty() && v.build.is_empty(),
            #[cfg(feature = "cargo-projects")]
            Version::Cargo(v) => v.pre.is_empty() && v.build.is_empty(),
            #[cfg(feature = "npm-projects")]
            Version::Npm(v) => v.pre_release.is_empty() && v.build.is_empty(),
        }
    }

    /// Gets a copy of the version with only the stable parts (pre/build components stripped)
    pub fn stable_part(&self) -> Self {
        match self {
            #[cfg(feature = "generic-projects")]
            Version::Generic(v) => {
                Version::Generic(semver::Version::new(v.major, v.minor, v.patch))
            }
            #[cfg(feature = "cargo-projects")]
            Version::Cargo(v) => Version::Cargo(semver::Version::new(v.major, v.minor, v.patch)),
            #[cfg(feature = "npm-projects")]
            Version::Npm(v) => Version::Npm(node_semver::Version {
                major: v.major,
                minor: v.minor,
                patch: v.patch,
                build: vec![],
                pre_release: vec![],
            }),
        }
    }
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

/// Tries to find information about the workspace at start_dir, walking up
/// ancestors as necessary until we reach clamp_to_dir (or run out of ancestors).
///
/// Behaviour is unspecified if only part of the workspace is nested in clamp_to_dir.
///
/// In the future setting clamp_to_dir may cause the output's paths to be relative
/// to that directory, but for now they're always absolute. The cli does this
/// relativizing, but not the library.
///
/// This can be either a cargo project or an npm project. Support for each
/// one is behind feature flags:
///
/// * cargo-projects
/// * npm-projects
///
/// Concepts of both will largely be conflated, the only distinction will be
/// the top level [`WorkspaceKind`][].
pub fn get_workspaces(start_dir: &Utf8Path, clamp_to_dir: Option<&Utf8Path>) -> Workspaces {
    Workspaces {
        #[cfg(feature = "generic-projects")]
        generic: generic::get_workspace(start_dir, clamp_to_dir),
        #[cfg(feature = "cargo-projects")]
        rust: rust::get_workspace(start_dir, clamp_to_dir),
        #[cfg(feature = "npm-projects")]
        javascript: javascript::get_workspace(start_dir, clamp_to_dir),
    }
}

/// Find auto-includeable files in a dir
///
/// This includes:
///
/// * reamde: `README*`
/// * license: `LICENSE*` and `UNLICENSE*`
/// * changelog: `CHANGELOG*` and `RELEASES*`
///
/// This doesn't look at parent/child dirs, and doesn't factor in user provided paths.
/// Handle those details by using [`merge_auto_includes`][] to merge the results into a [`PackageInfo`].
pub fn find_auto_includes(dir: &Utf8Path) -> Result<AutoIncludes> {
    find_auto_includes_inner(dir).map_err(|details| AxoprojectError::AutoIncludeSearch {
        dir: dir.to_owned(),
        details,
    })
}

fn find_auto_includes_inner(dir: &Utf8Path) -> std::result::Result<AutoIncludes, std::io::Error> {
    // Is there a better way to get the path to sniff?
    // Should we spider more than just package_root and workspace_root?
    // Should we more carefully prevent grabbing LICENSES from both dirs?
    // Should we not spider the workspace root for README since Cargo has a proper field for this?
    // Should we check for a "readme=..." on the workspace root Cargo.toml?

    let mut includes = AutoIncludes {
        readme: None,
        licenses: vec![],
        changelog: None,
    };

    // Iterate over files in the dir
    let entries = dir.read_dir_utf8()?;

    for entry in entries {
        // Make sure it's a file
        //
        // I think this *may* mishandle symlinks, Rust's docs have some notes that
        // the only reliable way to check if something is a file is to try to Open it,
        // but honestly I don't super care about someone symlinking a README???
        let entry = entry?;
        let meta = entry.file_type()?;
        if !meta.is_file() {
            continue;
        }
        let file_name = entry.file_name();
        if file_name.starts_with("README") {
            // Found a readme! It doesn't really make sense to have multiple of these,
            // so we just need to pick one (probably will never be stressed...)
            if includes.readme.is_none() {
                let path = entry.path().to_owned();
                info!("Found README at {}", path);
                includes.readme = Some(path);
            } else {
                info!("Ignoring duplicate candidate README at {}", entry.path());
            }
        } else if file_name.starts_with("LICENSE") || file_name.starts_with("UNLICENSE") {
            // Found a license! Dual licensing means we will often have multiple of these,
            // so we should grab every one we can find!
            let path = entry.path().to_owned();
            info!("Found LICENSE at {}", path);
            includes.licenses.push(path);
        } else if file_name.starts_with("CHANGELOG") || file_name.starts_with("RELEASES") {
            // Found a changelog! It doesn't really make sense to have multiple of these,
            // so we just need to pick one? Might one day become untrue if we work out
            // how to do changelogs for independently versioned/released monorepos.
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

/// Merge AutoIncluded files into PackageInfo, preferring already existing values
/// over the AutoIncludes. The expected way to use this is:
///
/// 1. Compute PackageInfo from a manifest, populate fields with user-provided paths
/// 2. Compute AutoIncludes for the package's root dir, merge them in
/// 3. Compute AutoIncludes for the workspace's root dir, merge them in
///
/// This naturally cascades results.
pub fn merge_auto_includes(info: &mut PackageInfo, auto_includes: &AutoIncludes) {
    if info.readme_file.is_none() {
        info.readme_file.clone_from(&auto_includes.readme);
    }
    if info.changelog_file.is_none() {
        info.changelog_file.clone_from(&auto_includes.changelog);
    }
    // Note that even though we allow for multiple licenses, it's supremely wonky
    // to source them from multiple locations, so if any source provides a license
    // we will ignore all the other ones.
    if info.license_files.is_empty() {
        info.license_files.clone_from(&auto_includes.licenses);
    }
}

/// Find a file with the given name, starting at the given dir and walking up to ancestor dirs,
/// optionally clamped to a given ancestor dir
pub fn find_file(
    name: &str,
    start_dir: &Utf8Path,
    clamp_to_dir: Option<&Utf8Path>,
) -> Result<Utf8PathBuf> {
    let manifest = LocalAsset::search_ancestors(start_dir, name)?;

    if let Some(root_dir) = clamp_to_dir {
        let root_dir = if root_dir.is_relative() {
            let current_dir = LocalAsset::current_dir()?;
            current_dir.join(root_dir)
        } else {
            root_dir.to_owned()
        };

        let improperly_nested = pathdiff::diff_utf8_paths(&manifest, root_dir)
            .map(|p| p.starts_with(".."))
            .unwrap_or(true);

        if improperly_nested {
            Err(AxoassetError::SearchFailed {
                start_dir: start_dir.to_owned(),
                desired_filename: name.to_owned(),
            })?;
        }
    }

    Ok(manifest)
}
