//! Inherent `Item::Impl` rendering (no `for_trait`): constructor (`new`),
//! `&self` method, async method, and I-218 `type_params` / `constraint`
//! (generic impl).
//!
//! Trait impls (`Item::Impl` with `for_trait: Some(_)`) live in
//! [`super::trait_items`] since they primarily exercise trait-related
//! rendering.

use super::*;
use crate::ir::{Expr, Item, Method, Param, RustType, Stmt, TypeParam, Visibility};

#[test]
fn test_generate_impl_new() {
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![],
        for_trait: None,
        consts: vec![],
        methods: vec![Method {
            vis: Visibility::Public,
            name: "new".to_string(),
            is_async: false,
            has_self: false,
            has_mut_self: false,
            params: vec![Param {
                name: "x".to_string(),
                ty: Some(RustType::F64),
            }],
            return_type: Some(RustType::Named {
                name: "Self".to_string(),
                type_args: vec![],
            }),
            body: Some(vec![Stmt::TailExpr(Expr::Ident("Self { x }".to_string()))]),
        }],
    };
    let expected = "\
impl Foo {
    pub fn new(x: f64) -> Self {
        Self { x }
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_impl_self_method() {
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![],
        for_trait: None,
        consts: vec![],
        methods: vec![Method {
            vis: Visibility::Public,
            name: "get_name".to_string(),
            is_async: false,
            has_self: true,
            has_mut_self: false,
            params: vec![],
            return_type: Some(RustType::String),
            body: Some(vec![Stmt::TailExpr(Expr::FieldAccess {
                object: Box::new(Expr::Ident("self".to_string())),
                field: "name".to_string(),
            })]),
        }],
    };
    let expected = "\
impl Foo {
    pub fn get_name(&self) -> String {
        self.name
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_async_impl_method() {
    let item = Item::Impl {
        struct_name: "MyHandler".to_string(),
        type_params: vec![],
        for_trait: None,
        consts: vec![],
        methods: vec![Method {
            vis: Visibility::Public,
            name: "process".to_string(),
            is_async: true,
            has_self: true,
            has_mut_self: false,
            params: vec![],
            return_type: Some(RustType::String),
            body: Some(vec![Stmt::TailExpr(Expr::StringLit("done".to_string()))]),
        }],
    };
    let output = generate(&[item]);
    assert!(
        output.contains("pub async fn process"),
        "impl method should have async keyword: {output}"
    );
}

// --- I-218: Item::Impl type_params (inherent impl) ---

#[test]
fn test_generate_impl_with_type_params() {
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: None,
            default: None,
        }],
        for_trait: None,
        consts: vec![],
        methods: vec![],
    };
    let output = generate_item(&item);
    assert_eq!(output, "impl<T> Foo<T> {\n}");
}

#[test]
fn test_generate_impl_with_constraint() {
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: Some(RustType::Named {
                name: "Clone".to_string(),
                type_args: vec![],
            }),
            default: None,
        }],
        for_trait: None,
        consts: vec![],
        methods: vec![],
    };
    let output = generate_item(&item);
    assert_eq!(output, "impl<T: Clone> Foo<T> {\n}");
}
