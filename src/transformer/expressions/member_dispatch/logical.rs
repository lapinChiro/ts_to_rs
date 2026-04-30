//! Logical compound assign context dispatch helpers (`obj.x ??= d` /
//! `obj.x &&= v` / `obj.x ||= v` etc.).
//!
//! Called from the [`super::Transformer::try_dispatch_member_logical_compound`]
//! entry method which routes `convert_assign_expr`'s logical compound (`??=` /
//! `&&=` / `||=`) × Member × non-Computed gate (expression context) plus the
//! statement-context paths (`try_convert_nullish_assign_stmt` /
//! `try_convert_compound_logical_assign_stmt`). I-205 T9 cells 36-41 lock-in
//! plus orthogonality-equivalent extensions for Any / TypeVar / non-Option /
//! always-truthy LHS types (T9 Iteration v14 deep-deep review structural
//! completeness).
//!
//! ## Architectural concern
//!
//! Symmetric counterpart of [`super::compound`] (T8 arithmetic / bitwise compound
//! assign): both dispatch B4 setter desugar via the [`super::shared`] helpers,
//! but logical compound differs in the **emission shape**:
//! - Arithmetic / bitwise compound (T8): `{ let __ts_new = obj.x() OP rhs;
//!   obj.set_x(__ts_new); __ts_new }` — unconditional setter call.
//! - Logical compound (T9): per-LHS-strategy emission cohesive with existing
//!   `nullish_assign.rs::pick_strategy` + `compound_logical_assign.rs`
//!   `const_fold_always_truthy_stmts` patterns:
//!
//! ### `??=` (NullishAssign) strategies (per [`pick_strategy`])
//!
//! - `ShadowLet` (LHS = `Option<T>`): conditional setter desugar
//!   `{ if <getter>.is_none() { <setter>(Some(rhs)); }; <tail> }` — cells 36
//!   (Fallback B1 field route)、38 (B4 instance)、41-d (B8 static)。
//! - `Identity` (LHS = non-Option non-Any: F64 / String / Bool / Named / Vec /
//!   etc.): TS `??=` is dead code on non-nullable T. Emit no-setter form with
//!   INV-3 1-evaluate compliance for SE-having receiver:
//!     - Statement / SE-free: empty Block (`{}` no-op)
//!     - Statement / SE-having: `{ <obj>; }` evaluate-discard
//!     - Expression / SE-free: `<obj>.x()` direct getter call
//!     - Expression / SE-having: `{ let __ts_recv = <obj>; __ts_recv.x() }`
//!       IIFE evaluate-once + yield
//!     - Static dispatch: same shapes substituting `Class::x()` for
//!       `<receiver>.x()` (no IIFE; class TypeName side-effect-free)
//! - `BlockedByI050` (LHS = `Any`): Tier 2 honest error
//!   `"nullish-assign on Any class member (I-050 Any coercion umbrella)"` —
//!   consistent with existing `nullish_assign.rs::try_convert_nullish_assign_stmt`
//!   `BlockedByI050` wording for symmetric Ident-target case。
//!
//! ### `&&=` / `||=` strategies (per [`is_always_truthy_type`] + truthy.rs Matrix A.12)
//!
//! - **Any / TypeVar**: Tier 2 honest error
//!   `"compound logical assign on Any/TypeVar class member (I-050 umbrella /
//!     generic bounds)"` — consistent with existing
//!   `compound_logical_assign.rs::desugar_compound_logical_assign_stmts`
//!   blocked path wording。
//! - **Always-truthy** (Vec / Fn / StdCollection / DynTrait / Ref / Tuple /
//!   Named non-union): const-fold (predicate is statically `true`/`false`):
//!     - `&&=` always-truthy (predicate = `true`): unconditional setter call
//!       `<setter>(rhs);` (statement) or `{ <setter>(rhs); <tail getter> }`
//!       (expression with INV-3 IIFE for SE-having)
//!     - `||=` always-truthy (predicate = `false`): no-op (statement) or
//!       getter-yield (expression with INV-3 IIFE for SE-having)
//! - **Predicate-supported** (Bool / F64 / String / Option / Primitive /
//!   Named-synthetic-union): conditional setter desugar
//!   `{ if <truthy/falsy_predicate(<getter>)> { <setter>(<wrap(rhs)>); };
//!      <tail> }`。
//! - **NA per truthy.rs Matrix A.12** (Unit / Never / Result / QSelf): Tier 2
//!   honest error `"logical compound assign on unsupported lhs type
//!     (truthy/falsy predicate unavailable)"`。
//!
//! ### Setter argument wrapping
//!
//! `wrap_setter_value` wraps `rhs` in `Some(_)` when LHS = `Option<T>` (matches
//! `compound_assign_value` pattern + cell 38 ideal output). For non-Option LHS
//! (always-truthy or predicate-supported non-Option), pass raw `rhs`。
//!
//! ## INV-3 1-evaluate compliance
//!
//! For SE-having instance receiver (`getInstance().value ??= 42` 等)、IIFE form
//! `{ let mut __ts_recv = <obj>; ... __ts_recv 経由の getter / setter calls ... }`
//! で receiver 1-evaluate 保証 (T7/T8 IIFE pattern reuse via
//! [`is_side_effect_free`] + [`TS_RECV_BINDING`])。Static dispatch では receiver
//! = class TypeName で side-effect なし、IIFE wrap 不要。

