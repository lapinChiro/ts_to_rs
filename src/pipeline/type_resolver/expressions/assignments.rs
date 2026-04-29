//! Assignment expression type resolution (`Assign` arm of `resolve_expr_inner`).
//!
//! Handles plain `=` and compound `??= &&= ||= += -= …` operators. For plain `=`,
//! propagates the LHS type to the RHS as expected type. For propagating compound
//! operators (`??= &&= ||=`), the [`rhs_expected_for_compound`] helper computes the
//! correct RHS expected type (LHS inner for `Option<T>` LHS, full LHS for non-Option).

use swc_common::Spanned;
use swc_ecma_ast as ast;

use super::super::*;
use crate::pipeline::type_resolution::Span;

impl<'a> TypeResolver<'a> {
    pub(super) fn resolve_assign_expr(&mut self, assign: &ast::AssignExpr) -> ResolvedType {
        // Propagate LHS type as expected on RHS (only for plain `=`, not `+=`/`-=` etc.)
        if assign.op == ast::AssignOp::Assign {
            if let Some(simple) = assign.left.as_simple() {
                let lhs_type = match simple {
                    ast::SimpleAssignTarget::Ident(ident) => {
                        self.mark_var_mutable(ident.id.sym.as_ref());
                        // I-142: record LHS type at the ident's span so
                        // downstream ??= handling in the Transformer can
                        // read it. Plain `=` also benefits from this for
                        // consistency (assign-target idents previously
                        // had no expr_types entry even for plain `=`).
                        self.record_assign_target_ident_type(ident)
                    }
                    ast::SimpleAssignTarget::Member(member) => {
                        // Mark the object variable as mutable
                        if let ast::Expr::Ident(ident) = member.obj.as_ref() {
                            self.mark_var_mutable(ident.sym.as_ref());
                        }
                        let obj_type = self.resolve_expr(&member.obj);
                        if let ResolvedType::Known(ref ty) = obj_type {
                            self.resolve_member_type(ty, &member.prop)
                        } else {
                            ResolvedType::Unknown
                        }
                    }
                    _ => ResolvedType::Unknown,
                };
                if let ResolvedType::Known(ref ty) = lhs_type {
                    let rhs_span = Span::from_swc(assign.right.span());
                    self.result.expected_types.insert(rhs_span, ty.clone());
                    self.propagate_expected(&assign.right, ty);
                }
            }
        } else {
            // Compound assignments (+=, -=, ??=, &&=, ||=, etc.) mark
            // the target mutable. `??=` (I-142) and `&&=`/`||=` (I-161)
            // additionally need the LHS type recorded at the ident's
            // span and expected-type propagation onto the RHS so the
            // Transformer can desugar to conditional-assign with a
            // correctly-coerced RHS (e.g., object literal → Named
            // struct instead of synthetic `_TypeLit0`).
            //
            // For `??=`: RHS expected = inner of Option<T> (the default
            // value is substituted for the None slot).
            //
            // For `&&=`/`||=`: RHS expected = the LHS type as a whole
            // (the assign branch substitutes the full LHS shape,
            // including `Some(...)` wrap for Option<T>). Using the
            // outer LHS type preserves TS `Option<T> &&= T` semantics —
            // the RHS expression supplies a `T` that the Transformer
            // wraps back into `Some(T)` at emission time. Passing the
            // inner `T` for expected would break Named struct coercion
            // when the LHS is `Option<Named>` (the object literal RHS
            // would be typed against `Named` directly, which is still
            // correct), but passing outer Option<T> here keeps the
            // propagation symmetric with plain `=` for the non-Option
            // case — both forms RHS-expect whatever the surrounding
            // assignment produces.
            //
            // Other compound ops (`+=`, `-=`, …) do not read the LHS
            // type from expr_types for emission dispatch, so we leave
            // their historical no-op behavior untouched to avoid
            // rippling expected-type side effects through unrelated
            // code paths. I-175 tracks the remaining compound-ops
            // coercion gap (outside this PRD's scope).
            let is_propagating_op = matches!(
                assign.op,
                ast::AssignOp::NullishAssign | ast::AssignOp::AndAssign | ast::AssignOp::OrAssign
            );
            match assign.left.as_simple() {
                Some(ast::SimpleAssignTarget::Ident(ident)) => {
                    self.mark_var_mutable(ident.id.sym.as_ref());
                    // Record LHS type for ALL compound assignments
                    // (including `+=`/`-=`/bitwise) so downstream
                    // `convert_bang_expr` Layer 3c can resolve the
                    // assignment target's storage type regardless of
                    // whether expected-type propagation fires. Without
                    // this, `!(x += v)` Layer 3c returns `None`, and
                    // Layer 4's generic fallback emits
                    // `let tmp: T = (x = x + v)` — an E0308 mismatch
                    // because Rust assign evaluates to `()`.
                    let lhs_type = self.record_assign_target_ident_type(ident);
                    if is_propagating_op {
                        if let ResolvedType::Known(ty) = &lhs_type {
                            if let Some(expected_ty) = rhs_expected_for_compound(&assign.op, ty) {
                                let rhs_span = Span::from_swc(assign.right.span());
                                self.result
                                    .expected_types
                                    .insert(rhs_span, expected_ty.clone());
                                self.propagate_expected(&assign.right, &expected_ty);
                            }
                        }
                    }
                }
                Some(ast::SimpleAssignTarget::Member(member)) => {
                    if let ast::Expr::Ident(ident) = member.obj.as_ref() {
                        self.mark_var_mutable(ident.sym.as_ref());
                    }
                    // I-205 T8 Iteration v12 Spec gap fix: **receiver** (= `member.obj`)
                    // の expr_type を全 compound op で unconditional resolve する。
                    // pre-T8 では `is_propagating_op` (`??= &&= ||=`) のみ resolve 経路を
                    // 通り、arithmetic/bitwise compound (`+= -= *= ... |=`) で receiver
                    // の expr_type が cache されず → T8 `dispatch_member_compound` の
                    // `classify_member_receiver` で `get_expr_type(receiver) = None` →
                    // silent Fallback dispatch (= class member setter dispatch を逃す
                    // silent semantic loss、本 T8 implementation で発覚した Spec gap)。
                    // T7 Iteration v11 で同 pattern の `Update.arg` 未再帰を fix した
                    // 構造的解消 と整合 (= TypeResolver visit coverage of operand-context
                    // expressions の cohesion)。
                    //
                    // Scope clarification (Iteration v12 second-review F-SX-1 由来): 本 fix
                    // で register されるのは **receiver の expr_type のみ** (= `member.obj`
                    // span)。**field type** (= `member.span` 全体の expr_types entry =
                    // `lhs.x` の resolve 結果型) は依然 `is_propagating_op` ブロック内のみ
                    // で register される。`classify_member_receiver` は receiver の
                    // expr_type のみ参照するため本 T8 dispatch の動作に影響なし、ただし
                    // 将来 `get_expr_type(&assign.left)` 等で field type を読む code path
                    // (例: T9 logical compound の member LHS type-aware emission) が
                    // arithmetic/bitwise compound では `Unknown` を得る latent gap あり。
                    // T9 着手時 (logical compound + Member target dispatch) に再 audit、
                    // 必要に応じて field type も全 op で register する extension を検討
                    // (本 T8 scope では unnecessary、`1 PRD = 1 architectural concern`
                    // 厳格適用)。
                    let obj_type = self.resolve_expr(&member.obj);
                    if is_propagating_op {
                        if let ResolvedType::Known(ref ty) = obj_type {
                            // For named fields: use resolve_member_type.
                            // For computed index (HashMap): extract value type.
                            let field_type = match &member.prop {
                                ast::MemberProp::Computed(_) => {
                                    // HashMap<K, V> → value type is V
                                    match ty {
                                        RustType::StdCollection {
                                            kind: crate::ir::StdCollectionKind::HashMap,
                                            args,
                                        } if args.len() == 2 => {
                                            ResolvedType::Known(args[1].clone())
                                        }
                                        _ => self.resolve_member_type(ty, &member.prop),
                                    }
                                }
                                _ => self.resolve_member_type(ty, &member.prop),
                            };
                            if let ResolvedType::Known(ref ft) = field_type {
                                let member_span = Span::from_swc(member.span());
                                self.result
                                    .expr_types
                                    .insert(member_span, ResolvedType::Known(ft.clone()));
                                if let Some(expected_ty) = rhs_expected_for_compound(&assign.op, ft)
                                {
                                    let rhs_span = Span::from_swc(assign.right.span());
                                    self.result
                                        .expected_types
                                        .insert(rhs_span, expected_ty.clone());
                                    self.propagate_expected(&assign.right, &expected_ty);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        self.resolve_expr(&assign.right)
    }
}

/// RHS expected-type policy for compound assignment operators (I-142 / I-161).
///
/// Propagates the LHS type down into the RHS expression so the Transformer can
/// emission-time-coerce object literals, `Option` auto-wraps, and synthetic
/// union hints against a known target shape instead of synthesising
/// `_TypeLit0` structs.
///
/// | op    | LHS `Option<T>`         | LHS `T` (non-Option) |
/// |-------|--------------------------|----------------------|
/// | `??=` | inner `T`                | None (dead at runtime) |
/// | `&&=` | inner `T` (emission wraps `Some`) | `T` |
/// | `\|\|=`| inner `T` (emission wraps `Some`) | `T` |
/// | other | None                      | None                 |
///
/// Returning `None` preserves the historical no-op propagation for `+=`,
/// `-=`, and the `??=` non-Option case — both intentionally skipped for the
/// reasons noted in-line at the call site (`??=` non-Option is dead, `+=`
/// et al. do not read LHS type from `expr_types`).
fn rhs_expected_for_compound(op: &ast::AssignOp, lhs_type: &RustType) -> Option<RustType> {
    match op {
        ast::AssignOp::NullishAssign => match lhs_type {
            RustType::Option(inner) => Some((**inner).clone()),
            _ => None,
        },
        ast::AssignOp::AndAssign | ast::AssignOp::OrAssign => match lhs_type {
            RustType::Option(inner) => Some((**inner).clone()),
            other => Some(other.clone()),
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nullish_assign_on_option_returns_inner() {
        let ty = RustType::Option(Box::new(RustType::F64));
        assert_eq!(
            rhs_expected_for_compound(&ast::AssignOp::NullishAssign, &ty),
            Some(RustType::F64)
        );
    }

    #[test]
    fn nullish_assign_on_non_option_returns_none() {
        // Historical no-op: `??=` on non-Option is dead, skip propagation.
        assert_eq!(
            rhs_expected_for_compound(&ast::AssignOp::NullishAssign, &RustType::F64),
            None
        );
    }

    #[test]
    fn and_assign_on_option_returns_inner() {
        let ty = RustType::Option(Box::new(RustType::Named {
            name: "Point".to_string(),
            type_args: vec![],
        }));
        assert_eq!(
            rhs_expected_for_compound(&ast::AssignOp::AndAssign, &ty),
            Some(RustType::Named {
                name: "Point".to_string(),
                type_args: vec![]
            })
        );
    }

    #[test]
    fn and_assign_on_non_option_returns_lhs_itself() {
        assert_eq!(
            rhs_expected_for_compound(&ast::AssignOp::AndAssign, &RustType::String),
            Some(RustType::String)
        );
    }

    #[test]
    fn or_assign_on_option_returns_inner() {
        let ty = RustType::Option(Box::new(RustType::String));
        assert_eq!(
            rhs_expected_for_compound(&ast::AssignOp::OrAssign, &ty),
            Some(RustType::String)
        );
    }

    #[test]
    fn or_assign_on_non_option_returns_lhs_itself() {
        assert_eq!(
            rhs_expected_for_compound(&ast::AssignOp::OrAssign, &RustType::Bool),
            Some(RustType::Bool)
        );
    }

    #[test]
    fn other_compound_ops_return_none() {
        for op in [
            ast::AssignOp::AddAssign,
            ast::AssignOp::SubAssign,
            ast::AssignOp::MulAssign,
            ast::AssignOp::DivAssign,
            ast::AssignOp::ModAssign,
            ast::AssignOp::BitAndAssign,
            ast::AssignOp::BitOrAssign,
            ast::AssignOp::BitXorAssign,
            ast::AssignOp::LShiftAssign,
            ast::AssignOp::RShiftAssign,
            ast::AssignOp::ZeroFillRShiftAssign,
        ] {
            assert_eq!(
                rhs_expected_for_compound(&op, &RustType::F64),
                None,
                "{op:?} should not propagate"
            );
        }
    }
}
