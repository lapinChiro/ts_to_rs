//! I-224 fn main mechanism: top-level execution capture + axis classification +
//! dispatch arm derivation.
//!
//! This module is the foundational layer for I-224's `fn main` mechanism (PRD α-1).
//! It provides the IR enums, predicates, and the [`Transformer::collect_top_level_executions`]
//! shared helper that turn a SWC [`Module`] into the inputs of the dispatch tree
//! (Design section #2 of the PRD): `(is_executable_mode, user_main_kind,
//! has_top_level_await)`.
//!
//! # Layered architecture (read top-to-bottom for the call graph)
//!
//! 1. **IR enums** ([`MainStmt`], [`UserMainKind`], [`InitKind`], [`DeclVarPath`],
//!    [`DispatchArm`]): classification result types consumed by Implementation Stage T3
//!    ([`Transformer::synthesize_fn_main`], not yet implemented).
//! 2. **Per-init-shape predicates** ([`classify_init_kind`], [`has_side_effect_init`],
//!    [`classify_decl_var_path`]): operate on a single `Decl::Var` and decide its
//!    Library / Toplevel-const / Fn-main-body-capture path (PRD Design section #3
//!    "per-item runtime decision").
//! 3. **Module-level predicates** ([`is_executable_mode`], [`detect_user_main`]):
//!    walk the module body and return `(is_executable_mode: bool,
//!    user_main_kind: UserMainKind, has_top_level_await: bool)` axis values.
//! 4. **Dispatch arm classifier** ([`classify_dispatch_arm`]): turns the 3-tuple into
//!    one of 13 [`DispatchArm`] leaves + 1 structurally unreachable arm
//!    (`(false, *, true)` after Collision absorption — locked-in by
//!    `tests/swc_parser_top_level_await_test.rs`).
//! 5. **Capture helper** ([`Transformer::collect_top_level_executions`]): the
//!    single shared scan that produces the dispatch tree's inputs **plus** the
//!    Vec<MainStmt> capture for fn main body emission.
//!
//! # Rule 11 (d-1) self-applied compliance
//!
//! Every `match` over `Stmt` / `Decl` / `ModuleItem` / `ast::DefaultDecl` enumerates
//! variants explicitly — no `_ =>` arms. This is the same standard the PRD applies
//! to `transform_module_item`'s `_` arm refactor (T4-2). New SWC AST variants will
//! produce compile errors here, forcing every dispatch site to be updated together.
//! Inner-binding `_` placeholders inside enumerated arms (e.g., `Decl::Fn(_)`) are not
//! `_ =>` arms and remain permitted.
//!
//! # Integration status
//!
//! Implementation Stage T2 lands the helper as a **standalone foundation**: the
//! production [`Transformer::transform_module`] / `transform_module_collecting`
//! still uses the legacy `init_stmts` + `pub fn init` mechanism. Integration is
//! T4's responsibility (`transform_module` refactor + `pub fn init` removal).
//! The 80-cell unit tests + `tests/i224_helper_test.rs::test_dispatch_arm_one_to_one_mapping_per_in_scope_cell`
//! exercise this module directly and lock in the dispatch tree's Rule 9 (a) 1-to-1
//! mapping invariant ahead of T3/T4 emission code landing.
//!
//! Most public items (`UserMainKind`, `DispatchArm`, `classify_dispatch_arm`,
//! `is_executable_mode`, `detect_user_main`, `has_top_level_await`) are exercised by
//! the external integration tests (`tests/i224_helper_test.rs`,
//! `tests/i224_invariants_test.rs`); they are reachable from the test target and
//! never trigger the `dead_code` lint. Four items, however, are consumed only by
//! Implementation Stage T3 / T4-1 and are unreachable from any current call site:
//!
//! - [`MainStmt`] — fn main body capture variants emitted by T3's `synthesize_fn_main`.
//! - [`DeclVarPath`] — return type of [`classify_decl_var_path`], consumed by both
//!   [`Transformer::collect_top_level_executions`] and T4-1's per-item routing.
//! - [`classify_decl_var_path`] — predicate consumed by T4-1's per-item routing.
//! - [`Transformer::collect_top_level_executions`] — wired into `transform_module`
//!   by T4-1.
//!
//! Per-item `#[allow(dead_code)]` records the staging intent and points to the
//! consumer task. T4-1 will remove each `#[allow]` once the production call site
//! exists. The pattern follows
//! [`crate::transformer::expressions::TS_MAIN_RENAME`] (declared in T1-1, consumed
//! by T1-2 / T3).

use anyhow::Result;
use swc_ecma_ast::{self as ast, Decl, Expr, Module, ModuleDecl, ModuleItem, Stmt, VarDecl};

use crate::ir::{Expr as IrExpr, Item, Stmt as IrStmt, Visibility};
use crate::transformer::Transformer;

// Recursive Await walker sub-module (= I-228 main fix per Spec stage 逆戻り
// 2026-05-07). Hand-rolled walker covering all 38 SWC Expr variants explicitly,
// extracted to await_walker.rs to keep mod.rs under the 1000-line file-line check
// threshold while preserving Rule 11 (d-1) self-applied compliance.
mod await_walker;
use await_walker::{class_contains_await_recursive, expr_contains_await_recursive};

// User `main` detection sub-module (B-axis classification + ambient filter +
// `__ts_main` collision precedence). Extracted from mod.rs for the same
// file-line reason as await_walker.
mod user_main;
#[doc(hidden)] // I-224 internal predicate, exposed for external integration tests.
pub use user_main::detect_user_main;

