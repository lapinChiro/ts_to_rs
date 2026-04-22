//! JS truthy/falsy predicate emission (I-144 T6-3 / I-171 T2).
//!
//! JavaScript の `if (x)` truthy semantics を Rust に 1:1 で再現するために、
//! 値の RustType に応じた predicate expression を組み立てる。
//!
//! # API 階層
//!
//! - [`truthy_predicate`] / [`falsy_predicate`]: 既存 API (I-144 T6-3)。
//!   Ident 引数 + primitive 限定。`control_flow.rs` の per-variant match
//!   guard (`build_union_variant_truthy_arms`) と `generate_truthiness_condition`
//!   fallback が使用。既存 signature を維持 (backwards compat)。
//! - [`truthy_predicate_for_expr`] / [`falsy_predicate_for_expr`]: I-171 T2
//!   新 API。任意 `Expr` + 全 RustType 対応 (Option<T>, Option<synthetic union>,
//!   always-truthy). side-effect-prone operand に対して [`TempBinder`] による
//!   `{ let __ts_tmp = <e>; <pred>(_tmp) }` block form を生成。
//! - [`is_always_truthy_type`]: `&&=` / `||=` const-fold、`!<always-truthy>`
//!   → `false` fold の判定で使用。
//! - [`try_constant_fold_bang`]: `!null` / `!<lit>` / `!<arrow>` 等の AST-level
//!   const fold。`convert_unary_expr` Bang arm で実装前に適用。
//!
//! | TS LHS type           | Truthy predicate                        | Source cell |
//! |-----------------------|-----------------------------------------|-------------|
//! | `F64`                 | `x != 0.0 && !x.is_nan()`               | cell-t4d    |
//! | `String`              | `!x.is_empty()`                          | cell-t4c    |
//! | `Bool`                | `x`                                     | —           |
//! | `Primitive(int)`      | `x != 0`                                | —           |
//! | `Option<T Copy>`      | `x.is_some_and(\|v\| <pred(*v)>)`         | A-5         |
//! | `Option<T !Copy>`     | `x.as_ref().is_some_and(\|v\| <pred(v)>)` | A-5s        |
//! | `Option<synthetic U>` | per-variant `matches!` chain              | A-6         |
//! | `Option<Named other>` | `x.is_some()`                            | A-7         |
//! | always-truthy         | const `true`                            | A-8         |

use swc_ecma_ast as ast;

use crate::ir::{
    BinOp, BuiltinVariant, Expr, Pattern, PatternCtor, PrimitiveIntKind, RustType, Stmt, UnOp,
    UserTypeRef,
};
use crate::pipeline::synthetic_registry::{SyntheticTypeKind, SyntheticTypeRegistry};

/// Allocates unique `__ts_tmp_*` names for side-effect-prone operands in
/// truthy/falsy predicate emission.
///
/// When the operand is non-trivially-pure (e.g., `Call`, `BinaryOp`, `Cond`),
/// emitting a predicate like `<e> != 0.0 && !<e>.is_nan()` would evaluate
/// `<e>` twice — diverging from JS semantics when `<e>` has side effects.
/// `TempBinder` vends fresh names so the emission can wrap the operand in
/// `{ let <tmp> = <e>; <pred>(<tmp>) }`.
pub(crate) struct TempBinder {
    counter: u32,
}

impl TempBinder {
    /// Creates a fresh binder starting at index 0.
    pub(crate) fn new() -> Self {
        Self { counter: 0 }
    }

    /// Returns a fresh `__ts_tmp_<prefix>_<n>` identifier and increments the counter.
    ///
    /// The `__ts_` prefix follows the internal-var convention established in
    /// I-154 (arm-local scope guarantees no collision with outer bindings).
    pub(crate) fn fresh(&mut self, prefix: &str) -> String {
        let name = format!("__ts_tmp_{prefix}_{}", self.counter);
        self.counter += 1;
        name
    }
}

/// Returns `true` for expressions whose evaluation is side-effect free and
/// idempotent, so duplicating them in a predicate (e.g., `x != 0.0 && !x.is_nan()`)
/// does not change JS semantics.
///
/// The set is intentionally conservative — anything that might observe
/// mutable state or have side effects returns `false`, triggering
/// `TempBinder`-based tmp binding.
pub(crate) fn is_pure_operand(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::NumberLit(_)
            | Expr::IntLit(_)
            | Expr::BoolLit(_)
            | Expr::StringLit(_)
            | Expr::Ident(_)
            | Expr::Unit
            | Expr::EnumVariant { .. }
            | Expr::PrimitiveAssocConst { .. }
            | Expr::StdConst(_)
            | Expr::BuiltinVariantValue(_)
    ) || matches!(
        expr,
        Expr::FieldAccess { object, .. } if is_pure_operand(object)
    )
}

