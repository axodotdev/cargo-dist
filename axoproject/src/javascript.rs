//! Support for npm-based JavaScript projects

use axoasset::{AxoassetError, LocalAsset, SourceFile};
use camino::{Utf8Path, Utf8PathBuf};
use oro_common::{Manifest, Repository};
use oro_package_spec::GitInfo;

use crate::{
    errors::AxoprojectError, PackageInfo, Result, Version, WorkspaceInfo, WorkspaceKind,
    WorkspaceSearch,
};

/// Try to find an npm/js workspace at the given path.
///
/// See [`crate::get_workspaces`][] for the semantics.
///
/// This relies on orogene's understanding of npm packages.
pub fn get_workspace(root_dir: Option<&Utf8Path>, start_dir: &Utf8Path) -> WorkspaceSearch {
    let manifest_path = match workspace_manifest(root_dir, start_dir) {
        Ok(path) => path,
        Err(e) => {
            return WorkspaceSearch::Missing(e);
        }
    };
    match read_workspace(manifest_path) {
        Ok(workspace) => WorkspaceSearch::Found(workspace),
        Err(e) => WorkspaceSearch::Broken(e),
    }
}

fn read_workspace(manifest_path: Utf8PathBuf) -> Result<WorkspaceInfo> {
    let root = manifest_path.parent().unwrap().to_owned();
    let manifest = load_manifest(&manifest_path)?;

    // For now this code is fairly naive and doesn't understand workspaces.
    // We assume the first package.json we find is "the root package" and
    // has the binary we care about.

    // Just assume ./node_modules is the target?
    let target_dir = root.join("node_modules");

    let root_auto_includes = crate::find_auto_includes(&root)?;

    // Not having a name is common for virtual manifests, but we don't handle those!
    let Some(package_name) = manifest.name else {
        return Err(crate::errors::AxoprojectError::NamelessNpmPackage { manifest: manifest_path });
    };
    let version = manifest.version.map(Version::Npm);
    let authors = manifest
        .author
        .and_then(|a| match a {
            oro_common::PersonField::Str(s) => Some(vec![s]),
            // FIXME: Not yet implemented!
            oro_common::PersonField::Obj(_) => None,
        })
        .unwrap_or_default();

    // FIXME: do we care that we're dropping lots of useful semantic info on the ground here?
    let mut repository_url = manifest.repository.and_then(|url| match url {
        Repository::Str(magic) => {
            // This "shorthand" form can be all kinds of magic things that we need to try to
            // parse out. Thankfully oro-package-spec provides an implementation of this with
            // the FromString impl of GitInfo. If we can't parse it, that's fine, just drop it.
            let obj: Option<GitInfo> = magic.parse().ok();
            obj.and_then(|obj| obj.https())
                .as_ref()
                .map(ToString::to_string)
        }
        Repository::Obj { url, .. } => url,
    });
    // Normalize away trailing `/` on repo URL
    if let Some(repo_url) = &mut repository_url {
        if repo_url.ends_with('/') {
            repo_url.pop();
        }
    }

    // FIXME: it's unfortunate that we're loading the package.json twice!
    // Also arguably we shouldn't hard fail if we fail to make sense of the
    // binaries... except the whole point of axoproject is to find binaries?
    let build_manifest =
        oro_common::BuildManifest::from_path(&manifest_path).map_err(|details| {
            AxoprojectError::BuildInfoParse {
                manifest_path: manifest_path.clone(),
                details,
            }
        })?;
    let mut binaries = build_manifest
        .bin
        .into_iter()
        .map(|k| k.0)
        .collect::<Vec<_>>();
    binaries.sort();

    let mut info = PackageInfo {
        name: package_name,
        version,
        manifest_path: manifest_path.clone(),
        package_root: root.clone(),
        description: manifest.description,
        authors,
        license: manifest.license,
        // FIXME: is there any JS equivalent to this?
        publish: true,
        repository_url: repository_url.clone(),
        homepage_url: manifest.homepage,
        // FIXME: is there any JS equivalent to this?
        documentation_url: None,
        // FIXME: is there any JS equivalent to this?
        readme_file: None,
        // FIXME: is there any JS equivalent to this?
        license_files: vec![],
        // FIXME: is there any JS equivalent to this?
        changelog_file: None,
        binaries,
        // FIXME: is there any JS equivalent to this?
        cdylibs: vec![],
        // FIXME: is there any JS equivalent to this?
        cstaticlibs: vec![],
        #[cfg(feature = "cargo-projects")]
        cargo_metadata_table: None,
        #[cfg(feature = "cargo-projects")]
        cargo_package_id: None,
    };
    crate::merge_auto_includes(&mut info, &root_auto_includes);

    let package_info = vec![info];

    Ok(WorkspaceInfo {
        kind: WorkspaceKind::Javascript,
        target_dir,
        workspace_dir: root,
        package_info,
        manifest_path,
        repository_url,
        root_auto_includes,
        warnings: vec![],
        #[cfg(feature = "cargo-projects")]
        cargo_metadata_table: None,
        #[cfg(feature = "cargo-projects")]
        cargo_profiles: crate::rust::CargoProfiles::new(),
    })
}

/// Find the workspace root given this starting dir (potentially walking up to ancestor dirs)
fn workspace_manifest(root_dir: Option<&Utf8Path>, start_dir: &Utf8Path) -> Result<Utf8PathBuf> {
    let manifest = LocalAsset::search_ancestors(start_dir, "package.json")?;

    if let Some(root_dir) = root_dir {
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
            return Err(AxoassetError::SearchFailed {
                start_dir: start_dir.to_owned(),
                desired_filename: "package.json".to_owned(),
            })?;
        }
    }

    Ok(manifest)
}

/// Load and parse a package.json
fn load_manifest(manifest_path: &Utf8Path) -> Result<Manifest> {
    let source = SourceFile::load_local(manifest_path)?;
    let manifest = source.deserialize_json()?;
    Ok(manifest)
}
