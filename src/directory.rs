//! Directory traversal and module structure generation for multi-file conversion.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

/// Recursively collects `.ts` files from a directory, excluding:
/// - `node_modules/` directories
/// - Hidden directories (starting with `.`)
/// - `.d.ts` files
///
/// # Errors
///
/// Returns an error if the directory cannot be read.
pub fn collect_ts_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_ts_files_recursive(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_ts_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("failed to read directory: {}", dir.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        if path.is_dir() {
            // Skip node_modules and hidden directories
            if name == "node_modules" || name.starts_with('.') {
                continue;
            }
            collect_ts_files_recursive(&path, files)?;
        } else if path.is_file() {
            // Skip .d.ts files
            if name.ends_with(".d.ts") {
                continue;
            }
            if name.ends_with(".ts") {
                files.push(path);
            }
        }
    }
    Ok(())
}

/// Computes the output `.rs` path for a given `.ts` input path.
///
/// Maintains the relative directory structure from `input_dir` to `output_dir`.
/// Example: `compute_output_path("src/foo/bar.ts", "src", "out")` → `"out/foo/bar.rs"`
pub fn compute_output_path(ts_path: &Path, input_dir: &Path, output_dir: &Path) -> Result<PathBuf> {
    let relative = ts_path
        .strip_prefix(input_dir)
        .with_context(|| format!("{} is not under {}", ts_path.display(), input_dir.display()))?;
    let rs_relative = relative.with_extension("rs");
    Ok(output_dir.join(rs_relative))
}

/// Generates `mod.rs` content for a directory based on its `.rs` files and subdirectories.
///
/// - Each `.rs` file (except `mod.rs`) → `pub mod <stem>;`
/// - Each subdirectory containing `.rs` files → `pub mod <dir_name>;`
///
/// Returns `None` if there are no modules to declare.
pub fn generate_mod_rs(dir: &Path) -> Result<Option<String>> {
    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("failed to read directory: {}", dir.display()))?;

    let mut modules = BTreeSet::new();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if path.is_file() && name_str.ends_with(".rs") && name_str != "mod.rs" {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                modules.insert(stem.to_string());
            }
        } else if path.is_dir() {
            // Include subdirectory if it contains a mod.rs or any .rs files
            let sub_mod_rs = path.join("mod.rs");
            if sub_mod_rs.exists() {
                if let Some(dir_name) = path.file_name().and_then(|s| s.to_str()) {
                    modules.insert(dir_name.to_string());
                }
            }
        }
    }

    if modules.is_empty() {
        return Ok(None);
    }

    let content = modules
        .iter()
        .map(|m| format!("pub mod {m};"))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(Some(format!("{content}\n")))
}

/// Collects all directories that contain at least one `.rs` file (for mod.rs generation).
///
/// Returns directories in bottom-up order (deepest first) so that mod.rs files
/// in child directories exist before parent mod.rs files reference them.
pub fn collect_output_dirs(output_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut dirs = Vec::new();
    collect_output_dirs_recursive(output_dir, &mut dirs)?;
    // Reverse for bottom-up order (deepest directories first)
    dirs.sort();
    dirs.reverse();
    Ok(dirs)
}

fn collect_output_dirs_recursive(dir: &Path, dirs: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }

    let mut has_rs_files = false;
    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("failed to read directory: {}", dir.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_output_dirs_recursive(&path, dirs)?;
        } else if path.is_file() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.ends_with(".rs") && name_str != "mod.rs" {
                has_rs_files = true;
            }
        }
    }

    if has_rs_files {
        dirs.push(dir.to_path_buf());
    }

    Ok(())
}

