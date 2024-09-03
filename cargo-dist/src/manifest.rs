//! Utilities for managing DistManifests
//!
//! dist-manifest.json serves 3 purposes:
//!
//! * providing a preview of what the build will produce before doing it
//! * providing final information for a build
//! * being a communication protocol between build machines
//!
//! The flow of data into the manifest is as follows (see gather_work):
//!
//! 1. Create DistGraphBuilder with a nearly default/empty manifest.
//!    This is a baseline value that will be iteratively refined.
//!
//! 2. Find dist-manifest files in the dist dir, import and merge them.
//!    This typically is importing manifests from other machines, such
//!    as the 'plan' machine which allocated a hosting bucket or
//!    the 'build-*' machines which computed system info and linkage.
//!
//! 3. Compute Hosting (if not covered by 2), potentially allocating a
//!    hosting bucket that we'll be uploading final results to. This needs
//!    to be known early because the resulting URLs need to be baked into
//!    installers.
//!
//! 3. Build the DistGraph, representing the things the current machine
//!    is supposed to build. Update the dist-manifest.json with those
//!    entries (Releases and Artifacts).
//!
//! 4. Compute Announcement info, potentially populating things like
//!    changelogs and titles.
//!
//! 5. Compute CI Info, potentially populating things like github ci matrices.
//!
//! 6. Build binaries, adding information about each built binary to Assets.
//!
//! 7. Build installers, using information in the manifest from steps 2, 3, and 4.

use std::collections::btree_map::Entry;

use camino::{Utf8Path, Utf8PathBuf};
use cargo_dist_schema::{
    Artifact, ArtifactId, Asset, AssetKind, DistManifest, DynamicLibraryAsset, ExecutableAsset,
    Hosting, StaticLibraryAsset,
};
use tracing::warn;

use crate::{
    announce::AnnouncementTag,
    backend::{
        installer::{homebrew::HomebrewInstallerInfo, npm::NpmInstallerInfo, InstallerImpl},
        templates::{TemplateEntry, TEMPLATE_INSTALLER_NPM},
    },
    config::Config,
    errors::DistResult,
    ArtifactIdx, ArtifactKind, DistGraph, Release, StaticAssetKind,
};

/// Load DistManifests into the given dir and merge them into the current one
pub(crate) fn load_and_merge_manifests(
    manifest_dir: &Utf8Path,
    output: &mut DistManifest,
    announcing: &AnnouncementTag,
) -> DistResult<()> {
    // Hey! Update the loop below too if you're adding a field!

    let manifests = load_manifests(manifest_dir)?;
    for manifest in manifests {
        let DistManifest {
            // There's one value and N machines (redesign required for per-machine values)
            // although dist_version *really* should be stable across all machines
            dist_version: _,
            // one value N machines
            system_info: _,
            announcement_tag,
            announcement_tag_is_implicit: _,
            announcement_is_prerelease: _,
            announcement_title: _,
            announcement_changelog: _,
            announcement_github_body: _,
            publish_prereleases: _,
            force_latest: _,
            upload_files: _,
            artifacts,
            releases,
            systems,
            assets,
            ci,
            linkage,
            github_attestations: _,
        } = manifest;

        // Discard clearly unrelated manifests
        if let Some(tag) = &announcement_tag {
            if tag != &announcing.tag {
                warn!("found old manifest for the tag {announcement_tag:?}, ignoring it");
                continue;
            }
        }

        // Merge every release
        for release in releases {
            // Ensure a release with this name and version exists
            let out_release =
                output.ensure_release(release.app_name.clone(), release.app_version.clone());
            // If the input has hosting info, apply it
            let Hosting { axodotdev, github } = release.hosting;
            if let Some(hosting) = axodotdev {
                out_release.hosting.axodotdev = Some(hosting);
            }
            if let Some(hosting) = github {
                out_release.hosting.github = Some(hosting);
            }
            // If the input has a list of artifacts for this release, merge them
            for artifact in release.artifacts {
                if !out_release.artifacts.contains(&artifact) {
                    out_release.artifacts.push(artifact);
                }
            }
        }

        for (artifact_id, artifact) in artifacts {
            merge_artifact(output, artifact_id, artifact);
        }

        if let Some(val) = ci {
            // Don't bother doing an inner merge here, all or nothing
            output.ci = Some(val);
        };

        // Just merge all the system-specific info
        output.systems.extend(systems);
        output.assets.extend(assets);
        output.linkage.extend(linkage);
    }

    Ok(())
}

