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
use swc_ecma_ast::{
    self as ast, Decl, Expr, Lit, Module, ModuleDecl, ModuleItem, Stmt, UnaryOp, VarDecl,
};

use crate::ir::Expr as IrExpr;
use crate::transformer::Transformer;

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

/// Decl::Var initializer expression classification.
///
/// Drives [`classify_decl_var_path`] (Library / Toplevel-const / Fn-main-body-capture)
/// and contributes to [`is_executable_mode`] (the A3 partition trigger).
///
/// **Multi-declarator note**: when a `VarDecl` contains multiple declarators
/// (`const a = 1, b = 2;`), this classifier inspects only the **first** declarator
/// per the PRD Design section #3 spec. Multi-declarator with mixed init shapes is
/// not enumerated in the I-224 problem space; if encountered, it falls under the
/// behavior of the first declarator (out-of-scope for this PRD).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InitKind {
    /// `Expr::Lit(_)` or `-Lit::Num/-Lit::BigInt` — Rust `const` compatible.
    Lit,
    /// `Expr::Await(_)` — top-level await initializer (Axis C1).
    AwaitInit,
    /// Any other expression (Call / Ident / New / etc.) — Axis A3 trigger.
    SideEffect,
}

/// Decl::Var dispatch path: where the declaration's emission goes.
#[allow(dead_code)]
// Consumed by T4-1 (`transform_module` per-item routing) — until
// then, only `Transformer::collect_top_level_executions`
// (also dead at T2) constructs / inspects these variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeclVarPath {
    /// Library mode: existing `convert_var_decl_module_level` path emits a top-level
    /// `Item::Const` / `Item::Let`.
    LibraryMode,
    /// Executable mode + Lit init: same top-level emission as `LibraryMode` (the const
    /// is hoisted to top-level). INV-1 source-order preservation holds because Lit
    /// init has no observable runtime side effect.
    ToplevelConst,
    /// Executable mode + side-effect / await init: captured into [`MainStmt::Let`] /
    /// [`MainStmt::LetAwait`] and emitted inside the synthesized fn main body.
    FnMainBodyCapture,
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

/// Classifies a `VarDecl`'s initializer expression into [`InitKind`].
///
/// See [`InitKind`] for the partition definition and the PRD Design section #3 for the
/// rationale behind the literal-only / await / side-effect three-way split.
///
/// **Lit variant filtering (PRD design-intent reconciliation, T2 review fix)**:
/// PRD Design section #3 line 970 prefix defines `InitKind::Lit` as "compile-time
/// constant expressible form (Rust `const` 適合)" but the bullet enumeration
/// includes `Lit::Regex`, which is **NOT** Rust-const-compatible
/// (`regex::Regex::new("...")` is a runtime call, not a `const fn`). Following
/// the prefix's design intent (the load-bearing semantic — `classify_decl_var_path`
/// routes `InitKind::Lit` to `DeclVarPath::ToplevelConst` = `Item::Const` emit,
/// which fails to compile for Regex), this implementation matches **only the
/// Rust-const-compatible Lit variants** (`Num` / `Bool` / `Str` / `Null` /
/// `BigInt`). Regex / JSXText fall through to `InitKind::SideEffect` so T3
/// emission routes them to `MainStmt::Let` (fn main body capture,
/// `let r = Regex::new("ab").unwrap();` — Rust-compilable). Lit::JSXText is
/// structurally unreachable in VarDecl init position (JSX text only appears
/// inside JSX element children); the filtered match excludes it defensively.
///
/// **Spec-stage cleanup TODO**: PRD Design section #3 line 970-971 bullet list
/// of `Lit::Num/Bool/Str/Null/BigInt/Regex` should be revised to remove `Regex`,
/// reconciling the prefix and the bullets. Tracked alongside `[I-228]` Axis C
/// Spec gap (= same pattern of "AST-shape literal vs design intent" PRD
/// inconsistency).
///
/// # Panics
///
/// Panics if `var.decls` is empty or the first declarator has no init expression.
/// Both shapes are caller-precondition violations: ambient `declare const x: T;`
/// and uninitialized `let x;` are filtered upstream by `has_side_effect_init` /
/// `classify_decl_var_path` defensive guards. The `unreachable!` macro records
/// the precondition explicitly so any future bypass-path caller surfaces as a
/// loud panic instead of a silent misclassification.
pub(crate) fn classify_init_kind(var: &VarDecl) -> InitKind {
    let Some(first_decl) = var.decls.first() else {
        unreachable!(
            "VarDecl must have at least 1 declarator (TS parser invariant); callers \
             must guard against empty-decls Var via has_side_effect_init / \
             classify_decl_var_path before calling this predicate"
        );
    };
    match first_decl.init.as_deref() {
        // Rust-const-compatible Lit variants only (per PRD design intent).
        Some(Expr::Lit(
            Lit::Num(_) | Lit::Bool(_) | Lit::Str(_) | Lit::Null(_) | Lit::BigInt(_),
        )) => InitKind::Lit,
        Some(Expr::Unary(unary))
            if matches!(unary.op, UnaryOp::Minus)
                && matches!(*unary.arg, Expr::Lit(Lit::Num(_) | Lit::BigInt(_))) =>
        {
            InitKind::Lit
        }
        Some(Expr::Await(_)) => InitKind::AwaitInit,
        // Lit::Regex / Lit::JSXText fall here: not Rust-const-compatible, route to
        // FnMainBodyCapture for runtime-evaluated emission (`let r = Regex::new(...)`).
        Some(_) => InitKind::SideEffect,
        None => unreachable!(
            "TS Decl::Var requires init (let/const without init = parse error in strict mode); \
             callers must guard against no-init Var via has_side_effect_init / \
             classify_decl_var_path defensive guards"
        ),
    }
}

