//! AST to IR transformation.
//!
//! This module converts SWC TypeScript AST nodes into the IR representation
//! defined in [`crate::ir`].

pub mod classes;
pub mod context;
pub mod expressions;
pub mod functions;
pub(crate) mod helpers;
mod injections;
pub mod main_synthesis;
mod namespace_lint;
pub(crate) mod return_wrap;
pub mod statements;
mod ts_enum;
pub(crate) mod type_position;

pub(crate) use main_synthesis::UserMainSubstitution;

pub(crate) use helpers::option_builders::{
    build_option_get_or_insert_with, build_option_or_option, build_option_unwrap_with_default,
};
pub(crate) use type_position::{wrap_trait_for_position, TypePosition};

use namespace_lint::scan_for_ts_namespace_collisions;

use anyhow::Result;
use swc_common::Spanned;
use swc_ecma_ast as ast;
use swc_ecma_ast::{Decl, ImportSpecifier, Module, ModuleDecl, ModuleItem, Stmt};

use std::collections::HashMap;

use crate::ir::{Item, Visibility};
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
    /// I-224 user `main` rename + call substitution mode.
    ///
    /// Set by `transform_module` / `transform_module_collecting` after computing
    /// `(is_executable_mode, user_main_kind)` from the module body. See
    /// [`UserMainSubstitution`] for the dispatch table mapping (= `None` /
    /// `SyncRename` / `AsyncRename`).
    ///
    /// When the variant is non-`None`, two transparent rewrites apply during
    /// IR conversion:
    ///
    /// 1. **Decl emission rename**: `convert_fn_decl` (B1a / B2a function decl)
    ///    and the FnExpr / Arrow init paths in
    ///    `convert_var_decl_module_level` (B1b / B2b / B1c / B2c) substitute
    ///    the emitted `Item::Fn { name }` from `"main"` to
    ///    [`crate::transformer::expressions::TS_MAIN_RENAME`] (= `"__ts_main"`).
    ///    The legacy `is_async && name == "main"` → `#[tokio::main]` attribute
    ///    in `convert_fn_decl` is automatically dropped because the rename
    ///    runs before the attribute computation, so the post-rename name no
    ///    longer matches `"main"`.
    /// 2. **Call substitution**: `convert_call_expr` substitutes
    ///    `Expr::Call { callee: Ident("main"), .. }` to `CallTarget::Free("__ts_main")`
    ///    so every `main()` call site (including the multi-call boundary cases
    ///    locked in by INV-2) targets the renamed user main rather than the
    ///    synthesized binary entry. For [`UserMainSubstitution::AsyncRename`]
    ///    (B2 + executable mode) the substituted call is additionally wrapped in
    ///    [`crate::ir::Expr::Await`] so the synthesized
    ///    `#[tokio::main] async fn main()` awaits the renamed `__ts_main().await`
    ///    — without this wrap the returned `Future` would be silently dropped
    ///    (= Iteration v11 2026-05-08 Tier 1 silent-loss fix for cells 11 / 23 / 75).
    ///
    /// **Inheritance to nested scopes**: `spawn_nested_scope` /
    /// `spawn_nested_scope_with_local_synthetic` propagate this flag verbatim
    /// so call substitution fires uniformly within function bodies, arrow
    /// bodies, class methods, etc.
    ///
    /// **Library mode + B1 / B2 (cells 3 / 5 / 23 / 25)**: this flag stays
    /// [`UserMainSubstitution::None`] because the user's `main` is the binary
    /// entry point directly per the `LibraryFnSyncDirect` /
    /// `LibraryFnAsyncDirect` arms — no rename or substitution required. The
    /// dispatch tree's `(false, FnSync, false)` and `(false, FnAsync, false)`
    /// arms in `Transformer::synthesize_fn_main` correspondingly emit nothing,
    /// leaving `transform_decl`'s emit as the binary entry.
    pub(crate) user_main_substitution: UserMainSubstitution,
    /// I-224 T5-2 deep review structural fix (2026-05-08, Iteration v11): suppress the
    /// substitute-time `.await` wrap when the substituted call is itself wrapped by a
    /// caller-supplied `Expr::Await` (= source-level `await main();` or top-level capture
    /// of `await main();` / `const x = await main();`).
    ///
    /// **Why this flag exists**: `convert_call_expr` synthesizes a `.await` wrap for B2
    /// async user main substitute calls so that the renamed `__ts_main()` runs to
    /// completion (= Iteration v11 Tier 1 silent-loss fix for cells 11 / 23 / 75). When
    /// the source itself contains `await main();`, the **enclosing context already
    /// supplies the `.await`** (via the source-level `Expr::Await` arm of `convert_expr`
    /// or the top-level capture's `Expr::Await` branch in
    /// `try_capture_module_item_into_main_stmts` / `capture_var_decl_into_main_stmts`).
    /// Without suppression the substitute would emit an inner `Expr::Await` and the outer
    /// wrap would emit another, producing the double-`.await` bug
    /// `__ts_main().await.await;` (= compile error in cells 16 / 30 / 36 fixtures).
    ///
    /// **Lifecycle**: the flag is push/pop'd at the three `Expr::Await` entry sites
    /// (= the three places where the **caller** guarantees a `.await` wrap will be
    /// emitted around the converted child IR):
    ///
    /// 1. `convert_expr`'s `Expr::Await(_)` arm (= source-level `await x;` inside any
    ///    expression context, e.g. user-defined async fn body).
    /// 2. `try_capture_module_item_into_main_stmts` Stmt::Expr arm's `Expr::Await(_)`
    ///    branch (= top-level `await main();`).
    /// 3. `capture_var_decl_into_main_stmts` `Expr::Await(_)` branch (= top-level
    ///    `const x = await main();`).
    ///
    /// At each entry the flag is set to `true` before recursing into the awaitee, then
    /// restored on exit. The substitute logic in `convert_call_expr` checks this flag
    /// and skips the synthesized `.await` wrap when it is set.
    ///
    /// **Inheritance to nested scopes**: propagated by `spawn_nested_scope` /
    /// `spawn_nested_scope_with_local_synthetic` so context-aware suppression survives
    /// scope boundaries (= an arrow / fn-expr / class-method body inside an `await`
    /// awaitee position retains the suppression).
    pub(crate) suppress_main_await_wrap: bool,
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
            user_main_substitution: UserMainSubstitution::None,
            suppress_main_await_wrap: false,
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
            user_main_substitution: self.user_main_substitution,
            suppress_main_await_wrap: self.suppress_main_await_wrap,
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
            user_main_substitution: self.user_main_substitution,
            suppress_main_await_wrap: self.suppress_main_await_wrap,
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

