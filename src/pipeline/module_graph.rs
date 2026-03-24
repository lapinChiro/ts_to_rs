//! Module graph construction and query API.
//!
//! Builds a graph of module relationships from parsed TypeScript ASTs,
//! resolving re-export chains and mapping file paths to Rust module paths.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use swc_ecma_ast::{Decl, ExportSpecifier, ModuleDecl, ModuleExportName, ModuleItem};

use super::types::{ModuleResolver, ParsedFiles};

/// Rust reserved keywords that require `r#` prefix when used as module names.
const RUST_KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for",
    "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where",
    "while", "async", "await", "dyn",
];

/// The origin of an exported name, tracing through re-export chains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportOrigin {
    /// The Rust module path where the name is originally defined.
    pub module_path: String,
    /// The name as defined at the origin.
    pub name: String,
}

/// A resolved import with its Rust module path and name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedImport {
    /// The Rust module path of the defining module.
    pub module_path: String,
    /// The imported name.
    pub name: String,
}

/// Immutable module graph built from parsed TypeScript files.
///
/// Provides query APIs for resolving imports, looking up module paths,
/// and traversing the module hierarchy.
pub struct ModuleGraph {
    /// TS file path (relative to root) -> Rust module path (e.g., "crate::adapter::bun::server").
    file_to_module: HashMap<PathBuf, String>,
    /// Per-file exports: file path -> (export name -> origin).
    exports: HashMap<PathBuf, HashMap<String, ExportOrigin>>,
    /// The root directory for path calculations.
    root_dir: PathBuf,
    /// Module resolver for import specifier resolution.
    resolver: Box<dyn ModuleResolver>,
}

/// Categorized export information collected from AST.
#[derive(Debug, Clone)]
enum RawExport {
    /// Locally defined export (function, class, interface, etc.).
    Local { name: String },
    /// Re-export from another module: `export { name } from './source'`.
    ReExport {
        name: String,
        source_file: PathBuf,
        original_name: String,
    },
    /// Wildcard re-export: `export * from './source'`.
    WildcardReExport { source_file: PathBuf },
}

/// Builds a [`ModuleGraph`] from parsed files and a module resolver.
pub struct ModuleGraphBuilder<'a> {
    parsed_files: &'a ParsedFiles,
    resolver: &'a dyn ModuleResolver,
    root_dir: &'a Path,
}

impl<'a> ModuleGraphBuilder<'a> {
    /// Creates a new builder.
    pub fn new(
        parsed_files: &'a ParsedFiles,
        resolver: &'a dyn ModuleResolver,
        root_dir: &'a Path,
    ) -> Self {
        Self {
            parsed_files,
            resolver,
            root_dir,
        }
    }

    /// Builds the module graph by analyzing all files' ASTs.
    ///
    /// Steps:
    /// 1. Compute file-to-module mapping for all files
    /// 2. Collect raw exports from each file's AST
    /// 3. Resolve re-export chains to find original definitions
    pub fn build(self) -> ModuleGraph {
        let file_to_module = self.build_file_to_module_map();
        let raw_exports = self.collect_raw_exports();
        let exports = self.resolve_export_chains(&raw_exports, &file_to_module);

        // Clone the resolver by creating a new boxed instance wrapping it
        // We need to store it for resolve_import queries at runtime.
        // Since ModuleResolver is a trait object, we store an indirect reference
        // by rebuilding the graph with the resolver info baked in.
        // For now, we create a snapshot-based resolver using the file_to_module + exports.
        ModuleGraph {
            file_to_module,
            exports,
            root_dir: self.root_dir.to_path_buf(),
            resolver: Box::new(SnapshotResolver::new(
                self.resolver,
                self.parsed_files,
                self.root_dir,
            )),
        }
    }

    /// Builds the file path -> Rust module path mapping.
    fn build_file_to_module_map(&self) -> HashMap<PathBuf, String> {
        let mut map = HashMap::new();
        for file in &self.parsed_files.files {
            let rel_path = strip_root(&file.path, self.root_dir);
            let module_path = file_path_to_module_path(&rel_path);
            map.insert(file.path.clone(), module_path);
        }
        map
    }