/// Returns `true` if the (first declarator of the) `VarDecl` is the A3 trigger of
/// [`is_executable_mode`] — i.e., a non-`Lit` initializer.
///
/// `AwaitInit` returns `true` so that Axis C1 cells reach the `is_executable_mode=true`
/// arm of the dispatch tree.
///
/// **Ambient / no-init guard**: returns `false` for `declare const x: T;` (ambient,
/// type-only) and `let x;` (no init expression). Both shapes lie outside the I-224
/// matrix's A3 partition (which requires a non-Lit init expression) and never
/// contribute to executable mode.
pub(crate) fn has_side_effect_init(var: &VarDecl) -> bool {
    if var.declare {
        return false;
    }
    let Some(first) = var.decls.first() else {
        return false;
    };
    if first.init.is_none() {
        return false;
    }
    matches!(
        classify_init_kind(var),
        InitKind::SideEffect | InitKind::AwaitInit
    )
}

/// Classifies a Decl::Var into a [`DeclVarPath`] given the module-level
/// `is_executable_mode` value.
///
/// See [`DeclVarPath`] for the path semantics; the table here records the full
/// `(is_executable_mode, init_kind)` Cartesian product:
///
/// | is_executable_mode | InitKind::Lit | InitKind::SideEffect | InitKind::AwaitInit |
/// |---|---|---|---|
/// | `false` (library) | LibraryMode | LibraryMode | LibraryMode (AST-impossible per `swc_parser_top_level_await_test`) |
/// | `true` (executable) | ToplevelConst | FnMainBodyCapture | FnMainBodyCapture |
#[allow(dead_code)] // Consumed by T4-1 (`transform_module` per-item routing); also
                    // called by `Transformer::collect_top_level_executions` (also
                    // dead at T2). Removable when T4-1 lands the integration.
pub(crate) fn classify_decl_var_path(var: &VarDecl, is_executable_mode: bool) -> DeclVarPath {
    // Defensive: ambient / no-init Var has no init expression to classify and
    // emits via the existing library-mode path (no fn main capture). This
    // mirrors the guard in [`has_side_effect_init`] so callers can pass any
    // `&VarDecl` without precondition checks.
    if var.declare || var.decls.first().is_none_or(|d| d.init.is_none()) {
        return DeclVarPath::LibraryMode;
    }
    let init_kind = classify_init_kind(var);
    match (is_executable_mode, init_kind) {
        (false, _) => DeclVarPath::LibraryMode,
        (true, InitKind::Lit) => DeclVarPath::ToplevelConst,
        (true, InitKind::SideEffect) | (true, InitKind::AwaitInit) => {
            DeclVarPath::FnMainBodyCapture
        }
    }
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

            // === Declarations partition — type-system / namespace only, no execution ===
            Stmt::Decl(
                Decl::Fn(_)
                | Decl::Class(_)
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

        // === Module-level declarations (Axis E E1 partition) — orthogonal ===
        // Imports / exports / TS-namespace-export / etc. preserve their semantics
        // regardless of executable_mode (per the PRD Axis E orthogonality probe);
        // the inner ModuleDecl variant is I-203 scope per Rule 11 (d-6) Architectural
        // concern relevance.
        ModuleItem::ModuleDecl(_) => false,
    })
}