/// Transforms an SWC [`Module`] with a pre-built [`TransformContext`], collecting
/// unsupported syntax instead of aborting.
///
/// Used by tests (e.g., `TctxFixture::transform_collecting`) that need both
/// TypeResolver-populated type information *and* the list of
/// [`UnsupportedSyntaxError`]s. Unlike [`transform_module_collecting`], which
/// builds a bare `TransformContext` from just a [`TypeRegistry`], this variant
/// reuses the caller's context so features driven by TypeResolver output
/// (narrowing, `get_type_for_var`, `get_emission_hint`, etc.) are available —
/// e.g., I-142 Cell #5 / #9 / #14 which need `any` / `number | null` parameter
/// types to resolve before the transformer's `pick_strategy` / `??=`
/// emission-hint dispatch runs.
///
/// The public-API parallel to [`transform_module_with_context`].
pub fn transform_module_collecting_with_context(
    module: &Module,
    ctx: &context::TransformContext<'_>,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<(Vec<Item>, Vec<UnsupportedSyntaxError>)> {
    let mut t = Transformer::for_module(ctx, synthetic);
    t.transform_module_collecting(module)
}

// --- Transformer methods for module-level transformation ---

impl<'a> Transformer<'a> {
    /// Transforms an SWC [`Module`] into IR items.
    ///
    /// The file's crate-relative directory (used for `../` import path resolution) is
    /// derived from `self.tctx.file_path` via [`current_file_dir()`](Self::current_file_dir).
    pub(crate) fn transform_module(&mut self, module: &Module) -> Result<Vec<Item>> {
        // I-224 collision-detection (INV-5 highest precedence): reject
        // user-defined `__ts_main` (and any other `__ts_`-prefixed module-level
        // identifier) before any A-axis structural dispatch. In abort mode the
        // first collision becomes the function's `Err` return.
        if let Some(collision) = scan_for_ts_namespace_collisions(module).into_iter().next() {
            return Err(collision.into());
        }

        // I-224 T3-2 + T4-1: pre-compute (`is_executable_mode`, `user_main_kind`,
        // `has_top_level_await`) and activate the user `main` rename + call
        // substitution gate **before** any `convert_expr` call (the gate is
        // consumed by `convert_call_expr` / `convert_fn_decl` /
        // `convert_var_decl_module_level` during emission). Library mode + B1/B2
        // (cells 3 / 5 / 23 / 25) keeps the gate `false` so the user's `main`
        // remains the binary entry directly per the `LibraryFnSyncDirect` /
        // `LibraryFnAsyncDirect` dispatch arms.
        let exec_mode = main_synthesis::is_executable_mode(module);
        let user_main_kind = main_synthesis::detect_user_main(module);
        let has_top_level_await_flag = main_synthesis::has_top_level_await(module);
        self.user_main_substitution =
            UserMainSubstitution::from_dispatch(exec_mode, user_main_kind);

        // Pre-scan: collect class info for inheritance resolution. Must precede
        // both the capture pass (`try_capture_module_item_into_main_stmts` →
        // `convert_expr`) and the emit pass (`transform_module_item` →
        // `transform_decl`); `build_mut_method_names` mutates Transformer state
        // consumed by both.
        let class_map = self.pre_scan_classes(module);
        let iface_methods = classes::pre_scan_interface_methods(module);
        self.build_mut_method_names(&class_map);

        // Single-pass dispatch (I-224 T4-1 unification): each ModuleItem is
        // routed to exactly one of three sinks per `try_capture_module_item_into_main_stmts`:
        //   • silent skip (`Stmt::Empty` — pre-arm continue)
        //   • capture into `main_stmts` (Stmt::Expr / Decl::Var FnMainBodyCapture /
        //     ExportDecl-wrapped Decl::Var FnMainBodyCapture)
        //   • emit as Item via `transform_module_item`
        // The capture and emit sinks are mutually exclusive (= the same item
        // never produces both a `MainStmt` and an `Item`), guaranteed by the
        // `try_capture` return value (`Ok(true)` short-circuits the emit pass).
        let mut items = Vec::new();
        let mut main_stmts = Vec::new();
        for module_item in &module.body {
            // I-224 T3-4: A5a (`Stmt::Empty`) silent skip per PRD Design section #3
            // ("Stmt::Empty: silent skip per the per-item dispatch table; no
            // capture"). Pre-arm continue keeps the body terse and avoids
            // routing Empty through the capture/emit dispatch — both downstream
            // sinks treat Empty as no-op anyway, but the pre-arm makes the
            // silent-skip semantic explicit at the loop level.
            if matches!(module_item, ModuleItem::Stmt(Stmt::Empty(_))) {
                continue;
            }
            let captured = self.try_capture_module_item_into_main_stmts(
                module_item,
                exec_mode,
                &mut main_stmts,
            )?;
            if captured {
                continue;
            }
            let (converted, _warnings) =
                self.transform_module_item(module_item, &class_map, &iface_methods, false)?;
            items.extend(converted);
        }

        // T3 synthesis (I-224 T4-1 wiring): produce `fn main` Items per
        // dispatch arm. Library arms emit nothing (returns empty Vec); executable
        // arms emit the synthesized `fn main()` (sync) or `#[tokio::main] async
        // fn main()` (async) wrapping `main_stmts` in source order (= INV-1).
        let synthesized =
            self.synthesize_fn_main(main_stmts, user_main_kind, has_top_level_await_flag);
        items.extend(synthesized);

        injections::inject_regex_import_if_needed(&mut items);
        injections::inject_js_typeof_if_needed(&mut items);
        Ok(items)
    }

