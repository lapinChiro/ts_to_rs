//! `Item::Struct` rendering: public/private + type params, unit-struct
//! marker (`is_unit_struct`) vs empty non-unit struct, and reserved-word
//! field escaping via `r#`.

use super::*;
use crate::ir::{Item, RustType, StructField, TypeParam, Visibility};

#[test]
fn test_generate_struct_public() {
    let item = Item::Struct {
        vis: Visibility::Public,
        name: "Foo".to_string(),
        type_params: vec![],
        fields: vec![
            StructField {
                vis: None,
                name: "name".to_string(),
                ty: RustType::String,
            },
            StructField {
                vis: None,
                name: "age".to_string(),
                ty: RustType::F64,
            },
        ],
        is_unit_struct: false,
    };
    let expected = "\
#[derive(Debug, Clone, PartialEq)]
pub struct Foo {
    pub name: String,
    pub age: f64,
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_struct_private() {
    let item = Item::Struct {
        vis: Visibility::Private,
        name: "Bar".to_string(),
        type_params: vec![],
        fields: vec![StructField {
            vis: None,
            name: "x".to_string(),
            ty: RustType::Bool,
        }],
        is_unit_struct: false,
    };
    let expected = "\
#[derive(Debug, Clone, PartialEq)]
struct Bar {
    x: bool,
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_struct_with_type_params() {
    let item = Item::Struct {
        vis: Visibility::Public,
        name: "Container".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: None,
            default: None,
        }],
        fields: vec![StructField {
            vis: None,
            name: "value".to_string(),
            ty: RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            },
        }],
        is_unit_struct: false,
    };
    let expected = "\
#[derive(Debug, Clone, PartialEq)]
pub struct Container<T> {
    pub value: T,
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_unit_struct_marker() {
    let item = Item::Struct {
        vis: Visibility::Private,
        name: "GetCookieImpl".to_string(),
        type_params: vec![],
        fields: vec![],
        is_unit_struct: true,
    };
    assert_eq!(
        generate(&[item]),
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\nstruct GetCookieImpl;"
    );
}

#[test]
fn test_generate_non_unit_empty_struct_keeps_braces() {
    let item = Item::Struct {
        vis: Visibility::Public,
        name: "Empty".to_string(),
        type_params: vec![],
        fields: vec![],
        is_unit_struct: false,
    };
    let output = generate(&[item]);
    assert!(
        output.contains("struct Empty {"),
        "non-unit empty struct should use braces: {output}"
    );
}

#[test]
fn test_escape_ident_struct_field_reserved_word_adds_r_hash() {
    let item = Item::Struct {
        vis: Visibility::Public,
        name: "Foo".to_string(),
        type_params: vec![],
        fields: vec![StructField {
            vis: Some(Visibility::Public),
            name: "type".to_string(),
            ty: RustType::String,
        }],
        is_unit_struct: false,
    };
    let output = generate(&[item]);
    assert!(
        output.contains("r#type: String"),
        "expected r#type in: {output}"
    );
}
