//! Shared AST pattern detection utilities for narrowing.
//!
//! Pure functions that extract narrowing-related patterns from SWC AST nodes.
//! Used by the type-resolver's narrowing module (type pre-computation) and
//! by the narrowing analyzer (reset classification).

use swc_ecma_ast as ast;

/// Extracts the typeof operand and comparison string from a binary expression.
///
/// Handles both orderings:
/// - `typeof x === "string"` → `Some((&x_expr, "string"))`
/// - `"string" === typeof x` → `Some((&x_expr, "string"))`
///
/// Returns `None` if the expression is not a typeof comparison.
pub(crate) fn extract_typeof_and_string(bin: &ast::BinExpr) -> Option<(&ast::Expr, String)> {
    // Only comparison operators can form typeof checks
    if !matches!(
        bin.op,
        ast::BinaryOp::EqEq | ast::BinaryOp::EqEqEq | ast::BinaryOp::NotEq | ast::BinaryOp::NotEqEq
    ) {
        return None;
    }

    // Left is typeof, right is string literal
    if let ast::Expr::Unary(unary) = bin.left.as_ref() {
        if unary.op == ast::UnaryOp::TypeOf {
            if let ast::Expr::Lit(ast::Lit::Str(s)) = bin.right.as_ref() {
                return Some((&unary.arg, s.value.to_string_lossy().into_owned()));
            }
        }
    }
    // Right is typeof, left is string literal
    if let ast::Expr::Unary(unary) = bin.right.as_ref() {
        if unary.op == ast::UnaryOp::TypeOf {
            if let ast::Expr::Lit(ast::Lit::Str(s)) = bin.left.as_ref() {
                return Some((&unary.arg, s.value.to_string_lossy().into_owned()));
            }
        }
    }
    None
}

/// Returns true if the expression is a `null` literal or the `undefined`
/// identifier, **peeling through** `Paren` and TS wrapper expressions.
///
/// Legal JS / TS often wraps the null-check RHS: `x === (null)`,
/// `x === null as any`, `x === null satisfies unknown`, etc. This helper
/// unwraps every such wrapper so the semantic check (is the RHS null or
/// undefined?) is independent of syntactic sugar.
pub(crate) fn is_null_or_undefined(expr: &ast::Expr) -> bool {
    match peel_wrappers(expr) {
        ast::Expr::Lit(ast::Lit::Null(..)) => true,
        ast::Expr::Ident(ident) => ident.sym.as_ref() == "undefined",
        _ => false,
    }
}

/// Returns true if the expression is the `undefined` identifier (strict),
/// **peeling through** `Paren` and TS wrapper expressions.
///
/// Distinct from [`is_null_or_undefined`] — used by null-check classifiers
/// that must differentiate `x === null` from `x === undefined`.
pub(crate) fn is_undefined_ident(expr: &ast::Expr) -> bool {
    matches!(peel_wrappers(expr), ast::Expr::Ident(ident) if ident.sym.as_ref() == "undefined")
}

/// Peels through `Paren` and TS type-system wrappers that are transparent
/// to value semantics (`as`, `<T>x`, `x!`, `x as const`, `x satisfies T`,
/// `x<T>`), returning the innermost value-bearing expression.
///
/// Shared by [`is_null_or_undefined`] and [`is_undefined_ident`] so
/// null / undefined detection is uniformly wrapper-aware.
fn peel_wrappers(expr: &ast::Expr) -> &ast::Expr {
    let mut current = expr;
    loop {
        current = match current {
            ast::Expr::Paren(p) => &p.expr,
            ast::Expr::TsAs(a) => &a.expr,
            ast::Expr::TsTypeAssertion(a) => &a.expr,
            ast::Expr::TsNonNull(a) => &a.expr,
            ast::Expr::TsConstAssertion(a) => &a.expr,
            ast::Expr::TsSatisfies(a) => &a.expr,
            ast::Expr::TsInstantiation(a) => &a.expr,
            _ => return current,
        };
    }
}