    /// Transforms an SWC [`Module`], collecting unsupported syntax instead of aborting.
    pub(crate) fn transform_module_collecting(
        &mut self,
        module: &Module,
    ) -> Result<(Vec<Item>, Vec<UnsupportedSyntaxError>)> {
        // I-224 collision-detection (INV-5 highest precedence): all
        // user-defined `__ts_`-prefixed module-level identifiers are
        // accumulated as Tier 2 honest errors before any A-axis structural
        // dispatch. In collecting mode the scan seeds the `unsupported`
        // accumulator with every offending identifier; the rest of the module
        // continues to be transformed for partial output.
        let mut unsupported: Vec<UnsupportedSyntaxError> = scan_for_ts_namespace_collisions(module);

        // I-224 T3-2 + T4-1 (= symmetric with `transform_module`): pre-compute
        // dispatch-tree inputs and activate the rename / call-substitution gate
        // before any `convert_expr` call. Flag activation runs after the
        // namespace-lint scan so collision-mode modules with user `main` still
        // get the rename gate set; the rename applies to non-collision
        // identifiers and the namespace lint accumulates the `__ts_main`
        // collision separately.
        let exec_mode = main_synthesis::is_executable_mode(module);
        let user_main_kind = main_synthesis::detect_user_main(module);
        let has_top_level_await_flag = main_synthesis::has_top_level_await(module);
        self.user_main_substitution =
            UserMainSubstitution::from_dispatch(exec_mode, user_main_kind);

        let class_map = self.pre_scan_classes(module);
        let iface_methods = classes::pre_scan_interface_methods(module);
        self.build_mut_method_names(&class_map);

        // Single-pass dispatch (I-224 T4-1 unification、symmetric with
        // `transform_module`). Errors from the capture pass and the emit pass
        // are accumulated into `unsupported` and the loop continues; this
        // mirrors the existing collecting-mode contract that partial output is
        // preferable to abort.
        let mut items = Vec::new();
        let mut main_stmts = Vec::new();
        for module_item in &module.body {
            // A5a (`Stmt::Empty`) silent skip per PRD Design section #3 (=
            // symmetric with `transform_module`'s loop).
            if matches!(module_item, ModuleItem::Stmt(Stmt::Empty(_))) {
                continue;
            }
            match self.try_capture_module_item_into_main_stmts(
                module_item,
                exec_mode,
                &mut main_stmts,
            ) {
                // Captured into main_stmts → skip the emit pass for this item.
                Ok(true) => continue,
                // Not in capture scope → fall through to the emit pass.
                Ok(false) => {}
                // Capture attempt failed (convert_expr error) → record and skip
                // emission for this item to preserve the "single sink" invariant.
                Err(e) => {
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

        // T3 synthesis (= symmetric with `transform_module`).
        let synthesized =
            self.synthesize_fn_main(main_stmts, user_main_kind, has_top_level_await_flag);
        items.extend(synthesized);

        injections::inject_regex_import_if_needed(&mut items);
        injections::inject_js_typeof_if_needed(&mut items);
        Ok((items, unsupported))
    }

    /// Transforms a single module item into IR [`Item`]s.
    ///
    /// When `resilient` is true, type conversion failures in function parameters and
    /// return types fall back to `RustType::Any` instead of aborting.
    ///
    /// **Rule 11 (d-1) self-applied compliance** (T4-2): every [`ModuleItem`] /
    /// [`ModuleDecl`] / [`Stmt`] variant is enumerated explicitly. New SWC AST
    /// variants force a compile error here so every dispatch site is updated in
    /// lock-step. Variants that are pre-skipped or pre-captured by the caller
    /// (`Transformer::transform_module` / `transform_module_collecting`) are
    /// matched with [`unreachable!`] documenting the structural invariant — they
    /// never reach this method in production.
    fn transform_module_item(
        &mut self,
        module_item: &ModuleItem,
        class_map: &HashMap<String, ClassInfo>,
        iface_methods: &HashMap<String, Vec<String>>,
        resilient: bool,
    ) -> Result<(Vec<Item>, Vec<String>)> {
        match module_item {
            // ============== Decl-bearing items (private / public visibility) ==============
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

            // ============== Import / re-export items ==============
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

            // ============== Default exports / TS legacy module forms (Tier 2 honest reject) ==============
            // `export default <decl>` (named function / class / interface) and the
            // anonymous `export default <expr>` form have no direct Rust equivalent at
            // the module-item level (Rust's binary-entry convention is `fn main()`,
            // not arbitrary default exports). The TS-only `import =` / `export =` /
            // `export as namespace` forms are vestiges of the CommonJS / namespace
            // module systems and have no Rust counterpart. All five are reported as
            // Tier 2 honest errors with the SWC kind name; downstream tooling can
            // disambiguate via the span.
            ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(_))
            | ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultExpr(_))
            | ModuleItem::ModuleDecl(ModuleDecl::TsImportEquals(_))
            | ModuleItem::ModuleDecl(ModuleDecl::TsExportAssignment(_))
            | ModuleItem::ModuleDecl(ModuleDecl::TsNamespaceExport(_)) => {
                Err(UnsupportedSyntaxError::new(
                    format_module_item_kind(module_item),
                    module_item.span(),
                )
                .into())
            }

            // ============== A4 (control-flow) — Tier 2 honest reject with guidance ==============
            // Top-level control-flow statements (`if`, loops, `try`, etc.) execute
            // at module-load time in TS but have no Rust module-item analogue —
            // Rust has no top-level execution context outside `fn main()`. The
            // `fn main()` synthesis (T3 / T4-1) captures `Stmt::Expr` and side-effect
            // `Decl::Var` but does **not** wrap A4 statements: doing so would
            // require lifting all referenced bindings into the synthesized fn main
            // body, changing scope semantics for any subsequent declaration that
            // reads / writes them. The PRD I-224 design partition reserves A4 for
            // a future expansion (= I-203 codebase-wide AST exhaustiveness +
            // structural lift) where the user's intent of "module-load control
            // flow" can be surfaced explicitly.
            //
            // Spec stage Axis A4 cells: `assert_ts_namespace_collision`-style
            // tests cover the wording substring `ControlFlow at top-level`.
            ModuleItem::Stmt(
                Stmt::Block(_)
                | Stmt::If(_)
                | Stmt::Switch(_)
                | Stmt::Throw(_)
                | Stmt::Try(_)
                | Stmt::While(_)
                | Stmt::DoWhile(_)
                | Stmt::For(_)
                | Stmt::ForIn(_)
                | Stmt::ForOf(_)
                | Stmt::Labeled(_)
                | Stmt::Continue(_)
                | Stmt::Break(_)
                | Stmt::Return(_)
                | Stmt::With(_),
            ) => Err(UnsupportedSyntaxError::new(
                format!(
                    "ControlFlow at top-level requires fn main wrapping; lift to a \
                     named function or use I-203 future expansion ({})",
                    format_module_item_kind(module_item),
                ),
                module_item.span(),
            )
            .into()),

            // ============== A5b (Debugger) — Tier 2 honest reject with guidance ==============
            // The TS `debugger;` statement signals a debugger breakpoint at module
            // load. Rust has no built-in debugger-breakpoint statement; the user
            // selects an explicit alternative themselves (compile-time `panic!()`
            // for hard stop, or `std::dbg!(...)` for value tracing). Reporting as
            // Tier 2 with explicit guidance is more useful than the generic
            // SWC-kind wording.
            ModuleItem::Stmt(Stmt::Debugger(_)) => Err(UnsupportedSyntaxError::new(
                "`debugger` statement has no Rust equivalent (use `panic!()` for \
                 a hard stop or `std::dbg!()` for value tracing per the user's \
                 intent)"
                    .to_string(),
                module_item.span(),
            )
            .into()),

            // ============== Pre-handled by the caller — defensive unreachable!() ==============
            // `Stmt::Empty` is silently skipped by `transform_module(_collecting)`'s
            // pre-arm `continue`. `Stmt::Expr` is captured into `main_stmts` by
            // `try_capture_module_item_into_main_stmts` (in executable mode) or
            // is structurally impossible (library mode contains no Stmt::Expr per
            // the `is_executable_mode` definition). Reaching this method with
            // either variant indicates a bug in the caller's dispatch.
            ModuleItem::Stmt(Stmt::Empty(_)) => unreachable!(
                "Stmt::Empty must be silently skipped before reaching transform_module_item; \
                 the pre-arm `continue` in transform_module(_collecting) is the contract"
            ),
            ModuleItem::Stmt(Stmt::Expr(_)) => unreachable!(
                "Stmt::Expr must be captured into main_stmts (= try_capture returns Ok(true)) \
                 before reaching transform_module_item; library mode contains no Stmt::Expr \
                 per the is_executable_mode invariant, and executable mode unconditionally \
                 captures via try_capture_module_item_into_main_stmts"
            ),
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
                let items = ts_enum::convert_ts_enum(ts_enum, vis)?;
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
pub(crate) mod test_fixtures;
#[cfg(test)]
mod tests;
