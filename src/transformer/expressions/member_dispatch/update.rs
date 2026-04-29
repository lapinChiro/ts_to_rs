//! Update context dispatch helpers (`obj.x++` / `Class.x--` etc.).
//!
//! Called from `assignments.rs::convert_update_expr` Member arm. I-205 T7 cells 42-45
//! lock-in、Iteration v12 で T8 INV-3 back-port 経由 [`super::shared`] helpers と DRY 統合。

use anyhow::Result;
use swc_common::Spanned;
use swc_ecma_ast as ast;

use crate::ir::{BinOp, Expr, MethodKind, RustType};
use crate::registry::MethodSignature;
use crate::transformer::UnsupportedSyntaxError;

use super::shared::{
    build_instance_setter_desugar_with_iife_wrap, build_static_setter_desugar_block,
    MemberKindFlags,
};

/// Returns `true` if the getter's return type is numeric (`f64` or any integer
/// `Primitive` variant), where TS `++`/`--` operators are runtime-defined and
/// can be desugared to Rust `+ 1.0` / `- 1.0` arithmetic without semantic loss.
///
/// For non-numeric getter return types (`String`, `Bool`, `Vec<T>`, struct, enum,
/// `Any`, etc.), TS `++`/`--` performs runtime `Number(value)` coercion and may
/// yield `NaN`. Rust has no NaN coercion semantic for `String + 1.0` (E0277), so
/// matrix cell 44 (B4 + non-numeric ++) and its `--` symmetric counterpart are
/// reclassified as Tier 2 honest error per Rule 3 (3-3) (NA → Tier 2 reclassify
/// because SWC parser empirical observation in
/// `tests/swc_parser_increment_non_numeric_test.rs` accepts the syntax).
///
/// I-205 T7 invariant: this helper is only called from the B4 (`has_getter &&
/// has_setter`) arm where the getter signature exists; the `Some(_)` extraction
/// failure (= no Getter sig found) cannot occur in that path. For defensive
/// coding it returns `false` (= treat as non-numeric, fire honest error) when
/// the lookup unexpectedly misses, surfacing any future caller-site invariant
/// violation as a Tier 2 honest error rather than a silent setter desugar with
/// wrong semantic.
fn getter_return_is_numeric(sigs: &[MethodSignature]) -> bool {
    sigs.iter()
        .find(|s| s.kind == MethodKind::Getter)
        .and_then(|s| s.return_type.as_ref())
        .map(|ty| matches!(ty, RustType::F64 | RustType::Primitive(_)))
        .unwrap_or(false)
}

/// Operator-specific Tier 2 honest error message for non-numeric UpdateExpr.
///
/// `++` (BinOp::Add) → `"increment of non-numeric (String/etc.) — TS NaN
/// coercion semantic"` (matches cell-44-increment-string-nan.expected fixture
/// content modulo `(Tier 2 honest error)` postfix that the e2e harness adds).
/// `--` (BinOp::Sub) → `"decrement of non-numeric (String/etc.) — TS NaN
/// coercion semantic"` (symmetric counterpart).
fn non_numeric_update_message(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "increment of non-numeric (String/etc.) — TS NaN coercion semantic",
        BinOp::Sub => "decrement of non-numeric (String/etc.) — TS NaN coercion semantic",
        // Caller (`convert_update_expr`) only passes `BinOp::Add` (++) or
        // `BinOp::Sub` (--). Other ops are not produced by `ast::UpdateOp`
        // (UpdateOp = PlusPlus | MinusMinus only); `unreachable!()` codifies
        // this invariant.
        _ => unreachable!(
            "non_numeric_update_message: caller must pass BinOp::Add (++) or BinOp::Sub (--), \
             got {op:?}"
        ),
    }
}

