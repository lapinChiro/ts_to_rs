//! Dedup-related registration tests.
//!
//! Covers [`SyntheticTypeRegistry::register_union`],
//! [`SyntheticTypeRegistry::register_inline_struct`],
//! [`SyntheticTypeRegistry::register_intersection_struct`],
//! [`SyntheticTypeRegistry::register_intersection_enum`] —
//! basic registration + idempotency + cross-origin dedup + variant-name dedup.

use super::super::*;
use super::helpers::pub_field;

// ── register_union ──

#[test]
fn test_register_union_basic() {
    let mut reg = SyntheticTypeRegistry::new();
    let name = reg.register_union(&[RustType::String, RustType::F64]);
    assert!(!name.is_empty());
    assert!(reg.get(&name).is_some());
}

#[test]
fn test_register_union_idempotent() {
    let mut reg = SyntheticTypeRegistry::new();
    let name1 = reg.register_union(&[RustType::String, RustType::F64]);
    let name2 = reg.register_union(&[RustType::String, RustType::F64]);
    assert_eq!(name1, name2, "same union should return same name");
}

#[test]
fn test_register_union_order_independent() {
    let mut reg = SyntheticTypeRegistry::new();
    let name1 = reg.register_union(&[RustType::String, RustType::F64]);
    let name2 = reg.register_union(&[RustType::F64, RustType::String]);
    assert_eq!(
        name1, name2,
        "same members in different order should return same name"
    );
}

#[test]
fn test_register_union_different_types_get_different_names() {
    let mut reg = SyntheticTypeRegistry::new();
    let name1 = reg.register_union(&[RustType::String, RustType::F64]);
    let name2 = reg.register_union(&[RustType::String, RustType::Bool]);
    assert_ne!(name1, name2);
}

#[test]
fn test_register_union_name_format() {
    let mut reg = SyntheticTypeRegistry::new();
    let name = reg.register_union(&[RustType::String, RustType::F64]);
    // Names are sorted alphabetically: F64 comes before String
    assert_eq!(name, "F64OrString");
}

#[test]
fn test_union_name_with_path_type_produces_valid_identifier() {
    let mut reg = SyntheticTypeRegistry::new();
    let name = reg.register_union(&[
        RustType::String,
        RustType::Named {
            name: "serde_json::Value".to_string(),
            type_args: vec![],
        },
    ]);
    assert_eq!(name, "StringOrValue");
}

#[test]
fn test_union_name_no_collision_with_type_args() {
    let mut reg = SyntheticTypeRegistry::new();
    let name1 = reg.register_union(&[
        RustType::Named {
            name: "Foo".to_string(),
            type_args: vec![RustType::String],
        },
        RustType::F64,
    ]);
    let name2 = reg.register_union(&[
        RustType::Named {
            name: "Foo".to_string(),
            type_args: vec![RustType::F64],
        },
        RustType::F64,
    ]);
    assert_ne!(
        name1, name2,
        "unions with different type_args should have different names"
    );
}

#[test]
fn test_register_union_deduplicates_variant_names() {
    // Two different types that produce the same variant name after path extraction:
    // "ns::Foo" and "Foo" both produce "Foo" via rsplit_once("::")
    let mut reg = SyntheticTypeRegistry::new();
    let name = reg.register_union(&[
        RustType::Named {
            name: "ns::Foo".to_string(),
            type_args: vec![],
        },
        RustType::Named {
            name: "Foo".to_string(),
            type_args: vec![],
        },
        RustType::F64,
    ]);
    let def = reg.get(&name).unwrap();
    match &def.item {
        Item::Enum { variants, .. } => {
            let names: Vec<&str> = variants.iter().map(|v| v.name.as_str()).collect();
            // Should not have duplicate "Foo" variants
            let unique_count = {
                let mut seen = vec![];
                for n in &names {
                    if !seen.contains(n) {
                        seen.push(n);
                    }
                }
                seen.len()
            };
            assert_eq!(
                names.len(),
                unique_count,
                "register_union should deduplicate variant names, got: {names:?}"
            );
        }
        _ => panic!("expected Item::Enum"),
    }
}

#[test]
fn test_register_union_fn_same_return_type_deduped() {
    // Two Fn types with same return type produce same variant name "FnBool"
    let mut reg = SyntheticTypeRegistry::new();
    let name = reg.register_union(&[
        RustType::Fn {
            params: vec![RustType::String],
            return_type: Box::new(RustType::Bool),
        },
        RustType::Fn {
            params: vec![RustType::F64],
            return_type: Box::new(RustType::Bool),
        },
    ]);
    let def = reg.get(&name).unwrap();
    match &def.item {
        Item::Enum { variants, .. } => {
            assert_eq!(
                variants.len(),
                1,
                "Fn types with same return type should be deduped to single variant"
            );
        }
        _ => panic!("expected Item::Enum"),
    }
}

// ── register_inline_struct ──

#[test]
fn test_register_inline_struct_basic() {
    let mut reg = SyntheticTypeRegistry::new();
    let name = reg.register_inline_struct(&[
        ("x".to_string(), RustType::F64),
        ("y".to_string(), RustType::String),
    ]);
    assert_eq!(name, "_TypeLit0");
    assert!(reg.get(&name).is_some());
}

#[test]
fn test_register_inline_struct_idempotent() {
    let mut reg = SyntheticTypeRegistry::new();
    let name1 = reg.register_inline_struct(&[("x".to_string(), RustType::F64)]);
    let name2 = reg.register_inline_struct(&[("x".to_string(), RustType::F64)]);
    assert_eq!(name1, name2);
}

