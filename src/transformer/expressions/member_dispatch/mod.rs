//! Class member dispatch logic for receiver-type detection and Read/Write/Update/Compound
//! context dispatch.
//!
//! ## Architectural concern
//!
//! Centralizes the cross-cutting "how to dispatch class member access for a given receiver"
//! knowledge that is shared by:
//! - **Read context** (`Transformer::resolve_member_access` in `member_access.rs`): calls
//!   [`read::dispatch_instance_member_read`] / [`read::dispatch_static_member_read`] after
//!   Read-only special cases (Enum variant, `Math.PI`, `.length`).
//! - **Write context** ([`Transformer::dispatch_member_write`]): plain assignment
//!   (`obj.x = v` / `Class.x = v`) routed through `convert_assign_expr`'s
//!   `AssignOp::Assign` × Member × non-Computed gate (T6 cells 11-19).
//! - **Update context** ([`update::dispatch_instance_member_update`] /
//!   [`update::dispatch_static_member_update`]): UpdateExpr (`obj.x++` / `Class.x--`)
//!   called from `assignments.rs::convert_update_expr` Member arm (T7 cells 42-45).
//! - **Compound context** ([`Transformer::dispatch_member_compound`]): arithmetic /
//!   bitwise compound (`obj.x += v` / `Class.x -= v`、11 ops) routed through
//!   `convert_assign_expr`'s arithmetic-compound × Member × non-Computed gate
//!   (T8 cells 21-29 + 30-35).
//! - **Subsequent T9** (logical compound `obj.x ??= d` / `obj.x &&= v` / `obj.x ||= v`):
//!   planned to leverage the same classifier via context-specific dispatch helpers,
//!   structurally preventing further DRY propagation.
//!
//! ## File split rationale (Iteration v12 third-review、`design-integrity.md` cohesion)
//!
//! Pre-split: 単一 `member_dispatch.rs` (1179 行、CLAUDE.md "0 errors / 0 warnings" の
//! file-line threshold 1000 行 violation)、4 architectural concern (Read / Write / Update /
//! Compound) が単一 file に同居。Post-split: 各 architectural concern を独立 file に分離
//! (read.rs / write.rs / update.rs / compound.rs) + cross-cutting infrastructure
//! (MemberKindFlags / is_side_effect_free / wrap_with_recv_binding / build_setter_desugar_block /
//! IIFE wrap helpers) を [`shared`] module に集約。
//!
//! ## DRY refactor (Iteration v12、`design-integrity.md` "DRY")
//!
//! T7 update (`dispatch_instance_member_update`) と T8 compound (`dispatch_instance_member_compound`)
//! の **B4 setter desugar arm** が完全 identical な receiver-type detection + IIFE wrap +
//! getter/setter call construction を 30 行 × 2 helper = 60 行で重複していた (Iteration v11
//! T7 implementation + Iteration v12 T8 implementation)。本 split で `build_instance_setter_desugar_with_iife_wrap`
//! shared helper に集約 (= IIFE wrap concern が 1 箇所に集中、subsequent T9 logical compound
//! も同 helper を leverage 可能、DRY violation 増殖を構造的に防止)。Static dispatch (T7 +
//! T8) も同様に `build_static_setter_desugar_block` で集約 (受信者 = class TypeName で
//! side-effect なし path、IIFE wrap 不要 = simpler shared helper)。
//!
//! ## DRY rationale (pre-split history、Iteration v10 second + third review extract)
//!
//! Pre-extract: T5 `resolve_member_access` + T6 first-review `dispatch_member_write` had the
//! identical Static gate (Ident + `get_expr_type` None + Struct + lookup hit) + Instance gate
//! (`Named/Option<Named>` + lookup hit) + Fallback flow inlined; member-kind classification
//! (`has_getter`/`has_setter`/`has_method`) was duplicated 3 lines × 4 dispatch helpers = 12
//! lines.
//!
//! Post-extract:
//! - Receiver-type detection lives in [`Transformer::classify_member_receiver`].
//! - Member-kind classification lives in [`shared::MemberKindFlags`] (single `from_sigs`
//!   constructor + 3 boolean fields, consumed once per dispatch helper).
//! - Dispatch arms (Read/Write/Update/Compound × Instance/Static = 8 helpers) reuse the
//!   classifier and the flags with context-specific emit logic only (= IR shape + error
//!   wording).
//!
//! All 8 dispatch helpers terminate with `unreachable!()` macro after the if-block flag
//! checks (`has_getter`/`has_setter`/`has_method`), codifying the structural invariant
//! "`MethodKind` is 3-variant exhaustive + lookup returns non-empty `Vec<MethodSignature>` ⇒
//! one of the if-blocks must fire".

