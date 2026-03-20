//! Pattern detection and comparison conversions.
//!
//! Handles `typeof`, `undefined` comparison, enum string comparison,
//! `in` operator, and `instanceof` patterns.

use swc_ecma_ast as ast;

use crate::ir::{BinOp, Expr, RustType};
use crate::registry::{TypeDef, TypeRegistry};
use crate::transformer::TypeEnv;

use super::literals::lookup_string_enum_variant;
use super::type_resolution::resolve_expr_type;

/// Detects `typeof x === "type"` / `typeof x !== "type"` patterns and resolves
/// them using TypeEnv. Returns `None` if the pattern is not recognized.
/// Detects `x === undefined` / `x !== undefined` and converts to `is_none()` / `is_some()`.
pub(super) fn try_convert_undefined_comparison(
    bin: &ast::BinExpr,
    type_env: &TypeEnv,
    reg: &TypeRegistry,
) -> Option<Expr> {
    let is_eq = matches!(bin.op, ast::BinaryOp::EqEq | ast::BinaryOp::EqEqEq);
    let is_neq = matches!(bin.op, ast::BinaryOp::NotEq | ast::BinaryOp::NotEqEq);
    if !is_eq && !is_neq {
        return None;
    }

    // Extract the non-undefined side
    let other_expr = if is_undefined_ident(&bin.right) {
        Some(bin.left.as_ref())
    } else if is_undefined_ident(&bin.left) {
        Some(bin.right.as_ref())
    } else {
        None
    }?;

    // Cat A: comparison operand
    let other_ir =
        super::convert_expr(other_expr, reg, &super::ExprContext::none(), type_env).ok()?;
    let method = if is_eq { "is_none" } else { "is_some" };
    Some(Expr::MethodCall {
        object: Box::new(other_ir),
        method: method.to_string(),
        args: vec![],
    })
}

/// 等値比較で一方が string literal union enum 型の場合、文字列リテラル側を enum バリアントに変換する。
///
/// `d == "up"` → `d == Direction::Up`、`"up" != d` → `Direction::Up != d`
pub(super) fn try_convert_enum_string_comparison(
    bin: &ast::BinExpr,
    type_env: &TypeEnv,
    reg: &TypeRegistry,
) -> Option<Expr> {
    let is_eq = matches!(bin.op, ast::BinaryOp::EqEq | ast::BinaryOp::EqEqEq);
    let is_neq = matches!(bin.op, ast::BinaryOp::NotEq | ast::BinaryOp::NotEqEq);
    if !is_eq && !is_neq {
        return None;
    }

    let op = if is_eq { BinOp::Eq } else { BinOp::NotEq };

    // Try: left is enum variable, right is string literal
    if let Some(str_value) = extract_string_lit(&bin.right) {
        if let Some(enum_name) = resolve_enum_type_name(&bin.left, type_env, reg) {
            if let Some(variant) = lookup_string_enum_variant(reg, &enum_name, &str_value) {
                // Cat A: comparison operand
                let left =
                    super::convert_expr(&bin.left, reg, &super::ExprContext::none(), type_env)
                        .ok()?;
                return Some(Expr::BinaryOp {
                    left: Box::new(left),
                    op,
                    right: Box::new(Expr::Ident(format!("{enum_name}::{variant}"))),
                });
            }
        }
    }

    // Try: left is string literal, right is enum variable
    if let Some(str_value) = extract_string_lit(&bin.left) {
        if let Some(enum_name) = resolve_enum_type_name(&bin.right, type_env, reg) {
            if let Some(variant) = lookup_string_enum_variant(reg, &enum_name, &str_value) {
                // Cat A: comparison operand
                let right =
                    super::convert_expr(&bin.right, reg, &super::ExprContext::none(), type_env)
                        .ok()?;
                return Some(Expr::BinaryOp {
                    left: Box::new(Expr::Ident(format!("{enum_name}::{variant}"))),
                    op,
                    right: Box::new(right),
                });
            }
        }
    }

    None
}

/// 式から文字列リテラル値を抽出する。
fn extract_string_lit(expr: &ast::Expr) -> Option<String> {
    if let ast::Expr::Lit(ast::Lit::Str(s)) = expr {
        Some(s.value.to_string_lossy().into_owned())
    } else {
        None
    }
}

/// 式の型が string literal union enum の場合、その enum 名を返す。
fn resolve_enum_type_name(
    expr: &ast::Expr,
    type_env: &TypeEnv,
    reg: &TypeRegistry,
) -> Option<String> {
    let ty = resolve_expr_type(expr, type_env, reg)?;
    if let RustType::Named { name, .. } = &ty {
        if let Some(TypeDef::Enum { string_values, .. }) = reg.get(name) {
            if !string_values.is_empty() {
                return Some(name.clone());
            }
        }
    }
    None
}

