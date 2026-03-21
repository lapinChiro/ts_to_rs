//! AST to IR transformation.
//!
//! This module converts SWC TypeScript AST nodes into the IR representation
//! defined in [`crate::ir`].

pub(crate) mod any_narrowing;
pub mod classes;
pub mod expressions;
pub mod functions;
pub mod statements;
pub(crate) mod type_env;
pub mod types;

pub use type_env::TypeEnv;
pub(crate) use type_env::{wrap_trait_for_position, TypePosition};

use anyhow::Result;
use swc_common::Spanned;
use swc_ecma_ast as ast;
use swc_ecma_ast::{Decl, ImportSpecifier, Module, ModuleDecl, ModuleItem, Stmt};

use std::collections::HashMap;

use crate::ir::{EnumValue, EnumVariant, Item, Visibility};
use crate::registry::TypeRegistry;
use crate::transformer::classes::ClassInfo;

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

/// Converts an identifier parameter pattern into an IR [`Param`].
///
/// Extracts name and type annotation from a `BindingIdent`, converts the type,
/// and returns a `Param`. Used by both function and class method parameter conversion.
pub fn convert_ident_to_param(
    ident: &ast::BindingIdent,
    reg: &crate::registry::TypeRegistry,
) -> Result<crate::ir::Param> {
    let name = ident.id.sym.to_string();
    let ty = ident
        .type_ann
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("parameter '{}' has no type annotation", name))?;
    let rust_type = types::convert_type_for_position(&ty.type_ann, TypePosition::Param, reg)?;
    Ok(crate::ir::Param {
        name,
        ty: Some(rust_type),
    })
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
    transform_module_with_path(module, reg, None)
}

