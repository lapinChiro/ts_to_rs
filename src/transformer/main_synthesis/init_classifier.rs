//! Decl::Var initializer classification — [`InitKind`] / [`DeclVarPath`] enums
//! and the [`classify_init_kind`] / [`expr_init_kind`] / [`has_side_effect_init`]
//! / [`classify_decl_var_path`] helpers.
//!
//! Extracted from `mod.rs` to keep the file under the 1000-line file-line check
//! threshold while preserving Rule 11 (d-1) self-applied compliance — every
//! `Expr` variant is enumerated explicitly in [`expr_init_kind`].
//!
//! ## Partition map
//!
//! [`InitKind`] partitions Decl::Var initializers into 4 disjoint classes:
//!
//! - **`Lit`** — Rust `const`-compatible literals (`Num` / `Bool` / `Str` /
//!   `Null` / `BigInt`, plus `-Lit::Num` / `-Lit::BigInt`).
//! - **`NonTrigger`** — function / class definitions (`Arrow` / `Fn` / `Class`),
//!   aggregate literals (`Object` / `Array`), and type-only wrappers
//!   (`TsAs` / `TsConstAssertion` / etc., recursively classified via
//!   [`expr_init_kind`]). No observable module-load side effect.
//! - **`AwaitInit`** — `Expr::Await` (or any expression containing `await`
//!   reachable without crossing a function boundary).
//! - **`SideEffect`** — anything else (`Call` / `Ident` / `New` / `Member` /
//!   `Bin` / etc.). Has potential observable side effect at module-load time.
//!
//! [`DeclVarPath`] determines the emission path:
//!
//! - **`LibraryMode`** — existing `convert_var_decl_module_level` path
//!   (`Item::Const` for Lit, `Item::Fn` for Arrow / Fn, silent drop for
//!   Object / Array / Class / type-wrapped — pre-existing I-016 owner).
//! - **`ToplevelConst`** — same `Item::Const` emission as `LibraryMode` but
//!   in executable-mode contexts.
//! - **`FnMainBodyCapture`** — captured into the synthesized fn main body as
//!   [`super::MainStmt::Let`] / [`super::MainStmt::LetAwait`].

use swc_ecma_ast::{Expr, Lit, UnaryOp, VarDecl};

use super::await_walker::expr_contains_await_recursive;

