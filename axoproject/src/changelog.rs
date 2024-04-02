//! Support for interpretting changelogs

use camino::Utf8Path;

use crate::errors::Result;
use crate::{PackageInfo, Version, WorkspaceInfo};

/// Info about a changelog entry
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone)]
pub struct ChangelogInfo {
    /// Title of the entry
    pub title: String,
    /// Body of the entry
    pub body: String,
}

impl WorkspaceInfo {
    /// Get the changelog for a given version from the root workspace's changelog
    pub fn changelog_for_version(&self, version: &Version) -> Result<Option<ChangelogInfo>> {
        if let Some(changelog_path) = self.root_auto_includes.changelog.as_deref() {
            changelog_for_version(changelog_path, version)
        } else {
            Ok(None)
        }
    }
}

impl PackageInfo {
    /// Get the changelog for a given version from a package's changelog
    pub fn changelog_for_version(&self, version: &Version) -> Result<Option<ChangelogInfo>> {
        if let Some(changelog_path) = self.changelog_file.as_deref() {
            changelog_for_version(changelog_path, version)
        } else {
            Ok(None)
        }
    }
}

/// Get the changelog for a version
pub fn changelog_for_version(
    changelog_path: &Utf8Path,
    version: &Version,
) -> Result<Option<ChangelogInfo>> {
    // Load and parse the changelog
    let changelog_str = axoasset::LocalAsset::load_string(changelog_path)?;
    changelog_for_version_inner(changelog_path, &changelog_str, version)
}

/// Get the changelog for a version (inner version for testing)
pub fn changelog_for_version_inner(
    changelog_path: &Utf8Path,
    changelog_str: &str,
    version: &Version,
) -> Result<Option<ChangelogInfo>> {
    let changelogs = parse_changelog::parse(changelog_str)?;

    // Try to extract the changelog entry for the given version.
    //
    // First, try to find the exact version.
    //
    // If that fails, try to find this version without the prerelease suffix.
    // Because releasing a prerelease for a version that was already published before does not
    // make much sense it is fairly safe to assume that this entry is in fact just our WIP state
    // of the release notes.
    //
    // If that fails, try to find a section called "Unreleased" and use that (if it's a prerelease).
    if let Some(info) = try_extract_changelog_exact(&changelogs, version)
        .or_else(|| try_extract_changelog_normalized(&changelogs, version))
        .or_else(|| try_extract_changelog_unreleased(&changelogs, version))
    {
        Ok(Some(info))
    } else {
        Err(crate::errors::AxoprojectError::ChangelogVersionNotFound {
            path: changelog_path.to_owned(),
            version: version.clone(),
        })
    }
}

/// Tries to find a changelog entry with the exact version given and returns its title and notes.
fn try_extract_changelog_exact(
    changelogs: &parse_changelog::Changelog,
    version: &Version,
) -> Option<ChangelogInfo> {
    let version_string = format!("{}", version);

    changelogs
        .get(&*version_string)
        .map(|release_notes| ChangelogInfo {
            title: release_notes.title_no_link().to_string(),
            body: release_notes.notes.to_string(),
        })
}

/// Tries to find a changelog entry that matches the given version's normalized form. That is, just
/// the `major.minor.patch` part. If successful, the entry's title is modified to include the
/// version's prerelease part before it is returned together with the notes.
///
/// Noop if the given version is already normalized.
fn try_extract_changelog_normalized(
    changelogs: &parse_changelog::Changelog,
    version: &Version,
) -> Option<ChangelogInfo> {
    if version.is_stable() {
        return None;
    }

    let stable_version = version.stable_part();
    let stable_version_string = format!("{}", stable_version);

    let release_notes = changelogs.get(&*stable_version_string)?;

    // title looks something like '<prefix><version><freeform>'
    // `prefix` could be 'v' or 'Version ' for example
    let raw_title = release_notes.title_no_link();
    let (prefix, freeform) = raw_title.split_once(&stable_version_string)?;

    // insert prerelease suffix into the title
    let title = format!("{}{}{}", prefix, version, freeform);

    Some(ChangelogInfo {
        title,
        body: release_notes.notes.to_string(),
    })
}

// Tries to find the "Unreleased" changelog heading and replaces it with "Version {version}"
//
// Noop if the given version isn't a prerelease.
fn try_extract_changelog_unreleased(
    changelogs: &parse_changelog::Changelog,
    version: &Version,
) -> Option<ChangelogInfo> {
    if version.is_stable() {
        return None;
    }

    let release_notes = changelogs.get("Unreleased")?;
    let title = format!("Version {version}");

    Some(ChangelogInfo {
        title,
        body: release_notes.notes.to_string(),
    })
}
