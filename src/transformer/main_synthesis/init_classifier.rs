//! Decl::Var initializer classification — [`InitKind`] / [`DeclVarPath`] enums
//! and the [`classify_init_kind`] / [`expr_init_kind`] / [`has_side_effect_init`]
//! / [`classify_per_decl_path`] / [`all_decls_captured`] helpers.
//!
//! Extracted from `mod.rs` to keep the file under the 1000-line file-line check
//! threshold while preserving Rule 11 (d-1) self-applied compliance — every
//! `Expr` variant is enumerated explicitly in [`expr_init_kind`].
//!
//! ## Partition map
//!
//! [`InitKind`] partitions Decl::Var initializers into 5 disjoint classes:
//!
//! - **`Lit`** — Rust `const`-compatible literals (`Num` / `Bool` / `Str` /
//!   `Null` / `BigInt`, plus `-Lit::Num` / `-Lit::BigInt`).
//! - **`NonTriggerDef`** — function / class definitions (`Arrow` / `Fn` /
//!   `Class`). Bodies execute only on invocation / instantiation; module-load
//!   has no observable side effect. Emitted by `convert_var_decl_module_level`
//!   as `Item::Fn` (Arrow / FnExpr) or top-level class shapes — never
//!   captured into the synthesized fn main body.
//! - **`NonTriggerData`** — aggregate literals (`Object` / `Array`) and
//!   type-only wrappers around them. Module-load has no observable side
//!   effect (no calls, no I/O), but the emission path differs from
//!   `NonTriggerDef`: in **executable mode** the data declaration is
//!   captured into the synthesized fn main body as a `let` binding (=
//!   preserves the TS module-level binding visible at runtime, structurally
//!   symmetric with the side-effect Var Decl path); in library mode it
//!   stays in `LibraryMode` (= existing `convert_var_decl_module_level`
//!   path which currently silently drops Object / Array — pre-existing
//!   I-016 owner). The split from `NonTriggerDef` exists because
//!   data literals in executable scripts (e.g., `const v: T = { ... };`
//!   followed by `console.log(v.x);`) MUST be captured to preserve runtime
//!   semantics; treating them as `NonTriggerDef` would silently drop the
//!   binding and produce E0425 "cannot find value" downstream.
//! - **`AwaitInit`** — `Expr::Await` (or any expression containing `await`
//!   reachable without crossing a function boundary).
//! - **`SideEffect`** — anything else (`Call` / `Ident` / `New` / `Member` /
//!   `Bin` / etc.). Has potential observable side effect at module-load time.
//!
//! [`DeclVarPath`] determines the emission path:
//!
//! - **`LibraryMode`** — existing `convert_var_decl_module_level` path
//!   (`Item::Const` for Lit, `Item::Fn` for Arrow / Fn, top-level class for
//!   Class definition; silent drop for Object / Array literals in library
//!   mode — pre-existing I-016 owner).
//! - **`ToplevelConst`** — same `Item::Const` emission as `LibraryMode` but
//!   in executable-mode contexts.
//! - **`FnMainBodyCapture`** — captured into the synthesized fn main body as
//!   [`super::MainStmt::Let`] / [`super::MainStmt::LetAwait`]. Reached by
//!   side-effect / await inits, AND by `NonTriggerData` (Object / Array
//!   aggregate literals) in executable mode (= the I-224 T5-1 Spec-stage
//!   逆戻り extension that resolves the cell-12 / cell-24 silent-drop Tier 1
//!   semantic loss).

use swc_ecma_ast::{Expr, Lit, UnaryOp, VarDecl};

use super::await_walker::expr_contains_await_recursive;