/// Returns `true` if instance values of this type are unconditionally truthy
/// in JS semantics (so `&&=` becomes unconditional assign and `||=` becomes
/// no-op for variables of this type, and `!<e>` const-folds to `false`).
///
/// `Named` types require the synthetic registry to distinguish between
/// synthetic union enums (whose variants may carry primitive values that can
/// be falsy) and struct / non-synthetic-union enum types (whose instance
/// values are always truthy references).
pub(crate) fn is_always_truthy_type(ty: &RustType, synthetic: &SyntheticTypeRegistry) -> bool {
    match ty {
        RustType::Vec(_)
        | RustType::Fn { .. }
        | RustType::StdCollection { .. }
        | RustType::DynTrait(_)
        | RustType::Ref(_)
        | RustType::Tuple(_) => true,
        RustType::Named { name, .. } => !is_synthetic_union_enum(synthetic, name),
        _ => false,
    }
}

/// Returns `true` if the given Named type name resolves to a synthetic union
/// enum in the registry. Synthetic union enums are produced for TS unions
/// like `number | string` and their per-variant inner values can be falsy.
fn is_synthetic_union_enum(synthetic: &SyntheticTypeRegistry, name: &str) -> bool {
    matches!(
        synthetic.get(name),
        Some(def) if def.kind == SyntheticTypeKind::UnionEnum
    )
}

// --- Existing Ident API (I-144 T6-3) ---------------------------------------

/// JS truthy predicate expression for a named variable of the given type.
///
/// Returns `None` for types not supported in the primitive T6-3 scope
/// (`Option`, `Named`, `Vec`, `Tuple`, `Fn`, ...). The composite Option
/// truthy path ([`crate::transformer::statements::control_flow::Transformer::build_option_truthy_match_arms`])
/// handles those cases via match-based emission.
///
/// For unified dispatch on arbitrary expressions (with `is_some_and` /
/// `matches!` chain / always-truthy fold), prefer [`truthy_predicate_for_expr`].
pub(crate) fn truthy_predicate(name: &str, ty: &RustType) -> Option<Expr> {
    let ident = Expr::Ident(name.to_string());
    truthy_predicate_primitive(&ident, ty)
}

/// JS falsy predicate (De Morgan inverse of [`truthy_predicate`]).
///
/// For `RustType::F64` this yields `x == 0.0 || x.is_nan()` (NaN falsy is
/// required for JS parity — naive `x == 0.0` misclassifies `NaN` as truthy).
pub(crate) fn falsy_predicate(name: &str, ty: &RustType) -> Option<Expr> {
    let ident = Expr::Ident(name.to_string());
    falsy_predicate_primitive(&ident, ty)
}

/// Primitive (`Bool` / `F64` / `String` / `Primitive(int)`) truthy predicate
/// on an arbitrary operand expression. Returns `None` for non-primitive types.
fn truthy_predicate_primitive(operand: &Expr, ty: &RustType) -> Option<Expr> {
    match ty {
        RustType::F64 => Some(f64_truthy(operand)),
        RustType::String => Some(string_truthy(operand)),
        RustType::Bool => Some(operand.clone()),
        RustType::Primitive(kind) => Some(int_truthy(operand, *kind)),
        _ => None,
    }
}

fn falsy_predicate_primitive(operand: &Expr, ty: &RustType) -> Option<Expr> {
    match ty {
        RustType::F64 => Some(f64_falsy(operand)),
        RustType::String => Some(Expr::MethodCall {
            object: Box::new(operand.clone()),
            method: "is_empty".to_string(),
            args: vec![],
        }),
        RustType::Bool => Some(Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(operand.clone()),
        }),
        RustType::Primitive(kind) => Some(int_falsy(operand, *kind)),
        _ => None,
    }
}

