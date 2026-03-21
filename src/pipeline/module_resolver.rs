//! Module resolution strategies for import specifier → file path mapping.

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use super::types::ModuleResolver;

/// Node.js / Bundler-style module resolver.
///
/// Resolves relative import specifiers (`./foo`, `../bar`) to file paths,
/// with automatic `.ts` extension completion and `index.ts` directory resolution.
///
/// Only resolves to files within the `known_files` set. External packages
/// (non-relative specifiers like `"lodash"`) always return `None`.
pub struct NodeModuleResolver {
    /// Root directory for the conversion project.
    root_dir: PathBuf,
    /// Set of known `.ts` file paths (canonical, relative to root_dir).
    known_files: HashSet<PathBuf>,
}

impl NodeModuleResolver {
    /// Creates a new resolver.
    ///
    /// `root_dir` is the base directory for the project.
    /// `known_files` are the `.ts` files that exist in the project,
    /// stored as paths relative to `root_dir`.
    pub fn new(root_dir: PathBuf, known_files: HashSet<PathBuf>) -> Self {
        Self {
            root_dir,
            known_files,
        }
    }
}

impl ModuleResolver for NodeModuleResolver {
    fn resolve(&self, from_file: &Path, specifier: &str) -> Option<PathBuf> {
        // Only handle relative imports
        if !specifier.starts_with("./") && !specifier.starts_with("../") {
            return None;
        }

        // Compute the directory of the importing file (relative to root)
        let from_dir = from_file.parent().unwrap_or(Path::new(""));

        // Join the specifier path with the importing file's directory
        let joined = from_dir.join(specifier);

        // Normalize the path (resolve `.` and `..` without filesystem access)
        let normalized = normalize_path(&joined);

        // Try resolution strategies in order:
        // 1. Exact match with .ts extension
        let with_ts = normalized.with_extension("ts");
        if self.known_files.contains(&with_ts) {
            return Some(self.root_dir.join(&with_ts));
        }

        // 2. Directory with index.ts
        let index_ts = normalized.join("index.ts");
        if self.known_files.contains(&index_ts) {
            return Some(self.root_dir.join(&index_ts));
        }

        // 3. Exact match (specifier already has extension)
        if self.known_files.contains(&normalized) {
            return Some(self.root_dir.join(&normalized));
        }

        None
    }
}

/// Normalizes a path by resolving `.` and `..` components without filesystem access.
///
/// Example: `adapter/bun/../../helper/conninfo` → `helper/conninfo`
fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {} // skip `.`
            Component::ParentDir => {
                components.pop(); // go up one level
            }
            _ => {
                components.push(component);
            }
        }
    }
    components.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_resolver(files: &[&str]) -> NodeModuleResolver {
        let root = PathBuf::from("/project");
        let known: HashSet<PathBuf> = files.iter().map(PathBuf::from).collect();
        NodeModuleResolver::new(root, known)
    }

    // --- NodeModuleResolver tests ---

    #[test]
    fn test_resolve_relative_same_dir() {
        let resolver = make_resolver(&["foo.ts"]);
        let result = resolver.resolve(Path::new("bar.ts"), "./foo");
        assert_eq!(result, Some(PathBuf::from("/project/foo.ts")));
    }

    #[test]
    fn test_resolve_relative_same_dir_nested() {
        let resolver = make_resolver(&["adapter/bun/server.ts"]);
        let result = resolver.resolve(Path::new("adapter/bun/client.ts"), "./server");
        assert_eq!(
            result,
            Some(PathBuf::from("/project/adapter/bun/server.ts"))
        );
    }

    #[test]
    fn test_resolve_relative_parent() {
        let resolver = make_resolver(&["adapter/context.ts"]);
        let result = resolver.resolve(Path::new("adapter/bun/server.ts"), "../context");
        assert_eq!(result, Some(PathBuf::from("/project/adapter/context.ts")));
    }

    #[test]
    fn test_resolve_relative_grandparent() {
        // I-222 case: ../../helper/conninfo from adapter/bun/conninfo.ts
        let resolver = make_resolver(&["helper/conninfo.ts"]);
        let result = resolver.resolve(
            Path::new("adapter/bun/conninfo.ts"),
            "../../helper/conninfo",
        );
        assert_eq!(result, Some(PathBuf::from("/project/helper/conninfo.ts")));
    }

    #[test]
    fn test_resolve_index_ts_current_dir() {
        // I-222 case: ./ from helper/streaming/text.ts resolves to helper/streaming/index.ts
        let resolver = make_resolver(&["helper/streaming/index.ts"]);
        let result = resolver.resolve(Path::new("helper/streaming/text.ts"), "./");
        assert_eq!(
            result,
            Some(PathBuf::from("/project/helper/streaming/index.ts"))
        );
    }

    #[test]
    fn test_resolve_index_ts_parent_dir() {
        // ../.. from adapter/bun/conninfo.ts resolves to index.ts (root)
        let resolver = make_resolver(&["index.ts"]);
        let result = resolver.resolve(Path::new("adapter/bun/conninfo.ts"), "../..");
        assert_eq!(result, Some(PathBuf::from("/project/index.ts")));
    }

    #[test]
    fn test_resolve_extension_already_present() {
        let resolver = make_resolver(&["foo.ts"]);
        let result = resolver.resolve(Path::new("bar.ts"), "./foo.ts");
        assert_eq!(result, Some(PathBuf::from("/project/foo.ts")));
    }

    #[test]
    fn test_resolve_nonexistent_returns_none() {
        let resolver = make_resolver(&["foo.ts"]);
        let result = resolver.resolve(Path::new("bar.ts"), "./nonexistent");
        assert_eq!(result, None);
    }

    #[test]
    fn test_resolve_npm_package_returns_none() {
        let resolver = make_resolver(&["foo.ts"]);
        assert_eq!(resolver.resolve(Path::new("bar.ts"), "lodash"), None);
        assert_eq!(resolver.resolve(Path::new("bar.ts"), "node:fs"), None);
        assert_eq!(resolver.resolve(Path::new("bar.ts"), "@scope/pkg"), None);
    }

    #[test]
    fn test_resolve_subpath_with_index() {
        // ./helper/conninfo resolves to helper/conninfo/index.ts if no helper/conninfo.ts
        let resolver = make_resolver(&["helper/conninfo/index.ts"]);
        let result = resolver.resolve(Path::new("adapter.ts"), "./helper/conninfo");
        assert_eq!(
            result,
            Some(PathBuf::from("/project/helper/conninfo/index.ts"))
        );
    }

    #[test]
    fn test_resolve_prefers_file_over_directory() {
        // If both helper/conninfo.ts and helper/conninfo/index.ts exist,
        // prefer helper/conninfo.ts
        let resolver = make_resolver(&["helper/conninfo.ts", "helper/conninfo/index.ts"]);
        let result = resolver.resolve(Path::new("adapter.ts"), "./helper/conninfo");
        assert_eq!(result, Some(PathBuf::from("/project/helper/conninfo.ts")));
    }

    // --- normalize_path tests ---

    #[test]
    fn test_normalize_path_parent_dir() {
        assert_eq!(
            normalize_path(Path::new("adapter/bun/../../helper/conninfo")),
            PathBuf::from("helper/conninfo")
        );
    }

    #[test]
    fn test_normalize_path_current_dir() {
        assert_eq!(
            normalize_path(Path::new("helper/streaming/./text")),
            PathBuf::from("helper/streaming/text")
        );
    }

    #[test]
    fn test_normalize_path_to_root() {
        assert_eq!(
            normalize_path(Path::new("adapter/bun/../..")),
            PathBuf::from("")
        );
    }
}