/// Decl::Var initializer expression classification.
///
/// Drives [`classify_per_decl_path`] (Library / Toplevel-const / Fn-main-body-capture)
/// and contributes to [`super::is_executable_mode`] (the A3 partition trigger).
///
/// **Multi-declarator note**: when a `VarDecl` contains multiple declarators
/// (`const a = 1, b = 2;`), this classifier walks every declarator and applies
/// the ANY-rule precedence
/// (`AwaitInit > SideEffect > NonTriggerData > NonTriggerDef > Lit`) per the
/// I-228-d Spec stage 逆戻り 2026-05-07 fix (extended with `NonTrigger`
/// by T3-4 e2e regression fix; further split into `NonTriggerDef` /
/// `NonTriggerData` by T5-1 Spec stage 逆戻り 2026-05-08 to resolve the
/// cell-12 / cell-24 Object literal silent-drop Tier 1 semantic loss).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InitKind {
    /// `Expr::Lit(Lit::Num/Bool/Str/Null/BigInt)` or `-Lit::Num/-Lit::BigInt` —
    /// Rust `const` compatible literals.
    Lit,
    /// **Function / closure / class definitions** — `Expr::Arrow`, `Expr::Fn`,
    /// `Expr::Class` (and their type-only wrappers via [`expr_init_kind`]
    /// recursive walk). Bodies execute only when invoked / instantiated, not
    /// at declaration time → no observable runtime side effect at TypeScript
    /// module-load time → does not trigger executable mode.
    ///
    /// Emitted by `convert_var_decl_module_level` as standalone `Item::Fn`
    /// (Arrow / FnExpr) or top-level class shapes — never captured into the
    /// synthesized fn main body. Routes to `LibraryMode` regardless of
    /// `is_executable_mode`.
    ///
    /// Split out from the original `NonTrigger` variant by I-224 T5-1 Spec
    /// stage 逆戻り 2026-05-08 to keep the function/class-definition emission
    /// path (= top-level `Item::Fn`) distinct from the data-literal capture
    /// path (= `NonTriggerData` → `FnMainBodyCapture` in executable mode).
    NonTriggerDef,
    /// **Aggregate data literals** — `Expr::Object`, `Expr::Array` (and their
    /// type-only wrappers via [`expr_init_kind`] recursive walk). No
    /// observable runtime side effect at module-load time (no calls, no I/O
    /// in literal construction itself), so does not trigger executable mode.
    ///
    /// **Emission path differs by mode**:
    /// - **Library mode** (`is_executable_mode=false`): routes to
    ///   `LibraryMode` (= existing `convert_var_decl_module_level` path
    ///   which currently silently drops Object / Array — pre-existing I-016
    ///   owner; resolved by a follow-up PRD scope, not I-224 T5-1).
    /// - **Executable mode** (`is_executable_mode=true`): routes to
    ///   `FnMainBodyCapture` so the data declaration becomes a `let` binding
    ///   inside the synthesized fn main body. This preserves the TS
    ///   module-level binding visible to subsequent top-level statements
    ///   (= cell-12 / cell-24 Tier 1 silent-drop fix: previously `const v: T
    ///   = { ... };` was dropped, leaving `console.log(v.x)` referencing an
    ///   undefined `v` and producing E0425 downstream).
    ///
    /// **Why split from `NonTriggerDef`**: the function/class-definition
    /// path (`Item::Fn` etc.) MUST stay at top level (Rust functions are not
    /// expressible as fn-main-body `let` bindings), while data literals MUST
    /// be captured into fn main when other top-level execution exists. The
    /// two paths share the "no module-load side effect" property but
    /// require structurally different Rust emission.
    NonTriggerData,
    /// `Expr::Await(_)` — top-level await initializer (Axis C1).
    AwaitInit,
    /// Any other expression (Call / Ident / New / Member / Bin / etc.) —
    /// Axis A3 trigger. Has potential observable side effects at module-load
    /// time (function calls, getter access, constructor invocation,
    /// arithmetic with side-effecting operands, etc.).
    SideEffect,
}

