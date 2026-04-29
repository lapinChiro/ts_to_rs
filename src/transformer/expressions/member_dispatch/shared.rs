//! Shared infrastructure for class member dispatch helpers.
//!
//! Houses cross-cutting structures and helpers consumed by every dispatch context
//! (Read / Write / Update / Compound × Instance / Static):
//!
//! - [`MemberKindFlags`]: 3-flag (`has_getter`/`has_setter`/`has_method`) classification
//!   computed once from a non-empty `[MethodSignature]` slice. Eliminates the original
//!   3-line `let has_X = sigs.iter()...` duplication across 4 dispatch helpers found
//!   in Iteration v10 second-review (= 8 dispatch helpers post-T7/T8).
//! - [`is_side_effect_free`]: Decides whether the receiver IR can be embedded twice in
//!   emitted Rust source (Ident / depth-bounded FieldAccess of Ident → true) or must
//!   be hoisted into an IIFE binding for INV-3 1-evaluate compliance (everything else
//!   conservatively → false).
//! - [`wrap_with_recv_binding`]: IIFE wrapper that prepends `let mut __ts_recv = <receiver>;`
//!   to a setter-desugar inner block, so the receiver is evaluated exactly once
//!   (INV-3 (a) Property statement compliance).
//! - [`build_setter_desugar_block`]: Generalized setter desugar block builder shared by
//!   Update (`obj.x++` / `Class.x--`) and Compound assign (`obj.x += v` / `Class.x -= v`)
//!   helpers. Postfix update yields old value (`yield_old=true`); prefix update + compound
//!   assign yield new value (`yield_old=false`、structurally identical after operator/rhs
//!   unification).
//! - [`build_instance_setter_desugar_with_iife_wrap`]: DRY-extracted from T7
//!   `dispatch_instance_member_update` + T8 `dispatch_instance_member_compound` (Iteration
//!   v12 third-review)。両 helper の B4 setter desugar arm が完全 identical だった
//!   IIFE wrap + getter/setter call construction を集約、本 helper 経由で 2 site の
//!   ~30 行 boilerplate を ~3 行に圧縮。
//! - [`build_static_setter_desugar_block`]: DRY-extracted from T7
//!   `dispatch_static_member_update` + T8 `dispatch_static_member_compound` (Iteration
//!   v12 third-review)。Static dispatch は receiver = class TypeName で side-effect
//!   なし (= IIFE wrap 不要)、本 helper は `Class::xxx`/`Class::set_xxx` form FnCall
//!   構築 + `build_setter_desugar_block` 呼出を集約。
//!
//! All structural-invariant unreachable!() macro placements live in the per-context
//! dispatch helper files (read/write/update/compound.rs) since the panic message
//! identifies the specific dispatch helper for failure mode debugging.

use crate::ir::{BinOp, CallTarget, Expr, MethodKind, Stmt, UserTypeRef};
use crate::registry::MethodSignature;

use super::super::{TS_NEW_BINDING, TS_OLD_BINDING, TS_RECV_BINDING};

// =============================================================================
// MemberKindFlags
// =============================================================================

/// Member-kind flags computed once from a non-empty `[MethodSignature]` slice.
///
/// `MethodKind` is a 3-variant exhaustive enum (`Method`/`Getter`/`Setter`); the
/// [`crate::registry::TypeRegistry::lookup_method_sigs_in_inheritance_chain`] invariant
/// (returns `Some(non-empty Vec)` or `None`) guarantees that at least one flag is `true`
/// whenever `MemberKindFlags` is constructed from a `Some(_)` lookup result. Each dispatch
/// arm consumes the flags in a fixed precedence order (= explicitly enumerated by the
/// caller per Read/Write/Update/Compound context semantic), and the trailing
/// `unreachable!()` macro at each call site codifies the structural invariant.
///
/// Centralizing the flag computation eliminates the 3-line `let has_X = sigs.iter()...`
/// pattern × 8 dispatch helpers (= Read/Write/Update/Compound × Instance/Static) DRY
/// violation found in Iteration v10 second-review (extended to T7/T8 helpers in
/// Iteration v12).
pub(super) struct MemberKindFlags {
    pub(super) has_getter: bool,
    pub(super) has_setter: bool,
    pub(super) has_method: bool,
}

