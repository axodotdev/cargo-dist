//! Computing the Announcement
//!
//! This is both "selection of what we're announcing via the tag" and "changelog stuff"

use axoproject::platforms::triple_to_display_name;
use axoproject::PackageIdx;
use axotag::{parse_tag, Package, PartialAnnouncementTag, ReleaseType};
use itertools::Itertools;
use semver::Version;
use tracing::{info, warn};

use crate::{
    backend::installer::{homebrew::HomebrewInstallerInfo, npm::NpmInstallerInfo, InstallerImpl},
    config::CiStyle,
    errors::{DistError, DistResult},
    ArtifactKind, DistGraphBuilder, SortedMap,
};

/// details on what we're announcing
pub(crate) struct AnnouncementTag {
    /// The full tag
    pub tag: String,
    /// The version we're announcing (if doing a unified version announcement)
    pub version: Option<Version>,
    /// The package we're announcing (if doing a single-package announcement)
    pub package: Option<PackageIdx>,
    /// whether we're prereleasing
    pub prerelease: bool,
    /// Which packages+bins we're announcing
    pub rust_releases: Vec<(PackageIdx, Vec<String>)>,
}

impl<'a> DistGraphBuilder<'a> {
    pub(crate) fn compute_announcement_info(&mut self, announcing: &AnnouncementTag) {
        // Default to using the tag as a title
        self.manifest.announcement_title = Some(announcing.tag.clone());
        self.manifest.announcement_tag = Some(announcing.tag.clone());
        self.manifest.announcement_is_prerelease = announcing.prerelease;

        // Refine the answers
        self.compute_announcement_changelog(announcing);
        self.compute_announcement_github();
    }

    /// Try to compute changelogs for the announcement
    pub fn compute_announcement_changelog(&mut self, announcing: &AnnouncementTag) {
        let info = if let Some(announcing_version) = &announcing.version {
            // Try to find the version we're announcing in the top level CHANGELOG/RELEASES
            let version = axoproject::Version::Cargo(announcing_version.clone());
            let Ok(Some(info)) = self.workspace.changelog_for_version(&version) else {
                info!(
                    "failed to find {version} in workspace changelogs, skipping changelog generation"
                );
                return;
            };

            info
        } else if let Some(announcing_package) = announcing.package {
            // Try to find the package's specific CHANGELOG/RELEASES
            let package = self.workspace.package(announcing_package);
            let package_name = &package.name;
            let version = package
                .version
                .as_ref()
                .expect("cargo package without a version!?");
            let Ok(Some(info)) = self
                .workspace
                .package(announcing_package)
                .changelog_for_version(version)
            else {
                info!(
                    "failed to find {version} in {package_name} changelogs, skipping changelog generation"
                );
                return;
            };

            info
        } else {
            unreachable!("you're neither announcing a version or a package!?");
        };

        info!("successfully parsed changelog!");
        self.manifest.announcement_title = Some(info.title);
        // Those windows newlines get everywhere...
        let clean_notes = newline_converter::dos2unix(&info.body);
        self.manifest.announcement_changelog = Some(clean_notes.into_owned());
    }