/// Decl::Var dispatch path: where the declaration's emission goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeclVarPath {
    /// Library mode: existing `convert_var_decl_module_level` path emits a top-level
    /// `Item::Const` / `Item::Let`.
    LibraryMode,
    /// Executable mode + Lit init: same top-level emission as `LibraryMode` (the const
    /// is hoisted to top-level). INV-1 source-order preservation holds because Lit
    /// init has no observable runtime side effect.
    ToplevelConst,
    /// Executable mode + side-effect / await / data-literal init: captured into
    /// [`super::MainStmt::Let`] / [`super::MainStmt::LetAwait`] and emitted
    /// inside the synthesized fn main body. Reached by `InitKind::SideEffect`,
    /// `InitKind::AwaitInit`, **and** `InitKind::NonTriggerData` (= I-224 T5-1
    /// Spec stage 逆戻り 2026-05-08 cell-12 / cell-24 Tier 1 silent-drop fix).
    FnMainBodyCapture,
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
/// the prefix's design intent (the load-bearing semantic — `classify_per_decl_path`
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
/// `classify_per_decl_path` defensive guards. The `unreachable!` macro records
/// the precondition explicitly so any future bypass-path caller surfaces as a
/// loud panic instead of a silent misclassification.
pub(crate) fn classify_init_kind(var: &VarDecl) -> InitKind {
    if var.decls.is_empty() {
        unreachable!(
            "VarDecl must have at least 1 declarator (TS parser invariant); callers \
             must guard against empty-decls Var via has_side_effect_init / \
             classify_per_decl_path before calling this predicate"
        );
    };
    let mut has_init_in_any_declarator = false;
    let mut has_await = false;
    let mut has_side_effect = false;
    let mut has_non_trigger_data = false;
    let mut has_non_trigger_def = false;
    for decl in &var.decls {
        let Some(init) = decl.init.as_deref() else {
            // Mid-list no-init declarator (e.g., `let a = 1, b;`): TS allows for
            // `let` only. No contribution to side-effect detection — the lacking
            // init is a separate concern handled by the existing convert_var_decl_*
            // path. We continue scanning the remaining declarators rather than
            // panicking, so a partial-no-init multi-declarator can still be
            // classified by its initialized declarators.
            continue;
        };
        has_init_in_any_declarator = true;
        // Recursive Await detection (= I-228 main fix): nested `Expr::Await` inside
        // an init expression is a top-level await trigger, even if the init's outer
        // shape isn't Expr::Await directly. Examples:
        //   const c = process(await fetch());  // outer Call, inner Await arg
        //   const c = await fetch();           // outer Await directly
        //   const c = -await x;                // outer Unary, inner Await operand
        //
        // **Function-boundary skip**: `expr_contains_await_recursive` correctly
        // treats `Expr::Fn` / `Expr::Arrow` / `Expr::Class` as boundaries (= it
        // does NOT walk into the bodies of nested closures). This means
        // `const f = async () => { await x; };` does NOT trigger
        // `has_top_level_await` here, which is the structurally correct behavior
        // (the `await` inside the closure body executes in the closure's
        // own async context, not at module-load time).
        if expr_contains_await_recursive(init) {
            has_await = true;
            continue;
        }
        match expr_init_kind(init) {
            InitKind::Lit => {}
            InitKind::NonTriggerDef => has_non_trigger_def = true,
            InitKind::NonTriggerData => has_non_trigger_data = true,
            InitKind::SideEffect => has_side_effect = true,
            // AwaitInit is unreachable in practice — the
            // `expr_contains_await_recursive` short-circuit above sets
            // `has_await = true` for any await-containing init. Enumerated
            // defensively for compile-time exhaustiveness.
            InitKind::AwaitInit => unreachable!(
                "expr_init_kind never returns AwaitInit; await-containing inits are \
                 short-circuited by the expr_contains_await_recursive check above"
            ),
        }
    }
    // Precondition violation guard: VarDecl with ALL declarators having no init
    // (= declare-marked Var or empty-init multi-declarator) must be filtered by
    // upstream defensive guards (`has_side_effect_init` / `classify_per_decl_path`)
    // before reaching this predicate. Reaching here with no initialized declarator
    // indicates a caller-side bug; loud panic surfaces it immediately rather than
    // silently misclassifying.
    if !has_init_in_any_declarator {
        unreachable!(
            "TS Decl::Var requires init in at least one declarator (callers must guard \
             against all-no-init Var via has_side_effect_init / classify_per_decl_path \
             defensive guards before calling classify_init_kind)"
        );
    }
    // Precedence: AwaitInit > SideEffect > NonTriggerData > NonTriggerDef >
    // Lit (= I-228-d ANY-rule per PRD spec revision 2026-05-07, extended with
    // NonTrigger partition by T3-4, further split into NonTriggerDef /
    // NonTriggerData by I-224 T5-1 Spec stage 逆戻り 2026-05-08 to resolve
    // the cell-12 / cell-24 silent-drop Tier 1 semantic loss).
    //
    // Multi-declarator with mixed init shapes (e.g.,
    // `const a = 1, b = compute();`) classifies based on the union of all
    // declarators' shapes: ANY AwaitInit → AwaitInit, ANY SideEffect →
    // SideEffect, ANY NonTriggerData → NonTriggerData, ANY NonTriggerDef →
    // NonTriggerDef, all Lit → Lit. The NonTriggerData over NonTriggerDef
    // precedence is structurally honest: a mixed VarDecl (`const a = () => 1,
    // b = { x: 1 };`) carries a runtime-visible binding (the data) that must
    // be captured in executable mode; the function-definition declarator's
    // `Item::Fn` emission path can be retained alongside via the existing
    // `convert_var_decl_module_level` routing (multi-declarator emission is
    // already split per-declarator in that path; see `capture_var_decl_into_main_stmts`
    // for the symmetric per-declarator capture in the FnMainBodyCapture branch).
    //
    // ToplevelConst routing requires unanimous Lit; FnMainBodyCapture captures
    // the entire VarDecl into fn main body when ANY declarator is SideEffect /
    // AwaitInit / NonTriggerData (in executable mode). NonTriggerDef stays in
    // `LibraryMode` regardless of executable_mode (= the function/class-def
    // declarator's emission path is independent of the synthesized fn main
    // body) — see `classify_per_decl_path` for the full routing table.
    if has_await {
        InitKind::AwaitInit
    } else if has_side_effect {
        InitKind::SideEffect
    } else if has_non_trigger_data {
        InitKind::NonTriggerData
    } else if has_non_trigger_def {
        InitKind::NonTriggerDef
    } else {
        InitKind::Lit
    }
}