#[test]
fn test_register_inline_struct_different_fields() {
    let mut reg = SyntheticTypeRegistry::new();
    let name1 = reg.register_inline_struct(&[("x".to_string(), RustType::F64)]);
    let name2 = reg.register_inline_struct(&[("y".to_string(), RustType::String)]);
    assert_ne!(name1, name2);
    assert_eq!(name1, "_TypeLit0");
    assert_eq!(name2, "_TypeLit1");
}

#[test]
fn test_struct_signature_normalizes_field_names() {
    // raw name "my-field" and sanitized name "my_field" should produce the same struct
    let mut reg = SyntheticTypeRegistry::new();
    let name1 = reg.register_inline_struct(&[("my-field".to_string(), RustType::F64)]);
    let name2 = reg.register_inline_struct(&[("my_field".to_string(), RustType::F64)]);
    assert_eq!(
        name1, name2,
        "raw and sanitized field names should dedup to same struct"
    );
}

// ── register_intersection_struct ──

#[test]
fn test_register_intersection_struct_basic() {
    let mut reg = SyntheticTypeRegistry::new();
    let fields = vec![
        pub_field("x", RustType::F64),
        pub_field("y", RustType::String),
    ];
    let (name, is_new) = reg.register_intersection_struct(&fields);
    assert!(is_new);
    assert!(reg.get(&name).is_some());
}

#[test]
fn test_register_intersection_struct_dedup() {
    let mut reg = SyntheticTypeRegistry::new();
    let fields = vec![pub_field("x", RustType::F64)];
    let (name1, is_new1) = reg.register_intersection_struct(&fields);
    let (name2, is_new2) = reg.register_intersection_struct(&fields);
    assert!(is_new1);
    assert!(!is_new2, "second registration should be a dedup hit");
    assert_eq!(name1, name2);
}

#[test]
fn test_intersection_struct_dedup_with_type_lit() {
    // Intersection struct and TypeLit with same fields should dedup
    let mut reg = SyntheticTypeRegistry::new();
    let type_lit_name = reg.register_inline_struct(&[("x".to_string(), RustType::F64)]);
    let (intersection_name, is_new) =
        reg.register_intersection_struct(&[pub_field("x", RustType::F64)]);
    assert!(!is_new, "should dedup with existing TypeLit");
    assert_eq!(type_lit_name, intersection_name);
}

#[test]
fn test_intersection_struct_field_order_independent() {
    let mut reg = SyntheticTypeRegistry::new();
    let (name1, _) = reg.register_intersection_struct(&[
        pub_field("a", RustType::F64),
        pub_field("b", RustType::String),
    ]);
    let (name2, is_new) = reg.register_intersection_struct(&[
        pub_field("b", RustType::String),
        pub_field("a", RustType::F64),
    ]);
    assert!(!is_new, "same fields in different order should dedup");
    assert_eq!(name1, name2);
}

// ── register_intersection_enum ──

#[test]
fn test_register_intersection_enum_basic() {
    let mut reg = SyntheticTypeRegistry::new();
    let variants = vec![
        EnumVariant {
            name: "Variant0".to_string(),
            value: None,
            data: None,
            fields: vec![pub_field("x", RustType::F64)],
        },
        EnumVariant {
            name: "Variant1".to_string(),
            value: None,
            data: None,
            fields: vec![pub_field("y", RustType::String)],
        },
    ];
    let (name, is_new) = reg.register_intersection_enum(None, variants);
    assert!(is_new);
    assert!(reg.get(&name).is_some());
}

#[test]
fn test_register_intersection_enum_dedup() {
    let mut reg = SyntheticTypeRegistry::new();
    let make_variants = || {
        vec![
            EnumVariant {
                name: "A".to_string(),
                value: Some(crate::ir::EnumValue::Str("a".to_string())),
                data: None,
                fields: vec![pub_field("x", RustType::F64)],
            },
            EnumVariant {
                name: "B".to_string(),
                value: Some(crate::ir::EnumValue::Str("b".to_string())),
                data: None,
                fields: vec![pub_field("y", RustType::String)],
            },
        ]
    };
    let (name1, is_new1) = reg.register_intersection_enum(Some("type"), make_variants());
    let (name2, is_new2) = reg.register_intersection_enum(Some("type"), make_variants());
    assert!(is_new1);
    assert!(!is_new2, "second registration should be a dedup hit");
    assert_eq!(name1, name2);
}

#[test]
fn test_register_intersection_enum_different_tag() {
    let mut reg = SyntheticTypeRegistry::new();
    let variants = vec![EnumVariant {
        name: "A".to_string(),
        value: None,
        data: None,
        fields: vec![],
    }];
    let (name1, _) = reg.register_intersection_enum(Some("type"), variants.clone());
    let (name2, _) = reg.register_intersection_enum(Some("kind"), variants);
    assert_ne!(
        name1, name2,
        "different tags should produce different enums"
    );
}

#[test]
fn test_intersection_enum_different_variants() {
    let mut reg = SyntheticTypeRegistry::new();
    let (name1, _) = reg.register_intersection_enum(
        None,
        vec![EnumVariant {
            name: "A".to_string(),
            value: None,
            data: None,
            fields: vec![pub_field("x", RustType::F64)],
        }],
    );
    let (name2, _) = reg.register_intersection_enum(
        None,
        vec![EnumVariant {
            name: "B".to_string(),
            value: None,
            data: None,
            fields: vec![pub_field("y", RustType::String)],
        }],
    );
    assert_ne!(
        name1, name2,
        "different variants should produce different enums"
    );
}