/// Validates that a directory input has at least one `.ts` file to convert.
///
/// # Errors
///
/// Returns an error if no `.ts` files are found.
pub fn validate_has_ts_files(files: &[PathBuf], dir: &Path) -> Result<()> {
    if files.is_empty() {
        bail!("no .ts files found in directory: {}", dir.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_dir() -> TempDir {
        let tmp = TempDir::new().unwrap();

        // Create structure:
        // root/
        //   foo.ts
        //   bar.ts
        //   types.d.ts        (should be excluded)
        //   .hidden/
        //     secret.ts       (should be excluded)
        //   node_modules/
        //     lib.ts          (should be excluded)
        //   sub/
        //     baz.ts

        let root = tmp.path();
        fs::write(root.join("foo.ts"), "interface Foo {}").unwrap();
        fs::write(root.join("bar.ts"), "interface Bar {}").unwrap();
        fs::write(root.join("types.d.ts"), "declare interface X {}").unwrap();

        fs::create_dir(root.join(".hidden")).unwrap();
        fs::write(root.join(".hidden/secret.ts"), "").unwrap();

        fs::create_dir(root.join("node_modules")).unwrap();
        fs::write(root.join("node_modules/lib.ts"), "").unwrap();

        fs::create_dir(root.join("sub")).unwrap();
        fs::write(root.join("sub/baz.ts"), "interface Baz {}").unwrap();

        tmp
    }

    #[test]
    fn test_collect_ts_files_finds_ts_files() {
        let tmp = create_test_dir();
        let files = collect_ts_files(tmp.path()).unwrap();

        let names: Vec<&str> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();

        assert!(names.contains(&"foo.ts"));
        assert!(names.contains(&"bar.ts"));
        assert!(names.contains(&"baz.ts"));
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn test_collect_ts_files_excludes_d_ts() {
        let tmp = create_test_dir();
        let files = collect_ts_files(tmp.path()).unwrap();

        let names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();

        assert!(!names.contains(&"types.d.ts".to_string()));
    }

    #[test]
    fn test_collect_ts_files_excludes_node_modules() {
        let tmp = create_test_dir();
        let files = collect_ts_files(tmp.path()).unwrap();

        for f in &files {
            assert!(
                !f.to_string_lossy().contains("node_modules"),
                "should not include files from node_modules"
            );
        }
    }

    #[test]
    fn test_collect_ts_files_excludes_hidden_dirs() {
        let tmp = create_test_dir();
        let files = collect_ts_files(tmp.path()).unwrap();

        for f in &files {
            assert!(
                !f.to_string_lossy().contains(".hidden"),
                "should not include files from hidden directories"
            );
        }
    }

    #[test]
    fn test_compute_output_path() {
        let result = compute_output_path(
            Path::new("src/foo/bar.ts"),
            Path::new("src"),
            Path::new("out"),
        )
        .unwrap();
        assert_eq!(result, PathBuf::from("out/foo/bar.rs"));
    }

    #[test]
    fn test_compute_output_path_flat() {
        let result =
            compute_output_path(Path::new("src/main.ts"), Path::new("src"), Path::new("out"))
                .unwrap();
        assert_eq!(result, PathBuf::from("out/main.rs"));
    }

    #[test]
    fn test_generate_mod_rs() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::write(root.join("foo.rs"), "").unwrap();
        fs::write(root.join("bar.rs"), "").unwrap();

        let content = generate_mod_rs(root).unwrap().unwrap();
        assert_eq!(content, "pub mod bar;\npub mod foo;\n");
    }

    #[test]
    fn test_generate_mod_rs_excludes_mod_rs() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::write(root.join("foo.rs"), "").unwrap();
        fs::write(root.join("mod.rs"), "").unwrap();

        let content = generate_mod_rs(root).unwrap().unwrap();
        assert_eq!(content, "pub mod foo;\n");
    }

    #[test]
    fn test_generate_mod_rs_includes_subdirs_with_mod_rs() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::write(root.join("foo.rs"), "").unwrap();
        fs::create_dir(root.join("sub")).unwrap();
        fs::write(root.join("sub/mod.rs"), "").unwrap();

        let content = generate_mod_rs(root).unwrap().unwrap();
        assert_eq!(content, "pub mod foo;\npub mod sub;\n");
    }

    #[test]
    fn test_generate_mod_rs_empty_returns_none() {
        let tmp = TempDir::new().unwrap();
        let result = generate_mod_rs(tmp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_validate_has_ts_files_empty_returns_error() {
        let result = validate_has_ts_files(&[], Path::new("/some/dir"));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_has_ts_files_non_empty_ok() {
        let result = validate_has_ts_files(&[PathBuf::from("foo.ts")], Path::new("/some/dir"));
        assert!(result.is_ok());
    }
}