/// Per-expression classification helper for the executable-mode trigger.
///
/// Recursively walks type-only wrappers (`TsAs` / `TsConstAssertion` /
/// `TsTypeAssertion` / `TsNonNull` / `TsInstantiation` / `TsSatisfies` /
/// `Paren`) so that the inner expression's classification is inherited by the
/// outer wrapped form. Stops at function/class boundaries (the body's
/// classification doesn't propagate up — function definitions are non-trigger
/// regardless of inner contents).
///
/// **Returns**: never `InitKind::AwaitInit` — `await` containment is detected
/// separately by [`expr_contains_await_recursive`] in the calling
/// `classify_init_kind`.
///
/// **Rule 11 (d-1) self-applied compliance**: every `Expr` variant is
/// enumerated explicitly. New SWC variants will produce a compile error and
/// force this match to be updated.
fn expr_init_kind(expr: &Expr) -> InitKind {
    match expr {
        // === Rust-const-compatible Lit variants ===
        Expr::Lit(Lit::Num(_) | Lit::Bool(_) | Lit::Str(_) | Lit::Null(_) | Lit::BigInt(_)) => {
            InitKind::Lit
        }
        Expr::Unary(unary)
            if matches!(unary.op, UnaryOp::Minus)
                && matches!(*unary.arg, Expr::Lit(Lit::Num(_) | Lit::BigInt(_))) =>
        {
            InitKind::Lit
        }

        // === NonTriggerDef: function / class definitions ===
        // Bodies execute only on invocation / instantiation. Emitted by
        // `convert_var_decl_module_level` as standalone `Item::Fn` /
        // top-level class shapes; never captured into fn main body.
        Expr::Arrow(_) | Expr::Fn(_) | Expr::Class(_) => InitKind::NonTriggerDef,
        // === NonTriggerData: aggregate literals ===
        // Object / Array literal construction has no observable module-load
        // side effect (no calls, no I/O). In **library mode** these flow
        // through `convert_var_decl_module_level`'s `_ => continue`
        // fall-through and are silently dropped (= pre-existing I-016 owner;
        // resolved by a follow-up PRD scope, not I-224 T5-1). In
        // **executable mode** (`is_executable_mode=true`) they route to
        // `FnMainBodyCapture` per `classify_per_decl_path` so the data
        // declaration becomes a `let` binding inside the synthesized fn
        // main body — this resolves the cell-12 / cell-24 silent-drop Tier 1
        // semantic loss (= the structural extension introduced by I-224 T5-1
        // Spec stage 逆戻り 2026-05-08).
        //
        // Despite the executable-mode capture, `has_side_effect_init` still
        // returns false for `NonTriggerData` (Object / Array do NOT trigger
        // executable mode by themselves), so a pure data module like
        // `const Phase = { Stringify: 1, ... } as const;` stays in library
        // mode. The capture only fires when other top-level execution
        // (Stmt::Expr or SideEffect Var) has already triggered exec mode.
        Expr::Object(_) | Expr::Array(_) => InitKind::NonTriggerData,

        // === Type-only wrappers: recurse on inner ===
        Expr::Paren(p) => expr_init_kind(&p.expr),
        Expr::TsAs(t) => expr_init_kind(&t.expr),
        Expr::TsConstAssertion(t) => expr_init_kind(&t.expr),
        Expr::TsTypeAssertion(t) => expr_init_kind(&t.expr),
        Expr::TsNonNull(t) => expr_init_kind(&t.expr),
        Expr::TsInstantiation(t) => expr_init_kind(&t.expr),
        Expr::TsSatisfies(t) => expr_init_kind(&t.expr),

        // === Side-effect-bearing variants (= ALL other Expr shapes) ===
        // Each variant is enumerated explicitly per Rule 11 (d-1); the grouped
        // `|` pattern keeps the body single-statement while preserving
        // exhaustive enumeration. Rationale per variant:
        //
        // - `Lit::Regex` / `Lit::JSXText`: Regex::new() runtime call (can
        //   panic); JSXText is a runtime fragment string.
        // - `This` / `Ident` / `MetaProp` / `PrivateName`: identifier
        //   reference — getter access on global / module scope can have
        //   side effects.
        // - `Invalid` / `JSXEmpty` / `JSXNamespacedName` / `JSXMember`: SWC
        //   parse-stage variants; conservatively side-effect.
        // - `Unary` (non-Minus-Lit fallback): general unary like `!x` or
        //   `typeof x` — depends on operand.
        // - `Update`, `Bin`, `Assign`, `Member`, `SuperProp`, `Cond`,
        //   `Call`, `New`, `Seq`, `Tpl`, `TaggedTpl`, `Yield`: side-effect
        //   potential (function calls, getter access, mutation, etc.).
        // - `Await`: defensive — short-circuited upstream by
        //   `expr_contains_await_recursive`; reaching this arm is
        //   unreachable in practice but enumerated for compile-time
        //   exhaustiveness.
        // - `JSXElement` / `JSXFragment`: JSX-specific runtime construction.
        // - `OptChain`: `?.` chain — depends on chained ops.
        Expr::Lit(Lit::Regex(_) | Lit::JSXText(_))
        | Expr::This(_)
        | Expr::Ident(_)
        | Expr::MetaProp(_)
        | Expr::PrivateName(_)
        | Expr::Invalid(_)
        | Expr::JSXEmpty(_)
        | Expr::JSXNamespacedName(_)
        | Expr::JSXMember(_)
        | Expr::Unary(_)
        | Expr::Update(_)
        | Expr::Bin(_)
        | Expr::Assign(_)
        | Expr::Member(_)
        | Expr::SuperProp(_)
        | Expr::Cond(_)
        | Expr::Call(_)
        | Expr::New(_)
        | Expr::Seq(_)
        | Expr::Tpl(_)
        | Expr::TaggedTpl(_)
        | Expr::Yield(_)
        | Expr::Await(_)
        | Expr::JSXElement(_)
        | Expr::JSXFragment(_)
        | Expr::OptChain(_) => InitKind::SideEffect,
    }
}