// Decl::Var initializer classification (`InitKind` / `DeclVarPath` enums +
// `classify_init_kind` / `has_side_effect_init` / `classify_decl_var_path` +
// `expr_init_kind` private helper). Extracted from mod.rs for the same
// file-line reason as await_walker / user_main.
mod init_classifier;
// Items used directly in mod.rs (via `Self::capture_var_decl_into_main_stmts`
// and the `Decl::Var` arm of `collect_top_level_executions`) — `DeclVarPath`,
// `has_side_effect_init`, `classify_decl_var_path`.
pub(crate) use init_classifier::{classify_decl_var_path, has_side_effect_init, DeclVarPath};
// Items consumed by the unit tests in `tests/mod.rs` (via `super::*`) and by
// callers of `classify_init_kind` outside this module. `#[allow(unused_imports)]`
// records that `mod.rs` itself does not directly use these symbols — they are
// re-exported for downstream consumers.
#[allow(unused_imports)]
pub(crate) use init_classifier::{classify_init_kind, InitKind};

/// Top-level execution statement, captured into the synthesized `fn main` body.
///
/// Each variant corresponds to one row of the per-item dispatch table in PRD Design
/// section #3 ("Top-level execution stmt capture + per-item runtime decision"):
///
/// | TS source shape | MainStmt variant | Rust emission (T3 will implement) |
/// |---|---|---|
/// | `console.log(...);` (Stmt::Expr, non-await) | [`MainStmt::Expr`] | `<expr>;` |
/// | `await fetch();` (Stmt::Expr Await) | [`MainStmt::ExprAwait`] | `<inner>.await;` |
/// | `const c = compute();` (Decl::Var side-effect init) | [`MainStmt::Let`] | `let c = <init>;` |
/// | `const c = await fetch();` (Decl::Var await init) | [`MainStmt::LetAwait`] | `let c = <inner>.await;` |
///
/// **Await-variant invariant**: `ExprAwait(inner)` and `LetAwait { init, .. }` both
/// store the **awaitee** (the operand of `await`), not the `Expr::Await(...)` wrapper.
/// T3 emission applies `.await` based on the variant tag. This makes ExprAwait /
/// LetAwait symmetric with the Rust syntax `<expr>.await;` / `let x = <expr>.await;`.
#[allow(dead_code)]
// Consumed by T3 (`Transformer::synthesize_fn_main` body emission)
// and T4-1 (`transform_module` integration of the capture path).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MainStmt {
    /// Synchronous expression statement: `<expr>;`.
    Expr(IrExpr),
    /// Top-level awaited expression statement: emits `<inner>.await;` in async fn main.
    ExprAwait(IrExpr),
    /// Side-effect / non-literal Decl::Var capture: emits `let <name> = <init>;`.
    Let { name: String, init: IrExpr },
    /// Await-init Decl::Var capture: emits `let <name> = <init>.await;` in async fn main.
    LetAwait { name: String, init: IrExpr },
}

/// User-defined `main` symbol classification (Axis B of the PRD problem space).
///
/// Public to support `tests/i224_helper_test.rs::test_dispatch_arm_one_to_one_mapping_per_in_scope_cell`,
/// which composes [`is_executable_mode`] / [`detect_user_main`] / [`has_top_level_await`]
/// with [`classify_dispatch_arm`] to lock in the Rule 9 (a) 1-to-1 mapping invariant.
///
/// Detected by [`detect_user_main`] from a single pass over the module body.
/// Determines the `user_main_kind` dimension of the dispatch tree (Design section #2).
///
/// **Collision precedence**: when multiple module items introduce names matching either
/// `main` or `__ts_main`, [`UserMainKind::Collision`] takes precedence over all other
/// kinds (= INV-5 priority arm at the dispatch level — independent of, and complementary
/// to, the namespace-lint rejection performed by `scan_for_ts_namespace_collisions`
/// in `transformer/mod.rs`).
#[doc(hidden)]
// I-224 internal classification, exposed publicly only for external
// integration tests; not part of the documented public API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserMainKind {
    /// B0: no user-defined `main` symbol.
    None,
    /// B1: synchronous `function main` / sync arrow init / sync fn-expr init.
    FnSync,
    /// B2: async `function main` / async arrow init / async fn-expr init.
    FnAsync,
    /// B3: non-callable `main` symbol — `class main` / `interface main` /
    /// `type main = ...` / `enum main` / `namespace main` / `const main = <non-callable>`.
    NonFn,
    /// B4: user-defined `__ts_main` identifier — collides with the synthesized rename
    /// target [`crate::transformer::expressions::TS_MAIN_RENAME`].
    Collision,
}