    /// Collects raw export information from all files' ASTs.
    fn collect_raw_exports(&self) -> HashMap<PathBuf, Vec<RawExport>> {
        let mut all_exports = HashMap::new();
        for file in &self.parsed_files.files {
            let rel_path = strip_root(&file.path, self.root_dir);
            let exports = collect_file_exports(&file.module, &rel_path, self.resolver);
            all_exports.insert(file.path.clone(), exports);
        }
        all_exports
    }

    /// Resolves re-export chains to find the original definition module.
    fn resolve_export_chains(
        &self,
        raw_exports: &HashMap<PathBuf, Vec<RawExport>>,
        file_to_module: &HashMap<PathBuf, String>,
    ) -> HashMap<PathBuf, HashMap<String, ExportOrigin>> {
        let mut resolved: HashMap<PathBuf, HashMap<String, ExportOrigin>> = HashMap::new();

        // First pass: resolve local exports
        for (path, exports) in raw_exports {
            let module_path = match file_to_module.get(path) {
                Some(p) => p.clone(),
                None => continue,
            };
            let entry = resolved.entry(path.clone()).or_default();
            for export in exports {
                if let RawExport::Local { name } = export {
                    entry.insert(
                        name.clone(),
                        ExportOrigin {
                            module_path: module_path.clone(),
                            name: name.clone(),
                        },
                    );
                }
            }
        }

        // Second pass: resolve re-exports (iterate until stable)
        // Max iterations to prevent infinite loops in circular re-exports
        for _ in 0..20 {
            let mut changed = false;
            for (path, exports) in raw_exports {
                for export in exports {
                    match export {
                        RawExport::ReExport {
                            name,
                            source_file,
                            original_name,
                        } => {
                            // Look up the origin in the source file's resolved exports
                            let new_origin = resolved
                                .get(source_file)
                                .and_then(|m| m.get(original_name))
                                .cloned()
                                .or_else(|| {
                                    // Source might not be in our file set; use module path directly
                                    file_to_module.get(source_file).map(|mp| ExportOrigin {
                                        module_path: mp.clone(),
                                        name: original_name.clone(),
                                    })
                                });
                            if let Some(new_origin) = new_origin {
                                let entry = resolved.entry(path.clone()).or_default();
                                let should_update = entry.get(name) != Some(&new_origin);
                                if should_update {
                                    entry.insert(name.clone(), new_origin);
                                    changed = true;
                                }
                            }
                        }
                        RawExport::WildcardReExport { source_file } => {
                            // Copy all resolved exports from the source file
                            let source_exports = resolved.get(source_file).cloned();
                            if let Some(source_exports) = source_exports {
                                let entry = resolved.entry(path.clone()).or_default();
                                for (name, origin) in source_exports {
                                    if let std::collections::hash_map::Entry::Vacant(e) =
                                        entry.entry(name)
                                    {
                                        e.insert(origin);
                                        changed = true;
                                    }
                                }
                            }
                        }
                        RawExport::Local { .. } => {} // Already handled
                    }
                }
            }
            if !changed {
                break;
            }
        }

        resolved
    }
}

impl ModuleGraph {
    /// Creates an empty module graph with a `TrivialResolver`.
    ///
    /// Used for single-file mode and testing. The `TrivialResolver` resolves
    /// relative import specifiers to file paths by path manipulation,
    /// allowing `resolve_import()` to compute module paths dynamically
    /// without requiring parsed target files.
    pub fn empty() -> Self {
        Self {
            file_to_module: HashMap::new(),
            exports: HashMap::new(),
            root_dir: PathBuf::new(),
            resolver: Box::new(super::module_resolver::TrivialResolver),
        }
    }

