//! Lazy type materialization for `any`-typed variables.
//!
//! Scans function bodies for typeof/instanceof usage on `any`-typed parameters
//! and generates minimal enum types to replace `serde_json::Value`.

use std::collections::HashMap;

use swc_ecma_ast as ast;

use crate::ir::{EnumVariant, RustType};
use crate::pipeline::narrowing_patterns;

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
                    let is_any_or_unknown = ident.type_ann.as_ref().is_some_and(|ann| {
                        matches!(
                            ann.type_ann.as_ref(),
                            ast::TsType::TsKeywordType(kw)
                                if matches!(kw.kind,
                                    ast::TsKeywordTypeKind::TsAnyKeyword
                                    | ast::TsKeywordTypeKind::TsUnknownKeyword
                                )
                        )
                    });
                    if is_any_or_unknown {
                        names.push(ident.id.sym.to_string());
                    }
                }
            }
        }
    }
    names
}

/// Returns the variants for an any-narrowing enum based on collected typeof/instanceof constraints.
///
/// Callers should register these via `SyntheticTypeRegistry::register_any_enum()`.
pub(crate) fn build_any_enum_variants(constraints: &AnyTypeConstraints) -> Vec<EnumVariant> {
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
        let sanitized = crate::ir::sanitize_rust_type_name(class_name);
        if !variants.iter().any(|v: &EnumVariant| v.name == sanitized) {
            variants.push(EnumVariant {
                name: sanitized.clone(),
                value: None,
                data: Some(RustType::Named {
                    name: sanitized,
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

    variants
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
        ast::Stmt::Switch(switch) => {
            // Detect `switch (typeof x) { case "string": ... }` pattern
            if let ast::Expr::Unary(unary) = switch.discriminant.as_ref() {
                if unary.op == ast::UnaryOp::TypeOf {
                    if let ast::Expr::Ident(ident) = unary.arg.as_ref() {
                        let name = ident.sym.to_string();
                        if param_names.contains(&name) {
                            for case in &switch.cases {
                                if let Some(ast::Expr::Lit(ast::Lit::Str(s))) = case.test.as_deref()
                                {
                                    let type_str = s.value.to_string_lossy().into_owned();
                                    let entry = result.entry(name.clone()).or_default();
                                    if !entry.typeof_checks.contains(&type_str) {
                                        entry.typeof_checks.push(type_str);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            collect_from_expr(&switch.discriminant, param_names, result);
            for case in &switch.cases {
                if let Some(test) = &case.test {
                    collect_from_expr(test, param_names, result);
                }
                for s in &case.cons {
                    collect_from_stmt(s, param_names, result);
                }
            }
        }
        ast::Stmt::While(while_stmt) => {
            collect_from_expr(&while_stmt.test, param_names, result);
            collect_from_stmt(&while_stmt.body, param_names, result);
        }
        ast::Stmt::DoWhile(do_while) => {
            collect_from_stmt(&do_while.body, param_names, result);
            collect_from_expr(&do_while.test, param_names, result);
        }
        ast::Stmt::For(for_stmt) => {
            collect_from_stmt(&for_stmt.body, param_names, result);
        }
        ast::Stmt::ForOf(for_of) => {
            collect_from_stmt(&for_of.body, param_names, result);
        }
        ast::Stmt::ForIn(for_in) => {
            collect_from_stmt(&for_in.body, param_names, result);
        }
        ast::Stmt::Try(try_stmt) => {
            for s in &try_stmt.block.stmts {
                collect_from_stmt(s, param_names, result);
            }
            if let Some(handler) = &try_stmt.handler {
                for s in &handler.body.stmts {
                    collect_from_stmt(s, param_names, result);
                }
            }
            if let Some(finalizer) = &try_stmt.finalizer {
                for s in &finalizer.stmts {
                    collect_from_stmt(s, param_names, result);
                }
            }
        }
        ast::Stmt::Labeled(labeled) => {
            collect_from_stmt(&labeled.body, param_names, result);
        }
        ast::Stmt::Throw(throw_stmt) => {
            collect_from_expr(&throw_stmt.arg, param_names, result);
        }
        ast::Stmt::Decl(ast::Decl::Var(var_decl)) => {
            for decl in &var_decl.decls {
                if let Some(init) = &decl.init {
                    collect_from_expr(init, param_names, result);
                }
            }
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
            if let Some((ast::Expr::Ident(ident), type_str)) =
                narrowing_patterns::extract_typeof_and_string(bin)
            {
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

/// Converts a snake_case or kebab-case string to PascalCase.
///
/// `foo_bar` → `FooBar`, `my-name` → `MyName`, `hello` → `Hello`
pub(crate) fn to_pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;
    for ch in s.chars() {
        if ch == '_' || ch == '-' {
            capitalize_next = true;
        } else if capitalize_next {
            result.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_typescript;

    /// Parses a function body from TS source like `function f(x: any) { ... }`.
    /// Returns the BlockStmt of the first function declaration found.
    fn parse_fn_body(source: &str) -> ast::BlockStmt {
        let module = parse_typescript(source).unwrap();
        for item in &module.body {
            if let ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl))) = item {
                if let Some(body) = &fn_decl.function.body {
                    return body.clone();
                }
            }
        }
        panic!("no function body found in source");
    }

    /// Parses and extracts the first expression from `function f() { return <expr>; }`.
    fn parse_expr(source: &str) -> Box<ast::Expr> {
        let fn_source = format!("function f() {{ return {source}; }}");
        let body = parse_fn_body(&fn_source);
        for stmt in &body.stmts {
            if let ast::Stmt::Return(ret) = stmt {
                if let Some(arg) = &ret.arg {
                    return arg.clone();
                }
            }
        }
        panic!("no return expression found");
    }

    // --- collect_any_constraints tests ---

    #[test]
    fn test_collect_constraints_typeof_boolean_detected() {
        let body = parse_fn_body(r#"function f(x: any) { if (typeof x === "boolean") { } }"#);
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result.get("x").expect("should have constraints for x");
        assert!(
            c.typeof_checks.contains(&"boolean".to_string()),
            "typeof 'boolean' should be collected, got: {:?}",
            c.typeof_checks
        );
    }

    #[test]
    fn test_collect_constraints_typeof_object_detected() {
        let body = parse_fn_body(r#"function f(x: any) { if (typeof x === "object") { } }"#);
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result.get("x").expect("should have constraints for x");
        assert!(
            c.typeof_checks.contains(&"object".to_string()),
            "typeof 'object' should be collected, got: {:?}",
            c.typeof_checks
        );
    }

    #[test]
    fn test_collect_constraints_typeof_function_detected() {
        let body = parse_fn_body(r#"function f(x: any) { if (typeof x === "function") { } }"#);
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result.get("x").expect("should have constraints for x");
        assert!(
            c.typeof_checks.contains(&"function".to_string()),
            "typeof 'function' should be collected, got: {:?}",
            c.typeof_checks
        );
    }

    #[test]
    fn test_collect_constraints_instanceof_detected() {
        let body = parse_fn_body(r#"function f(x: any) { if (x instanceof Date) { } }"#);
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result.get("x").expect("should have constraints for x");
        assert!(
            c.instanceof_checks.contains(&"Date".to_string()),
            "instanceof 'Date' should be collected, got: {:?}",
            c.instanceof_checks
        );
    }

    #[test]
    fn test_collect_constraints_strict_not_equal_still_collects() {
        let body = parse_fn_body(r#"function f(x: any) { if (typeof x !== "string") { } }"#);
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result.get("x").expect("should have constraints for x");
        assert!(
            c.typeof_checks.contains(&"string".to_string()),
            "!== should still collect the constraint"
        );
    }

    #[test]
    fn test_collect_constraints_loose_not_equal_still_collects() {
        let body = parse_fn_body(r#"function f(x: any) { if (typeof x != "number") { } }"#);
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result.get("x").expect("should have constraints for x");
        assert!(
            c.typeof_checks.contains(&"number".to_string()),
            "!= should still collect the constraint"
        );
    }

    #[test]
    fn test_collect_constraints_in_else_branch_detected() {
        let body = parse_fn_body(
            r#"function f(x: any) { if (true) { } else { if (typeof x === "number") { } } }"#,
        );
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result.get("x").expect("should have constraints for x");
        assert!(
            c.typeof_checks.contains(&"number".to_string()),
            "typeof in else branch should be collected"
        );
    }

    #[test]
    fn test_collect_constraints_multiple_typeof_all_collected() {
        let body = parse_fn_body(
            r#"function f(x: any) {
                if (typeof x === "string") { }
                if (typeof x === "number") { }
            }"#,
        );
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result.get("x").expect("should have constraints for x");
        assert_eq!(
            c.typeof_checks.len(),
            2,
            "should collect both typeof checks"
        );
        assert!(
            c.typeof_checks.contains(&"string".to_string()),
            "should contain 'string'"
        );
        assert!(
            c.typeof_checks.contains(&"number".to_string()),
            "should contain 'number'"
        );
    }

    #[test]
    fn test_collect_constraints_unknown_typeof_string_collected() {
        // Unknown typeof strings (e.g. "symbol") are collected by collect_any_constraints;
        // build_any_enum_variants is responsible for filtering them.
        let body = parse_fn_body(r#"function f(x: any) { if (typeof x === "symbol") { } }"#);
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result.get("x").expect("should have constraints for x");
        assert!(
            c.typeof_checks.contains(&"symbol".to_string()),
            "unknown typeof string should still be collected as a constraint"
        );
    }

    #[test]
    fn test_collect_constraints_non_tracked_param_not_collected() {
        let body = parse_fn_body(r#"function f(x: any) { if (typeof y === "string") { } }"#);
        let result = collect_any_constraints(&body, &["x".to_string()]);
        assert!(
            !result.contains_key("y"),
            "y is not in param_names so should not be collected"
        );
    }

    #[test]
    fn test_collect_constraints_reversed_typeof_order_detected() {
        // "string" === typeof x (right-hand typeof)
        let body = parse_fn_body(r#"function f(x: any) { if ("string" === typeof x) { } }"#);
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result
            .get("x")
            .expect("should detect reversed typeof pattern");
        assert!(
            c.typeof_checks.contains(&"string".to_string()),
            "reversed typeof pattern should be detected"
        );
    }

    #[test]
    fn test_collect_constraints_in_return_expr_detected() {
        let body = parse_fn_body(r#"function f(x: any) { return typeof x === "string"; }"#);
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result
            .get("x")
            .expect("should detect typeof in return expr");
        assert!(
            c.typeof_checks.contains(&"string".to_string()),
            "typeof in return expression should be detected"
        );
    }

    #[test]
    fn test_collect_constraints_duplicate_typeof_deduplicated() {
        let body = parse_fn_body(
            r#"function f(x: any) {
                if (typeof x === "string") { }
                if (typeof x === "string") { }
            }"#,
        );
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result.get("x").unwrap();
        assert_eq!(
            c.typeof_checks.iter().filter(|s| *s == "string").count(),
            1,
            "duplicate typeof checks should be deduplicated"
        );
    }

    // --- collect_any_local_var_names tests ---

    #[test]
    fn test_local_var_any_type_detected() {
        let body = parse_fn_body(r#"function f() { let x: any = 1; }"#);
        let names = collect_any_local_var_names(&body);
        assert_eq!(names, vec!["x".to_string()]);
    }

    #[test]
    fn test_local_var_non_any_type_excluded() {
        let body = parse_fn_body(r#"function f() { let x: string = "hello"; }"#);
        let names = collect_any_local_var_names(&body);
        assert!(names.is_empty(), "non-any variables should be excluded");
    }

    #[test]
    fn test_local_var_no_annotation_excluded() {
        let body = parse_fn_body(r#"function f() { let x = 1; }"#);
        let names = collect_any_local_var_names(&body);
        assert!(
            names.is_empty(),
            "variables without type annotation should be excluded"
        );
    }

    #[test]
    fn test_local_var_multiple_any_all_collected() {
        let body = parse_fn_body(
            r#"function f() { let a: any = 1; let b: string = ""; let c: any = 2; }"#,
        );
        let names = collect_any_local_var_names(&body);
        assert_eq!(names, vec!["a".to_string(), "c".to_string()]);
    }

    #[test]
    fn test_local_var_unknown_type_detected() {
        // I-333: unknown-typed local variables should be collected like any-typed ones
        let body = parse_fn_body(r#"function f() { let x: unknown = 1; }"#);
        let names = collect_any_local_var_names(&body);
        assert_eq!(names, vec!["x".to_string()]);
    }

    // --- build_any_enum_variants tests ---

    #[test]
    fn test_build_variants_typeof_string_produces_string_variant() {
        let constraints = AnyTypeConstraints {
            typeof_checks: vec!["string".to_string()],
            instanceof_checks: vec![],
        };
        let variants = build_any_enum_variants(&constraints);
        assert_eq!(variants.len(), 2, "should produce String + Other");
        assert_eq!(variants[0].name, "String");
        assert!(
            matches!(variants[0].data, Some(RustType::String)),
            "String variant data should be RustType::String, got: {:?}",
            variants[0].data
        );
        assert_eq!(variants[1].name, "Other");
    }

    #[test]
    fn test_build_variants_instanceof_produces_named_variant() {
        let constraints = AnyTypeConstraints {
            typeof_checks: vec![],
            instanceof_checks: vec!["Date".to_string()],
        };
        let variants = build_any_enum_variants(&constraints);
        assert_eq!(variants.len(), 2, "should produce Date + Other");
        assert_eq!(variants[0].name, "Date");
        assert!(
            matches!(&variants[0].data, Some(RustType::Named { name, .. }) if name == "Date"),
            "Date variant data should be Named('Date'), got: {:?}",
            variants[0].data
        );
        assert_eq!(variants[1].name, "Other");
    }

    #[test]
    fn test_build_variants_duplicate_typeof_produces_single_variant() {
        let constraints = AnyTypeConstraints {
            typeof_checks: vec!["string".to_string(), "string".to_string()],
            instanceof_checks: vec![],
        };
        let variants = build_any_enum_variants(&constraints);
        assert_eq!(
            variants.len(),
            2,
            "duplicate typeof should produce one variant + Other"
        );
        assert_eq!(variants[0].name, "String");
        assert_eq!(variants[1].name, "Other");
    }

    #[test]
    fn test_build_variants_empty_constraints_produces_only_other() {
        let constraints = AnyTypeConstraints {
            typeof_checks: vec![],
            instanceof_checks: vec![],
        };
        let variants = build_any_enum_variants(&constraints);
        assert_eq!(
            variants.len(),
            1,
            "empty constraints should produce only Other"
        );
        assert_eq!(variants[0].name, "Other");
    }

    #[test]
    fn test_build_variants_all_typeof_types_produces_correct_order() {
        let constraints = AnyTypeConstraints {
            typeof_checks: vec![
                "string".to_string(),
                "number".to_string(),
                "boolean".to_string(),
                "object".to_string(),
                "function".to_string(),
            ],
            instanceof_checks: vec!["Error".to_string()],
        };
        let variants = build_any_enum_variants(&constraints);
        assert_eq!(variants.len(), 7, "5 typeof + 1 instanceof + Other");
        let names: Vec<&str> = variants.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["String", "F64", "Bool", "Object", "Function", "Error", "Other"]
        );
    }

    #[test]
    fn test_build_variants_unknown_typeof_skipped_in_output() {
        let constraints = AnyTypeConstraints {
            typeof_checks: vec!["symbol".to_string(), "string".to_string()],
            instanceof_checks: vec![],
        };
        let variants = build_any_enum_variants(&constraints);
        assert_eq!(
            variants.len(),
            2,
            "'symbol' should be skipped, only String + Other"
        );
        assert_eq!(variants[0].name, "String");
        assert_eq!(variants[1].name, "Other");
    }

    // --- to_pascal_case tests ---

    #[test]
    fn test_pascal_case_snake_case_converted() {
        assert_eq!(to_pascal_case("foo_bar"), "FooBar");
    }

    #[test]
    fn test_pascal_case_first_char_capitalized() {
        // to_pascal_case capitalizes the first character and characters after delimiters.
        // It does NOT detect camelCase boundaries — "processData" becomes "ProcessData"
        // only because the first 'p' is capitalized.
        assert_eq!(to_pascal_case("processData"), "ProcessData");
    }

    #[test]
    fn test_pascal_case_kebab_case_converted() {
        assert_eq!(to_pascal_case("my-name"), "MyName");
    }

    #[test]
    fn test_pascal_case_empty_returns_empty() {
        assert_eq!(to_pascal_case(""), "");
    }

    #[test]
    fn test_pascal_case_single_word_capitalized() {
        assert_eq!(to_pascal_case("hello"), "Hello");
    }

    #[test]
    fn test_pascal_case_no_delimiters_preserves_body() {
        // Without delimiters, only the first character is capitalized.
        // Internal casing is preserved as-is.
        assert_eq!(to_pascal_case("FooBar"), "FooBar");
        assert_eq!(to_pascal_case("fooBar"), "FooBar");
    }

    // --- collect_any_constraints_from_expr tests ---

    #[test]
    fn test_constraints_from_expr_ternary_typeof_detected() {
        let expr = parse_expr(r#"typeof x === "string" ? x : 0"#);
        let result = collect_any_constraints_from_expr(&expr, &["x".to_string()]);
        let c = result.get("x").expect("should detect typeof in ternary");
        assert!(
            c.typeof_checks.contains(&"string".to_string()),
            "typeof in ternary condition should be detected"
        );
    }

    #[test]
    fn test_constraints_from_expr_parenthesized_typeof_detected() {
        let expr = parse_expr(r#"(typeof x === "number")"#);
        let result = collect_any_constraints_from_expr(&expr, &["x".to_string()]);
        let c = result.get("x").expect("should detect typeof in parens");
        assert!(
            c.typeof_checks.contains(&"number".to_string()),
            "typeof in parenthesized expression should be detected"
        );
    }

    // --- AnyTypeConstraints tests ---

    #[test]
    fn test_constraints_is_empty_when_no_checks() {
        let c = AnyTypeConstraints::default();
        assert!(c.is_empty(), "default constraints should be empty");
    }

    #[test]
    fn test_constraints_is_not_empty_with_typeof() {
        let c = AnyTypeConstraints {
            typeof_checks: vec!["string".to_string()],
            instanceof_checks: vec![],
        };
        assert!(!c.is_empty(), "constraints with typeof should not be empty");
    }

    #[test]
    fn test_constraints_is_not_empty_with_instanceof() {
        let c = AnyTypeConstraints {
            typeof_checks: vec![],
            instanceof_checks: vec!["Date".to_string()],
        };
        assert!(
            !c.is_empty(),
            "constraints with instanceof should not be empty"
        );
    }

    // --- collect_from_stmt: missing statement types (I-256) ---

    #[test]
    fn test_collect_constraints_in_switch_detected() {
        let body = parse_fn_body(
            r#"function f(x: any) {
                switch (typeof x) {
                    case "string": break;
                }
            }"#,
        );
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result.get("x").expect("should detect typeof in switch");
        assert!(
            c.typeof_checks.contains(&"string".to_string()),
            "typeof in switch discriminant should be detected"
        );
    }

    #[test]
    fn test_collect_constraints_in_switch_case_body_detected() {
        let body = parse_fn_body(
            r#"function f(x: any) {
                switch (true) {
                    case true:
                        if (typeof x === "number") { }
                        break;
                }
            }"#,
        );
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result
            .get("x")
            .expect("should detect typeof in switch case body");
        assert!(
            c.typeof_checks.contains(&"number".to_string()),
            "typeof in switch case body should be detected"
        );
    }

    #[test]
    fn test_collect_constraints_in_while_detected() {
        let body = parse_fn_body(
            r#"function f(x: any) {
                while (typeof x === "string") { break; }
            }"#,
        );
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result.get("x").expect("should detect typeof in while");
        assert!(
            c.typeof_checks.contains(&"string".to_string()),
            "typeof in while condition should be detected"
        );
    }

    #[test]
    fn test_collect_constraints_in_while_body_detected() {
        let body = parse_fn_body(
            r#"function f(x: any) {
                while (true) {
                    if (typeof x === "boolean") { }
                    break;
                }
            }"#,
        );
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result.get("x").expect("should detect typeof in while body");
        assert!(
            c.typeof_checks.contains(&"boolean".to_string()),
            "typeof in while body should be detected"
        );
    }

    #[test]
    fn test_collect_constraints_in_for_body_detected() {
        let body = parse_fn_body(
            r#"function f(x: any) {
                for (let i = 0; i < 1; i++) {
                    if (typeof x === "string") { }
                }
            }"#,
        );
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result.get("x").expect("should detect typeof in for body");
        assert!(
            c.typeof_checks.contains(&"string".to_string()),
            "typeof in for loop body should be detected"
        );
    }

    #[test]
    fn test_collect_constraints_in_for_of_body_detected() {
        let body = parse_fn_body(
            r#"function f(x: any) {
                for (const item of [1]) {
                    if (typeof x === "number") { }
                }
            }"#,
        );
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result
            .get("x")
            .expect("should detect typeof in for-of body");
        assert!(
            c.typeof_checks.contains(&"number".to_string()),
            "typeof in for-of body should be detected"
        );
    }

    #[test]
    fn test_collect_constraints_in_for_in_body_detected() {
        let body = parse_fn_body(
            r#"function f(x: any) {
                for (const key in {}) {
                    if (typeof x === "boolean") { }
                }
            }"#,
        );
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result
            .get("x")
            .expect("should detect typeof in for-in body");
        assert!(
            c.typeof_checks.contains(&"boolean".to_string()),
            "typeof in for-in body should be detected"
        );
    }

    #[test]
    fn test_collect_constraints_in_try_catch_detected() {
        let body = parse_fn_body(
            r#"function f(x: any) {
                try {
                    if (typeof x === "string") { }
                } catch (e) { }
            }"#,
        );
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result.get("x").expect("should detect typeof in try block");
        assert!(
            c.typeof_checks.contains(&"string".to_string()),
            "typeof in try block should be detected"
        );
    }

    #[test]
    fn test_collect_constraints_in_do_while_detected() {
        let body = parse_fn_body(
            r#"function f(x: any) {
                do {
                    if (typeof x === "number") { }
                } while (false);
            }"#,
        );
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result
            .get("x")
            .expect("should detect typeof in do-while body");
        assert!(
            c.typeof_checks.contains(&"number".to_string()),
            "typeof in do-while body should be detected"
        );
    }

    #[test]
    fn test_collect_constraints_in_labeled_detected() {
        let body = parse_fn_body(
            r#"function f(x: any) {
                label: if (typeof x === "string") { }
            }"#,
        );
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result
            .get("x")
            .expect("should detect typeof in labeled stmt");
        assert!(
            c.typeof_checks.contains(&"string".to_string()),
            "typeof in labeled statement should be detected"
        );
    }

    #[test]
    fn test_collect_constraints_in_throw_detected() {
        let body = parse_fn_body(
            r#"function f(x: any) {
                throw typeof x === "string" ? x : new Error();
            }"#,
        );
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result.get("x").expect("should detect typeof in throw expr");
        assert!(
            c.typeof_checks.contains(&"string".to_string()),
            "typeof in throw expression should be detected"
        );
    }

    #[test]
    fn test_collect_constraints_in_var_decl_detected() {
        let body = parse_fn_body(
            r#"function f(x: any) {
                const isStr = typeof x === "string";
            }"#,
        );
        let result = collect_any_constraints(&body, &["x".to_string()]);
        let c = result
            .get("x")
            .expect("should detect typeof in var decl init");
        assert!(
            c.typeof_checks.contains(&"string".to_string()),
            "typeof in variable declaration initializer should be detected"
        );
    }
}