/// Identifier of the dispatch tree leaf (PRD Design section #2) selected by a
/// `(is_executable_mode, user_main_kind, has_top_level_await)` 3-tuple.
///
/// **Rule 9 (a) 1-to-1 mapping**: each in-scope matrix cell of the PRD problem space
/// maps to exactly one variant of this enum; conversely, each variant lists the
/// matrix cells it covers in the corresponding `match` arm of [`classify_dispatch_arm`].
/// `tests/i224_helper_test.rs::test_dispatch_arm_one_to_one_mapping_per_in_scope_cell`
/// locks this invariant in.
///
/// Naming convention: `<Mode><UserMain>[Async]` where `Mode` is `Library` / `Exec`,
/// `UserMain` is `None` / `FnSync` / `FnAsync` / `NonFn`, and `Async` suffix marks
/// `has_top_level_await=true`.
#[doc(hidden)]
// I-224 internal dispatch leaf, exposed publicly only for external
// integration tests; not part of the documented public API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchArm {
    /// `(_, Collision, _)` — INV-5 priority arm; Tier 2 honest reject.
    /// Covers matrix cells 9 / 19 / 20 / 29 / 39 / 40 / 79 / 80 (+ A4/A5a/A5b-merged
    /// 49 / 59 / 69 via the namespace-lint upstream path).
    Collision,
    /// `(false, None, false)` — library mode, declarations only, no fn main.
    /// Covers cells 1 / 21.
    LibraryNone,
    /// `(false, FnSync, false)` — library mode, sync user main directly emit.
    /// Covers cells 3 / 23.
    LibraryFnSyncDirect,
    /// `(false, FnAsync, false)` — library mode, async user main directly emit.
    /// Covers cells 5 / 25.
    LibraryFnAsyncDirect,
    /// `(false, NonFn, false)` — library mode, non-fn `main` preserved.
    /// Covers cells 7 / 27.
    LibraryNonFn,
    /// `(true, None, false)` — executable mode, no user main, sync fn main synthesis.
    /// Covers cells 11 / 31 / 71.
    ExecNoneSync,
    /// `(true, FnSync, false)` — executable mode, sync user main rename + sync synthesis.
    /// Covers cells 13 / 33 / 73.
    ExecFnSyncRename,
    /// `(true, FnAsync, false)` — executable mode, async user main rename +
    /// `#[tokio::main]` synthesis (FnAsync triggers async dispatch even with no top-await).
    /// Covers cells 15 / 35 / 75.
    ExecFnAsyncRename,
    /// `(true, NonFn, false)` — executable mode, non-fn `main` preserved + sync synthesis.
    /// Covers cells 17 / 37 / 77.
    ExecNonFnSync,
    /// `(true, None, true)` — executable mode, no user main, top-await capture +
    /// `#[tokio::main]` synthesis.
    /// Covers cells 12 / 32 / 72.
    ExecNoneAsync,
    /// `(true, FnSync, true)` — sync user main + top-await cohabitation (cell 14 edge).
    /// Async fn main wraps non-await sync `__ts_main()` call.
    /// Covers cells 14 / 34 / 74.
    ExecFnSyncRenameAsync,
    /// `(true, FnAsync, true)` — Trigger 1 + Trigger 2 combined (= async user main +
    /// top-await).
    /// Covers cells 16 / 36 / 76.
    ExecFnAsyncRenameAsync,
    /// `(true, NonFn, true)` — non-fn preserved + top-await capture + `#[tokio::main]`.
    /// Covers cells 18 / 38 / 78.
    ExecNonFnAsync,
}

/// Returns `true` iff the module body contains any A1 (`Stmt::Expr`) or A3
/// (`Decl::Var` with side-effect / await init) top-level statement.
///
/// This is the predicate that distinguishes "library mode" (no synthesized fn main)
/// from "executable mode" (synthesized fn main wraps the captured execution stmts).
///
/// **Rule 11 (d-1) self-applied compliance**: every `Stmt` variant is enumerated; new
/// SWC variants produce a compile error and force this predicate to be updated.
#[doc(hidden)] // I-224 internal predicate, exposed for external integration tests.
pub fn is_executable_mode(module: &Module) -> bool {
    module.body.iter().any(|item| match item {
        ModuleItem::Stmt(stmt) => match stmt {
            // === A1 partition (Stmt::Expr) — execution trigger ===
            Stmt::Expr(_) => true,

            // === A3 partition (Decl::Var with side-effect / await init) — runtime check ===
            Stmt::Decl(Decl::Var(var)) => has_side_effect_init(var),

            // === Class declaration with outer-context await (I-228 main scope
            // extension): `class C extends f(await x) {}` evaluates the super_class
            // / decorators / member computed keys at class-definition time (=
            // module-load) in the outer async context. await reachability there
            // requires async fn main, so this is an executable-mode trigger. ===
            Stmt::Decl(Decl::Class(class_decl)) => {
                class_contains_await_recursive(&class_decl.class)
            }

            // === Declarations partition — type-system / namespace only, no execution ===
            Stmt::Decl(
                Decl::Fn(_)
                | Decl::TsInterface(_)
                | Decl::TsTypeAlias(_)
                | Decl::TsEnum(_)
                | Decl::TsModule(_)
                | Decl::Using(_),
            ) => false,

            // === A5a (Stmt::Empty) — silent skip target, no execution ===
            Stmt::Empty(_) => false,

            // === A5b (Stmt::Debugger) — Tier 2 honest reclassify (T4-2 work) ===
            // (rejected upstream by `transform_module_item` after T4-2; before T4-2 the
            // legacy `_ => Err` path produces the same Tier 2 reject. Either way,
            // Debugger does not contribute to executable_mode.)
            Stmt::Debugger(_) => false,

            // === A4 partition (control-flow) — Tier 2 honest reject (T4-2 work) ===
            // (same upstream-reject reasoning as A5b; not an executable trigger.)
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
            | Stmt::With(_) => false,
        },

        // === ExportDecl-wrapped Decl::Var with side-effect / await init (I-228-c
        // fix、Spec stage 逆戻り 2026-05-07): semantically belongs to Axis A3
        // partition (= module-load runtime evaluation), so it triggers exec mode
        // even though the outer ModuleItem is ModuleDecl. The PRD Axis E
        // orthogonality claim ("export presence is orthogonal to A-axis") holds
        // for Lit-init export const but NOT for side-effect-init export const,
        // which requires fn main body capture for Rust-compilable emission. ===
        ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => match &export.decl {
            Decl::Var(var) => has_side_effect_init(var),
            // ExportDecl-wrapped Class: same outer-context await detection as bare
            // Decl::Class (see I-228 main scope extension above).
            Decl::Class(class_decl) => class_contains_await_recursive(&class_decl.class),
            // Other ExportDecl-wrapped Decl variants (Fn / Interface / TypeAlias /
            // Enum / Module / Using): no executable trigger.
            Decl::Fn(_)
            | Decl::TsInterface(_)
            | Decl::TsTypeAlias(_)
            | Decl::TsEnum(_)
            | Decl::TsModule(_)
            | Decl::Using(_) => false,
        },

        // === Module-level declarations (Axis E E1 partition、non-ExportDecl) ===
        // Imports / re-exports / namespace exports / default exports etc. preserve
        // their semantics regardless of executable_mode (per the PRD Axis E
        // orthogonality probe); the inner ModuleDecl variant is I-203 scope per
        // Rule 11 (d-6) Architectural concern relevance.
        ModuleItem::ModuleDecl(
            ModuleDecl::Import(_)
            | ModuleDecl::ExportNamed(_)
            | ModuleDecl::ExportAll(_)
            | ModuleDecl::ExportDefaultDecl(_)
            | ModuleDecl::ExportDefaultExpr(_)
            | ModuleDecl::TsImportEquals(_)
            | ModuleDecl::TsExportAssignment(_)
            | ModuleDecl::TsNamespaceExport(_),
        ) => false,
    })
}

