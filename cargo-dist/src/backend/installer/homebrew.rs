//! Code for generating installer.sh

use axoasset::LocalAsset;
use camino::Utf8PathBuf;
use cargo_dist_schema::DistManifest;
use serde::Serialize;

use super::InstallerInfo;
use crate::{
    backend::templates::{Templates, TEMPLATE_INSTALLER_RB},
    errors::DistResult,
    generate_checksum,
    installer::ExecutableZipFragment,
    linkage::{Linkage, PackageManager},
    tasks::DistGraph,
};

/// Info about a Homebrew formula
#[derive(Debug, Clone, Serialize)]
pub struct HomebrewInstallerInfo {
    /// The application's name
    pub name: String,
    /// Formula class name
    pub formula_class: String,
    /// The application's license, in SPDX format
    pub license: Option<String>,
    /// The URL to the application's homepage
    pub homepage: Option<String>,
    /// A brief description of the application
    pub desc: Option<String>,
    /// A GitHub repository to write the formula to, in owner/name format
    pub tap: Option<String>,
    /// macOS AMD64 artifact
    pub x86_64_macos: Option<ExecutableZipFragment>,
    /// sha256 of macOS AMD64 artifact
    pub x86_64_macos_sha256: Option<String>,
    /// macOS ARM64 artifact
    pub arm64_macos: Option<ExecutableZipFragment>,
    /// sha256 of macOS ARM64 artifact
    pub arm64_macos_sha256: Option<String>,
    /// Linux AMD64 artifact
    pub x86_64_linux: Option<ExecutableZipFragment>,
    /// sha256 of Linux AMD64 artifact
    pub x86_64_linux_sha256: Option<String>,
    /// Linux ARM64 artifact
    pub arm64_linux: Option<ExecutableZipFragment>,
    /// sha256 of Linux ARM64 artifact
    pub arm64_linux_sha256: Option<String>,
    /// Generic installer info
    pub inner: InstallerInfo,
    /// Additional packages to specify as dependencies
    pub dependencies: Vec<String>,
}

pub(crate) fn write_homebrew_formula(
    templates: &Templates,
    graph: &DistGraph,
    source_info: &HomebrewInstallerInfo,
    manifest: &DistManifest,
) -> DistResult<()> {
    let mut info = source_info.clone();

    // Fetch any detected dependencies from the linkage data
    let dependencies = manifest
        .linkage
        .iter()
        .flat_map(|l| Linkage::from_schema(l).packages_from(PackageManager::Homebrew));

    // Merge with the manually-specified deps
    info.dependencies.extend(dependencies);

    // Generate sha256 as late as possible; the artifacts might not exist
    // earlier to do that.
    if let Some(arm64_ref) = &info.arm64_macos {
        let path = Utf8PathBuf::from(&graph.dist_dir).join(&arm64_ref.id);
        if path.exists() {
            let sha256 = generate_checksum(&crate::config::ChecksumStyle::Sha256, &path)?;
            info.arm64_macos_sha256 = Some(sha256);
        }
    }
    if let Some(x86_64_ref) = &info.x86_64_macos {
        let path = Utf8PathBuf::from(&graph.dist_dir).join(&x86_64_ref.id);
        if path.exists() {
            let sha256 = generate_checksum(&crate::config::ChecksumStyle::Sha256, &path)?;
            info.x86_64_macos_sha256 = Some(sha256);
        }
    }

    // Linuxbrew
    if let Some(arm64_ref) = &info.arm64_linux {
        let path = Utf8PathBuf::from(&graph.dist_dir).join(&arm64_ref.id);
        if path.exists() {
            let sha256 = generate_checksum(&crate::config::ChecksumStyle::Sha256, &path)?;
            info.arm64_linux_sha256 = Some(sha256);
        }
    }
    if let Some(x86_64_ref) = &info.x86_64_linux {
        let path = Utf8PathBuf::from(&graph.dist_dir).join(&x86_64_ref.id);
        if path.exists() {
            let sha256 = generate_checksum(&crate::config::ChecksumStyle::Sha256, &path)?;
            info.x86_64_linux_sha256 = Some(sha256);
        }
    }

    let script = templates.render_file_to_clean_string(TEMPLATE_INSTALLER_RB, &info)?;
    LocalAsset::write_new(&script, &info.inner.dest_path)?;
    Ok(())
}