use anyhow::Result;
use swc_common::Spanned;
use swc_ecma_ast as ast;

use crate::ir::{BuiltinVariant, CallTarget, Expr, MethodKind, RustType, Stmt, UserTypeRef};
use crate::pipeline::synthetic_registry::SyntheticTypeRegistry;
use crate::registry::MethodSignature;
use crate::transformer::helpers::truthy::{
    falsy_predicate_for_expr, is_always_truthy_type, truthy_predicate_for_expr, TempBinder,
};
use crate::transformer::statements::nullish_assign::{pick_strategy, NullishAssignStrategy};
use crate::transformer::UnsupportedSyntaxError;

use super::super::TS_RECV_BINDING;
use super::shared::{is_side_effect_free, MemberKindFlags};

/// Whether the dispatched block should yield the post-assign LHS value as a
/// [`Stmt::TailExpr`] (= expression context), or be statement-only without a
/// trailing value (= statement context).
///
/// Statement context (`obj.x ??= d;` as a bare `Stmt::Expr`): no tail needed,
/// caller wraps the resulting block in `Stmt::Expr(Expr::Block(stmts))`。
///
/// Expression context (`obj.x ??= d` inside call args / return value / ternary
/// branch / etc.): tail = `<getter>` returning the post-state value, matching
/// TS semantics where `obj.x ??= d` evaluates to the (post-assign) `obj.x` value
/// (= matrix-acknowledged Option<T> divergence from TS narrowing-after-??=
/// semantic; subsequent PRD candidate per `## Spec Revision Log` deferral)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogicalCompoundContext {
    Statement,
    Expression,
}

/// Receiver-dependent dispatch context for class member logical compound
/// emission, type-safely enumerating the two variants:
///
/// - [`Self::Instance`]: instance receiver (caller-converted `Expr`)、
///   getter / setter calls emitted as `MethodCall` with receiver expression。
///   IIFE wrap fires for SE-having receiver per INV-3 1-evaluate compliance。
/// - [`Self::Static`]: static class TypeName receiver、getter / setter calls
///   emitted as `FnCall::UserAssocFn` (Rust associated fn path access)。
///   class TypeName is statically side-effect-free → IIFE wrap never fires。
///
/// Decouples the receiver-specific call construction (instance / static)
/// from the strategy-specific emission shape (ShadowLet / Identity /
/// always-truthy / predicate-supported)、shared by `dispatch_b4_strategy` +
/// 4 strategy emitters (`emit_nullish_shadow_let` / `emit_identity_dispatch`
/// / `emit_always_truthy_const_fold` / `emit_logical_compound_predicate_dispatch`)
/// uniformly。
///
/// Invariant codification (`design-integrity.md` 凝集度 / 責務分離):
/// - `Static` variant: receiver is a class TypeName (= statically SE-free)、
///   IIFE wrap never fires → SE-having branches (`emit_identity_dispatch`'s
///   SE-having statement / expression、`wrap_with_iife_if_needed`'s IIFE arm)
///   use `unreachable!()` to codify the invariant via Rust compiler structural
///   enforcement (= future T11 (11-c) static matrix expansion 等で invariant
///   violation を compile error / panic で immediate detect)。
/// - `Instance` variant: receiver is a runtime expression (Ident / FieldAccess
///   / FnCall / etc.) requiring [`is_side_effect_free`] check + IIFE wrap for
///   SE-having case。
enum ReceiverCalls<'a> {
    /// Instance receiver: getter / setter calls emit as `MethodCall` with
    /// `receiver_for_calls` (= `object.clone()` for SE-free / `Ident("__ts_recv")`
    /// for SE-having IIFE binding)。
    Instance {
        /// Original receiver expression IR (caller-converted via
        /// `convert_expr(&member.obj)`)。Used for IIFE binding init for SE-having
        /// receivers (`let mut __ts_recv = <object>;`)、SE-free path embeds
        /// `object.clone()` directly twice (cheap reference copy at the Rust
        /// source level)。
        object: &'a Expr,
        /// Field / method name for getter (`obj.field()`) and setter
        /// (`obj.set_field(arg)`) emission。
        field: &'a str,
    },
    /// Static class TypeName receiver: getter / setter calls emit as
    /// `FnCall::UserAssocFn` with `class_name` as the type path。
    Static {
        /// Class TypeName for `Class::field()` / `Class::set_field(arg)` path
        /// emission。
        class_name: &'a str,
        /// Field / method name (associated fn path component)。
        field: &'a str,
    },
}