/// AST-level scan that returns `true` iff the module body contains a top-level
/// `await` expression — either as a bare `Stmt::Expr(Expr::Await)` or as the
/// initializer of a (non-ambient, non-empty-init) `Decl::Var`.
///
/// Equivalent to the `has_top_level_await` field of the tuple returned by
/// [`Transformer::collect_top_level_executions`], but computed without IR
/// conversion or a [`Transformer`] instance — used by external integration tests
/// (`tests/i224_helper_test.rs`) that classify the dispatch arm before any T3/T4
/// emission code lands.
///
/// Restricted to the same AST shapes the IR-converting helper recognizes, so the
/// two computations agree on every module supported by the I-224 problem space.
///
/// **Known scope limitation (Spec gap candidate, see TODO `[I-224-NESTED-AWAIT]`)**:
/// this predicate detects `await` only in the **direct** position
/// (`Stmt::Expr(Expr::Await)` or `Decl::Var.init = Expr::Await`). Nested
/// `await` inside a sub-expression (e.g., `console.log(await fetch())` whose
/// outer is `Stmt::Expr(Call)` not `Stmt::Expr(Await)`) is **not** detected.
/// The PRD's Axis C definition (Design section #2 line 762) is AST-shape-based
/// and matches this implementation literally; semantic correctness for nested
/// awaits requires a recursive walker (out of T2 scope, escalated as a Spec
/// gap candidate to Spec stage iteration).
#[doc(hidden)] // I-224 internal predicate, exposed for external integration tests.
pub fn has_top_level_await(module: &Module) -> bool {
    module.body.iter().any(|item| match item {
        ModuleItem::Stmt(Stmt::Expr(expr_stmt)) => matches!(*expr_stmt.expr, Expr::Await(_)),
        ModuleItem::Stmt(Stmt::Decl(Decl::Var(var))) => {
            if var.declare {
                return false;
            }
            var.decls.iter().any(|d| {
                d.init
                    .as_deref()
                    .is_some_and(|e| matches!(e, Expr::Await(_)))
            })
        }
        // All other ModuleItem shapes (declarations / control-flow / Empty / Debugger /
        // Module-level imports/exports) cannot host a top-level await per the AST
        // mutual exclusion locked in by `tests/swc_parser_top_level_await_test.rs`.
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
                | Decl::Class(_)
                | Decl::TsInterface(_)
                | Decl::TsTypeAlias(_)
                | Decl::TsEnum(_)
                | Decl::TsModule(_)
                | Decl::Using(_),
            ),
        ) => false,
        ModuleItem::ModuleDecl(_) => false,
    })
}

/// Classifies a single module-level identifier `name` introduced by a declaration of
/// shape `shape` into a [`UserMainKind`], or `None` if `name` is unrelated.
///
/// **Shape parameter**: encodes the structural kind of the declaration's value
/// (`Fn { is_async }` for callable shapes, `NonFn` for non-callable). This lets the
/// classifier produce `FnSync` vs `FnAsync` vs `NonFn` without inspecting the AST itself.
fn classify_main_identifier(name: &str, shape: DeclShape) -> Option<UserMainKind> {
    if name == "__ts_main" {
        // B4: collides with the synthesized rename target.
        Some(UserMainKind::Collision)
    } else if name == "main" {
        match shape {
            DeclShape::Fn { is_async: false } => Some(UserMainKind::FnSync),
            DeclShape::Fn { is_async: true } => Some(UserMainKind::FnAsync),
            DeclShape::NonFn => Some(UserMainKind::NonFn),
        }
    } else {
        // Other identifiers (including non-`__ts_main` `__ts_*` namespace violations,
        // which are rejected separately by `scan_for_ts_namespace_collisions`) are
        // not B-axis triggers.
        None
    }
}

/// Structural shape of a declaration's value, passed to [`classify_main_identifier`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclShape {
    /// Callable shape: `function`, arrow init, fn-expr init.
    Fn { is_async: bool },
    /// Non-callable shape: class / interface / type alias / enum / namespace /
    /// non-callable Var init.
    NonFn,
}