/// Returns true if the expression is the `undefined` identifier.
fn is_undefined_ident(expr: &ast::Expr) -> bool {
    matches!(expr, ast::Expr::Ident(ident) if ident.sym.as_ref() == "undefined")
}

/// Detects `typeof x === "type"` / `typeof x !== "type"` patterns and resolves
/// them using TypeEnv. Returns `None` if the pattern is not recognized.
pub(super) fn try_convert_typeof_comparison(
    bin: &ast::BinExpr,
    type_env: &TypeEnv,
    reg: &TypeRegistry,
) -> Option<Expr> {
    let is_eq = matches!(bin.op, ast::BinaryOp::EqEq | ast::BinaryOp::EqEqEq);
    let is_neq = matches!(bin.op, ast::BinaryOp::NotEq | ast::BinaryOp::NotEqEq);
    if !is_eq && !is_neq {
        return None;
    }

    // Extract (typeof operand, type string) from either order
    let (typeof_operand, type_str) = extract_typeof_and_string(bin)?;

    // Resolve the operand's type from TypeEnv
    let operand_type = resolve_expr_type(typeof_operand, type_env, reg);

    let result = match &operand_type {
        Some(ty) => resolve_typeof_match(ty, &type_str),
        None => TypeofMatch::Placeholder,
    };

    let expr = match result {
        TypeofMatch::True => Expr::BoolLit(!is_neq),
        TypeofMatch::False => Expr::BoolLit(is_neq),
        TypeofMatch::IsNone => {
            // Cat A: typeof operand
            let operand_ir =
                super::convert_expr(typeof_operand, reg, &super::ExprContext::none(), type_env)
                    .ok()?;
            let method = if is_neq { "is_some" } else { "is_none" };
            Expr::MethodCall {
                object: Box::new(operand_ir),
                method: method.to_string(),
                args: vec![],
            }
        }
        TypeofMatch::Placeholder => {
            // Unknown type → optimistic true (negated for !==)
            Expr::BoolLit(!is_neq)
        }
    };

    Some(expr)
}

/// Extracts the typeof operand and the comparison string from a binary expression.
/// Handles both `typeof x === "string"` and `"string" === typeof x`.
fn extract_typeof_and_string(bin: &ast::BinExpr) -> Option<(&ast::Expr, String)> {
    // Left is typeof, right is string
    if let ast::Expr::Unary(unary) = bin.left.as_ref() {
        if unary.op == ast::UnaryOp::TypeOf {
            if let ast::Expr::Lit(ast::Lit::Str(s)) = bin.right.as_ref() {
                return Some((&unary.arg, s.value.to_string_lossy().into_owned()));
            }
        }
    }
    // Right is typeof, left is string
    if let ast::Expr::Unary(unary) = bin.right.as_ref() {
        if unary.op == ast::UnaryOp::TypeOf {
            if let ast::Expr::Lit(ast::Lit::Str(s)) = bin.left.as_ref() {
                return Some((&unary.arg, s.value.to_string_lossy().into_owned()));
            }
        }
    }
    None
}

/// Result of matching a `RustType` against a typeof string.
enum TypeofMatch {
    /// The type definitely matches the typeof string.
    True,
    /// The type definitely does not match.
    False,
    /// The type is `Option<T>` and typeof is `"undefined"` — convert to `.is_none()`.
    IsNone,
    /// The operand type is unknown — use optimistic fallback.
    Placeholder,
}

/// Resolves whether a RustType matches a typeof string.
fn resolve_typeof_match(ty: &RustType, typeof_str: &str) -> TypeofMatch {
    match typeof_str {
        "string" => {
            if matches!(ty, RustType::String) {
                TypeofMatch::True
            } else {
                TypeofMatch::False
            }
        }
        "number" => {
            if matches!(ty, RustType::F64) {
                TypeofMatch::True
            } else {
                TypeofMatch::False
            }
        }
        "boolean" => {
            if matches!(ty, RustType::Bool) {
                TypeofMatch::True
            } else {
                TypeofMatch::False
            }
        }
        "undefined" => {
            if matches!(ty, RustType::Option(_)) {
                TypeofMatch::IsNone
            } else {
                TypeofMatch::False
            }
        }
        "object" => {
            if matches!(ty, RustType::Named { .. } | RustType::Vec(_)) {
                TypeofMatch::True
            } else {
                TypeofMatch::False
            }
        }
        "function" => {
            if matches!(ty, RustType::Fn { .. }) {
                TypeofMatch::True
            } else {
                TypeofMatch::False
            }
        }
        _ => TypeofMatch::False,
    }
}

