//! Read context dispatch helpers (`obj.x` / `Class.x` reads).
//!
//! Called from `member_access.rs::resolve_member_access` after Read-only special cases
//! (Enum variant, `Math.PI`, `.length`). I-205 T5 cells 1-10 lock-in。

use anyhow::Result;
use swc_common::Spanned;
use swc_ecma_ast as ast;

use crate::ir::{CallTarget, Expr, UserTypeRef};
use crate::registry::MethodSignature;
use crate::transformer::UnsupportedSyntaxError;

use super::shared::MemberKindFlags;

/// Read context dispatch helper for instance access (`obj.x`).
///
/// I-205 T5: B2 / B3 / B4 / B6 / B7 dispatch arms。`is_inherited = true` (B7) の場合、
/// architectural concern が "Class inheritance dispatch" (別 PRD I-206) と orthogonal
/// のため Tier 2 honest error reclassify。
///
/// I-205 T6 Iteration v10 review fix (Layer 1 Defect 2 = T5/T6 structural enforcement
/// asymmetry): pre-fix の最終 arm = `Ok(Expr::FieldAccess { object, field })` は
/// `lookup_method_sigs_in_inheritance_chain` non-empty vec invariant + `MethodKind`
/// 3-variant exhaustive (Method/Getter/Setter) により構造的 unreachable な dead code、
/// `dispatch_static_member_read` (Iteration v9 deep deep review で `unreachable!()` 化済) と
/// **asymmetric**。本 v10 review で symmetry restored、`unreachable!()` macro で structural
/// invariant codified (= Read context instance/static + Write context instance/static の
/// 4 helper 全てが `unreachable!()` で symmetric structural enforcement 統一)。
pub(crate) fn dispatch_instance_member_read(
    object: &Expr,
    field: &str,
    sigs: &[MethodSignature],
    is_inherited: bool,
    ts_obj: &ast::Expr,
) -> Result<Expr> {
    if is_inherited {
        return Err(UnsupportedSyntaxError::new(
            "inherited accessor access (Rust struct inheritance not directly supported)",
            ts_obj.span(),
        )
        .into());
    }
    let kinds = MemberKindFlags::from_sigs(sigs);
    if kinds.has_getter {
        return Ok(Expr::MethodCall {
            object: Box::new(object.clone()),
            method: field.to_string(),
            args: vec![],
        });
    }
    if kinds.has_setter {
        return Err(
            UnsupportedSyntaxError::new("read of write-only property", ts_obj.span()).into(),
        );
    }
    if kinds.has_method {
        return Err(UnsupportedSyntaxError::new(
            "method-as-fn-reference (no-paren)",
            ts_obj.span(),
        )
        .into());
    }
    unreachable!(
        "dispatch_instance_member_read: sigs is non-empty (lookup_method_sigs_in_inheritance_chain \
         never returns Some(empty vec)) and MethodKind is exhaustive (Method/Getter/Setter), \
         so one of the 3 if-blocks above must fire. field={field}"
    );
}

/// Read context dispatch helper for static access (`Class.x`).
///
/// I-205 T5: B8 cell。`Class::field()` を associated fn call として emit (Getter)。
/// Class の static method は parent class からの inheritance を持たない (TS の static
/// member は prototype chain inheritance するが Rust associated fn は構造的に inherited
/// dispatch を持たない、本 PRD scope は class direct のみ = B7 inherited は別 PRD I-206)。
pub(crate) fn dispatch_static_member_read(
    class_name: &str,
    field: &str,
    sigs: &[MethodSignature],
    is_inherited: bool,
    ts_obj: &ast::Expr,
) -> Result<Expr> {
    if is_inherited {
        return Err(UnsupportedSyntaxError::new(
            "inherited static accessor access (Rust associated fn does not chain inheritance)",
            ts_obj.span(),
        )
        .into());
    }
    let kinds = MemberKindFlags::from_sigs(sigs);
    if kinds.has_getter {
        return Ok(Expr::FnCall {
            target: CallTarget::UserAssocFn {
                ty: UserTypeRef::new(class_name),
                method: field.to_string(),
            },
            args: vec![],
        });
    }
    if kinds.has_setter {
        return Err(UnsupportedSyntaxError::new(
            "read of write-only static property",
            ts_obj.span(),
        )
        .into());
    }
    if kinds.has_method {
        return Err(UnsupportedSyntaxError::new(
            "static-method-as-fn-reference (no-paren)",
            ts_obj.span(),
        )
        .into());
    }
    // I-205 T5 Iteration v9 deep deep review で本 arm を `unreachable!()` 化、Static field
    // access (`Class.staticField`、matrix cell 化なし) は本 dispatch を経由せず
    // `resolve_member_access` の最終 fallback (5. FieldAccess) 経由で emit (subsequent T11
    // (11-d) で associated const path access に修正予定)。
    unreachable!(
        "dispatch_static_member_read: sigs is non-empty (lookup_method_sigs_in_inheritance_chain \
         never returns Some(empty vec)) and MethodKind is exhaustive (Method/Getter/Setter), \
         so one of the 3 if-blocks above must fire. class={class_name}, field={field}"
    );
}
