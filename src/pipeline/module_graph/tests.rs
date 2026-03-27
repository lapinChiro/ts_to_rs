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
    let result = graph.resolve_import(Path::new("adapter/bun/index.ts"), "../../helper/types", "*");
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
