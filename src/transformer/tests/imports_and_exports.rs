use super::*;

#[test]
fn test_transform_module_import_single() {
    let source = r#"import { Foo } from "./bar";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Private,
            path: "crate::bar".to_string(),
            names: vec!["Foo".to_string()],
        }
    );
}

#[test]
fn test_transform_module_import_multiple() {
    let source = r#"import { A, B } from "./bar";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Private,
            path: "crate::bar".to_string(),
            names: vec!["A".to_string(), "B".to_string()],
        }
    );
}

#[test]
fn test_transform_module_import_nested_path() {
    let source = r#"import { Foo } from "./sub/bar";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Private,
            path: "crate::sub::bar".to_string(),
            names: vec!["Foo".to_string()],
        }
    );
}

#[test]
fn test_transform_module_import_hyphen_to_underscore() {
    let source = r#"import { Foo } from "./hono-base";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Private,
            path: "crate::hono_base".to_string(),
            names: vec!["Foo".to_string()],
        }
    );
}

#[test]
fn test_transform_module_import_nested_hyphen_path() {
    let source = r#"import { StatusCode } from "./utils/http-status";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Private,
            path: "crate::utils::http_status".to_string(),
            names: vec!["StatusCode".to_string()],
        }
    );
}

#[test]
fn test_transform_module_import_multiple_hyphens() {
    let source = r#"import { Foo } from "./my-long-name";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Private,
            path: "crate::my_long_name".to_string(),
            names: vec!["Foo".to_string()],
        }
    );
}

#[test]
fn test_transform_module_export_named_reexport_single() {
    let source = r#"export { Foo } from "./bar";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Public,
            path: "crate::bar".to_string(),
            names: vec!["Foo".to_string()],
        }
    );
}

#[test]
fn test_transform_module_export_named_reexport_multiple() {
    let source = r#"export { Foo, Bar } from "./baz";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Public,
            path: "crate::baz".to_string(),
            names: vec!["Foo".to_string(), "Bar".to_string()],
        }
    );
}

#[test]
fn test_transform_module_export_named_local_skipped() {
    let source = r#"
interface Foo { name: string; }
export { Foo };
"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    // Only the interface should be converted; the export { Foo } should be skipped
    assert_eq!(items.len(), 1);
    assert!(matches!(&items[0], Item::Struct { name, .. } if name == "Foo"));
}

#[test]
fn test_transform_module_import_external_skipped() {
    let source = r#"import { Foo } from "lodash";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert!(items.is_empty());
}

// ---- export * ----

#[test]
fn test_transform_module_export_all_relative() {
    let source = r#"export * from "./utils";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Public,
            path: "crate::utils".to_string(),
            names: vec!["*".to_string()],
        }
    );
}

#[test]
fn test_transform_module_export_all_external_skipped() {
    let source = r#"export * from "some-package";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();
    assert!(items.is_empty());
}

// --- D1: import resolution with ModuleGraph ---

#[test]
fn test_transform_import_module_graph_fallback_when_empty_graph() {
    // ModuleGraph::empty() (single-file mode) → falls back to convert_relative_path_to_crate_path
    let source = r#"import { Foo } from "./bar";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Private,
            path: "crate::bar".to_string(),
            names: vec!["Foo".to_string()],
        }
    );
}

#[test]
fn test_transform_import_module_graph_resolves_import_path() {
    // When ModuleGraph can resolve the import, it should use the resolved path
    // instead of convert_relative_path_to_crate_path
    use crate::pipeline::ModuleGraphBuilder;

    let root = std::path::Path::new("");
    let file_a = std::path::PathBuf::from("adapter/server.ts");
    let file_b = std::path::PathBuf::from("types.ts");
    let source_a = r#"import { Config } from "../types";"#;
    let source_b = r#"export interface Config { port: number; }"#;

    let known_files: std::collections::HashSet<std::path::PathBuf> = [
        std::path::PathBuf::from("adapter/server.ts"),
        std::path::PathBuf::from("types.ts"),
    ]
    .into_iter()
    .collect();

    let parsed = crate::pipeline::parse_files(vec![
        (file_a.clone(), source_a.to_string()),
        (file_b.clone(), source_b.to_string()),
    ])
    .unwrap();

    let resolver =
        crate::pipeline::module_resolver::NodeModuleResolver::new(root.to_path_buf(), known_files);
    let module_graph = ModuleGraphBuilder::new(&parsed, &resolver, root).build();

    let reg = TypeRegistry::new();
    let res = crate::pipeline::type_resolution::FileTypeResolution::empty();
    let tctx =
        crate::transformer::context::TransformContext::new(&module_graph, &reg, &res, &file_a);

    let mut synthetic = crate::pipeline::SyntheticTypeRegistry::new();
    let items = crate::transformer::transform_module_with_context(
        &parsed.files[0].module,
        &tctx,
        &mut synthetic,
    )
    .unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Private,
            path: "crate::types".to_string(),
            names: vec!["Config".to_string()],
        }
    );
}

#[test]
fn test_transform_import_module_graph_resolves_reexport_chain() {
    // When B re-exports from C, importing from B should resolve to C's module path
    use crate::pipeline::ModuleGraphBuilder;

    let root = std::path::Path::new("");
    let file_a = std::path::PathBuf::from("app.ts");
    let file_b = std::path::PathBuf::from("index.ts");
    let file_c = std::path::PathBuf::from("types.ts");
    let source_a = r#"import { Config } from "./index";"#;
    let source_b = r#"export { Config } from "./types";"#;
    let source_c = r#"export interface Config { port: number; }"#;

    let known_files: std::collections::HashSet<std::path::PathBuf> = [
        std::path::PathBuf::from("app.ts"),
        std::path::PathBuf::from("index.ts"),
        std::path::PathBuf::from("types.ts"),
    ]
    .into_iter()
    .collect();

    let parsed = crate::pipeline::parse_files(vec![
        (file_a.clone(), source_a.to_string()),
        (file_b.clone(), source_b.to_string()),
        (file_c.clone(), source_c.to_string()),
    ])
    .unwrap();

    let resolver =
        crate::pipeline::module_resolver::NodeModuleResolver::new(root.to_path_buf(), known_files);
    let module_graph = ModuleGraphBuilder::new(&parsed, &resolver, root).build();

    let reg = TypeRegistry::new();
    let res = crate::pipeline::type_resolution::FileTypeResolution::empty();
    let tctx =
        crate::transformer::context::TransformContext::new(&module_graph, &reg, &res, &file_a);

    let mut synthetic = crate::pipeline::SyntheticTypeRegistry::new();
    let items = crate::transformer::transform_module_with_context(
        &parsed.files[0].module,
        &tctx,
        &mut synthetic,
    )
    .unwrap();

    assert_eq!(items.len(), 1);
    // Config should resolve to crate::types (where it's originally defined),
    // NOT crate (where index.ts re-exports it from)
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Private,
            path: "crate::types".to_string(),
            names: vec!["Config".to_string()],
        }
    );
}
