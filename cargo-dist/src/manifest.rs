//! Utilities for managing DistManifests

use std::collections::BTreeMap;

use camino::{Utf8Path, Utf8PathBuf};
use cargo_dist_schema::{Asset, AssetKind, DistManifest, ExecutableAsset, Hosting};

use crate::{
    backend::{
        installer::{homebrew::HomebrewInstallerInfo, npm::NpmInstallerInfo, InstallerImpl},
        templates::{TemplateEntry, TEMPLATE_INSTALLER_NPM},
    },
    config::Config,
    errors::DistResult,
    ArtifactIdx, ArtifactKind, DistGraph, StaticAssetKind,
};

/// Load DistManifests into the given dir and merge them into the current one
pub fn load_and_merge_manifests(
    manifest_dir: &Utf8Path,
    output: &mut DistManifest,
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
            artifacts: _,
            releases,
            publish_prereleases,
            announcement_tag,
            announcement_is_prerelease,
            announcement_title,
            announcement_changelog,
            announcement_github_body,
            ci,
            linkage,
        } = manifest;

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
            // NOTE: *do not* merge artifact info, it's currently load-bearing for each machine
            // to only list the artifacts it specifically generates, so we don't want to merge
            // in artifacts from other machines (`cargo dist plan` should know them all for now).
        }

        if let Some(val) = announcement_tag {
            output.announcement_tag = Some(val);
            // Didn't wrap these in an option, so use announcement_tag as a proxy
            output.announcement_is_prerelease = announcement_is_prerelease;
            output.publish_prereleases = publish_prereleases;
        }
        if let Some(val) = announcement_title {
            output.announcement_title = Some(val);
        }
        if let Some(val) = announcement_changelog {
            output.announcement_changelog = Some(val);
        }
        if let Some(val) = announcement_github_body {
            output.announcement_github_body = Some(val);
        }
        if let Some(val) = ci {
            // Don't bother doing an inner merge here, all or nothing
            output.ci = Some(val);
        };

        // Just merge all the linkage
        output.linkage.extend(linkage);
    }

    Ok(())
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
    let mut all_artifacts = BTreeMap::<String, cargo_dist_schema::Artifact>::new();
    for release in &dist.releases {
        // Gather up all the local and global artifacts
        let mut artifacts = vec![];
        for &artifact_idx in &release.global_artifacts {
            let id = &dist.artifact(artifact_idx).id;
            all_artifacts.insert(id.clone(), manifest_artifact(cfg, dist, artifact_idx));
            artifacts.push(id.clone());
        }
        for &variant_idx in &release.variants {
            let variant = dist.variant(variant_idx);
            for &artifact_idx in &variant.local_artifacts {
                let id = &dist.artifact(artifact_idx).id;
                all_artifacts.insert(id.clone(), manifest_artifact(cfg, dist, artifact_idx));
                artifacts.push(id.clone());
            }
        }

        // Add the artifacts to this release
        manifest
            .ensure_release(release.app_name.clone(), release.version.to_string())
            .artifacts = artifacts;
    }
    manifest.artifacts = all_artifacts;

    Ok(())
}

fn manifest_artifact(
    cfg: &Config,
    dist: &DistGraph,
    artifact_idx: ArtifactIdx,
) -> cargo_dist_schema::Artifact {
    let artifact = dist.artifact(artifact_idx);
    let mut assets = vec![];

    let built_assets = artifact
        .required_binaries
        .iter()
        .map(|(&binary_idx, exe_path)| {
            let binary = &dist.binary(binary_idx);
            let symbols_artifact = binary.symbols_artifact.map(|a| dist.artifact(a).id.clone());
            Asset {
                name: Some(binary.name.clone()),
                // Always copied to the root... for now
                path: Some(exe_path.file_name().unwrap().to_owned()),
                kind: AssetKind::Executable(ExecutableAsset { symbols_artifact }),
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
        ArtifactKind::Installer(InstallerImpl::Docker(..)) => {
            install_hint = None;
            description = Some("try it out in docker".to_owned());
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
    };

    let checksum = artifact.checksum.map(|idx| dist.artifact(idx).id.clone());

    cargo_dist_schema::Artifact {
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
    }
}
