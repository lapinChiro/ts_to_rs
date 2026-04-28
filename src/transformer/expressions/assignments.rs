//! Assignment and update expression conversion.

use anyhow::{anyhow, Result};
use swc_common::Spanned;
use swc_ecma_ast as ast;

use crate::ir::{BinOp, ClosureBody, Expr, RustType, Stmt};
use crate::transformer::statements::nullish_assign::{pick_strategy, NullishAssignStrategy};
use crate::transformer::{Transformer, UnsupportedSyntaxError};

impl<'a> Transformer<'a> {
    /// Converts an assignment expression (`target = value`) to `Expr::Assign`.
    pub(crate) fn convert_assign_expr(&mut self, assign: &ast::AssignExpr) -> Result<Expr> {
        // I-205 T6: plain `obj.x = v` (or `Class.x = v`) Member target は class member
        // dispatch (Write context) を経由する。setter dispatch (B3 / B4 / B8 setter) は
        // `Expr::MethodCall set_x` / `Expr::FnCall UserAssocFn set_x`、Tier 2 honest error
        // (B2 read-only / B6 method / B7 inherited) は `UnsupportedSyntaxError`、B1 field /
        // B9 unknown / static field は既存 FieldAccess Assign emission を `dispatch_member_write`
        // helper 内 fallback で統合実装。`AssignOp::Assign` (= `=`) で `MemberProp::Ident |
        // PrivateName` のみ gate (Computed `obj[i] = v` は既存 `convert_member_expr_for_write`
        // の `Expr::Index` 経路で handle、本 dispatch では unreachable)。Compound (+=, -=, etc.)
        // / nullish (??=) / logical (&&=, ||=) は subsequent T7-T9 で別途 setter dispatch 実装、
        // 本 T6 では plain `=` のみ gate。本 fix なしだと B3/B4/B8 instance/static setter で
        // existing `convert_member_expr_for_write` の FieldAccess Assign 経路に流れ、
        // `obj.x = v;` (struct field assign for non-existent field) Rust syntax で Tier 2
        // compile error を emit する状態 (= 既存 Tier 2 broken framework)。
        if assign.op == ast::AssignOp::Assign {
            if let ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(member)) = &assign.left
            {
                if matches!(
                    &member.prop,
                    ast::MemberProp::Ident(_) | ast::MemberProp::PrivateName(_)
                ) {
                    let value = self.convert_expr(&assign.right)?;
                    return self.dispatch_member_write(member, value);
                }
            }
        }

        let target = match &assign.left {
            ast::AssignTarget::Simple(simple) => match simple {
                ast::SimpleAssignTarget::Member(member) => {
                    self.convert_member_expr_for_write(member)?
                }
                ast::SimpleAssignTarget::Ident(ident) => Expr::Ident(ident.id.sym.to_string()),
                _ => return Err(anyhow!("unsupported assignment target")),
            },
            _ => return Err(anyhow!("unsupported assignment target pattern")),
        };