fn f64_truthy(operand: &Expr) -> Expr {
    let ne_zero = Expr::BinaryOp {
        left: Box::new(operand.clone()),
        op: BinOp::NotEq,
        right: Box::new(Expr::NumberLit(0.0)),
    };
    let not_nan = Expr::UnaryOp {
        op: UnOp::Not,
        operand: Box::new(Expr::MethodCall {
            object: Box::new(operand.clone()),
            method: "is_nan".to_string(),
            args: vec![],
        }),
    };
    Expr::BinaryOp {
        left: Box::new(ne_zero),
        op: BinOp::LogicalAnd,
        right: Box::new(not_nan),
    }
}

fn f64_falsy(operand: &Expr) -> Expr {
    let eq_zero = Expr::BinaryOp {
        left: Box::new(operand.clone()),
        op: BinOp::Eq,
        right: Box::new(Expr::NumberLit(0.0)),
    };
    let is_nan = Expr::MethodCall {
        object: Box::new(operand.clone()),
        method: "is_nan".to_string(),
        args: vec![],
    };
    Expr::BinaryOp {
        left: Box::new(eq_zero),
        op: BinOp::LogicalOr,
        right: Box::new(is_nan),
    }
}

fn string_truthy(operand: &Expr) -> Expr {
    Expr::UnaryOp {
        op: UnOp::Not,
        operand: Box::new(Expr::MethodCall {
            object: Box::new(operand.clone()),
            method: "is_empty".to_string(),
            args: vec![],
        }),
    }
}

fn int_truthy(operand: &Expr, _kind: PrimitiveIntKind) -> Expr {
    Expr::BinaryOp {
        left: Box::new(operand.clone()),
        op: BinOp::NotEq,
        right: Box::new(Expr::IntLit(0)),
    }
}

fn int_falsy(operand: &Expr, _kind: PrimitiveIntKind) -> Expr {
    Expr::BinaryOp {
        left: Box::new(operand.clone()),
        op: BinOp::Eq,
        right: Box::new(Expr::IntLit(0)),
    }
}

// --- New expr-level API (I-171 T2) -----------------------------------------

/// JS truthy predicate expression for an arbitrary operand at the given
/// effective type.
///
/// Returns a boolean-valued Rust expression that is `true` iff `<expr>`
/// would be truthy in JS. For side-effect-prone operands (non-pure per
/// [`is_pure_operand`]), the returned expression wraps a `TempBinder`-vended
/// temporary binding to guarantee single evaluation.
///
/// Dispatches per RustType:
/// - `Bool`: passthrough `<e>`
/// - `F64`: `<e> != 0.0 && !<e>.is_nan()` (tmp bind for non-pure)
/// - `String`: `!<e>.is_empty()` (tmp bind for non-pure)
/// - `Primitive(int)`: `<e> != 0`
/// - `Option<T Copy>`: `<e>.is_some_and(|v| <truthy(*v)>)`
/// - `Option<T !Copy>`: `<e>.as_ref().is_some_and(|v| <truthy(v)>)`
/// - `Option<synthetic union>`: chain of `matches!(&<e>, Some(U::V(_)) if ...)` OR'd
/// - `Option<Named other>`: `<e>.is_some()`
/// - always-truthy (`Vec` / `Fn` / `StdCollection` / `DynTrait` / `Ref` / `Tuple` / `Named non-union`):
///   const `true` (with tmp bind emission preserved for non-pure `<e>` side effects)
/// - `Any` / `TypeVar`: `None` (blocked I-050 / generic bounds PRD)
/// - `Unit` / `Never` / `Result` / `QSelf`: `None` (NA — see Matrix A.12)
pub(crate) fn truthy_predicate_for_expr(
    operand: &Expr,
    ty: &RustType,
    synthetic: &SyntheticTypeRegistry,
    binder: &mut TempBinder,
) -> Option<Expr> {
    dispatch_predicate(operand, ty, synthetic, binder, Polarity::Truthy)
}

