//! Binary and unary expression conversion.

use anyhow::{anyhow, Result};
use swc_common::Spanned;
use swc_ecma_ast as ast;

use crate::ir::{BinOp, Expr, RustType, UnOp};

use super::literals::{is_string_like, is_string_type};
use super::patterns::typeof_to_string;
use crate::transformer::helpers::coerce_default::{
    build_option_coerce_to_string, build_option_coerce_to_t,
};
use crate::transformer::Transformer;

impl<'a> Transformer<'a> {
    /// Converts a binary expression to IR, handling special patterns like
    /// nullish coalescing, typeof/undefined comparisons, and string concatenation.
    pub(crate) fn convert_bin_expr(
        &mut self,
        bin: &ast::BinExpr,
        expected: Option<&RustType>,
    ) -> Result<Expr> {
        // typeof x === "type" / typeof x !== "type" pattern
        if let Some(result) = self.try_convert_typeof_comparison(bin) {
            return Ok(result);
        }

        // x === undefined / x !== undefined pattern
        if let Some(result) = self.try_convert_undefined_comparison(bin) {
            return Ok(result);
        }

        // string literal enum comparison: d == "up" → d == Direction::Up
        if let Some(result) = self.try_convert_enum_string_comparison(bin) {
            return Ok(result);
        }

        // x instanceof ClassName pattern
        if bin.op == ast::BinaryOp::InstanceOf {
            return Ok(self.convert_instanceof(bin));
        }

        // "key" in obj pattern
        if bin.op == ast::BinaryOp::In {
            return Ok(self.convert_in_operator(bin));
        }

        // `x ?? y` emission (I-022):
        // - LHS Option + RHS non-Option  → `x.unwrap_or(y)` / `x.unwrap_or_else(|| y)`
        // - LHS Option + RHS Option      → `x.or(y)` / `x.or_else(|| y)` (chain case)
        // - LHS definitively non-Option  → short-circuit return LHS (TS: `??` is no-op)
        //
        // `is_option_left` combines the TS-inferred type with a structural IR check
        // via `produces_option_result`, catching `arr[i]` (emitted as `.get().cloned()`
        // via `resolve_bin_expr` LHS span propagation) and wrapped `Some(_)` literals.
        if bin.op == ast::BinaryOp::NullishCoalescing {
            // Cat A: ?? left operand — type is resolved separately for Option detection
            let left = self.convert_expr(&bin.left)?;
            let left_type = self.get_expr_type(&bin.left);
            let is_option_left = left_type.is_some_and(|ty| matches!(ty, RustType::Option(_)))
                || super::produces_option_result(&left);

            // Short-circuit: LHS is definitively non-Option (known static type + no
            // Option-producing IR shape). TS `??` with non-null LHS evaluates to LHS.
            if !is_option_left && left_type.is_some() {
                return Ok(left);
            }

            let right = self.convert_expr(&bin.right)?;
            let right_type = self.get_expr_type(&bin.right);
            let is_option_right = right_type.is_some_and(|ty| matches!(ty, RustType::Option(_)))
                || super::produces_option_result(&right);

            // RHS is also Option<T> (chain `a ?? b ?? c` inner case): preserve Option
            // via `.or()` / `.or_else()` so outer `??` can terminate with unwrap_or.
            if is_option_right {
                return Ok(crate::transformer::build_option_or_option(left, right));
            }
            return Ok(crate::transformer::build_option_unwrap_with_default(
                left, right,
            ));
        }

        // Cat A: binary operands — result type depends on operator, not context
        let left = self.convert_expr(&bin.left)?;
        let right = self.convert_expr(&bin.right)?;
        let op = convert_binary_op(bin.op)?;

        // String concatenation: wrap RHS in Ref(&) when LHS is string-like.
        // Priority: type inference → expected type → IR heuristic (is_string_like fallback).
        let is_string_context = if op == BinOp::Add {
            let left_type = self.get_expr_type(&bin.left);
            let type_inferred = left_type.is_some_and(is_string_type);
            type_inferred || matches!(expected, Some(RustType::String)) || is_string_like(&left)
        } else {
            false
        };

        // Mixed-type concatenation: one side is string, other is known non-string → format!
        // Handles: `42 + " px"` (f64 + &str) and `"val: " + x` (String + f64)
        if op == BinOp::Add && is_string_context {
            let left_type = self.get_expr_type(&bin.left);
            let right_type = self.get_expr_type(&bin.right);
            let left_is_string = left_type.is_some_and(is_string_type) || is_string_like(&left);
            let left_known_non_string = (left_type.is_some()
                && !left_type.is_some_and(is_string_type))
                && !is_string_like(&left);
            let right_known_non_string = (right_type.is_some()
                && !right_type.is_some_and(is_string_type))
                && !is_string_like(&right);

            if (left_known_non_string && !left_is_string)
                || (right_known_non_string && left_is_string)
            {
                // I-144 T6-2: coerce closure-reassigned Option<T> args to String
                // via the JS coerce_default table (`null` → `"null"`).
                let left = self.maybe_coerce_for_string_concat(&bin.left, left);
                let right = self.maybe_coerce_for_string_concat(&bin.right, right);
                return Ok(Expr::FormatMacro {
                    template: "{}{}".to_string(),
                    args: vec![left, right],
                });
            }
        }

        // I-144 T6-2: arithmetic context (`+`/`-`/`*`/`/`/`%`) — when an Ident
        // operand refers to a closure-reassigned `Option<T>` var, wrap with the
        // JS coerce_default value (`null` → `0.0` for `F64`) so post-stale
        // reads reproduce JS runtime semantics (`null + 1 = 1`).
        let (left, right) = if matches!(
            op,
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod
        ) && !is_string_context
        {
            (
                self.maybe_coerce_for_arith(&bin.left, left),
                self.maybe_coerce_for_arith(&bin.right, right),
            )
        } else {
            (left, right)
        };

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

    /// I-144 T6-2: if `ast_expr` is an `Ident` referring to a closure-reassigned
    /// `Option<T>` variable (narrow suppressed), wrap `ir_expr` with the
    /// JS coerce_default value via `unwrap_or` so an arithmetic read site
    /// reproduces JS runtime semantics. Otherwise returns `ir_expr` unchanged.
    fn maybe_coerce_for_arith(&self, ast_expr: &ast::Expr, ir_expr: Expr) -> Expr {
        let ast::Expr::Ident(id) = ast_expr else {
            return ir_expr;
        };
        if !self.is_var_closure_reassigned(id.sym.as_ref(), ast_expr.span().lo.0) {
            return ir_expr;
        }
        let Some(RustType::Option(inner)) = self.get_expr_type(ast_expr) else {
            return ir_expr;
        };
        build_option_coerce_to_t(ir_expr.clone(), inner).unwrap_or(ir_expr)
    }

    /// I-144 T6-2: same as [`maybe_coerce_for_arith`] but emits the string-context
    /// coerce shape (`x.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string())`).
    fn maybe_coerce_for_string_concat(&self, ast_expr: &ast::Expr, ir_expr: Expr) -> Expr {
        let ast::Expr::Ident(id) = ast_expr else {
            return ir_expr;
        };
        if !self.is_var_closure_reassigned(id.sym.as_ref(), ast_expr.span().lo.0) {
            return ir_expr;
        }
        let Some(RustType::Option(inner)) = self.get_expr_type(ast_expr) else {
            return ir_expr;
        };
        build_option_coerce_to_string(ir_expr.clone(), inner).unwrap_or(ir_expr)
    }

    /// Converts a unary expression (`!x`, `-x`, `typeof x`) to IR.
    pub(crate) fn convert_unary_expr(&mut self, unary: &ast::UnaryExpr) -> Result<Expr> {
        // typeof x → resolve based on FileTypeResolution
        if unary.op == ast::UnaryOp::TypeOf {
            let operand_type = self.get_expr_type(&unary.arg);
            return match operand_type {
                Some(RustType::Option(inner)) => {
                    // Option<T>: runtime branch — is_some() → typeof inner, else "undefined"
                    let operand = self.convert_expr(&unary.arg)?;
                    let inner_typeof = typeof_to_string(inner);
                    Ok(Expr::If {
                        condition: Box::new(Expr::MethodCall {
                            object: Box::new(operand),
                            method: "is_some".to_string(),
                            args: vec![],
                        }),
                        then_expr: Box::new(Expr::StringLit(inner_typeof.to_string())),
                        else_expr: Box::new(Expr::StringLit("undefined".to_string())),
                    })
                }
                Some(RustType::Any) => {
                    // Any type: runtime typeof via js_typeof helper
                    let operand = self.convert_expr(&unary.arg)?;
                    Ok(Expr::RuntimeTypeof {
                        operand: Box::new(operand),
                    })
                }
                Some(ty) => Ok(Expr::StringLit(typeof_to_string(ty).to_string())),
                None => {
                    // Type unresolvable: report as unsupported instead of silent "object"
                    Err(super::super::UnsupportedSyntaxError::new(
                        "typeof on unresolved type",
                        unary.span,
                    )
                    .into())
                }
            };
        }

        // Unary plus: +x → numeric conversion
        if unary.op == ast::UnaryOp::Plus {
            let operand_type = self.get_expr_type(&unary.arg);
            let operand = self.convert_expr(&unary.arg)?;
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
        let operand = self.convert_expr(&unary.arg)?;
        Ok(Expr::UnaryOp {
            op,
            operand: Box::new(operand),
        })
    }
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