    /// If we're publishing to Github, generate some Github notes
    fn compute_announcement_github(&mut self) {
        use std::fmt::Write;

        if !self.inner.ci_style.contains(&CiStyle::Github) {
            info!("not publishing to Github, skipping Github Release Notes");
            return;
        }

        let mut gh_body = String::new();

        // add release notes
        if let Some(changelog) = self.manifest.announcement_changelog.as_ref() {
            gh_body.push_str("## Release Notes\n\n");
            gh_body.push_str(changelog);
            gh_body.push_str("\n\n");
        }

        // Add the contents of each Release to the body
        for release in &self.inner.releases {
            let heading_suffix = format!("{} {}", release.app_name, release.version);

            // Delineate releases if there's more than 1
            if self.inner.releases.len() > 1 {
                writeln!(gh_body, "# {heading_suffix}\n").unwrap();
            }

            // Sort out all the artifacts in this Release
            let mut global_installers = vec![];
            let mut local_installers = vec![];
            let mut bundles = vec![];
            let mut symbols = vec![];

            for &artifact_idx in &release.global_artifacts {
                let artifact = self.artifact(artifact_idx);
                match &artifact.kind {
                    ArtifactKind::ExecutableZip(zip) => bundles.push((artifact, zip)),
                    ArtifactKind::Symbols(syms) => symbols.push((artifact, syms)),
                    ArtifactKind::Checksum(_) => {}
                    ArtifactKind::Installer(installer) => {
                        global_installers.push((artifact, installer))
                    }
                }
            }

            for &variant_idx in &release.variants {
                let variant = self.variant(variant_idx);
                for &artifact_idx in &variant.local_artifacts {
                    let artifact = self.artifact(artifact_idx);
                    match &artifact.kind {
                        ArtifactKind::ExecutableZip(zip) => bundles.push((artifact, zip)),
                        ArtifactKind::Symbols(syms) => symbols.push((artifact, syms)),
                        ArtifactKind::Checksum(_) => {}
                        ArtifactKind::Installer(installer) => {
                            local_installers.push((artifact, installer))
                        }
                    }
                }
            }

            if !global_installers.is_empty() {
                writeln!(gh_body, "## Install {heading_suffix}\n").unwrap();
                for (_installer, details) in global_installers {
                    let info = match details {
                        InstallerImpl::Shell(info)
                        | InstallerImpl::Homebrew(HomebrewInstallerInfo { inner: info, .. })
                        | InstallerImpl::Powershell(info)
                        | InstallerImpl::Npm(NpmInstallerInfo { inner: info, .. }) => info,
                        InstallerImpl::Msi(_) => {
                            // Should be unreachable, but let's not crash over it
                            continue;
                        }
                    };
                    writeln!(&mut gh_body, "### {}\n", info.desc).unwrap();
                    writeln!(&mut gh_body, "```sh\n{}\n```\n", info.hint).unwrap();
                }
            }

            let other_artifacts: Vec<_> = bundles
                .iter()
                .map(|i| i.0)
                .chain(local_installers.iter().map(|i| i.0))
                .chain(symbols.iter().map(|i| i.0))
                .collect();

            let download_url = self
                .manifest
                .release_by_name(&release.app_name)
                .and_then(|r| r.artifact_download_url());
            if !other_artifacts.is_empty() && download_url.is_some() {
                let download_url = download_url.as_ref().unwrap();
                writeln!(gh_body, "## Download {heading_suffix}\n",).unwrap();
                gh_body.push_str("|  File  | Platform | Checksum |\n");
                gh_body.push_str("|--------|----------|----------|\n");

                for artifact in other_artifacts {
                    let mut targets = String::new();
                    let mut multi_target = false;
                    for target in &artifact.target_triples {
                        if multi_target {
                            targets.push_str(", ");
                        }
                        targets.push_str(target);
                        multi_target = true;
                    }
                    let name = &artifact.id;
                    let artifact_download_url = format!("{download_url}/{name}");
                    let download = format!("[{name}]({artifact_download_url})");
                    let checksum = if let Some(checksum_idx) = artifact.checksum {
                        let checksum_name = &self.artifact(checksum_idx).id;
                        let checksum_download_url = format!("{download_url}/{checksum_name}");
                        format!("[checksum]({checksum_download_url})")
                    } else {
                        String::new()
                    };
                    let mut triple = artifact
                        .target_triples
                        .iter()
                        .filter_map(|t| triple_to_display_name(t))
                        .join(", ");
                    if triple.is_empty() {
                        triple = "Unknown".to_string();
                    }
                    writeln!(&mut gh_body, "| {download} | {triple} | {checksum} |").unwrap();
                }
                writeln!(&mut gh_body).unwrap();
            }
        }

        info!("successfully generated github release body!");
        // self.inner.artifact_download_url = Some(download_url);
        self.manifest.announcement_github_body = Some(gh_body);
    }
}

/// See if we should dist this package.
///
/// Some(disabled_reason) is returned if it shouldn't be.
///
/// This code is written to assume a package and its binaries should be distable,
/// and then runs through a battery of disqualifying reasons.
///
/// A notable consequence of this is that if --tag wasn't passed, then we will default to
/// letting through all the packages that aren't intrinsically disqualified by things like
/// publish=false. Later steps will then check if a coherent announcement tag exists that
/// covers everything this function spat out.
fn check_dist_package(
    graph: &DistGraphBuilder,
    pkg_id: PackageIdx,
    pkg: &axoproject::PackageInfo,
    announcing: &PartialAnnouncementTag,
) -> Option<String> {
    // Nothing to publish if there's no binaries!
    if pkg.binaries.is_empty() {
        return Some("no binaries".to_owned());
    }

    // If [metadata.dist].dist is explicitly set, respect it!
    let override_publish = if let Some(do_dist) = graph.package_metadata(pkg_id).dist {
        if !do_dist {
            return Some("dist = false".to_owned());
        } else {
            true
        }
    } else {
        false
    };

    // Otherwise defer to Cargo's `publish = false`
    if !pkg.publish && !override_publish {
        return Some("publish = false".to_owned());
    }

    // If we're announcing a package, reject every other package
    match &announcing.release {
        ReleaseType::Package(id) => {
            if pkg_id != PackageIdx(*id) {
                return Some(format!("didn't match tag {}", announcing.tag));
            }
        }
        ReleaseType::Version(ver) => {
            if pkg.version.as_ref().unwrap().semver() != ver {
                return Some(format!("didn't match tag {}", announcing.tag));
            }
        }
        ReleaseType::None => {}
    }

    // If it passes the guantlet, dist it
    None
}