/// JS falsy predicate (De Morgan inverse of [`truthy_predicate_for_expr`]).
///
/// Returns a boolean-valued Rust expression that is `true` iff `<expr>`
/// would be falsy in JS. Same dispatch rules as truthy; `Option<synthetic union>`
/// falsy is `match &<e> { None => true, Some(U::V(v)) if <v falsy> => true,
/// _ => false }` (per-variant falsy guards plus None arm).
pub(crate) fn falsy_predicate_for_expr(
    operand: &Expr,
    ty: &RustType,
    synthetic: &SyntheticTypeRegistry,
    binder: &mut TempBinder,
) -> Option<Expr> {
    dispatch_predicate(operand, ty, synthetic, binder, Polarity::Falsy)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Polarity {
    Truthy,
    Falsy,
}

fn dispatch_predicate(
    operand: &Expr,
    ty: &RustType,
    synthetic: &SyntheticTypeRegistry,
    binder: &mut TempBinder,
    polarity: Polarity,
) -> Option<Expr> {
    match ty {
        RustType::Bool | RustType::F64 | RustType::String | RustType::Primitive(_) => {
            predicate_primitive_with_tmp(operand, ty, binder, polarity)
        }
        RustType::Option(inner) => predicate_option(operand, inner, synthetic, binder, polarity),
        RustType::Vec(_)
        | RustType::Fn { .. }
        | RustType::StdCollection { .. }
        | RustType::DynTrait(_)
        | RustType::Ref(_)
        | RustType::Tuple(_) => Some(const_truthiness_with_side_effect(operand, binder, polarity)),
        RustType::Named { name, .. } => {
            // `Option<Named>` is handled via `RustType::Option` above; a bare
            // `Named` as the declared variable type is always-truthy unless it
            // is a synthetic union enum whose variants carry primitives.
            if is_synthetic_union_enum(synthetic, name) {
                predicate_synthetic_union_bare(operand, name, synthetic, binder, polarity)
            } else {
                Some(const_truthiness_with_side_effect(operand, binder, polarity))
            }
        }
        RustType::Any | RustType::TypeVar { .. } => None,
        // NA cells — see Matrix A.12 / O.12.
        RustType::Unit | RustType::Never | RustType::Result { .. } | RustType::QSelf { .. } => None,
    }
}

/// Primitive predicate with tmp-binding for non-pure operands.
fn predicate_primitive_with_tmp(
    operand: &Expr,
    ty: &RustType,
    binder: &mut TempBinder,
    polarity: Polarity,
) -> Option<Expr> {
    let build = |op: &Expr| -> Option<Expr> {
        match polarity {
            Polarity::Truthy => truthy_predicate_primitive(op, ty),
            Polarity::Falsy => falsy_predicate_primitive(op, ty),
        }
    };
    if is_pure_operand(operand) {
        build(operand)
    } else {
        let name = binder.fresh("op");
        let tmp_ref = Expr::Ident(name.clone());
        let pred = build(&tmp_ref)?;
        Some(Expr::Block(vec![
            Stmt::Let {
                mutable: false,
                name,
                ty: Some(ty.clone()),
                init: Some(operand.clone()),
            },
            Stmt::TailExpr(pred),
        ]))
    }
}

/// Constant-truthiness emission that preserves observable side effects.
///
/// For always-truthy types, the JS result of `!<e>` is `false` (or truthy
/// predicate = `true`). But if `<e>` has observable side effects, we must
/// still evaluate it before returning the constant. Pure operands emit a
/// bare `true`/`false`; non-pure operands emit `{ let _ = <e>; <const>; }`.
fn const_truthiness_with_side_effect(
    operand: &Expr,
    binder: &mut TempBinder,
    polarity: Polarity,
) -> Expr {
    // Always-truthy: truthy predicate is `true`, falsy is `false`.
    let value = Expr::BoolLit(polarity == Polarity::Truthy);
    if is_pure_operand(operand) {
        value
    } else {
        let name = binder.fresh("eval");
        Expr::Block(vec![
            Stmt::Let {
                mutable: false,
                name,
                ty: None,
                init: Some(operand.clone()),
            },
            Stmt::TailExpr(value),
        ])
    }
}

/// `Option<T>` predicate dispatch.
fn predicate_option(
    operand: &Expr,
    inner: &RustType,
    synthetic: &SyntheticTypeRegistry,
    binder: &mut TempBinder,
    polarity: Polarity,
) -> Option<Expr> {
    match inner {
        RustType::Bool | RustType::F64 | RustType::String | RustType::Primitive(_) => {
            predicate_option_primitive(operand, inner, binder, polarity)
        }
        RustType::Named { name, .. } => {
            if is_synthetic_union_enum(synthetic, name) {
                predicate_option_synthetic_union(operand, name, synthetic, binder, polarity)
            } else {
                // Option<Named other>: is_some / is_none.
                Some(option_is_some_or_none(operand, polarity))
            }
        }
        RustType::Vec(_)
        | RustType::Fn { .. }
        | RustType::StdCollection { .. }
        | RustType::DynTrait(_)
        | RustType::Ref(_)
        | RustType::Tuple(_) => {
            // Option<always-truthy>: is_some / is_none (inner never falsy).
            Some(option_is_some_or_none(operand, polarity))
        }
        // Option<Option<T>> / Option<Result> / Option<QSelf>: NA invariant —
        // IR `wrap_optional` idempotent + Option<Result>/QSelf never emitted.
        RustType::Option(_)
        | RustType::Result { .. }
        | RustType::Unit
        | RustType::Never
        | RustType::QSelf { .. } => None,
        RustType::Any | RustType::TypeVar { .. } => None,
    }
}

fn option_is_some_or_none(operand: &Expr, polarity: Polarity) -> Expr {
    let method = match polarity {
        Polarity::Truthy => "is_some",
        Polarity::Falsy => "is_none",
    };
    Expr::MethodCall {
        object: Box::new(operand.clone()),
        method: method.to_string(),
        args: vec![],
    }
}

/// `Option<primitive>` predicate.
///
/// - Copy inner (`Bool` / `F64` / `Primitive(int)`): `<e>.is_some_and(|v| <inner truthy(*v)>)`
/// - Non-Copy inner (`String`): `<e>.as_ref().is_some_and(|v| <inner truthy(v)>)`
///
/// `is_some_and` consumes `self`, so for non-Copy `<e>` we take `as_ref()`
/// first. Copy inner consumes by copy, safe for subsequent reads.
fn predicate_option_primitive(
    operand: &Expr,
    inner: &RustType,
    binder: &mut TempBinder,
    polarity: Polarity,
) -> Option<Expr> {
    let inner_copy = inner.is_copy_type();
    // Closure parameter for `is_some_and` / `as_ref().is_some_and`.
    // - `Option<T Copy>::is_some_and(|v| ...)` — `v: T` (by value).
    // - `Option<T !Copy>::as_ref().is_some_and(|v| ...)` — `v: &T`.
    //
    // In both cases, primitive predicates on `v` typecheck without explicit
    // deref: Copy inner gives a value; `&T` auto-derefs for method calls
    // (`v.is_empty()`) and numeric comparisons against primitives typecheck
    // via auto-deref coercion (e.g., `*v != 0.0` is required only when the
    // inner primitive is accessed through a reference that does not auto-deref
    // for `!=` — which applies to `&f64` vs `f64`). To keep the Copy-path
    // consistent with JS (v is the value), we emit `v != 0.0` directly. For
    // the !Copy path (e.g., `Option<String>`), `v` is `&String` which
    // auto-derefs for `.is_empty()` so no explicit deref is needed either.
    let v_name = "v";
    let inner_operand_for_pred = Expr::Ident(v_name.to_string());
    // Both polarities build the inner closure as the TRUTHY predicate of `v`
    // because `is_some_and` is a positive check: the outer Option-level
    // polarity flip is applied via `!<is_some_and>` for `Falsy`.
    //
    // Rationale (De Morgan on Option):
    //   truthy(Option<T>) = x.is_some_and(|v| truthy(v))
    //   falsy(Option<T>)  = !truthy(Option<T>)
    //                     = !x.is_some_and(|v| truthy(v))
    //                    ≡ x.is_none() || x.is_some_and(|v| falsy(v))
    //
    // Using `!is_some_and(truthy)` keeps a single check; the explicit
    // is_none OR form works too but doubles the predicate surface for no
    // additional clarity.
    let inner_pred = truthy_predicate_primitive(&inner_operand_for_pred, inner)?;
    // Build the receiver: `<e>` (Copy inner) or `<e>.as_ref()` (!Copy inner).
    let receiver = maybe_tmp_bind(operand, binder, |op| {
        if inner_copy {
            op.clone()
        } else {
            Expr::MethodCall {
                object: Box::new(op.clone()),
                method: "as_ref".to_string(),
                args: vec![],
            }
        }
    });
    // Truthy: `<receiver>.is_some_and(|v| <inner truthy>)`
    // Falsy: `!<receiver>.is_some_and(|v| <inner truthy>)` (De Morgan: None is falsy → result true)
    let closure = Expr::Closure {
        params: vec![closure_param(v_name)],
        return_type: None,
        body: crate::ir::ClosureBody::Expr(Box::new(inner_pred)),
    };
    let is_some_and = Expr::MethodCall {
        object: Box::new(receiver.tail.clone()),
        method: "is_some_and".to_string(),
        args: vec![closure],
    };
    let pred = match polarity {
        Polarity::Truthy => is_some_and,
        Polarity::Falsy => Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(is_some_and),
        },
    };
    Some(receiver.wrap(pred))
}

