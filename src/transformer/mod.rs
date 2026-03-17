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

use crate::ir::{EnumValue, EnumVariant, Item, RustType, Visibility};
use crate::registry::TypeRegistry;
use crate::transformer::classes::ClassInfo;
use crate::transformer::expressions::convert_arrow_expr;
use crate::transformer::types::convert_ts_type;

/// ローカル変数の型情報を保持する型環境。
///
/// スコープチェーンにより、ブロックスコープでの変数シャドウイングを正しく追跡する。
/// 変数宣言時にエントリを追加し、後続の式変換で参照する。
#[derive(Debug, Clone)]
pub struct TypeEnv {
    scopes: Vec<HashMap<String, RustType>>,
}

impl Default for TypeEnv {
    fn default() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }
}

impl TypeEnv {
    /// 新しい空の型環境を作成する。ルートスコープが 1 つ含まれる。
    pub fn new() -> Self {
        Self::default()
    }

    /// 新しい子スコープを開始する。
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// 現在のスコープを終了し、その中の変数を破棄する。
    /// ルートスコープは pop しない。
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// 変数の型を現在のスコープに登録する。同スコープ内の同名変数は上書きされる。
    pub fn insert(&mut self, name: String, ty: RustType) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, ty);
        }
    }

    /// 既存の変数の型を更新する。スコープチェーンを内側から探索し、
    /// 最初に見つかったスコープで更新する。どのスコープにも存在しない場合は
    /// 現在のスコープに挿入する。
    pub fn update(&mut self, name: String, ty: RustType) {
        for scope in self.scopes.iter_mut().rev() {
            if let std::collections::hash_map::Entry::Occupied(mut e) = scope.entry(name.clone()) {
                e.insert(ty);
                return;
            }
        }
        // どのスコープにも存在しない → 現在のスコープに挿入
        self.insert(name, ty);
    }

    /// 変数名から型を取得する。最内スコープから順に探索する。
    pub fn get(&self, name: &str) -> Option<&RustType> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty);
            }
        }
        None
    }
}

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
    let rust_type = types::convert_ts_type(&ty.type_ann, &mut Vec::new(), reg)?;
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
    // Pre-scan: collect class info for inheritance resolution
    let class_map = pre_scan_classes(module, reg);
    let iface_methods = pre_scan_interface_methods(module);

    let mut items = Vec::new();
    for module_item in &module.body {
        let (converted, _warnings) =
            transform_module_item(module_item, reg, &class_map, &iface_methods, false)?;
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
    let iface_methods = pre_scan_interface_methods(module);

    let mut items = Vec::new();
    let mut unsupported = Vec::new();

    for module_item in &module.body {
        match transform_module_item(module_item, reg, &class_map, &iface_methods, true) {
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

/// Pre-scans all interface declarations to collect method names per interface.
///
/// Used by `implements` processing to determine which class methods belong to
/// which trait impl block.
fn pre_scan_interface_methods(module: &Module) -> HashMap<String, Vec<String>> {
    let mut map = HashMap::new();

    for module_item in &module.body {
        let decl = match module_item {
            ModuleItem::Stmt(Stmt::Decl(Decl::TsInterface(d))) => d,
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => {
                if let Decl::TsInterface(d) = &export.decl {
                    d
                } else {
                    continue;
                }
            }
            _ => continue,
        };

        let name = decl.id.sym.to_string();
        let method_names: Vec<String> = decl
            .body
            .body
            .iter()
            .filter_map(|member| {
                if let swc_ecma_ast::TsTypeElement::TsMethodSignature(method) = member {
                    if let swc_ecma_ast::Expr::Ident(ident) = method.key.as_ref() {
                        return Some(ident.sym.to_string());
                    }
                }
                None
            })
            .collect();

        if !method_names.is_empty() {
            map.insert(name, method_names);
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
    iface_methods: &HashMap<String, Vec<String>>,
    resilient: bool,
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
            let items: Vec<Item> = transform_import(import_decl).into_iter().collect();
            Ok((items, vec![]))
        }
        ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(export)) => {
            let items: Vec<Item> = transform_export_named(export).into_iter().collect();
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
fn transform_export_named(export: &swc_ecma_ast::NamedExport) -> Option<Item> {
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

    let path = convert_relative_path_to_crate_path(&src_str);
    Some(Item::Use {
        vis: Visibility::Public,
        path,
        names,
    })
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
            let items =
                transform_class_with_inheritance(class_decl, vis, reg, class_map, iface_methods)?;
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

/// Transforms a class declaration, handling inheritance and `implements` if applicable.
///
/// - If the class is a parent (extended by another class): generates struct + trait + impls
/// - If the class is a child (extends another class): generates struct + impl + trait impl
/// - If the class implements interfaces: generates struct + impl + impl Trait for Struct
/// - Otherwise: generates struct + impl (no trait)
fn transform_class_with_inheritance(
    class_decl: &ast::ClassDecl,
    vis: Visibility,
    reg: &TypeRegistry,
    class_map: &HashMap<String, ClassInfo>,
    iface_methods: &HashMap<String, Vec<String>>,
) -> Result<Vec<Item>> {
    let info = classes::extract_class_info(class_decl, vis, reg)?;
    let parent_names = find_parent_class_names(class_map);

    if info.is_abstract {
        // Abstract class — generate trait (not struct)
        classes::generate_abstract_class_items(&info)
    } else if parent_names.contains(&info.name) {
        // This class is a parent — generate struct + trait + impls
        classes::generate_parent_class_items(&info)
    } else if let Some(parent_name) = &info.parent {
        let parent_info = class_map.get(parent_name);
        if parent_info.is_some_and(|p| p.is_abstract) {
            // Parent is abstract — generate struct + impl AbstractParent for Child
            classes::generate_child_of_abstract(&info, parent_name)
        } else if !info.implements.is_empty() {
            // Child class with interface implementations
            classes::generate_child_class_with_implements(&info, parent_info, iface_methods)
        } else {
            // This class is a child — generate struct + impl + trait impl
            classes::generate_items_for_class(&info, parent_info)
        }
    } else if !info.implements.is_empty() {
        // Class implements interfaces — split methods into trait impls
        classes::generate_class_with_implements(&info, iface_methods)
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
        let closure = convert_arrow_expr(
            arrow,
            reg,
            resilient,
            &mut fallback_warnings,
            &TypeEnv::new(),
        )?;
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
                        .and_then(|ann| convert_ts_type(&ann.type_ann, &mut Vec::new(), reg).ok())
                });
                let mut fn_body = match body {
                    crate::ir::ClosureBody::Expr(expr) => {
                        vec![crate::ir::Stmt::Return(Some(*expr))]
                    }
                    crate::ir::ClosureBody::Block(stmts) => stmts,
                };
                functions::convert_last_return_to_tail(&mut fn_body);
                // Check for untyped parameters — these produce invalid Rust in Item::Fn
                let mut checked_params = Vec::new();
                for p in params {
                    if p.ty.is_none() {
                        if resilient {
                            fallback_warnings
                                .push(format!("parameter '{}' has no type annotation", p.name));
                            checked_params.push(crate::ir::Param {
                                name: p.name,
                                ty: Some(crate::ir::RustType::Any),
                            });
                        } else {
                            return Err(anyhow::anyhow!(
                                "parameter '{}' has no type annotation",
                                p.name
                            ));
                        }
                    } else {
                        checked_params.push(p);
                    }
                }

                let type_params =
                    crate::transformer::types::extract_type_params(arrow.type_params.as_deref());
                items.push(Item::Fn {
                    vis: vis.clone(),
                    attributes: vec![],
                    is_async: arrow.is_async,
                    name,
                    type_params,
                    params: checked_params,
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