/// Update context dispatch helper for instance access (`obj.x++` / `obj.x--`).
///
/// I-205 T7: dispatches A6 Increment/Decrement on Member target with instance
/// receiver. Unlike [`super::write::dispatch_instance_member_write`] (which fires
/// setter for any has_setter), update dispatch must distinguish B3 (setter only、
/// read of write-only fails because `++/--` requires read first) from B4 (both、
/// setter desugar with numeric type check).
///
/// Dispatch arms (matrix cells 42-45):
/// - `is_inherited = true` (B7) → Tier 2 honest `"write to inherited accessor"`
///   (cells 45-dc and `++` symmetric)
/// - `has_getter && has_setter` (B4) with **numeric getter return type** → setter
///   desugar block via [`build_instance_setter_desugar_with_iife_wrap`] (cells 43,
///   45-c)、INV-3 1-evaluate compliance for side-effect-having receiver (T8
///   Iteration v12 back-port)
/// - `has_getter && has_setter` (B4) with **non-numeric getter return type** →
///   Tier 2 honest `"increment of non-numeric (String/etc.) — TS NaN coercion
///   semantic"` for `++` / `"decrement of non-numeric (String/etc.) — TS NaN
///   coercion semantic"` for `--` (cell 44 and `--` symmetric、Rule 3 (3-3) NA
///   → Tier 2 reclassify)
/// - `has_getter` only (B2) → Tier 2 honest `"write to read-only property"`
///   (cell 45-b and `++` symmetric)
/// - `has_setter` only (B3) → Tier 2 honest `"read of write-only property"`
///   (B3 update with write-only fails because `++/--` reads first)
/// - `has_method` only (B6) → Tier 2 honest `"write to method"` (cells 45-db
///   and `++` symmetric)
///
/// Cells 42 (B1 field) / 45-a (B1 field --) / 45-de (B9 unknown --) do **not**
/// reach this helper — they take the `MemberReceiverClassification::Fallback`
/// path in [`crate::transformer::Transformer::convert_update_expr`] which builds
/// a direct `FieldAccess` postfix/prefix block (regression Tier 2 → Tier 1
/// transition).
pub(crate) fn dispatch_instance_member_update(
    object: &Expr,
    field: &str,
    sigs: &[MethodSignature],
    is_inherited: bool,
    op: BinOp,
    is_postfix: bool,
    ts_obj: &ast::Expr,
) -> Result<Expr> {
    if is_inherited {
        return Err(
            UnsupportedSyntaxError::new("write to inherited accessor", ts_obj.span()).into(),
        );
    }
    let kinds = MemberKindFlags::from_sigs(sigs);
    if kinds.has_getter && kinds.has_setter {
        // B4: setter desugar with numeric type gate + INV-3 1-evaluate compliance
        // (T8 Iteration v12 で T7 latent gap を IIFE wrap で structural fix back-port、
        // shared `build_instance_setter_desugar_with_iife_wrap` 経由で T8 compound と DRY 統合)
        if !getter_return_is_numeric(sigs) {
            return Err(
                UnsupportedSyntaxError::new(non_numeric_update_message(op), ts_obj.span()).into(),
            );
        }
        return Ok(build_instance_setter_desugar_with_iife_wrap(
            object,
            field,
            op,
            Expr::NumberLit(1.0),
            /* yield_old = */ is_postfix,
        ));
    }
    if kinds.has_getter {
        // B2 getter only: write attempt fails (++/-- requires write of new value)
        return Err(
            UnsupportedSyntaxError::new("write to read-only property", ts_obj.span()).into(),
        );
    }
    if kinds.has_setter {
        // B3 setter only: read attempt fails (++/-- requires read first)
        return Err(
            UnsupportedSyntaxError::new("read of write-only property", ts_obj.span()).into(),
        );
    }
    if kinds.has_method {
        // B6 method only
        return Err(UnsupportedSyntaxError::new("write to method", ts_obj.span()).into());
    }
    unreachable!(
        "dispatch_instance_member_update: sigs is non-empty (lookup_method_sigs_in_inheritance_chain \
         never returns Some(empty vec)) and MethodKind is exhaustive (Method/Getter/Setter), \
         so one of the 4 if-blocks above must fire. field={field}"
    );
}

/// Update context dispatch helper for static access (`Class.x++` / `Class.x--`).
///
/// I-205 T7: dispatches A6 Increment/Decrement on Member target with static
/// (class TypeName) receiver. Symmetric to [`dispatch_instance_member_update`]
/// with `Class::method`-form FnCall emit instead of `obj.method`-form
/// MethodCall emit (= [`build_static_setter_desugar_block`] 経由)。
///
/// Dispatch arms (matrix cell 45-dd is primary、A6 `++` static symmetric is the
/// `op = BinOp::Add` mirror; static B2/B3/B6/B7 are matrix cell 化なし but
/// covered defensively per the same `dispatch_static_member_write` pattern from
/// T6 Iteration v9 deep deep review).
pub(crate) fn dispatch_static_member_update(
    class_name: &str,
    field: &str,
    sigs: &[MethodSignature],
    is_inherited: bool,
    op: BinOp,
    is_postfix: bool,
    ts_obj: &ast::Expr,
) -> Result<Expr> {
    if is_inherited {
        return Err(UnsupportedSyntaxError::new(
            "write to inherited static accessor",
            ts_obj.span(),
        )
        .into());
    }
    let kinds = MemberKindFlags::from_sigs(sigs);
    if kinds.has_getter && kinds.has_setter {
        if !getter_return_is_numeric(sigs) {
            return Err(
                UnsupportedSyntaxError::new(non_numeric_update_message(op), ts_obj.span()).into(),
            );
        }
        // Static dispatch では receiver = class TypeName で side-effect なし (= class
        // path access、Rust associated fn の `Class::xxx` form)、INV-3 IIFE wrap 不要。
        return Ok(build_static_setter_desugar_block(
            class_name,
            field,
            op,
            Expr::NumberLit(1.0),
            /* yield_old = */ is_postfix,
        ));
    }
    if kinds.has_getter {
        return Err(UnsupportedSyntaxError::new(
            "write to read-only static property",
            ts_obj.span(),
        )
        .into());
    }
    if kinds.has_setter {
        return Err(UnsupportedSyntaxError::new(
            "read of write-only static property",
            ts_obj.span(),
        )
        .into());
    }
    if kinds.has_method {
        return Err(UnsupportedSyntaxError::new("write to static method", ts_obj.span()).into());
    }
    unreachable!(
        "dispatch_static_member_update: sigs is non-empty (lookup_method_sigs_in_inheritance_chain \
         never returns Some(empty vec)) and MethodKind is exhaustive (Method/Getter/Setter), \
         so one of the 4 if-blocks above must fire. class={class_name}, field={field}"
    );
}