impl<'a> ReceiverCalls<'a> {
    fn build_getter_call(&self, receiver_for_calls: &Expr) -> Expr {
        match self {
            Self::Instance { field, .. } => Expr::MethodCall {
                object: Box::new(receiver_for_calls.clone()),
                method: (*field).to_string(),
                args: vec![],
            },
            Self::Static { class_name, field } => Expr::FnCall {
                target: CallTarget::UserAssocFn {
                    ty: UserTypeRef::new(*class_name),
                    method: (*field).to_string(),
                },
                args: vec![],
            },
        }
    }

    fn build_setter_call(&self, receiver_for_calls: &Expr, arg: Expr) -> Expr {
        match self {
            Self::Instance { field, .. } => Expr::MethodCall {
                object: Box::new(receiver_for_calls.clone()),
                method: format!("set_{field}"),
                args: vec![arg],
            },
            Self::Static { class_name, field } => Expr::FnCall {
                target: CallTarget::UserAssocFn {
                    ty: UserTypeRef::new(*class_name),
                    method: format!("set_{field}"),
                },
                args: vec![arg],
            },
        }
    }
}

/// Dispatches logical compound (`??=` / `&&=` / `||=`) on a class member with
/// instance receiver. Returns `Expr::Block` for both contexts (statement caller
/// wraps with `Stmt::Expr`)。
///
/// Dispatch arms (matrix cells 36-41 logical):
/// - `is_inherited = true` (B7) → Tier 2 honest `"logical compound assign to
///   inherited accessor"` (cell 41-c)
/// - `has_getter && has_setter` (B4) → strategy-driven dispatch per op:
///     - `??=`: `pick_strategy(lhs_type)` → ShadowLet / Identity / BlockedByI050
///     - `&&=`/`||=`: Any/TypeVar gate → always-truthy const-fold → predicate-based
/// - `has_getter` only (B2) → Tier 2 honest `"logical compound assign to
///   read-only property"` (cell 37 + `&&=`/`||=` symmetric)
/// - `has_setter` only (B3) → Tier 2 honest `"logical compound assign read of
///   write-only property"` (read-side fail because predicate evaluation reads
///   the getter)
/// - `has_method` only (B6) → Tier 2 honest `"logical compound assign to
///   method"` (cell 41-b)
///
/// Cells 36 (B1 field、`obj.x.get_or_insert_with(|| d)` regression preserve) and
/// 41-e (B9 unknown、existing fallback regression preserve) do **not** reach
/// this helper — they take the `MemberReceiverClassification::Fallback` path in
/// caller-side gates which fall through to existing `nullish_assign.rs` /
/// `compound_logical_assign.rs` emission logic。
#[allow(clippy::too_many_arguments)]
pub(super) fn dispatch_instance_member_logical_compound(
    object: &Expr,
    field: &str,
    sigs: &[MethodSignature],
    is_inherited: bool,
    op: ast::AssignOp,
    rhs: Expr,
    synthetic: &SyntheticTypeRegistry,
    ts_obj: &ast::Expr,
    context: LogicalCompoundContext,
) -> Result<Expr> {
    if is_inherited {
        return Err(UnsupportedSyntaxError::new(
            "logical compound assign to inherited accessor",
            ts_obj.span(),
        )
        .into());
    }
    let kinds = MemberKindFlags::from_sigs(sigs);
    if kinds.has_getter && kinds.has_setter {
        let lhs_type = extract_getter_return_type(sigs).ok_or_else(|| {
            UnsupportedSyntaxError::new(
                "logical compound assign on getter without return type annotation",
                ts_obj.span(),
            )
        })?;
        let calls = ReceiverCalls::Instance { object, field };
        return dispatch_b4_strategy(&calls, &lhs_type, op, rhs, synthetic, ts_obj, context);
    }
    if kinds.has_getter {
        return Err(UnsupportedSyntaxError::new(
            "logical compound assign to read-only property",
            ts_obj.span(),
        )
        .into());
    }
    if kinds.has_setter {
        return Err(UnsupportedSyntaxError::new(
            "logical compound assign read of write-only property",
            ts_obj.span(),
        )
        .into());
    }
    if kinds.has_method {
        return Err(UnsupportedSyntaxError::new(
            "logical compound assign to method",
            ts_obj.span(),
        )
        .into());
    }
    unreachable!(
        "dispatch_instance_member_logical_compound: sigs is non-empty (lookup_method_sigs_in_inheritance_chain \
         never returns Some(empty vec)) and MethodKind is exhaustive (Method/Getter/Setter), \
         so one of the 4 if-blocks above must fire. field={field}"
    );
}