impl MemberKindFlags {
    pub(super) fn from_sigs(sigs: &[MethodSignature]) -> Self {
        Self {
            has_getter: sigs.iter().any(|s| s.kind == MethodKind::Getter),
            has_setter: sigs.iter().any(|s| s.kind == MethodKind::Setter),
            has_method: sigs.iter().any(|s| s.kind == MethodKind::Method),
        }
    }
}

// =============================================================================
// is_side_effect_free — INV-3 1-evaluate compliance receiver judgment
// =============================================================================

/// Returns `true` when `expr` can be embedded twice in emitted Rust source
/// without altering observable side effects. Used by `dispatch_instance_member_*`
/// helpers (T7 update + T8 compound) to decide whether to embed the receiver
/// directly (Ident / FieldAccess of Ident / etc.、cheap reference-copy at the
/// Rust source level) or wrap the setter desugar block with an IIFE binding
/// `let mut __ts_recv = <receiver>;` to enforce INV-3 1-evaluate compliance.
///
/// Decidably side-effect-free shapes (truthy):
/// - [`Expr::Ident`] — local variable / `self` / parameter (no eval cost in Rust)
/// - [`Expr::FieldAccess`] — recursive on `object` (e.g., `a.b.c` is SE-free
///   iff `a.b` is SE-free)
///
/// All other shapes (`FnCall` / `MethodCall` / `BinaryOp` / `Block` / `Closure`
/// / etc.) are conservatively treated as side-effect-bearing — even pure
/// expressions (`1 + 2`) take this path because the conservative IIFE wrap is
/// always semantically safe (just an extra binding) while the inline path is
/// only safe when source-level duplication can't change observable behavior.
///
/// `Expr::This` is not handled because the Transformer converts `ast::Expr::This`
/// to `Expr::Ident("self")` (= IR-level `this` representation)、which the `Ident`
/// arm already covers.
pub(super) fn is_side_effect_free(expr: &Expr) -> bool {
    match expr {
        Expr::Ident(_) => true,
        Expr::FieldAccess { object, .. } => is_side_effect_free(object),
        _ => false,
    }
}

// =============================================================================
// wrap_with_recv_binding — IIFE wrapper for INV-3 compliance
// =============================================================================

/// Wraps a setter-desugar inner `Expr::Block` in an IIFE binding
/// `let mut __ts_recv = <receiver>;` so the receiver is evaluated **exactly
/// once** per INV-3 (a) Property statement.
///
/// Input (inner Block from [`build_setter_desugar_block`]):
/// ```text
/// { let __ts_old/new = <getter via __ts_recv>; <setter via __ts_recv>; __ts_old/new }
/// ```
///
/// Output:
/// ```text
/// {
///   let mut __ts_recv = <receiver>;
///   let __ts_old/new = <getter via __ts_recv>;
///   <setter via __ts_recv>;
///   __ts_old/new
/// }
/// ```
///
/// `mutable: true` is conservative — the binding may not strictly require `mut`
/// when the receiver is already `&mut T`, but Rust's `unused_mut` warning is
/// not catastrophic for the rare cases where it fires (clippy lint, not a
/// compile error). Setter dispatch (`obj.set_x(...)`) requires `&mut self`
/// auto-borrow which works identically for `let mut __ts_recv = T` (owned) and
/// `let __ts_recv = &mut T` (borrow); the `mut` keyword on the binding allows
/// the auto-borrow path to fire for owned receivers without divergent emit
/// strategy per receiver type.
pub(super) fn wrap_with_recv_binding(receiver: Expr, inner_block: Expr) -> Expr {
    let inner_stmts = match inner_block {
        Expr::Block(stmts) => stmts,
        // `build_setter_desugar_block` is the only caller of this function,
        // and it always returns `Expr::Block(_)`. The unreachable here codifies
        // that contract so a future refactor that breaks it surfaces as a
        // structural panic rather than a silent IR shape divergence.
        other => unreachable!(
            "wrap_with_recv_binding: inner must be Expr::Block (built by \
             build_setter_desugar_block)、got {other:?}"
        ),
    };
    let mut combined = Vec::with_capacity(inner_stmts.len() + 1);
    combined.push(Stmt::Let {
        mutable: true,
        name: TS_RECV_BINDING.to_string(),
        ty: None,
        init: Some(receiver),
    });
    combined.extend(inner_stmts);
    Expr::Block(combined)
}

// =============================================================================
// build_setter_desugar_block — shared block shape for Update + Compound dispatch
// =============================================================================

