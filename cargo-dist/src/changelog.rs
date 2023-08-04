//! Logic for parsing changelogs

use std::fs::File;
use std::io::{BufReader, Read};

use camino::Utf8Path;
use miette::{Context, IntoDiagnostic};
use semver::Version;
use tracing::info;

use crate::errors::Result;
use crate::tasks::DistGraphBuilder;

impl DistGraphBuilder<'_> {
    /// Try to compute changelogs for the announcement
    pub fn compute_announcement_changelog(&mut self, announcing_version: Option<&Version>) {
        // FIXME: currently this only supports a top-level changelog, it would be nice
        // to allow individual apps to have individual streams

        // FIXME: derive changelogs from semantic commits?

        // Try to find the version we're announcing in the top level CHANGELOG/RELEASES
        let Some(announcing_version) = announcing_version else {
            info!("not announcing a consistent version, skipping changelog generation");
            return;
        };

        // Load and parse the changelog
        let Some(changelog_path) = &self.workspace.root_auto_includes.changelog else {
            info!("no root changelog found, skipping changelog generation");
            return;
        };
        let Ok(changelog_str) = try_load_changelog(changelog_path) else {
            info!("failed to load changelog, skipping changelog generation");
            return;
        };
        let changelogs = parse_changelog::parse(&changelog_str)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to parse changelog at {changelog_path}"));
        let changelogs = match changelogs {
            Ok(changelogs) => changelogs,
            Err(e) => {
                info!(
                    "failed to parse changelog, skipping changelog generation\n{:?}",
                    e
                );
                return;
            }
        };

        // Try to extract the changelog entry for the announcing version.
        //
        // First, try to find the exact version.
        //
        // If that fails, try to find this version without the prerelease suffix.
        // Because releasing a prerelease for a version that was already published before does not
        // make much sense it is fairly safe to assume that this entry is in fact just our WIP state
        // of the release notes.
        //
        // If that fails, try to find a section called "Unreleased" and use that (if it's a prerelease).
        let Some((title, notes)) = try_extract_changelog_exact(&changelogs, announcing_version)
            .or_else(|| try_extract_changelog_normalized(&changelogs, announcing_version))
            .or_else(|| try_extract_changelog_unreleased(&changelogs, announcing_version))
        else {
            info!(
                "failed to find {announcing_version} in changelogs, skipping changelog generation"
            );
            return;
        };

        info!("successfully parsed changelog!");
        self.inner.announcement_title = Some(title);
        // Those windows newlines get everywhere...
        let clean_notes = newline_converter::dos2unix(&notes);
        self.inner.announcement_changelog = Some(clean_notes.into_owned());
    }
}

/// Load a changelog to a string
fn try_load_changelog(changelog_path: &Utf8Path) -> Result<String> {
    let file = File::open(changelog_path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to open changelog at {changelog_path}"))?;
    let mut data = BufReader::new(file);
    let mut changelog_str = String::new();
    data.read_to_string(&mut changelog_str)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to read changelog at {changelog_path}"))?;
    Ok(changelog_str)
}

/// Tries to find a changelog entry with the exact version given and returns its title and notes.
fn try_extract_changelog_exact(
    changelogs: &parse_changelog::Changelog,
    version: &Version,
) -> Option<(String, String)> {
    let version_string = format!("{}", version);

    changelogs.get(&*version_string).map(|release_notes| {
        (
            release_notes.title.to_string(),
            release_notes.notes.to_string(),
        )
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
) -> Option<(String, String)> {
    if version.pre.is_empty() {
        return None;
    }

    let version_normalized = Version::new(version.major, version.minor, version.patch);
    let version_normalized_string = format!("{}", version_normalized);

    let release_notes = changelogs.get(&*version_normalized_string)?;

    // title looks something like '<prefix><version><freeform>'
    // prefix could be 'v' or 'Version ' for example
    let (prefix_and_version, freeform) = release_notes.title.split_at(
        release_notes
            .title
            .find(&*version_normalized_string)
            .unwrap() // impossible that this version string is not present in the header
            + version_normalized_string.len(),
    );

    // insert prerelease suffix into the title
    let title = format!(
        "{}-{} {}",
        prefix_and_version.trim(),
        version.pre,
        freeform.trim()
    );

    Some((title.trim().to_string(), release_notes.notes.to_string()))
}

// Tries to find the "Unreleased" changelog heading and replaces it with "Version {version}"
//
// Noop if the given version isn't a prerelease.
fn try_extract_changelog_unreleased(
    changelogs: &parse_changelog::Changelog,
    version: &Version,
) -> Option<(String, String)> {
    if version.pre.is_empty() {
        return None;
    }

    let release_notes = changelogs.get("Unreleased")?;
    let title = format!("Version {version}");

    Some((title, release_notes.notes.to_string()))
}