/// Dispatches logical compound (`??=` / `&&=` / `||=`) on a class member with
/// static (class TypeName) receiver. Symmetric to
/// [`dispatch_instance_member_logical_compound`] with `Class::method`-form
/// FnCall emit instead of `obj.method`-form MethodCall emit, and **without**
/// the IIFE wrap (= class TypeName receiver is a Rust path access, side-effect
/// なし, receiver evaluation count is statically zero)。
///
/// Dispatch arms (matrix cell 41-d primary, defensive arms for static B2/B3/B6/B7):
/// - `is_inherited = true` (defensive、static B7) → Tier 2 honest
///   `"logical compound assign to inherited static accessor"`
/// - `has_getter && has_setter` (B8) → strategy-driven dispatch per op (same as
///   instance B4 but no IIFE wrap)
/// - `has_getter` only (defensive、static B2) → Tier 2 honest
///   `"logical compound assign to read-only static property"`
/// - `has_setter` only (defensive、static B3) → Tier 2 honest
///   `"logical compound assign read of write-only static property"`
/// - `has_method` only (defensive、static B6) → Tier 2 honest
///   `"logical compound assign to static method"`
#[allow(clippy::too_many_arguments)]
pub(super) fn dispatch_static_member_logical_compound(
    class_name: &str,
    field: &str,
    sigs: &[MethodSignature],
    is_inherited: bool,
    op: ast::AssignOp,
    rhs: Expr,
    synthetic: &SyntheticTypeRegistry,
    ts_obj: &ast::Expr,
    context: LogicalCompoundContext,
) -> Result<Expr> {
    if is_inherited {
        return Err(UnsupportedSyntaxError::new(
            "logical compound assign to inherited static accessor",
            ts_obj.span(),
        )
        .into());
    }
    let kinds = MemberKindFlags::from_sigs(sigs);
    if kinds.has_getter && kinds.has_setter {
        let lhs_type = extract_getter_return_type(sigs).ok_or_else(|| {
            UnsupportedSyntaxError::new(
                "logical compound assign on static getter without return type annotation",
                ts_obj.span(),
            )
        })?;
        let calls = ReceiverCalls::Static { class_name, field };
        return dispatch_b4_strategy(&calls, &lhs_type, op, rhs, synthetic, ts_obj, context);
    }
    if kinds.has_getter {
        return Err(UnsupportedSyntaxError::new(
            "logical compound assign to read-only static property",
            ts_obj.span(),
        )
        .into());
    }
    if kinds.has_setter {
        return Err(UnsupportedSyntaxError::new(
            "logical compound assign read of write-only static property",
            ts_obj.span(),
        )
        .into());
    }
    if kinds.has_method {
        return Err(UnsupportedSyntaxError::new(
            "logical compound assign to static method",
            ts_obj.span(),
        )
        .into());
    }
    unreachable!(
        "dispatch_static_member_logical_compound: sigs is non-empty (lookup_method_sigs_in_inheritance_chain \
         never returns Some(empty vec)) and MethodKind is exhaustive (Method/Getter/Setter), \
         so one of the 4 if-blocks above must fire. class={class_name}, field={field}"
    );
}

// =============================================================================
// B4 strategy dispatch (shared between instance B4 and static B8)
// =============================================================================