    /// Resolves an import from a file to its original definition.
    ///
    /// Uses the module resolver to find the target file, then looks up
    /// the name in that file's exports to trace through re-export chains.
    pub fn resolve_import(
        &self,
        from_file: &Path,
        specifier: &str,
        name: &str,
    ) -> Option<ResolvedImport> {
        let target_path = self.resolver.resolve(from_file, specifier)?;

        // Wildcard exports (`export * from './types'`) don't have named entries
        // in the exports map. Return the target file's module path directly.
        if name != "*" {
            // Check exports map for re-export chain resolution
            if let Some(exports) = self.exports.get(&target_path) {
                if let Some(origin) = exports.get(name) {
                    return Some(ResolvedImport {
                        module_path: origin.module_path.clone(),
                        name: origin.name.clone(),
                    });
                }
            }
        }

        // Use the target file's module path from the file_to_module map,
        // or compute it dynamically for files not in the parsed set
        // (e.g., single-file mode where import targets are not parsed).
        let module_path = self
            .file_to_module
            .get(&target_path)
            .cloned()
            .unwrap_or_else(|| {
                let rel = strip_root(&target_path, &self.root_dir);
                file_path_to_module_path(&rel)
            });
        Some(ResolvedImport {
            module_path,
            name: name.to_string(),
        })
    }

    /// Returns the Rust module path for a file.
    pub fn module_path(&self, file: &Path) -> Option<&str> {
        self.file_to_module.get(file).map(|s| s.as_str())
    }

    /// Lists child module names of a directory.
    ///
    /// Returns the immediate child module names (not full paths) for modules
    /// whose parent directory matches `dir`. The `dir` path is relative to root.
    pub fn children_of(&self, dir: &Path) -> Vec<String> {
        let mut children: Vec<String> = Vec::new();
        let dir_rel = strip_root(dir, &self.root_dir);

        for file_path in self.file_to_module.keys() {
            let rel = strip_root(file_path, &self.root_dir);
            let parent = rel.parent().unwrap_or(Path::new(""));

            // Check if this file is a direct child of the target directory
            if parent == dir_rel {
                // Extract the module name (stem of the file)
                let stem = rel.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if stem == "index" {
                    // index.ts maps to the parent, not a child
                    continue;
                }
                let module_name = sanitize_module_name(stem);
                if !children.contains(&module_name) {
                    children.push(module_name);
                }
            }

            // Also check for subdirectories (files whose grandparent is the target)
            // to detect directory modules
            if let Some(grandparent) = parent.parent() {
                if grandparent == dir_rel {
                    let dir_name = parent.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    let module_name = sanitize_module_name(dir_name);
                    if !children.contains(&module_name) {
                        children.push(module_name);
                    }
                }
            }
        }

        children.sort();
        children
    }

    /// Lists re-exports from a file.
    pub fn reexports_of(&self, file: &Path) -> Vec<ResolvedImport> {
        let mut result = Vec::new();
        let file_module = match self.file_to_module.get(file) {
            Some(m) => m,
            None => return result,
        };
        if let Some(exports) = self.exports.get(file) {
            for (name, origin) in exports {
                // A re-export is an export whose origin is in a different module
                if &origin.module_path != file_module {
                    result.push(ResolvedImport {
                        module_path: origin.module_path.clone(),
                        name: name.clone(),
                    });
                }
            }
        }
        result.sort_by(|a, b| a.name.cmp(&b.name));
        result
    }
}

/// Snapshot-based resolver that wraps a `ModuleResolver` for use after building.
///
/// Stores resolved paths at build time so the graph can resolve imports
/// without holding a reference to the original resolver.
struct SnapshotResolver {
    /// Pre-resolved import paths: (from_file_rel, specifier) -> absolute target path.
    cache: HashMap<(PathBuf, String), PathBuf>,
}

impl std::fmt::Debug for SnapshotResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SnapshotResolver")
            .field("cache_size", &self.cache.len())
            .finish()
    }
}

