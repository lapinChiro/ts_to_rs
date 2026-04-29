//! Write context dispatch helpers (`obj.x = v` / `Class.x = v`).
//!
//! Called from the [`super::Transformer::dispatch_member_write`] entry method which routes
//! `convert_assign_expr`'s plain `=` × Member × non-Computed gate. I-205 T6 cells 11-19
//! lock-in。

use anyhow::Result;
use swc_common::Spanned;
use swc_ecma_ast as ast;

use crate::ir::{CallTarget, Expr, UserTypeRef};
use crate::registry::MethodSignature;
use crate::transformer::UnsupportedSyntaxError;

use super::shared::MemberKindFlags;

/// Write context dispatch helper for instance access (`obj.x = v`).
///
/// I-205 T6: B2 / B3 / B4 / B6 / B7 dispatch arms。`is_inherited = true` (B7) は
/// architectural concern = "Class inheritance dispatch" (別 PRD I-206) と orthogonal
/// のため Tier 2 honest error reclassify (= Read context cell 8 と symmetric)。
pub(super) fn dispatch_instance_member_write(
    object: &Expr,
    field: &str,
    sigs: &[MethodSignature],
    is_inherited: bool,
    value: Expr,
    ts_obj: &ast::Expr,
) -> Result<Expr> {
    if is_inherited {
        return Err(
            UnsupportedSyntaxError::new("write to inherited accessor", ts_obj.span()).into(),
        );
    }
    let kinds = MemberKindFlags::from_sigs(sigs);
    if kinds.has_setter {
        return Ok(Expr::MethodCall {
            object: Box::new(object.clone()),
            method: format!("set_{field}"),
            args: vec![value],
        });
    }
    if kinds.has_getter {
        return Err(
            UnsupportedSyntaxError::new("write to read-only property", ts_obj.span()).into(),
        );
    }
    if kinds.has_method {
        return Err(UnsupportedSyntaxError::new("write to method", ts_obj.span()).into());
    }
    unreachable!(
        "dispatch_instance_member_write: sigs is non-empty (lookup_method_sigs_in_inheritance_chain \
         never returns Some(empty vec)) and MethodKind is exhaustive (Method/Getter/Setter), \
         so one of the 3 if-blocks above must fire. field={field}"
    );
}

/// Write context dispatch helper for static access (`Class.x = v`).
///
/// I-205 T6: B8 setter cell 18 が primary。matrix cell 化されていない static B3 setter
/// only (Read tier 2)、static B6 method、static B7 inherited は subsequent T11 (11-c) で
/// matrix expansion 予定だが、本 helper は **defensive Tier 2 honest error reclassify** で
/// 全 dispatch arm を実装 (= Read context `dispatch_static_member_read` と symmetric、
/// silent fallback 排除 + matrix expansion 後の T11 work piece を前倒し)。
pub(super) fn dispatch_static_member_write(
    class_name: &str,
    field: &str,
    sigs: &[MethodSignature],
    is_inherited: bool,
    value: Expr,
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
    if kinds.has_setter {
        return Ok(Expr::FnCall {
            target: CallTarget::UserAssocFn {
                ty: UserTypeRef::new(class_name),
                method: format!("set_{field}"),
            },
            args: vec![value],
        });
    }
    if kinds.has_getter {
        return Err(UnsupportedSyntaxError::new(
            "write to read-only static property",
            ts_obj.span(),
        )
        .into());
    }
    if kinds.has_method {
        return Err(UnsupportedSyntaxError::new("write to static method", ts_obj.span()).into());
    }
    unreachable!(
        "dispatch_static_member_write: sigs is non-empty (lookup_method_sigs_in_inheritance_chain \
         never returns Some(empty vec)) and MethodKind is exhaustive (Method/Getter/Setter), \
         so one of the 3 if-blocks above must fire. class={class_name}, field={field}"
    );
}
