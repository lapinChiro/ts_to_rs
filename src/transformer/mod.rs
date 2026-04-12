//! AST to IR transformation.
//!
//! This module converts SWC TypeScript AST nodes into the IR representation
//! defined in [`crate::ir`].

pub mod classes;
pub mod context;
pub mod expressions;
pub mod functions;
pub mod statements;
pub(crate) mod type_position;
pub(crate) use type_position::{wrap_trait_for_position, TypePosition};

use anyhow::Result;
use swc_common::Spanned;
use swc_ecma_ast as ast;
use swc_ecma_ast::{Decl, ImportSpecifier, Module, ModuleDecl, ModuleItem, Stmt};

use std::collections::HashMap;

use crate::ir::{EnumValue, EnumVariant, Item, Visibility};
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::TypeRegistry;
use crate::transformer::classes::ClassInfo;
use crate::transformer::context::TransformContext;

/// 変換処理の状態を保持する構造体。
///
/// 不変コンテキスト (`tctx`) と可変状態 (`synthetic`) を束ね、
/// 全変換関数をメソッドとして提供する。各サブモジュールに `impl Transformer`
/// ブロックを配置し、ファイル構成を変更せずにメソッド化する。
pub(crate) struct Transformer<'a> {
    /// 不変コンテキスト（TypeRegistry, ModuleGraph, TypeResolution, file path）
    tctx: &'a TransformContext<'a>,
    /// 合成型レジストリ（可変 — 変換中に型が追加される）
    synthetic: &'a mut SyntheticTypeRegistry,
    /// ユーザー定義クラスの `&mut self` メソッド名の集合。
    /// `pre_scan_classes` 後に構築し、mutation 検出で参照する。
    mut_method_names: std::collections::HashSet<String>,
    /// Callable interface marker struct 名の使用済み集合 (INV-1)。
    /// PascalCase collision を検出し、suffix 付与で unique 化する。
    used_marker_names: std::collections::HashSet<String>,
}

impl<'a> Transformer<'a> {
    /// モジュール変換用の Transformer を構築する。
    pub(crate) fn for_module(
        tctx: &'a TransformContext<'a>,
        synthetic: &'a mut SyntheticTypeRegistry,
    ) -> Self {
        Self {
            tctx,
            synthetic,
            mut_method_names: std::collections::HashSet::new(),
            used_marker_names: std::collections::HashSet::new(),
        }
    }

