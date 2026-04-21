//! Control-flow `Expr` variants: `Vec` (vec! literal), `If`
//! (expression form), `Match` (match with enum variant bindings),
//! `Block` (block expression with trailing tail), and the
//! `RuntimeTypeof` helper-call shape.

use super::*;
use crate::ir::{BinOp, Expr, Stmt};

// --- Vec literal ---

#[test]
fn test_generate_expr_vec_numbers() {
    let expr = Expr::Vec {
        elements: vec![
            Expr::NumberLit(1.0),
            Expr::NumberLit(2.0),
            Expr::NumberLit(3.0),
        ],
    };
    assert_eq!(generate_expr(&expr), "vec![1.0, 2.0, 3.0]");
}

#[test]
fn test_generate_expr_vec_empty() {
    let expr = Expr::Vec { elements: vec![] };
    assert_eq!(generate_expr(&expr), "vec![]");
}

#[test]
fn test_generate_expr_vec_single() {
    let expr = Expr::Vec {
        elements: vec![Expr::StringLit("hello".to_string())],
    };
    assert_eq!(generate_expr(&expr), "vec![\"hello\"]");
}

#[test]
fn test_generate_expr_vec_nested() {
    let expr = Expr::Vec {
        elements: vec![
            Expr::Vec {
                elements: vec![Expr::NumberLit(1.0)],
            },
            Expr::Vec {
                elements: vec![Expr::NumberLit(2.0)],
            },
        ],
    };
    assert_eq!(generate_expr(&expr), "vec![vec![1.0], vec![2.0]]");
}

// --- If expression ---

#[test]
fn test_generate_expr_if_basic() {
    let expr = Expr::If {
        condition: Box::new(Expr::Ident("flag".to_string())),
        then_expr: Box::new(Expr::Ident("x".to_string())),
        else_expr: Box::new(Expr::Ident("y".to_string())),
    };
    assert_eq!(generate_expr(&expr), "if flag { x } else { y }");
}

#[test]
fn test_generate_expr_if_with_literals() {
    let expr = Expr::If {
        condition: Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Ident("a".to_string())),
            op: BinOp::Gt,
            right: Box::new(Expr::NumberLit(0.0)),
        }),
        then_expr: Box::new(Expr::NumberLit(1.0)),
        else_expr: Box::new(Expr::NumberLit(2.0)),
    };
    assert_eq!(generate_expr(&expr), "if a > 0.0 { 1.0 } else { 2.0 }");
}

#[test]
fn test_generate_expr_if_nested() {
    let expr = Expr::If {
        condition: Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Ident("x".to_string())),
            op: BinOp::Gt,
            right: Box::new(Expr::NumberLit(0.0)),
        }),
        then_expr: Box::new(Expr::StringLit("positive".to_string())),
        else_expr: Box::new(Expr::If {
            condition: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Lt,
                right: Box::new(Expr::NumberLit(0.0)),
            }),
            then_expr: Box::new(Expr::StringLit("negative".to_string())),
            else_expr: Box::new(Expr::StringLit("zero".to_string())),
        }),
    };
    assert_eq!(
        generate_expr(&expr),
        "if x > 0.0 { \"positive\" } else { if x < 0.0 { \"negative\" } else { \"zero\" } }"
    );
}

// --- Block / Match / RuntimeTypeof ---

#[test]
fn test_generate_expr_block_renders_block_expression() {
    let expr = Expr::Block(vec![
        Stmt::Let {
            mutable: true,
            name: "_v".to_string(),
            ty: None,
            init: Some(Expr::Vec {
                elements: vec![Expr::NumberLit(1.0)],
            }),
        },
        Stmt::TailExpr(Expr::Ident("_v".to_string())),
    ]);
    let expected = "{\n    let mut _v = vec![1.0];\n    _v\n}";
    assert_eq!(generate_expr(&expr), expected);
}

#[test]
fn test_generate_expr_match_with_enum_variant_bindings() {
    use crate::ir::MatchArm;
    let expr = Expr::Match {
        expr: Box::new(Expr::Ref(Box::new(Expr::Ident("s".to_string())))),
        arms: vec![
            MatchArm {
                patterns: vec![crate::ir::Pattern::Struct {
                    ctor: crate::ir::PatternCtor::UserEnumVariant {
                        enum_ty: crate::ir::UserTypeRef::new("Shape"),
                        variant: "Circle".to_string(),
                    },
                    fields: vec![("radius".to_string(), crate::ir::Pattern::binding("radius"))],
                    rest: true,
                }],
                guard: None,
                body: vec![Stmt::TailExpr(Expr::MethodCall {
                    object: Box::new(Expr::Ident("radius".to_string())),
                    method: "clone".to_string(),
                    args: vec![],
                })],
            },
            MatchArm {
                patterns: vec![crate::ir::Pattern::Wildcard],
                guard: None,
                body: vec![Stmt::TailExpr(Expr::MacroCall {
                    name: "panic".to_string(),
                    args: vec![Expr::StringLit("unexpected variant".to_string())],
                    use_debug: vec![false],
                })],
            },
        ],
    };
    let expected = "match &s {\n    Shape::Circle { radius, .. } => {\n        radius.clone()\n    }\n    _ => {\n        panic!(\"unexpected variant\")\n    }\n}";
    assert_eq!(generate_expr(&expr), expected);
}

#[test]
fn test_generate_expr_runtime_typeof_produces_helper_call() {
    let expr = Expr::RuntimeTypeof {
        operand: Box::new(Expr::Ident("x".to_string())),
    };
    assert_eq!(generate_expr(&expr), "js_typeof(&x)");
}
