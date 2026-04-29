//! Class member dispatch logic for receiver-type detection and Read/Write context dispatch.
//!
//! ## Architectural concern
//!
//! Centralizes the cross-cutting "how to dispatch class member access for a given receiver"
//! knowledge that is shared by:
//! - **Read context** (`Transformer::resolve_member_access` in `member_access.rs`): calls
//!   [`dispatch_instance_member_read`] / [`dispatch_static_member_read`] after Read-only
//!   special cases (Enum variant, `Math.PI`, `.length`).
//! - **Write context** ([`Transformer::dispatch_member_write`] in this file): plain
//!   assignment (`obj.x = v` / `Class.x = v`) routed through `convert_assign_expr`'s
//!   `AssignOp::Assign` × Member × non-Computed gate.
//! - **Subsequent T7-T9** (compound `+= -= ??= &&= ||= ++/--`): planned to leverage the same
//!   classifier via context-specific dispatch helpers, structurally preventing further DRY
//!   propagation.
//!
//! ## DRY rationale (Iteration v10 second + third review extract、`design-integrity.md` "DRY")
//!
//! Pre-extract: T5 `resolve_member_access` + T6 first-review `dispatch_member_write` had the
//! identical Static gate (Ident + `get_expr_type` None + Struct + lookup hit) + Instance gate
//! (`Named/Option<Named>` + lookup hit) + Fallback flow inlined; member-kind classification
//! (`has_getter`/`has_setter`/`has_method`) was duplicated 3 lines × 4 dispatch helpers = 12
//! lines.
//!
//! Post-extract:
//! - Receiver-type detection lives in [`Transformer::classify_member_receiver`].
//! - Member-kind classification lives in [`MemberKindFlags`] (single `from_sigs` constructor +
//!   3 boolean fields, consumed once per dispatch helper).
//! - Dispatch arms (Read/Write × Instance/Static = 4 helpers) reuse the classifier and the
//!   flags with context-specific emit logic only (= IR shape + error wording).
//!
//! All 4 dispatch helpers terminate with `unreachable!()` macro after the 3 if-block flag
//! checks (`has_getter`/`has_setter`/`has_method`), codifying the structural invariant
//! "`MethodKind` is 3-variant exhaustive + lookup returns non-empty `Vec<MethodSignature>` ⇒
//! one of the 3 if-blocks must fire". This Iteration v9 deep deep review fix pattern is
//! applied symmetrically across all 4 helpers (= Read/Write × Instance/Static, restored in
//! Iteration v10 second-review by replacing T5's stray `Ok(Expr::FieldAccess)` dead-code arm
//! in `dispatch_instance_member_read`).

use anyhow::Result;
use swc_common::Spanned;
use swc_ecma_ast as ast;

use crate::ir::{BinOp, CallTarget, Expr, MethodKind, RustType, Stmt};
use crate::registry::{MethodSignature, TypeDef};
use crate::transformer::{Transformer, UnsupportedSyntaxError};

use super::member_access::extract_non_computed_field_name;
use super::{TS_NEW_BINDING, TS_OLD_BINDING};

// =============================================================================
// MemberKindFlags — DRY-extracted member-kind classification (Iteration v10 second-review)
// =============================================================================

/// Member-kind flags computed once from a non-empty `[MethodSignature]` slice.
///
/// `MethodKind` is a 3-variant exhaustive enum (`Method`/`Getter`/`Setter`); the
/// [`crate::registry::TypeRegistry::lookup_method_sigs_in_inheritance_chain`] invariant
/// (returns `Some(non-empty Vec)` or `None`) guarantees that at least one flag is `true`
/// whenever `MemberKindFlags` is constructed from a `Some(_)` lookup result. Each dispatch
/// arm consumes the flags in a fixed precedence order (= explicitly enumerated by the
/// caller per Read/Write context semantic), and the trailing `unreachable!()` macro codifies
/// the structural invariant.
///
/// Centralizing the flag computation eliminates the 3-line `let has_X = sigs.iter()...`
/// pattern × 4 dispatch helpers DRY violation found in Iteration v10 second-review.
struct MemberKindFlags {
    has_getter: bool,
    has_setter: bool,
    has_method: bool,
}

