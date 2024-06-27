//! Support for npm-based JavaScript projects

use axoasset::SourceFile;
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
pub fn get_workspace(start_dir: &Utf8Path, clamp_to_dir: Option<&Utf8Path>) -> WorkspaceSearch {
    let manifest_path = match workspace_manifest(start_dir, clamp_to_dir) {
        Ok(path) => path,
        Err(e) => {
            return WorkspaceSearch::Missing(e);
        }
    };
    match read_workspace(&manifest_path) {
        Ok(workspace) => WorkspaceSearch::Found(workspace),
        Err(e) => WorkspaceSearch::Broken {
            manifest_path,
            cause: e,
        },
    }
}

fn read_workspace(manifest_path: &Utf8Path) -> Result<WorkspaceInfo> {
    let root = manifest_path.parent().unwrap().to_owned();
    let manifest = load_manifest(manifest_path)?;

    // For now this code is fairly naive and doesn't understand workspaces.
    // We assume the first package.json we find is "the root package" and
    // has the binary we care about.

    // Just assume ./node_modules is the target?
    let target_dir = root.join("node_modules");

    let root_auto_includes = crate::find_auto_includes(&root)?;

    // Not having a name is common for virtual manifests, but we don't handle those!
    let Some(package_name) = manifest.name else {
        return Err(crate::errors::AxoprojectError::NamelessNpmPackage {
            manifest: manifest_path.to_owned(),
        });
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
    let repository_url = manifest.repository.and_then(|url| match url {
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

    // FIXME: it's unfortunate that we're loading the package.json twice!
    // Also arguably we shouldn't hard fail if we fail to make sense of the
    // binaries... except the whole point of axoproject is to find binaries?
    let build_manifest =
        oro_common::BuildManifest::from_path(manifest_path).map_err(|details| {
            AxoprojectError::BuildInfoParse {
                manifest_path: manifest_path.to_owned(),
                details,
            }
        })?;
    let mut binaries = build_manifest
        .bin
        .into_iter()
        .map(|k| k.0)
        .collect::<Vec<_>>();
    binaries.sort();

    let keywords = if manifest.keywords.is_empty() {
        None
    } else {
        // `manifest.keywords` is a `Vec<String, Global>`, which we need to normalize.
        Some(manifest.keywords.into_iter().collect::<Vec<String>>())
    };

    let mut info = PackageInfo {
        name: package_name,
        version,
        manifest_path: manifest_path.to_owned(),
        package_root: root.clone(),
        description: manifest.description,
        authors,
        license: manifest.license,
        // FIXME: is there any JS equivalent to this?
        publish: true,
        repository_url: repository_url.clone(),
        homepage_url: manifest.homepage,
        keywords,
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
        #[cfg(feature = "generic-projects")]
        build_command: None,
    };
    crate::merge_auto_includes(&mut info, &root_auto_includes);

    let package_info = vec![info];

    Ok(WorkspaceInfo {
        kind: WorkspaceKind::Javascript,
        target_dir,
        workspace_dir: root,
        _sub_workspaces: vec![],
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

/// Find a package.json, starting at the given dir and walking up to ancestor dirs,
/// optionally clamped to a given ancestor dir
fn workspace_manifest(
    start_dir: &Utf8Path,
    clamp_to_dir: Option<&Utf8Path>,
) -> Result<Utf8PathBuf> {
    crate::find_file("package.json", start_dir, clamp_to_dir)
}

/// Load and parse a package.json
fn load_manifest(manifest_path: &Utf8Path) -> Result<Manifest> {
    let source = SourceFile::load_local(manifest_path)?;
    let manifest = source.deserialize_json()?;
    Ok(manifest)
}