/// Builds the setter desugar block expression for both UpdateExpr (`++` / `--`)
/// and arithmetic / bitwise compound assign (`+= -= *= /= ... |=`) on a class
/// member with both getter and setter (B4 instance, B8 static getter+setter).
///
/// Postfix update only (`obj.x++` / `obj.x--`、`yield_old = true`、old value yielded):
/// ```text
/// { let __ts_old = <getter_call>; <setter_emit(__ts_old <op> rhs)>; __ts_old }
/// ```
///
/// Prefix update + Compound assign (`++obj.x` / `obj.x += v`、`yield_old = false`、
/// new value yielded):
/// ```text
/// { let __ts_new = <getter_call> <op> rhs; <setter_emit(__ts_new)>; __ts_new }
/// ```
///
/// `rhs` is the right-hand operand: `Expr::NumberLit(1.0)` for UpdateExpr (T7)、
/// arbitrary IR `Expr` for compound assign (T8). Generalized from the T7-specific
/// `build_update_setter_block` (Iteration v11) at T8 (Iteration v12) to share the
/// emission shape between update and compound assign — both yield the new value
/// in non-postfix-update modes (= prefix update and compound assign are
/// structurally identical after operator/rhs unification).
///
/// `setter_emit_for_arg` is a closure that takes the setter argument
/// (`__ts_old <op> rhs` for postfix update, or `__ts_new` for prefix update /
/// compound assign) and returns the setter call IR (`Expr::MethodCall` for
/// instance, `Expr::FnCall` for static). This abstraction shares the block shape
/// between instance and static dispatch while keeping the call IR specific to
/// each context.
///
/// Variable names use the `__ts_` prefix per [I-154 namespace reservation
/// rule](crate::transformer::statements) so user code containing identifiers
/// like `_old` / `_new` cannot shadow or collide with this emission. (T7
/// extends the `__ts_` namespace from labels (I-154) to value bindings,
/// T8 extends it further to receiver IIFE binding via [`TS_RECV_BINDING`].)
///
/// INV-3 1-evaluate compliance is **not** handled inside this helper — the
/// caller (`build_instance_setter_desugar_with_iife_wrap` / dispatch helpers)
/// decides whether to call this helper directly (side-effect-free receiver,
/// receiver embedded twice via [`is_side_effect_free`] = `true`) or wrap the
/// result with [`wrap_with_recv_binding`] for IIFE form (side-effect-having
/// receiver). This separation keeps the block-shape concern (this helper) and
/// the receiver-evaluation-count concern (caller + IIFE wrapper) orthogonal.
pub(super) fn build_setter_desugar_block(
    getter_call: Expr,
    op: BinOp,
    rhs: Expr,
    yield_old: bool,
    setter_emit_for_arg: impl FnOnce(Expr) -> Expr,
) -> Expr {
    let (binding_name, binding_init, setter_arg) = if yield_old {
        (
            TS_OLD_BINDING,
            getter_call,
            Expr::BinaryOp {
                left: Box::new(Expr::Ident(TS_OLD_BINDING.to_string())),
                op,
                right: Box::new(rhs),
            },
        )
    } else {
        (
            TS_NEW_BINDING,
            Expr::BinaryOp {
                left: Box::new(getter_call),
                op,
                right: Box::new(rhs),
            },
            Expr::Ident(TS_NEW_BINDING.to_string()),
        )
    };
    let setter_call = setter_emit_for_arg(setter_arg);
    Expr::Block(vec![
        Stmt::Let {
            mutable: false,
            name: binding_name.to_string(),
            ty: None,
            init: Some(binding_init),
        },
        Stmt::Expr(setter_call),
        Stmt::TailExpr(Expr::Ident(binding_name.to_string())),
    ])
}

// =============================================================================
// build_instance_setter_desugar_with_iife_wrap — DRY-extracted instance setter desugar
// =============================================================================

