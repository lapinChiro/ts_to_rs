//! Shared helpers for integration, E2E, and compile tests.
//!
//! Included via `#[path = "test_helpers.rs"] mod test_helpers;` from multiple
//! test crates, so not every item is used in every crate.
#![allow(dead_code)]

use std::fs;

/// Strips internal module `use` statements while preserving external crate imports.
///
/// Internal references (e.g., `use crate::`, `use super::`) cannot be resolved in
/// single-file compilation. External crate imports (e.g., `use serde`, `use scopeguard`)
/// must be kept for the code to compile with dependencies.
pub fn strip_internal_use_statements(rs_source: &str) -> String {
    rs_source
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("use ") && !trimmed.starts_with("pub use ") {
                return true;
            }
            !trimmed.contains("crate::") && !trimmed.contains("super::")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// RAII guard for temporary files. Removes the file on drop (including panic).
pub struct TempFile {
    path: String,
}

impl TempFile {
    /// Creates a temporary file with the given content and returns an RAII guard.
    pub fn new(path: String, content: &str) -> Self {
        fs::write(&path, content).unwrap_or_else(|e| panic!("failed to write {path}: {e}"));
        Self { path }
    }

    /// Wraps an existing file path for RAII cleanup without writing.
    ///
    /// Use when the file is written by another mechanism (e.g., `write_with_advancing_mtime`)
    /// but still needs cleanup on drop.
    pub fn guard(path: String) -> Self {
        Self { path }
    }

    /// Returns the file path.
    pub fn path(&self) -> &str {
        &self.path
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