impl MemberKindFlags {
    fn from_sigs(sigs: &[MethodSignature]) -> Self {
        Self {
            has_getter: sigs.iter().any(|s| s.kind == MethodKind::Getter),
            has_setter: sigs.iter().any(|s| s.kind == MethodKind::Setter),
            has_method: sigs.iter().any(|s| s.kind == MethodKind::Method),
        }
    }
}

// =============================================================================
// MemberReceiverClassification — receiver-type detection result (shared Read/Write/Compound)
// =============================================================================

/// Class member dispatch classification for a receiver expression.
///
/// Read context ([`Transformer::resolve_member_access`]) and Write context
/// ([`Transformer::dispatch_member_write`]) (and subsequent T7-T9 compound dispatch) consume
/// this enum to drive context-specific dispatch (Read = getter MethodCall / Write = setter
/// MethodCall / etc.). The classifier captures the cross-cutting "how to detect class member
/// dispatch context" knowledge in one place; context-specific dispatch arms are layered on
/// top via the variant-specific helpers below.
pub(crate) enum MemberReceiverClassification {
    /// Receiver = `Ident(class_name)` で `get_expr_type` = `None` (= class TypeName context、
    /// instance shadowing 抑止) かつ class TypeRegistry 登録 (`TypeDef::Struct`、
    /// `is_interface: false`) かつ inheritance chain lookup hit。
    /// Static dispatch (B8) で `Class::method`-form emit。
    Static {
        /// Class TypeName (= receiver Ident sym)
        class_name: String,
        /// Member overload signatures (`MethodKind::{Method,Getter,Setter}` mix possible、
        /// `lookup_method_sigs_in_inheritance_chain` invariant により non-empty)
        sigs: Vec<MethodSignature>,
        /// `true` なら parent class からの inheritance hit (B7 systematic Tier 2 reclassify)、
        /// `false` なら direct hit (cell 18 setter / cell 9 getter Tier 1 dispatch 候補)
        is_inherited: bool,
    },
    /// Receiver の TypeResolver 上の type が `RustType::Named { name, .. }` または
    /// `RustType::Option(Box<RustType::Named>)` (= narrowed nullable instance) であり、
    /// inheritance chain lookup hit。Instance dispatch (B1-B4, B6, B7) で `obj.method`-form emit。
    Instance {
        /// Member overload signatures (Static と同 semantics、non-empty 保証)
        sigs: Vec<MethodSignature>,
        /// `true` なら parent class inheritance、`false` なら direct hit
        is_inherited: bool,
    },
    /// Class member dispatch 不適用。以下のいずれか:
    /// - B1 field (instance 受信、lookup miss = `methods` に entry 不在)
    /// - B9 unknown (receiver type 未確定、`Any` / external library type 等)
    /// - non-class receiver (Vec, HashMap, primitive, function, enum 等)
    /// - static field write (Class TypeName + `methods` に entry 不在、T11 (11-d) で
    ///   `Class::set_field` associated fn emission strategy 確定予定)
    /// - `Ident` 直接 match に該当しない wrapped receiver (`Paren`/`TsAs`/`TsNonNull`、
    ///   pre-existing latent silent gap、T11 (11-f) で robustness 改善検討)
    ///
    /// Caller (Read: FieldAccess emit / Write: FieldAccess Assign emit / Compound: existing
    /// FieldAccess + BinaryOp emit) で context-specific fallback を実行。
    Fallback,
}

// =============================================================================
// classify_member_receiver — shared classifier (& dispatch_member_write entry)
// =============================================================================