/// B4 (getter+setter) strategy dispatch shared between instance and static.
///
/// Strategy axes (Layer 3 cross-axis enumeration):
/// - **op axis**: NullishAssign / AndAssign / OrAssign
/// - **lhs_type axis**: per `pick_strategy` for ??= (ShadowLet / Identity /
///   BlockedByI050)、per `is_always_truthy_type` + `truthy.rs Matrix A.12` for
///   &&=/||= (always-truthy / Any / TypeVar / NA / predicate-supported)
/// - **context axis**: Statement (no tail) / Expression (tail = post-state
///   getter call)
/// - **SE axis**: SE-free instance receiver direct embed / SE-having instance
///   receiver IIFE wrap / Static (always SE-free, no IIFE)
#[allow(clippy::too_many_arguments)]
fn dispatch_b4_strategy(
    calls: &ReceiverCalls<'_>,
    lhs_type: &RustType,
    op: ast::AssignOp,
    rhs: Expr,
    synthetic: &SyntheticTypeRegistry,
    ts_obj: &ast::Expr,
    context: LogicalCompoundContext,
) -> Result<Expr> {
    match op {
        ast::AssignOp::NullishAssign => {
            // pick_strategy enforces single source of truth for ??= LHS
            // dispatch (consistent with existing nullish_assign.rs Ident-target
            // emission)。
            match pick_strategy(lhs_type) {
                NullishAssignStrategy::ShadowLet => {
                    Ok(emit_nullish_shadow_let(calls, lhs_type, rhs, context))
                }
                NullishAssignStrategy::Identity => Ok(emit_identity_dispatch(calls, context)),
                NullishAssignStrategy::BlockedByI050 => Err(UnsupportedSyntaxError::new(
                    "nullish-assign on Any class member (I-050 Any coercion umbrella)",
                    ts_obj.span(),
                )
                .into()),
            }
        }
        ast::AssignOp::AndAssign | ast::AssignOp::OrAssign => {
            // Any / TypeVar gate first (consistent with
            // compound_logical_assign.rs::desugar_compound_logical_assign_stmts
            // blocked path wording)。
            if matches!(lhs_type, RustType::Any | RustType::TypeVar { .. }) {
                return Err(UnsupportedSyntaxError::new(
                    "compound logical assign on Any/TypeVar class member \
                     (I-050 umbrella / generic bounds)",
                    ts_obj.span(),
                )
                .into());
            }
            // Always-truthy const-fold (consistent with
            // compound_logical_assign.rs::const_fold_always_truthy_stmts)。
            if is_always_truthy_type(lhs_type, synthetic) {
                return Ok(emit_always_truthy_const_fold(
                    calls, lhs_type, op, rhs, context,
                ));
            }
            // Predicate-supported (Bool / F64 / String / Option / Primitive /
            // Named-synthetic-union) or NA (Unit / Never / Result / QSelf →
            // truthy/falsy predicate returns None → Tier 2 honest error)。
            emit_logical_compound_predicate_dispatch(
                calls, lhs_type, op, rhs, synthetic, ts_obj, context,
            )
        }
        // Caller (`convert_assign_expr` / `try_convert_*_stmt`) gates on the 3
        // logical compound ops above; other AssignOp variants cannot reach here.
        _ => unreachable!(
            "dispatch_b4_strategy: caller must gate on \
             AssignOp::NullishAssign | AndAssign | OrAssign, got {op:?}"
        ),
    }
}

// =============================================================================
// Strategy emitters
// =============================================================================

/// `??=` ShadowLet emission (LHS = `Option<T>`、cells 38 / 41-d)。
///
/// `{ if <getter>.is_none() { <setter>(Some(rhs)); }; <tail = getter for expr ctx> }`
/// with INV-3 IIFE wrap for SE-having instance receiver。
fn emit_nullish_shadow_let(
    calls: &ReceiverCalls<'_>,
    lhs_type: &RustType,
    rhs: Expr,
    context: LogicalCompoundContext,
) -> Expr {
    let (receiver_for_calls, se_free) = resolve_receiver_for_calls(calls);
    let getter_call = calls.build_getter_call(&receiver_for_calls);
    let setter_arg = wrap_setter_value(rhs, lhs_type);
    let setter_call = calls.build_setter_call(&receiver_for_calls, setter_arg);
    let predicate = Expr::MethodCall {
        object: Box::new(getter_call),
        method: "is_none".to_string(),
        args: vec![],
    };
    let inner_if = Stmt::If {
        condition: predicate,
        then_body: vec![Stmt::Expr(setter_call)],
        else_body: None,
    };
    let tail = if matches!(context, LogicalCompoundContext::Expression) {
        Some(calls.build_getter_call(&receiver_for_calls))
    } else {
        None
    };
    wrap_with_iife_if_needed(calls, se_free, vec![inner_if], tail)
}