/// Returns `true` if the `VarDecl` is the A3 trigger of [`super::is_executable_mode`]
/// — i.e., a `SideEffect` or `AwaitInit` initializer.
///
/// `Lit` and `NonTrigger` initializers do NOT trigger executable mode: literal
/// constants are compile-time values, and `NonTrigger` (function/class
/// definitions, aggregate literals, type-only wrappers) introduce no
/// observable runtime side effect at module-load time. Both are emitted by
/// the existing `convert_var_decl_module_level` path (`Item::Const` for Lit,
/// `Item::Fn` for Arrow/Fn, silent drop for Object/Array/Class/type-wrapped),
/// independent of the synthesized fn main.
///
/// `AwaitInit` returns `true` so that Axis C1 cells reach the
/// `is_executable_mode=true` arm of the dispatch tree.
///
/// **Ambient / no-init guard**: returns `false` for `declare const x: T;`
/// (ambient, type-only) and `let x;` (no init expression). Both shapes lie
/// outside the I-224 matrix's A3 partition (which requires a side-effect /
/// await init expression) and never contribute to executable mode.
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

// **Removed by `/check_job deep deep` 2026-05-08 structural fix**:
// `classify_per_decl_path` (= the legacy VarDecl-level aggregating
// classifier with ANY-rule precedence) was deleted entirely. Production
// callers were migrated to [`classify_per_decl_path`] by the prior
// `/check_job deep` iteration; remaining test usages were migrated to
// either [`classify_per_decl_path`] or to direct [`classify_init_kind`]
// partition assertions. The dual-classifier maintenance burden is removed
// and the codebase now has a single canonical routing helper per
// `ideal-implementation-primacy.md` (= no test-only retention of code
// production no longer requires).

