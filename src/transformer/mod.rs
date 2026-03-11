//! AST to IR transformation.
//!
//! This module converts SWC TypeScript AST nodes into the IR representation
//! defined in [`crate::ir`].

pub mod expressions;
pub mod functions;
pub mod statements;
pub mod types;

use anyhow::Result;
use swc_ecma_ast::{Decl, Module, ModuleDecl, ModuleItem, Stmt};

use crate::ir::Item;

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
                if let Some(item) = transform_decl(decl)? {
                    items.push(item);
                }
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => {
                if let Some(item) = transform_decl(&export.decl)? {
                    // Export declarations keep Public visibility (already default)
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

/// Transforms a single declaration into an IR [`Item`], if supported.
///
/// Returns `Ok(None)` for unsupported declarations (e.g., variable declarations).
fn transform_decl(decl: &Decl) -> Result<Option<Item>> {
    match decl {
        Decl::TsInterface(interface_decl) => {
            let item = types::convert_interface(interface_decl)?;
            Ok(Some(item))
        }
        Decl::TsTypeAlias(type_alias_decl) => {
            let item = types::convert_type_alias(type_alias_decl)?;
            Ok(Some(item))
        }
        Decl::Fn(fn_decl) => {
            let item = functions::convert_fn_decl(fn_decl)?;
            Ok(Some(item))
        }
        // Unsupported declarations are silently skipped for now
        _ => Ok(None),
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
    fn test_transform_module_single_interface() {
        let source = "interface Foo { name: string; age: number; }";
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0],
            Item::Struct {
                vis: Visibility::Public,
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
                vis: Visibility::Public,
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
