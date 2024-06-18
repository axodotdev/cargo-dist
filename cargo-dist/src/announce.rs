//! Computing the Announcement
//!
//! This is both "selection of what we're announcing via the tag" and "changelog stuff"

use axoproject::platforms::triple_to_display_name;
use axoproject::PackageIdx;
use axotag::{parse_tag, Package, PartialAnnouncementTag, ReleaseType};
use cargo_dist_schema::{DistManifest, GithubHosting};
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

/// Settings for `select_tag`
#[derive(Debug, Clone)]
pub struct TagSettings {
    /// Whether the tag and versions need to be coherent with each other.
    ///
    /// If false, `select_tag` is allowed to make up a fake tag/version
    /// that doesn't need to match the packages.
    ///
    /// This is allowed to be false for commands which are intended to
    /// just work on the full workspace regardless of whether it ever
    /// makes sense to actually announce the full thing at the same time.
    ///
    /// Notably commands like `init` and `generate` can set this false.
    pub needs_coherence: bool,
    /// How we're tagging the announcement
    pub tag: TagMode,
}

/// How we're tagging the announcement
#[derive(Debug, Clone)]
pub enum TagMode {
    /// No tag is provided, infer the result.
    ///
    /// If [`TagSettings::needs_coherence`][] is false, the inference
    /// can pick some garbage to just keep things moving along.
    Infer,
    /// The user gave us this tag, which should be parsed and used
    /// for selecting the packages we're announcing.
    Select(String),
    /// The user gave us this tag, and wants us to force all distable to conform to its version.
    ///
    /// Currently this means just mutating our own metadata on the versions, but in the future
    /// we could actually mutate the in-tree manifests so things like
    /// `my-app --version` report the given value.
    Force(String),
    /// The user just wants us to release whatever's in the tree, triggered on untagged pushes.
    ///
    /// This raises several ambiguities.
    ///
    /// First, the packages could have different versions,
    /// but we don't really support that being the case. Or at least, we need a version to
    /// pick for the tag, and things like axo Releases expect the tag's version to match
    /// all the releases in the announcement.
    ///
    /// Second, since we're triggering on every push it's essentially guaranteed that
    /// we'll be asked to publish a version that's already published.
    ///
    /// We avoid the first issue by selecting the maximum version among distable packages.
    /// We avoid the second issue by adding a timestamp buildid to the version, making
    /// every release with this flow a prerelease. We then use the same logic as
    /// `TagMode::Force` to rewrite the packages versions to match this value.
    ForceMaxAndTimestamp,
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
            let Ok(Some(info)) = self
                .workspaces
                .root_workspace()
                .changelog_for_version(&version)
            else {
                info!(
                    "failed to find {version} in workspace changelogs, skipping changelog generation"
                );
                return;
            };

            info
        } else if let Some(announcing_package) = announcing.package {
            // Try to find the package's specific CHANGELOG/RELEASES
            let package = self.workspaces.package(announcing_package);
            let package_name = &package.name;
            let version = package
                .version
                .as_ref()
                .expect("cargo package without a version!?");
            let Ok(Some(info)) = self
                .workspaces
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
    settings: &TagSettings,
) -> DistResult<AnnouncementTag> {
    let mut announcing = match &settings.tag {
        TagMode::Select(tag) => {
            // If we're given a selection tag, immediately parse it to use as a selector
            parse_tag_for_all_packages(graph, tag)?
        }
        TagMode::Infer | TagMode::ForceMaxAndTimestamp | TagMode::Force(_) => {
            // Otherwise, start with all packages
            PartialAnnouncementTag::default()
        }
    };

    // Further filter down the list of packages based on whether they're "distable",
    // and do some debug printouts of the conclusions
    let releases = select_packages(graph, &announcing);

    // Don't proceed if we failed to select any packages
    require_releases(graph, &releases)?;

    // If we still need to compute a tag, do so now
    ensure_tag(graph, &releases, &mut announcing, settings)?;

    // Make sure axotag agrees with what we did
    require_axotag_consistency(graph, &announcing, settings)?;

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

    // Ignoring whatever we calculated, mark it as stable if the
    // user asked us to.
    let prerelease = if graph.manifest.force_latest {
        false
    } else {
        announcing.prerelease
    };

    Ok(AnnouncementTag {
        tag: announcing.tag,
        version,
        package,
        prerelease,
        rust_releases: releases,
    })
}