/// Merge the artifact entries at a more granular level.
///
/// At a fundamental level here we're trying to populate artifact[].assets[].id
/// if another machine set it (indicating they actually built that asset), while
/// still allowing for other manifests to contain these same artifacts entries
/// without any conflict.
fn merge_artifact(output: &mut DistManifest, artifact_id: ArtifactId, artifact: Artifact) {
    match output.artifacts.entry(artifact_id) {
        Entry::Vacant(out_artifact) => {
            out_artifact.insert(artifact);
        }
        Entry::Occupied(mut out_artifact) => {
            let out_artifact = out_artifact.get_mut();

            // Merge checksums
            out_artifact.checksums.extend(artifact.checksums);

            // Merge assets
            for asset in artifact.assets {
                if let Some(out_asset) = out_artifact
                    .assets
                    .iter_mut()
                    .find(|a| a.path == asset.path)
                {
                    if let Some(id) = asset.id {
                        out_asset.id = Some(id);
                    }
                } else {
                    out_artifact.assets.push(asset);
                }
            }
        }
    }
}

/// Load manifests from the current dir
fn load_manifests(manifest_dir: &Utf8Path) -> DistResult<Vec<crate::DistManifest>> {
    // This happens on clean builds with no manifests to slurp up, and the dist-dir
    // not yet created. In that case there's clearly nothing to import!
    if !manifest_dir.exists() {
        return Ok(vec![]);
    }

    // Collect all dist-manifests and fetch the appropriate Mac ones
    let mut manifests = vec![];
    for file in manifest_dir.read_dir()? {
        let path = file?.path();
        if let Some(filename) = path.file_name() {
            if !filename.to_string_lossy().ends_with("dist-manifest.json") {
                continue;
            }
        }

        let json_path = Utf8PathBuf::try_from(path)?;
        let data = axoasset::SourceFile::load_local(json_path)?;
        let manifest: crate::DistManifest = data.deserialize_json()?;

        manifests.push(manifest);
    }
    Ok(manifests)
}

/// Save a manifest to the given path
pub fn save_manifest(manifest_path: &Utf8Path, manifest: &crate::DistManifest) -> DistResult<()> {
    let contents = serde_json::to_string_pretty(manifest).unwrap();
    axoasset::LocalAsset::write_new_all(&contents, manifest_path)?;
    Ok(())
}

/// Add release/artifact info to the current dist-manifest
pub(crate) fn add_releases_to_manifest(
    cfg: &Config,
    dist: &DistGraph,
    manifest: &mut DistManifest,
) -> DistResult<()> {
    for release in &dist.releases {
        // Gather up all the local and global artifacts
        for &artifact_idx in &release.global_artifacts {
            add_manifest_artifact(cfg, dist, manifest, release, artifact_idx);
        }
        for &variant_idx in &release.variants {
            let variant = dist.variant(variant_idx);
            for &artifact_idx in &variant.local_artifacts {
                add_manifest_artifact(cfg, dist, manifest, release, artifact_idx);
            }
        }
        let out_release =
            manifest.ensure_release(release.app_name.clone(), release.version.to_string());
        out_release.display = release.display;
        out_release.display_name.clone_from(&release.display_name);
    }

    Ok(())
}

