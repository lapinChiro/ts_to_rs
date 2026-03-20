//! Assignment and update expression conversion.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{BinOp, ClosureBody, Expr, Stmt};
use crate::registry::TypeRegistry;
use crate::transformer::TypeEnv;

use super::member_access::convert_member_expr;
use super::{convert_expr, ExprContext};

/// Converts an assignment expression (`target = value`) to `Expr::Assign`.
pub(super) fn convert_assign_expr(
    assign: &ast::AssignExpr,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Result<Expr> {
    // Extract target variable name for type lookup before converting target expr
    let target_var_name = match &assign.left {
        ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(ident)) => {
            Some(ident.id.sym.to_string())
        }
        _ => None,
    };
    let target = match &assign.left {
        ast::AssignTarget::Simple(simple) => match simple {
            ast::SimpleAssignTarget::Member(member) => convert_member_expr(member, reg, type_env)?,
            ast::SimpleAssignTarget::Ident(ident) => Expr::Ident(ident.id.sym.to_string()),
            _ => return Err(anyhow!("unsupported assignment target")),
        },
        _ => return Err(anyhow!("unsupported assignment target pattern")),
    };
    // Propagate target variable's type from TypeEnv to RHS (Category B)
    let rhs_ctx = match &target_var_name {
        Some(name) => match type_env.get(name) {
            Some(ty) => ExprContext::with_expected(ty),
            None => ExprContext::none(),
        },
        None => ExprContext::none(),
    };
    let right = convert_expr(&assign.right, reg, &rhs_ctx, type_env)?;

    // ??= (nullish coalescing assignment): x ??= y → x.get_or_insert_with(|| y)
    if assign.op == ast::AssignOp::NullishAssign {
        return Ok(Expr::MethodCall {
            object: Box::new(target),
            method: "get_or_insert_with".to_string(),
            args: vec![Expr::Closure {
                params: vec![],
                return_type: None,
                body: ClosureBody::Expr(Box::new(right)),
            }],
        });
    }

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