/// Returns `true` if `decl` is an ambient (`declare`-marked) declaration that
/// emits no Rust runtime construct.
///
/// The PRD's B-axis (Axis B) classifies user-defined `main` symbols by their
/// **runtime shape** — `function main()` with a body becomes B1 / B2 (callable),
/// `class main { ... }` with a runtime constructor becomes B3 (non-callable
/// preserved into Rust). Ambient counterparts (`declare function main(): void;`,
/// `declare class main {}`, `declare const main: T;`, `declare enum main`,
/// `declare namespace main`) are TS-namespace-only — they assert the existence
/// of `main` at runtime without providing a body. From the Rust transpiler's POV
/// they introduce no rename target (no body to rename) and no preserved Rust
/// item (nothing to emit), so they are **not B-axis triggers** and are
/// classified as B0 (No user main).
///
/// **Interface / type alias** are inherently type-only regardless of the
/// `declare` keyword (TS spec); they remain B3 NonFn (= name preserved in TS
/// namespace, no Rust-side collision because Rust type / value namespaces are
/// disjoint). **Using declarations** are runtime resource bindings, never
/// `declare`-marked in practice.
///
/// **Collision precedence note**: ambient `__ts_main` (`declare function
/// __ts_main(): void;`) is independently rejected by the namespace lint
/// (`scan_for_ts_namespace_collisions` in `transformer/mod.rs`) regardless of
/// this filter — the namespace reservation invariant operates at the
/// identifier level, not the AST-shape level. Treating ambient `__ts_main` as
/// B0 here is therefore consistent with namespace-lint semantics: the lint
/// fires upstream, and the dispatch tree never sees the source.
fn is_ambient_decl(decl: &Decl) -> bool {
    match decl {
        Decl::Fn(fn_decl) => fn_decl.declare,
        Decl::Var(var_decl) => var_decl.declare,
        Decl::Class(class_decl) => class_decl.declare,
        Decl::TsEnum(enum_decl) => enum_decl.declare,
        Decl::TsModule(module_decl) => module_decl.declare,
        // Always type-only — declare keyword is redundant.
        Decl::TsInterface(_) | Decl::TsTypeAlias(_) => false,
        // Runtime resource bindings.
        Decl::Using(_) => false,
    }
}

/// Classifies a [`Decl`] for B-axis main detection.
///
/// **Rule 11 (d-1) compliance**: every `Decl` variant is enumerated.
fn detect_user_main_from_decl(decl: &Decl) -> Option<UserMainKind> {
    if is_ambient_decl(decl) {
        // Ambient declarations introduce no Rust runtime construct (see
        // [`is_ambient_decl`]'s docstring); they are not B-axis triggers.
        return None;
    }
    match decl {
        Decl::Fn(fn_decl) => {
            let shape = DeclShape::Fn {
                is_async: fn_decl.function.is_async,
            };
            classify_main_identifier(fn_decl.ident.sym.as_str(), shape)
        }
        Decl::Var(var_decl) => {
            for declarator in &var_decl.decls {
                let ast::Pat::Ident(binding) = &declarator.name else {
                    // Destructuring (`const { main } = ...` / `const [main] = ...`) is not a
                    // documented B-axis vector; the matrix's B variants only cover plain
                    // BindingIdent. Skip.
                    continue;
                };
                let name = binding.id.sym.as_str();
                let shape = match declarator.init.as_deref() {
                    Some(Expr::Arrow(arrow)) => DeclShape::Fn {
                        is_async: arrow.is_async,
                    },
                    Some(Expr::Fn(fn_expr)) => DeclShape::Fn {
                        is_async: fn_expr.function.is_async,
                    },
                    // Any other init (Lit / Call / Ident / etc.) is non-callable from
                    // the B-axis perspective: `const main = 42;` is B3 (NonFn).
                    Some(_) | None => DeclShape::NonFn,
                };
                if let Some(kind) = classify_main_identifier(name, shape) {
                    return Some(kind);
                }
            }
            None
        }
        Decl::Class(class_decl) => {
            classify_main_identifier(class_decl.ident.sym.as_str(), DeclShape::NonFn)
        }
        Decl::TsInterface(interface) => {
            classify_main_identifier(interface.id.sym.as_str(), DeclShape::NonFn)
        }
        Decl::TsTypeAlias(alias) => {
            classify_main_identifier(alias.id.sym.as_str(), DeclShape::NonFn)
        }
        Decl::TsEnum(enum_decl) => {
            classify_main_identifier(enum_decl.id.sym.as_str(), DeclShape::NonFn)
        }
        Decl::TsModule(module_decl) => match &module_decl.id {
            ast::TsModuleName::Ident(ident) => {
                classify_main_identifier(ident.sym.as_str(), DeclShape::NonFn)
            }
            // Ambient string-named modules (`declare module "fs" { ... }`) do not
            // introduce a B-axis identifier in the user's namespace.
            ast::TsModuleName::Str(_) => None,
        },
        Decl::Using(using_decl) => {
            for declarator in &using_decl.decls {
                let ast::Pat::Ident(binding) = &declarator.name else {
                    continue;
                };
                if let Some(kind) =
                    classify_main_identifier(binding.id.sym.as_str(), DeclShape::NonFn)
                {
                    return Some(kind);
                }
            }
            None
        }
    }
}