/// AST-level scan that returns `true` iff the module body contains a top-level
/// `await` expression reachable WITHOUT crossing a function / arrow / class body
/// boundary (= same async context as the synthesized fn main body).
///
/// **Spec stage 逆戻り (2026-05-07、I-228 fix)**: revised from "AST shape direct
/// only" (= `Stmt::Expr(Expr::Await)` / `Decl::Var.init = Expr::Await(_)`) to
/// "**recursive walk** of top-level Stmt::Expr / Decl::Var.init / ExportDecl-
/// wrapped Decl::Var.init expressions, looking for any [`Expr::Await`] sub-node
/// not enclosed in a nested function / arrow / class context". The previous
/// shape-direct interpretation missed `console.log(await fetch())` and similar
/// nested-await sources, causing T3 to emit sync `fn main` containing `.await`
/// = Rust E0728 compile error.
///
/// Equivalent to the `has_top_level_await` field of the tuple returned by
/// [`Transformer::collect_top_level_executions`], but computed without IR
/// conversion or a [`Transformer`] instance.
#[doc(hidden)] // I-224 internal predicate, exposed for external integration tests.
pub fn has_top_level_await(module: &Module) -> bool {
    module.body.iter().any(|item| match item {
        ModuleItem::Stmt(Stmt::Expr(expr_stmt)) => expr_contains_await_recursive(&expr_stmt.expr),
        ModuleItem::Stmt(Stmt::Decl(Decl::Var(var))) => {
            if var.declare {
                return false;
            }
            var.decls
                .iter()
                .any(|d| d.init.as_deref().is_some_and(expr_contains_await_recursive))
        }
        // ExportDecl-wrapped Decl::Var (I-228-c fix): `export const c = await fetch();`
        // is `ModuleDecl::ExportDecl(ExportDecl { decl: Decl::Var(...) })`. Treated
        // semantically the same as bare `Stmt::Decl(Decl::Var)` for top-level await
        // detection purposes (= same recursive walk).
        ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => match &export.decl {
            Decl::Var(var) if !var.declare => var
                .decls
                .iter()
                .any(|d| d.init.as_deref().is_some_and(expr_contains_await_recursive)),
            // ExportDecl-wrapped Class: outer-context await detection (super_class /
            // decorators / member computed keys) — same as bare Decl::Class case.
            Decl::Class(class_decl) => class_contains_await_recursive(&class_decl.class),
            // Other ExportDecl-wrapped declarations (Fn / Interface / TypeAlias /
            // Enum / Module / Using or declare-marked Var): no init expr / outer-
            // context expression reachable from top-level execution.
            Decl::Fn(_)
            | Decl::TsInterface(_)
            | Decl::TsTypeAlias(_)
            | Decl::TsEnum(_)
            | Decl::TsModule(_)
            | Decl::Using(_)
            | Decl::Var(_) => false,
        },
        // All other ModuleItem shapes cannot host a top-level await: control-flow /
        // Empty / Debugger never reach this predicate (= rejected upstream by
        // transform_module_item Tier 2 honest reject), and remaining ModuleDecl
        // variants (Import / ExportNamed / ExportAll / ExportDefaultDecl /
        // ExportDefaultExpr / TsImportEquals / TsExportAssignment /
        // TsNamespaceExport) do not introduce top-level execution expressions.
        ModuleItem::Stmt(
            Stmt::Block(_)
            | Stmt::Empty(_)
            | Stmt::Debugger(_)
            | Stmt::With(_)
            | Stmt::Return(_)
            | Stmt::Labeled(_)
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::If(_)
            | Stmt::Switch(_)
            | Stmt::Throw(_)
            | Stmt::Try(_)
            | Stmt::While(_)
            | Stmt::DoWhile(_)
            | Stmt::For(_)
            | Stmt::ForIn(_)
            | Stmt::ForOf(_)
            | Stmt::Decl(
                Decl::Fn(_)
                | Decl::TsInterface(_)
                | Decl::TsTypeAlias(_)
                | Decl::TsEnum(_)
                | Decl::TsModule(_)
                | Decl::Using(_),
            ),
        ) => false,
        // === Bare Decl::Class with outer-context await (I-228 main scope
        // extension): super_class / decorators / member computed keys are
        // evaluated at class-definition time. Reuses the same walker as
        // is_executable_mode + Expr::Class to keep the two paths in lock-step. ===
        ModuleItem::Stmt(Stmt::Decl(Decl::Class(class_decl))) => {
            class_contains_await_recursive(&class_decl.class)
        }
        ModuleItem::ModuleDecl(
            ModuleDecl::Import(_)
            | ModuleDecl::ExportNamed(_)
            | ModuleDecl::ExportAll(_)
            | ModuleDecl::ExportDefaultDecl(_)
            | ModuleDecl::ExportDefaultExpr(_)
            | ModuleDecl::TsImportEquals(_)
            | ModuleDecl::TsExportAssignment(_)
            | ModuleDecl::TsNamespaceExport(_),
        ) => false,
    })
}