impl<'a> Transformer<'a> {
    /// Classify the member access receiver for class member dispatch (Read/Write/Compound 共通)。
    ///
    /// `&self` (immutable) で TypeRegistry + TypeResolver を query するのみで、
    /// receiver expression 自体の Expr conversion は行わない (= caller responsibility、
    /// classification 結果に応じて conversion 必要時のみ実行することで dead conversion 排除)。
    ///
    /// Read 側 (`resolve_member_access`) と Write 側 (`dispatch_member_write`) は本 helper の
    /// classification 結果に応じて context-specific dispatch helper (Read getter / Write
    /// setter 等) を呼ぶ。Subsequent T7-T9 (compound `+= -= ??= &&= ||= ++/--`) も同 helper を
    /// leverage、receiver 同定 logic の更なる duplication 増殖を防止。
    pub(crate) fn classify_member_receiver(
        &self,
        receiver: &ast::Expr,
        field: &str,
    ) -> MemberReceiverClassification {
        // Static (B8) gate: receiver = `Ident(class_name)` で TypeResolver が instance type を
        // 解決していない (= class TypeName context、instance variable shadowing 抑止) かつ
        // class TypeRegistry 登録あり、かつ inheritance chain lookup hit。
        //
        // Note: `Paren`/`TsAs`/`TsNonNull` 等で wrap された Ident は本 direct match に hit せず
        // skip される (pre-existing latent silent gap、T11 (11-f) で robustness 改善検討予定)。
        if let ast::Expr::Ident(ident) = receiver {
            let name = ident.sym.as_ref();
            if self.get_expr_type(receiver).is_none() {
                if let Some(TypeDef::Struct {
                    is_interface: false,
                    ..
                }) = self.reg().get(name)
                {
                    if let Some((sigs, is_inherited)) = self
                        .reg()
                        .lookup_method_sigs_in_inheritance_chain(name, field)
                    {
                        return MemberReceiverClassification::Static {
                            class_name: name.to_string(),
                            sigs,
                            is_inherited,
                        };
                    }
                    // lookup miss = static field、Fallback fall-through (T11 (11-d))
                }
            }
        }

        // Instance (B1-B4, B6, B7, B9) gate: receiver type = `RustType::Named { name, .. }`
        // または `RustType::Option(Box<RustType::Named>)` (= narrowed nullable instance、TS
        // narrowing で nullable 除去された field access に該当)、かつ inheritance chain lookup hit。
        let receiver_type_name = match self.get_expr_type(receiver) {
            Some(RustType::Named { name, .. }) => Some(name.clone()),
            Some(RustType::Option(inner)) => match inner.as_ref() {
                RustType::Named { name, .. } => Some(name.clone()),
                _ => None,
            },
            _ => None,
        };
        if let Some(type_name) = receiver_type_name {
            if let Some((sigs, is_inherited)) = self
                .reg()
                .lookup_method_sigs_in_inheritance_chain(&type_name, field)
            {
                return MemberReceiverClassification::Instance { sigs, is_inherited };
            }
        }

        MemberReceiverClassification::Fallback
    }

