//! `Item::Enum` rendering: numeric (auto / explicit) + string enums with
//! `as_str` / `Display`, data-carrying variants with `{}` vs `{:?}`
//! selection, and non-derivable skip (Box<dyn Fn>) omitting `#[derive]` +
//! `Display`.

use super::*;
use crate::ir::{EnumValue, EnumVariant, Item, RustType, Visibility};

#[test]
fn test_generate_enum_numeric_auto() {
    let item = Item::Enum {
        vis: Visibility::Public,
        name: "Color".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![
            EnumVariant {
                name: "Red".to_string(),
                value: None,
                data: None,
                fields: vec![],
            },
            EnumVariant {
                name: "Green".to_string(),
                value: None,
                data: None,
                fields: vec![],
            },
            EnumVariant {
                name: "Blue".to_string(),
                value: None,
                data: None,
                fields: vec![],
            },
        ],
    };
    let expected = "\
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum Color {
    Red = 0,
    Green = 1,
    Blue = 2,
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, \"{}\", *self as i64)
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_enum_numeric_explicit() {
    let item = Item::Enum {
        vis: Visibility::Public,
        name: "Status".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![
            EnumVariant {
                name: "Active".to_string(),
                value: Some(EnumValue::Number(1)),
                data: None,
                fields: vec![],
            },
            EnumVariant {
                name: "Inactive".to_string(),
                value: Some(EnumValue::Number(0)),
                data: None,
                fields: vec![],
            },
        ],
    };
    let expected = "\
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum Status {
    Active = 1,
    Inactive = 0,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, \"{}\", *self as i64)
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_enum_string() {
    let item = Item::Enum {
        vis: Visibility::Public,
        name: "Direction".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![
            EnumVariant {
                name: "Up".to_string(),
                value: Some(EnumValue::Str("UP".to_string())),
                data: None,
                fields: vec![],
            },
            EnumVariant {
                name: "Down".to_string(),
                value: Some(EnumValue::Str("DOWN".to_string())),
                data: None,
                fields: vec![],
            },
        ],
    };
    let expected = "\
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
}

impl Direction {
    pub fn as_str(&self) -> &str {
        match self {
            Direction::Up => \"UP\",
            Direction::Down => \"DOWN\",
        }
    }
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, \"{}\", self.as_str())
    }
}";
    assert_eq!(generate(&[item]), expected);
}

#[test]
fn test_generate_enum_private() {
    let item = Item::Enum {
        vis: Visibility::Private,
        name: "Color".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![EnumVariant {
            name: "Red".to_string(),
            value: None,
            data: None,
            fields: vec![],
        }],
    };
    let result = generate(&[item]);
    assert!(!result.contains("pub enum"));
    assert!(result.contains("enum Color"));
}

#[test]
fn test_generate_enum_data_variants() {
    let item = Item::Enum {
        vis: Visibility::Public,
        name: "Value".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![
            EnumVariant {
                name: "String".to_string(),
                value: None,
                data: Some(RustType::String),
                fields: vec![],
            },
            EnumVariant {
                name: "F64".to_string(),
                value: None,
                data: Some(RustType::F64),
                fields: vec![],
            },
            EnumVariant {
                name: "Bool".to_string(),
                value: None,
                data: Some(RustType::Bool),
                fields: vec![],
            },
        ],
    };
    let result = generate(&[item]);
    // derive 行は存在する（全 variant が derivable）
    assert!(result.contains("#[derive(Debug, Clone, PartialEq)]"));
    // I-007: data enum に Display impl が生成される
    assert!(
        result.contains("impl std::fmt::Display for Value"),
        "data enum should have Display impl, got:\n{result}"
    );
    // 各 variant 型に応じた Display フォーマット
    assert!(result.contains("Value::String(v) => write!(f, \"{}\", v)"));
    assert!(result.contains("Value::F64(v) => write!(f, \"{}\", v)"));
    assert!(result.contains("Value::Bool(v) => write!(f, \"{}\", v)"));
}

#[test]
fn test_generate_enum_data_non_derivable_omits_derive() {
    // I-008: Box<dyn Fn> を含む enum は Debug/Clone/PartialEq を derive できない
    let item = Item::Enum {
        vis: Visibility::Public,
        name: "StringOrFn".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![
            EnumVariant {
                name: "String".to_string(),
                value: None,
                data: Some(RustType::String),
                fields: vec![],
            },
            EnumVariant {
                name: "Fn".to_string(),
                value: None,
                data: Some(RustType::Fn {
                    params: vec![RustType::F64],
                    return_type: Box::new(RustType::String),
                }),
                fields: vec![],
            },
        ],
    };
    let result = generate(&[item]);
    // derive 行が存在しないことを検証
    assert!(
        !result.contains("#[derive("),
        "enum with Box<dyn Fn> variant should not have derive attributes, got:\n{result}"
    );
    // enum 定義自体は存在する
    assert!(result.contains("pub enum StringOrFn"));
    assert!(result.contains("Fn(Box<dyn Fn(f64) -> String>)"));
    // Display impl も生成されないこと（Debug がないため {:?} が使えない）
    assert!(
        !result.contains("impl std::fmt::Display"),
        "non-derivable enum should not have Display impl, got:\n{result}"
    );
}

#[test]
fn test_generate_enum_data_display_with_named_type() {
    // I-007: Named 型を含む data enum の Display は {:?} フォーマットを使用
    let item = Item::Enum {
        vis: Visibility::Public,
        name: "ArrayBufferOrString".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![
            EnumVariant {
                name: "ArrayBuffer".to_string(),
                value: None,
                data: Some(RustType::Named {
                    name: "ArrayBuffer".to_string(),
                    type_args: vec![],
                }),
                fields: vec![],
            },
            EnumVariant {
                name: "String".to_string(),
                value: None,
                data: Some(RustType::String),
                fields: vec![],
            },
        ],
    };
    let result = generate(&[item]);
    // derive は存在する（Named 型は derivable）
    assert!(result.contains("#[derive(Debug, Clone, PartialEq)]"));
    // Display impl が存在する
    assert!(result.contains("impl std::fmt::Display for ArrayBufferOrString"));
    // Named 型は {:?} (Debug format)
    assert!(result.contains("ArrayBufferOrString::ArrayBuffer(v) => write!(f, \"{:?}\", v)"));
    // String 型は {} (Display format)
    assert!(result.contains("ArrayBufferOrString::String(v) => write!(f, \"{}\", v)"));
}
