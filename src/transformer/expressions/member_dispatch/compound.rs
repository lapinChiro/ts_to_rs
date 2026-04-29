//! Compound assign context dispatch helpers (`obj.x += v` / `Class.x -= v` etc.).
//!
//! Called from the [`super::Transformer::dispatch_member_compound`] entry method which
//! routes `convert_assign_expr`'s arithmetic / bitwise compound (`+= -= *= ... |=`、
//! 11 ops collectively) × Member × non-Computed gate. I-205 T8 cells 21-29 + 30-35
//! lock-in、Iteration v12 で [`super::shared`] helpers と DRY 統合。

use anyhow::Result;
use swc_common::Spanned;
use swc_ecma_ast as ast;

use crate::ir::{BinOp, Expr};
use crate::registry::MethodSignature;
use crate::transformer::UnsupportedSyntaxError;

use super::shared::{
    build_instance_setter_desugar_with_iife_wrap, build_static_setter_desugar_block,
    MemberKindFlags,
};

/// Compound assign dispatch helper for instance access (`obj.x += v` / `obj.x -= v`
/// / `obj.x |= v` / etc.、arithmetic + bitwise compound 11 ops collectively).
///
/// I-205 T8: dispatches A3 / A4 compound ops on Member target with instance
/// receiver. Symmetric to [`super::update::dispatch_instance_member_update`] (T7) —
/// both use the shared [`build_instance_setter_desugar_with_iife_wrap`] for IR shape
/// (yield_new for compound assign + side-effect-having receiver IIFE)、differing
/// only in the `rhs` parameter source (NumberLit(1.0) for update vs. arbitrary
/// `Expr` for compound) and the `yield_old` flag (postfix update only = true、
/// prefix update + compound assign = false).
///
/// Dispatch arms (matrix cells 21-29, 30-35-c via op-axis orthogonality merge):
/// - `is_inherited = true` (B7) → Tier 2 honest `"compound assign to inherited
///   accessor"` (cell 26 + ops symmetric)
/// - `has_getter && has_setter` (B4) → setter desugar block (cell 21 + op variants)、
///   IIFE wrapped if receiver carries side effects per [`super::shared::is_side_effect_free`]
/// - `has_getter` only (B2) → Tier 2 honest `"compound assign to read-only
///   property"` (cell 22 + ops symmetric)
/// - `has_setter` only (B3) → Tier 2 honest `"compound assign read of write-only
///   property"` (cell 23 + ops symmetric、compound は read 先行で getter 不在で
///   read fail)
/// - `has_method` only (B6) → Tier 2 honest `"compound assign to method"`
///   (cell 25 + ops symmetric)
///
/// Cells 20 (B1 field) / 28 (B9 unknown) do **not** reach this helper — they
/// take the `MemberReceiverClassification::Fallback` path in
/// [`super::Transformer::dispatch_member_compound`] which emits the legacy
/// `Expr::Assign { target: FieldAccess, value: BinaryOp }` (regression preserve).
pub(super) fn dispatch_instance_member_compound(
    object: &Expr,
    field: &str,
    sigs: &[MethodSignature],
    is_inherited: bool,
    op: BinOp,
    rhs: Expr,
    ts_obj: &ast::Expr,
) -> Result<Expr> {
    if is_inherited {
        return Err(UnsupportedSyntaxError::new(
            "compound assign to inherited accessor",
            ts_obj.span(),
        )
        .into());
    }
    let kinds = MemberKindFlags::from_sigs(sigs);
    if kinds.has_getter && kinds.has_setter {
        // B4: setter desugar with INV-3 1-evaluate compliance (shared with T7 update
        // via `build_instance_setter_desugar_with_iife_wrap`、Iteration v12 DRY 統合)。
        return Ok(build_instance_setter_desugar_with_iife_wrap(
            object, field, op, rhs, /* yield_old = */ false,
        ));
    }
    if kinds.has_getter {
        return Err(UnsupportedSyntaxError::new(
            "compound assign to read-only property",
            ts_obj.span(),
        )
        .into());
    }
    if kinds.has_setter {
        return Err(UnsupportedSyntaxError::new(
            "compound assign read of write-only property",
            ts_obj.span(),
        )
        .into());
    }
    if kinds.has_method {
        return Err(UnsupportedSyntaxError::new("compound assign to method", ts_obj.span()).into());
    }
    unreachable!(
        "dispatch_instance_member_compound: sigs is non-empty (lookup_method_sigs_in_inheritance_chain \
         never returns Some(empty vec)) and MethodKind is exhaustive (Method/Getter/Setter), \
         so one of the 4 if-blocks above must fire. field={field}"
    );
}

/// Compound assign dispatch helper for static access (`Class.x += v` /
/// `Class.x -= v` / etc.).
///
/// I-205 T8: dispatches A3 / A4 compound ops on Member target with static
/// (class TypeName) receiver. Symmetric to [`dispatch_instance_member_compound`]
/// with `Class::method`-form FnCall emit instead of `obj.method`-form
/// MethodCall emit (= [`build_static_setter_desugar_block`] 経由)、and **without**
/// the IIFE wrap (= class TypeName receiver is a Rust path access、side-effect なし、
/// receiver evaluation count is statically zero).
///
/// Dispatch arms (matrix cells 27, 29-e-d, 35-d via op-axis orthogonality merge):
/// - `is_inherited = true` (defensive、static B7) → Tier 2 honest `"compound
///   assign to inherited static accessor"` (matrix cell 化なし、subsequent T11
///   (11-c) で expansion)
/// - `has_getter && has_setter` (B8) → static setter desugar block (cell 27 +
///   op variants)
/// - `has_getter` only (defensive、static B2) → Tier 2 honest `"compound assign
///   to read-only static property"` (matrix cell 化なし)
/// - `has_setter` only (defensive、static B3) → Tier 2 honest `"compound assign
///   read of write-only static property"` (matrix cell 化なし)
/// - `has_method` only (defensive、static B6) → Tier 2 honest `"compound assign
///   to static method"` (matrix cell 化なし)
pub(super) fn dispatch_static_member_compound(
    class_name: &str,
    field: &str,
    sigs: &[MethodSignature],
    is_inherited: bool,
    op: BinOp,
    rhs: Expr,
    ts_obj: &ast::Expr,
) -> Result<Expr> {
    if is_inherited {
        return Err(UnsupportedSyntaxError::new(
            "compound assign to inherited static accessor",
            ts_obj.span(),
        )
        .into());
    }
    let kinds = MemberKindFlags::from_sigs(sigs);
    if kinds.has_getter && kinds.has_setter {
        return Ok(build_static_setter_desugar_block(
            class_name, field, op, rhs, /* yield_old = */ false,
        ));
    }
    if kinds.has_getter {
        return Err(UnsupportedSyntaxError::new(
            "compound assign to read-only static property",
            ts_obj.span(),
        )
        .into());
    }
    if kinds.has_setter {
        return Err(UnsupportedSyntaxError::new(
            "compound assign read of write-only static property",
            ts_obj.span(),
        )
        .into());
    }
    if kinds.has_method {
        return Err(
            UnsupportedSyntaxError::new("compound assign to static method", ts_obj.span()).into(),
        );
    }
    unreachable!(
        "dispatch_static_member_compound: sigs is non-empty (lookup_method_sigs_in_inheritance_chain \
         never returns Some(empty vec)) and MethodKind is exhaustive (Method/Getter/Setter), \
         so one of the 4 if-blocks above must fire. class={class_name}, field={field}"
    );
}
