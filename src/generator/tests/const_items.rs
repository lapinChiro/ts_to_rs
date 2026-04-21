//! `Item::Const` rendering: primitive values (f64, String) and unit-struct
//! initializer (`const x: FooImpl = FooImpl;`).

use super::*;
use crate::ir::{Expr, Item, RustType, Visibility};

#[test]
fn test_generate_const_private_number() {
    let item = Item::Const {
        vis: Visibility::Private,
        name: "MY_VAL".to_string(),
        ty: RustType::F64,
        value: Expr::NumberLit(42.0),
    };
    assert_eq!(generate(&[item]), "const MY_VAL: f64 = 42.0;");
}

#[test]
fn test_generate_const_public_string() {
    let item = Item::Const {
        vis: Visibility::Public,
        name: "GREETING".to_string(),
        ty: RustType::String,
        value: Expr::StringLit("hello".to_string()),
    };
    assert_eq!(generate(&[item]), "pub const GREETING: String = \"hello\";");
}

#[test]
fn test_generate_const_unit_struct_init() {
    let item = Item::Const {
        vis: Visibility::Private,
        name: "getCookie".to_string(),
        ty: RustType::Named {
            name: "GetCookieImpl".to_string(),
            type_args: vec![],
        },
        value: Expr::StructInit {
            name: "GetCookieImpl".to_string(),
            fields: vec![],
            base: None,
        },
    };
    assert_eq!(
        generate(&[item]),
        "const getCookie: GetCookieImpl = GetCookieImpl;"
    );
}
