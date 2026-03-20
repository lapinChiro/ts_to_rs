//! Lazy type materialization for `any`-typed variables.
//!
//! Scans function bodies for typeof/instanceof usage on `any`-typed parameters
//! and generates minimal enum types to replace `serde_json::Value`.

use std::collections::HashMap;

use swc_ecma_ast as ast;

use crate::ir::{EnumVariant, Item, RustType, Visibility};

/// Constraints collected from typeof/instanceof usage on an `any`-typed variable.
#[derive(Debug, Default)]
pub(crate) struct AnyTypeConstraints {
    /// typeof strings detected (e.g., "string", "number")
    pub typeof_checks: Vec<String>,
    /// instanceof class names detected (e.g., "Date", "Error")
    pub instanceof_checks: Vec<String>,
}

impl AnyTypeConstraints {
    /// Returns true if no typeof or instanceof checks were found.
    pub(crate) fn is_empty(&self) -> bool {
        self.typeof_checks.is_empty() && self.instanceof_checks.is_empty()
    }
}

/// Scans a function body for typeof/instanceof checks on `any`-typed parameters.
///
/// Returns a map from parameter name to collected constraints.
/// Only parameters with at least one typeof/instanceof check are included.
pub(crate) fn collect_any_constraints(
    body: &ast::BlockStmt,
    any_param_names: &[String],
) -> HashMap<String, AnyTypeConstraints> {
    let mut result: HashMap<String, AnyTypeConstraints> = HashMap::new();

    for stmt in &body.stmts {
        collect_from_stmt(stmt, any_param_names, &mut result);
    }

    result
}

/// Collects any-type constraints from an expression (for expression-body arrow functions).
pub(crate) fn collect_any_constraints_from_expr(
    expr: &ast::Expr,
    any_param_names: &[String],
) -> HashMap<String, AnyTypeConstraints> {
    let mut result: HashMap<String, AnyTypeConstraints> = HashMap::new();
    collect_from_expr(expr, any_param_names, &mut result);
    result
}

/// Collects any-typed local variable names from a function body.
///
/// Scans variable declarations for explicit `any` type annotations or missing annotations.
pub(crate) fn collect_any_local_var_names(body: &ast::BlockStmt) -> Vec<String> {
    let mut names = Vec::new();
    for stmt in &body.stmts {
        if let ast::Stmt::Decl(ast::Decl::Var(var_decl)) = stmt {
            for decl in &var_decl.decls {
                if let ast::Pat::Ident(ident) = &decl.name {
                    let is_any = ident.type_ann.as_ref().is_some_and(|ann| {
                        matches!(
                            ann.type_ann.as_ref(),
                            ast::TsType::TsKeywordType(kw)
                                if kw.kind == ast::TsKeywordTypeKind::TsAnyKeyword
                        )
                    });
                    if is_any {
                        names.push(ident.id.sym.to_string());
                    }
                }
            }
        }
    }
    names
}

/// Generates an enum Item and its RustType from collected constraints.
///
/// Returns `(enum_item, rust_type)` where `rust_type` is `Named { name: enum_name }`.
pub(crate) fn generate_any_enum(
    fn_name: &str,
    param_name: &str,
    constraints: &AnyTypeConstraints,
) -> (Item, RustType) {
    let enum_name = format!(
        "{}{}Type",
        to_pascal_case(fn_name),
        to_pascal_case(param_name)
    );

    let mut variants = Vec::new();

    for typeof_str in &constraints.typeof_checks {
        let (variant_name, data_type) = match typeof_str.as_str() {
            "string" => ("String".to_string(), RustType::String),
            "number" => ("F64".to_string(), RustType::F64),
            "boolean" => ("Bool".to_string(), RustType::Bool),
            "object" => ("Object".to_string(), RustType::Any),
            "function" => ("Function".to_string(), RustType::Any),
            _ => continue,
        };
        // Avoid duplicates
        if !variants
            .iter()
            .any(|v: &EnumVariant| v.name == variant_name)
        {
            variants.push(EnumVariant {
                name: variant_name,
                value: None,
                data: Some(data_type),
                fields: vec![],
            });
        }
    }

    // Add instanceof class variants
    for class_name in &constraints.instanceof_checks {
        if !variants.iter().any(|v: &EnumVariant| v.name == *class_name) {
            variants.push(EnumVariant {
                name: class_name.clone(),
                value: None,
                data: Some(RustType::Named {
                    name: class_name.clone(),
                    type_args: vec![],
                }),
                fields: vec![],
            });
        }
    }

    // Add Other fallback variant for unmatched types
    variants.push(EnumVariant {
        name: "Other".to_string(),
        value: None,
        data: Some(RustType::Any),
        fields: vec![],
    });

    let item = Item::Enum {
        vis: Visibility::Public,
        name: enum_name.clone(),
        serde_tag: None,
        variants,
    };

    let rust_type = RustType::Named {
        name: enum_name,
        type_args: vec![],
    };

    (item, rust_type)
}