/// Returns the [`DispatchArm`] selected by the `(is_executable_mode, user_main_kind,
/// has_top_level_await)` 3-tuple per PRD Design section #2.
///
/// **Rule 9 (a) 1-to-1 mapping**: each in-scope matrix cell maps to exactly one arm;
/// see the per-variant docstring on [`DispatchArm`] for the cell ↔ arm table.
///
/// # Panics
///
/// Panics on `(false, _, true)` after the Collision arm has absorbed `(false,
/// Collision, true)`. This combination is structurally impossible per the
/// AST-level mutual exclusion proven by `tests/swc_parser_top_level_await_test.rs`
/// (library mode contains no execution stmt, so it cannot host a top-level await
/// expression). The `unreachable!` macro is a defensive lock-in.
#[doc(hidden)] // I-224 internal classifier, exposed for external integration tests.
pub fn classify_dispatch_arm(
    is_executable_mode: bool,
    user_main_kind: UserMainKind,
    has_top_level_await: bool,
) -> DispatchArm {
    match (is_executable_mode, user_main_kind, has_top_level_await) {
        // INV-5 priority arm — must come first to absorb (_, Collision, _).
        (_, UserMainKind::Collision, _) => DispatchArm::Collision,

        // Library mode (declarations only or A2 Lit init only).
        (false, UserMainKind::None, false) => DispatchArm::LibraryNone,
        (false, UserMainKind::FnSync, false) => DispatchArm::LibraryFnSyncDirect,
        (false, UserMainKind::FnAsync, false) => DispatchArm::LibraryFnAsyncDirect,
        (false, UserMainKind::NonFn, false) => DispatchArm::LibraryNonFn,

        // Executable mode + no top-await (sync dispatch unless FnAsync triggers).
        (true, UserMainKind::None, false) => DispatchArm::ExecNoneSync,
        (true, UserMainKind::FnSync, false) => DispatchArm::ExecFnSyncRename,
        (true, UserMainKind::FnAsync, false) => DispatchArm::ExecFnAsyncRename,
        (true, UserMainKind::NonFn, false) => DispatchArm::ExecNonFnSync,

        // Executable mode + top-await (always async dispatch via Trigger 2).
        (true, UserMainKind::None, true) => DispatchArm::ExecNoneAsync,
        (true, UserMainKind::FnSync, true) => DispatchArm::ExecFnSyncRenameAsync,
        (true, UserMainKind::FnAsync, true) => DispatchArm::ExecFnAsyncRenameAsync,
        (true, UserMainKind::NonFn, true) => DispatchArm::ExecNonFnAsync,

        // Structurally unreachable — library mode + top-await is AST-impossible
        // (the Collision arm absorbed (false, Collision, true), so we know the
        // UserMainKind here is None / FnSync / FnAsync / NonFn).
        (
            false,
            UserMainKind::None | UserMainKind::FnSync | UserMainKind::FnAsync | UserMainKind::NonFn,
            true,
        ) => unreachable!(
            "Library mode + has_top_level_await=true is structurally impossible \
             (library mode has no execution stmt = no Stmt::Expr/Decl::Var with await \
             partition; empirically locked-in by tests/swc_parser_top_level_await_test.rs)"
        ),
    }
}

