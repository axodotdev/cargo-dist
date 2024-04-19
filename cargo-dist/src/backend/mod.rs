//! The backend of cargo-dist -- things it outputs

use std::time::Duration;

use axoasset::SourceFile;
use camino::Utf8Path;
use newline_converter::dos2unix;

use crate::errors::{DistError, DistResult};

pub mod ci;
pub mod installer;
pub mod templates;

/// Check if the given file has the same contents we generated
pub fn diff_files(existing_file: &Utf8Path, new_file_contents: &str) -> DistResult<()> {
    // FIXME: should we catch all errors, or only LocalAssetNotFound?
    let existing = if let Ok(file) = SourceFile::load_local(existing_file) {
        file
    } else {
        SourceFile::new(existing_file.as_str(), String::new())
    };
    diff_source(existing, new_file_contents)
}

pub(crate) fn diff_source(existing: SourceFile, new_file_contents: &str) -> DistResult<()> {
    // Normalize away newline differences, those aren't worth failing things over
    let a = dos2unix(existing.contents());
    let b = dos2unix(new_file_contents);

    // Diff the files with the Pretty "patience" algorithm
    //
    // The timeout exists because essentially diff algorithms iteratively refine the results,
    // and can convince themselves to try way too hard for minimum benefit. Hitting the timeout
    // isn't fatal, it just tells the algorithm to call the result "good enough" if it hits
    // something pathalogical.
    let diff = similar::TextDiff::configure()
        .algorithm(similar::Algorithm::Patience)
        .timeout(Duration::from_millis(10))
        .diff_lines(&a, &b)
        .unified_diff()
        .header(existing.origin_path(), existing.origin_path())
        .to_string();

    if !diff.is_empty() {
        Err(DistError::CheckFileMismatch {
            file: existing,
            diff,
        })
    } else {
        Ok(())
    }
}
