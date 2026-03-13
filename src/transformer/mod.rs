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
use swc_common::Spanned;
use swc_ecma_ast as ast;
use swc_ecma_ast::{Decl, ImportSpecifier, Module, ModuleDecl, ModuleItem, Stmt};

use std::collections::HashMap;

use crate::ir::{EnumValue, EnumVariant, Item, Visibility};
use crate::registry::TypeRegistry;
use crate::transformer::classes::ClassInfo;
use crate::transformer::expressions::convert_arrow_expr;
use crate::transformer::types::convert_ts_type;

/// Extracts the identifier name from a [`ast::Pat::Ident`] pattern.
///
/// Returns an error if the pattern is not an identifier binding.
pub fn extract_pat_ident_name(pat: &ast::Pat) -> Result<String> {
    match pat {
        ast::Pat::Ident(ident) => Ok(ident.id.sym.to_string()),
        _ => Err(anyhow::anyhow!("unsupported pattern: expected identifier")),
    }
}

/// Extracts the single declarator from a [`ast::VarDecl`].
///
/// Returns an error if the declaration contains zero or more than one declarator.
pub fn single_declarator(var_decl: &ast::VarDecl) -> Result<&ast::VarDeclarator> {
    if var_decl.decls.len() != 1 {
        return Err(anyhow::anyhow!(
            "multiple variable declarators in one statement are not supported"
        ));
    }
    Ok(&var_decl.decls[0])
}

/// Extracts the property name string from a [`ast::PropName::Ident`].
///
/// Returns an error if the property name is not a simple identifier.
pub fn extract_prop_name(prop_name: &ast::PropName) -> Result<String> {
    match prop_name {
        ast::PropName::Ident(ident) => Ok(ident.sym.to_string()),
        _ => Err(anyhow::anyhow!(
            "unsupported property name (only identifiers)"
        )),
    }
}

/// Error type for unsupported TypeScript syntax encountered during transformation.
///
/// Used to distinguish unsupported-syntax errors from other transformation errors,
/// enabling collection mode to gather all unsupported items without aborting.
#[derive(Debug, Clone)]
pub struct UnsupportedSyntaxError {
    /// The SWC AST node kind (e.g., `"ExportDefaultExpr"`, `"TsModuleDecl"`)
    pub kind: String,
    /// Byte offset (SWC `BytePos`) of the syntax in the source
    pub byte_pos: u32,
}

impl std::fmt::Display for UnsupportedSyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unsupported syntax: {}", self.kind)
    }
}

impl std::error::Error for UnsupportedSyntaxError {}

/// Transforms an SWC [`Module`] into a list of IR [`Item`]s.
///
/// Returns an error on unsupported syntax. Use [`transform_module_collecting`]
/// to collect unsupported items instead of aborting.
///
/// # Errors
///
/// Returns an error if transformation fails or unsupported syntax is encountered.
pub fn transform_module(module: &Module, reg: &TypeRegistry) -> Result<Vec<Item>> {
    // Pre-scan: collect class info for inheritance resolution
    let class_map = pre_scan_classes(module, reg);

    let mut items = Vec::new();
    for module_item in &module.body {
        let (converted, _warnings) = transform_module_item(module_item, reg, &class_map, false)?;
        items.extend(converted);
    }

    Ok(items)
}

/// Transforms an SWC [`Module`], collecting unsupported syntax instead of aborting.
///
/// Returns the converted items and a list of unsupported syntax entries.
/// All transformation errors — both [`UnsupportedSyntaxError`] and transformer-internal
/// errors (e.g., unsupported parameter patterns inside functions/classes) — are collected
/// at the top-level item granularity rather than propagated.
///
/// # Errors
///
/// Returns an error only if pre-processing (e.g., class pre-scan) fails fatally.
pub fn transform_module_collecting(
    module: &Module,
    reg: &TypeRegistry,
) -> Result<(Vec<Item>, Vec<UnsupportedSyntaxError>)> {
    let class_map = pre_scan_classes(module, reg);

    let mut items = Vec::new();
    let mut unsupported = Vec::new();

    for module_item in &module.body {
        match transform_module_item(module_item, reg, &class_map, true) {
            Ok((converted, warnings)) => {
                items.extend(converted);
                for warning in warnings {
                    unsupported.push(UnsupportedSyntaxError {
                        kind: warning,
                        byte_pos: module_item.span().lo.0,
                    });
                }
            }
            Err(e) => match e.downcast::<UnsupportedSyntaxError>() {
                Ok(unsup) => unsupported.push(unsup),
                Err(other) => {
                    // Transformer-internal errors (e.g. unsupported parameter patterns
                    // inside functions/classes) are collected instead of aborting.
                    unsupported.push(UnsupportedSyntaxError {
                        kind: other.to_string(),
                        byte_pos: module_item.span().lo.0,
                    });
                }
            },
        }
    }

    Ok((items, unsupported))
}