/// Transforms an SWC [`Module`] into IR items, with the file's crate-relative directory.
///
/// `current_file_dir` is the directory of the source file relative to the crate root
/// (e.g., `Some("adapter/bun")` for `adapter/bun/server.ts`). This is used to resolve
/// `../` in import paths. When `None`, the file is assumed to be at the crate root.
pub fn transform_module_with_path(
    module: &Module,
    reg: &TypeRegistry,
    current_file_dir: Option<&str>,
) -> Result<Vec<Item>> {
    // Pre-scan: collect class info for inheritance resolution
    let class_map = classes::pre_scan_classes(module, reg);
    let iface_methods = classes::pre_scan_interface_methods(module);

    let mut items = Vec::new();
    for module_item in &module.body {
        let (converted, _warnings) = transform_module_item(
            module_item,
            reg,
            &class_map,
            &iface_methods,
            false,
            current_file_dir,
        )?;
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
    transform_module_collecting_with_path(module, reg, None)
}

/// Like [`transform_module_collecting`] but with file path context for import resolution.
pub fn transform_module_collecting_with_path(
    module: &Module,
    reg: &TypeRegistry,
    current_file_dir: Option<&str>,
) -> Result<(Vec<Item>, Vec<UnsupportedSyntaxError>)> {
    let class_map = classes::pre_scan_classes(module, reg);
    let iface_methods = classes::pre_scan_interface_methods(module);

    let mut items = Vec::new();
    let mut unsupported = Vec::new();

    for module_item in &module.body {
        match transform_module_item(
            module_item,
            reg,
            &class_map,
            &iface_methods,
            true,
            current_file_dir,
        ) {
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

/// Transforms a single module item into IR [`Item`]s.
///
/// When `resilient` is true, type conversion failures in function parameters and
/// return types fall back to `RustType::Any` instead of aborting.
fn transform_module_item(
    module_item: &ModuleItem,
    reg: &TypeRegistry,
    class_map: &HashMap<String, ClassInfo>,
    iface_methods: &HashMap<String, Vec<String>>,
    resilient: bool,
    current_file_dir: Option<&str>,
) -> Result<(Vec<Item>, Vec<String>)> {
    match module_item {
        ModuleItem::Stmt(Stmt::Decl(decl)) => transform_decl(
            decl,
            Visibility::Private,
            reg,
            class_map,
            iface_methods,
            resilient,
        ),
        ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => transform_decl(
            &export.decl,
            Visibility::Public,
            reg,
            class_map,
            iface_methods,
            resilient,
        ),
        ModuleItem::ModuleDecl(ModuleDecl::Import(import_decl)) => {
            let items: Vec<Item> = transform_import(import_decl, current_file_dir)
                .into_iter()
                .collect();
            Ok((items, vec![]))
        }
        ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(export)) => {
            let items: Vec<Item> = transform_export_named(export, current_file_dir)
                .into_iter()
                .collect();
            Ok((items, vec![]))
        }
        ModuleItem::ModuleDecl(ModuleDecl::ExportAll(export_all)) => {
            let src = export_all.src.value.to_string_lossy().into_owned();
            if src.starts_with("./") || src.starts_with("../") {
                let path = convert_relative_path_to_crate_path(&src, current_file_dir);
                Ok((
                    vec![Item::Use {
                        vis: Visibility::Public,
                        path,
                        names: vec!["*".to_string()],
                    }],
                    vec![],
                ))
            } else {
                // External package re-exports are skipped
                Ok((vec![], vec![]))
            }
        }
        // Top-level expression statements (e.g., `globalThis.crypto ??= crypto`)
        // Rust has no top-level expressions; skip silently
        ModuleItem::Stmt(Stmt::Expr(_)) => Ok((vec![], vec![])),
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
fn transform_import(
    import_decl: &swc_ecma_ast::ImportDecl,
    current_file_dir: Option<&str>,
) -> Option<Item> {
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

    let path = convert_relative_path_to_crate_path(&src, current_file_dir);
    Some(Item::Use {
        vis: Visibility::Private,
        path,
        names,
    })
}

/// Transforms a named export into an IR [`Item::Use`] with public visibility.
///
/// - Re-exports (`export { Foo } from "./bar"`) become `pub use bar::Foo;`
/// - Local name exports (`export { Foo }`) are skipped (declarations are already `pub`)
fn transform_export_named(
    export: &swc_ecma_ast::NamedExport,
    current_file_dir: Option<&str>,
) -> Option<Item> {
    // Local name exports (no source path) are skipped
    let src = export.src.as_ref()?;
    let src_str = src.value.to_string_lossy().into_owned();

    // Only handle relative imports
    if !src_str.starts_with("./") && !src_str.starts_with("../") {
        return None;
    }

    let names: Vec<String> = export
        .specifiers
        .iter()
        .filter_map(|spec| match spec {
            swc_ecma_ast::ExportSpecifier::Named(named) => {
                // Use the original name (not the renamed alias)
                match &named.orig {
                    swc_ecma_ast::ModuleExportName::Ident(ident) => Some(ident.sym.to_string()),
                    swc_ecma_ast::ModuleExportName::Str(s) => {
                        Some(s.value.to_string_lossy().into_owned())
                    }
                }
            }
            _ => None,
        })
        .collect();

    if names.is_empty() {
        return None;
    }

    let path = convert_relative_path_to_crate_path(&src_str, current_file_dir);
    Some(Item::Use {
        vis: Visibility::Public,
        path,
        names,
    })
}

/// Converts a TypeScript relative import path to a Rust `crate::` path.
///
/// `current_file_dir` is the directory of the importing file, relative to the
/// crate root (e.g., `Some("adapter/bun")` for `adapter/bun/server.ts`).
/// When `None`, the file is assumed to be at the crate root.
/// Hyphens in path segments are replaced with underscores.
fn convert_relative_path_to_crate_path(rel_path: &str, current_file_dir: Option<&str>) -> String {
    let resolved = if rel_path.starts_with("../") {
        // Resolve parent-relative paths using the current file's directory
        let base_parts: Vec<&str> = current_file_dir
            .unwrap_or("")
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();
        let mut parts = base_parts;
        let mut remaining = rel_path;
        while let Some(rest) = remaining.strip_prefix("../") {
            parts.pop();
            remaining = rest;
        }
        // remaining may still have ./ prefix
        let remaining = remaining.strip_prefix("./").unwrap_or(remaining);
        if remaining.is_empty() {
            parts.join("/")
        } else {
            let suffix = remaining;
            if parts.is_empty() {
                suffix.to_string()
            } else {
                format!("{}/{suffix}", parts.join("/"))
            }
        }
    } else {
        // ./foo or ./sub/bar — resolve relative to current file's directory
        let stripped = rel_path.strip_prefix("./").unwrap_or(rel_path);
        match current_file_dir {
            Some(dir) if !dir.is_empty() => format!("{dir}/{stripped}"),
            _ => stripped.to_string(),
        }
    };

    let crate_path: Vec<String> = resolved
        .split('/')
        .map(|seg| seg.replace('-', "_"))
        .collect();
    format!("crate::{}", crate_path.join("::"))
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
    iface_methods: &HashMap<String, Vec<String>>,
    resilient: bool,
) -> Result<(Vec<Item>, Vec<String>)> {
    match decl {
        Decl::TsInterface(interface_decl) => {
            let items = types::convert_interface_items(interface_decl, vis, reg)?;
            Ok((items, vec![]))
        }
        Decl::TsTypeAlias(type_alias_decl) => {
            let items = types::convert_type_alias_items(type_alias_decl, vis, reg)?;
            Ok((items, vec![]))
        }
        Decl::Fn(fn_decl) => {
            let (items, warnings) = functions::convert_fn_decl(fn_decl, vis, reg, resilient)?;
            Ok((items, warnings))
        }
        Decl::Class(class_decl) => {
            let items = classes::transform_class_with_inheritance(
                class_decl,
                vis,
                reg,
                class_map,
                iface_methods,
            )?;
            Ok((items, vec![]))
        }
        Decl::Var(var_decl) => functions::convert_var_decl_arrow_fns(var_decl, vis, reg, resilient),
        Decl::TsEnum(ts_enum) => {
            let items = convert_ts_enum(ts_enum, vis)?;
            Ok((items, vec![]))
        }
        Decl::TsModule(ts_module) => {
            // `declare module 'name' { ... }` — process internal declarations
            let mut items = Vec::new();
            if let Some(ast::TsNamespaceBody::TsModuleBlock(block)) = &ts_module.body {
                for item in &block.body {
                    if let ModuleItem::Stmt(ast::Stmt::Decl(inner_decl)) = item {
                        if let Ok((inner_items, _)) = transform_decl(
                            inner_decl,
                            vis.clone(),
                            reg,
                            class_map,
                            iface_methods,
                            resilient,
                        ) {
                            items.extend(inner_items);
                        }
                    }
                }
            }
            Ok((items, vec![]))
        }
        _ => Err(UnsupportedSyntaxError {
            kind: format_decl_kind(decl),
            byte_pos: decl.span().lo.0,
        }
        .into()),
    }
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
            swc_ecma_ast::Expr::Bin(bin) => format_bin_expr(bin).map(EnumValue::Expr),
            _ => None,
        });

        variants.push(EnumVariant {
            name: variant_name,
            value,
            data: None,
            fields: vec![],
        });
    }

    Ok(vec![Item::Enum {
        vis,
        name,
        serde_tag: None,
        variants,
    }])
}

/// Formats a binary expression AST node as a Rust expression string.
///
/// Supports numeric literals and binary operators (e.g., `1 << 0`, `1 | 2`).
/// Returns `None` for unsupported operands.
fn format_bin_expr(bin: &swc_ecma_ast::BinExpr) -> Option<String> {
    let left = format_simple_expr(&bin.left)?;
    let right = format_simple_expr(&bin.right)?;
    let op = match bin.op {
        swc_ecma_ast::BinaryOp::LShift => "<<",
        swc_ecma_ast::BinaryOp::RShift => ">>",
        swc_ecma_ast::BinaryOp::BitOr => "|",
        swc_ecma_ast::BinaryOp::BitAnd => "&",
        swc_ecma_ast::BinaryOp::BitXor => "^",
        swc_ecma_ast::BinaryOp::Add => "+",
        swc_ecma_ast::BinaryOp::Sub => "-",
        swc_ecma_ast::BinaryOp::Mul => "*",
        _ => return None,
    };
    Some(format!("{left} {op} {right}"))
}

/// Formats a simple expression (numeric literal or nested binary) as a string.
fn format_simple_expr(expr: &swc_ecma_ast::Expr) -> Option<String> {
    match expr {
        swc_ecma_ast::Expr::Lit(swc_ecma_ast::Lit::Num(n)) => Some(format!("{}", n.value as i64)),
        swc_ecma_ast::Expr::Bin(bin) => format_bin_expr(bin),
        _ => None,
    }
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
mod tests;