/// `Option<synthetic union enum>` predicate (single match on a borrow).
///
/// Emits a single `match &<e> { ... }` expression with per-variant guards so
/// the operand is borrowed exactly once and non-`Copy` synthetic union values
/// are not moved across multiple pattern-test arms.
///
/// Shape (truthy polarity):
///
/// ```ignore
/// match &<e> {
///     Some(U::V0(v)) if <v truthy>  => true,
///     Some(U::V1(v))                => true,  // non-primitive variant — always truthy
///     _                             => false,
/// }
/// ```
///
/// Falsy polarity is the De Morgan inverse:
///
/// ```ignore
/// match &<e> {
///     None                          => true,
///     Some(U::V0(v)) if <v falsy>   => true,
///     Some(U::V1(_))                => false, // non-primitive — never falsy
///     _                             => false,
/// }
/// ```
///
/// Non-primitive variants that would be `VariantGuard::ConstFalse` are
/// elided (they can't contribute a `true` arm); the default `_ => false`
/// arm absorbs them.
fn predicate_option_synthetic_union(
    operand: &Expr,
    enum_name: &str,
    synthetic: &SyntheticTypeRegistry,
    binder: &mut TempBinder,
    polarity: Polarity,
) -> Option<Expr> {
    let def = synthetic.get(enum_name)?;
    if def.kind != SyntheticTypeKind::UnionEnum {
        return None;
    }
    let crate::ir::Item::Enum { variants, .. } = &def.item else {
        return None;
    };
    let enum_ref = UserTypeRef::new(enum_name.to_string());
    let inner_bind = "__ts_union_inner";

    let mut arms: Vec<crate::ir::MatchArm> = Vec::with_capacity(variants.len() + 2);
    if polarity == Polarity::Falsy {
        arms.push(crate::ir::MatchArm {
            patterns: vec![Pattern::none()],
            guard: None,
            body: vec![Stmt::TailExpr(Expr::BoolLit(true))],
        });
    }
    for variant in variants {
        let variant_ty = variant.data.as_ref()?;
        let guard = build_variant_guard_for_ref_bind(inner_bind, variant_ty, polarity);
        let pattern = Pattern::TupleStruct {
            ctor: PatternCtor::Builtin(BuiltinVariant::Some),
            fields: vec![Pattern::TupleStruct {
                ctor: PatternCtor::UserEnumVariant {
                    enum_ty: enum_ref.clone(),
                    variant: variant.name.clone(),
                },
                fields: vec![Pattern::binding(inner_bind)],
            }],
        };
        match guard {
            VariantGuard::ConstTrue => arms.push(crate::ir::MatchArm {
                patterns: vec![pattern],
                guard: None,
                body: vec![Stmt::TailExpr(Expr::BoolLit(true))],
            }),
            VariantGuard::ConstFalse => {
                // Variant never contributes a `true` arm; the default `_`
                // arm below absorbs it with `false`.
            }
            VariantGuard::Guard(g) => arms.push(crate::ir::MatchArm {
                patterns: vec![pattern],
                guard: Some(g),
                body: vec![Stmt::TailExpr(Expr::BoolLit(true))],
            }),
        }
    }
    arms.push(crate::ir::MatchArm {
        patterns: vec![Pattern::Wildcard],
        guard: None,
        body: vec![Stmt::TailExpr(Expr::BoolLit(false))],
    });

    // `match &<e>` — borrow so non-Copy values are not moved.
    let borrow = maybe_tmp_bind(operand, binder, |e| Expr::Ref(Box::new(e.clone())));
    let match_expr = Expr::Match {
        expr: Box::new(borrow.tail.clone()),
        arms,
    };
    Some(borrow.wrap(match_expr))
}

