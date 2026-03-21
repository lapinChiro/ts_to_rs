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
    // Replace hyphens with underscores in all path components (Rust module names cannot contain hyphens)
    let sanitized: PathBuf = relative
        .iter()
        .map(|component| {
            let s = component.to_string_lossy();
            std::ffi::OsString::from(s.replace('-', "_"))
        })
        .collect();
    let rs_relative = sanitized.with_extension("rs");
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

/// Collects all directories that need `mod.rs` generation.
///
/// A directory is included if it contains `.rs` files directly, or if any of
/// its subdirectories (recursively) contain `.rs` files. This ensures that
/// intermediate directories (e.g., `adapter/` which has no `.rs` files but has
/// subdirectories like `adapter/bun/`) are also included.
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

/// Returns `true` if this directory (or any descendant) contains `.rs` files.
fn collect_output_dirs_recursive(dir: &Path, dirs: &mut Vec<PathBuf>) -> Result<bool> {
    if !dir.is_dir() {
        return Ok(false);
    }

    let mut has_rs_files = false;
    let mut has_rs_descendants = false;
    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("failed to read directory: {}", dir.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            if collect_output_dirs_recursive(&path, dirs)? {
                has_rs_descendants = true;
            }
        } else if path.is_file() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.ends_with(".rs") && name_str != "mod.rs" {
                has_rs_files = true;
            }
        }
    }

    if has_rs_files || has_rs_descendants {
        dirs.push(dir.to_path_buf());
    }

    Ok(has_rs_files || has_rs_descendants)
}

/// Compute default output directory path by appending `_rs` suffix.
///
/// Example: `"path/to/src"` → `"path/to/src_rs"`
pub fn default_output_dir(input_dir: &Path) -> PathBuf {
    let mut name = input_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    name.push_str("_rs");
    input_dir.with_file_name(name)
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
    fn test_compute_output_path_hyphen_to_underscore() {
        let result = compute_output_path(
            Path::new("src/hono-base.ts"),
            Path::new("src"),
            Path::new("out"),
        )
        .unwrap();
        assert_eq!(result, PathBuf::from("out/hono_base.rs"));
    }

    #[test]
    fn test_compute_output_path_nested_hyphen() {
        let result = compute_output_path(
            Path::new("src/my-dir/some-file.ts"),
            Path::new("src"),
            Path::new("out"),
        )
        .unwrap();
        assert_eq!(result, PathBuf::from("out/my_dir/some_file.rs"));
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
    fn test_default_output_dir_appends_rs_suffix() {
        let result = default_output_dir(Path::new("path/to/src"));
        assert_eq!(result, PathBuf::from("path/to/src_rs"));
    }

    #[test]
    fn test_default_output_dir_root_path() {
        let result = default_output_dir(Path::new("/"));
        assert_eq!(result, PathBuf::from("/_rs"));
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

    #[test]
    fn test_collect_output_dirs_includes_intermediate_dirs() {
        // Structure: root/adapter/bun/server.rs
        // adapter/ has no .rs files directly, only subdirectories.
        // collect_output_dirs should include adapter/ so mod.rs is generated there.
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        let bun_dir = root.join("adapter").join("bun");
        fs::create_dir_all(&bun_dir).unwrap();
        fs::write(bun_dir.join("server.rs"), "").unwrap();

        let dirs = collect_output_dirs(root).unwrap();
        let relative_dirs: Vec<String> = dirs
            .iter()
            .map(|d| d.strip_prefix(root).unwrap().to_string_lossy().into_owned())
            .collect();

        assert!(
            relative_dirs.contains(&"adapter/bun".to_string()),
            "should include adapter/bun (has .rs files)"
        );
        assert!(
            relative_dirs.contains(&"adapter".to_string()),
            "should include adapter (intermediate dir with subdirectories)"
        );
    }

    #[test]
    fn test_collect_output_dirs_three_level_nesting() {
        // Structure: root/a/b/c/file.rs
        // All intermediate dirs (a, a/b, a/b/c) should be included.
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        let deep = root.join("a").join("b").join("c");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("file.rs"), "").unwrap();

        let dirs = collect_output_dirs(root).unwrap();
        let relative_dirs: Vec<String> = dirs
            .iter()
            .map(|d| d.strip_prefix(root).unwrap().to_string_lossy().into_owned())
            .collect();

        assert!(relative_dirs.contains(&"a/b/c".to_string()));
        assert!(relative_dirs.contains(&"a/b".to_string()));
        assert!(relative_dirs.contains(&"a".to_string()));
    }

    #[test]
    fn test_collect_output_dirs_bottom_up_order() {
        // Directories should be in bottom-up order (deepest first)
        // so mod.rs in children exists before parent references them.
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        let deep = root.join("a").join("b");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("file.rs"), "").unwrap();
        fs::write(root.join("top.rs"), "").unwrap();

        let dirs = collect_output_dirs(root).unwrap();
        let relative_dirs: Vec<String> = dirs
            .iter()
            .map(|d| d.strip_prefix(root).unwrap().to_string_lossy().into_owned())
            .collect();

        let idx_ab = relative_dirs.iter().position(|d| d == "a/b").unwrap();
        let idx_a = relative_dirs.iter().position(|d| d == "a").unwrap();
        let idx_root = relative_dirs.iter().position(|d| d.is_empty()).unwrap();

        assert!(idx_ab < idx_a, "a/b should come before a (bottom-up order)");
        assert!(
            idx_a < idx_root,
            "a should come before root (bottom-up order)"
        );
    }
}
