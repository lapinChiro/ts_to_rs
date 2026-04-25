use super::*;
use crate::parser::parse_typescript;
use crate::pipeline::synthetic_registry::SyntheticTypeRegistry;
use crate::transformer::statements::helpers::{
    extract_conditional_assignment, generate_falsy_condition, generate_truthiness_condition,
};
use swc_ecma_ast::ModuleItem;

fn empty_synth() -> SyntheticTypeRegistry {
    SyntheticTypeRegistry::new()
}

/// Parse a TS expression statement and return the SWC Expr.
fn parse_expr(source: &str) -> swc_ecma_ast::Expr {
    let module = parse_typescript(source).expect("parse failed");
    match &module.body[0] {
        ModuleItem::Stmt(swc_ecma_ast::Stmt::Expr(expr_stmt)) => *expr_stmt.expr.clone(),
        _ => panic!("expected expression statement"),
    }
}

// ─── extract_conditional_assignment ─────────────────────────────────

#[test]
fn test_extract_conditional_assignment_bare_assignment_returns_some() {
    let expr = parse_expr("x = expr;");
    let result = extract_conditional_assignment(&expr);
    let ca = result.expect("should return Some for bare assignment");
    assert_eq!(ca.var_name, "x");
    assert!(ca.outer_comparison.is_none());
    // rhs should be the identifier `expr`
    assert!(matches!(ca.rhs, swc_ecma_ast::Expr::Ident(id) if id.sym == "expr"));
}

#[test]
fn test_extract_conditional_assignment_comparison_with_left_assign_returns_outer() {
    // (x = expr) > 0
    let expr = parse_expr("(x = expr) > 0;");
    let result = extract_conditional_assignment(&expr);
    let ca = result.expect("should return Some for comparison with left assignment");
    assert_eq!(ca.var_name, "x");
    let outer = ca.outer_comparison.expect("should have outer comparison");
    assert_eq!(outer.op, swc_ecma_ast::BinaryOp::Gt);
    assert!(outer.assign_on_left);
    // other_operand should be the number 0
    assert!(matches!(
        outer.other_operand,
        swc_ecma_ast::Expr::Lit(swc_ecma_ast::Lit::Num(_))
    ));
}

#[test]
fn test_extract_conditional_assignment_comparison_with_right_assign_returns_outer() {
    // 0 < (x = expr)
    let expr = parse_expr("0 < (x = expr);");
    let result = extract_conditional_assignment(&expr);
    let ca = result.expect("should return Some for comparison with right assignment");
    assert_eq!(ca.var_name, "x");
    let outer = ca.outer_comparison.expect("should have outer comparison");
    assert_eq!(outer.op, swc_ecma_ast::BinaryOp::Lt);
    assert!(!outer.assign_on_left);
}

#[test]
fn test_extract_conditional_assignment_no_assignment_returns_none() {
    let expr = parse_expr("x > 0;");
    let result = extract_conditional_assignment(&expr);
    assert!(result.is_none(), "plain comparison should return None");
}

#[test]
fn test_extract_conditional_assignment_nested_parens_unwraps() {
    // (((x = expr))) — multiple layers of parens should be unwrapped
    let expr = parse_expr("(((x = expr)));");
    let result = extract_conditional_assignment(&expr);
    let ca = result.expect("should return Some after unwrapping nested parens");
    assert_eq!(ca.var_name, "x");
    assert!(ca.outer_comparison.is_none());
    assert!(matches!(ca.rhs, swc_ecma_ast::Expr::Ident(id) if id.sym == "expr"));
}

// ─── generate_truthiness_condition / generate_falsy_condition ────────

#[test]
fn test_generate_truthiness_condition_f64_generates_ne_zero_and_not_nan() {
    // I-144 T6-3 E10: F64 truthy must exclude both 0.0 AND NaN (JS parity).
    let synth = empty_synth();
    let result = generate_truthiness_condition("val", &RustType::F64, &synth);
    assert_eq!(
        result,
        Expr::BinaryOp {
            left: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("val".to_string())),
                op: BinOp::NotEq,
                right: Box::new(Expr::NumberLit(0.0)),
            }),
            op: BinOp::LogicalAnd,
            right: Box::new(Expr::UnaryOp {
                op: UnOp::Not,
                operand: Box::new(Expr::MethodCall {
                    object: Box::new(Expr::Ident("val".to_string())),
                    method: "is_nan".to_string(),
                    args: vec![],
                }),
            }),
        }
    );
}

#[test]
fn test_generate_truthiness_condition_string_generates_not_is_empty() {
    let synth = empty_synth();
    let result = generate_truthiness_condition("s", &RustType::String, &synth);
    assert_eq!(
        result,
        Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("s".to_string())),
                method: "is_empty".to_string(),
                args: vec![],
            }),
        }
    );
}

#[test]
fn test_generate_truthiness_condition_bool_generates_ident() {
    let synth = empty_synth();
    let result = generate_truthiness_condition("flag", &RustType::Bool, &synth);
    assert_eq!(result, Expr::Ident("flag".to_string()));
}