use anyhow::Result;
use swc_ecma_ast as ast;

use crate::ir::{BinOp, Expr, RustType};
use crate::registry::{MethodSignature, TypeDef};
use crate::transformer::Transformer;

use super::member_access::extract_non_computed_field_name;

mod compound;
mod logical;
pub(crate) mod read;
mod shared;
mod update;
mod write;

pub(crate) use logical::LogicalCompoundContext;
pub(crate) use update::{dispatch_instance_member_update, dispatch_static_member_update};

// =============================================================================
// MemberReceiverClassification — receiver-type detection result (shared Read/Write/Update/Compound)
// =============================================================================

/// Class member dispatch classification for a receiver expression.
///
/// All 4 dispatch contexts (Read / Write / Update / Compound) consume this enum to drive
/// context-specific dispatch (Read = getter MethodCall / Write = setter MethodCall /
/// Update = setter desugar block / Compound = setter desugar block / etc.). The classifier
/// captures the cross-cutting "how to detect class member dispatch context" knowledge in
/// one place; context-specific dispatch arms are layered on top via the variant-specific
/// helpers in submodules ([`read`] / [`write`] / [`update`] / [`compound`]).
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
    /// Caller (Read: FieldAccess emit / Write: FieldAccess Assign emit / Update / Compound:
    /// existing FieldAccess + BinaryOp emit) で context-specific fallback を実行。
    Fallback,
}