        // ??= (NullishAssign) — expression-context path (I-142).
        //
        // Statement-context `x ??= d;` is intercepted earlier in
        // `convert_stmt` by `try_convert_nullish_assign_stmt` and rewritten to
        // a shadow-let that preserves TS's post-`??=` narrowing. We only get
        // here when `x ??= d` appears inside a larger expression (call arg,
        // return value, ternary branch, condition, etc.), where the value of
        // the `??=` expression is observed.
        //
        // LHS-type dispatch goes through `pick_strategy` so that the Problem
        // Space matrix is encoded in exactly one place (see
        // `backlog/I-142-nullish-assign-shadow-let.md`):
        //
        // - `Option<T>` (`ShadowLet`): emit `*x.get_or_insert_with(|| d)`
        //   (Copy) or `x.get_or_insert_with(|| d).clone()` (!Copy).
        // - non-nullable `T` (`Identity`): `??=` is dead code at runtime →
        //   emit just `target` (Copy) or `target.clone()` (!Copy).
        // - `Any` (`BlockedByI050`): requires `serde_json::Value`-aware
        //   runtime null check + RHS coercion — surfaced as unsupported
        //   until the I-050 Any coercion umbrella PRD lands.
        //
        // `FieldAccess` LHS uses `get_or_insert_with` (same as Ident) or
        // `if is_none { assign Some(d) }` (stmt). `Index` LHS (HashMap) uses
        // `entry().or_insert_with()` — dispatched before `pick_strategy`
        // because HashMap ??= is key-existence, not Option null. (I-142-b/c)
        //
        // D-3: RHS conversion is strategy-local. `Identity` and `BlockedByI050`
        // never observe the RHS (dead at runtime / surfaced before emission),
        // so converting it up front would be dead work *and* could introduce
        // side-effect IR (expr-type recording, mutability marking) that TS
        // semantics don't perform — in `x ??= (y ??= z)` TS skips evaluation
        // of the inner `??=` entirely when `x` is non-null. Only `ShadowLet`
        // converts the RHS.
        if assign.op == ast::AssignOp::NullishAssign {
            // Resolve LHS type for strategy dispatch. Ident uses scoped var
            // type; Member uses TypeResolver's expr_types (populated by the
            // I-142-b/c TypeResolver extension above).
            let lhs_type = match &assign.left {
                ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(ident)) => self
                    .get_type_for_var(&ident.id.sym, ident.id.span)
                    .ok_or_else(|| {
                        UnsupportedSyntaxError::new(
                            "nullish-assign on unresolved type",
                            assign.span(),
                        )
                    })?,
                ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(member)) => self
                    .get_expr_type(&ast::Expr::Member(member.clone()))
                    .ok_or_else(|| {
                        UnsupportedSyntaxError::new(
                            "nullish-assign on unresolved member type",
                            assign.span(),
                        )
                    })?,
                _ => {
                    return Err(UnsupportedSyntaxError::new(
                        "unsupported nullish-assign target",
                        assign.span(),
                    )
                    .into())
                }
            };

            // I-142-c: Index on HashMap → entry().or_insert_with() bypasses
            // pick_strategy because HashMap ??= is key-existence, not Option null.
            if let Expr::Index {
                ref object,
                ref index,
            } = target
            {
                let right = self.convert_expr(&assign.right)?;
                let is_copy = lhs_type.is_copy_type();
                let closure = Expr::Closure {
                    params: vec![],
                    return_type: None,
                    body: ClosureBody::Expr(Box::new(right)),
                };
                // Clone the key for entry() because HashMap::entry takes
                // ownership, but the RHS closure may also reference the key
                // (e.g., `cache[key] ??= "prefix:" + key`).
                let key_for_entry = Expr::MethodCall {
                    object: Box::new(*index.clone()),
                    method: "clone".to_string(),
                    args: vec![],
                };
                let entry_call = Expr::MethodCall {
                    object: Box::new(Expr::MethodCall {
                        object: object.clone(),
                        method: "entry".to_string(),
                        args: vec![key_for_entry],
                    }),
                    method: "or_insert_with".to_string(),
                    args: vec![closure],
                };
                return Ok(if is_copy {
                    Expr::Deref(Box::new(entry_call))
                } else {
                    Expr::MethodCall {
                        object: Box::new(entry_call),
                        method: "clone".to_string(),
                        args: vec![],
                    }
                });
            }

            let strategy = pick_strategy(lhs_type);
            match strategy {
                NullishAssignStrategy::ShadowLet => {
                    let is_copy_inner = match lhs_type {
                        RustType::Option(inner) => inner.is_copy_type(),
                        _ => unreachable!("ShadowLet strategy is only picked for Option<T>"),
                    };
                    let right = self.convert_expr(&assign.right)?;
                    let closure = Expr::Closure {
                        params: vec![],
                        return_type: None,
                        body: ClosureBody::Expr(Box::new(right)),
                    };
                    let method_call = Expr::MethodCall {
                        object: Box::new(target),
                        method: "get_or_insert_with".to_string(),
                        args: vec![closure],
                    };

                    return Ok(if is_copy_inner {
                        Expr::Deref(Box::new(method_call))
                    } else {
                        Expr::MethodCall {
                            object: Box::new(method_call),
                            method: "clone".to_string(),
                            args: vec![],
                        }
                    });
                }
                NullishAssignStrategy::Identity => {
                    // `??=` on a non-nullable LHS is dead: the assign branch
                    // never fires and (D-3) the RHS is *intentionally* not
                    // converted so its side effects don't leak into IR. Emit
                    // the identity (with `.clone()` when `T: !Copy` so the
                    // expression yields an owned value rather than moving out
                    // of `ident`, matching the prior `.clone()` suffix of the
                    // ShadowLet path).
                    //
                    // INTERIM (I-048): the unconditional `.clone()` is
                    // conservative — an allocating copy is emitted even when
                    // the surrounding flow doesn't use `ident` again and a
                    // move would suffice. A precise move-vs-clone decision
                    // requires the ownership-inference umbrella (I-048); until
                    // it lands, we clone to keep the emission compile-safe.
                    let is_copy = lhs_type.is_copy_type();
                    return Ok(if is_copy {
                        target
                    } else {
                        Expr::MethodCall {
                            object: Box::new(target),
                            method: "clone".to_string(),
                            args: vec![],
                        }
                    });
                }
                NullishAssignStrategy::BlockedByI050 => {
                    return Err(UnsupportedSyntaxError::new(
                        "nullish-assign on Any LHS (I-050 Any coercion umbrella)",
                        assign.span(),
                    )
                    .into());
                }
            }
        }

        // I-161: `&&=` / `||=` expression-context desugar.
        //
        // Stmt-context `x &&= y;` is intercepted earlier in `convert_stmt` by
        // `try_convert_compound_logical_assign_stmt` and rewritten to a bare
        // `Stmt::If { cond, then_body: [assign], .. }`. We only get here when
        // the compound logical assign appears inside a larger expression, so
        // we emit a block-expression that performs the conditional assign
        // and yields the current LHS value as the tail.
        //
        // The per-type dispatch lives in `desugar_compound_logical_assign_expr`
        // (see `src/transformer/statements/compound_logical_assign.rs`) so
        // the Problem Space matrix is encoded in exactly one place.
        if matches!(
            assign.op,
            ast::AssignOp::AndAssign | ast::AssignOp::OrAssign
        ) {
            let lhs_type = match &assign.left {
                ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(ident)) => self
                    .get_type_for_var(&ident.id.sym, ident.id.span)
                    .cloned()
                    .ok_or_else(|| {
                        UnsupportedSyntaxError::new(
                            "compound logical assign on unresolved ident type",
                            assign.span(),
                        )
                    })?,
                ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(member)) => self
                    .get_expr_type(&ast::Expr::Member(member.clone()))
                    .cloned()
                    .ok_or_else(|| {
                        UnsupportedSyntaxError::new(
                            "compound logical assign on unresolved member type",
                            assign.span(),
                        )
                    })?,
                _ => {
                    return Err(UnsupportedSyntaxError::new(
                        "unsupported compound logical assign target",
                        assign.span(),
                    )
                    .into());
                }
            };
            let right = self.convert_expr(&assign.right)?;
            return self.desugar_compound_logical_assign_expr(
                target,
                right,
                &lhs_type,
                assign.op,
                assign.span(),
            );
        }

        // Non-??= path: for compound assignment (=, +=, -=, *=, /=) and plain
        // `=`, the RHS is always observed. Convert it lazily here so the ??=
        // strategy arms above can skip it when dead.
        let right = self.convert_expr(&assign.right)?;

        // For compound assignment (+=, -=, *=, /=), desugar to target = target op value
        let value = match assign.op {
            ast::AssignOp::Assign => right,
            ast::AssignOp::AddAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::Add,
                right: Box::new(right),
            },
            ast::AssignOp::SubAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::Sub,
                right: Box::new(right),
            },
            ast::AssignOp::MulAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::Mul,
                right: Box::new(right),
            },
            ast::AssignOp::DivAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::Div,
                right: Box::new(right),
            },
            ast::AssignOp::ModAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::Mod,
                right: Box::new(right),
            },
            ast::AssignOp::BitAndAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::BitAnd,
                right: Box::new(right),
            },
            ast::AssignOp::BitOrAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::BitOr,
                right: Box::new(right),
            },
            ast::AssignOp::BitXorAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::BitXor,
                right: Box::new(right),
            },
            ast::AssignOp::LShiftAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::Shl,
                right: Box::new(right),
            },
            ast::AssignOp::RShiftAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::Shr,
                right: Box::new(right),
            },
            // `AndAssign` / `OrAssign` are handled above by the I-161
            // desugar path and never reach this match arm.
            ast::AssignOp::ZeroFillRShiftAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::UShr,
                right: Box::new(right),
            },
            _ => return Err(anyhow!("unsupported compound assignment operator")),
        };
        Ok(Expr::Assign {
            target: Box::new(target),
            value: Box::new(value),
        })
    }
}