    /// Dispatches the LHS of a plain assignment `obj.x = value` (or `Class.x = value`)
    /// according to the class member shape registered for the receiver type.
    ///
    /// Symmetric counterpart of [`Transformer::resolve_member_access`] (Read context, T5):
    /// the receiver-type detection (via [`Self::classify_member_receiver`]) is shared,
    /// satisfying INV-2 (External (E1) と internal (E2 this) dispatch path symmetry の
    /// Read/Write 両方向 cohesion).
    ///
    /// Dispatch arms (Spec → Impl Mapping、`backlog/I-205-...md` `dispatch_member_write` table):
    /// - `lookup` returns `(Setter, false)` (instance B3 / B4) → `obj.set_x(value)` MethodCall
    /// - `lookup` returns `(Getter, false)` and Setter absent (B2) → Tier 2 honest error
    ///   `"write to read-only property"`
    /// - `lookup` returns `(Method, false)` (B6) → Tier 2 honest error `"write to method"`
    /// - `lookup` returns `is_inherited=true` (B7) → Tier 2 honest error
    ///   `"write to inherited accessor"` (orthogonal architectural concern = 別 PRD I-206)
    /// - Static dispatch: similar 4 arms with `Class::set_x(value)` FnCall + Tier 2 error wording
    /// - Fallback (B1 field, B9 unknown, static field) → `Expr::Assign { FieldAccess, value }`
    ///
    /// Computed properties (`obj[i] = v`) は本 helper 経由で **dispatch されない** (caller
    /// 側 `convert_assign_expr` で `MemberProp::Ident | PrivateName` のみ gate)、
    /// 既存 `convert_member_expr_for_write` の `Expr::Index` 経路で handle。本 helper 内
    /// `extract_non_computed_field_name` の `None` return path は `unreachable!()` で gate
    /// invariant を codify。
    pub(crate) fn dispatch_member_write(
        &mut self,
        member: &ast::MemberExpr,
        value: Expr,
    ) -> Result<Expr> {
        let field = extract_non_computed_field_name(&member.prop).unwrap_or_else(|| {
            // Caller (`convert_assign_expr`) gates on `Ident | PrivateName`, so
            // `Computed` cannot reach here. `unreachable!()` codifies the gate invariant
            // (= dispatch_*_member_read/write の `unreachable!()` symmetric)
            unreachable!(
                "dispatch_member_write: caller must gate on MemberProp::Ident | PrivateName \
                 (Computed access is handled by convert_member_expr_for_write's Expr::Index path)"
            )
        });

        match self.classify_member_receiver(&member.obj, &field) {
            MemberReceiverClassification::Static {
                class_name,
                sigs,
                is_inherited,
            } => dispatch_static_member_write(
                &class_name,
                &field,
                &sigs,
                is_inherited,
                value,
                &member.obj,
            ),
            MemberReceiverClassification::Instance { sigs, is_inherited } => {
                // Instance dispatch のみ receiver expression conversion が必要 (Static は
                // `Class::method`-form path emit で receiver Expr 不要、Fallback は
                // `convert_member_expr_for_write` 経由で内部的に conversion 実施)。
                let object = self.convert_expr(&member.obj)?;
                dispatch_instance_member_write(
                    &object,
                    &field,
                    &sigs,
                    is_inherited,
                    value,
                    &member.obj,
                )
            }
            MemberReceiverClassification::Fallback => {
                // Fallback (B1 field / B9 unknown / non-class receiver / static field):
                // existing FieldAccess Assign emit。T5 で導入した
                // `convert_member_expr_inner(member, for_write=true)` skip path と
                // token-level identical な emit (regression lock-in、unit test
                // `test_t6_fallback_emits_same_ir_as_t5_skip_path` で structural verify)。
                let target = self.convert_member_expr_for_write(member)?;
                Ok(Expr::Assign {
                    target: Box::new(target),
                    value: Box::new(value),
                })
            }
        }
    }
}

