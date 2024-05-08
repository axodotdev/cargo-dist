//! Computing the Announcement
//!
//! This is both "selection of what we're announcing via the tag" and "changelog stuff"

use axoproject::platforms::triple_to_display_name;
use axoproject::PackageIdx;
use axotag::{parse_tag, Package, PartialAnnouncementTag, ReleaseType};
use cargo_dist_schema::DistManifest;
use itertools::Itertools;
use semver::Version;
use tracing::info;

use crate::{
    errors::{DistError, DistResult},
    DistGraphBuilder, SortedMap, TargetTriple,
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

type ReleasesAndBins = Vec<(PackageIdx, Vec<String>)>;

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
        announcement_github(&mut self.manifest);
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
        ReleaseType::Package { idx, version: _ } => {
            if pkg_id != PackageIdx(*idx) {
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
    graph: &mut DistGraphBuilder,
    tag: Option<&str>,
    needs_coherent_announcement_tag: bool,
) -> DistResult<AnnouncementTag> {
    let mut announcing = if let Some(tag) = tag {
        // If we're given a specific real tag to use, ask axotag to parse it
        // and identify which packages are selected by it.
        let packages: Vec<Package> = graph
            .workspace()
            .packages()
            .map(|(_, info)| Package {
                name: info.name.clone(),
                version: info.version.clone().map(|v| v.semver().clone()),
            })
            .collect();

        parse_tag(&packages, tag)?
    } else {
        // Otherwise, start with all packages
        PartialAnnouncementTag::default()
    };

    // Further filter down the list of packages based on whether they're "distable",
    // and do some debug printouts of the conclusions
    let releases = select_packages(graph, &announcing);

    // Don't proceed if we failed to select any packages
    require_releases(graph, &releases)?;

    // If we still need to compute a tag, do so now
    ensure_tag(
        graph,
        &releases,
        &mut announcing,
        needs_coherent_announcement_tag,
    )?;

    // Make sure axotag agrees with what we did
    require_axotag_consistency(
        graph,
        &announcing,
        &releases,
        needs_coherent_announcement_tag,
    )?;

    // Ok, we're done, return the result
    let mut version = None;
    let mut package = None;
    match &announcing.release {
        ReleaseType::Package { idx, version: _ } => package = Some(PackageIdx(*idx)),
        ReleaseType::Version(ver) => version = Some(ver.clone()),
        ReleaseType::None => {
            unreachable!("internal dist error: failed to ensure a release tag")
        }
    }

    Ok(AnnouncementTag {
        tag: announcing.tag,
        version,
        package,
        prerelease: announcing.prerelease,
        rust_releases: releases,
    })
}

// Do an internal integrity check that axotag still agrees
fn require_axotag_consistency(
    graph: &mut DistGraphBuilder,
    announcing: &PartialAnnouncementTag,
    releases: &ReleasesAndBins,
    needs_coherent_announcement_tag: bool,
) -> DistResult<()> {
    if !needs_coherent_announcement_tag {
        // Don't care
        return Ok(());
    }

    let expected = announcing;
    let packages = releases
        .iter()
        .map(|(p, _)| {
            let info = graph.workspace().package(*p);
            Package {
                name: info.name.clone(),
                version: info.version.clone().map(|v| v.semver().clone()),
            }
        })
        .collect::<Vec<_>>();
    let computed = parse_tag(&packages, &announcing.tag)?;

    match (&computed.release, &expected.release) {
        (ReleaseType::Version(computed), ReleaseType::Version(expected)) => {
            assert_eq!(
                computed, expected,
                "internal dist error: axotag parsed a different version from tag"
            );
        }
        (
            ReleaseType::Package {
                idx: _,
                version: computed_version,
            },
            ReleaseType::Package {
                idx: _,
                version: expected_version,
            },
        ) => {
            // FIXME: compare indices (use original package list to make them comparable?)
            assert_eq!(
                computed_version, expected_version,
                "internal dist error: axotag parsed a different version from tag"
            );
        }
        (ReleaseType::None, _) | (_, ReleaseType::None) => {
            unreachable!("internal dist error: failed to ensure a release tag")
        }
        _ => {
            unreachable!("internal dist error: axotag parsed tag as different class of tag");
        }
    }
    assert_eq!(
        computed.tag, expected.tag,
        "internal dist error: axotag parsed a different version from tag"
    );
    assert_eq!(
        computed.prerelease, expected.prerelease,
        "internal dist error: axotag disagreed on prerelease status"
    );
    Ok(())
}

/// Select which packages/binaries the announcement includes and print info about the process
///
/// See `check_dist_package` for the actual selection logic and some notes on inference
/// when `--tag` is absent.
fn select_packages(
    graph: &DistGraphBuilder,
    announcing: &PartialAnnouncementTag,
) -> ReleasesAndBins {
    info!("");
    info!("selecting packages from workspace: ");
    // Choose which binaries we want to release
    let disabled_sty = console::Style::new().dim();
    let enabled_sty = console::Style::new();
    let mut releases = vec![];
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
        let mut binaries = vec![];
        for binary in &pkg.binaries {
            info!("    {}", sty.apply_to(format!("[bin] {}", binary)));
            // In the future might want to allow this to be granular for each binary
            if disabled_reason.is_none() {
                binaries.push(binary.to_owned());
            }
        }

        // If any binaries were accepted for this package, it's a Release!
        if !binaries.is_empty() {
            releases.push((pkg_id, binaries));
        }
    }
    info!("");

    // If no binaries were selected but we are trying to specifically release One Package,
    // add that package as a release still, on the assumption it's a Library
    if releases.is_empty() {
        if let ReleaseType::Package { idx, version: _ } = announcing.release {
            releases.push((PackageIdx(idx), vec![]));
        }
    }

    releases
}

/// Require at least one release, otherwise provide helpful info
fn require_releases(graph: &DistGraphBuilder, releases: &ReleasesAndBins) -> DistResult<()> {
    if !releases.is_empty() {
        return Ok(());
    }

    // No binaries were selected, and they weren't trying to announce a library,
    // we've gotta bail out, this is too weird.
    //
    // To get better help messages, we explore a hypothetical world where they didn't pass
    // `--tag` so we can get all the options for a good help message.
    let announcing = PartialAnnouncementTag::default();
    let rust_releases = select_packages(graph, &announcing);
    let versions = possible_tags(graph, rust_releases.iter().map(|(idx, _)| *idx));
    let help = tag_help(graph, versions, "You may need to pass the current version as --tag, or need to give all your packages the same version");
    Err(DistError::NothingToRelease { help })
}

/// If we don't have a tag yet we MUST successfully select one here or fail
fn ensure_tag(
    graph: &mut DistGraphBuilder,
    releases: &ReleasesAndBins,
    announcing: &mut PartialAnnouncementTag,
    needs_coherent_announcement_tag: bool,
) -> DistResult<()> {
    // This extra logic only applies if we didn't already have a tag,
    // which would have set ReleaseType
    if !matches!(announcing.release, ReleaseType::None) {
        return Ok(());
    }

    // Group distable packages by version, if there's only one then use that as the tag
    let versions = possible_tags(graph, releases.iter().map(|(idx, _)| *idx));
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
    Ok(())
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

/// If we're publishing to Axodotdev, generate the announcement body
pub fn announcement_axodotdev(manifest: &DistManifest) -> String {
    // Create a merged announcement body to send, announcement_title should always be set at this point
    let title = manifest.announcement_title.clone().unwrap_or_default();
    let body = manifest.announcement_changelog.clone().unwrap_or_default();
    format!("# {title}\n\n{body}")
}

/// If we're publishing to Github, generate the announcement body
///
/// Currently mutates the manifest, in the future it should output it
pub fn announcement_github(manifest: &mut DistManifest) {
    use std::fmt::Write;

    let mut gh_body = String::new();

    // add release notes
    if let Some(changelog) = manifest.announcement_changelog.as_ref() {
        gh_body.push_str("## Release Notes\n\n");
        gh_body.push_str(changelog);
        gh_body.push_str("\n\n");
    }

    // Add the contents of each Release to the body
    let mut announcing_github = false;
    for release in &manifest.releases {
        // Only bother if there's actually github hosting
        if release.hosting.github.is_none() {
            continue;
        }
        announcing_github = true;

        let heading_suffix = format!("{} {}", release.app_name, release.app_version);

        // Delineate releases if there's more than 1
        if manifest.releases.len() > 1 {
            writeln!(gh_body, "# {heading_suffix}\n").unwrap();
        }

        // Sort out all the artifacts in this Release
        let mut global_installers = vec![];
        let mut local_installers = vec![];
        let mut bundles = vec![];
        let mut symbols = vec![];

        for (_name, artifact) in manifest.artifacts_for_release(release) {
            match artifact.kind {
                cargo_dist_schema::ArtifactKind::ExecutableZip => bundles.push(artifact),
                cargo_dist_schema::ArtifactKind::Symbols => symbols.push(artifact),
                cargo_dist_schema::ArtifactKind::Installer => {
                    if let (Some(desc), Some(hint)) =
                        (&artifact.description, &artifact.install_hint)
                    {
                        global_installers.push((desc, hint));
                    } else {
                        local_installers.push(artifact);
                    }
                }
                cargo_dist_schema::ArtifactKind::Checksum => {
                    // Do Nothing (will be included with the artifact it checksums)
                }
                cargo_dist_schema::ArtifactKind::Unknown => {
                    // Do nothing
                }
                _ => {
                    // Do nothing
                }
            }
        }

        if !global_installers.is_empty() {
            writeln!(gh_body, "## Install {heading_suffix}\n").unwrap();
            for (desc, hint) in global_installers {
                writeln!(&mut gh_body, "### {}\n", desc).unwrap();
                writeln!(&mut gh_body, "```sh\n{}\n```\n", hint).unwrap();
            }
        }

        let mut other_artifacts: Vec<_> = bundles
            .into_iter()
            .chain(local_installers)
            .chain(symbols)
            .collect();

        other_artifacts.sort_by_cached_key(|a| sortable_triples(&a.target_triples));

        let download_url = release.artifact_download_url();
        if !other_artifacts.is_empty() && download_url.is_some() {
            let download_url = download_url.as_ref().unwrap();
            writeln!(gh_body, "## Download {heading_suffix}\n",).unwrap();
            gh_body.push_str("|  File  | Platform | Checksum |\n");
            gh_body.push_str("|--------|----------|----------|\n");

            for artifact in other_artifacts {
                // Artifacts with no name do not exist as files, and should have had install-hints
                let Some(name) = &artifact.name else {
                    continue;
                };

                let mut targets = String::new();
                let mut multi_target = false;
                for target in &artifact.target_triples {
                    if multi_target {
                        targets.push_str(", ");
                    }
                    targets.push_str(target);
                    multi_target = true;
                }

                let artifact_download_url = format!("{download_url}/{name}");
                let download = format!("[{name}]({artifact_download_url})");
                let checksum = if let Some(checksum_name) = &artifact.checksum {
                    let checksum_download_url = format!("{download_url}/{checksum_name}");
                    format!("[checksum]({checksum_download_url})")
                } else {
                    String::new()
                };
                let mut triple = artifact
                    .target_triples
                    .iter()
                    .map(|t| triple_to_display_name(t).unwrap_or_else(|| t))
                    .join(", ");
                if triple.is_empty() {
                    triple = "Unknown".to_string();
                }
                writeln!(&mut gh_body, "| {download} | {triple} | {checksum} |").unwrap();
            }
            writeln!(&mut gh_body).unwrap();
        }
    }

    if announcing_github {
        info!("successfully generated github release body!");
        manifest.announcement_github_body = Some(gh_body);
    }
}

/// Create a key for Properly sorting a list of target triples
fn sortable_triples(triples: &[TargetTriple]) -> Vec<Vec<String>> {
    // Make each triple sortable, and then sort the list of triples by those
    // (usually there's only one triple but DETERMINISM)
    let mut output: Vec<Vec<String>> = triples.iter().map(sortable_triple).collect();
    output.sort();
    output
}

/// Create a key for Properly sorting target triples
fn sortable_triple(triple: &TargetTriple) -> Vec<String> {
    // We want to sort lexically by: os, abi, arch
    // We are given arch, vendor, os, abi
    //
    // vendor is essentially irrelevant / pairs with os,
    // ("unknown" as a vendor is basically "not windows or macos")
    // so a simple solution here is to just move arch to the end,
    // giving us a sort of: vendor, os, abi, arch.
    //
    // In particular doing sorting this way avoids worrying about
    // gunk like fuchsia omitting vendor sometimes, or the occasional
    // absence of abi.
    //
    // Notable inputs:
    //
    //  arch    vendor     os      abi
    // --------------------------------------
    // x86_64  -pc      -windows -msvc
    // aarch64 -apple   -darwin
    // aarch64 -unknown -linux   -musl
    // aarch64 -unknown -linux   -gnu
    // armv7   -unknown -linux   -gnueabihf
    // aarch64 -unknown -fuchsia
    // aarch64          -fuchsia
    let mut parts = triple.split('-');
    let arch = parts.next();
    let order = parts.chain(arch);
    order.map(|s| s.to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::sortable_triple;
    #[test]
    fn sort_platforms() {
        let mut targets = vec![
            "aarch64-unknown-linux-gnu",
            "x86_64-unknown-linux-gnu",
            "i686-unknown-linux-gnu",
            "aarch64-apple-darwin",
            "x86_64-apple-darwin",
            "aarch64-unknown-linux-musl",
            "x86_64-unknown-linux-musl",
            "i686-unknown-linux-musl",
            "aarch64-pc-windows-msvc",
            "x86_64-pc-windows-msvc",
            "i686-pc-windows-msvc",
            "armv7-unknown-linux-gnueabihf",
            "powerpc64-unknown-linux-gnu",
            "powerpc64le-unknown-linux-gnu",
            "s390x-unknown-linux-gnu",
            "aarch64-fuschsia",
            "x86_64-fuschsia",
            "universal2-apple-darwin",
            "x86_64-unknown-linux-gnu.2.31",
            "x86_64-unknown-linux-musl-static",
        ];
        targets.sort_by_cached_key(|t| sortable_triple(&t.to_string()));
        assert_eq!(
            targets,
            vec![
                "aarch64-apple-darwin",
                "universal2-apple-darwin",
                "x86_64-apple-darwin",
                "aarch64-fuschsia",
                "x86_64-fuschsia",
                "aarch64-pc-windows-msvc",
                "i686-pc-windows-msvc",
                "x86_64-pc-windows-msvc",
                "aarch64-unknown-linux-gnu",
                "i686-unknown-linux-gnu",
                "powerpc64-unknown-linux-gnu",
                "powerpc64le-unknown-linux-gnu",
                "s390x-unknown-linux-gnu",
                "x86_64-unknown-linux-gnu",
                "x86_64-unknown-linux-gnu.2.31",
                "armv7-unknown-linux-gnueabihf",
                "aarch64-unknown-linux-musl",
                "i686-unknown-linux-musl",
                "x86_64-unknown-linux-musl-static",
                "x86_64-unknown-linux-musl",
            ]
        );
    }
}
