//! Binary and unary expression conversion.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{BinOp, Expr, RustType, UnOp};
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::TypeRegistry;
use crate::transformer::TypeEnv;

use super::literals::{is_string_like, is_string_type};
use super::patterns::{
    convert_in_operator, convert_instanceof, try_convert_enum_string_comparison,
    try_convert_typeof_comparison, try_convert_undefined_comparison, typeof_to_string,
};
use super::type_resolution::resolve_expr_type;
use crate::ir::ClosureBody;

use super::convert_expr;
use crate::transformer::context::TransformContext;

/// Converts a binary expression to IR, handling special patterns like
/// nullish coalescing, typeof/undefined comparisons, and string concatenation.
pub(super) fn convert_bin_expr(
    bin: &ast::BinExpr,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    expected: Option<&RustType>,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    // typeof x === "type" / typeof x !== "type" pattern
    if let Some(result) = try_convert_typeof_comparison(bin, type_env, tctx, reg, synthetic) {
        return Ok(result);
    }

    // x === undefined / x !== undefined pattern
    if let Some(result) = try_convert_undefined_comparison(bin, type_env, tctx, reg, synthetic) {
        return Ok(result);
    }

    // string literal enum comparison: d == "up" → d == Direction::Up
    if let Some(result) = try_convert_enum_string_comparison(bin, type_env, tctx, reg, synthetic) {
        return Ok(result);
    }

    // x instanceof ClassName pattern
    if bin.op == ast::BinaryOp::InstanceOf {
        return Ok(convert_instanceof(bin, type_env, tctx, reg));
    }

    // "key" in obj pattern
    if bin.op == ast::BinaryOp::In {
        return Ok(convert_in_operator(bin, tctx, reg, type_env));
    }

    // `x ?? y` → `x.unwrap_or_else(|| y)` (Option) or `x` (non-Option)
    if bin.op == ast::BinaryOp::NullishCoalescing {
        let left_type = resolve_expr_type(&bin.left, type_env, tctx, reg);
        let is_option = left_type
            .as_ref()
            .is_some_and(|ty| matches!(ty, RustType::Option(_)));

        // Cat A: ?? left operand — type is resolved separately for Option detection
        let left = convert_expr(&bin.left, tctx, reg, type_env, synthetic)?;
        if !is_option && left_type.is_some() {
            // Non-Option type: nullish coalescing is a no-op, return left as-is
            return Ok(left);
        }
        let right = convert_expr(&bin.right, tctx, reg, type_env, synthetic)?;
        return Ok(Expr::MethodCall {
            object: Box::new(left),
            method: "unwrap_or_else".to_string(),
            args: vec![Expr::Closure {
                params: vec![],
                return_type: None,
                body: ClosureBody::Expr(Box::new(right)),
            }],
        });
    }

    // Cat A: binary operands — result type depends on operator, not context
    let left = convert_expr(&bin.left, tctx, reg, type_env, synthetic)?;
    let right = convert_expr(&bin.right, tctx, reg, type_env, synthetic)?;
    let op = convert_binary_op(bin.op)?;

    // String concatenation: wrap RHS in Ref(&) when LHS is string-like.
    // Priority: type inference → expected type → IR heuristic (is_string_like fallback).
    let is_string_context = if op == BinOp::Add {
        let left_type = resolve_expr_type(&bin.left, type_env, tctx, reg);
        let type_inferred = left_type.is_some_and(|ty| is_string_type(&ty));
        type_inferred || matches!(expected, Some(RustType::String)) || is_string_like(&left)
    } else {
        false
    };

    // Mixed-type concatenation: one side is string, other is known non-string → format!
    // Handles: `42 + " px"` (f64 + &str) and `"val: " + x` (String + f64)
    if op == BinOp::Add && is_string_context {
        let left_type = resolve_expr_type(&bin.left, type_env, tctx, reg);
        let right_type = resolve_expr_type(&bin.right, type_env, tctx, reg);
        let left_is_string =
            left_type.as_ref().is_some_and(is_string_type) || is_string_like(&left);
        let left_known_non_string = (left_type.is_some()
            && !left_type.as_ref().is_some_and(is_string_type))
            && !is_string_like(&left);
        let right_known_non_string = (right_type.is_some()
            && !right_type.as_ref().is_some_and(is_string_type))
            && !is_string_like(&right);

        if (left_known_non_string && !left_is_string) || (right_known_non_string && left_is_string)
        {
            return Ok(Expr::FormatMacro {
                template: "{}{}".to_string(),
                args: vec![left, right],
            });
        }
    }

    // In string concat context:
    // - LHS StringLit needs .to_string() (Rust: &str can't use + operator directly)
    // - LHS self.field needs .clone() (Rust: can't move out of &self)
    // - RHS non-literal needs & (Rust: String + &str)
    let left = if is_string_context && matches!(left, Expr::StringLit(_)) {
        Expr::MethodCall {
            object: Box::new(left),
            method: "to_string".to_string(),
            args: vec![],
        }
    } else if is_string_context
        && matches!(
            &left,
            Expr::FieldAccess { object, .. } if matches!(object.as_ref(), Expr::Ident(name) if name == "self")
        )
    {
        Expr::MethodCall {
            object: Box::new(left),
            method: "clone".to_string(),
            args: vec![],
        }
    } else {
        left
    };

    let right = if is_string_context && !matches!(right, Expr::StringLit(_)) {
        Expr::Ref(Box::new(right))
    } else {
        right
    };

    Ok(Expr::BinaryOp {
        left: Box::new(left),
        op,
        right: Box::new(right),
    })
}

