#![deny(missing_docs)]
#![allow(clippy::result_large_err)]

//! # axotag
//!
//! This library contains tag-parsing code for use with cargo-dist.

use errors::{TagError, TagResult};
pub use semver;
pub use semver::Version;

pub mod errors;
#[cfg(test)]
mod tests;

/// Represents an opaque package.
pub struct Package {
    /// The package's name
    pub name: String,
    /// The package's version, if specified
    pub version: Option<Version>,
}

/// details on what we're announcing (partially computed)
pub struct PartialAnnouncementTag {
    /// The full tag
    pub tag: String,
    /// The release
    pub release: ReleaseType,
    /// whether we're prereleasing
    pub prerelease: bool,
}

impl Default for PartialAnnouncementTag {
    /// Constructs an empty PartialAnnouncementTag
    fn default() -> PartialAnnouncementTag {
        PartialAnnouncementTag {
            tag: String::new(),
            release: ReleaseType::None,
            prerelease: false,
        }
    }
}

/// which type of release we're announcing
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReleaseType {
    /// none
    None,
    /// unified release
    Version(Version),
    /// package
    Package {
        /// The index of the package from the passed in list
        idx: usize,
        /// The version of the package (in case the package didn't yet have one)
        version: Version,
    },
}

/// Do the actual parsing logic for a tag
///
/// If `tag` is None, then we had no --tag to parse, and need to do inference.
/// The return value is then essentially a default/empty PartialAnnouncementTag
/// which later passes will fill in.
pub fn parse_tag(packages: &[Package], tag: &str) -> TagResult<PartialAnnouncementTag> {
    // First thing's first: if they gave us an announcement tag then we should try to parse it
    let mut announcing_package = None;
    let announcing_version;
    let announcing_prerelease;
    let announcement_tag = tag.to_owned();
    let mut tag_suffix;
    // Check if we're using `/`'s to delimit things
    if let Some((prefix, suffix)) = announcement_tag.rsplit_once('/') {
        // We're at least in "blah/v1.0.0" format
        let maybe_package = if let Some((_prefix, package)) = prefix.rsplit_once('/') {
            package
        } else {
            // There's only one `/`, assume the whole prefix could be a package name
            prefix
        };
        // Check if this is "blah/blah/some-package/v1.0.0" format by checking if the last slash-delimited
        // component is exactly a package name (strip_prefix produces empty string)
        if let Some((package, "")) = strip_prefix_package(maybe_package, packages) {
            announcing_package = Some(package);
        }
        tag_suffix = suffix;
    } else {
        tag_suffix = &announcement_tag;
    };

    // If we don't have an announcing_package yet, check if this is "some-package-v1.0.0" format
    if announcing_package.is_none() {
        if let Some((package, suffix)) = strip_prefix_package(tag_suffix, packages) {
            // Must be followed by a dash to be accepted
            if let Some(suffix) = suffix.strip_prefix('-') {
                tag_suffix = suffix;
                announcing_package = Some(package);
            }
        }
    }

    // At this point, assuming the input is valid, tag_suffix should just be the version
    // component with an optional "v" prefix, so strip that "v"
    if let Some(suffix) = tag_suffix.strip_prefix('v') {
        tag_suffix = suffix;
    }

    // Now parse the version out
    match tag_suffix.parse::<Version>() {
        Ok(version) => {
            // Register whether we're announcing a prerelease
            announcing_prerelease = !version.pre.is_empty();
            announcing_version = version;

            // If there's an announcing package, validate that the version matches
            if let Some(pkg_idx) = announcing_package {
                if let Some(package) = packages.get(pkg_idx) {
                    if let Some(real_version) = &package.version {
                        if real_version != &announcing_version {
                            return Err(TagError::ContradictoryTagVersion {
                                tag: tag.to_owned(),
                                package_name: package.name.clone(),
                                tag_version: announcing_version,
                                real_version: real_version.clone(),
                            });
                        }
                    }
                }
            }
        }
        Err(e) => {
            return Err(TagError::TagVersionParse {
                tag: tag.to_owned(),
                details: e,
            })
        }
    }

    let release = if let Some(idx) = announcing_package {
        ReleaseType::Package {
            idx,
            version: announcing_version,
        }
    } else {
        ReleaseType::Version(announcing_version)
    };

    Ok(PartialAnnouncementTag {
        tag: announcement_tag,
        prerelease: announcing_prerelease,
        release,
    })
}

/// Try to strip-prefix a package name from the given input, preferring whichever one is longest
/// (to disambiguate situations where you have `my-app` and `my-app-helper`).
///
/// If a match is found, then the return value is:
/// * the idx of the package
/// * the rest of the input
fn strip_prefix_package<'a>(input: &'a str, packages: &[Package]) -> Option<(usize, &'a str)> {
    let mut result: Option<(usize, &'a str)> = None;
    for (pkg_id, package) in packages.iter().enumerate() {
        if let Some(rest) = input.strip_prefix(&package.name) {
            if let Some((_, best)) = result {
                if best.len() <= rest.len() {
                    continue;
                }
            }
            result = Some((pkg_id, rest))
        }
    }
    result
}