/// Converts `"key" in obj` to a Rust expression.
///
/// - struct with known fields → static `true`/`false`
/// - HashMap → `obj.contains_key("key")`
/// - unknown type → `todo!()` (no silent `true` fallback)
pub(super) fn convert_in_operator(
    bin: &ast::BinExpr,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Expr {
    // Extract the key string from LHS (must be a string literal)
    let key = match bin.left.as_ref() {
        ast::Expr::Lit(ast::Lit::Str(s)) => s.value.to_string_lossy().into_owned(),
        _ => {
            return Expr::FnCall {
                name: "todo!".to_string(),
                args: vec![Expr::StringLit(
                    "in operator with non-string key".to_string(),
                )],
            };
        }
    };

    // Resolve the RHS object type
    let obj_type = resolve_expr_type(&bin.right, type_env, reg);

    match &obj_type {
        Some(RustType::Named { name, .. }) if name == "HashMap" || name == "BTreeMap" => {
            // HashMap/BTreeMap → obj.contains_key("key")
            let obj_ir = match bin.right.as_ref() {
                ast::Expr::Ident(ident) => Expr::Ident(ident.sym.as_ref().to_owned()),
                _ => {
                    return Expr::FnCall {
                        name: "todo!".to_string(),
                        args: vec![Expr::StringLit(
                            "in operator with complex RHS expression".to_string(),
                        )],
                    };
                }
            };
            Expr::MethodCall {
                object: Box::new(obj_ir),
                method: "contains_key".to_string(),
                args: vec![Expr::StringLit(key)],
            }
        }
        Some(RustType::Named { name, .. }) => {
            // Check TypeRegistry for field existence
            match reg.get(name) {
                Some(TypeDef::Struct { fields, .. }) => {
                    Expr::BoolLit(fields.iter().any(|(f, _)| f == &key))
                }
                Some(TypeDef::Enum {
                    tag_field,
                    variant_fields,
                    ..
                }) => {
                    // discriminated union: check if any variant has this field
                    if tag_field.as_deref() == Some(key.as_str()) {
                        Expr::BoolLit(true) // tag field always exists
                    } else {
                        Expr::BoolLit(
                            variant_fields
                                .values()
                                .any(|fields| fields.iter().any(|(f, _)| f == &key)),
                        )
                    }
                }
                _ => Expr::FnCall {
                    name: "todo!".to_string(),
                    args: vec![Expr::StringLit(format!(
                        "in operator — type '{name}' has unknown shape"
                    ))],
                },
            }
        }
        _ => Expr::FnCall {
            name: "todo!".to_string(),
            args: vec![Expr::StringLit(format!(
                "in operator — cannot resolve type of RHS for key '{key}'"
            ))],
        },
    }
}

/// Converts `x instanceof ClassName` using TypeEnv.
///
/// - Known matching type → `true`
/// - Known non-matching type → `false`
/// - `Option<T>` where T matches → `x.is_some()`
/// - Unknown type → `todo!()` placeholder (compile error, not silent `true`)
pub(super) fn convert_instanceof(bin: &ast::BinExpr, type_env: &TypeEnv) -> Expr {
    // Get the class name from the RHS
    let class_name = match bin.right.as_ref() {
        ast::Expr::Ident(ident) => ident.sym.to_string(),
        _ => {
            // Non-identifier RHS (e.g., `x instanceof expr`) — cannot resolve statically
            return Expr::FnCall {
                name: "todo!".to_string(),
                args: vec![Expr::StringLit(
                    "instanceof with non-identifier RHS".to_string(),
                )],
            };
        }
    };

    // Get the LHS variable name and type
    let lhs_type = match bin.left.as_ref() {
        ast::Expr::Ident(ident) => type_env.get(ident.sym.as_ref()).cloned(),
        _ => None,
    };

    match lhs_type {
        Some(RustType::Named { name, .. }) => Expr::BoolLit(name == class_name),
        Some(RustType::Option(inner)) => match inner.as_ref() {
            RustType::Named { name, .. } if name == &class_name => {
                let lhs_ir = match bin.left.as_ref() {
                    ast::Expr::Ident(ident) => Expr::Ident(ident.sym.to_string()),
                    _ => {
                        return Expr::FnCall {
                            name: "todo!".to_string(),
                            args: vec![Expr::StringLit("instanceof with complex LHS".to_string())],
                        };
                    }
                };
                Expr::MethodCall {
                    object: Box::new(lhs_ir),
                    method: "is_some".to_string(),
                    args: vec![],
                }
            }
            _ => Expr::BoolLit(false),
        },
        Some(_) => Expr::BoolLit(false),
        // Unknown type: generate todo!() instead of silent true
        None => Expr::FnCall {
            name: "todo!".to_string(),
            args: vec![Expr::StringLit(format!(
                "instanceof {class_name} — type unknown"
            ))],
        },
    }
}

/// Resolves typeof to a string literal based on TypeEnv type.
pub(super) fn typeof_to_string(ty: &RustType) -> &'static str {
    match ty {
        RustType::String => "string",
        RustType::F64 => "number",
        RustType::Bool => "boolean",
        RustType::Option(_) => "undefined",
        RustType::Named { .. } | RustType::Vec(_) => "object",
        RustType::Fn { .. } => "function",
        _ => "object",
    }
}