/// Classifies a single [`VarDeclarator`] into a [`DeclVarPath`] given the
/// module-level `is_executable_mode` flag and the parent VarDecl's `declare`
/// flag. Decides the routing path **independently per declarator**, enabling
/// mixed Def+Data multi-declarator VarDecls (e.g., `const f = () => 1, x = {
/// a: 1 };`) to route Arrow → `LibraryMode` (= top-level `Item::Fn`) AND
/// Object → `FnMainBodyCapture` (= `MainStmt::Let` inside fn main body)
/// simultaneously.
///
/// See [`DeclVarPath`] for the path semantics; the per-declarator decision
/// table is the same `(is_executable_mode, InitKind)` Cartesian product the
/// removed `classify_per_decl_path` documented:
///
/// | is_executable_mode | Lit           | NonTriggerDef | NonTriggerData    | SideEffect        | AwaitInit                    |
/// |--------------------|---------------|---------------|-------------------|-------------------|------------------------------|
/// | `false` (library)  | LibraryMode   | LibraryMode   | LibraryMode       | LibraryMode       | LibraryMode (AST-impossible) |
/// | `true` (executable)| ToplevelConst | LibraryMode   | FnMainBodyCapture | FnMainBodyCapture | FnMainBodyCapture            |
///
/// AST-impossible note: `(false, AwaitInit)` cannot occur because top-level
/// `await` shifts the module into executable mode by the
/// `has_side_effect_init` predicate that drives `is_executable_mode`. This is
/// structurally locked in by `tests/swc_parser_top_level_await_test.rs`.
///
/// **`NonTriggerDef` rationale**: Arrow / FnExpr initializers emit `Item::Fn`
/// at the top level via `convert_var_decl_module_level::convert_arrow_var_decl`
/// (or the FnExpr → synthetic FnDecl path added by T3-2). Class expression
/// initializers similarly emit top-level class shapes. These cannot be
/// expressed as fn-main-body `let` bindings, so they always route to
/// `LibraryMode` regardless of `is_executable_mode`.
///
/// **`NonTriggerData` rationale (I-224 T5-1 Spec stage 逆戻り 2026-05-08)**:
/// Object / Array literal initializers and type-only wrappers around them
/// carry runtime-visible bindings that subsequent top-level statements may
/// reference (e.g., `const v: T = { ... }; console.log(v.x);`). In library
/// mode they remain in `LibraryMode` (= currently silent-dropped by the
/// existing `convert_var_decl_module_level` `_ => continue` fall-through;
/// pre-existing I-016 owner, resolved by a follow-up PRD scope). In
/// executable mode they route to `FnMainBodyCapture` so the binding becomes
/// a `let` inside the synthesized fn main body, preserving the runtime
/// visibility that the user's downstream `Stmt::Expr` references depend on.
/// This resolves the cell-12 / cell-24 silent-drop Tier 1 semantic loss.
///
/// **Lesson source**: `/check_job deep` review iteration v2 2026-05-08 found
/// that an earlier VarDecl-level routing (`classify_per_decl_path`) forced
/// mixed Def+Data into a single path (NonTriggerData precedence over
/// NonTriggerDef → whole VarDecl routes to `FnMainBodyCapture` → Arrow
/// becomes Rust closure literal `let f = || 1.0;` inside fn main, losing the
/// top-level `fn f() -> f64 { 1.0 }` emission path). The per-declarator
/// routing achieves the architecturally cleanest separation: each
/// declarator's emission path is decided by its own `InitKind`, regardless
/// of sibling declarators in the same VarDecl.
///
/// **Reachability note**: Hono codebase has 0 instances of mixed Def+Data
/// multi-declarator VarDecl (verified by `grep -rE` 2026-05-08), and the
/// I-224 matrix doesn't enumerate this pattern as a separate cell. The fix
/// is architecturally honest rather than reachability-driven; per
/// `ideal-implementation-primacy.md` the structural correctness wins over
/// reachability rationalization.
pub(crate) fn classify_per_decl_path(
    decl: &swc_ecma_ast::VarDeclarator,
    is_executable_mode: bool,
    parent_var_declare: bool,
) -> DeclVarPath {
    if parent_var_declare {
        return DeclVarPath::LibraryMode;
    }
    let Some(init) = decl.init.as_deref() else {
        return DeclVarPath::LibraryMode;
    };
    if !is_executable_mode {
        return DeclVarPath::LibraryMode;
    }
    // Per-declarator init kind: `expr_contains_await_recursive` short-circuit
    // mirrors the VarDecl-level `classify_init_kind` precedence (AwaitInit
    // wins over expr_init_kind output when await is present anywhere in the
    // init's recursive walk).
    let kind = if expr_contains_await_recursive(init) {
        InitKind::AwaitInit
    } else {
        expr_init_kind(init)
    };
    match kind {
        InitKind::Lit => DeclVarPath::ToplevelConst,
        InitKind::NonTriggerDef => DeclVarPath::LibraryMode,
        InitKind::NonTriggerData => DeclVarPath::FnMainBodyCapture,
        InitKind::SideEffect | InitKind::AwaitInit => DeclVarPath::FnMainBodyCapture,
    }
}

/// Returns `true` iff **every** declarator in `var` is bound for
/// [`DeclVarPath::FnMainBodyCapture`] (= captured into the synthesized fn
/// main body). Used by `try_capture_module_item_into_main_stmts` to decide
/// whether `transform_module_item` should also process the VarDecl (=
/// `false` return) for LibraryMode / ToplevelConst declarators that the
/// existing `convert_var_decl_module_level` per-init dispatch path emits.
///
/// **T5-1 deep review structural fix 2026-05-08**: per-declarator routing
/// signal that enables mixed Def+Data multi-declarator VarDecls to route
/// Arrow → top-level `Item::Fn` AND Object → fn-main-body `let` binding
/// simultaneously. See [`classify_per_decl_path`] for the lesson source.
pub(crate) fn all_decls_captured(var: &VarDecl, is_executable_mode_flag: bool) -> bool {
    if var.declare {
        // declare-marked: all decls are LibraryMode (no init runtime
        // semantics), `transform_module_item` handles ambient emission.
        return false;
    }
    var.decls.iter().all(|d| {
        matches!(
            classify_per_decl_path(d, is_executable_mode_flag, var.declare),
            DeclVarPath::FnMainBodyCapture
        )
    })
}