/// `??=` Identity emission for non-Option non-Any class member (= TS dead code
/// semantic、no setter call、yield current getter value for expression ctx)。
///
/// Statement context: SE-free → empty Block / SE-having → evaluate-discard
/// `{ <obj>; }` / Static → empty Block (always SE-free)
/// Expression context: SE-free → direct getter call `<obj>.x()` / SE-having →
/// IIFE `{ let __ts_recv = <obj>; __ts_recv.x() }` / Static → `Class::x()`
///
/// INV-3 1-evaluate compliance: SE-having instance receiver evaluated **exactly
/// once** via IIFE Let binding; Static receiver = class TypeName (zero
/// evaluations, no preservation needed)。
fn emit_identity_dispatch(calls: &ReceiverCalls<'_>, context: LogicalCompoundContext) -> Expr {
    let (receiver_for_calls, se_free) = resolve_receiver_for_calls(calls);
    match (context, se_free) {
        (LogicalCompoundContext::Statement, true) => {
            // SE-free + statement: no-op (empty Block)。Static dispatch always
            // takes this branch (Static is statically SE-free per
            // resolve_receiver_for_calls)。Instance with SE-free object also
            // hits this branch。
            Expr::Block(vec![])
        }
        (LogicalCompoundContext::Statement, false) => {
            // SE-having + statement: evaluate-discard receiver (Stmt::Expr
            // contains the receiver expression, side-effect preserved, value
            // discarded)。`se_free == false` invariant ⇒ Instance variant per
            // resolve_receiver_for_calls (Static is statically se_free=true)。
            let object = match calls {
                ReceiverCalls::Instance { object, .. } => *object,
                ReceiverCalls::Static { .. } => unreachable!(
                    "emit_identity_dispatch: Static is statically SE-free per \
                     resolve_receiver_for_calls; SE-having branch (se_free=false) \
                     cannot fire for Static variant"
                ),
            };
            Expr::Block(vec![Stmt::Expr(object.clone())])
        }
        (LogicalCompoundContext::Expression, true) => {
            // SE-free + expression: direct getter call yields current value。
            // Static dispatch takes this branch with `Class::x()` form。
            calls.build_getter_call(&receiver_for_calls)
        }
        (LogicalCompoundContext::Expression, false) => {
            // SE-having + expression: IIFE evaluate-once + yield via __ts_recv。
            // `se_free == false` invariant ⇒ Instance variant (Static is
            // statically se_free=true)。
            let object = match calls {
                ReceiverCalls::Instance { object, .. } => *object,
                ReceiverCalls::Static { .. } => unreachable!(
                    "emit_identity_dispatch: Static is statically SE-free per \
                     resolve_receiver_for_calls; SE-having branch cannot fire"
                ),
            };
            let getter_call = calls.build_getter_call(&receiver_for_calls);
            Expr::Block(vec![
                Stmt::Let {
                    mutable: true,
                    name: TS_RECV_BINDING.to_string(),
                    ty: None,
                    init: Some(object.clone()),
                },
                Stmt::TailExpr(getter_call),
            ])
        }
    }
}

/// `&&=` / `||=` always-truthy const-fold emission (cohesive with
/// `compound_logical_assign.rs::const_fold_always_truthy_stmts`)。
///
/// - `&&=` always-truthy (predicate = true): unconditional setter call。
///   Statement: `<setter>(rhs);` / Expression: `{ <setter>(rhs); <tail = getter> }`
///   with INV-3 IIFE for SE-having instance receiver。
/// - `||=` always-truthy (predicate = false): no-op。
///   Statement: `{}` (or evaluate-discard for SE-having) / Expression:
///   `<getter>` (yield current value) with INV-3 IIFE for SE-having。
fn emit_always_truthy_const_fold(
    calls: &ReceiverCalls<'_>,
    lhs_type: &RustType,
    op: ast::AssignOp,
    rhs: Expr,
    context: LogicalCompoundContext,
) -> Expr {
    let (receiver_for_calls, se_free) = resolve_receiver_for_calls(calls);
    match op {
        ast::AssignOp::AndAssign => {
            // &&= always-truthy: unconditional setter call。
            let setter_arg = wrap_setter_value(rhs, lhs_type);
            let setter_call = calls.build_setter_call(&receiver_for_calls, setter_arg);
            let setter_stmt = Stmt::Expr(setter_call);
            let tail = if matches!(context, LogicalCompoundContext::Expression) {
                Some(calls.build_getter_call(&receiver_for_calls))
            } else {
                None
            };
            wrap_with_iife_if_needed(calls, se_free, vec![setter_stmt], tail)
        }
        ast::AssignOp::OrAssign => {
            // ||= always-truthy: no-op (setter never called, since LHS truthy)。
            // Yields current getter value for expression context。
            emit_identity_dispatch(calls, context)
        }
        _ => unreachable!(
            "emit_always_truthy_const_fold: caller must gate on \
             AssignOp::AndAssign | OrAssign, got {op:?}"
        ),
    }
}