/// Converts an SWC binary operator to an IR [`BinOp`].
pub(crate) fn convert_binary_op(op: ast::BinaryOp) -> Result<BinOp> {
    match op {
        ast::BinaryOp::Add => Ok(BinOp::Add),
        ast::BinaryOp::Sub => Ok(BinOp::Sub),
        ast::BinaryOp::Mul => Ok(BinOp::Mul),
        ast::BinaryOp::Div => Ok(BinOp::Div),
        ast::BinaryOp::Mod => Ok(BinOp::Mod),
        ast::BinaryOp::EqEq | ast::BinaryOp::EqEqEq => Ok(BinOp::Eq),
        ast::BinaryOp::NotEq | ast::BinaryOp::NotEqEq => Ok(BinOp::NotEq),
        ast::BinaryOp::Lt => Ok(BinOp::Lt),
        ast::BinaryOp::LtEq => Ok(BinOp::LtEq),
        ast::BinaryOp::Gt => Ok(BinOp::Gt),
        ast::BinaryOp::GtEq => Ok(BinOp::GtEq),
        ast::BinaryOp::LogicalAnd => Ok(BinOp::LogicalAnd),
        ast::BinaryOp::LogicalOr => Ok(BinOp::LogicalOr),
        ast::BinaryOp::BitAnd => Ok(BinOp::BitAnd),
        ast::BinaryOp::BitOr => Ok(BinOp::BitOr),
        ast::BinaryOp::BitXor => Ok(BinOp::BitXor),
        ast::BinaryOp::LShift => Ok(BinOp::Shl),
        ast::BinaryOp::RShift => Ok(BinOp::Shr),
        ast::BinaryOp::ZeroFillRShift => Ok(BinOp::UShr),
        _ => Err(anyhow!("unsupported binary operator: {:?}", op)),
    }
}

/// Converts a unary expression (`!x`, `-x`, `typeof x`) to IR.
pub(super) fn convert_unary_expr(
    unary: &ast::UnaryExpr,
    tctx: &TransformContext<'_>,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Expr> {
    // typeof x → resolve based on TypeEnv
    if unary.op == ast::UnaryOp::TypeOf {
        let operand_type = resolve_expr_type(&unary.arg, type_env, tctx, reg);
        return Ok(match operand_type {
            Some(RustType::Option(inner)) => {
                // Option<T>: runtime branch — is_some() → typeof inner, else "undefined"
                // Cat A: typeof operand — only used for type discrimination
                let operand = convert_expr(&unary.arg, tctx, reg, type_env, synthetic)?;
                let inner_typeof = typeof_to_string(&inner);
                Expr::If {
                    condition: Box::new(Expr::MethodCall {
                        object: Box::new(operand),
                        method: "is_some".to_string(),
                        args: vec![],
                    }),
                    then_expr: Box::new(Expr::StringLit(inner_typeof.to_string())),
                    else_expr: Box::new(Expr::StringLit("undefined".to_string())),
                }
            }
            Some(ty) => Expr::StringLit(typeof_to_string(&ty).to_string()),
            None => Expr::StringLit("object".to_string()),
        });
    }

    // Unary plus: +x → numeric conversion
    if unary.op == ast::UnaryOp::Plus {
        let operand_type = resolve_expr_type(&unary.arg, type_env, tctx, reg);
        let operand = convert_expr(&unary.arg, tctx, reg, type_env, synthetic)?;
        return Ok(match operand_type {
            Some(RustType::F64) => operand, // already numeric, identity
            Some(RustType::String) => Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(operand),
                    method: "parse::<f64>".to_string(),
                    args: vec![],
                }),
                method: "unwrap".to_string(),
                args: vec![],
            },
            _ => operand, // fallback: return as-is, let compiler catch type errors
        });
    }

    let op = match unary.op {
        ast::UnaryOp::Bang => UnOp::Not,
        ast::UnaryOp::Minus => UnOp::Neg,
        _ => return Err(anyhow!("unsupported unary operator: {:?}", unary.op)),
    };
    // Cat A: unary operand — type depends on operator semantics
    let operand = convert_expr(&unary.arg, tctx, reg, type_env, synthetic)?;
    Ok(Expr::UnaryOp {
        op,
        operand: Box::new(operand),
    })
}
