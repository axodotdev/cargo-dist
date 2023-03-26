//! Support for npm-based JavaScript projects

use std::{fs::File, io::BufReader};

use camino::{Utf8Path, Utf8PathBuf};
use miette::{miette, Context, IntoDiagnostic};
use oro_common::{Manifest, Repository};
use oro_package_spec::GitInfo;

use crate::{PackageInfo, Result, Version, WorkspaceInfo, WorkspaceKind};

/// Try to find an npm/js project at the given path.
///
/// This relies on orogene's understanding of npm packages.
pub fn get_project(start_dir: &Utf8Path) -> Result<WorkspaceInfo> {
    let root = workspace_root(start_dir)?;
    let manifest_path = root.join("package.json");
    let manifest = load_manifest(&manifest_path)?;

    // For now this code is fairly naive and doesn't understand workspaces.
    // We assume the first package.json we find is "the root package" and
    // has the binary we care about.

    // Just assume ./node_modules is the target?
    let target_dir = root.join("node_modules");

    let root_auto_includes = crate::find_auto_includes(&root)?;

    // Not having a name is common for virtual manifests, but we don't handle those!
    let package_name = manifest
        .name
        .expect("your package doesn't have a name, is it a workspace? We don't support that yet.");
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
        if repo_url.ends_with("/") {
            repo_url.pop();
        }
    }

    // FIXME: it's unfortunate that we're loading the package.json twice!
    // Also arguably we shouldn't hard fail if we fail to make sense of the
    // binaries... except the whole point of axo-project is to find binaries?
    let build_manifest = oro_common::BuildManifest::from_path(&manifest_path)
        .into_diagnostic()
        .wrap_err("failed to parse package.json binary info")?;
    let binaries = build_manifest
        .bin
        .into_iter()
        .map(|k| k.0)
        .collect::<Vec<_>>();

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
        // FIXME: don't just assume this is a binary?
        binaries,
        #[cfg(feature = "cargo-projects")]
        cargo_metadata_table: None,
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
        #[cfg(feature = "cargo-projects")]
        cargo_metadata_table: None,
        #[cfg(feature = "cargo-projects")]
        cargo_profiles: crate::rust::CargoProfiles::new(),
    })
}

/// Find the workspace root given this starting dir (potentially walking up to ancestor dirs)
fn workspace_root(start_dir: &Utf8Path) -> Result<Utf8PathBuf> {
    for path in start_dir.ancestors() {
        // NOTE: orogene also looks for node_modules, but we can't do anything if there's
        // no package.json, so we can just ignore that approach?
        let pkg_json = path.join("package.json");
        if pkg_json.is_file() {
            return Ok(path.to_owned());
        }
    }
    Err(miette!("failed to find a dir with a package.json"))
}

/// Load and parse a package.json
fn load_manifest(manifest_path: &Utf8Path) -> Result<Manifest> {
    let file = File::open(manifest_path)
        .into_diagnostic()
        .wrap_err("failed to read package.json")?;
    let reader = BufReader::new(file);
    let manifest: Manifest = serde_json::from_reader(reader)
        .into_diagnostic()
        .wrap_err("failed to parse package.json")?;
    Ok(manifest)
}