/// Converts the provided app name into a Ruby class-compatible
/// string suitable for use as the class in a Homebrew formula.
// Homebrew implementation is Formulary.class_s:
// https://github.com/Homebrew/brew/blob/8c7cd3c0fd46f7808e782e40359c19271f950a75/Library/Homebrew/formulary.rb#L447-L453
pub fn to_class_case(app_name: &str) -> String {
    if app_name.is_empty() {
        return app_name.to_owned();
    }

    let mut out = app_name.to_owned();
    // First, we uppercase the first character in the string
    out[..1].make_ascii_uppercase();

    let mut chars = vec![];
    let mut iter = out.chars().peekable();
    let mut el = iter.next();
    let mut at_replaced = false;
    while el.is_some() {
        let char = el.unwrap();
        // -, _ and . are invalid characters in Ruby classes.
        // Homebrew handles these by stripping them, then uppercasing
        // the following character
        match char {
            '-' | '_' | '.' => {
                // Only perform a replacement if the following character is
                // in the range [a-zA-Z0-9]
                if let Some(next) = iter.peek() {
                    if next.is_ascii_digit() || next.is_ascii_alphabetic() {
                        chars.push(next.to_ascii_uppercase());
                        iter.next();
                    } else {
                        chars.push(char);
                    }
                } else {
                    chars.push(char);
                }
            }
            // Perform an @ replacement, but only if followed by a digit
            // We also perform this replacement only once
            '@' => {
                if let Some(next) = iter.peek() {
                    if next.is_ascii_digit() && !at_replaced {
                        chars.push('A');
                        chars.push('T');
                        chars.push(*next);
                        iter.next();
                        at_replaced = true;
                    } else {
                        chars.push(char);
                    }
                } else {
                    chars.push(char);
                }
            }
            // So that things like c++ become cxx
            '+' => {
                chars.push('x');
            }
            _ => chars.push(char),
        }

        el = iter.next();
    }
    chars.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::to_class_case;

    fn run_comparison(in_str: &str, expected: &str) {
        let out_str = to_class_case(in_str);

        assert_eq!(out_str, expected);
    }

    #[test]
    fn class_case_basic() {
        run_comparison("ccd2cue", "Ccd2cue");
    }

    #[test]
    fn handles_dashes() {
        run_comparison("akaikatana-repack", "AkaikatanaRepack");
    }

    #[test]
    fn handles_single_letter_then_dash() {
        run_comparison("c-lang", "CLang");
    }

    #[test]
    fn handles_underscores() {
        run_comparison("abc_def", "AbcDef");
    }

    #[test]
    fn handles_strings_with_dots() {
        run_comparison("last.fm", "LastFm");
    }

    #[test]
    fn replaces_plus_with_x() {
        run_comparison("c++", "Cxx");
    }

    #[test]
    fn replaces_ampersand_with_at() {
        run_comparison("openssl@3", "OpensslAT3");
    }

    // The following are some extra test cases not covered in Homebrew's specs
    // to ensure we remain quirk-for-quirk compatible.
    #[test]
    fn class_caps_after_numbers() {
        run_comparison("mni2mz3", "Mni2mz3");
    }

    #[test]
    fn handles_pluralization() {
        run_comparison("tetanes", "Tetanes");
    }

    #[test]
    fn multiple_underscores() {
        run_comparison("abc__def", "Abc_Def");
    }

    // Yes, it's correct that Homebrew produces a class-incompatible string
    #[test]
    fn multiple_periods() {
        run_comparison("abc..def", "Abc.Def");
    }

    #[test]
    fn multiple_special_chars() {
        run_comparison("abc-.def", "Abc-Def");
    }

    #[test]
    fn ends_with_dash() {
        run_comparison("abc-", "Abc-");
    }

    #[test]
    fn multiple_ampersands() {
        run_comparison("openssl@@3", "Openssl@AT3");
    }

    #[test]
    fn ampersand_but_no_digit() {
        run_comparison("openssl@blah", "Openssl@blah");
    }
}