/// Parse the announcement tag and determine what we're announcing
///
/// `tag` being None here is equivalent to `--tag` not being passed, and tells us to infer
/// the tag based on things like "every package has the same version, assume we're
/// announcing that version".
///
/// `needs_coherent_announcement_tag = false` tells us to produce a result even if inference
/// fails to find a tag that will unambiguously work. This is used by commands like `init`
/// and `generate` which want to consider "everything" even if the user never actually
/// could announce everything at once. In this case dummy values will appear in every field
/// except for `AnnouncementTag::rust_releases` which will contain every distable binary
/// in the workspace.
pub(crate) fn select_tag(
    graph: &DistGraphBuilder,
    tag: Option<&str>,
    needs_coherent_announcement_tag: bool,
) -> DistResult<AnnouncementTag> {
    let packages: Vec<Package> = graph
        .workspace()
        .packages()
        .map(|(_, info)| Package {
            name: info.name.clone(),
            version: info.version.clone().map(|v| v.semver().clone()),
        })
        .collect();

    // Parse the tag
    let mut announcing = if let Some(tag) = tag {
        parse_tag(&packages, tag)?
    } else {
        PartialAnnouncementTag::default()
    };
    // Select which packages/binaries are available from that tag
    let rust_releases = select_packages(graph, &announcing);

    // Don't proceed if the conclusions don't make sense
    if rust_releases.is_empty() {
        // It's ok for there to be no selected binaries if the user explicitly requested an
        // announcement for a library with `--tag=my-lib-1.0.0`
        if matches!(announcing.release, ReleaseType::Package(_)) {
            warn!("You're trying to explicitly Release a library, only minimal functionality will work");
        } else {
            // No binaries were selected, and they weren't trying to announce a library,
            // we've gotta bail out, this is too weird.
            //
            // To get better help messages, we explore a hypothetical world where they didn't pass
            // `--tag` so we can get all the options for a good help message.
            let announcing = PartialAnnouncementTag::default();
            let rust_releases = select_packages(graph, &announcing);
            let versions = possible_tags(graph, rust_releases.iter().map(|(idx, _)| *idx));
            let help = tag_help(graph, versions, "You may need to pass the current version as --tag, or need to give all your packages the same version");
            return Err(DistError::NothingToRelease { help });
        }
    }

    // If we don't have a tag yet we MUST successfully select one here or fail
    if matches!(announcing.release, ReleaseType::None) {
        // Group distable packages by version, if there's only one then use that as the tag
        let versions = possible_tags(graph, rust_releases.iter().map(|(idx, _)| *idx));
        if versions.len() == 1 {
            // Nice, one version, use it
            let version = *versions.first_key_value().unwrap().0;
            let tag = format!("v{version}");
            info!("inferred Announcement tag: {}", tag);
            announcing.tag = tag;
            announcing.prerelease = !version.pre.is_empty();
            announcing.release = ReleaseType::Version(version.clone());
        } else if needs_coherent_announcement_tag {
            // More than one version, give the user some suggestions
            let help = tag_help(
                graph,
                versions,
                "Please either specify --tag, or give them all the same version",
            );
            return Err(DistError::TooManyUnrelatedApps { help });
        } else {
            // Ok we got more than one version but we're being run by a command
            // like `init` or `generate` which just wants us to hand it everything
            // and doesn't care about coherent announcements. So use a fake tag
            // and hand it the fully unconstrained list of rust_releases.
            announcing.tag = "v1.0.0-FAKEVER".to_owned();
            announcing.prerelease = true;
            announcing.release = ReleaseType::Version("1.0.0-FAKEVER".parse().unwrap());
        }
    }

    let mut version = None;
    let mut package = None;
    match &announcing.release {
        ReleaseType::Package(id) => package = Some(PackageIdx(*id)),
        ReleaseType::Version(ver) => version = Some(ver.clone()),
        ReleaseType::None => {}
    }

    Ok(AnnouncementTag {
        tag: announcing.tag,
        version,
        package,
        prerelease: announcing.prerelease,
        rust_releases,
    })
}

