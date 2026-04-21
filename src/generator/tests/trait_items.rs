//! Trait-related rendering: `Item::Trait` (signature, supertraits),
//! `Item::Impl` with `for_trait: Some(_)` (trait impl), `async fn` in
//! trait method signature, and I-218 `type_params` / trait `type_args` on
//! trait impls + supertrait `type_args`.

use super::*;
use crate::ir::{Expr, Item, Method, Param, RustType, Stmt, TraitRef, TypeParam, Visibility};

#[test]
fn test_generate_trait() {
    let item = Item::Trait {
        vis: Visibility::Public,
        name: "AnimalTrait".to_string(),
        type_params: vec![],
        methods: vec![Method {
            vis: Visibility::Private,
            name: "speak".to_string(),
            is_async: false,
            has_self: true,
            has_mut_self: false,
            params: vec![],
            return_type: Some(RustType::String),
            body: None,
        }],
        supertraits: vec![],
        associated_types: vec![],
    };
    let expected = "\
pub trait AnimalTrait {
    fn speak(&self) -> String;
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_trait_with_supertraits_outputs_bounds() {
    let item = Item::Trait {
        vis: Visibility::Public,
        name: "Dog".to_string(),
        type_params: vec![],
        supertraits: vec![
            TraitRef {
                name: "Animal".to_string(),
                type_args: vec![],
            },
            TraitRef {
                name: "Debug".to_string(),
                type_args: vec![],
            },
        ],
        methods: vec![Method {
            vis: Visibility::Private,
            name: "bark".to_string(),
            is_async: false,
            has_self: true,
            has_mut_self: false,
            params: vec![],
            return_type: None,
            body: None,
        }],
        associated_types: vec![],
    };
    let expected = "\
pub trait Dog: Animal + Debug {
    fn bark(&self);
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_async_trait_method_sig() {
    let item = Item::Trait {
        vis: Visibility::Public,
        name: "Handler".to_string(),
        type_params: vec![],
        supertraits: vec![],
        methods: vec![Method {
            vis: Visibility::Private,
            name: "handle".to_string(),
            is_async: true,
            has_self: true,
            has_mut_self: false,
            params: vec![Param {
                name: "req".to_string(),
                ty: Some(RustType::String),
            }],
            return_type: Some(RustType::String),
            body: None,
        }],
        associated_types: vec![],
    };
    let output = generate(&[item]);
    assert!(
        output.contains("async fn handle"),
        "trait method should have async keyword: {output}"
    );
}

#[test]
fn test_generate_impl_for_trait() {
    let item = Item::Impl {
        struct_name: "Dog".to_string(),
        type_params: vec![],
        for_trait: Some(TraitRef {
            name: "AnimalTrait".to_string(),
            type_args: vec![],
        }),
        consts: vec![],
        methods: vec![Method {
            vis: Visibility::Private,
            name: "speak".to_string(),
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
impl AnimalTrait for Dog {
    fn speak(&self) -> String {
        self.name
    }
}";
    assert_eq!(generate(&[item]), expected);
}

// --- I-218: Impl::type_params / TraitRef::type_args on trait impls ---

#[test]
fn test_generate_impl_for_trait_with_type_params() {
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: None,
            default: None,
        }],
        for_trait: Some(TraitRef {
            name: "Display".to_string(),
            type_args: vec![],
        }),
        consts: vec![],
        methods: vec![],
    };
    let output = generate_item(&item);
    assert_eq!(output, "impl<T> Display for Foo<T> {\n}");
}

#[test]
fn test_generate_impl_for_trait_with_trait_type_args() {
    let item = Item::Impl {
        struct_name: "FooData".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: None,
            default: None,
        }],
        for_trait: Some(TraitRef {
            name: "Container".to_string(),
            type_args: vec![RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            }],
        }),
        consts: vec![],
        methods: vec![],
    };
    let output = generate_item(&item);
    assert_eq!(output, "impl<T> Container<T> for FooData<T> {\n}");
}

#[test]
fn test_generate_impl_for_trait_with_concrete_type_args() {
    let item = Item::Impl {
        struct_name: "Child".to_string(),
        type_params: vec![],
        for_trait: Some(TraitRef {
            name: "ParentTrait".to_string(),
            type_args: vec![RustType::String],
        }),
        consts: vec![],
        methods: vec![],
    };
    let output = generate_item(&item);
    assert_eq!(output, "impl ParentTrait<String> for Child {\n}");
}

#[test]
fn test_generate_trait_with_supertrait_type_args() {
    let item = Item::Trait {
        vis: Visibility::Public,
        name: "Foo".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: None,
            default: None,
        }],
        supertraits: vec![TraitRef {
            name: "Bar".to_string(),
            type_args: vec![RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            }],
        }],
        methods: vec![],
        associated_types: vec![],
    };
    let output = generate_item(&item);
    assert!(
        output.starts_with("pub trait Foo<T>: Bar<T>"),
        "expected 'pub trait Foo<T>: Bar<T>', got: {output}"
    );
}