impl<'a> Transformer<'a> {
    /// Scans the module body and produces the inputs to the I-224 fn main dispatch
    /// tree (PRD Design section #2 + #3):
    ///
    /// - `main_stmts`: top-level execution stmts (Stmt::Expr / Decl::Var with
    ///   side-effect or await init) captured into the synthesized fn main body, in
    ///   source order. Decl::Var with `Lit` init is **not** captured here — it is
    ///   left for the legacy library-mode path to emit as a top-level `Item::Const`.
    /// - `user_main_kind`: B-axis classification per [`detect_user_main`].
    /// - `has_top_level_await`: `true` iff any Stmt::Expr Await or Decl::Var with
    ///   await init was captured.
    ///
    /// **Rule 11 (d-1) compliance**: every `Stmt` / `Decl` variant inside the body is
    /// enumerated. Variants other than the four execution shapes (`Stmt::Expr`,
    /// `Stmt::Decl(Decl::Var)`, declarations, control-flow / Empty / Debugger,
    /// ModuleDecl) are no-ops in this scan — they are handled by sibling helpers
    /// ([`Transformer::transform_module_item`] for declarations / control-flow /
    /// Debugger reject, [`detect_user_main`] for B-axis classification).
    ///
    /// # Errors
    ///
    /// Propagates [`Transformer::convert_expr`] / [`Transformer::convert_var_decl`]
    /// errors (e.g., unsupported subexpression). When `transform_module` /
    /// `transform_module_collecting` integrate this helper (T4), the caller will
    /// decide between early-abort and accumulating-collect semantics.
    #[allow(dead_code)] // Wired into `Transformer::transform_module` /
                        // `transform_module_collecting` by T4-1; until then, this
                        // method is exercised only by `tests` (unit) and is not
                        // reachable from the lib build's root.
    pub(crate) fn collect_top_level_executions(
        &mut self,
        module: &Module,
    ) -> Result<(Vec<MainStmt>, UserMainKind, bool)> {
        let exec_mode = is_executable_mode(module);
        let user_main_kind = detect_user_main(module);
        let has_top_level_await_flag = has_top_level_await(module);
        let mut main_stmts = Vec::new();

        for item in &module.body {
            match item {
                // ============== Stmt::Expr (top-level execution) ==============
                ModuleItem::Stmt(Stmt::Expr(expr_stmt)) => {
                    if !exec_mode {
                        // Library mode never hosts Stmt::Expr per `is_executable_mode`
                        // definition; reaching this arm in library mode would be a
                        // self-contradiction (a Stmt::Expr would have flipped the mode
                        // to true). The defensive `unreachable!` documents the
                        // invariant.
                        unreachable!(
                            "Stmt::Expr present in library mode contradicts is_executable_mode: \
                             fix is_executable_mode or this scan loop"
                        );
                    }
                    // Bare `await x;` → MainStmt::ExprAwait (T3 emits `<inner>.await;`).
                    // Nested await `f(await x);` → MainStmt::Expr (= whole IR; the inner
                    // Expr::Await sub-node is preserved by convert_expr and rendered as
                    // `.await` by T3 emission within the async fn main context).
                    if let Expr::Await(await_expr) = &*expr_stmt.expr {
                        let inner = self.convert_expr(&await_expr.arg)?;
                        main_stmts.push(MainStmt::ExprAwait(inner));
                    } else {
                        let converted = self.convert_expr(&expr_stmt.expr)?;
                        main_stmts.push(MainStmt::Expr(converted));
                    }
                }

                // ============== Decl::Var (capture or top-level const path) ==============
                ModuleItem::Stmt(Stmt::Decl(Decl::Var(var))) => {
                    self.capture_var_decl_into_main_stmts(var, exec_mode, &mut main_stmts)?;
                }

                // ============== ExportDecl-wrapped Decl::Var (I-228-c fix) ==============
                // `export const c = compute();` semantically belongs to Axis A3
                // (= side-effect Decl::Var) when init is non-Lit. is_executable_mode
                // now triggers on this shape, so we capture it identically to bare
                // Stmt::Decl(Decl::Var) — the `pub` modifier of the export is dropped
                // (= cosmetic loss for executable mode、PRD Spec stage 逆戻り Axis E
                // orthogonality merge revision 2026-05-07)。Other ExportDecl-wrapped
                // Decl variants (Fn / Class / Interface / TypeAlias / Enum / Module /
                // Using) are emitted by transform_module_item via existing path with
                // `pub` modifier preserved — no main_stmts capture.
                ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => match &export.decl {
                    Decl::Var(var) => {
                        self.capture_var_decl_into_main_stmts(var, exec_mode, &mut main_stmts)?;
                    }
                    Decl::Fn(_)
                    | Decl::Class(_)
                    | Decl::TsInterface(_)
                    | Decl::TsTypeAlias(_)
                    | Decl::TsEnum(_)
                    | Decl::TsModule(_)
                    | Decl::Using(_) => {}
                },

                // ============== Other ModuleDecl: no capture ==============
                ModuleItem::ModuleDecl(
                    ModuleDecl::Import(_)
                    | ModuleDecl::ExportNamed(_)
                    | ModuleDecl::ExportAll(_)
                    | ModuleDecl::ExportDefaultDecl(_)
                    | ModuleDecl::ExportDefaultExpr(_)
                    | ModuleDecl::TsImportEquals(_)
                    | ModuleDecl::TsExportAssignment(_)
                    | ModuleDecl::TsNamespaceExport(_),
                ) => {}

                // ============== Stmt: declarations / control-flow / Empty / Debugger ==============
                // Declarations (Fn / Class / Interface / TypeAlias / Enum / Module / Using):
                // emitted as Rust items by transform_module_item; not main_stmts capture.
                ModuleItem::Stmt(Stmt::Decl(
                    Decl::Fn(_)
                    | Decl::Class(_)
                    | Decl::TsInterface(_)
                    | Decl::TsTypeAlias(_)
                    | Decl::TsEnum(_)
                    | Decl::TsModule(_)
                    | Decl::Using(_),
                )) => {}

                // A5a Empty: silent skip per the per-item dispatch table; no capture.
                ModuleItem::Stmt(Stmt::Empty(_)) => {}

                // A5b Debugger + A4 control-flow: rejected by transform_module_item
                // (Tier 2 honest reject); reaching this scan in executable mode is
                // possible only in collecting mode where transform_module_collecting
                // accumulates errors and continues. Skip the capture; the rejection
                // is recorded by the caller's accumulator.
                ModuleItem::Stmt(Stmt::Debugger(_)) => {}
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
                ) => {}
            }
        }

