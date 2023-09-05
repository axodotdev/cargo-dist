//! The backend of cargo-dist -- things it outputs

use axoasset::SourceFile;
use camino::Utf8Path;

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

    if new_file_contents != existing.contents() {
        // Get more precise info
        let mut existing_lines = existing.contents().lines();
        let mut new_lines = new_file_contents.lines();

        let existing_line_count = existing_lines.clone().count();
        let new_line_count = new_lines.clone().count();
        let max_lines = existing_line_count.max(new_line_count);

        for line_number in 1..=max_lines {
            match (existing_lines.next(), new_lines.next()) {
                (Some(existing_line), Some(new_line)) => {
                    if existing_line != new_line {
                        return Err(DistError::CheckFileMismatch {
                            existing_line: existing_line.to_owned(),
                            new_line: new_line.to_owned(),
                            file: existing,
                            line_number,
                        });
                    }
                }
                (None, Some(new_line)) => {
                    return Err(DistError::CheckFileMismatch {
                        existing_line: String::new(),
                        new_line: new_line.to_owned(),
                        file: existing,
                        line_number,
                    });
                }
                (Some(existing_line), None) => {
                    return Err(DistError::CheckFileMismatch {
                        existing_line: existing_line.to_owned(),
                        new_line: String::new(),
                        file: existing,
                        line_number,
                    });
                }
                (None, None) => {}
            }
        }
        unreachable!()
    } else {
        Ok(())
    }
}