/// Select which packages/binaries the announcement includes and print info about the process
///
/// See `check_dist_package` for the actual selection logic and some notes on inference
/// when `--tag` is absent.
fn select_packages(
    graph: &DistGraphBuilder,
    announcing: &PartialAnnouncementTag,
) -> Vec<(PackageIdx, Vec<String>)> {
    info!("");
    info!("selecting packages from workspace: ");
    // Choose which binaries we want to release
    let disabled_sty = console::Style::new().dim();
    let enabled_sty = console::Style::new();
    let mut rust_releases = vec![];
    for (pkg_id, pkg) in graph.workspace().packages() {
        let pkg_name = &pkg.name;

        // Determine if this package's binaries should be Released
        let disabled_reason = check_dist_package(graph, pkg_id, pkg, announcing);

        // Report our conclusion/discoveries
        let sty;
        if let Some(reason) = &disabled_reason {
            sty = &disabled_sty;
            info!("  {}", sty.apply_to(format!("{pkg_name} ({reason})")));
        } else {
            sty = &enabled_sty;
            info!("  {}", sty.apply_to(pkg_name));
        }

        // Report each binary and potentially add it to the Release for this package
        let mut rust_binaries = vec![];
        for binary in &pkg.binaries {
            info!("    {}", sty.apply_to(format!("[bin] {}", binary)));
            // In the future might want to allow this to be granular for each binary
            if disabled_reason.is_none() {
                rust_binaries.push(binary.to_owned());
            }
        }

        // If any binaries were accepted for this package, it's a Release!
        if !rust_binaries.is_empty() {
            rust_releases.push((pkg_id, rust_binaries));
        }
    }
    info!("");

    rust_releases
}

/// Get a list of possible version --tags to use, given a list of packages we want to Announce
///
/// This is the set of options used by tag inference. Inference succeeds if
/// there's only one key in the output.
fn possible_tags<'a>(
    graph: &'a DistGraphBuilder,
    rust_releases: impl IntoIterator<Item = PackageIdx>,
) -> SortedMap<&'a Version, Vec<PackageIdx>> {
    let mut versions = SortedMap::<&Version, Vec<PackageIdx>>::new();
    for pkg_idx in rust_releases {
        let info = graph.workspace().package(pkg_idx);
        let version = info.version.as_ref().unwrap().semver();
        versions.entry(version).or_default().push(pkg_idx);
    }
    versions
}

/// Get a help printout for what --tags could have been passed
fn tag_help(
    graph: &DistGraphBuilder,
    versions: SortedMap<&Version, Vec<PackageIdx>>,
    base_suggestion: &str,
) -> String {
    use std::fmt::Write;
    let mut help = String::new();

    let Some(some_pkg) = versions
        .first_key_value()
        .and_then(|(_, packages)| packages.first())
    else {
        return r#"It appears that you have no packages in your workspace with distable binaries. You can rerun with "--verbose=info" to see what cargo-dist thinks is in your workspace. Here are some typical issues:

    If you're trying to use cargo-dist to announce libraries, we require you explicitly select the library with e.g. "--tag=my-library-v1.0.0", as this mode is experimental.

    If you have binaries in your workspace, `publish = false` could be hiding them and adding "dist = true" to [package.metadata.dist] in your Cargo.toml may help."#.to_owned();
    };

    help.push_str(base_suggestion);
    help.push_str("\n\n");
    help.push_str("Here are some options:\n\n");
    for (version, packages) in &versions {
        write!(help, "--tag=v{version} will Announce: ").unwrap();
        let mut multi_package = false;
        for &pkg_id in packages {
            let info = &graph.workspace().package(pkg_id);
            if multi_package {
                write!(help, ", ").unwrap();
            } else {
                multi_package = true;
            }
            write!(help, "{}", info.name).unwrap();
        }
        writeln!(help).unwrap();
    }
    help.push('\n');
    let info = &graph.workspace().package(*some_pkg);
    let some_tag = format!(
        "--tag={}-v{}",
        info.name,
        info.version.as_ref().unwrap().semver()
    );

    writeln!(
        help,
        "you can also request any single package with {some_tag}"
    )
    .unwrap();

    help
}
