//! Literal and string-related conversion helpers.
//!
//! Contains functions for converting SWC literal nodes to IR expressions
//! and utilities for detecting string types and format specifiers.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{BinOp, Expr, RustType};
use crate::registry::{TypeDef, TypeRegistry};
use crate::transformer::Transformer;

impl<'a> Transformer<'a> {
    /// Converts an SWC literal to an IR expression.
    ///
    /// When `expected` is `RustType::String`, string literals are wrapped with `.to_string()`
    /// to produce an owned `String` instead of `&str`.
    pub(crate) fn convert_lit(
        &mut self,
        lit: &ast::Lit,
        expected: Option<&RustType>,
    ) -> Result<Expr> {
        let reg = self.reg();
        match lit {
            ast::Lit::Num(n) => Ok(Expr::NumberLit(n.value)),
            ast::Lit::Str(s) => {
                let value = s.value.to_string_lossy().into_owned();
                // Check if the expected type is a string literal union enum
                if let Some(RustType::Named { name, .. }) = expected {
                    if let Some(variant) = lookup_string_enum_variant(reg, name, &value) {
                        return Ok(Expr::Ident(format!("{name}::{variant}")));
                    }
                }
                let expr = Expr::StringLit(value);
                if matches!(expected, Some(RustType::String)) {
                    Ok(Expr::MethodCall {
                        object: Box::new(expr),
                        method: "to_string".to_string(),
                        args: vec![],
                    })
                } else {
                    Ok(expr)
                }
            }
            ast::Lit::Bool(b) => Ok(Expr::BoolLit(b.value)),
            ast::Lit::Null(_) => Ok(Expr::Ident("None".to_string())),
            ast::Lit::Regex(regex) => {
                let pattern = regex.exp.to_string();
                let flags = regex.flags.to_string();
                // Embed supported flags as inline flags in the pattern
                let mut prefix = String::new();
                if flags.contains('i') {
                    prefix.push_str("(?i)");
                }
                if flags.contains('m') {
                    prefix.push_str("(?m)");
                }
                if flags.contains('s') {
                    prefix.push_str("(?s)");
                }
                // 'u' flag: Rust regex is Unicode-aware by default — no action needed.
                let full_pattern = format!("{prefix}{pattern}");
                Ok(Expr::Regex {
                    pattern: full_pattern,
                    global: flags.contains('g'),
                    sticky: flags.contains('y'),
                })
            }
            ast::Lit::BigInt(bigint) => {
                // BigInt literals (e.g., 123n) → i64 (matching TsBigIntKeyword → i64 type conversion)
                let value = bigint.value.to_string().parse::<i64>().unwrap_or(0);
                Ok(Expr::IntLit(value))
            }
            _ => Err(anyhow!("unsupported literal: {:?}", lit)),
        }
    }
}


/// 文字列リテラル値から string literal union enum のバリアント名を逆引きする。
pub(super) fn lookup_string_enum_variant<'a>(
    reg: &'a TypeRegistry,
    enum_name: &str,
    string_value: &str,
) -> Option<&'a String> {
    if let Some(TypeDef::Enum { string_values, .. }) = reg.get(enum_name) {
        string_values.get(string_value)
    } else {
        None
    }
}

/// Checks whether a RustType represents a string (including Option<String>).
pub(super) fn is_string_type(ty: &RustType) -> bool {
    matches!(ty, RustType::String)
        || matches!(ty, RustType::Option(inner) if matches!(inner.as_ref(), RustType::String))
}

/// `println!` の引数で `{:?}` (Debug) を使うべき型かどうかを判定する。
///
/// `Vec<T>`, `Option<T>`, `Tuple`, 型不明の場合は Debug フォーマットを使う。
/// プリミティブ型と Named 型（enum/struct）は Display を使う。
pub(super) fn needs_debug_format(ty: Option<&RustType>) -> bool {
    match ty {
        None => false, // 型不明の場合は Display を試みる（コンパイルエラーで発見できる）
        Some(RustType::Vec(_)) => true,
        Some(RustType::Option(_)) => true,
        Some(RustType::Tuple(_)) => true,
        _ => false,
    }
}

/// Checks whether an IR expression is known to produce a String value.
///
/// Used to detect string concatenation (`+`) and wrap the RHS in `&`.
pub(super) fn is_string_like(expr: &Expr) -> bool {
    match expr {
        Expr::StringLit(_) | Expr::FormatMacro { .. } => true,
        Expr::MethodCall { method, .. }
            if method == "to_string"
                || method == "to_uppercase"
                || method == "to_lowercase"
                || method == "trim"
                || method == "replacen" =>
        {
            true
        }
        Expr::BinaryOp {
            op: BinOp::Add,
            left,
            ..
        } => is_string_like(left),
        _ => false,
    }
}