/// `&&=` / `||=` predicate-supported emission (Bool / F64 / String / Option /
/// Primitive / Named-synthetic-union) — conditional setter desugar via
/// existing `truthy_predicate_for_expr` / `falsy_predicate_for_expr`。
#[allow(clippy::too_many_arguments)]
fn emit_logical_compound_predicate_dispatch(
    calls: &ReceiverCalls<'_>,
    lhs_type: &RustType,
    op: ast::AssignOp,
    rhs: Expr,
    synthetic: &SyntheticTypeRegistry,
    ts_obj: &ast::Expr,
    context: LogicalCompoundContext,
) -> Result<Expr> {
    let (receiver_for_calls, se_free) = resolve_receiver_for_calls(calls);
    let getter_call = calls.build_getter_call(&receiver_for_calls);
    let setter_arg = wrap_setter_value(rhs, lhs_type);
    let setter_call = calls.build_setter_call(&receiver_for_calls, setter_arg);
    let mut binder = TempBinder::new();
    let predicate = match op {
        ast::AssignOp::AndAssign => {
            truthy_predicate_for_expr(&getter_call, lhs_type, synthetic, &mut binder)
        }
        ast::AssignOp::OrAssign => {
            falsy_predicate_for_expr(&getter_call, lhs_type, synthetic, &mut binder)
        }
        _ => unreachable!(
            "emit_logical_compound_predicate_dispatch: caller must gate on \
             AssignOp::AndAssign | OrAssign, got {op:?}"
        ),
    };
    let predicate = predicate.ok_or_else(|| {
        // truthy.rs Matrix A.12 NA cells (Unit / Never / Result / QSelf) →
        // None return → Tier 2 honest error。Span = `ts_obj.span()` (Member
        // receiver location) for consistency with sibling Tier 2 errors in
        // `dispatch_b4_strategy` (BlockedByI050 / Any-TypeVar gate / B7
        // inherited / B2 read-only / etc. all use `ts_obj.span()`)。
        let predicate_kind = if matches!(op, ast::AssignOp::AndAssign) {
            "truthy"
        } else {
            "falsy"
        };
        UnsupportedSyntaxError::new(
            format!(
                "logical compound assign on unsupported lhs type \
                 ({predicate_kind} predicate unavailable)"
            ),
            ts_obj.span(),
        )
    })?;
    let inner_if = Stmt::If {
        condition: predicate,
        then_body: vec![Stmt::Expr(setter_call)],
        else_body: None,
    };
    let tail = if matches!(context, LogicalCompoundContext::Expression) {
        Some(calls.build_getter_call(&receiver_for_calls))
    } else {
        None
    };
    Ok(wrap_with_iife_if_needed(
        calls,
        se_free,
        vec![inner_if],
        tail,
    ))
}

// =============================================================================
// Internal helpers
// =============================================================================

/// Resolves the receiver expression IR used inside getter / setter calls,
/// honoring INV-3 1-evaluate compliance for SE-having instance receivers.
///
/// Returns `(receiver_for_calls, se_free)` where:
/// - **`Static` variant**: `receiver_for_calls` is the placeholder Ident
///   (unused for `Class::x()` form FnCall construction; build_getter_call /
///   build_setter_call use the `class_name` field directly via match-dispatch);
///   `se_free` is statically `true` (class TypeName has no side effect)。
/// - **`Instance` variant**:
///     - SE-free object: `receiver_for_calls = object.clone()` (cheap reference
///       copy at the Rust source level, embed twice for getter + setter
///       without semantic change)
///     - SE-having object: `receiver_for_calls = Expr::Ident(TS_RECV_BINDING)`
///       (caller wraps with IIFE Let binding via [`wrap_with_iife_if_needed`])
fn resolve_receiver_for_calls(calls: &ReceiverCalls<'_>) -> (Expr, bool) {
    match calls {
        ReceiverCalls::Static { class_name, .. } => {
            // Static dispatch: receiver_for_calls is unused (build_getter_call /
            // build_setter_call use class_name field directly for FnCall path).
            // Return a placeholder Ident; caller never embeds it in IR.
            (Expr::Ident((*class_name).to_string()), true)
        }
        ReceiverCalls::Instance { object, .. } => {
            let se_free = is_side_effect_free(object);
            let receiver_for_calls = if se_free {
                (*object).clone()
            } else {
                Expr::Ident(TS_RECV_BINDING.to_string())
            };
            (receiver_for_calls, se_free)
        }
    }
}