    /// 親 Transformer の synthetic を共有するネスト scope を生成する。
    ///
    /// arrow body, class member, expression function, loop 内 fn 等、
    /// 親と同じ合成型レジストリを共有する sub-Transformer に使用する。
    pub(crate) fn spawn_nested_scope(&mut self) -> Transformer<'_> {
        Transformer {
            tctx: self.tctx,
            synthetic: &mut *self.synthetic,
            mut_method_names: self.mut_method_names.clone(),
            used_marker_names: std::collections::HashSet::new(),
        }
    }

    /// ローカルの synthetic レジストリを持つネスト scope を生成する。
    ///
    /// fn body のように独自の合成型空間を持つ sub-Transformer に使用する。
    pub(crate) fn spawn_nested_scope_with_local_synthetic<'b>(
        &'b self,
        local: &'b mut SyntheticTypeRegistry,
    ) -> Transformer<'b>
    where
        'a: 'b,
    {
        Transformer {
            tctx: self.tctx,
            synthetic: local,
            mut_method_names: self.mut_method_names.clone(),
            used_marker_names: std::collections::HashSet::new(),
        }
    }

    /// Marker struct 名を allocate し、衝突時に suffix で unique 化する (INV-1)。
    ///
    /// `base` → `base` (未使用) or `base1` → `base2` ... で unique 化。
    pub(crate) fn allocate_marker_name(&mut self, base: &str) -> String {
        if self.used_marker_names.insert(base.to_string()) {
            return base.to_string();
        }
        let mut i = 1;
        loop {
            let candidate = format!("{base}{i}");
            if self.used_marker_names.insert(candidate.clone()) {
                return candidate;
            }
            i += 1;
        }
    }

    /// `tctx.type_registry` へのショートカット。
    pub(crate) fn reg(&self) -> &'a TypeRegistry {
        self.tctx.type_registry
    }

    /// 単一モジュール変換用のヘルパー。
    ///
    /// 空の ModuleGraph / FileTypeResolution / file_path で TransformContext を構築し、
    /// クロージャに Transformer を渡す。マルチモジュール機能は利用できない。
    ///
    /// `synthetic` はパラメータとして渡す（クロージャ完了後に `into_items()` を
    /// 呼び出す必要があるため、内部作成では呼び出し元に返せない）。
    /// Builds the set of `&mut self` method names from pre-scanned class info.
    ///
    /// Called after `pre_scan_classes` in `transform_module` / `transform_module_collecting`.
    /// The resulting set is used by `mark_mutated_vars` and `mark_mut_params_from_body` to
    /// detect mutations via user-defined `&mut self` method calls.
    fn build_mut_method_names(&mut self, class_map: &HashMap<String, ClassInfo>) {
        self.mut_method_names = class_map
            .values()
            .flat_map(|info| info.methods.iter())
            .filter(|m| m.has_mut_self)
            .map(|m| m.name.clone())
            .collect();
    }

    fn with_single_module<R>(
        reg: &TypeRegistry,
        synthetic: &mut SyntheticTypeRegistry,
        f: impl FnOnce(&mut Transformer<'_>) -> R,
    ) -> R {
        let mg = crate::pipeline::ModuleGraph::empty();
        let resolution = crate::pipeline::type_resolution::FileTypeResolution::empty();
        let tctx = context::TransformContext::new(&mg, reg, &resolution, std::path::Path::new(""));
        let mut t = Transformer::for_module(&tctx, synthetic);
        f(&mut t)
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

impl UnsupportedSyntaxError {
    /// Creates a new `UnsupportedSyntaxError` from a kind string and an SWC `Span`.
    ///
    /// Extracts `byte_pos` from the span's lower bound (`span.lo`).
    /// Prefer this over manual struct construction for consistency.
    pub fn new(kind: impl Into<String>, span: swc_common::Span) -> Self {
        Self {
            kind: kind.into(),
            byte_pos: span.lo.0,
        }
    }
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
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut items =
        Transformer::with_single_module(reg, &mut synthetic, |t| t.transform_module(module))?;
    let mut all = synthetic.into_items();
    all.append(&mut items);
    Ok(all)
}

/// Transforms an SWC [`Module`] into IR items using a [`TransformContext`].
pub fn transform_module_with_context(
    module: &Module,
    ctx: &context::TransformContext<'_>,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Vec<Item>> {
    let mut t = Transformer::for_module(ctx, synthetic);
    t.transform_module(module)
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
    let mut synthetic = SyntheticTypeRegistry::new();
    let (mut items, unsupported) = Transformer::with_single_module(reg, &mut synthetic, |t| {
        t.transform_module_collecting(module)
    })?;
    let mut all = synthetic.into_items();
    all.append(&mut items);
    Ok((all, unsupported))
}

// --- Transformer methods for module-level transformation ---

impl<'a> Transformer<'a> {
    /// Transforms an SWC [`Module`] into IR items.
    ///
    /// The file's crate-relative directory (used for `../` import path resolution) is
    /// derived from `self.tctx.file_path` via [`current_file_dir()`](Self::current_file_dir).
    pub(crate) fn transform_module(&mut self, module: &Module) -> Result<Vec<Item>> {
        // Pre-scan: collect class info for inheritance resolution
        let class_map = self.pre_scan_classes(module);
        let iface_methods = classes::pre_scan_interface_methods(module);
        self.build_mut_method_names(&class_map);

        let mut items = Vec::new();
        let mut init_stmts = Vec::new();
        for module_item in &module.body {
            // Collect top-level expression statements for init() function
            if let ModuleItem::Stmt(Stmt::Expr(expr_stmt)) = module_item {
                let expr = self.convert_expr(&expr_stmt.expr)?;
                init_stmts.push(crate::ir::Stmt::Expr(expr));
                continue;
            }
            let (converted, _warnings) =
                self.transform_module_item(module_item, &class_map, &iface_methods, false)?;
            items.extend(converted);
        }

        if !init_stmts.is_empty() {
            items.push(build_init_fn(init_stmts));
        }

        inject_regex_import_if_needed(&mut items);
        inject_js_typeof_if_needed(&mut items);
        Ok(items)
    }

    /// Transforms an SWC [`Module`], collecting unsupported syntax instead of aborting.
    pub(crate) fn transform_module_collecting(
        &mut self,
        module: &Module,
    ) -> Result<(Vec<Item>, Vec<UnsupportedSyntaxError>)> {
        let class_map = self.pre_scan_classes(module);
        let iface_methods = classes::pre_scan_interface_methods(module);
        self.build_mut_method_names(&class_map);

        let mut items = Vec::new();
        let mut unsupported = Vec::new();
        let mut init_stmts = Vec::new();

        for module_item in &module.body {
            // Collect top-level expression statements for init() function
            if let ModuleItem::Stmt(Stmt::Expr(expr_stmt)) = module_item {
                match self.convert_expr(&expr_stmt.expr) {
                    Ok(expr) => {
                        init_stmts.push(crate::ir::Stmt::Expr(expr));
                        continue;
                    }
                    Err(e) => {
                        // Record as unsupported and continue
                        match e.downcast::<UnsupportedSyntaxError>() {
                            Ok(unsup) => unsupported.push(unsup),
                            Err(other) => {
                                unsupported.push(UnsupportedSyntaxError::new(
                                    other.to_string(),
                                    module_item.span(),
                                ));
                            }
                        }
                        continue;
                    }
                }
            }
            match self.transform_module_item(module_item, &class_map, &iface_methods, true) {
                Ok((converted, warnings)) => {
                    items.extend(converted);
                    for warning in warnings {
                        unsupported.push(UnsupportedSyntaxError::new(warning, module_item.span()));
                    }
                }
                Err(e) => match e.downcast::<UnsupportedSyntaxError>() {
                    Ok(unsup) => unsupported.push(unsup),
                    Err(other) => {
                        // Transformer-internal errors (e.g. unsupported parameter patterns
                        // inside functions/classes) are collected instead of aborting.
                        unsupported.push(UnsupportedSyntaxError::new(
                            other.to_string(),
                            module_item.span(),
                        ));
                    }
                },
            }
        }

        if !init_stmts.is_empty() {
            items.push(build_init_fn(init_stmts));
        }

        inject_regex_import_if_needed(&mut items);
        inject_js_typeof_if_needed(&mut items);
        Ok((items, unsupported))
    }

    /// Transforms a single module item into IR [`Item`]s.
    ///
    /// When `resilient` is true, type conversion failures in function parameters and
    /// return types fall back to `RustType::Any` instead of aborting.
    fn transform_module_item(
        &mut self,
        module_item: &ModuleItem,
        class_map: &HashMap<String, ClassInfo>,
        iface_methods: &HashMap<String, Vec<String>>,
        resilient: bool,
    ) -> Result<(Vec<Item>, Vec<String>)> {
        match module_item {
            ModuleItem::Stmt(Stmt::Decl(decl)) => self.transform_decl(
                decl,
                Visibility::Private,
                class_map,
                iface_methods,
                resilient,
            ),
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => self.transform_decl(
                &export.decl,
                Visibility::Public,
                class_map,
                iface_methods,
                resilient,
            ),
            ModuleItem::ModuleDecl(ModuleDecl::Import(import_decl)) => {
                let items = self.transform_import(import_decl);
                Ok((items, vec![]))
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(export)) => {
                let items = self.transform_export_named(export);
                Ok((items, vec![]))
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportAll(export_all)) => {
                let src = export_all.src.value.to_string_lossy().into_owned();
                if let Some(path) = self.resolve_import_path(&src, "*") {
                    Ok((
                        vec![Item::Use {
                            vis: Visibility::Public,
                            path,
                            names: vec!["*".to_string()],
                        }],
                        vec![],
                    ))
                } else {
                    // External package or unresolvable re-exports are skipped
                    Ok((vec![], vec![]))
                }
            }
            // Top-level expression statements (e.g., `globalThis.crypto ??= crypto`)
            // Rust has no top-level expressions; skip silently
            ModuleItem::Stmt(Stmt::Expr(_)) => Ok((vec![], vec![])),
            _ => Err(UnsupportedSyntaxError::new(
                format_module_item_kind(module_item),
                module_item.span(),
            )
            .into()),
        }
    }

    /// Resolves an import path to a Rust `crate::` module path via ModuleGraph.
    ///
    /// Handles re-export chains (e.g., importing `Config` from `./index`
    /// which re-exports from `./types` → resolves to `crate::types`),
    /// wildcard re-exports (`export * from './types'`), and single-file mode
    /// (where import targets are not in parsed files).
    ///
    /// Returns `None` for external packages or unresolvable specifiers.
    fn resolve_import_path(&self, specifier: &str, name: &str) -> Option<String> {
        self.tctx
            .module_graph
            .resolve_import(self.tctx.file_path, specifier, name)
            .map(|resolved| resolved.module_path)
    }

    /// Transforms an import declaration into IR [`Item::Use`] items.
    ///
    /// Uses `ModuleGraph.resolve_import()` to resolve import paths through re-export chains.
    /// Falls back to heuristic path conversion when resolution fails.
    /// Only relative path imports with named specifiers are converted.
    fn transform_import(&self, import_decl: &swc_ecma_ast::ImportDecl) -> Vec<Item> {
        let src = import_decl.src.value.to_string_lossy().into_owned();

        // Only handle relative imports
        if !src.starts_with("./") && !src.starts_with("../") {
            return vec![];
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
            return vec![];
        }

        // Resolve each name through ModuleGraph to handle re-export chains.
        // Names resolving to different module paths produce separate use items.
        // Unresolvable names (external packages) are silently dropped.
        let mut path_groups: Vec<(String, Vec<String>)> = Vec::new();
        for name in &names {
            if let Some(path) = self.resolve_import_path(&src, name) {
                if let Some(group) = path_groups.iter_mut().find(|(p, _)| p == &path) {
                    group.1.push(name.clone());
                } else {
                    path_groups.push((path, vec![name.clone()]));
                }
            }
        }

        path_groups
            .into_iter()
            .map(|(path, names)| Item::Use {
                vis: Visibility::Private,
                path,
                names,
            })
            .collect()
    }

    /// Transforms a named export into IR [`Item::Use`] items with public visibility.
    ///
    /// - Re-exports (`export { Foo } from "./bar"`) become `pub use bar::Foo;`
    /// - Local name exports (`export { Foo }`) are skipped (declarations are already `pub`)
    ///
    /// Uses `ModuleGraph.resolve_import()` to resolve re-export chains.
    fn transform_export_named(&self, export: &swc_ecma_ast::NamedExport) -> Vec<Item> {
        // Local name exports (no source path) are skipped
        let src = match export.src.as_ref() {
            Some(s) => s,
            None => return vec![],
        };
        let src_str = src.value.to_string_lossy().into_owned();

        // Only handle relative imports
        if !src_str.starts_with("./") && !src_str.starts_with("../") {
            return vec![];
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
            return vec![];
        }

        // Resolve each name through ModuleGraph (same logic as transform_import)
        let mut path_groups: Vec<(String, Vec<String>)> = Vec::new();
        for name in &names {
            if let Some(path) = self.resolve_import_path(&src_str, name) {
                if let Some(group) = path_groups.iter_mut().find(|(p, _)| p == &path) {
                    group.1.push(name.clone());
                } else {
                    path_groups.push((path, vec![name.clone()]));
                }
            }
        }

        path_groups
            .into_iter()
            .map(|(path, names)| Item::Use {
                vis: Visibility::Public,
                path,
                names,
            })
            .collect()
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
        &mut self,
        decl: &Decl,
        vis: Visibility,
        class_map: &HashMap<String, ClassInfo>,
        iface_methods: &HashMap<String, Vec<String>>,
        resilient: bool,
    ) -> Result<(Vec<Item>, Vec<String>)> {
        match decl {
            Decl::TsInterface(interface_decl) => {
                let items = crate::pipeline::type_converter::convert_interface_items(
                    interface_decl,
                    vis,
                    self.synthetic,
                    self.reg(),
                )?;
                Ok((items, vec![]))
            }
            Decl::TsTypeAlias(type_alias_decl) => {
                let items = crate::pipeline::type_converter::convert_type_alias_items(
                    type_alias_decl,
                    vis,
                    self.synthetic,
                    self.reg(),
                )?;
                Ok((items, vec![]))
            }
            Decl::Fn(fn_decl) => {
                let (items, warnings) = self.convert_fn_decl(fn_decl, vis, resilient)?;
                Ok((items, warnings))
            }
            Decl::Class(class_decl) => {
                let items = self.transform_class_with_inheritance(
                    class_decl,
                    vis,
                    class_map,
                    iface_methods,
                )?;
                Ok((items, vec![]))
            }
            Decl::Var(var_decl) => self.convert_var_decl_module_level(var_decl, vis, resilient),
            Decl::TsEnum(ts_enum) => {
                let items = convert_ts_enum(ts_enum, vis)?;
                Ok((items, vec![]))
            }
            Decl::TsModule(ts_module) => {
                // `declare module 'name' { ... }` — process internal declarations
                let mut items = Vec::new();
                let mut warnings = Vec::new();
                if let Some(ast::TsNamespaceBody::TsModuleBlock(block)) = &ts_module.body {
                    for item in &block.body {
                        if let ModuleItem::Stmt(ast::Stmt::Decl(inner_decl)) = item {
                            match self.transform_decl(
                                inner_decl,
                                vis,
                                class_map,
                                iface_methods,
                                resilient,
                            ) {
                                Ok((inner_items, inner_warnings)) => {
                                    items.extend(inner_items);
                                    warnings.extend(inner_warnings);
                                }
                                Err(e) if resilient => {
                                    warnings.push(e.to_string());
                                }
                                Err(e) => return Err(e),
                            }
                        }
                    }
                }
                Ok((items, warnings))
            }
            _ => Err(UnsupportedSyntaxError::new(format_decl_kind(decl), decl.span()).into()),
        }
    }
} // end impl Transformer (module-level transformation)

/// Converts a TS enum declaration into an IR [`Item::Enum`].
///
/// Handles numeric enums (auto-incrementing and explicit values) and string enums.
fn convert_ts_enum(ts_enum: &swc_ecma_ast::TsEnumDecl, vis: Visibility) -> Result<Vec<Item>> {
    let name = crate::ir::sanitize_rust_type_name(&ts_enum.id.sym);
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
        type_params: vec![],
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

/// Injects a `js_typeof` helper function if any item contains `Expr::RuntimeTypeof`.
///
/// The helper maps `serde_json::Value` variants to JavaScript typeof strings at runtime,
/// preserving TypeScript's `typeof` semantics for dynamically-typed values.
fn inject_js_typeof_if_needed(items: &mut Vec<Item>) {
    if !items_contain_runtime_typeof(items) {
        return;
    }
    items.push(Item::RawCode(
        r#"fn js_typeof(val: &serde_json::Value) -> &'static str {
    match val {
        serde_json::Value::String(_) => "string",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Null => "undefined",
        _ => "object",
    }
}"#
        .to_string(),
    ));
}

/// `Expr::RuntimeTypeof` が任意の項目内に存在するかを構造的に検出する visitor。
///
/// I-377 以前は `expr_contains_runtime_typeof` / `stmts_contain_runtime_typeof` /
/// `items_contain_runtime_typeof` の 3 つの手書き再帰関数で実装されており、
/// `Expr::Closure` / `Expr::StructInit` / `Expr::Match` などの variant を
/// `_ => false` で黙殺していた（latent バグ: closure 内の RuntimeTypeof が
/// 未検出になり `js_typeof` helper が注入されない）。`IrVisitor` 化により全
/// variant が `walk_*` で走査されるため、この抜けが構造的に解消される。
#[derive(Default)]
struct RuntimeTypeofDetector {
    found: bool,
}

impl crate::ir::visit::IrVisitor for RuntimeTypeofDetector {
    fn visit_expr(&mut self, expr: &crate::ir::Expr) {
        if self.found {
            return;
        }
        if matches!(expr, crate::ir::Expr::RuntimeTypeof { .. }) {
            self.found = true;
            return;
        }
        crate::ir::visit::walk_expr(self, expr);
    }
}

fn items_contain_runtime_typeof(items: &[Item]) -> bool {
    use crate::ir::visit::IrVisitor;
    let mut detector = RuntimeTypeofDetector::default();
    for item in items {
        detector.visit_item(item);
        if detector.found {
            return true;
        }
    }
    false
}

fn inject_regex_import_if_needed(items: &mut Vec<Item>) {
    if items_contain_regex(items) {
        items.insert(
            0,
            Item::Use {
                vis: Visibility::Private,
                path: "regex".to_string(),
                names: vec!["Regex".to_string()],
            },
        );
    }
}

/// `Expr::Regex` が任意の項目内に存在するかを構造的に検出する visitor。
///
/// `RuntimeTypeofDetector` と同じ理由で `IrVisitor` 化されている：以前の手書き
/// `expr_contains_regex` / `stmts_contain_regex` は `Expr::Closure` や
/// `Expr::IfLet` 内の Regex を検出できなかった（`_ => false` で黙殺）。
#[derive(Default)]
struct RegexDetector {
    found: bool,
}

impl crate::ir::visit::IrVisitor for RegexDetector {
    fn visit_expr(&mut self, expr: &crate::ir::Expr) {
        if self.found {
            return;
        }
        if matches!(expr, crate::ir::Expr::Regex { .. }) {
            self.found = true;
            return;
        }
        crate::ir::visit::walk_expr(self, expr);
    }
}

fn items_contain_regex(items: &[Item]) -> bool {
    use crate::ir::visit::IrVisitor;
    let mut detector = RegexDetector::default();
    for item in items {
        detector.visit_item(item);
        if detector.found {
            return true;
        }
    }
    false
}

/// Builds an `unwrap_or` or `unwrap_or_else` expression for an Option field with a default value.
///
/// Uses `unwrap_or` (eager) only for cheap Copy literals (numbers, bools, unit).
/// Everything else uses `unwrap_or_else` (lazy) to avoid:
/// - Eager evaluation of side-effecting expressions (correctness)
/// - Unnecessary String/struct allocation when Option is Some (performance)
/// - Unconditional move of non-Copy values (ownership safety)
///
/// This is the single source of truth for Option unwrap-with-default generation,
/// used by destructuring defaults, function parameter defaults, and `??` operator.
pub(crate) fn build_option_unwrap_with_default(
    field_access: crate::ir::Expr,
    default_ir: crate::ir::Expr,
) -> crate::ir::Expr {
    if default_ir.is_copy_literal() {
        crate::ir::Expr::MethodCall {
            object: Box::new(field_access),
            method: "unwrap_or".to_string(),
            args: vec![default_ir],
        }
    } else {
        crate::ir::Expr::MethodCall {
            object: Box::new(field_access),
            method: "unwrap_or_else".to_string(),
            args: vec![crate::ir::Expr::Closure {
                params: vec![],
                return_type: None,
                body: crate::ir::ClosureBody::Expr(Box::new(default_ir)),
            }],
        }
    }
}

/// Builds an `init()` function from accumulated top-level expression statements.
///
/// TypeScript modules can have top-level expressions that run once when the module
/// is first imported. Rust has no module-load-time execution, so these are collected
/// into a `pub fn init()` that the consumer can call explicitly.
fn build_init_fn(stmts: Vec<crate::ir::Stmt>) -> Item {
    Item::Fn {
        name: "init".to_string(),
        vis: Visibility::Public,
        attributes: vec![],
        params: vec![],
        return_type: None,
        body: stmts,
        is_async: false,
        type_params: vec![],
    }
}

#[cfg(test)]
pub(crate) mod test_fixtures;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod detector_tests {
    use super::*;
    use crate::ir::{BinOp, CallTarget, ClosureBody, Expr, Param, RustType, Stmt, Visibility};

    fn fn_item(body: Vec<Stmt>) -> Item {
        Item::Fn {
            vis: Visibility::Private,
            attributes: vec![],
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body,
        }
    }

    /// `RuntimeTypeof` nested inside a closure body must be detected.
    ///
    /// 以前の手書き `expr_contains_runtime_typeof` は `Expr::Closure` arm を
    /// `_ => false` で黙殺しており、この入力では false を返していた（latent bug）。
    /// `RuntimeTypeofDetector: IrVisitor` 化により `walk_expr` 経由で
    /// Closure 内部が走査され、正しく true を返す。
    #[test]
    fn runtime_typeof_detected_inside_closure_body() {
        let closure_expr = Expr::Closure {
            params: vec![Param {
                name: "x".to_string(),
                ty: Some(RustType::Any),
            }],
            return_type: None,
            body: ClosureBody::Expr(Box::new(Expr::RuntimeTypeof {
                operand: Box::new(Expr::Ident("x".to_string())),
            })),
        };
        let item = fn_item(vec![Stmt::TailExpr(closure_expr)]);
        assert!(
            items_contain_runtime_typeof(&[item]),
            "RuntimeTypeof inside a closure body must be detected"
        );
    }

    /// `RuntimeTypeof` nested inside a `Match` arm body must be detected.
    #[test]
    fn runtime_typeof_detected_inside_match_arm() {
        let item = fn_item(vec![Stmt::Match {
            expr: Expr::Ident("x".to_string()),
            arms: vec![crate::ir::MatchArm {
                patterns: vec![crate::ir::Pattern::Wildcard],
                guard: None,
                body: vec![Stmt::TailExpr(Expr::RuntimeTypeof {
                    operand: Box::new(Expr::Ident("x".to_string())),
                })],
            }],
        }]);
        assert!(
            items_contain_runtime_typeof(&[item]),
            "RuntimeTypeof inside a match arm must be detected"
        );
    }

    /// Items without `RuntimeTypeof` must return false.
    #[test]
    fn runtime_typeof_absent_returns_false() {
        let item = fn_item(vec![Stmt::TailExpr(Expr::BinaryOp {
            left: Box::new(Expr::Ident("x".to_string())),
            op: BinOp::Add,
            right: Box::new(Expr::IntLit(1)),
        })]);
        assert!(!items_contain_runtime_typeof(&[item]));
    }

    /// `Regex` nested inside a closure body must be detected.
    ///
    /// 以前の手書き `expr_contains_regex` は Closure arm を `_ => false` で
    /// 黙殺していた（latent bug）。
    #[test]
    fn regex_detected_inside_closure_body() {
        let closure_expr = Expr::Closure {
            params: vec![],
            return_type: None,
            body: ClosureBody::Expr(Box::new(Expr::Regex {
                pattern: "abc".to_string(),
                global: false,
                sticky: false,
            })),
        };
        let item = fn_item(vec![Stmt::TailExpr(closure_expr)]);
        assert!(
            items_contain_regex(&[item]),
            "Regex inside a closure body must be detected"
        );
    }

    /// Items without `Regex` must return false.
    #[test]
    fn regex_absent_returns_false() {
        let item = fn_item(vec![Stmt::Expr(Expr::FnCall {
            target: CallTarget::Free("f".to_string()),
            args: vec![],
        })]);
        assert!(!items_contain_regex(&[item]));
    }

    /// FnCall args are walked.
    #[test]
    fn runtime_typeof_detected_inside_fncall_args() {
        let item = fn_item(vec![Stmt::Expr(Expr::FnCall {
            target: CallTarget::Free("wrap".to_string()),
            args: vec![Expr::RuntimeTypeof {
                operand: Box::new(Expr::Ident("x".to_string())),
            }],
        })]);
        assert!(items_contain_runtime_typeof(&[item]));
    }
}