/// Decl::Var initializer expression classification.
///
/// Drives [`classify_decl_var_path`] (Library / Toplevel-const / Fn-main-body-capture)
/// and contributes to [`super::is_executable_mode`] (the A3 partition trigger).
///
/// **Multi-declarator note**: when a `VarDecl` contains multiple declarators
/// (`const a = 1, b = 2;`), this classifier walks every declarator and applies
/// the ANY-rule precedence (`AwaitInit > SideEffect > NonTrigger > Lit`) per
/// the I-228-d Spec stage 逆戻り 2026-05-07 fix (extended with `NonTrigger`
/// by T3-4 e2e regression fix).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InitKind {
    /// `Expr::Lit(Lit::Num/Bool/Str/Null/BigInt)` or `-Lit::Num/-Lit::BigInt` —
    /// Rust `const` compatible literals.
    Lit,
    /// Initializer expressions that introduce **no observable runtime side
    /// effect at TypeScript module-load time**. Includes:
    ///
    /// - **Function/closure/class definitions**: `Expr::Arrow`, `Expr::Fn`,
    ///   `Expr::Class`. The bodies execute only when invoked / instantiated,
    ///   not at declaration time.
    /// - **Type-only wrappers**: `Expr::TsAs`, `Expr::TsConstAssertion`,
    ///   `Expr::TsTypeAssertion`, `Expr::TsNonNull`, `Expr::TsInstantiation`,
    ///   `Expr::TsSatisfies`, `Expr::Paren` — these are AST decorators with
    ///   no runtime semantics; the inner expression's classification is
    ///   inherited (recursive walk via [`expr_init_kind`]).
    /// - **Aggregate literals**: `Expr::Object`, `Expr::Array` — currently
    ///   silently dropped by `convert_var_decl_module_level`'s `_ =>
    ///   continue` fall-through (= I-016 owner; pre-existing gap), so they
    ///   contribute no runtime semantics regardless of whether classified
    ///   as `NonTrigger` or `SideEffect`. Treating them as `NonTrigger`
    ///   prevents the rename gate from firing falsely on library-mode
    ///   modules whose only non-Lit Decl::Var items are pure data
    ///   declarations like `const Phase = { Stringify: 1, ... } as const;`.
    ///
    /// These shapes are NOT executable-mode triggers and are emitted by
    /// `convert_var_decl_module_level` as standalone `Item::Fn` items
    /// (Arrow / Fn) or silently dropped (Object / Array / Class — pre-existing).
    /// They are never captured into the synthesized fn main body.
    ///
    /// This variant resolves a Spec gap discovered during T3-4 e2e
    /// verification: classifying every non-Lit init as `SideEffect` falsely
    /// triggered `is_executable_mode=true` (= rename gate fired) for
    /// library-mode modules whose only non-Lit Decl::Var items were
    /// declarations / type-only wrappers / function definitions. The runtime
    /// semantics are `Lit`-equivalent (no module-load side effect → no exec
    /// trigger), but the Rust emission path differs (`Item::Fn` instead of
    /// `Item::Const`, or silent drop), so a separate variant is structurally
    /// honest.
    NonTrigger,
    /// `Expr::Await(_)` — top-level await initializer (Axis C1).
    AwaitInit,
    /// Any other expression (Call / Ident / New / Member / Bin / etc.) —
    /// Axis A3 trigger. Has potential observable side effects at module-load
    /// time (function calls, getter access, constructor invocation,
    /// arithmetic with side-effecting operands, etc.).
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
    /// Executable mode + side-effect / await init: captured into [`super::MainStmt::Let`] /
    /// [`super::MainStmt::LetAwait`] and emitted inside the synthesized fn main body.
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
    if var.decls.is_empty() {
        unreachable!(
            "VarDecl must have at least 1 declarator (TS parser invariant); callers \
             must guard against empty-decls Var via has_side_effect_init / \
             classify_decl_var_path before calling this predicate"
        );
    };
    let mut has_init_in_any_declarator = false;
    let mut has_await = false;
    let mut has_side_effect = false;
    let mut has_non_trigger = false;
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
            InitKind::NonTrigger => has_non_trigger = true,
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
    // upstream defensive guards (`has_side_effect_init` / `classify_decl_var_path`)
    // before reaching this predicate. Reaching here with no initialized declarator
    // indicates a caller-side bug; loud panic surfaces it immediately rather than
    // silently misclassifying.
    if !has_init_in_any_declarator {
        unreachable!(
            "TS Decl::Var requires init in at least one declarator (callers must guard \
             against all-no-init Var via has_side_effect_init / classify_decl_var_path \
             defensive guards before calling classify_init_kind)"
        );
    }
    // Precedence: AwaitInit > SideEffect > NonTrigger > Lit (= I-228-d ANY-rule
    // per PRD spec revision 2026-05-07, extended with NonTrigger partition by
    // T3-4). Multi-declarator with mixed init shapes (e.g.,
    // `const a = 1, b = compute();`) classifies based on the union of all
    // declarators' shapes: ANY AwaitInit → AwaitInit, ANY SideEffect →
    // SideEffect, ANY NonTrigger → NonTrigger, all Lit → Lit. ToplevelConst
    // routing requires unanimous Lit; FnMainBodyCapture captures the entire
    // VarDecl into fn main body when ANY declarator is SideEffect or AwaitInit.
    // NonTrigger stays in `LibraryMode` regardless of executable_mode (= the
    // NonTrigger declarator's emission path is independent of the synthesized
    // fn main body) — see `classify_decl_var_path` for the routing.
    if has_await {
        InitKind::AwaitInit
    } else if has_side_effect {
        InitKind::SideEffect
    } else if has_non_trigger {
        InitKind::NonTrigger
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

        // === NonTrigger: function / class definitions, aggregate literals ===
        // Function definitions: bodies execute only on invocation.
        Expr::Arrow(_) | Expr::Fn(_) | Expr::Class(_) => InitKind::NonTrigger,
        // Aggregate literals: currently silently dropped by
        // `convert_var_decl_module_level` (I-016 owner). No runtime
        // semantics are emitted regardless of whether classified as
        // NonTrigger or SideEffect; classifying as NonTrigger prevents the
        // rename gate from firing falsely on modules whose only non-Lit
        // Decl::Var items are pure data declarations like `const Phase = {
        // ... } as const;`.
        Expr::Object(_) | Expr::Array(_) => InitKind::NonTrigger,

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

/// Classifies a Decl::Var into a [`DeclVarPath`] given the module-level
/// `is_executable_mode` value.
///
/// See [`DeclVarPath`] for the path semantics; the table here records the full
/// `(is_executable_mode, init_kind)` Cartesian product:
///
/// | is_executable_mode | InitKind::Lit | InitKind::NonTrigger | InitKind::SideEffect | InitKind::AwaitInit |
/// |---|---|---|---|---|
/// | `false` (library) | LibraryMode | LibraryMode | LibraryMode | LibraryMode (AST-impossible per `swc_parser_top_level_await_test`) |
/// | `true` (executable) | ToplevelConst | LibraryMode (= `Item::Fn` / silent drop via `convert_var_decl_module_level`) | FnMainBodyCapture | FnMainBodyCapture |
///
/// **`NonTrigger` rationale**: Arrow / FnExpr initializers emit `Item::Fn` at
/// the top level via `convert_var_decl_module_level::convert_arrow_var_decl`
/// (or the FnExpr → synthetic FnDecl path added by T3-2). Object / Array /
/// Class literal initializers and type-only wrappers (`as const`, etc.) are
/// either silently dropped by the existing path's `_ => continue` fallback
/// (pre-existing I-016 owner) or routed unchanged. They are NOT captured
/// into the synthesized fn main body — their declaration semantics are
/// independent of executable mode, so they always route to `LibraryMode`
/// regardless of `is_executable_mode`.
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
        // NonTrigger: emit via existing convert_var_decl_module_level path
        // (= `Item::Fn` for Arrow/Fn, silent drop for Object/Array/Class /
        // type-wrapped — pre-existing I-016 owner). No fn main capture.
        (true, InitKind::NonTrigger) => DeclVarPath::LibraryMode,
        (true, InitKind::SideEffect) | (true, InitKind::AwaitInit) => {
            DeclVarPath::FnMainBodyCapture
        }
    }
}