/// Wraps the inner statements + optional tail in an IIFE Block if the receiver
/// is SE-having (instance with non-pure object); otherwise emits the inner
/// statements + tail directly。
///
/// IIFE shape (SE-having instance):
/// ```text
/// { let mut __ts_recv = <object>; <inner_stmts>; <tail?> }
/// ```
///
/// Direct shape (SE-free instance / Static):
/// ```text
/// { <inner_stmts>; <tail?> }
/// ```
fn wrap_with_iife_if_needed(
    calls: &ReceiverCalls<'_>,
    se_free: bool,
    inner_stmts: Vec<Stmt>,
    tail: Option<Expr>,
) -> Expr {
    if se_free {
        return assemble_block(inner_stmts, tail);
    }
    // SE-having branch: `se_free == false` invariant ⇒ Instance variant per
    // `resolve_receiver_for_calls` (Static is statically se_free=true)。
    // `unreachable!()` codifies the invariant via Rust compiler structural
    // enforcement (= future T11 (11-c) static matrix expansion 等で invariant
    // violation を panic で immediate detect、design-integrity.md 凝集度)。
    let object = match calls {
        ReceiverCalls::Instance { object, .. } => *object,
        ReceiverCalls::Static { .. } => unreachable!(
            "wrap_with_iife_if_needed: Static is statically SE-free per \
             resolve_receiver_for_calls; SE-having branch (se_free=false) \
             cannot fire for Static variant"
        ),
    };
    let mut stmts = Vec::with_capacity(inner_stmts.len() + 2);
    stmts.push(Stmt::Let {
        mutable: true,
        name: TS_RECV_BINDING.to_string(),
        ty: None,
        init: Some(object.clone()),
    });
    stmts.extend(inner_stmts);
    assemble_block(stmts, tail)
}

/// Wraps the RHS value in `Some(_)` when LHS type is `Option<T>` (matches
/// `compound_logical_assign.rs::compound_assign_value` pattern + cell 38 ideal
/// output)。
///
/// For `??=` ShadowLet path the wrap is **always** applied (caller invariant:
/// `lhs_type = Option<T>`). For `&&=`/`||=` predicate-supported / always-truthy
/// const-fold path, the wrap fires only when LHS happens to be `Option<T>` —
/// bool / F64 / String / Named LHS pass the raw `rhs` through。
fn wrap_setter_value(rhs: Expr, lhs_type: &RustType) -> Expr {
    match lhs_type {
        RustType::Option(_) => Expr::FnCall {
            target: CallTarget::BuiltinVariant(BuiltinVariant::Some),
            args: vec![rhs],
        },
        _ => rhs,
    }
}

/// Extracts the Getter return type from a non-empty [`MethodSignature`] slice
/// (= the resolved `obj.x` LHS type for class member access with a getter)。
///
/// Used by [`dispatch_instance_member_logical_compound`] +
/// [`dispatch_static_member_logical_compound`] to compute `lhs_type` without
/// relying on TypeResolver `expr_types[member_span]` (which is not populated
/// for class member getter access — `lookup_field_type` on `TypeRegistry` only
/// checks struct fields, not methods. The Spec gap for TypeResolver's
/// getter-return type registration was predicted by T8 Iteration v12
/// second-review F-SX-1; T9 self-contained extraction from `sigs` avoids the
/// broader TypeResolver-level extension scope)。
///
/// Returns `None` when no Getter sig exists (= B3 setter only / B6 method
/// only) or when the Getter sig lacks `return_type` (= ambient declaration
/// `get x;` without explicit type — rare). Caller treats `None` as Tier 2
/// honest error。
fn extract_getter_return_type(sigs: &[MethodSignature]) -> Option<RustType> {
    sigs.iter()
        .find(|s| s.kind == MethodKind::Getter)
        .and_then(|s| s.return_type.clone())
}

/// Assembles the final block expression with optional tail expression。
///
/// Statement context: `tail = None` → `Expr::Block(stmts)` (no `Stmt::TailExpr`、
/// unit-yielding block)。
/// Expression context: `tail = Some(getter)` → `Expr::Block(stmts +
/// [Stmt::TailExpr(getter)])` yielding the post-assign value。
fn assemble_block(mut stmts: Vec<Stmt>, tail: Option<Expr>) -> Expr {
    if let Some(t) = tail {
        stmts.push(Stmt::TailExpr(t));
    }
    Expr::Block(stmts)
}