/// Classifies an [`ast::DefaultDecl`] (the inner of `export default function main() {}`
/// etc.) for B-axis main detection.
fn detect_user_main_from_default_decl(decl: &ast::DefaultDecl) -> Option<UserMainKind> {
    match decl {
        ast::DefaultDecl::Fn(fn_expr) => fn_expr.ident.as_ref().and_then(|ident| {
            let shape = DeclShape::Fn {
                is_async: fn_expr.function.is_async,
            };
            classify_main_identifier(ident.sym.as_str(), shape)
        }),
        ast::DefaultDecl::Class(class_expr) => class_expr
            .ident
            .as_ref()
            .and_then(|ident| classify_main_identifier(ident.sym.as_str(), DeclShape::NonFn)),
        ast::DefaultDecl::TsInterfaceDecl(interface) => {
            classify_main_identifier(interface.id.sym.as_str(), DeclShape::NonFn)
        }
    }
}

/// Walks `module.body` and returns the [`UserMainKind`] for the user's `main` /
/// `__ts_main` symbol (or `None` if neither is present).
///
/// **Collision precedence (B4 priority)**: when the body contains both a
/// `__ts_main` collision marker AND a non-collision `main` declaration,
/// [`UserMainKind::Collision`] wins — matching the PRD dispatch tree's
/// `(_, Collision, _)` arm having priority over A/C-axis dispatch.
///
/// **Rule 11 (d-1) compliance**: every `ModuleItem` / `ModuleDecl` variant that can
/// introduce a module-level identifier is enumerated.
#[doc(hidden)] // I-224 internal predicate, exposed for external integration tests.
pub fn detect_user_main(module: &Module) -> UserMainKind {
    let mut detected = UserMainKind::None;
    for item in &module.body {
        let candidate: Option<UserMainKind> = match item {
            ModuleItem::Stmt(Stmt::Decl(decl)) => detect_user_main_from_decl(decl),
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => {
                detect_user_main_from_decl(&export.decl)
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(default)) => {
                detect_user_main_from_default_decl(&default.decl)
            }
            // The remaining ModuleDecl variants (Import / ExportNamed / ExportAll /
            // ExportDefaultExpr / TsImportEquals / TsExportAssignment /
            // TsNamespaceExport) carry references to already-declared names rather
            // than introducing fresh ones — no B-axis detection here.
            ModuleItem::ModuleDecl(
                ModuleDecl::Import(_)
                | ModuleDecl::ExportNamed(_)
                | ModuleDecl::ExportAll(_)
                | ModuleDecl::ExportDefaultExpr(_)
                | ModuleDecl::TsImportEquals(_)
                | ModuleDecl::TsExportAssignment(_)
                | ModuleDecl::TsNamespaceExport(_),
            ) => None,
            // Non-Decl Stmt variants do not introduce module-level identifiers.
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
                | Stmt::Expr(_),
            ) => None,
        };
        if let Some(kind) = candidate {
            // Collision wins immediately (= INV-5 priority); other kinds fill the
            // first non-None slot and are not overwritten by later non-collision
            // detections (first-decl precedence within the non-collision class).
            if kind == UserMainKind::Collision {
                return UserMainKind::Collision;
            }
            if detected == UserMainKind::None {
                detected = kind;
            }
        }
    }
    detected
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
            // Per-Stmt processing only; ModuleDecl items (imports / exports /
            // namespace exports) are orthogonal to fn main capture (Axis E).
            let ModuleItem::Stmt(stmt) = item else {
                continue;
            };

            match stmt {
                Stmt::Expr(expr_stmt) => {
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
                    if let Expr::Await(await_expr) = &*expr_stmt.expr {
                        let inner = self.convert_expr(&await_expr.arg)?;
                        main_stmts.push(MainStmt::ExprAwait(inner));
                    } else {
                        let converted = self.convert_expr(&expr_stmt.expr)?;
                        main_stmts.push(MainStmt::Expr(converted));
                    }
                }

                Stmt::Decl(Decl::Var(var)) => {
                    let path = classify_decl_var_path(var, exec_mode);
                    match path {
                        DeclVarPath::FnMainBodyCapture => {
                            // `classify_decl_var_path` returning FnMainBodyCapture
                            // guarantees a non-ambient Var with a non-empty first
                            // declarator that has an init expression — those are the
                            // exact preconditions of `classify_init_kind`. The
                            // `let-else` + `unreachable!` pair re-declares the
                            // invariant explicitly so any future driver that bypasses
                            // `classify_decl_var_path` fails loudly here instead of
                            // misclassifying a missing init.
                            let Some(first) = var.decls.first() else {
                                unreachable!(
                                    "FnMainBodyCapture path implies var.decls.first().is_some() \
                                     — classify_decl_var_path filtered empty-decls Var"
                                );
                            };
                            let init_kind = classify_init_kind(var);
                            let ast::Pat::Ident(binding) = &first.name else {
                                // Destructuring captures (`const { a } = compute()`) are not
                                // a documented Axis A3 sub-shape; the matrix only enumerates
                                // BindingIdent. Skip the capture and let the existing
                                // library-mode path handle it (T4 will decide whether to
                                // upgrade this).
                                continue;
                            };
                            let name = binding.id.sym.to_string();
                            let Some(init_expr) = first.init.as_deref() else {
                                unreachable!(
                                    "FnMainBodyCapture path implies first.init.is_some() — \
                                     classify_decl_var_path filtered no-init Var"
                                );
                            };
                            match init_kind {
                                InitKind::AwaitInit => {
                                    let Expr::Await(await_expr) = init_expr else {
                                        unreachable!(
                                            "classify_init_kind returned AwaitInit but init is \
                                             not Expr::Await — `classify_init_kind` and this \
                                             extraction must stay in sync"
                                        );
                                    };
                                    let inner = self.convert_expr(&await_expr.arg)?;
                                    main_stmts.push(MainStmt::LetAwait { name, init: inner });
                                }
                                InitKind::SideEffect => {
                                    let init = self.convert_expr(init_expr)?;
                                    main_stmts.push(MainStmt::Let { name, init });
                                }
                                InitKind::Lit => unreachable!(
                                    "FnMainBodyCapture path implies non-Lit init (see \
                                     classify_decl_var_path's table); a Lit init would route \
                                     to ToplevelConst"
                                ),
                            }
                        }
                        // ToplevelConst / LibraryMode: emitted by transform_module_item via
                        // the existing convert_var_decl_module_level path; no main_stmts push.
                        DeclVarPath::ToplevelConst | DeclVarPath::LibraryMode => {}
                    }
                }

                // Declarations (Fn / Class / Interface / TypeAlias / Enum / Module / Using):
                // emitted as Rust items by transform_module_item; not main_stmts capture.
                Stmt::Decl(
                    Decl::Fn(_)
                    | Decl::Class(_)
                    | Decl::TsInterface(_)
                    | Decl::TsTypeAlias(_)
                    | Decl::TsEnum(_)
                    | Decl::TsModule(_)
                    | Decl::Using(_),
                ) => {}

                // A5a Empty: silent skip per the per-item dispatch table; no capture.
                Stmt::Empty(_) => {}

                // A5b Debugger + A4 control-flow: rejected by transform_module_item
                // (Tier 2 honest reject); reaching this scan in executable mode is
                // possible only in collecting mode where transform_module_collecting
                // accumulates errors and continues. Skip the capture; the rejection
                // is recorded by the caller's accumulator.
                Stmt::Debugger(_) => {}
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
                | Stmt::With(_) => {}
            }
        }

        Ok((main_stmts, user_main_kind, has_top_level_await_flag))
    }
}

#[cfg(test)]
mod tests;