        Ok((main_stmts, user_main_kind, has_top_level_await_flag))
    }

    /// Helper for [`Self::collect_top_level_executions`]: walks every declarator
    /// of a (non-ambient) `VarDecl` whose `classify_decl_var_path` resolves to
    /// `FnMainBodyCapture`, and pushes one [`MainStmt::Let`] / [`MainStmt::LetAwait`]
    /// per declarator into `main_stmts`.
    ///
    /// **I-228-d multi-declarator iteration (Spec stage 逆戻り 2026-05-07)**:
    /// `const a = 1, b = compute();` previously routed only the first declarator
    /// to capture; now all declarators are captured (= each becomes its own
    /// `let` binding inside fn main body, source-order preserved). When the
    /// VarDecl's [`classify_init_kind`] returns [`InitKind::AwaitInit`] (= ANY
    /// declarator contains await per the recursive walker) or
    /// [`InitKind::SideEffect`], routing to FnMainBodyCapture forces all
    /// declarators (including pure-Lit ones in mixed-init VarDecls) into local
    /// `let` bindings inside fn main; the `pub const` form is dropped to
    /// preserve TS module-load source-order semantics in Rust execution.
    ///
    /// **Per-declarator MainStmt variant selection**: `MainStmt::LetAwait` is
    /// emitted only for the **bare-Await** init shape (`const c = await fetch();`)
    /// where the awaitee can be cleanly extracted; nested-await init shapes
    /// (e.g., `const c = process(await fetch());`) emit `MainStmt::Let` with the
    /// whole IR expression — convert_expr preserves `Expr::Await` sub-nodes,
    /// and T3 emission renders them as `.await` within the async fn main
    /// context.
    ///
    /// **DeclVarPath::ToplevelConst / LibraryMode**: no main_stmts capture; the
    /// existing `convert_var_decl_module_level` path emits the top-level
    /// `Item::Const` (Lit init) or library-mode binding (declare-marked / no-init).
    fn capture_var_decl_into_main_stmts(
        &mut self,
        var: &VarDecl,
        is_executable_mode_flag: bool,
        main_stmts: &mut Vec<MainStmt>,
    ) -> Result<()> {
        match classify_decl_var_path(var, is_executable_mode_flag) {
            DeclVarPath::FnMainBodyCapture => {
                for decl in &var.decls {
                    let ast::Pat::Ident(binding) = &decl.name else {
                        // Destructuring captures (`const { a } = compute()` /
                        // `const [a] = arr`) are not enumerated by the I-224 matrix.
                        // Skip the per-declarator capture; the existing path will
                        // surface this as an unsupported pattern (T4 may revisit).
                        continue;
                    };
                    let name = binding.id.sym.to_string();
                    let Some(init_expr) = decl.init.as_deref() else {
                        // No init in this declarator (= mid-list `let a = 1, b;`
                        // type or other rare TS shape). Skip capture.
                        continue;
                    };
                    if let Expr::Await(await_expr) = init_expr {
                        // Bare-Await init: extract awaitee, emit LetAwait (T3 applies
                        // `.await` based on the variant tag).
                        let inner = self.convert_expr(&await_expr.arg)?;
                        main_stmts.push(MainStmt::LetAwait { name, init: inner });
                    } else {
                        // All other init shapes (Lit / SideEffect / nested-Await):
                        // emit Let with the whole IR. For nested-await, convert_expr
                        // preserves the inner `Expr::Await` sub-node which T3 emission
                        // renders as `.await` within the async fn main context.
                        let init = self.convert_expr(init_expr)?;
                        main_stmts.push(MainStmt::Let { name, init });
                    }
                }
            }
            // ToplevelConst / LibraryMode: no main_stmts capture.
            DeclVarPath::ToplevelConst | DeclVarPath::LibraryMode => {}
        }
        Ok(())
    }

    /// Synthesizes the `fn main` IR [`Item`]s required by the I-224 dispatch tree
    /// (PRD Design section #2 + #5).
    ///
    /// The 3-tuple `(is_executable_mode, user_main_kind, has_top_level_await)` is
    /// classified by [`classify_dispatch_arm`]; each arm produces:
    ///
    /// - **Library arms** (`LibraryNone` / `LibraryFnSyncDirect` /
    ///   `LibraryFnAsyncDirect` / `LibraryNonFn`): no synthesized fn main —
    ///   `transform_decl` directly emits the user's `main` (or no entry point at all
    ///   for `LibraryNone` / `LibraryNonFn`). Returns an empty `Vec<Item>`.
    /// - **Executable sync arms** (`ExecNoneSync` / `ExecFnSyncRename` /
    ///   `ExecNonFnSync`): synthesizes a single `fn main() { /* main_stmts */ }`
    ///   without the `#[tokio::main]` attribute (`is_async = false`).
    /// - **Executable async arms** (`ExecNoneAsync` / `ExecFnSyncRenameAsync` /
    ///   `ExecFnAsyncRename` / `ExecFnAsyncRenameAsync` / `ExecNonFnAsync`):
    ///   synthesizes `#[tokio::main] async fn main() { /* main_stmts */ }`
    ///   (`is_async = true`, `attributes = ["tokio::main"]`).
    /// - **Collision arm**: structurally unreachable — upstream
    ///   [`super::super::scan_for_ts_namespace_collisions`] in
    ///   `Transformer::transform_module` rejects the `__ts_main` user identifier
    ///   before any dispatch-tree call site is reached. The `unreachable!` here
    ///   is a defensive lock-in that surfaces as a loud panic if a future caller
    ///   bypasses the upstream namespace lint.
    ///
    /// **`is_executable_mode` derivation**: PRD Design section #2 defines exec mode
    /// as the union of (a) any `Stmt::Expr` / `Decl::Var` with side-effect / await
    /// init at the top level (= drives `main_stmts.is_empty()`) and (b) class
    /// declarations whose `super_class` / decorators / member computed keys contain
    /// outer-context await (= I-228 main scope extension, drives
    /// `has_top_level_await=true` even when `main_stmts` is empty). The disjunction
    /// `!main_stmts.is_empty() || has_top_level_await` reproduces the predicate
    /// without re-walking the module — it is exactly equivalent to
    /// [`is_executable_mode`] modulo the order in which the predicates were applied
    /// (the caller, when `main_stmts` came from
    /// [`Self::collect_top_level_executions`], has already ensured this equivalence).
    ///
    /// **Body emission**: the `Vec<MainStmt>` is converted in-place via
    /// [`main_stmts_to_ir_stmts`] — the per-variant mapping preserves source order
    /// (= INV-1) and applies `Expr::Await` wrapping for `ExprAwait` / `LetAwait`
    /// (which store the **awaitee**, not the `Expr::Await(_)` wrapper, per the
    /// [`MainStmt`] documentation).
    ///
    /// # Panics
    ///
    /// - `(_, Collision, _)`: defensive — unreachable in production after
    ///   `Transformer::transform_module` upstream rejection (see above).
    /// - `(false, _, true)` (forwarded from [`classify_dispatch_arm`]): library
    ///   mode + has_top_level_await=true is structurally impossible by the AST
    ///   mutual-exclusion locked in by
    ///   `tests/swc_parser_top_level_await_test.rs`. Reaching this combination
    ///   indicates an inconsistent caller that built a `(main_stmts,
    ///   has_top_level_await)` pair violating the
    ///   `is_executable_mode = !main_stmts.is_empty() || has_top_level_await`
    ///   invariant.
    #[allow(dead_code)]
    // Wired into `Transformer::transform_module` /
    // `transform_module_collecting` by T4-1; until then this method is exercised
    // only by `tests` (unit) and is not reachable from the lib build's root.
    pub(crate) fn synthesize_fn_main(
        &mut self,
        main_stmts: Vec<MainStmt>,
        user_main_kind: UserMainKind,
        has_top_level_await: bool,
    ) -> Vec<Item> {
        let is_executable_mode = !main_stmts.is_empty() || has_top_level_await;
        let arm = classify_dispatch_arm(is_executable_mode, user_main_kind, has_top_level_await);

        match arm {
            // ============== Library mode arms — no synthesized fn main ==============
            // The user's existing `main` (if any) is the binary entry directly via
            // `transform_decl` (or the binary has no entry point at all in
            // LibraryNone / LibraryNonFn — that is consistent with TS module-load
            // semantics: declarations only, no execution).
            DispatchArm::LibraryNone
            | DispatchArm::LibraryFnSyncDirect
            | DispatchArm::LibraryFnAsyncDirect
            | DispatchArm::LibraryNonFn => Vec::new(),

            // ============== Executable mode + sync dispatch ==============
            DispatchArm::ExecNoneSync
            | DispatchArm::ExecFnSyncRename
            | DispatchArm::ExecNonFnSync => {
                let body = main_stmts_to_ir_stmts(main_stmts);
                vec![build_synthesized_fn_main(body, /* is_async = */ false)]
            }

            // ============== Executable mode + async dispatch ==============
            // Trigger 1 (FnAsync) / Trigger 2 (top-await) / both — all collapse to
            // `#[tokio::main] async fn main()` per INV-3 (a) integration.
            DispatchArm::ExecNoneAsync
            | DispatchArm::ExecFnSyncRenameAsync
            | DispatchArm::ExecFnAsyncRename
            | DispatchArm::ExecFnAsyncRenameAsync
            | DispatchArm::ExecNonFnAsync => {
                let body = main_stmts_to_ir_stmts(main_stmts);
                vec![build_synthesized_fn_main(body, /* is_async = */ true)]
            }

            // ============== Collision — defensive unreachable ==============
            DispatchArm::Collision => unreachable!(
                "synthesize_fn_main reached with UserMainKind::Collision; the \
                 `__ts_main` identifier collision must be rejected upstream by \
                 `Transformer::transform_module`'s call to \
                 `scan_for_ts_namespace_collisions` before any dispatch-tree \
                 call site (INV-5 highest precedence)."
            ),
        }
    }
}