/// Composes [`build_setter_desugar_block`] with INV-3 1-evaluate compliance for an
/// **instance** receiver, used by both T7 update (`obj.x++`) and T8 compound assign
/// (`obj.x += v`) instance dispatch helpers.
///
/// DRY rationale (Iteration v12 third-review、`design-integrity.md` "DRY"): pre-extract、
/// `dispatch_instance_member_update` (T7) と `dispatch_instance_member_compound` (T8)
/// が完全 identical な以下の logic を 30 行 × 2 helper = 60 行で重複していた:
/// 1. `is_side_effect_free(object)` 判定で receiver IR 経路を分岐
/// 2. side-effect-free → `object.clone()` を receiver として直接 embed
/// 3. side-effect-having → `Expr::Ident(TS_RECV_BINDING)` を placeholder receiver として使用
/// 4. `getter_call = obj.x()` (MethodCall) を construct
/// 5. setter closure `arg → obj.set_x(arg)` を construct
/// 6. [`build_setter_desugar_block`] で block 構築
/// 7. side-effect-having なら [`wrap_with_recv_binding`] で IIFE wrap、SE-free なら inner 直接 return
///
/// Post-extract、両 helper の呼出は 3 行に圧縮 (`return Ok(build_instance_setter_desugar_with_iife_wrap(object, field, op, rhs, yield_old));`)。
///
/// 本 helper の存在により、subsequent T9 (logical compound assign setter dispatch:
/// `obj.x ??= d` / `obj.x &&= v` / `obj.x ||= v`) が compound 同様の IIFE wrap を
/// leverage する場合に 3 つ目の caller として加わるだけで、DRY violation 増殖を防止。
pub(super) fn build_instance_setter_desugar_with_iife_wrap(
    object: &Expr,
    field: &str,
    op: BinOp,
    rhs: Expr,
    yield_old: bool,
) -> Expr {
    // Cache the side-effect judgment once: branch decisions for both the
    // receiver-for-calls and the IIFE wrap fire on the same `object` value,
    // so re-evaluating `is_side_effect_free` would duplicate identical work
    // (T8 Iteration v12 review F1 fix、`design-integrity.md` "DRY" applied to
    // helper-internal expressions).
    let se_free = is_side_effect_free(object);
    let receiver_for_calls = if se_free {
        object.clone()
    } else {
        Expr::Ident(TS_RECV_BINDING.to_string())
    };
    let getter_call = Expr::MethodCall {
        object: Box::new(receiver_for_calls.clone()),
        method: field.to_string(),
        args: vec![],
    };
    let setter_method = format!("set_{field}");
    let receiver_for_setter = receiver_for_calls;
    let inner = build_setter_desugar_block(getter_call, op, rhs, yield_old, move |arg| {
        Expr::MethodCall {
            object: Box::new(receiver_for_setter),
            method: setter_method,
            args: vec![arg],
        }
    });
    if se_free {
        inner
    } else {
        wrap_with_recv_binding(object.clone(), inner)
    }
}

// =============================================================================
// build_static_setter_desugar_block — DRY-extracted static setter desugar
// =============================================================================

/// Composes [`build_setter_desugar_block`] for a **static** (class TypeName) receiver,
/// used by both T7 update (`Class.x++`) and T8 compound assign (`Class.x += v`) static
/// dispatch helpers.
///
/// Static dispatch では receiver = class TypeName (= Rust associated path access、
/// `Class::xxx`-form FnCall) で side-effect なし、INV-3 IIFE wrap 不要 (= caller-side
/// side-effect-free 判定なし、unconditional inline emit)。
///
/// DRY rationale (Iteration v12 third-review): T7 `dispatch_static_member_update` と
/// T8 `dispatch_static_member_compound` の B8 setter desugar arm が完全 identical な
/// `getter_call FnCall` 構築 + setter closure + `build_setter_desugar_block` 呼出 を
/// ~10 行 × 2 helper = 20 行で重複していた。Post-extract、両 helper の呼出は 1 行に
/// 圧縮 (`return Ok(build_static_setter_desugar_block(class_name, field, op, rhs, yield_old));`)。
pub(super) fn build_static_setter_desugar_block(
    class_name: &str,
    field: &str,
    op: BinOp,
    rhs: Expr,
    yield_old: bool,
) -> Expr {
    let getter_call = Expr::FnCall {
        target: CallTarget::UserAssocFn {
            ty: UserTypeRef::new(class_name),
            method: field.to_string(),
        },
        args: vec![],
    };
    let class_name_for_setter = class_name.to_string();
    let setter_method = format!("set_{field}");
    build_setter_desugar_block(getter_call, op, rhs, yield_old, move |arg| Expr::FnCall {
        target: CallTarget::UserAssocFn {
            ty: UserTypeRef::new(&class_name_for_setter),
            method: setter_method,
        },
        args: vec![arg],
    })
}
