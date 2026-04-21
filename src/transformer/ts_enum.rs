//! TypeScript enum (`TsEnumDecl`) → IR [`Item::Enum`] conversion.
//!
//! Handles numeric enums (auto-incrementing and explicit values), string
//! enums, and constant-expression initializers (`1 << 0`, `1 | 2`, …).
//! Constant-expression initializers that exceed the simple-formatter's
//! support fall through to `value: None` (variant has no explicit value).
//!
//! The two formatter helpers ([`format_bin_expr`] / [`format_simple_expr`])
//! are kept private to this module — they only support the enum variant
//! value expression subset (numeric literal + supported binary op + unary
//! minus) and have no other consumers.

use anyhow::Result;
use swc_ecma_ast as ast;

use crate::ir::{EnumValue, EnumVariant, Item, Visibility};

/// Converts a TS enum declaration into an IR [`Item::Enum`].
///
/// Handles numeric enums (auto-incrementing and explicit values), string
/// enums, unary-minus literals, and constant binary expressions.
pub(super) fn convert_ts_enum(ts_enum: &ast::TsEnumDecl, vis: Visibility) -> Result<Vec<Item>> {
    let name = crate::ir::sanitize_rust_type_name(&ts_enum.id.sym);
    let mut variants = Vec::new();

    for member in &ts_enum.members {
        let variant_name = match &member.id {
            ast::TsEnumMemberId::Ident(ident) => ident.sym.to_string(),
            ast::TsEnumMemberId::Str(s) => s.value.to_string_lossy().into_owned(),
        };

        let value = member.init.as_ref().and_then(|init| match init.as_ref() {
            ast::Expr::Lit(ast::Lit::Num(n)) => Some(EnumValue::Number(n.value as i64)),
            ast::Expr::Lit(ast::Lit::Str(s)) => {
                Some(EnumValue::Str(s.value.to_string_lossy().into_owned()))
            }
            ast::Expr::Unary(unary) if unary.op == ast::UnaryOp::Minus => {
                if let ast::Expr::Lit(ast::Lit::Num(n)) = unary.arg.as_ref() {
                    Some(EnumValue::Number(-(n.value as i64)))
                } else {
                    None
                }
            }
            ast::Expr::Bin(bin) => format_bin_expr(bin).map(EnumValue::Expr),
            _ => None,
        });

        variants.push(EnumVariant {
            name: variant_name,
            value,
            data: None,
            fields: vec![],
        });
    }

    Ok(vec![Item::Enum {
        vis,
        name,
        type_params: vec![],
        serde_tag: None,
        variants,
    }])
}

/// Formats a binary expression AST node as a Rust expression string.
///
/// Supports numeric literals and binary operators (e.g., `1 << 0`, `1 | 2`).
/// Returns `None` for unsupported operands.
fn format_bin_expr(bin: &ast::BinExpr) -> Option<String> {
    let left = format_simple_expr(&bin.left)?;
    let right = format_simple_expr(&bin.right)?;
    let op = match bin.op {
        ast::BinaryOp::LShift => "<<",
        ast::BinaryOp::RShift => ">>",
        ast::BinaryOp::BitOr => "|",
        ast::BinaryOp::BitAnd => "&",
        ast::BinaryOp::BitXor => "^",
        ast::BinaryOp::Add => "+",
        ast::BinaryOp::Sub => "-",
        ast::BinaryOp::Mul => "*",
        _ => return None,
    };
    Some(format!("{left} {op} {right}"))
}

/// Formats a simple expression (numeric literal or nested binary) as a string.
fn format_simple_expr(expr: &ast::Expr) -> Option<String> {
    match expr {
        ast::Expr::Lit(ast::Lit::Num(n)) => Some(format!("{}", n.value as i64)),
        ast::Expr::Bin(bin) => format_bin_expr(bin),
        _ => None,
    }
}