// Do an internal integrity check that axotag still agrees
fn require_axotag_consistency(
    graph: &mut DistGraphBuilder,
    announcing: &PartialAnnouncementTag,
    settings: &TagSettings,
) -> DistResult<()> {
    if !settings.needs_coherence {
        // Don't care
        return Ok(());
    }

    let expected = announcing;
    let computed = parse_tag_for_all_packages(graph, &announcing.tag)?;

    match (&computed.release, &expected.release) {
        (ReleaseType::Version(computed), ReleaseType::Version(expected)) => {
            assert_eq!(
                computed, expected,
                "internal dist error: axotag parsed a different version from tag"
            );
        }
        (
            ReleaseType::Package {
                version: computed_ver,
                ..
            },
            ReleaseType::Package {
                version: expected_ver,
                ..
            },
        ) => {
            // FIXME: compare indices (use original package list to make them comparable?)
            assert_eq!(
                computed_ver, expected_ver,
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
    for (pkg_id, pkg) in graph.workspaces.all_packages() {
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
    settings: &TagSettings,
) -> DistResult<()> {
    // This extra logic only applies if we didn't already have a tag,
    // which would have set ReleaseType
    if !matches!(announcing.release, ReleaseType::None) {
        return Ok(());
    }

    match &settings.tag {
        TagMode::Select(_) => {
            unreachable!("internal dist error: tag selection should have picked a tag");
        }
        TagMode::Infer => {
            // Group distable packages by version, if there's only one then use that as the tag
            let versions = possible_tags(graph, releases.iter().map(|(idx, _)| *idx));
            if versions.len() == 1 {
                // Nice, one version, use it
                let version = *versions.first_key_value().unwrap().0;
                let tag = format!("v{version}");
                info!("inferred Announcement tag: {}", tag);
                *announcing = parse_tag_for_all_packages(graph, &tag)?;
            } else if settings.needs_coherence {
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
                //
                // Note that we (currently) intentionally don't use overwrite_package_versions,
                // as this mode is intended to be for things like integrity checks
                "v1.0.0-FAKEVER".clone_into(&mut announcing.tag);
                announcing.prerelease = true;
                announcing.release = ReleaseType::Version("1.0.0-FAKEVER".parse().unwrap());
            }
        }
        TagMode::Force(tag) => {
            // We've been given a tag (presumably from a previous plan step)
            // to force all distable packages to conform to, mutating their versions to match.
            //
            // First, ask axotag to parse the tag for us. It doesn't matter that the version
            // doesn't match the packages, axotag only uses the list of packages for parsing
            // the "my-app" prefix out of my-app-v1.0.0. If the tag is just a unified version,
            // then it will parse that out for us.
            *announcing = parse_tag_for_all_packages(graph, tag)?;
            match &announcing.release {
                ReleaseType::None => {
                    unreachable!("internal dist error: tag selection should have picked a tag")
                }
                ReleaseType::Version(version) => {
                    // It was indeed a version tag, force all distable packages to have that version
                    let packages = releases.iter().map(|(idx, _)| *idx);
                    overwrite_package_versions(graph, packages.clone(), version);
                }
                ReleaseType::Package { idx, version } => {
                    // If this was a package tag, force just that one package to have that version
                    //
                    // NOTE: I believe currently axotag will actually error out on an integrity
                    // check for whether the version actually matches, so this branch is
                    // probably useless/impossible. However if we ever drop the limit in axotag,
                    // might as well make this work while we're thinking about this.
                    //
                    // ...that said this also probably requires mutating the input `releases` list
                    overwrite_package_versions(graph, Some(PackageIdx(*idx)), version);
                }
            }
        }
        TagMode::ForceMaxAndTimestamp => {
            // We've just been told to release all distable packages at all cost.
            //
            // The biggest issue with this is that they might be different versions,
            // but we need to make a tag that axotag agrees matches all the packages
            // we're trying to release (because e.g. axo Releases will check that server-side).
            //
            // So we do the following set of transforms to ensure that.
            let packages = releases.iter().map(|(idx, _)| *idx);
            // First, get the maximum version of all distable packages, as a way to get
            // a "reasonable" version. This will work for a fully unified version workspace,
            // or for a workspace where packages that didn't change are allowed to not bump ver.
            let mut forced_version = maximum_version(graph, packages.clone()).unwrap();
            // Add a timestamp buildid to the version so this release is unique and a prerelease.
            timestamp_version(&mut forced_version);
            // Overwrite all distable packages to have this new version
            overwrite_package_versions(graph, packages.clone(), &forced_version);
            // Make a tag for that version
            let tag = format!("v{forced_version}");
            // Ask axotag to make sense of it all
            *announcing = parse_tag_for_all_packages(graph, &tag)?;
        }
    }

    Ok(())
}

/// Modify the version to include a timestamp in the prerelease portion
fn timestamp_version(version: &mut Version) {
    // FIXME: it would be nice if this was configurable with a template
    // the current template here is `{version}-alpha.{timestamp}`.
    // Although as actually uses this is `{max_version}-alpha.{timestamp}`.
    if version.pre.is_empty() {
        // FIXME?: should we actually unconditionally do this?
        version.pre = semver::Prerelease::new("alpha").unwrap();
    }

    let now = std::time::SystemTime::now();
    let secs = now.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    // FIXME?: use chrono for better format
    version.pre = semver::Prerelease::new(&format!("{}.{}", version.pre, secs)).unwrap();
}

/// Get the maximum version among the given packages
fn maximum_version(
    graph: &DistGraphBuilder,
    packages: impl IntoIterator<Item = PackageIdx>,
) -> Option<Version> {
    packages
        .into_iter()
        .filter_map(|pkg_idx| graph.workspaces.package(pkg_idx).version.as_ref())
        .map(|v| v.cargo())
        .max()
        .cloned()
}

/// Overwrite the versions of the given packages
///
/// Currently this is
fn overwrite_package_versions(
    graph: &mut DistGraphBuilder,
    packages: impl IntoIterator<Item = PackageIdx>,
    version: &Version,
) {
    for pkg_idx in packages {
        graph.workspaces.package_mut(pkg_idx).version =
            Some(axoproject::Version::Cargo(version.clone()));
    }
}

/// Run axotag on the given tag. Primarily this exists to extract
/// the Version from a tag, but if the tag is for a specific package,
/// it will also identify that.
///
/// Note that in the case where the tag *isn't* for a specific package,
/// axotag will happily parse any version, and doesn't care about the versions.
///
/// FIXME: We should probably change axotag to behave similarly for specific packages,
/// separating out the version-match integrity check as an optional thing,
/// as we may want to take the tag as a Force command, in which case the versions
/// are expected to mismatch. If we do this we'll need to be careful about updating
/// all the users of axotag, who expect that check to be done for them.
fn parse_tag_for_all_packages(
    graph: &DistGraphBuilder,
    tag: &str,
) -> DistResult<PartialAnnouncementTag> {
    // If we're given a specific real tag to use, ask axotag to parse it
    // and identify which packages are selected by it.
    let packages: Vec<Package> = graph
        .workspaces
        .all_packages()
        .map(|(_, info)| Package {
            name: info.name.clone(),
            version: info.version.clone().map(|v| v.semver().clone()),
        })
        .collect();

    let announcing = parse_tag(&packages, tag)?;
    Ok(announcing)
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
        let info = graph.workspaces.package(pkg_idx);
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
            let info = graph.workspaces.package(pkg_id);
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
    let info = graph.workspaces.package(*some_pkg);
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
        // Skip "hidden" apps
        if !release.display.unwrap_or(true) {
            continue;
        }
        announcing_github = true;

        let display_name = release.display_name.as_ref().unwrap_or(&release.app_name);
        let heading_suffix = format!("{} {}", display_name, release.app_version);

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

            for artifact in &other_artifacts {
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

        if !other_artifacts.is_empty() && manifest.github_attestations {
            if let Some(GithubHosting { owner, repo, .. }) = &release.hosting.github {
                writeln!(&mut gh_body, "## Verifying GitHub Artifact Attestations\n",).unwrap();
                writeln!(&mut gh_body, "The artifacts in this release have attestations generated with GitHub Artifact Attestations. These can be verified by using the [GitHub CLI](https://cli.github.com/manual/gh_attestation_verify):").unwrap();
                writeln!(
                    &mut gh_body,
                    "```sh\ngh attestation verify <file-path of downloaded artifact> --repo {owner}/{repo}\n```\n",
                ).unwrap();
                writeln!(&mut gh_body, "You can also download the attestation from [GitHub](https://github.com/{owner}/{repo}/attestations) and verify against that directly:").unwrap();
                writeln!(
                    &mut gh_body,
                    "```sh\ngh attestation verify <file-path of downloaded artifact> --bundle <file-path of downloaded attestation>\n```\n",
                ).unwrap();
            }
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
