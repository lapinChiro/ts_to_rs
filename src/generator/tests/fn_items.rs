//! `Item::Fn` rendering: signature shape (return / params / type params),
//! reserved-word name escaping via `r#`, and `#[attr]` emission.

use super::*;
use crate::ir::{BinOp, Expr, Item, Param, RustType, Stmt, TypeParam, Visibility};

#[test]
fn test_generate_fn_simple_return() {
    let item = Item::Fn {
        vis: Visibility::Public,
        attributes: vec![],
        is_async: false,
        name: "add".to_string(),
        type_params: vec![],
        params: vec![
            Param {
                name: "a".to_string(),
                ty: Some(RustType::F64),
            },
            Param {
                name: "b".to_string(),
                ty: Some(RustType::F64),
            },
        ],
        return_type: Some(RustType::F64),
        body: vec![Stmt::TailExpr(Expr::BinaryOp {
            left: Box::new(Expr::Ident("a".to_string())),
            op: BinOp::Add,
            right: Box::new(Expr::Ident("b".to_string())),
        })],
    };
    let expected = "\
pub fn add(a: f64, b: f64) -> f64 {
    a + b
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_fn_no_return_type() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: false,
        name: "greet".to_string(),
        type_params: vec![],
        params: vec![Param {
            name: "name".to_string(),
            ty: Some(RustType::String),
        }],
        return_type: None,
        body: vec![Stmt::Expr(Expr::Ident("println!".to_string()))],
    };
    let expected = "\
fn greet(name: String) {
    println!;
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_fn_no_params() {
    let item = Item::Fn {
        vis: Visibility::Public,
        attributes: vec![],
        is_async: false,
        name: "get_value".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: Some(RustType::F64),
        body: vec![Stmt::TailExpr(Expr::NumberLit(42.0))],
    };
    let expected = "\
pub fn get_value() -> f64 {
    42.0
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_fn_with_type_params() {
    let item = Item::Fn {
        vis: Visibility::Public,
        attributes: vec![],
        is_async: false,
        name: "identity".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: None,
            default: None,
        }],
        params: vec![Param {
            name: "x".to_string(),
            ty: Some(RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            }),
        }],
        return_type: Some(RustType::Named {
            name: "T".to_string(),
            type_args: vec![],
        }),
        body: vec![Stmt::TailExpr(Expr::Ident("x".to_string()))],
    };
    let expected = "\
pub fn identity<T>(x: T) -> T {
    x
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_escape_ident_fn_name_reserved_word_adds_r_hash() {
    let item = Item::Fn {
        vis: Visibility::Public,
        attributes: vec![],
        is_async: false,
        name: "match".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![],
    };
    let output = generate(&[item]);
    assert!(
        output.contains("fn r#match()"),
        "expected r#match in: {output}"
    );
}

#[test]
fn test_generate_fn_with_attributes_outputs_attr_lines() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec!["tokio::main".to_string()],
        is_async: true,
        name: "main".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![],
    };
    let expected = "\
#[tokio::main]
async fn main() {
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_fn_without_attributes_no_attr_lines() {
    let item = Item::Fn {
        vis: Visibility::Private,
        attributes: vec![],
        is_async: true,
        name: "not_main".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![],
    };
    let output = generate(&[item]);
    assert!(
        !output.contains("#["),
        "expected no attributes in: {output}"
    );
}