impl<'a> Transformer<'a> {
    /// Classify the member access receiver for class member dispatch (Read/Write/Update/Compound 共通)。
    ///
    /// `&self` (immutable) で TypeRegistry + TypeResolver を query するのみで、
    /// receiver expression 自体の Expr conversion は行わない (= caller responsibility、
    /// classification 結果に応じて conversion 必要時のみ実行することで dead conversion 排除)。
    ///
    /// 全 4 dispatch context (Read = `resolve_member_access`、Write = `dispatch_member_write`、
    /// Update = `convert_update_expr_member_arm`、Compound = `dispatch_member_compound`) は
    /// 本 helper の classification 結果に応じて context-specific dispatch helper を呼ぶ。
    /// Subsequent T9 (logical compound `??= &&= ||=`) も同 helper を leverage、receiver
    /// 同定 logic の更なる duplication 増殖を防止。
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
            } => write::dispatch_static_member_write(
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
                write::dispatch_instance_member_write(
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

    /// Tries to dispatch the LHS of a logical compound assignment (`obj.x ??= d`
    /// / `obj.x &&= v` / `obj.x ||= v` or their static counterparts) through
    /// the class member dispatch framework. T9 entry counterpart of
    /// [`Self::dispatch_member_compound`] (T8) for A5 logical compound ops
    /// (`AssignOp::NullishAssign | AndAssign | OrAssign`).
    ///
    /// Single source of truth for the T9 dispatch gate, called from three sites:
    /// 1. `convert_assign_expr` (expression context、`obj.x ??= d` inside call
    ///    arg / return value / ternary branch / etc.) → wraps the returned
    ///    `Expr::Block` (TailExpr = post-state getter call).
    /// 2. `try_convert_nullish_assign_stmt` (statement context、`obj.x ??= d;`
    ///    bare stmt for `??=`) → wraps the returned `Expr::Block` in
    ///    `Stmt::Expr(...)` (no TailExpr).
    /// 3. `try_convert_compound_logical_assign_stmt` (statement context、
    ///    `obj.x &&= v;` / `obj.x ||= v;` bare stmt for `&&=`/`||=`) → wraps
    ///    the returned `Expr::Block` in `Stmt::Expr(...)`.
    ///
    /// Returns `Ok(Some(block))` when the receiver classifies as Static or
    /// Instance (= class member dispatch fires). Returns `Ok(None)` when:
    /// - `member.prop` is `Computed` (`obj[i] ??= d`、matrix scope 外、I-203
    ///   codebase-wide AST exhaustiveness defer)
    /// - `classify_member_receiver` returns `Fallback` (B1 field / B9 unknown
    ///   / non-class receiver / static field) — caller falls through to
    ///   existing `nullish_assign.rs` / `compound_logical_assign.rs` emission
    ///   logic, preserving cells 36 + 41-e regression behavior.
    ///
    /// `Err(UnsupportedSyntaxError)` is returned when the LHS member type
    /// cannot be resolved (rare; `nullish_assign.rs` / `compound_logical_assign.rs`
    /// surface the same wording for unresolved types).
    pub(crate) fn try_dispatch_member_logical_compound(
        &mut self,
        member: &ast::MemberExpr,
        op: ast::AssignOp,
        rhs_ast: &ast::Expr,
        context: LogicalCompoundContext,
    ) -> Result<Option<Expr>> {
        // `MemberProp::Computed` (`obj[i] ??= d`) は dispatch 外。Caller-side
        // で既存 path に流す前提で early Ok(None)。
        if !matches!(
            &member.prop,
            ast::MemberProp::Ident(_) | ast::MemberProp::PrivateName(_)
        ) {
            return Ok(None);
        }
        let Some(field) = extract_non_computed_field_name(&member.prop) else {
            // `MemberProp::Ident | PrivateName` filter passed but extraction
            // returned None: invariant violation (extract_non_computed_field_name
            // covers both arms). Codify as unreachable.
            unreachable!(
                "try_dispatch_member_logical_compound: MemberProp filter passed but \
                 extract_non_computed_field_name returned None"
            )
        };
        let classification = self.classify_member_receiver(&member.obj, &field);
        if matches!(classification, MemberReceiverClassification::Fallback) {
            return Ok(None);
        }
        // Static / Instance dispatch fires: convert rhs only (lhs_type は dispatch
        // helper 内 sigs から extract、TypeResolver `expr_types[member_span]` への
        // dependency を排除 = T8 second-review F-SX-1 で予測された Spec gap (= class
        // member access for getter は registry の `lookup_field_type` で None、
        // `expr_types` 未 populate) の self-contained 回避)。Iteration v14 deep-deep
        // review で entry method の lhs_type lookup を完全 remove (TypeVar generic
        // class member の expr_types 未 populate に対する resilience 確保)。
        let rhs = self.convert_expr(rhs_ast)?;
        match classification {
            MemberReceiverClassification::Static {
                class_name,
                sigs,
                is_inherited,
            } => Ok(Some(logical::dispatch_static_member_logical_compound(
                &class_name,
                &field,
                &sigs,
                is_inherited,
                op,
                rhs,
                self.synthetic,
                &member.obj,
                context,
            )?)),
            MemberReceiverClassification::Instance { sigs, is_inherited } => {
                let object = self.convert_expr(&member.obj)?;
                Ok(Some(logical::dispatch_instance_member_logical_compound(
                    &object,
                    &field,
                    &sigs,
                    is_inherited,
                    op,
                    rhs,
                    self.synthetic,
                    &member.obj,
                    context,
                )?))
            }
            MemberReceiverClassification::Fallback => unreachable!(
                "try_dispatch_member_logical_compound: Fallback classification was \
                 early-returned above; this match arm cannot fire"
            ),
        }
    }

    /// Dispatches the LHS of an arithmetic / bitwise compound assignment
    /// `obj.x <op>= rhs` (or `Class.x <op>= rhs`) according to the class member
    /// shape registered for the receiver type. T8 entry counterpart of
    /// [`Self::dispatch_member_write`] (T6 plain `=`)、ranged over A3 + A4 ops
    /// (`AddAssign`/`SubAssign`/`MulAssign`/`DivAssign`/`ModAssign`/`BitAndAssign`/
    /// `BitOrAssign`/`BitXorAssign`/`LShiftAssign`/`RShiftAssign`/
    /// `ZeroFillRShiftAssign`、collectively 11 ops mapped to `BinOp` via
    /// caller-side `arithmetic_compound_op_to_binop`).
    ///
    /// A5 logical compound (`??=` / `&&=` / `||=`) is handled by
    /// [`Self::try_dispatch_member_logical_compound`] (T9) — symmetric counterpart
    /// for class member dispatch、and falls through to existing `nullish_assign.rs`
    /// / `compound_logical_assign.rs` helpers on `Fallback`. Computed properties
    /// (`obj[i] += v`) are gated out by the caller (Computed `MemberProp::Computed`
    /// path falls through to the existing `convert_member_expr_for_write`
    /// `Expr::Index` route).
    ///
    /// Dispatch arms (Spec → Impl Mapping、`backlog/I-205-...md`
    /// `## Spec → Impl Dispatch Arm Mapping` の `convert_assign_expr compound
    /// branch` table 参照):
    /// - `Static { class_name, sigs, is_inherited }` → [`compound::dispatch_static_member_compound`]
    /// - `Instance { sigs, is_inherited }` → [`compound::dispatch_instance_member_compound`]
    /// - `Fallback` (B1 field、B9 unknown、non-class receiver、static field) →
    ///   既存 `Expr::Assign { target: FieldAccess, value: BinaryOp { left:
    ///   FieldAccess, op, right: rhs } }` emit (regression preserve)
    pub(crate) fn dispatch_member_compound(
        &mut self,
        member: &ast::MemberExpr,
        op: BinOp,
        rhs: Expr,
    ) -> Result<Expr> {
        let field = extract_non_computed_field_name(&member.prop).unwrap_or_else(|| {
            // Caller (`convert_assign_expr`) gates on `Ident | PrivateName`, so
            // `Computed` cannot reach here. `unreachable!()` codifies the gate
            // invariant (= dispatch_member_write の `unreachable!()` symmetric)
            unreachable!(
                "dispatch_member_compound: caller must gate on MemberProp::Ident | PrivateName \
                 (Computed access falls through to convert_member_expr_for_write's Expr::Index path)"
            )
        });

        match self.classify_member_receiver(&member.obj, &field) {
            MemberReceiverClassification::Static {
                class_name,
                sigs,
                is_inherited,
            } => compound::dispatch_static_member_compound(
                &class_name,
                &field,
                &sigs,
                is_inherited,
                op,
                rhs,
                &member.obj,
            ),
            MemberReceiverClassification::Instance { sigs, is_inherited } => {
                // Instance dispatch のみ receiver expression conversion が必要
                // (Static は class TypeName-form path emit で receiver Expr 不要、
                // Fallback は `convert_member_expr_for_write` 経由で内部 conversion)。
                let object = self.convert_expr(&member.obj)?;
                compound::dispatch_instance_member_compound(
                    &object,
                    &field,
                    &sigs,
                    is_inherited,
                    op,
                    rhs,
                    &member.obj,
                )
            }
            MemberReceiverClassification::Fallback => {
                // Fallback (B1 field / B9 unknown / non-class receiver / static field):
                // 既存 compound desugar emit (cells 20, 28, 29-a, 29-e-e, 30, 34-a,
                // 35-e regression preserve)。`Expr::Assign { target: FieldAccess,
                // value: BinaryOp { left: FieldAccess, op, right: rhs } }` で
                // pre-T8 既存挙動を token-level identical に維持。
                let target = self.convert_member_expr_for_write(member)?;
                Ok(Expr::Assign {
                    target: Box::new(target.clone()),
                    value: Box::new(Expr::BinaryOp {
                        left: Box::new(target),
                        op,
                        right: Box::new(rhs),
                    }),
                })
            }
        }
    }
}