impl SnapshotResolver {
    /// Builds a snapshot by pre-resolving all imports found in parsed files.
    ///
    /// Cache keys use root-relative paths (via `strip_root`) to ensure consistency
    /// with `resolve_import()`, which also strips the root before querying.
    fn new(resolver: &dyn ModuleResolver, parsed_files: &ParsedFiles, _root_dir: &Path) -> Self {
        let mut cache = HashMap::new();
        for file in &parsed_files.files {
            for item in &file.module.body {
                let specifier: Option<String> = match item {
                    ModuleItem::ModuleDecl(ModuleDecl::Import(import)) => {
                        Some(import.src.value.to_string_lossy().into_owned())
                    }
                    ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(named)) => named
                        .src
                        .as_ref()
                        .map(|s| s.value.to_string_lossy().into_owned()),
                    ModuleItem::ModuleDecl(ModuleDecl::ExportAll(all)) => {
                        Some(all.src.value.to_string_lossy().into_owned())
                    }
                    _ => None,
                };
                if let Some(spec) = specifier {
                    if let Some(target) = resolver.resolve(&file.path, &spec) {
                        cache.insert((file.path.clone(), spec), target);
                    }
                }
            }
        }
        Self { cache }
    }
}

impl ModuleResolver for SnapshotResolver {
    fn resolve(&self, from_file: &Path, specifier: &str) -> Option<PathBuf> {
        self.cache
            .get(&(from_file.to_path_buf(), specifier.to_string()))
            .cloned()
    }
}

/// Strips the root directory prefix from a path, returning a relative path.
fn strip_root(path: &Path, root: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

/// Converts a relative file path to a Rust module path.
///
/// Rules:
/// - Strips `.ts` extension
/// - Replaces hyphens with underscores
/// - Replaces path separators with `::`
/// - Prepends `crate::`
/// - `index.ts` maps to parent module
/// - Rust keywords get `r#` prefix
fn file_path_to_module_path(rel_path: &Path) -> String {
    let without_ext = rel_path.with_extension("");
    let components: Vec<String> = without_ext
        .components()
        .map(|c| {
            let s = c.as_os_str().to_str().unwrap_or("");
            sanitize_module_name(s)
        })
        .collect();

    // Handle index.ts -> parent module
    let components = if components.last().map(|s| s.as_str()) == Some("index") {
        &components[..components.len() - 1]
    } else {
        &components
    };

    if components.is_empty() {
        "crate".to_string()
    } else {
        format!("crate::{}", components.join("::"))
    }
}

/// Sanitizes a single module name component.
///
/// Replaces hyphens with underscores and prefixes Rust keywords with `r#`.
fn sanitize_module_name(name: &str) -> String {
    let sanitized = name.replace('-', "_");
    if RUST_KEYWORDS.contains(&sanitized.as_str()) {
        format!("r#{sanitized}")
    } else {
        sanitized
    }
}

/// Collects raw export information from a single file's AST.
fn collect_file_exports(
    module: &swc_ecma_ast::Module,
    rel_path: &Path,
    resolver: &dyn ModuleResolver,
) -> Vec<RawExport> {
    let mut exports = Vec::new();

    for item in &module.body {
        match item {
            // `export function foo() {}`, `export class Foo {}`, etc.
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export_decl)) => {
                if let Some(name) = decl_name(&export_decl.decl) {
                    exports.push(RawExport::Local { name });
                }
            }
            // `export { Foo } from './bar'` or `export { Foo }`
            ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(named)) => {
                match &named.src {
                    Some(src) => {
                        // Re-export from another module
                        let specifier = src.value.to_string_lossy().into_owned();
                        let target = resolver.resolve(rel_path, &specifier);
                        if let Some(target_path) = target {
                            for spec in &named.specifiers {
                                if let ExportSpecifier::Named(n) = spec {
                                    let orig_name = module_export_name_to_string(&n.orig);
                                    let exported_name = n
                                        .exported
                                        .as_ref()
                                        .map(module_export_name_to_string)
                                        .unwrap_or_else(|| orig_name.clone());
                                    exports.push(RawExport::ReExport {
                                        name: exported_name,
                                        source_file: target_path.clone(),
                                        original_name: orig_name,
                                    });
                                }
                            }
                        }
                    }
                    None => {
                        // Local export: `export { Foo }` — these reference local declarations
                        for spec in &named.specifiers {
                            if let ExportSpecifier::Named(n) = spec {
                                let name = module_export_name_to_string(&n.orig);
                                exports.push(RawExport::Local { name });
                            }
                        }
                    }
                }
            }
            // `export * from './sub'`
            ModuleItem::ModuleDecl(ModuleDecl::ExportAll(all)) => {
                let specifier = all.src.value.to_string_lossy().into_owned();
                let target = resolver.resolve(rel_path, &specifier);
                if let Some(target_path) = target {
                    exports.push(RawExport::WildcardReExport {
                        source_file: target_path,
                    });
                }
            }
            _ => {}
        }
    }

    exports
}

