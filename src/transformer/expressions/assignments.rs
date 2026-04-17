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
        // `FieldAccess` / `Index` LHS are reserved for separate PRDs
        // (I-142-b / I-142-c) — they need distinct designs (borrow-check +
        // match rewrite for fields, entry API for indexed containers) and
        // are surfaced as unsupported here rather than silently emitting
        // broken `get_or_insert_with` calls on a non-`Option` receiver.
        //
        // D-3: RHS conversion is strategy-local. `Identity` and `BlockedByI050`
        // never observe the RHS (dead at runtime / surfaced before emission),
        // so converting it up front would be dead work *and* could introduce
        // side-effect IR (expr-type recording, mutability marking) that TS
        // semantics don't perform — in `x ??= (y ??= z)` TS skips evaluation
        // of the inner `??=` entirely when `x` is non-null. Only `ShadowLet`
        // converts the RHS.
        if assign.op == ast::AssignOp::NullishAssign {
            let ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(ident)) = &assign.left
            else {
                return Err(UnsupportedSyntaxError::new(
                    "nullish-assign on member/index target (I-142-b/c)",
                    assign.span(),
                )
                .into());
            };
            // Dispatch on the LHS type via the shared strategy table. Missing
            // type info (unresolved) is symmetric with the stmt path: it
            // surfaces as unsupported rather than silently falling through.
            let lhs_type = self
                .get_type_for_var(&ident.id.sym, ident.id.span)
                .ok_or_else(|| {
                    UnsupportedSyntaxError::new("nullish-assign on unresolved type", assign.span())
                })?;
            let strategy = pick_strategy(lhs_type);
            match strategy {
                NullishAssignStrategy::ShadowLet => {
                    // Inner `T` of `Option<T>` decides Copy-ness of the deref
                    // path. Resolve to a `bool` *before* calling
                    // `convert_expr` so the `lhs_type` shared borrow is
                    // released before the `&mut self` re-borrow.
                    let is_copy_inner = match lhs_type {
                        RustType::Option(inner) => inner.is_copy_type(),
                        _ => unreachable!("ShadowLet strategy is only picked for Option<T>"),
                    };
                    let right = self.convert_expr(&assign.right)?;
                    let method_call = Expr::MethodCall {
                        object: Box::new(target),
                        method: "get_or_insert_with".to_string(),
                        args: vec![Expr::Closure {
                            params: vec![],
                            return_type: None,
                            body: ClosureBody::Expr(Box::new(right)),
                        }],
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
            ast::AssignOp::AndAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::LogicalAnd,
                right: Box::new(right),
            },
            ast::AssignOp::OrAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::LogicalOr,
                right: Box::new(right),
            },
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