/// Pre-scans all class declarations in the module to collect inheritance info.
///
/// Returns a map from class name to [`ClassInfo`]. Only classes that can be
/// successfully parsed are included; parse failures are silently skipped
/// (they will be reported during the main transformation pass).
fn pre_scan_classes(module: &Module, reg: &TypeRegistry) -> HashMap<String, ClassInfo> {
    let mut map = HashMap::new();

    for module_item in &module.body {
        let (decl, vis) = match module_item {
            ModuleItem::Stmt(Stmt::Decl(Decl::Class(cd))) => (cd, Visibility::Private),
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => {
                if let Decl::Class(cd) = &export.decl {
                    (cd, Visibility::Public)
                } else {
                    continue;
                }
            }
            _ => continue,
        };
        if let Ok(info) = classes::extract_class_info(decl, vis, reg) {
            map.insert(info.name.clone(), info);
        }
    }

    map
}

/// Identifies which classes are parents (are extended by another class).
fn find_parent_class_names(
    class_map: &HashMap<String, ClassInfo>,
) -> std::collections::HashSet<String> {
    class_map
        .values()
        .filter_map(|info| info.parent.clone())
        .collect()
}

/// Transforms a single module item into IR [`Item`]s.
///
/// When `resilient` is true, type conversion failures in function parameters and
/// return types fall back to `RustType::Any` instead of aborting.
fn transform_module_item(
    module_item: &ModuleItem,
    reg: &TypeRegistry,
    class_map: &HashMap<String, ClassInfo>,
    resilient: bool,
) -> Result<(Vec<Item>, Vec<String>)> {
    match module_item {
        ModuleItem::Stmt(Stmt::Decl(decl)) => {
            transform_decl(decl, Visibility::Private, reg, class_map, resilient)
        }
        ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => {
            transform_decl(&export.decl, Visibility::Public, reg, class_map, resilient)
        }
        ModuleItem::ModuleDecl(ModuleDecl::Import(import_decl)) => {
            let items: Vec<Item> = transform_import(import_decl).into_iter().collect();
            Ok((items, vec![]))
        }
        _ => Err(UnsupportedSyntaxError {
            kind: format_module_item_kind(module_item),
            byte_pos: module_item.span().lo.0,
        }
        .into()),
    }
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
/// Hyphens in path segments are replaced with underscores to produce valid Rust identifiers.
///
/// Examples:
/// - `./foo` → `crate::foo`
/// - `./sub/bar` → `crate::sub::bar`
/// - `./hono-base` → `crate::hono_base`
fn convert_relative_path_to_crate_path(rel_path: &str) -> String {
    let stripped = rel_path.strip_prefix("./").unwrap_or(rel_path);
    let parts: Vec<String> = stripped
        .split('/')
        .map(|seg| seg.replace('-', "_"))
        .collect();
    format!("crate::{}", parts.join("::"))
}

/// Transforms a single declaration into IR [`Item`]s.
///
/// When `resilient` is true, type conversion failures in functions fall back to
/// `RustType::Any` instead of aborting.
///
/// # Errors
///
/// Returns an [`UnsupportedSyntaxError`] for unhandled declaration types.
fn transform_decl(
    decl: &Decl,
    vis: Visibility,
    reg: &TypeRegistry,
    class_map: &HashMap<String, ClassInfo>,
    resilient: bool,
) -> Result<(Vec<Item>, Vec<String>)> {
    match decl {
        Decl::TsInterface(interface_decl) => {
            let item = types::convert_interface(interface_decl, vis)?;
            Ok((vec![item], vec![]))
        }
        Decl::TsTypeAlias(type_alias_decl) => {
            let item = types::convert_type_alias(type_alias_decl, vis)?;
            Ok((vec![item], vec![]))
        }
        Decl::Fn(fn_decl) => {
            let (item, warnings) = functions::convert_fn_decl(fn_decl, vis, reg, resilient)?;
            Ok((vec![item], warnings))
        }
        Decl::Class(class_decl) => {
            let items = transform_class_with_inheritance(class_decl, vis, reg, class_map)?;
            Ok((items, vec![]))
        }
        Decl::Var(var_decl) => convert_var_decl_arrow_fns(var_decl, vis, reg, resilient),
        Decl::TsEnum(ts_enum) => {
            let items = convert_ts_enum(ts_enum, vis)?;
            Ok((items, vec![]))
        }
        _ => Err(UnsupportedSyntaxError {
            kind: format_decl_kind(decl),
            byte_pos: decl.span().lo.0,
        }
        .into()),
    }
}

/// Transforms a class declaration, handling inheritance if applicable.
///
/// - If the class is a parent (extended by another class): generates struct + trait + impls
/// - If the class is a child (extends another class): generates struct + impl + trait impl
/// - Otherwise: generates struct + impl (no trait)
fn transform_class_with_inheritance(
    class_decl: &ast::ClassDecl,
    vis: Visibility,
    reg: &TypeRegistry,
    class_map: &HashMap<String, ClassInfo>,
) -> Result<Vec<Item>> {
    let info = classes::extract_class_info(class_decl, vis, reg)?;
    let parent_names = find_parent_class_names(class_map);

    if parent_names.contains(&info.name) {
        // This class is a parent — generate struct + trait + impls
        classes::generate_parent_class_items(&info)
    } else if let Some(parent_name) = &info.parent {
        // This class is a child — generate struct + impl + trait impl
        let parent_info = class_map.get(parent_name);
        classes::generate_items_for_class(&info, parent_info)
    } else {
        // Standalone class — no inheritance
        classes::generate_items_for_class(&info, None)
    }
}

/// Converts `const` variable declarations with arrow function initializers into `Item::Fn`.
///
/// `const double = (x: number): number => x * 2;`
/// becomes `fn double(x: f64) -> f64 { x * 2.0 }`
///
/// Non-arrow-function variable declarations are skipped.
fn convert_var_decl_arrow_fns(
    var_decl: &swc_ecma_ast::VarDecl,
    vis: Visibility,
    reg: &TypeRegistry,
    resilient: bool,
) -> Result<(Vec<Item>, Vec<String>)> {
    let mut items = Vec::new();
    let mut all_warnings = Vec::new();
    for decl in &var_decl.decls {
        let init = match &decl.init {
            Some(init) => init,
            None => continue,
        };
        // Only handle arrow function initializers
        let arrow = match init.as_ref() {
            swc_ecma_ast::Expr::Arrow(arrow) => arrow,
            _ => continue,
        };
        let name = match extract_pat_ident_name(&decl.name) {
            Ok(n) => n,
            Err(_) => continue,
        };

        // Convert the arrow to a closure IR, then extract parts for Item::Fn
        let mut fallback_warnings = Vec::new();
        let closure = convert_arrow_expr(arrow, reg, resilient, &mut fallback_warnings)?;
        match closure {
            crate::ir::Expr::Closure {
                params,
                return_type,
                body,
            } => {
                // If the arrow has no explicit return type annotation, try the variable's
                let ret = return_type.or_else(|| {
                    arrow
                        .return_type
                        .as_ref()
                        .and_then(|ann| convert_ts_type(&ann.type_ann).ok())
                });
                let fn_body = match body {
                    crate::ir::ClosureBody::Expr(expr) => {
                        vec![crate::ir::Stmt::Return(Some(*expr))]
                    }
                    crate::ir::ClosureBody::Block(stmts) => stmts,
                };
                let type_params =
                    crate::transformer::types::extract_type_params(arrow.type_params.as_deref());
                items.push(Item::Fn {
                    vis: vis.clone(),
                    is_async: arrow.is_async,
                    name,
                    type_params,
                    params,
                    return_type: ret,
                    body: fn_body,
                });
                all_warnings.extend(fallback_warnings);
            }
            _ => continue,
        }
    }
    Ok((items, all_warnings))
}

/// Converts a TS enum declaration into an IR [`Item::Enum`].
///
/// Handles numeric enums (auto-incrementing and explicit values) and string enums.
fn convert_ts_enum(ts_enum: &swc_ecma_ast::TsEnumDecl, vis: Visibility) -> Result<Vec<Item>> {
    let name = ts_enum.id.sym.to_string();
    let mut variants = Vec::new();

    for member in &ts_enum.members {
        let variant_name = match &member.id {
            swc_ecma_ast::TsEnumMemberId::Ident(ident) => ident.sym.to_string(),
            swc_ecma_ast::TsEnumMemberId::Str(s) => s.value.to_string_lossy().into_owned(),
        };

        let value = member.init.as_ref().and_then(|init| match init.as_ref() {
            swc_ecma_ast::Expr::Lit(swc_ecma_ast::Lit::Num(n)) => {
                Some(EnumValue::Number(n.value as i64))
            }
            swc_ecma_ast::Expr::Lit(swc_ecma_ast::Lit::Str(s)) => {
                Some(EnumValue::Str(s.value.to_string_lossy().into_owned()))
            }
            swc_ecma_ast::Expr::Unary(unary) if unary.op == swc_ecma_ast::UnaryOp::Minus => {
                if let swc_ecma_ast::Expr::Lit(swc_ecma_ast::Lit::Num(n)) = unary.arg.as_ref() {
                    Some(EnumValue::Number(-(n.value as i64)))
                } else {
                    None
                }
            }
            _ => None,
        });

        variants.push(EnumVariant {
            name: variant_name,
            value,
        });
    }

    Ok(vec![Item::Enum {
        vis,
        name,
        variants,
    }])
}

/// Returns a human-readable kind name for a module-level item.
fn format_module_item_kind(item: &ModuleItem) -> String {
    match item {
        ModuleItem::ModuleDecl(decl) => match decl {
            ModuleDecl::ExportDefaultDecl(_) => "ExportDefaultDecl".to_string(),
            ModuleDecl::ExportDefaultExpr(_) => "ExportDefaultExpr".to_string(),
            ModuleDecl::ExportAll(_) => "ExportAll".to_string(),
            ModuleDecl::ExportNamed(_) => "ExportNamed".to_string(),
            ModuleDecl::TsImportEquals(_) => "TsImportEquals".to_string(),
            ModuleDecl::TsExportAssignment(_) => "TsExportAssignment".to_string(),
            ModuleDecl::TsNamespaceExport(_) => "TsNamespaceExport".to_string(),
            _ => format!("ModuleDecl({decl:?})"),
        },
        ModuleItem::Stmt(stmt) => format!("Stmt({stmt:?})"),
    }
}

/// Returns a human-readable kind name for a declaration.
fn format_decl_kind(decl: &Decl) -> String {
    match decl {
        Decl::TsModule(_) => "TsModuleDecl".to_string(),
        Decl::Using(_) => "UsingDecl".to_string(),
        _ => format!("Decl({decl:?})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::Stmt;
    use crate::ir::{Expr, Param, RustType, StructField, Visibility};
    use crate::parser::parse_typescript;
    use crate::registry::TypeRegistry;

    #[test]
    fn test_transform_module_empty() {
        let module = parse_typescript("").expect("parse failed");
        let items = transform_module(&module, &TypeRegistry::new()).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_transform_module_import_single() {
        let source = r#"import { Foo } from "./bar";"#;
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module, &TypeRegistry::new()).unwrap();

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
        let items = transform_module(&module, &TypeRegistry::new()).unwrap();

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
        let items = transform_module(&module, &TypeRegistry::new()).unwrap();

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
    fn test_transform_module_import_hyphen_to_underscore() {
        let source = r#"import { Foo } from "./hono-base";"#;
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module, &TypeRegistry::new()).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0],
            Item::Use {
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
                path: "crate::my_long_name".to_string(),
                names: vec!["Foo".to_string()],
            }
        );
    }

    #[test]
    fn test_transform_module_import_external_skipped() {
        let source = r#"import { Foo } from "lodash";"#;
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module, &TypeRegistry::new()).unwrap();

        assert!(items.is_empty());
    }

    #[test]
    fn test_transform_module_non_exported_is_private() {
        let source = "interface Foo { name: string; }";
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module, &TypeRegistry::new()).unwrap();

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
        let items = transform_module(&module, &TypeRegistry::new()).unwrap();

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
        let items = transform_module(&module, &TypeRegistry::new()).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0],
            Item::Struct {
                vis: Visibility::Private,
                name: "Foo".to_string(),
                type_params: vec![],
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
        let items = transform_module(&module, &TypeRegistry::new()).unwrap();

        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_transform_module_type_alias_object() {
        let source = "type Point = { x: number; y: number; };";
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module, &TypeRegistry::new()).unwrap();

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
        let items = transform_module(&module, &TypeRegistry::new()).unwrap();

        // const x = 42 is skipped, only Foo is converted
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_transform_module_function_declaration() {
        let source = "function add(a: number, b: number): number { return a + b; }";
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module, &TypeRegistry::new()).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0],
            Item::Fn {
                vis: Visibility::Private,
                is_async: false,
                name: "add".to_string(),
                type_params: vec![],
                params: vec![
                    Param {
                        name: "a".to_string(),
                        ty: Some(RustType::F64),
                    },
                    Param {
                        name: "b".to_string(),
                        ty: Some(RustType::F64),
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
        let items = transform_module(&module, &TypeRegistry::new()).unwrap();

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

    #[test]
    fn test_transform_enum_numeric_auto_values() {
        let source = "enum Color { Red, Green, Blue }";
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module, &TypeRegistry::new()).unwrap();

        assert_eq!(items.len(), 1);
        match &items[0] {
            Item::Enum {
                vis,
                name,
                variants,
            } => {
                assert_eq!(*vis, Visibility::Private);
                assert_eq!(name, "Color");
                assert_eq!(variants.len(), 3);
                assert_eq!(variants[0].name, "Red");
                assert_eq!(variants[0].value, None);
                assert_eq!(variants[1].name, "Green");
                assert_eq!(variants[2].name, "Blue");
            }
            _ => panic!("expected Enum"),
        }
    }

    #[test]
    fn test_transform_enum_numeric_explicit_values() {
        let source = "enum Status { Active = 1, Inactive = 0 }";
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module, &TypeRegistry::new()).unwrap();

        assert_eq!(items.len(), 1);
        match &items[0] {
            Item::Enum { variants, .. } => {
                assert_eq!(variants[0].name, "Active");
                assert_eq!(variants[0].value, Some(crate::ir::EnumValue::Number(1)));
                assert_eq!(variants[1].name, "Inactive");
                assert_eq!(variants[1].value, Some(crate::ir::EnumValue::Number(0)));
            }
            _ => panic!("expected Enum"),
        }
    }

    #[test]
    fn test_transform_enum_string_values() {
        let source = r#"enum Direction { Up = "UP", Down = "DOWN" }"#;
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module, &TypeRegistry::new()).unwrap();

        assert_eq!(items.len(), 1);
        match &items[0] {
            Item::Enum { variants, .. } => {
                assert_eq!(variants[0].name, "Up");
                assert_eq!(
                    variants[0].value,
                    Some(crate::ir::EnumValue::Str("UP".to_string()))
                );
                assert_eq!(variants[1].name, "Down");
                assert_eq!(
                    variants[1].value,
                    Some(crate::ir::EnumValue::Str("DOWN".to_string()))
                );
            }
            _ => panic!("expected Enum"),
        }
    }

    #[test]
    fn test_transform_enum_export_is_public() {
        let source = "export enum Color { Red, Green }";
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module, &TypeRegistry::new()).unwrap();

        assert_eq!(items.len(), 1);
        match &items[0] {
            Item::Enum { vis, .. } => assert_eq!(*vis, Visibility::Public),
            _ => panic!("expected Enum"),
        }
    }

    #[test]
    fn test_transform_enum_empty() {
        let source = "enum Empty { }";
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module, &TypeRegistry::new()).unwrap();

        assert_eq!(items.len(), 1);
        match &items[0] {
            Item::Enum { variants, .. } => assert!(variants.is_empty()),
            _ => panic!("expected Enum"),
        }
    }

    #[test]
    fn test_transform_enum_single_member() {
        let source = "enum Single { Only = -1 }";
        let module = parse_typescript(source).expect("parse failed");
        let items = transform_module(&module, &TypeRegistry::new()).unwrap();

        assert_eq!(items.len(), 1);
        match &items[0] {
            Item::Enum { variants, .. } => {
                assert_eq!(variants.len(), 1);
                assert_eq!(variants[0].name, "Only");
                assert_eq!(variants[0].value, Some(crate::ir::EnumValue::Number(-1)));
            }
            _ => panic!("expected Enum"),
        }
    }
}