#[test]
fn test_generate_falsy_condition_is_inverse_of_truthiness() {
    // For F64 (I-144 T6-3 E10):
    //   truthy: `v != 0.0 && !v.is_nan()`
    //   falsy:  `v == 0.0 || v.is_nan()` (De Morgan)
    let synth = empty_synth();
    let truth_f64 = generate_truthiness_condition("v", &RustType::F64, &synth);
    let falsy_f64 = generate_falsy_condition("v", &RustType::F64, &synth);
    assert_eq!(
        truth_f64,
        Expr::BinaryOp {
            left: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("v".to_string())),
                op: BinOp::NotEq,
                right: Box::new(Expr::NumberLit(0.0)),
            }),
            op: BinOp::LogicalAnd,
            right: Box::new(Expr::UnaryOp {
                op: UnOp::Not,
                operand: Box::new(Expr::MethodCall {
                    object: Box::new(Expr::Ident("v".to_string())),
                    method: "is_nan".to_string(),
                    args: vec![],
                }),
            }),
        }
    );
    assert_eq!(
        falsy_f64,
        Expr::BinaryOp {
            left: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("v".to_string())),
                op: BinOp::Eq,
                right: Box::new(Expr::NumberLit(0.0)),
            }),
            op: BinOp::LogicalOr,
            right: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("v".to_string())),
                method: "is_nan".to_string(),
                args: vec![],
            }),
        }
    );

    // For String: truthiness is `!s.is_empty()`, falsy is `s.is_empty()`
    let truth_str = generate_truthiness_condition("s", &RustType::String, &synth);
    let falsy_str = generate_falsy_condition("s", &RustType::String, &synth);
    assert_eq!(
        truth_str,
        Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("s".to_string())),
                method: "is_empty".to_string(),
                args: vec![],
            }),
        }
    );
    assert_eq!(
        falsy_str,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("s".to_string())),
            method: "is_empty".to_string(),
            args: vec![],
        }
    );

    // For Bool: truthiness is `flag`, falsy is `!flag`
    let truth_bool = generate_truthiness_condition("flag", &RustType::Bool, &synth);
    let falsy_bool = generate_falsy_condition("flag", &RustType::Bool, &synth);
    assert_eq!(truth_bool, Expr::Ident("flag".to_string()));
    assert_eq!(
        falsy_bool,
        Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::Ident("flag".to_string())),
        }
    );
}

// ─── P4: non-primitive type coverage via expr-level dispatch ───────────────
//
// Pre-T6 the helpers fell back to `Expr::Ident(x)` / `!Expr::Ident(x)` for
// every non-primitive type, which was a Rust type error at the use site.
// The fence was acceptable while only `if (x = expr)` / `while (x = expr)`
// flowed primitives, but blocked Vec / Named / Option<T> for future
// extensions. T6 P1 routes through `truthy_predicate_for_expr` so all
// supported types now emit valid predicates.

#[test]
fn test_generate_truthiness_condition_option_f64_uses_is_some_and() {
    let synth = empty_synth();
    let result =
        generate_truthiness_condition("x", &RustType::Option(Box::new(RustType::F64)), &synth);
    let Expr::MethodCall {
        ref method,
        ref object,
        ..
    } = result
    else {
        panic!("expected MethodCall, got {result:?}");
    };
    assert_eq!(method, "is_some_and");
    assert!(matches!(
        object.as_ref(),
        Expr::Ident(name) if name == "x"
    ));
}

#[test]
fn test_generate_truthiness_condition_vec_is_always_true() {
    let synth = empty_synth();
    let result =
        generate_truthiness_condition("v", &RustType::Vec(Box::new(RustType::F64)), &synth);
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn test_generate_truthiness_condition_named_struct_is_always_true() {
    let synth = empty_synth();
    let result = generate_truthiness_condition(
        "p",
        &RustType::Named {
            name: "Point".into(),
            type_args: vec![],
        },
        &synth,
    );
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn test_generate_falsy_condition_option_named_uses_is_none() {
    let synth = empty_synth();
    let result = generate_falsy_condition(
        "opt",
        &RustType::Option(Box::new(RustType::Named {
            name: "Foo".into(),
            type_args: vec![],
        })),
        &synth,
    );
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("opt".to_string())),
            method: "is_none".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_generate_falsy_condition_vec_is_always_false() {
    let synth = empty_synth();
    let result = generate_falsy_condition("v", &RustType::Vec(Box::new(RustType::F64)), &synth);
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn test_generate_truthiness_condition_any_falls_back_to_ident() {
    // Any / TypeVar / Unit / Never / Result / QSelf intentionally hit the
    // compile-time fence: bare `Expr::Ident` produces a Rust type error
    // for non-Bool types, surfacing the gap via rustc rather than emitting
    // a silent semantic-changing guess (Tier 1).
    let synth = empty_synth();
    let result = generate_truthiness_condition("x", &RustType::Any, &synth);
    assert_eq!(result, Expr::Ident("x".to_string()));
}

#[test]
fn test_generate_falsy_condition_typevar_falls_back_to_negated_ident() {
    let synth = empty_synth();
    let result = generate_falsy_condition(
        "x",
        &RustType::TypeVar {
            name: "T".to_string(),
        },
        &synth,
    );
    assert_eq!(
        result,
        Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::Ident("x".to_string())),
        }
    );
}