/// Bare `Named synthetic union` (not wrapped in Option) predicate.
///
/// Same match-on-borrow shape as [`predicate_option_synthetic_union`] but
/// without the outer `Some(...)` wrapping and without the `None` arm for
/// falsy (a bare union value is always `Some` at the Rust level).
fn predicate_synthetic_union_bare(
    operand: &Expr,
    enum_name: &str,
    synthetic: &SyntheticTypeRegistry,
    binder: &mut TempBinder,
    polarity: Polarity,
) -> Option<Expr> {
    let def = synthetic.get(enum_name)?;
    if def.kind != SyntheticTypeKind::UnionEnum {
        return None;
    }
    let crate::ir::Item::Enum { variants, .. } = &def.item else {
        return None;
    };
    let enum_ref = UserTypeRef::new(enum_name.to_string());
    let inner_bind = "__ts_union_inner";

    let mut arms: Vec<crate::ir::MatchArm> = Vec::with_capacity(variants.len() + 1);
    for variant in variants {
        let variant_ty = variant.data.as_ref()?;
        let guard = build_variant_guard_for_ref_bind(inner_bind, variant_ty, polarity);
        let pattern = Pattern::TupleStruct {
            ctor: PatternCtor::UserEnumVariant {
                enum_ty: enum_ref.clone(),
                variant: variant.name.clone(),
            },
            fields: vec![Pattern::binding(inner_bind)],
        };
        match guard {
            VariantGuard::ConstTrue => arms.push(crate::ir::MatchArm {
                patterns: vec![pattern],
                guard: None,
                body: vec![Stmt::TailExpr(Expr::BoolLit(true))],
            }),
            VariantGuard::ConstFalse => {}
            VariantGuard::Guard(g) => arms.push(crate::ir::MatchArm {
                patterns: vec![pattern],
                guard: Some(g),
                body: vec![Stmt::TailExpr(Expr::BoolLit(true))],
            }),
        }
    }
    arms.push(crate::ir::MatchArm {
        patterns: vec![Pattern::Wildcard],
        guard: None,
        body: vec![Stmt::TailExpr(Expr::BoolLit(false))],
    });

    let borrow = maybe_tmp_bind(operand, binder, |e| Expr::Ref(Box::new(e.clone())));
    let match_expr = Expr::Match {
        expr: Box::new(borrow.tail.clone()),
        arms,
    };
    Some(borrow.wrap(match_expr))
}

