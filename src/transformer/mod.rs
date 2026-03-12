//! AST to IR transformation.
//!
//! This module converts SWC TypeScript AST nodes into the IR representation
//! defined in [`crate::ir`].

pub mod classes;
pub mod expressions;
pub mod functions;
pub mod statements;
pub mod types;

use anyhow::Result;
use swc_ecma_ast::{Decl, ImportSpecifier, Module, ModuleDecl, ModuleItem, Stmt};

use crate::ir::{Item, Visibility};

/// Transforms an SWC [`Module`] into a list of IR [`Item`]s.
///
/// Iterates over the module's top-level items and converts supported
/// declarations (interfaces, type aliases) into IR items. Unsupported
/// items are skipped with a warning (currently silent).
///
/// # Errors
///
/// Returns an error if a supported declaration fails to convert.
pub fn transform_module(module: &Module) -> Result<Vec<Item>> {
    let mut items = Vec::new();

    for module_item in &module.body {
        match module_item {
            ModuleItem::Stmt(Stmt::Decl(decl)) => {
                items.extend(transform_decl(decl, Visibility::Private)?);
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => {
                items.extend(transform_decl(&export.decl, Visibility::Public)?);
            }
            ModuleItem::ModuleDecl(ModuleDecl::Import(import_decl)) => {
                if let Some(item) = transform_import(import_decl) {
                    items.push(item);
                }
            }
            _ => {
                // Unsupported module items are silently skipped
            }
        }
    }

    Ok(items)
}

/// Transforms an import declaration into an IR [`Item::Use`], if applicable.
///
/// Only relative path imports with named specifiers are converted.
/// External package imports and non-named specifiers are skipped.
fn transform_import(import_decl: &swc_ecma_ast::ImportDecl) -> Option<Item> {
    let src = import_decl.src.value.to_string_lossy().into_owned();

    // Only handle relative imports
    if !src.starts_with("./") && !src.starts_with("../") {
        return None;
    }

    // Collect named specifiers only
    let names: Vec<String> = import_decl
        .specifiers
        .iter()
        .filter_map(|spec| match spec {
            ImportSpecifier::Named(named) => Some(named.local.sym.to_string()),
            _ => None,
        })
        .collect();

    if names.is_empty() {
        return None;
    }

    let path = convert_relative_path_to_crate_path(&src);
    Some(Item::Use { path, names })
}

/// Converts a relative TS import path to a Rust crate path.
///
/// Examples:
/// - `./foo` → `crate::foo`
/// - `./sub/bar` → `crate::sub::bar`
fn convert_relative_path_to_crate_path(rel_path: &str) -> String {
    let stripped = rel_path.strip_prefix("./").unwrap_or(rel_path);
    let parts: Vec<&str> = stripped.split('/').collect();
    format!("crate::{}", parts.join("::"))
}

/// Transforms a single declaration into IR [`Item`]s, if supported.
///
/// Returns `Ok` with an empty vec for unsupported declarations.
fn transform_decl(decl: &Decl, vis: Visibility) -> Result<Vec<Item>> {
    match decl {
        Decl::TsInterface(interface_decl) => {
            let item = types::convert_interface(interface_decl, vis)?;
            Ok(vec![item])
        }
        Decl::TsTypeAlias(type_alias_decl) => {
            let item = types::convert_type_alias(type_alias_decl, vis)?;
            Ok(vec![item])
        }
        Decl::Fn(fn_decl) => {
            let item = functions::convert_fn_decl(fn_decl, vis)?;
            Ok(vec![item])
        }
        Decl::Class(class_decl) => classes::convert_class_decl(class_decl, vis),
        // Unsupported declarations are silently skipped for now
        _ => Ok(vec![]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::Stmt;
    use crate::ir::{Expr, Param, RustType, StructField, Visibility};
    use crate::parser::parse_typescript;

    #[test]
    fn test_transform_module_empty() {
        let module = parse_typescript("").expect("parse failed");
        let items = transform_module(&module).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_transform_module_import_single() {
        let source = r#"import { Foo } from "./bar";"#;
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0],
            Item::Use {
                path: "crate::bar".to_string(),
                names: vec!["Foo".to_string()],
            }
        );
    }

    #[test]
    fn test_transform_module_import_multiple() {
        let source = r#"import { A, B } from "./bar";"#;
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0],
            Item::Use {
                path: "crate::bar".to_string(),
                names: vec!["A".to_string(), "B".to_string()],
            }
        );
    }

    #[test]
    fn test_transform_module_import_nested_path() {
        let source = r#"import { Foo } from "./sub/bar";"#;
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0],
            Item::Use {
                path: "crate::sub::bar".to_string(),
                names: vec!["Foo".to_string()],
            }
        );
    }

    #[test]
    fn test_transform_module_import_external_skipped() {
        let source = r#"import { Foo } from "lodash";"#;
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module).unwrap();

        assert!(items.is_empty());
    }

    #[test]
    fn test_transform_module_non_exported_is_private() {
        let source = "interface Foo { name: string; }";
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module).unwrap();

        assert_eq!(items.len(), 1);
        match &items[0] {
            Item::Struct { vis, .. } => assert_eq!(*vis, Visibility::Private),
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn test_transform_module_exported_is_public() {
        let source = "export interface Foo { name: string; }";
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module).unwrap();

        assert_eq!(items.len(), 1);
        match &items[0] {
            Item::Struct { vis, .. } => assert_eq!(*vis, Visibility::Public),
            _ => panic!("expected Struct"),
        }
    }

    #[test]
    fn test_transform_module_single_interface() {
        let source = "interface Foo { name: string; age: number; }";
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0],
            Item::Struct {
                vis: Visibility::Private,
                name: "Foo".to_string(),
                fields: vec![
                    StructField {
                        name: "name".to_string(),
                        ty: RustType::String,
                    },
                    StructField {
                        name: "age".to_string(),
                        ty: RustType::F64,
                    },
                ],
            }
        );
    }

    #[test]
    fn test_transform_module_multiple_interfaces() {
        let source = r#"
            interface Foo { name: string; }
            interface Bar { count: number; }
        "#;
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module).unwrap();

        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_transform_module_type_alias_object() {
        let source = "type Point = { x: number; y: number; };";
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module).unwrap();

        assert_eq!(items.len(), 1);
        match &items[0] {
            Item::Struct { name, .. } => assert_eq!(name, "Point"),
            _ => panic!("expected Item::Struct"),
        }
    }

    #[test]
    fn test_transform_module_skips_unsupported() {
        let source = r#"
            const x = 42;
            interface Foo { name: string; }
        "#;
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module).unwrap();

        // const x = 42 is skipped, only Foo is converted
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_transform_module_function_declaration() {
        let source = "function add(a: number, b: number): number { return a + b; }";
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0],
            Item::Fn {
                vis: Visibility::Private,
                name: "add".to_string(),
                params: vec![
                    Param {
                        name: "a".to_string(),
                        ty: RustType::F64,
                    },
                    Param {
                        name: "b".to_string(),
                        ty: RustType::F64,
                    },
                ],
                return_type: Some(RustType::F64),
                body: vec![Stmt::Return(Some(Expr::BinaryOp {
                    left: Box::new(Expr::Ident("a".to_string())),
                    op: "+".to_string(),
                    right: Box::new(Expr::Ident("b".to_string())),
                }))],
            }
        );
    }

    #[test]
    fn test_transform_module_mixed_items() {
        let source = r#"
            interface Foo { name: string; }
            function greet(name: string): string { return name; }
        "#;
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module).unwrap();

        assert_eq!(items.len(), 2);
        match &items[0] {
            Item::Struct { name, .. } => assert_eq!(name, "Foo"),
            _ => panic!("expected Item::Struct"),
        }
        match &items[1] {
            Item::Fn { name, .. } => assert_eq!(name, "greet"),
            _ => panic!("expected Item::Fn"),
        }
    }
}
