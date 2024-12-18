//! Code for generating formula.rb

use axoasset::LocalAsset;
use dist_schema::{ChecksumValue, DistManifest, HomebrewPackageName};
use serde::Serialize;
use spdx::{
    expression::{ExprNode, Operator},
    Expression, ParseError,
};

use super::InstallerInfo;
use crate::{
    backend::templates::TEMPLATE_INSTALLER_RB,
    config::{ChecksumStyle, LibraryStyle},
    errors::DistResult,
    installer::ExecutableZipFragment,
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
    pub desc: String,
    /// A GitHub repository to write the formula to, in owner/name format
    pub tap: Option<String>,
    /// Generic installer info
    pub inner: InstallerInfo,
    /// Additional packages to specify as dependencies
    pub dependencies: Vec<HomebrewPackageName>,
    /// Whether to install packaged C dynamic libraries
    pub install_libraries: Vec<LibraryStyle>,
}

/// All homebrew-specific fragments
#[derive(Debug, Clone, Serialize)]
pub struct HomebrewFragments<T> {
    /// macOS AMD64 artifact
    pub x86_64_macos: Option<T>,
    /// macOS ARM64 artifact
    pub arm64_macos: Option<T>,
    /// Linux AMD64 artifact
    pub x86_64_linux: Option<T>,
    /// Linux ARM64 artifact
    pub arm64_linux: Option<T>,
}