enum VariantGuard {
    /// Variant always matches this polarity (no guard needed).
    ConstTrue,
    /// Variant never matches this polarity (skip arm).
    ConstFalse,
    /// Variant matches conditionally on the given guard.
    Guard(Expr),
}

/// Emits a per-variant guard predicate for synthetic union variants where the
/// inner binding is `&T` (from a borrowed `match &<e>`). Numeric primitives
/// need explicit deref (`*v`) to typecheck against value-level predicates
/// (`*v != 0.0`, `*v == 0`).
fn build_variant_guard_for_ref_bind(
    inner_bind: &str,
    variant_ty: &RustType,
    polarity: Polarity,
) -> VariantGuard {
    let bind = Expr::Ident(inner_bind.to_string());
    match variant_ty {
        RustType::F64 | RustType::Bool | RustType::Primitive(_) => {
            // Deref for primitive comparison predicates.
            let deref = Expr::Deref(Box::new(bind));
            let pred = match polarity {
                Polarity::Truthy => truthy_predicate_primitive(&deref, variant_ty),
                Polarity::Falsy => falsy_predicate_primitive(&deref, variant_ty),
            };
            match pred {
                Some(g) => VariantGuard::Guard(g),
                None => VariantGuard::ConstTrue,
            }
        }
        RustType::String => {
            // `&String` auto-derefs to `&str` on `.is_empty()`; no explicit deref.
            let pred = match polarity {
                Polarity::Truthy => truthy_predicate_primitive(&bind, variant_ty),
                Polarity::Falsy => falsy_predicate_primitive(&bind, variant_ty),
            };
            match pred {
                Some(g) => VariantGuard::Guard(g),
                None => VariantGuard::ConstTrue,
            }
        }
        _ => match polarity {
            Polarity::Truthy => VariantGuard::ConstTrue,
            Polarity::Falsy => VariantGuard::ConstFalse,
        },
    }
}

/// Helper: tmp-bind a non-pure operand. Returns a struct with `tail` (the
/// expression to use in place of the operand, possibly the same Ident as
/// the let binding) and a `wrap` function to wrap the final predicate into
/// a block-with-binding if needed.
struct TmpBound {
    tail: Expr,
    stmts: Vec<Stmt>,
}