/// Converts an update expression (`i++`, `i--`, `++i`, `--i`) to `Expr::Assign`.
///
/// Both prefix and postfix forms are converted to the same assignment:
/// - `i++` / `++i` → `i = i + 1.0`
/// - `i--` / `--i` → `i = i - 1.0`
///
/// Note: In statement context, prefix/postfix distinction is irrelevant.
/// In expression context where the return value matters (e.g., `while (i--)`),
/// the prefix/postfix semantics differ, but this is not yet handled.
pub(super) fn convert_update_expr(up: &ast::UpdateExpr) -> Result<Expr> {
    let name = match up.arg.as_ref() {
        ast::Expr::Ident(ident) => ident.sym.to_string(),
        _ => return Err(anyhow!("unsupported update expression target")),
    };
    let op = match up.op {
        ast::UpdateOp::PlusPlus => BinOp::Add,
        ast::UpdateOp::MinusMinus => BinOp::Sub,
    };
    let assign = Stmt::Expr(Expr::Assign {
        target: Box::new(Expr::Ident(name.clone())),
        value: Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Ident(name.clone())),
            op,
            right: Box::new(Expr::NumberLit(1.0)),
        }),
    });

    if up.prefix {
        // Prefix: ++i → { i = i + 1.0; i }
        Ok(Expr::Block(vec![assign, Stmt::TailExpr(Expr::Ident(name))]))
    } else {
        // Postfix: i++ → { let _old = i; i = i + 1.0; _old }
        Ok(Expr::Block(vec![
            Stmt::Let {
                mutable: false,
                name: "_old".to_string(),
                ty: None,
                init: Some(Expr::Ident(name)),
            },
            assign,
            Stmt::TailExpr(Expr::Ident("_old".to_string())),
        ]))
    }
}