// =============================================================================
// Read context dispatch helpers (called from member_access.rs::resolve_member_access)
// =============================================================================

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
pub(super) fn dispatch_instance_member_read(
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
pub(super) fn dispatch_static_member_read(
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
                ty: crate::ir::UserTypeRef::new(class_name),
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

// =============================================================================
// Write context dispatch helpers (called only from dispatch_member_write、private)
// =============================================================================

/// Write context dispatch helper for instance access (`obj.x = v`).
///
/// I-205 T6: B2 / B3 / B4 / B6 / B7 dispatch arms。`is_inherited = true` (B7) は
/// architectural concern = "Class inheritance dispatch" (別 PRD I-206) と orthogonal
/// のため Tier 2 honest error reclassify (= Read context cell 8 と symmetric)。
fn dispatch_instance_member_write(
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
fn dispatch_static_member_write(
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
                ty: crate::ir::UserTypeRef::new(class_name),
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

// =============================================================================
// Update context dispatch helpers (T7、called from convert_update_expr Member arm)
// =============================================================================

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

/// Builds the setter desugar block expression for an UpdateExpr (`++` / `--`)
/// on a class member with both getter and setter (B4 instance, B8 setter +
/// getter static).
///
/// Postfix (`obj.x++` / `obj.x--`、old value yielded):
/// ```text
/// { let __ts_old = <getter_call>; <setter_emit(__ts_old <op> 1.0)>; __ts_old }
/// ```
///
/// Prefix (`++obj.x` / `--obj.x`、new value yielded):
/// ```text
/// { let __ts_new = <getter_call> <op> 1.0; <setter_emit(__ts_new)>; __ts_new }
/// ```
///
/// `setter_emit_for_arg` is a closure that takes the setter argument
/// (`__ts_old <op> 1.0` for postfix, or `__ts_new` for prefix) and returns the
/// setter call IR (`Expr::MethodCall` for instance, `Expr::FnCall` for static).
/// This abstraction shares the block shape between instance and static dispatch
/// while keeping the call IR specific to each context.
///
/// Variable names use the `__ts_` prefix per [I-154 namespace reservation
/// rule](crate::transformer::statements) so user code containing identifiers
/// like `_old` / `_new` cannot shadow or collide with this emission. (T7
/// extends the `__ts_` namespace from labels (I-154) to value bindings.)
fn build_update_setter_block(
    getter_call: Expr,
    op: BinOp,
    is_postfix: bool,
    setter_emit_for_arg: impl FnOnce(Expr) -> Expr,
) -> Expr {
    let one = Expr::NumberLit(1.0);
    let (binding_name, binding_init, setter_arg) = if is_postfix {
        (
            TS_OLD_BINDING,
            getter_call,
            Expr::BinaryOp {
                left: Box::new(Expr::Ident(TS_OLD_BINDING.to_string())),
                op,
                right: Box::new(one),
            },
        )
    } else {
        (
            TS_NEW_BINDING,
            Expr::BinaryOp {
                left: Box::new(getter_call),
                op,
                right: Box::new(one),
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

/// Update context dispatch helper for instance access (`obj.x++` / `obj.x--`).
///
/// I-205 T7: dispatches A6 Increment/Decrement on Member target with instance
/// receiver. Unlike [`dispatch_instance_member_write`] (which fires setter for
/// any has_setter), update dispatch must distinguish B3 (setter only、read of
/// write-only fails because `++/--` requires read first) from B4 (both、setter
/// desugar with numeric type check).
///
/// Dispatch arms (matrix cells 42-45):
/// - `is_inherited = true` (B7) → Tier 2 honest `"write to inherited accessor"`
///   (cells 45-dc and `++` symmetric)
/// - `has_getter && has_setter` (B4) with **numeric getter return type** → setter
///   desugar block (cells 43, 45-c)
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
/// path in [`Transformer::convert_update_expr`] which builds a direct
/// `FieldAccess` postfix/prefix block (regression Tier 2 → Tier 1 transition).
pub(super) fn dispatch_instance_member_update(
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
        // B4: setter desugar with numeric type gate
        if !getter_return_is_numeric(sigs) {
            return Err(
                UnsupportedSyntaxError::new(non_numeric_update_message(op), ts_obj.span()).into(),
            );
        }
        let getter_call = Expr::MethodCall {
            object: Box::new(object.clone()),
            method: field.to_string(),
            args: vec![],
        };
        let setter_method = format!("set_{field}");
        let object_for_setter = object.clone();
        return Ok(build_update_setter_block(
            getter_call,
            op,
            is_postfix,
            move |arg| Expr::MethodCall {
                object: Box::new(object_for_setter),
                method: setter_method,
                args: vec![arg],
            },
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
/// MethodCall emit.
///
/// Dispatch arms (matrix cell 45-dd is primary、A6 `++` static symmetric is the
/// `op = BinOp::Add` mirror; static B2/B3/B6/B7 are matrix cell 化なし but
/// covered defensively per the same `dispatch_static_member_write` pattern from
/// T6 Iteration v9 deep deep review).
pub(super) fn dispatch_static_member_update(
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
        let getter_call = Expr::FnCall {
            target: CallTarget::UserAssocFn {
                ty: crate::ir::UserTypeRef::new(class_name),
                method: field.to_string(),
            },
            args: vec![],
        };
        let class_name_for_setter = class_name.to_string();
        let setter_method = format!("set_{field}");
        return Ok(build_update_setter_block(
            getter_call,
            op,
            is_postfix,
            move |arg| Expr::FnCall {
                target: CallTarget::UserAssocFn {
                    ty: crate::ir::UserTypeRef::new(&class_name_for_setter),
                    method: setter_method,
                },
                args: vec![arg],
            },
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