impl TmpBound {
    fn wrap(mut self, pred: Expr) -> Expr {
        if self.stmts.is_empty() {
            pred
        } else {
            self.stmts.push(Stmt::TailExpr(pred));
            Expr::Block(self.stmts)
        }
    }
}

fn maybe_tmp_bind(
    operand: &Expr,
    binder: &mut TempBinder,
    shape: impl FnOnce(&Expr) -> Expr,
) -> TmpBound {
    if is_pure_operand(operand) {
        TmpBound {
            tail: shape(operand),
            stmts: vec![],
        }
    } else {
        let name = binder.fresh("op");
        let tmp_ref = Expr::Ident(name.clone());
        let tail = shape(&tmp_ref);
        TmpBound {
            tail,
            stmts: vec![Stmt::Let {
                mutable: false,
                name,
                ty: None,
                init: Some(operand.clone()),
            }],
        }
    }
}

fn closure_param(name: &str) -> crate::ir::Param {
    crate::ir::Param {
        name: name.to_string(),
        ty: None,
    }
}

// --- Constant fold (I-171 T2) ----------------------------------------------

/// AST-level const fold for `!<literal>` / `!<always-truthy literal>` so
/// `convert_unary_expr` Bang arm can emit `Expr::BoolLit` directly without
/// resolving the operand's type.
///
/// Returns `None` when the operand has observable side effects (the Bang arm
/// falls through to [`falsy_predicate_for_expr`] which handles tmp binding
/// for side-effect-prone types).
///
/// | Operand AST                         | Fold result |
/// |-------------------------------------|-------------|
/// | `Lit(Null)` / `undefined` (Ident)  | `true`      |
/// | `Lit(Bool(b))`                      | `!b`        |
/// | `Lit(Num(0.0))` / `Lit(Num(NaN))`   | `true`      |
/// | `Lit(Num(other))`                   | `false`     |
/// | `Lit(Str(""))`                      | `true`      |
/// | `Lit(Str(non-empty))`               | `false`     |
/// | `Lit(BigInt(0))`                    | `true`      |
/// | `Lit(BigInt(non-0))`                | `false`     |
/// | `Lit(Regex)`                        | `false`     |
/// | `Arrow` / `Fn` literal              | `false`     |
///
/// Array / Object / Tpl / New / This are NOT folded here — their elements
/// may have side effects; the caller emits `{ let _ = <e>; false }` via
/// [`falsy_predicate_for_expr`] / [`is_always_truthy_type`].
///
/// ## Current consumer: pending T4 (I-171 Layer 1)
///
/// This helper is T2-scoped infrastructure. T3 (I-161 `&&=`/`||=` desugar)
/// does not call it directly. T4 (`convert_unary_expr` Bang arm type-aware
/// dispatch) is the primary consumer. The `dead_code` allow is intentional
/// — the function is integration-tested (12 unit tests below) and ready for
/// T4 hook-up without further changes.
#[allow(dead_code)]
pub(crate) fn try_constant_fold_bang(expr: &ast::Expr) -> Option<Expr> {
    match expr {
        ast::Expr::Lit(ast::Lit::Null(_)) => Some(Expr::BoolLit(true)),
        ast::Expr::Ident(id) if id.sym.as_ref() == "undefined" => Some(Expr::BoolLit(true)),
        ast::Expr::Lit(ast::Lit::Bool(b)) => Some(Expr::BoolLit(!b.value)),
        ast::Expr::Lit(ast::Lit::Num(n)) => {
            let falsy = n.value == 0.0 || n.value.is_nan();
            Some(Expr::BoolLit(falsy))
        }
        ast::Expr::Lit(ast::Lit::Str(s)) => Some(Expr::BoolLit(s.value.is_empty())),
        ast::Expr::Lit(ast::Lit::BigInt(b)) => {
            // BigInt value is non-zero iff its textual representation is not "0".
            let is_zero = b.value.to_string() == "0";
            Some(Expr::BoolLit(is_zero))
        }
        ast::Expr::Lit(ast::Lit::Regex(_)) => Some(Expr::BoolLit(false)),
        // Arrow / Fn literal evaluates to a function reference (always truthy);
        // the definition itself has no side effects.
        ast::Expr::Arrow(_) | ast::Expr::Fn(_) => Some(Expr::BoolLit(false)),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
