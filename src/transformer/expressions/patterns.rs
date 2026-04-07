//! Pattern detection and comparison conversions.
//!
//! Handles `typeof`, `undefined` comparison, enum string comparison,
//! `in` operator, and `instanceof` patterns.

use swc_ecma_ast as ast;

use crate::ir::{BinOp, Expr, Pattern, RustType};
use crate::pipeline::narrowing_patterns;
use crate::registry::TypeDef;

use super::literals::lookup_string_enum_variant;
use crate::transformer::Transformer;

impl<'a> Transformer<'a> {
    /// Detects `x === undefined` / `x !== undefined` and converts to `is_none()` / `is_some()`.
    pub(crate) fn try_convert_undefined_comparison(&mut self, bin: &ast::BinExpr) -> Option<Expr> {
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
        let other_ir = self.convert_expr(other_expr).ok()?;
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
    pub(crate) fn try_convert_enum_string_comparison(
        &mut self,
        bin: &ast::BinExpr,
    ) -> Option<Expr> {
        let is_eq = matches!(bin.op, ast::BinaryOp::EqEq | ast::BinaryOp::EqEqEq);
        let is_neq = matches!(bin.op, ast::BinaryOp::NotEq | ast::BinaryOp::NotEqEq);
        if !is_eq && !is_neq {
            return None;
        }

        let op = if is_eq { BinOp::Eq } else { BinOp::NotEq };

        // Try: left is enum variable, right is string literal
        if let Some(str_value) = extract_string_lit(&bin.right) {
            if let Some(enum_name) = self.resolve_enum_type_name(&bin.left) {
                if let Some(variant) =
                    lookup_string_enum_variant(self.reg(), &enum_name, &str_value)
                {
                    // Cat A: comparison operand
                    let left = self.convert_expr(&bin.left).ok()?;
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
            if let Some(enum_name) = self.resolve_enum_type_name(&bin.right) {
                if let Some(variant) =
                    lookup_string_enum_variant(self.reg(), &enum_name, &str_value)
                {
                    // Cat A: comparison operand
                    let right = self.convert_expr(&bin.right).ok()?;
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

    /// Detects `typeof x === "type"` / `typeof x !== "type"` patterns and resolves
    /// them using FileTypeResolution. Returns `None` if the pattern is not recognized.
    pub(crate) fn try_convert_typeof_comparison(&mut self, bin: &ast::BinExpr) -> Option<Expr> {
        let is_eq = matches!(bin.op, ast::BinaryOp::EqEq | ast::BinaryOp::EqEqEq);
        let is_neq = matches!(bin.op, ast::BinaryOp::NotEq | ast::BinaryOp::NotEqEq);
        if !is_eq && !is_neq {
            return None;
        }

        // Extract (typeof operand, type string) from either order
        let (typeof_operand, type_str) = narrowing_patterns::extract_typeof_and_string(bin)?;

        // Resolve the operand's type from FileTypeResolution
        let operand_type = self.get_expr_type(typeof_operand);

        // If the operand is a union enum type, generate a matches!() expression
        if let Some(RustType::Named {
            name: enum_name, ..
        }) = operand_type
        {
            if let Some(crate::registry::TypeDef::Enum { variants, .. }) = self.reg().get(enum_name)
            {
                let expected_variant = match type_str.as_str() {
                    "string" => "String",
                    "number" => "F64",
                    "boolean" => "Bool",
                    "object" => "Object",
                    "function" => "Function",
                    _ => "",
                };
                if variants.iter().any(|v| v == expected_variant) {
                    let operand_ir = self.convert_expr(typeof_operand).ok()?;
                    let matches_expr = Expr::Matches {
                        expr: Box::new(operand_ir),
                        pattern: Box::new(Pattern::TupleStruct {
                            path: vec![enum_name.clone(), expected_variant.to_string()],
                            fields: vec![Pattern::Wildcard],
                        }),
                    };
                    return Some(if is_neq {
                        Expr::UnaryOp {
                            op: crate::ir::UnOp::Not,
                            operand: Box::new(matches_expr),
                        }
                    } else {
                        matches_expr
                    });
                }
            }
        }

        let result = match operand_type {
            Some(ty) => resolve_typeof_match(ty, &type_str),
            None => TypeofMatch::Placeholder,
        };

        let expr = match result {
            TypeofMatch::True => Expr::BoolLit(!is_neq),
            TypeofMatch::False => Expr::BoolLit(is_neq),
            TypeofMatch::IsNone => {
                // Cat A: typeof operand
                let operand_ir = self.convert_expr(typeof_operand).ok()?;
                let method = if is_neq { "is_some" } else { "is_none" };
                Expr::MethodCall {
                    object: Box::new(operand_ir),
                    method: method.to_string(),
                    args: vec![],
                }
            }
            TypeofMatch::Placeholder => {
                // Unknown operand type: treat as "type does not match" (same as False).
                // For narrowing: `if (typeof x === T)` → else branch;
                //                `if (typeof x !== T)` → then branch (both = "not T").
                // For non-narrowing (e.g. `const isT = typeof x === T`): static value
                // that may be incorrect, but strictly better than the previous todo!()
                // which always panicked.
                Expr::BoolLit(is_neq)
            }
        };

        Some(expr)
    }

    /// Converts `"key" in obj` to a Rust expression.
    ///
    /// - struct with known fields → static `true`/`false`
    /// - HashMap → `obj.contains_key("key")`
    /// - unknown type → `false` (conservative fallback)
    pub(crate) fn convert_in_operator(&mut self, bin: &ast::BinExpr) -> Expr {
        // Extract the key string from LHS (must be a string literal)
        let key = match bin.left.as_ref() {
            ast::Expr::Lit(ast::Lit::Str(s)) => s.value.to_string_lossy().into_owned(),
            _ => {
                // Non-string key in `in` operator: conservatively return false
                return Expr::BoolLit(false);
            }
        };

        // Resolve the RHS object type
        let obj_type = self.get_expr_type(&bin.right);

        match obj_type {
            Some(RustType::Named { name, .. }) if name == "HashMap" || name == "BTreeMap" => {
                // HashMap/BTreeMap → obj.contains_key("key")
                let obj_ir = match bin.right.as_ref() {
                    ast::Expr::Ident(ident) => Expr::Ident(ident.sym.as_ref().to_owned()),
                    _ => {
                        // Complex RHS expression: convert and use contains_key
                        match self.convert_expr(bin.right.as_ref()) {
                            Ok(rhs_ir) => {
                                return Expr::MethodCall {
                                    object: Box::new(rhs_ir),
                                    method: "contains_key".to_string(),
                                    args: vec![Expr::StringLit(key)],
                                };
                            }
                            Err(_) => return Expr::BoolLit(false),
                        }
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
                match self.reg().get(name) {
                    Some(TypeDef::Struct { fields, .. }) => {
                        Expr::BoolLit(fields.iter().any(|f| f.name == key))
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
                                    .any(|fields| fields.iter().any(|f| f.name == key)),
                            )
                        }
                    }
                    _ => {
                        // Type exists but shape unknown: conservatively return false
                        Expr::BoolLit(false)
                    }
                }
            }
            _ => {
                // Cannot resolve RHS type: conservatively return false
                Expr::BoolLit(false)
            }
        }
    }

    /// Converts `x instanceof ClassName` using FileTypeResolution.
    ///
    /// - Known matching type → `true`
    /// - Known non-matching type → `false`
    /// - `Option<T>` where T matches → `x.is_some()`
    /// - Unknown type → `false` (conservative fallback)
    pub(crate) fn convert_instanceof(&mut self, bin: &ast::BinExpr) -> Expr {
        // Get the class name from the RHS
        let class_name = match bin.right.as_ref() {
            ast::Expr::Ident(ident) => ident.sym.to_string(),
            _ => {
                // Non-identifier RHS (e.g., `x instanceof expr`) — conservatively false
                return Expr::BoolLit(false);
            }
        };

        // Get the LHS variable type (from FileTypeResolution, including any_enum_override)
        let lhs_type = self.get_expr_type(&bin.left).cloned();

        match lhs_type {
            Some(RustType::Any) | None => {
                // Unknown/Any operand type: conservatively return false
                Expr::BoolLit(false)
            }
            Some(RustType::Named { ref name, .. }) => {
                // Check if this is a union enum with a matching variant
                if let Some(crate::registry::TypeDef::Enum { variants, .. }) = self.reg().get(name)
                {
                    if variants.iter().any(|v| v == &class_name) {
                        let lhs_ir = match bin.left.as_ref() {
                            ast::Expr::Ident(ident) => Expr::Ident(ident.sym.to_string()),
                            _ => return Expr::BoolLit(true),
                        };
                        return Expr::Matches {
                            expr: Box::new(lhs_ir),
                            pattern: Box::new(Pattern::TupleStruct {
                                path: vec![name.clone(), class_name.clone()],
                                fields: vec![Pattern::Wildcard],
                            }),
                        };
                    }
                }
                Expr::BoolLit(*name == class_name)
            }
            Some(RustType::Option(inner)) => match inner.as_ref() {
                RustType::Named { name, .. } if name == &class_name => {
                    let lhs_ir = match bin.left.as_ref() {
                        ast::Expr::Ident(ident) => Expr::Ident(ident.sym.to_string()),
                        _ => {
                            // Complex LHS with Option<T> where T matches: convert LHS
                            match self.convert_expr(bin.left.as_ref()) {
                                Ok(lhs_ir) => {
                                    return Expr::MethodCall {
                                        object: Box::new(lhs_ir),
                                        method: "is_some".to_string(),
                                        args: vec![],
                                    };
                                }
                                Err(_) => return Expr::BoolLit(false),
                            }
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
        }
    }

    /// 式の型が string literal union enum の場合、その enum 名を返す。
    fn resolve_enum_type_name(&self, expr: &ast::Expr) -> Option<String> {
        let ty = self.get_expr_type(expr)?;
        if let RustType::Named { name, .. } = ty {
            if let Some(TypeDef::Enum { string_values, .. }) = self.reg().get(name) {
                if !string_values.is_empty() {
                    return Some(name.clone());
                }
            }
        }
        None
    }

    /// Resolves a typeof string to an enum variant name.
    ///
    /// Given a variable type like `Named { name: "StringOrF64" }` and typeof string `"string"`,
    /// looks up the enum in TypeRegistry and finds the variant that matches.
    pub(crate) fn resolve_typeof_to_enum_variant(
        &self,
        var_type: &RustType,
        typeof_str: &str,
    ) -> Option<(String, String)> {
        let enum_name = match var_type {
            RustType::Named { name, type_args } if type_args.is_empty() => name,
            _ => return None,
        };
        let type_def = self.reg().get(enum_name)?;
        let variants = match type_def {
            crate::registry::TypeDef::Enum { variants, .. } => variants,
            _ => return None,
        };
        let expected_variant = match typeof_str {
            "string" => "String",
            "number" => "F64",
            "boolean" => "Bool",
            "object" => "Object",
            "function" => "Function",
            _ => return None,
        };
        if variants.iter().any(|v| v == expected_variant) {
            Some((enum_name.clone(), expected_variant.to_string()))
        } else {
            None
        }
    }

    /// Resolves an instanceof class name to an enum variant.
    fn resolve_instanceof_to_enum_variant(
        &self,
        var_type: &RustType,
        class_name: &str,
    ) -> Option<(String, String)> {
        let enum_name = match var_type {
            RustType::Named { name, type_args } if type_args.is_empty() => name,
            _ => return None,
        };
        let type_def = self.reg().get(enum_name)?;
        let variants = match type_def {
            crate::registry::TypeDef::Enum { variants, .. } => variants,
            _ => return None,
        };
        if variants.iter().any(|v| v == class_name) {
            Some((enum_name.clone(), class_name.to_string()))
        } else {
            None
        }
    }

    /// Resolves the complement pattern for a narrowing guard.
    ///
    /// The complement pattern matches the variable when the positive guard does NOT match.
    /// Used to generate `match` arms that extract values for both the positive and complement cases.
    ///
    /// Returns `Some(pattern_string)` or `None` if no complement pattern can be determined.
    /// - Option: complement of `Some(x)` is `None`
    /// - 2-variant enum: complement of `Enum::A(x)` is `Enum::B(x)`
    /// - 3+ variant enum: returns `None` (no specific complement variant to extract)
    pub(crate) fn resolve_complement_pattern(&self, guard: &NarrowingGuard) -> Option<Pattern> {
        let var_type = self.get_type_for_var(guard.var_name(), guard.var_span())?;
        match guard {
            NarrowingGuard::NonNullish { .. } | NarrowingGuard::Truthy { .. } => {
                if matches!(var_type, RustType::Option(_)) {
                    Some(Pattern::none())
                } else {
                    None
                }
            }
            NarrowingGuard::Typeof { type_name, .. } => {
                let (enum_name, positive_variant) =
                    self.resolve_typeof_to_enum_variant(var_type, type_name)?;
                self.resolve_other_variant(&enum_name, &positive_variant, guard.var_name())
            }
            NarrowingGuard::InstanceOf { class_name, .. } => {
                let (enum_name, positive_variant) =
                    self.resolve_instanceof_to_enum_variant(var_type, class_name)?;
                self.resolve_other_variant(&enum_name, &positive_variant, guard.var_name())
            }
        }
    }

    /// Resolves the complement variant pattern for a 2-variant enum.
    ///
    /// Returns `Some(Pattern::TupleStruct { path: [EnumName, OtherVariant], ... })`
    /// for 2-variant enums, or `None` for 3+ variant enums (caller should use
    /// wildcard).
    fn resolve_other_variant(
        &self,
        enum_name: &str,
        positive_variant: &str,
        var_name: &str,
    ) -> Option<Pattern> {
        let type_def = self.reg().get(enum_name)?;
        let variants = match type_def {
            crate::registry::TypeDef::Enum { variants, .. } => variants,
            _ => return None,
        };
        // Filter out the positive variant and "Other" (any-narrowing fallback)
        let remaining: Vec<_> = variants
            .iter()
            .filter(|v| v.as_str() != positive_variant && v.as_str() != "Other")
            .collect();
        if remaining.len() == 1 {
            Some(Pattern::TupleStruct {
                path: vec![enum_name.to_string(), remaining[0].clone()],
                fields: vec![Pattern::binding(var_name)],
            })
        } else {
            // 3+ variant or 0 remaining: no specific complement pattern
            None
        }
    }

    /// NarrowingGuard から if-let `Pattern` を解決する。
    ///
    /// Returns `Some((pattern, is_swap))` where `is_swap` is true for `!==`/`!=` guards
    /// (meaning then/else branches should be swapped).
    /// Returns `None` if the guard cannot generate an if-let pattern.
    pub(crate) fn resolve_if_let_pattern(&self, guard: &NarrowingGuard) -> Option<(Pattern, bool)> {
        let var_type = self.get_type_for_var(guard.var_name(), guard.var_span())?;
        match guard {
            NarrowingGuard::NonNullish { is_neq, .. } => {
                if matches!(var_type, RustType::Option(_)) {
                    Some((Pattern::some_binding(guard.var_name()), !is_neq))
                } else {
                    None
                }
            }
            NarrowingGuard::Truthy { .. } => {
                if matches!(var_type, RustType::Option(_)) {
                    Some((Pattern::some_binding(guard.var_name()), false))
                } else {
                    None
                }
            }
            NarrowingGuard::Typeof {
                type_name, is_eq, ..
            } => {
                let (enum_name, variant) =
                    self.resolve_typeof_to_enum_variant(var_type, type_name)?;
                Some((
                    Pattern::TupleStruct {
                        path: vec![enum_name, variant],
                        fields: vec![Pattern::binding(guard.var_name())],
                    },
                    !is_eq,
                ))
            }
            NarrowingGuard::InstanceOf { class_name, .. } => {
                let (enum_name, variant) =
                    self.resolve_instanceof_to_enum_variant(var_type, class_name)?;
                Some((
                    Pattern::TupleStruct {
                        path: vec![enum_name, variant],
                        fields: vec![Pattern::binding(guard.var_name())],
                    },
                    false,
                ))
            }
        }
    }
}

/// 式から文字列リテラル値を抽出する。
fn extract_string_lit(expr: &ast::Expr) -> Option<String> {
    if let ast::Expr::Lit(ast::Lit::Str(s)) = expr {
        Some(s.value.to_string_lossy().into_owned())
    } else {
        None
    }
}

/// Returns true if the expression is the `undefined` identifier.
fn is_undefined_ident(expr: &ast::Expr) -> bool {
    matches!(expr, ast::Expr::Ident(ident) if ident.sym.as_ref() == "undefined")
}

/// Result of matching a `RustType` against a typeof string.
enum TypeofMatch {
    /// The type definitely matches the typeof string.
    True,
    /// The type definitely does not match.
    False,
    /// The type is `Option<T>` and typeof is `"undefined"` — convert to `.is_none()`.
    IsNone,
    /// The operand type is unknown — conservatively treated as "no match".
    Placeholder,
}

/// Resolves whether a RustType matches a typeof string.
fn resolve_typeof_match(ty: &RustType, typeof_str: &str) -> TypeofMatch {
    if matches!(ty, RustType::Any) {
        return TypeofMatch::Placeholder;
    }
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

/// Resolves typeof to a string literal based on FileTypeResolution type.
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

/// A narrowing guard extracted from an if-condition.
#[derive(Debug)]
pub(crate) enum NarrowingGuard {
    /// `typeof x === "string"` (or "number", "boolean", etc.)
    Typeof {
        var_name: String,
        /// The span of the variable Ident in the condition AST.
        var_span: swc_common::Span,
        type_name: String,
        /// true if the comparison is `===`/`==` (narrows in then branch)
        /// false if `!==`/`!=` (narrows in else branch)
        is_eq: bool,
    },
    /// `x !== null` / `x !== undefined`
    NonNullish {
        var_name: String,
        /// The span of the variable Ident in the condition AST.
        var_span: swc_common::Span,
        is_neq: bool,
    },
    /// `if (x)` — truthy check, narrows `Option<T>` to `T`
    Truthy {
        var_name: String,
        /// The span of the variable Ident in the condition AST.
        var_span: swc_common::Span,
    },
    /// `x instanceof Foo`
    InstanceOf {
        var_name: String,
        /// The span of the variable Ident in the condition AST.
        var_span: swc_common::Span,
        class_name: String,
    },
}

impl NarrowingGuard {
    /// Returns the variable name being narrowed.
    pub(crate) fn var_name(&self) -> &str {
        match self {
            NarrowingGuard::Typeof { var_name, .. }
            | NarrowingGuard::Truthy { var_name, .. }
            | NarrowingGuard::InstanceOf { var_name, .. }
            | NarrowingGuard::NonNullish { var_name, .. } => var_name,
        }
    }

    /// Returns the SWC span of the variable Ident in the condition AST.
    pub(crate) fn var_span(&self) -> swc_common::Span {
        match self {
            NarrowingGuard::Typeof { var_span, .. }
            | NarrowingGuard::Truthy { var_span, .. }
            | NarrowingGuard::InstanceOf { var_span, .. }
            | NarrowingGuard::NonNullish { var_span, .. } => *var_span,
        }
    }
}

/// Result of extracting narrowing guards from a compound condition.
///
/// For `typeof x === "string" && typeof y === "number"`, returns two guards.
/// For `typeof x === "string" && x.length > 0`, returns one guard and one remaining condition.
#[derive(Debug)]
pub(crate) struct CompoundGuards<'a> {
    /// Narrowing guards paired with their original AST expressions.
    pub guards: Vec<(NarrowingGuard, &'a ast::Expr)>,
    /// Sub-expressions in the && chain that are not narrowing guards.
    pub remaining: Vec<&'a ast::Expr>,
}

/// Extracts all narrowing guards from a `&&`-connected condition.
///
/// Recursively traverses `LogicalAnd` nodes and collects guards.
/// Non-guard sub-expressions are collected in `remaining`.
pub(crate) fn extract_narrowing_guards(condition: &ast::Expr) -> CompoundGuards<'_> {
    let mut result = CompoundGuards {
        guards: Vec::new(),
        remaining: Vec::new(),
    };
    collect_guards_from_and(condition, &mut result);
    result
}

fn collect_guards_from_and<'a>(expr: &'a ast::Expr, result: &mut CompoundGuards<'a>) {
    if let ast::Expr::Bin(bin) = expr {
        if bin.op == ast::BinaryOp::LogicalAnd {
            collect_guards_from_and(&bin.left, result);
            collect_guards_from_and(&bin.right, result);
            return;
        }
    }
    // Not a LogicalAnd — try to extract a single guard
    if let Some(guard) = extract_narrowing_guard(expr) {
        result.guards.push((guard, expr));
    } else {
        result.remaining.push(expr);
    }
}

/// Extracts a narrowing guard from an if-condition expression.
pub(crate) fn extract_narrowing_guard(condition: &ast::Expr) -> Option<NarrowingGuard> {
    match condition {
        ast::Expr::Bin(bin) => {
            // instanceof check: if (x instanceof Foo)
            if bin.op == ast::BinaryOp::InstanceOf {
                if let (ast::Expr::Ident(lhs), ast::Expr::Ident(rhs)) =
                    (bin.left.as_ref(), bin.right.as_ref())
                {
                    return Some(NarrowingGuard::InstanceOf {
                        var_name: lhs.sym.to_string(),
                        var_span: lhs.span,
                        class_name: rhs.sym.to_string(),
                    });
                }
                return None;
            }

            let is_eq = matches!(bin.op, ast::BinaryOp::EqEq | ast::BinaryOp::EqEqEq);
            let is_neq = matches!(bin.op, ast::BinaryOp::NotEq | ast::BinaryOp::NotEqEq);
            if !is_eq && !is_neq {
                return None;
            }

            // typeof x === "type"
            if let Some((ast::Expr::Ident(ident), type_str)) =
                narrowing_patterns::extract_typeof_and_string(bin)
            {
                return Some(NarrowingGuard::Typeof {
                    var_name: ident.sym.to_string(),
                    var_span: ident.span,
                    type_name: type_str,
                    is_eq,
                });
            }

            // x !== null / x !== undefined
            let (var_expr, is_nullish) = if narrowing_patterns::is_null_or_undefined(&bin.right) {
                (Some(&*bin.left), true)
            } else if narrowing_patterns::is_null_or_undefined(&bin.left) {
                (Some(&*bin.right), true)
            } else {
                (None, false)
            };
            if is_nullish {
                if let Some(ast::Expr::Ident(ident)) = var_expr {
                    return Some(NarrowingGuard::NonNullish {
                        var_name: ident.sym.to_string(),
                        var_span: ident.span,
                        is_neq,
                    });
                }
            }

            None
        }
        // Truthy check: if (x) where x is a simple identifier
        ast::Expr::Ident(ident) => {
            let name = ident.sym.to_string();
            // Exclude keywords that aren't variables
            if name == "undefined" || name == "null" || name == "true" || name == "false" {
                return None;
            }
            Some(NarrowingGuard::Truthy {
                var_name: name,
                var_span: ident.span,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_typescript;
    use swc_ecma_ast::{ModuleItem, Stmt};

    /// Helper: parse a TS expression statement and return the SWC Expr.
    fn parse_expr(source: &str) -> ast::Expr {
        let module = parse_typescript(source).expect("parse failed");
        match &module.body[0] {
            ModuleItem::Stmt(Stmt::Expr(expr_stmt)) => *expr_stmt.expr.clone(),
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn test_extract_narrowing_guard_typeof_returns_typeof_guard() {
        // typeof x === "string" → Typeof guard
        let expr = parse_expr(r#"typeof x === "string""#);
        let guard = extract_narrowing_guard(&expr).expect("should extract Typeof guard");
        assert!(
            matches!(&guard, NarrowingGuard::Typeof { var_name, type_name, is_eq, .. }
                if var_name == "x" && type_name == "string" && *is_eq),
            "expected Typeof guard, got: {guard:?}"
        );
    }

    #[test]
    fn test_extract_narrowing_guard_null_check_returns_non_nullish() {
        // x !== null → NonNullish(is_neq=true)
        let expr = parse_expr("x !== null");
        let guard = extract_narrowing_guard(&expr).expect("should extract NonNullish guard");
        assert!(
            matches!(&guard, NarrowingGuard::NonNullish { var_name, is_neq, .. }
                if var_name == "x" && *is_neq),
            "expected NonNullish with is_neq=true, got: {guard:?}"
        );
    }

    #[test]
    fn test_extract_narrowing_guard_reversed_null_returns_non_nullish() {
        // null !== x → NonNullish (reversed order)
        let expr = parse_expr("null !== x");
        let guard = extract_narrowing_guard(&expr).expect("should extract NonNullish guard");
        assert!(
            matches!(&guard, NarrowingGuard::NonNullish { var_name, is_neq, .. }
                if var_name == "x" && *is_neq),
            "expected NonNullish for reversed null !== x, got: {guard:?}"
        );
    }

    #[test]
    fn test_extract_narrowing_guard_instanceof_returns_instanceof_guard() {
        // x instanceof Foo → InstanceOf guard
        let expr = parse_expr("x instanceof Foo");
        let guard = extract_narrowing_guard(&expr).expect("should extract InstanceOf guard");
        assert!(
            matches!(&guard, NarrowingGuard::InstanceOf { var_name, class_name, .. }
                if var_name == "x" && class_name == "Foo"),
            "expected InstanceOf guard, got: {guard:?}"
        );
    }

    #[test]
    fn test_extract_narrowing_guard_keyword_ident_returns_none() {
        // CRITICAL: undefined / true / false should NOT produce Truthy guards.
        // If they did, it would cause a silent semantic change.
        assert!(
            extract_narrowing_guard(&parse_expr("undefined")).is_none(),
            "undefined should not produce a Truthy guard"
        );
        assert!(
            extract_narrowing_guard(&parse_expr("true")).is_none(),
            "true should not produce a Truthy guard"
        );
        assert!(
            extract_narrowing_guard(&parse_expr("false")).is_none(),
            "false should not produce a Truthy guard"
        );
        assert!(
            extract_narrowing_guard(&parse_expr("null")).is_none(),
            "null should not produce a Truthy guard"
        );
    }

    #[test]
    fn test_extract_narrowing_guard_non_bin_non_ident_returns_none() {
        // Numeric literal, string literal, etc. → None
        assert!(
            extract_narrowing_guard(&parse_expr("42")).is_none(),
            "numeric literal should not produce a guard"
        );
        assert!(
            extract_narrowing_guard(&parse_expr(r#""hello""#)).is_none(),
            "string literal should not produce a guard"
        );
    }
}