// --- AST scanning helpers ---

fn collect_from_stmt(
    stmt: &ast::Stmt,
    param_names: &[String],
    result: &mut HashMap<String, AnyTypeConstraints>,
) {
    match stmt {
        ast::Stmt::If(if_stmt) => {
            collect_from_expr(&if_stmt.test, param_names, result);
            collect_from_stmt(&if_stmt.cons, param_names, result);
            if let Some(alt) = &if_stmt.alt {
                collect_from_stmt(alt, param_names, result);
            }
        }
        ast::Stmt::Block(block) => {
            for s in &block.stmts {
                collect_from_stmt(s, param_names, result);
            }
        }
        ast::Stmt::Return(ret) => {
            if let Some(arg) = &ret.arg {
                collect_from_expr(arg, param_names, result);
            }
        }
        ast::Stmt::Expr(expr_stmt) => {
            collect_from_expr(&expr_stmt.expr, param_names, result);
        }
        _ => {}
    }
}

fn collect_from_expr(
    expr: &ast::Expr,
    param_names: &[String],
    result: &mut HashMap<String, AnyTypeConstraints>,
) {
    match expr {
        ast::Expr::Bin(bin) => {
            // typeof x === "string" pattern
            if let Some((ast::Expr::Ident(ident), type_str)) = extract_typeof_and_string(bin) {
                let name = ident.sym.to_string();
                if param_names.contains(&name) {
                    let entry = result.entry(name).or_default();
                    if !entry.typeof_checks.contains(&type_str) {
                        entry.typeof_checks.push(type_str);
                    }
                }
            }
            // x instanceof Foo pattern
            if bin.op == ast::BinaryOp::InstanceOf {
                if let ast::Expr::Ident(lhs) = bin.left.as_ref() {
                    let name = lhs.sym.to_string();
                    if param_names.contains(&name) {
                        if let ast::Expr::Ident(rhs) = bin.right.as_ref() {
                            let class_name = rhs.sym.to_string();
                            let entry = result.entry(name).or_default();
                            if !entry.instanceof_checks.contains(&class_name) {
                                entry.instanceof_checks.push(class_name);
                            }
                        }
                    }
                }
            }
            // Recurse into both sides
            collect_from_expr(&bin.left, param_names, result);
            collect_from_expr(&bin.right, param_names, result);
        }
        ast::Expr::Paren(paren) => collect_from_expr(&paren.expr, param_names, result),
        ast::Expr::Cond(cond) => {
            collect_from_expr(&cond.test, param_names, result);
            collect_from_expr(&cond.cons, param_names, result);
            collect_from_expr(&cond.alt, param_names, result);
        }
        _ => {}
    }
}

/// Extracts (typeof operand, type string) from a binary comparison.
/// Handles both `typeof x === "string"` and `"string" === typeof x`.
fn extract_typeof_and_string(bin: &ast::BinExpr) -> Option<(&ast::Expr, String)> {
    let is_eq = matches!(
        bin.op,
        ast::BinaryOp::EqEq | ast::BinaryOp::EqEqEq | ast::BinaryOp::NotEq | ast::BinaryOp::NotEqEq
    );
    if !is_eq {
        return None;
    }

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

fn to_pascal_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}