/// Returns `true` iff `stmt` is guaranteed to exit the enclosing
/// block-level scope (via `return` / `throw` / `break` / `continue`) on
/// every control-flow path.
///
/// Used by classifiers that must stop scanning after an always-exiting
/// statement so that unreachable code is not misclassified as a reset or
/// narrow-affecting mutation.
///
/// - A block always exits iff its last statement always exits.
/// - An `if` always exits iff **both** branches always exit.
/// - Other compound statements (loops, switch, try) may or may not exit
///   depending on their bodies, which is beyond this conservative predicate.
pub(crate) fn stmt_always_exits(stmt: &ast::Stmt) -> bool {
    match stmt {
        ast::Stmt::Return(_) | ast::Stmt::Throw(_) => true,
        ast::Stmt::Break(_) | ast::Stmt::Continue(_) => true,
        ast::Stmt::Block(block) => block.stmts.last().is_some_and(stmt_always_exits),
        ast::Stmt::If(if_stmt) => {
            let cons_exits = stmt_always_exits(&if_stmt.cons);
            let alt_exits = if_stmt.alt.as_ref().is_some_and(|s| stmt_always_exits(s));
            cons_exits && alt_exits
        }
        _ => false,
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

    fn as_bin_expr(expr: &ast::Expr) -> &ast::BinExpr {
        match expr {
            ast::Expr::Bin(bin) => bin,
            _ => panic!("expected binary expression"),
        }
    }

    // === extract_typeof_and_string ===

    #[test]
    fn test_extract_typeof_and_string_normal_order() {
        let expr = parse_expr(r#"typeof x === "string""#);
        let bin = as_bin_expr(&expr);
        let (operand, type_str) = extract_typeof_and_string(bin).expect("should extract");
        assert!(matches!(operand, ast::Expr::Ident(ident) if ident.sym.as_ref() == "x"));
        assert_eq!(type_str, "string");
    }

    #[test]
    fn test_extract_typeof_and_string_reversed_order() {
        let expr = parse_expr(r#""number" === typeof y"#);
        let bin = as_bin_expr(&expr);
        let (operand, type_str) = extract_typeof_and_string(bin).expect("should extract");
        assert!(matches!(operand, ast::Expr::Ident(ident) if ident.sym.as_ref() == "y"));
        assert_eq!(type_str, "number");
    }

    #[test]
    fn test_extract_typeof_and_string_non_typeof_unary_returns_none() {
        // !x === "string" — not a typeof expression
        let expr = parse_expr(r#"!x === "string""#);
        let bin = as_bin_expr(&expr);
        assert!(extract_typeof_and_string(bin).is_none());
    }

    #[test]
    fn test_extract_typeof_and_string_typeof_with_number_rhs_returns_none() {
        let expr = parse_expr("typeof x === 42");
        let bin = as_bin_expr(&expr);
        assert!(extract_typeof_and_string(bin).is_none());
    }

    #[test]
    fn test_extract_typeof_and_string_no_typeof_returns_none() {
        let expr = parse_expr("x === y");
        let bin = as_bin_expr(&expr);
        assert!(extract_typeof_and_string(bin).is_none());
    }

    #[test]
    fn test_extract_typeof_and_string_addition_operator_returns_none() {
        // typeof x + "string" is addition, not comparison — must return None
        let expr = parse_expr(r#"typeof x + "string""#);
        let bin = as_bin_expr(&expr);
        assert!(
            extract_typeof_and_string(bin).is_none(),
            "addition with typeof should not be detected as typeof comparison"
        );
    }

    #[test]
    fn test_extract_typeof_and_string_neq_operator_returns_some() {
        // typeof x !== "string" should still be detected
        let expr = parse_expr(r#"typeof x !== "string""#);
        let bin = as_bin_expr(&expr);
        assert!(extract_typeof_and_string(bin).is_some());
    }

    // === is_null_or_undefined ===

    #[test]
    fn test_is_null_or_undefined_null_literal_returns_true() {
        let expr = parse_expr("null");
        assert!(is_null_or_undefined(&expr));
    }

    #[test]
    fn test_is_null_or_undefined_undefined_ident_returns_true() {
        let expr = parse_expr("undefined");
        assert!(is_null_or_undefined(&expr));
    }

    #[test]
    fn test_is_null_or_undefined_other_ident_returns_false() {
        let expr = parse_expr("x");
        assert!(!is_null_or_undefined(&expr));
    }

    #[test]
    fn test_is_null_or_undefined_number_literal_returns_false() {
        let expr = parse_expr("42");
        assert!(!is_null_or_undefined(&expr));
    }

    #[test]
    fn test_is_null_or_undefined_paren_wrapped_null_returns_true() {
        let expr = parse_expr("(null)");
        assert!(is_null_or_undefined(&expr));
    }

    #[test]
    fn test_is_null_or_undefined_paren_wrapped_undefined_returns_true() {
        let expr = parse_expr("(undefined)");
        assert!(is_null_or_undefined(&expr));
    }

    #[test]
    fn test_is_null_or_undefined_ts_as_null_returns_true() {
        let expr = parse_expr("null as any");
        assert!(is_null_or_undefined(&expr));
    }

    #[test]
    fn test_is_null_or_undefined_ts_non_null_returns_true() {
        // `null!` is structurally unusual but legal — peel it too.
        let expr = parse_expr("null!");
        assert!(is_null_or_undefined(&expr));
    }

    #[test]
    fn test_is_null_or_undefined_nested_wrappers_returns_true() {
        let expr = parse_expr("(null as any)");
        assert!(is_null_or_undefined(&expr));
    }

    #[test]
    fn test_is_null_or_undefined_ts_as_non_null_ident_returns_false() {
        // `x as any` is NOT null or undefined regardless of wrappers.
        let expr = parse_expr("x as any");
        assert!(!is_null_or_undefined(&expr));
    }

    #[test]
    fn test_is_null_or_undefined_ts_type_assertion_null_returns_true() {
        // Angle-bracket TS type assertion: `<any>null`.
        let expr = parse_expr("<any>null");
        assert!(is_null_or_undefined(&expr));
    }

    #[test]
    fn test_is_null_or_undefined_ts_const_assertion_returns_true() {
        // `null as const` peels through TsConstAssertion.
        let expr = parse_expr("null as const");
        assert!(is_null_or_undefined(&expr));
    }

    #[test]
    fn test_is_null_or_undefined_ts_satisfies_returns_true() {
        // `null satisfies unknown` peels through TsSatisfies.
        let expr = parse_expr("null satisfies unknown");
        assert!(is_null_or_undefined(&expr));
    }

    #[test]
    fn test_is_null_or_undefined_double_paren_returns_true() {
        // Multi-level paren is peeled iteratively.
        let expr = parse_expr("(((null)))");
        assert!(is_null_or_undefined(&expr));
    }

    // === is_undefined_ident ===

    #[test]
    fn test_is_undefined_ident_bare_returns_true() {
        let expr = parse_expr("undefined");
        assert!(is_undefined_ident(&expr));
    }

    #[test]
    fn test_is_undefined_ident_null_returns_false() {
        let expr = parse_expr("null");
        assert!(!is_undefined_ident(&expr));
    }

    #[test]
    fn test_is_undefined_ident_paren_wrapped_returns_true() {
        let expr = parse_expr("(undefined)");
        assert!(is_undefined_ident(&expr));
    }

    #[test]
    fn test_is_undefined_ident_ts_as_undefined_returns_true() {
        let expr = parse_expr("undefined as any");
        assert!(is_undefined_ident(&expr));
    }

    #[test]
    fn test_is_undefined_ident_paren_null_returns_false() {
        // `(null)` is null, not undefined.
        let expr = parse_expr("(null)");
        assert!(!is_undefined_ident(&expr));
    }

    // === stmt_always_exits ===

    /// Parses a function body and returns its first statement.
    fn parse_first_stmt(fn_body: &str) -> Stmt {
        let source = format!("function f() {{ {fn_body} }}");
        let module = parse_typescript(&source).expect("parse failed");
        match &module.body[0] {
            ModuleItem::Stmt(Stmt::Decl(swc_ecma_ast::Decl::Fn(fn_decl))) => {
                fn_decl.function.body.as_ref().unwrap().stmts[0].clone()
            }
            _ => panic!("expected function declaration"),
        }
    }

    #[test]
    fn test_stmt_always_exits_return_true() {
        let stmt = parse_first_stmt("return 1;");
        assert!(stmt_always_exits(&stmt));
    }

    #[test]
    fn test_stmt_always_exits_throw_true() {
        let stmt = parse_first_stmt("throw new Error();");
        assert!(stmt_always_exits(&stmt));
    }

    #[test]
    fn test_stmt_always_exits_break_true() {
        let stmt = parse_first_stmt("while (true) { break; }");
        if let Stmt::While(w) = &stmt {
            if let Some(block) = w.body.as_block() {
                assert!(
                    stmt_always_exits(&block.stmts[0]),
                    "break should always exit"
                );
            }
        }
    }

    #[test]
    fn test_stmt_always_exits_continue_true() {
        let stmt = parse_first_stmt("while (true) { continue; }");
        if let Stmt::While(w) = &stmt {
            if let Some(block) = w.body.as_block() {
                assert!(
                    stmt_always_exits(&block.stmts[0]),
                    "continue should always exit"
                );
            }
        }
    }

    #[test]
    fn test_stmt_always_exits_empty_block_false() {
        let stmt = parse_first_stmt("{}");
        assert!(!stmt_always_exits(&stmt));
    }

    #[test]
    fn test_stmt_always_exits_block_ending_with_return_true() {
        let stmt = parse_first_stmt("{ const x = 1; return x; }");
        assert!(stmt_always_exits(&stmt));
    }

    #[test]
    fn test_stmt_always_exits_block_ending_with_expr_false() {
        let stmt = parse_first_stmt("{ const x = 1; x; }");
        assert!(!stmt_always_exits(&stmt));
    }

    #[test]
    fn test_stmt_always_exits_if_both_branches_exit_true() {
        let stmt = parse_first_stmt("if (true) { return 1; } else { return 2; }");
        assert!(stmt_always_exits(&stmt));
    }

    #[test]
    fn test_stmt_always_exits_if_only_then_exits_false() {
        let stmt = parse_first_stmt("if (true) { return 1; } else { console.log(1); }");
        assert!(!stmt_always_exits(&stmt));
    }

    #[test]
    fn test_stmt_always_exits_if_no_else_false() {
        let stmt = parse_first_stmt("if (true) { return 1; }");
        assert!(!stmt_always_exits(&stmt));
    }

    #[test]
    fn test_stmt_always_exits_expr_stmt_false() {
        let stmt = parse_first_stmt("console.log(1);");
        assert!(!stmt_always_exits(&stmt));
    }
}