pub(crate) fn write_homebrew_formula(
    dist: &DistGraph,
    info: &HomebrewInstallerInfo,
    fragments: &HomebrewFragments<ExecutableZipFragment>,
    manifest: &DistManifest,
) -> DistResult<()> {
    let info = info.clone();

    let checksum_key = ChecksumStyle::Sha256.ext();
    let map_fragment = |fragment: ExecutableZipFragment| -> HomebrewFragment {
        let sha256 = manifest
            .artifacts
            .get(&fragment.id)
            .and_then(|a| a.checksums.get(checksum_key))
            .cloned();
        let linkage = manifest.linkage_for_artifact(&fragment.id);

        let dependencies = linkage
            .homebrew
            .iter()
            .filter_map(|lib| lib.source.clone())
            .collect();

        HomebrewFragment {
            fragment,
            sha256,
            dependencies,
        }
    };

    macro_rules! map_fragments {
        ($fragments:ident = ($($name:ident),*)) => {
            let $fragments = HomebrewFragments {
                $($name: $fragments.$name.clone().map(map_fragment)),*
            };
        };
    }
    map_fragments!(fragments = (arm64_linux, x86_64_linux, arm64_macos, x86_64_macos));

    let dest_path = info.inner.dest_path.clone();
    let inputs = HomebrewTemplateInputs { info, fragments };

    let script = dist
        .templates
        .render_file_to_clean_string(TEMPLATE_INSTALLER_RB, &inputs)?;
    LocalAsset::write_new(&script, dest_path)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct HomebrewTemplateInputs {
    #[serde(flatten)]
    info: HomebrewInstallerInfo,

    #[serde(flatten)]
    fragments: HomebrewFragments<HomebrewFragment>,
}

#[derive(Debug, Clone, Serialize)]
struct HomebrewFragment {
    #[serde(flatten)]
    fragment: ExecutableZipFragment,

    /// SHA256 sum of the fragment. When building "just the installers", like
    /// when running `dist build --artifacts global` locally, we don't have
    /// the SHA256 for the fragment, since we didn't actually build them.
    sha256: Option<ChecksumValue>,

    /// homebrew package dependencies
    dependencies: Vec<String>,
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

/// Converts SPDX license string into Homebrew Ruby DSL
// Homebrew DSL reference: https://docs.brew.sh/License-Guidelines
pub fn to_homebrew_license_format(app_license: &str) -> Result<String, ParseError> {
    let spdx = Expression::parse(app_license)?;
    let mut spdx = spdx.iter().peekable();
    let mut buffer: Vec<String> = vec![];

    while let Some(token) = spdx.next() {
        match token {
            ExprNode::Req(req) => {
                // If token is a license, push to the buffer as-is for next operator or end.
                let requirement = format!("\"{}\"", req.req);
                buffer.push(requirement);
            }
            ExprNode::Op(op) => {
                // If token is an operation, group operands in buffer into all_of/any_of clause.
                // Operations are postfix, so we pop off the previous two elements and combine.
                let second_operand = buffer.pop().expect("Operator missing first operand.");
                let first_operand = buffer.pop().expect("Operator missing second operand.");
                let mut combined = format!("{}, {}", first_operand, second_operand);

                // If the operations that immediately follow are the same as the current operation,
                // squash their operands into the same all_of/any_of clause.
                while let Some(ExprNode::Op(next_op)) = spdx.peek() {
                    if next_op != op {
                        break;
                    }
                    let _ = spdx.next();
                    let operand = buffer.pop().expect("Operator missing first operand.");
                    combined = format!("{}, {}", operand, combined);
                }

                // Use corresponding homebrew DSL keyword and square bracket the list of licenses.
                let operation = match op {
                    Operator::And => "all_of",
                    Operator::Or => "any_of",
                };
                let mut enclosed = format!("{operation}: [{combined}]");

                // Only wrap all_of/any_of clause in brackets if it is nested within an outer clause.
                if spdx.peek().is_some() {
                    enclosed = format!("{{ {enclosed} }}");
                }

                // Push clause back onto the buffer, as it might be an operand in another clause.
                buffer.push(enclosed);
            }
        }
    }

    // After all tokens have been iterated through, if the SPDX expression is well-formed, there
    // should only be a single element left in the buffer: a single license or outermost clause.
    Ok(buffer[0].clone())
}

#[cfg(test)]
mod tests {
    use spdx::ParseError;

    use super::{to_class_case, to_homebrew_license_format};

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

    fn run_spdx_comparison(spdx_string: &str, homebrew_dsl: &str) {
        let result = to_homebrew_license_format(spdx_string).unwrap();
        assert_eq!(result, homebrew_dsl);
    }

    #[test]
    fn spdx_single_license() {
        run_spdx_comparison("MIT", r#""MIT""#);
    }

    #[test]
    fn spdx_single_license_with_plus() {
        run_spdx_comparison("Apache-2.0+", r#""Apache-2.0+""#);
    }

    #[test]
    fn spdx_two_licenses_any() {
        run_spdx_comparison("MIT OR 0BSD", r#"any_of: ["MIT", "0BSD"]"#);
    }

    #[test]
    fn spdx_two_licenses_all() {
        run_spdx_comparison("MIT AND 0BSD", r#"all_of: ["MIT", "0BSD"]"#);
    }

    #[test]
    fn spdx_two_licenses_with_plus() {
        run_spdx_comparison("MIT OR EPL-1.0+", r#"any_of: ["MIT", "EPL-1.0+"]"#);
    }

    #[test]
    fn spdx_three_licenses() {
        run_spdx_comparison(
            "MIT OR Apache-2.0 OR CC-BY-4.0",
            r#"any_of: ["MIT", "Apache-2.0", "CC-BY-4.0"]"#,
        );
    }

    #[test]
    fn spdx_three_licenses_or_and() {
        run_spdx_comparison(
            "MIT OR Apache-2.0 AND CC-BY-4.0",
            // NOTE: Homebrew parses this as {:all_of=>[{:any_of=>["MIT", "Apache-2.0"]}, "CC-BY-4.0"]}
            // According to the SPDX v3 spec, this seems to be wrong, as operator precedence is specified
            // as WITH > AND > OR (while Homebrew evaluates OR operators, then AND operators).
            // The result produced in this test is correct.
            //
            // https://spdx.github.io/spdx-spec/v3.0/annexes/SPDX-license-expressions/#d45-order-of-precedence-and-parentheses
            r#"any_of: ["MIT", { all_of: ["Apache-2.0", "CC-BY-4.0"] }]"#,
        );
    }

    #[test]
    fn spdx_three_licenses_and_or() {
        run_spdx_comparison(
            "MIT AND Apache-2.0 OR CC-BY-4.0",
            // Likewise, Homebrew parses this as {:all_of=>["MIT", {:any_of=>["Apache-2.0", "CC-BY-4.0"]}]}
            // Which appears to be incorrect.
            r#"any_of: [{ all_of: ["MIT", "Apache-2.0"] }, "CC-BY-4.0"]"#,
        );
    }

    #[test]
    fn spdx_parentheses() {
        run_spdx_comparison(
            "MIT OR (0BSD AND Zlib) OR curl",
            r#"any_of: ["MIT", { all_of: ["0BSD", "Zlib"] }, "curl"]"#,
        );
    }

    #[test]
    fn spdx_nested_parentheses() {
        run_spdx_comparison(
            "MIT AND (Apache-2.0 OR (CC-BY-4.0 AND 0BSD))",
            r#"all_of: ["MIT", { any_of: ["Apache-2.0", { all_of: ["CC-BY-4.0", "0BSD"] }] }]"#,
        );
    }

    fn run_malformed_spdx(spdx_string: &str) {
        let result = to_homebrew_license_format(spdx_string);
        assert!(matches!(result, Err(ParseError { .. })));
    }

    #[test]
    fn spdx_invalid_license_name() {
        run_malformed_spdx("foo");
    }

    #[test]
    fn spdx_invalid_just_operator() {
        run_malformed_spdx("AND");
    }

    #[test]
    fn spdx_invalid_dangling_operator() {
        run_malformed_spdx("MIT OR");
    }

    #[test]
    fn spdx_invalid_adjacent_operator() {
        run_malformed_spdx("MIT AND OR Apache-2.0");
    }
}