fn add_manifest_artifact(
    cfg: &Config,
    dist: &DistGraph,
    manifest: &mut DistManifest,
    release: &Release,
    artifact_idx: ArtifactIdx,
) {
    let artifact = dist.artifact(artifact_idx);
    let mut assets = vec![];

    let built_assets = artifact
        .required_binaries
        .iter()
        .map(|(&binary_idx, exe_path)| {
            let binary = &dist.binary(binary_idx);
            let symbols_artifact = binary.symbols_artifact.map(|a| dist.artifact(a).id.clone());
            let kind = match binary.kind {
                crate::BinaryKind::DynamicLibrary => {
                    AssetKind::CDynamicLibrary(DynamicLibraryAsset { symbols_artifact })
                }
                crate::BinaryKind::StaticLibrary => {
                    AssetKind::CStaticLibrary(StaticLibraryAsset { symbols_artifact })
                }
                crate::BinaryKind::Executable => {
                    AssetKind::Executable(ExecutableAsset { symbols_artifact })
                }
            };
            Asset {
                id: Some(binary.id.clone()),
                name: Some(binary.name.clone()),
                // Always copied to the root... for now
                path: Some(exe_path.file_name().unwrap().to_owned()),
                kind,
            }
        });

    let mut static_assets = artifact
        .archive
        .as_ref()
        .map(|archive| {
            archive
                .static_assets
                .iter()
                .map(|(kind, asset)| {
                    let kind = match kind {
                        StaticAssetKind::Changelog => AssetKind::Changelog,
                        StaticAssetKind::License => AssetKind::License,
                        StaticAssetKind::Readme => AssetKind::Readme,
                        StaticAssetKind::Other => AssetKind::Unknown,
                    };
                    Asset {
                        id: None,
                        name: Some(asset.file_name().unwrap().to_owned()),
                        path: Some(asset.file_name().unwrap().to_owned()),
                        kind,
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Record the files that we always add to an npm package
    //
    // These can't be pre-included in the normal static assets list above because
    // they're generated from templates, and not copied from the user's project.
    if let ArtifactKind::Installer(InstallerImpl::Npm(..)) = &artifact.kind {
        let root_dir = dist
            .templates
            .get_template_dir(TEMPLATE_INSTALLER_NPM)
            .expect("npm template missing!?");
        let mut queue = vec![root_dir];
        while let Some(dir) = queue.pop() {
            for entry in dir.entries.values() {
                match entry {
                    TemplateEntry::Dir(dir) => {
                        queue.push(dir);
                    }
                    TemplateEntry::File(file) => {
                        static_assets.push(Asset {
                            id: None,
                            name: Some(file.name.clone()),
                            path: Some(file.path_from_ancestor(root_dir).to_string()),
                            kind: AssetKind::Unknown,
                        });
                    }
                }
            }
        }
    }

    assets.extend(built_assets);
    assets.extend(static_assets);
    // Sort the assets by name to make things extra stable
    assets.sort_by(|k1, k2| k1.name.cmp(&k2.name));

    let install_hint;
    let description;
    let kind;

    match &artifact.kind {
        ArtifactKind::ExecutableZip(_) => {
            install_hint = None;
            description = None;
            kind = cargo_dist_schema::ArtifactKind::ExecutableZip;
        }
        ArtifactKind::Symbols(_) => {
            install_hint = None;
            description = None;
            kind = cargo_dist_schema::ArtifactKind::Symbols;
        }
        ArtifactKind::Installer(
            InstallerImpl::Powershell(info)
            | InstallerImpl::Shell(info)
            | InstallerImpl::Homebrew(HomebrewInstallerInfo { inner: info, .. })
            | InstallerImpl::Npm(NpmInstallerInfo { inner: info, .. }),
        ) => {
            install_hint = Some(info.hint.clone());
            description = Some(info.desc.clone());
            kind = cargo_dist_schema::ArtifactKind::Installer;
        }
        ArtifactKind::Installer(InstallerImpl::Msi(..)) => {
            install_hint = None;
            description = Some("install via msi".to_owned());
            kind = cargo_dist_schema::ArtifactKind::Installer;
        }
        ArtifactKind::Installer(InstallerImpl::Pkg(..)) => {
            install_hint = None;
            description = Some("install via pkg".to_owned());
            kind = cargo_dist_schema::ArtifactKind::Installer;
        }
        ArtifactKind::Checksum(_) => {
            install_hint = None;
            description = None;
            kind = cargo_dist_schema::ArtifactKind::Checksum;
        }
        ArtifactKind::SourceTarball(_) => {
            install_hint = None;
            description = None;
            kind = cargo_dist_schema::ArtifactKind::SourceTarball;
        }
        ArtifactKind::ExtraArtifact(_) => {
            install_hint = None;
            description = None;
            kind = cargo_dist_schema::ArtifactKind::ExtraArtifact;
        }
        ArtifactKind::Updater(_) => {
            install_hint = None;
            description = None;
            kind = cargo_dist_schema::ArtifactKind::Updater;
        }
    };

    let checksum = artifact.checksum.map(|idx| dist.artifact(idx).id.clone());

    let out_artifact = cargo_dist_schema::Artifact {
        name: Some(artifact.id.clone()),
        path: if cfg.no_local_paths {
            None
        } else {
            Some(artifact.file_path.to_string())
        },
        target_triples: artifact.target_triples.clone(),
        install_hint,
        description,
        assets,
        kind,
        checksum,
        checksums: Default::default(),
    };

    if !cfg.no_local_paths {
        manifest.upload_files.push(artifact.file_path.to_string());
    }
    merge_artifact(manifest, artifact.id.clone(), out_artifact);

    // If the input has a list of artifacts for this release, merge them
    let out_release =
        manifest.ensure_release(release.app_name.clone(), release.version.to_string());
    if !out_release.artifacts.contains(&artifact.id) {
        out_release.artifacts.push(artifact.id.clone());
    }
}