/// Maps a `Vec<MainStmt>` to the IR statement list inserted into the synthesized
/// `fn main` body, preserving source order (= INV-1).
///
/// The per-variant mapping is total (every [`MainStmt`] variant is enumerated):
///
/// | [`MainStmt`] variant | [`IrStmt`] result | Rust emission |
/// |---|---|---|
/// | `Expr(e)` | `IrStmt::Expr(e)` | `<e>;` |
/// | `ExprAwait(inner)` | `IrStmt::Expr(IrExpr::Await(Box::new(inner)))` | `<inner>.await;` |
/// | `Let { name, init }` | `IrStmt::Let { mutable: false, name, ty: None, init: Some(init) }` | `let <name> = <init>;` |
/// | `LetAwait { name, init }` | `IrStmt::Let { mutable: false, name, ty: None, init: Some(IrExpr::Await(Box::new(init))) }` | `let <name> = <init>.await;` |
///
/// **Await wrapping**: `ExprAwait` and `LetAwait` carry the **awaitee** (the operand
/// of TS `await`), not an outer `Expr::Await(_)`. This helper restores the
/// `Expr::Await(_)` wrapper at the point of IR emission, keeping the IR symmetric
/// with the Rust postfix `.await` syntax. (See the [`MainStmt`] doc comment's
/// "Await-variant invariant".)
#[allow(dead_code)]
// Consumed by `Transformer::synthesize_fn_main` (above); the `synthesize_fn_main`
// method itself is dead until T4-1 wires it into `transform_module`. The
// `#[allow(dead_code)]` is removable when T4-1 lands the production call site.
fn main_stmts_to_ir_stmts(main_stmts: Vec<MainStmt>) -> Vec<IrStmt> {
    main_stmts
        .into_iter()
        .map(|main_stmt| match main_stmt {
            MainStmt::Expr(e) => IrStmt::Expr(e),
            MainStmt::ExprAwait(inner) => IrStmt::Expr(IrExpr::Await(Box::new(inner))),
            MainStmt::Let { name, init } => IrStmt::Let {
                mutable: false,
                name,
                ty: None,
                init: Some(init),
            },
            MainStmt::LetAwait { name, init } => IrStmt::Let {
                mutable: false,
                name,
                ty: None,
                init: Some(IrExpr::Await(Box::new(init))),
            },
        })
        .collect()
}

/// Builds the IR [`Item::Fn`] for the synthesized binary entry-point `fn main`.
///
/// Sync dispatch (`is_async = false`) emits `fn main() { body }` with no attributes.
/// Async dispatch (`is_async = true`) emits `#[tokio::main] async fn main() { body }`
/// — this is the unified emission for all five executable-async dispatch arms
/// (`ExecNoneAsync` / `ExecFnSyncRenameAsync` / `ExecFnAsyncRename` /
/// `ExecFnAsyncRenameAsync` / `ExecNonFnAsync`) per INV-3 (a) async dispatch
/// trigger integration.
///
/// Visibility is `Private` (= no `pub`): the binary entry point convention does
/// not require / permit `pub fn main`, and the synthesized fn never participates
/// in cross-module API surfaces (= INV-5 / Axis E orthogonality cross-reference).
#[allow(dead_code)]
// Consumed by `Transformer::synthesize_fn_main` (above); same dead-until-T4-1
// rationale as `main_stmts_to_ir_stmts`.
fn build_synthesized_fn_main(body: Vec<IrStmt>, is_async: bool) -> Item {
    let attributes = if is_async {
        vec!["tokio::main".to_string()]
    } else {
        Vec::new()
    };
    Item::Fn {
        vis: Visibility::Private,
        attributes,
        is_async,
        name: "main".to_string(),
        type_params: Vec::new(),
        params: Vec::new(),
        return_type: None,
        body,
    }
}

#[cfg(test)]
mod tests;