/// Extracts the declared name from a declaration.
fn decl_name(decl: &Decl) -> Option<String> {
    match decl {
        Decl::Class(c) => Some(c.ident.sym.to_string()),
        Decl::Fn(f) => Some(f.ident.sym.to_string()),
        Decl::Var(var) => {
            // Take the first declarator's name
            var.decls.first().and_then(|d| match &d.name {
                swc_ecma_ast::Pat::Ident(ident) => Some(ident.id.sym.to_string()),
                _ => None,
            })
        }
        Decl::TsInterface(i) => Some(i.id.sym.to_string()),
        Decl::TsTypeAlias(t) => Some(t.id.sym.to_string()),
        Decl::TsEnum(e) => Some(e.id.sym.to_string()),
        Decl::TsModule(m) => match &m.id {
            swc_ecma_ast::TsModuleName::Ident(ident) => Some(ident.sym.to_string()),
            swc_ecma_ast::TsModuleName::Str(s) => Some(s.value.to_string_lossy().into_owned()),
        },
        Decl::Using(_) => None,
    }
}

/// Extracts a string from a `ModuleExportName`.
fn module_export_name_to_string(name: &ModuleExportName) -> String {
    match name {
        ModuleExportName::Ident(ident) => ident.sym.to_string(),
        ModuleExportName::Str(s) => s.value.to_string_lossy().into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::types::ParsedFile;
    use std::collections::HashSet;

    /// Test module resolver that resolves based on known file mappings.
    struct TestResolver {
        known_files: HashSet<PathBuf>,
    }

    impl TestResolver {
        fn new(files: &[&str]) -> Self {
            Self {
                known_files: files.iter().map(PathBuf::from).collect(),
            }
        }
    }

    impl ModuleResolver for TestResolver {
        fn resolve(&self, from_file: &Path, specifier: &str) -> Option<PathBuf> {
            if !specifier.starts_with("./") && !specifier.starts_with("../") {
                return None;
            }
            let from_dir = from_file.parent().unwrap_or(Path::new(""));
            let joined = from_dir.join(specifier);
            let normalized = normalize_test_path(&joined);

            // Try with .ts extension
            let with_ts = normalized.with_extension("ts");
            if self.known_files.contains(&with_ts) {
                return Some(with_ts);
            }
            // Try index.ts
            let index_ts = normalized.join("index.ts");
            if self.known_files.contains(&index_ts) {
                return Some(index_ts);
            }
            // Exact match
            if self.known_files.contains(&normalized) {
                return Some(normalized);
            }
            None
        }
    }

    /// Normalize path for tests (resolve `.` and `..`).
    fn normalize_test_path(path: &Path) -> PathBuf {
        let mut components = Vec::new();
        for component in path.components() {
            match component {
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    components.pop();
                }
                _ => {
                    components.push(component);
                }
            }
        }
        components.iter().collect()
    }

    fn parse_ts(source: &str) -> swc_ecma_ast::Module {
        crate::parser::parse_typescript(source).expect("failed to parse test TS")
    }

    fn make_parsed_files(files: &[(&str, &str)]) -> ParsedFiles {
        ParsedFiles {
            files: files
                .iter()
                .map(|(path, src)| ParsedFile {
                    path: PathBuf::from(path),
                    source: src.to_string(),
                    module: parse_ts(src),
                })
                .collect(),
        }
    }

    fn build_graph(files: &[(&str, &str)]) -> ModuleGraph {
        let known: Vec<&str> = files.iter().map(|(p, _)| *p).collect();
        let resolver = TestResolver::new(&known);
        let parsed = make_parsed_files(files);
        let root = Path::new("");
        ModuleGraphBuilder::new(&parsed, &resolver, root).build()
    }

    // --- file_to_module tests ---

    #[test]
    fn test_file_to_module_simple() {
        assert_eq!(file_path_to_module_path(Path::new("foo.ts")), "crate::foo");
    }

    #[test]
    fn test_file_to_module_nested() {
        assert_eq!(
            file_path_to_module_path(Path::new("adapter/bun/server.ts")),
            "crate::adapter::bun::server"
        );
    }

    #[test]
    fn test_file_to_module_hyphen() {
        assert_eq!(
            file_path_to_module_path(Path::new("hono-base.ts")),
            "crate::hono_base"
        );
    }

    #[test]
    fn test_file_to_module_index() {
        assert_eq!(
            file_path_to_module_path(Path::new("adapter/bun/index.ts")),
            "crate::adapter::bun"
        );
    }

    #[test]
    fn test_file_to_module_reserved_word() {
        assert_eq!(
            file_path_to_module_path(Path::new("r/mod/foo.ts")),
            "crate::r::r#mod::foo"
        );
    }

    #[test]
    fn test_file_to_module_root_index() {
        assert_eq!(file_path_to_module_path(Path::new("index.ts")), "crate");
    }

    // --- resolve_import tests ---

    #[test]
    fn test_resolve_import_simple() {
        let graph = build_graph(&[
            ("foo.ts", "import { Bar } from './bar';"),
            ("bar.ts", "export class Bar {}"),
        ]);
        let result = graph.resolve_import(Path::new("foo.ts"), "./bar", "Bar");
        assert_eq!(
            result,
            Some(ResolvedImport {
                module_path: "crate::bar".to_string(),
                name: "Bar".to_string(),
            })
        );
    }

    #[test]
    fn test_resolve_import_nested() {
        let graph = build_graph(&[
            (
                "adapter/bun/server.ts",
                "import { Context } from '../../context';",
            ),
            ("context.ts", "export class Context {}"),
        ]);
        let result = graph.resolve_import(
            Path::new("adapter/bun/server.ts"),
            "../../context",
            "Context",
        );
        assert_eq!(
            result,
            Some(ResolvedImport {
                module_path: "crate::context".to_string(),
                name: "Context".to_string(),
            })
        );
    }

    #[test]
    fn test_resolve_import_reexport() {
        let graph = build_graph(&[
            ("consumer.ts", "import { Foo } from './reexporter';"),
            ("reexporter.ts", "export { Foo } from './origin';"),
            ("origin.ts", "export class Foo {}"),
        ]);
        let result = graph.resolve_import(Path::new("consumer.ts"), "./reexporter", "Foo");
        assert_eq!(
            result,
            Some(ResolvedImport {
                module_path: "crate::origin".to_string(),
                name: "Foo".to_string(),
            })
        );
    }

    #[test]
    fn test_resolve_import_reexport_chain() {
        let graph = build_graph(&[
            ("consumer.ts", "import { X } from './a';"),
            ("a.ts", "export { X } from './b';"),
            ("b.ts", "export { X } from './c';"),
            ("c.ts", "export const X = 42;"),
        ]);
        let result = graph.resolve_import(Path::new("consumer.ts"), "./a", "X");
        assert_eq!(
            result,
            Some(ResolvedImport {
                module_path: "crate::c".to_string(),
                name: "X".to_string(),
            })
        );
    }

    #[test]
    fn test_resolve_import_export_all() {
        let graph = build_graph(&[
            ("consumer.ts", "import { Thing } from './barrel';"),
            ("barrel.ts", "export * from './sub';"),
            ("sub.ts", "export class Thing {}"),
        ]);
        let result = graph.resolve_import(Path::new("consumer.ts"), "./barrel", "Thing");
        assert_eq!(
            result,
            Some(ResolvedImport {
                module_path: "crate::sub".to_string(),
                name: "Thing".to_string(),
            })
        );
    }

    #[test]
    fn test_resolve_import_unknown_returns_none() {
        let graph = build_graph(&[
            ("foo.ts", "import { Bar } from './bar';"),
            ("bar.ts", "export class Baz {}"),
        ]);
        // "Nonexistent" is not exported by bar.ts, but we fallback to the module path
        // For truly unknown imports (unresolvable specifier), we return None
        let result = graph.resolve_import(Path::new("foo.ts"), "./nonexistent", "Bar");
        assert_eq!(result, None);
    }

    #[test]
    fn test_resolve_import_npm_returns_none() {
        let graph = build_graph(&[("foo.ts", "import { readFile } from 'fs';")]);
        let result = graph.resolve_import(Path::new("foo.ts"), "fs", "readFile");
        assert_eq!(result, None);
    }

    // --- module_path tests ---

    #[test]
    fn test_module_path_lookup() {
        let graph = build_graph(&[("adapter/bun/server.ts", "export class Server {}")]);
        assert_eq!(
            graph.module_path(Path::new("adapter/bun/server.ts")),
            Some("crate::adapter::bun::server")
        );
    }

    // --- children_of tests ---

    #[test]
    fn test_children_of_root() {
        let graph = build_graph(&[
            ("foo.ts", "export const A = 1;"),
            ("bar.ts", "export const B = 2;"),
            ("sub/index.ts", "export const C = 3;"),
        ]);
        let mut children = graph.children_of(Path::new(""));
        children.sort();
        assert_eq!(children, vec!["bar", "foo", "sub"]);
    }

    #[test]
    fn test_children_of_nested() {
        let graph = build_graph(&[
            ("adapter/bun/server.ts", "export class S {}"),
            ("adapter/bun/client.ts", "export class C {}"),
            ("adapter/deno/server.ts", "export class D {}"),
        ]);
        let mut children = graph.children_of(Path::new("adapter/bun"));
        children.sort();
        assert_eq!(children, vec!["client", "server"]);
    }

    // --- reexports_of tests ---

    #[test]
    fn test_reexports_of() {
        let graph = build_graph(&[
            ("barrel.ts", "export { Foo } from './origin';"),
            ("origin.ts", "export class Foo {}"),
        ]);
        let reexports = graph.reexports_of(Path::new("barrel.ts"));
        assert_eq!(
            reexports,
            vec![ResolvedImport {
                module_path: "crate::origin".to_string(),
                name: "Foo".to_string(),
            }]
        );
    }

    // --- sanitize_module_name tests ---

    #[test]
    fn test_sanitize_module_name_keyword() {
        assert_eq!(sanitize_module_name("mod"), "r#mod");
        assert_eq!(sanitize_module_name("type"), "r#type");
        assert_eq!(sanitize_module_name("fn"), "r#fn");
        assert_eq!(sanitize_module_name("self"), "r#self");
        assert_eq!(sanitize_module_name("super"), "r#super");
        assert_eq!(sanitize_module_name("async"), "r#async");
        assert_eq!(sanitize_module_name("await"), "r#await");
        assert_eq!(sanitize_module_name("dyn"), "r#dyn");
    }

    #[test]
    fn test_sanitize_module_name_hyphen() {
        assert_eq!(sanitize_module_name("hono-base"), "hono_base");
        assert_eq!(sanitize_module_name("my-module"), "my_module");
    }

    #[test]
    fn test_sanitize_module_name_normal() {
        assert_eq!(sanitize_module_name("foo"), "foo");
        assert_eq!(sanitize_module_name("bar_baz"), "bar_baz");
    }

    // --- resolve_import: wildcard (export *) ---

    #[test]
    fn test_resolve_import_wildcard_returns_target_module_path() {
        // `export * from './types'` should resolve to the target file's module path
        let graph = build_graph(&[
            ("index.ts", "export * from './types';"),
            ("types.ts", "export interface Foo {}"),
        ]);
        let result = graph.resolve_import(Path::new("index.ts"), "./types", "*");
        assert_eq!(
            result,
            Some(ResolvedImport {
                module_path: "crate::types".to_string(),
                name: "*".to_string(),
            })
        );
    }

    #[test]
    fn test_resolve_import_wildcard_nested_dir() {
        let graph = build_graph(&[
            (
                "adapter/bun/index.ts",
                "export * from '../../helper/types';",
            ),
            ("helper/types.ts", "export interface Config {}"),
        ]);
        let result =
            graph.resolve_import(Path::new("adapter/bun/index.ts"), "../../helper/types", "*");
        assert_eq!(
            result,
            Some(ResolvedImport {
                module_path: "crate::helper::types".to_string(),
                name: "*".to_string(),
            })
        );
    }

    // --- resolve_import: unknown target file (not in parsed files) ---

    #[test]
    fn test_resolve_import_unknown_file_computes_module_path() {
        // When the target file is resolved by the resolver but not in file_to_module,
        // resolve_import should compute the module path dynamically
        use crate::pipeline::module_resolver::TrivialResolver;

        let parsed = make_parsed_files(&[("app.ts", "import { Foo } from './unknown';")]);
        let resolver = TrivialResolver;
        let root = Path::new("");
        let graph = ModuleGraphBuilder::new(&parsed, &resolver, root).build();

        let result = graph.resolve_import(Path::new("app.ts"), "./unknown", "Foo");
        assert_eq!(
            result,
            Some(ResolvedImport {
                module_path: "crate::unknown".to_string(),
                name: "Foo".to_string(),
            })
        );
    }

    // --- absolute paths ---

    #[test]
    fn test_resolve_import_absolute_paths_export_all() {
        // Simulates directory mode with absolute paths (the production scenario).
        // In production, collect_ts_files() returns absolute paths and
        // known_files are also absolute paths.
        use crate::pipeline::module_resolver::NodeModuleResolver;

        let root = PathBuf::from("/tmp/project");
        let known: HashSet<PathBuf> = [
            "/tmp/project/helper/conninfo/index.ts",
            "/tmp/project/helper/conninfo/types.ts",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        let resolver = NodeModuleResolver::new(root.clone(), known);

        let parsed = make_parsed_files(&[
            (
                "/tmp/project/helper/conninfo/index.ts",
                "export * from './types';",
            ),
            (
                "/tmp/project/helper/conninfo/types.ts",
                "export interface ConnInfo {}",
            ),
        ]);

        let graph = ModuleGraphBuilder::new(&parsed, &resolver, &root).build();

        // resolve_import should work with absolute from_file
        let result = graph.resolve_import(
            Path::new("/tmp/project/helper/conninfo/index.ts"),
            "./types",
            "*",
        );
        assert_eq!(
            result,
            Some(ResolvedImport {
                module_path: "crate::helper::conninfo::types".to_string(),
                name: "*".to_string(),
            })
        );
    }

    #[test]
    fn test_resolve_import_absolute_paths_named_import() {
        use crate::pipeline::module_resolver::NodeModuleResolver;

        let root = PathBuf::from("/tmp/project");
        let known: HashSet<PathBuf> = [
            "/tmp/project/adapter/bun/server.ts",
            "/tmp/project/context.ts",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        let resolver = NodeModuleResolver::new(root.clone(), known);

        let parsed = make_parsed_files(&[
            (
                "/tmp/project/adapter/bun/server.ts",
                "import { Context } from '../../context';",
            ),
            ("/tmp/project/context.ts", "export class Context {}"),
        ]);

        let graph = ModuleGraphBuilder::new(&parsed, &resolver, &root).build();

        let result = graph.resolve_import(
            Path::new("/tmp/project/adapter/bun/server.ts"),
            "../../context",
            "Context",
        );
        assert_eq!(
            result,
            Some(ResolvedImport {
                module_path: "crate::context".to_string(),
                name: "Context".to_string(),
            })
        );
    }
}
