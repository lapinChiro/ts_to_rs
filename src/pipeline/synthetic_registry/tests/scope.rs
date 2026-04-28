//! Type param scope management + walker-based detection tests.
//!
//! Covers `push_type_param_scope` / `restore_type_param_scope` / `is_in_type_param_scope`
//! and the walker that propagates `RustType::TypeVar` references found in member /
//! field / variant types into the registered Item's `type_params` (I-387 / I-383).

use super::super::*;
use super::helpers::pub_field;

// ── walker: type-var detection from member / field / variant types ──

#[test]
fn test_register_union_detects_type_vars_from_member_types() {
    // I-387: extract_used_type_params は scope ではなく member 型の TypeVar variant
    // を walk して型パラメータを収集する。
    let mut reg = SyntheticTypeRegistry::new();
    let name = reg.register_union(&[
        RustType::TypeVar {
            name: "T".to_string(),
        },
        RustType::Vec(Box::new(RustType::TypeVar {
            name: "T".to_string(),
        })),
    ]);
    let def = reg.get(&name).unwrap();
    match &def.item {
        Item::Enum { type_params, .. } => {
            assert_eq!(type_params.len(), 1, "should detect T from TypeVar walk");
            assert_eq!(type_params[0].name, "T");
        }
        _ => panic!("expected Item::Enum for type param scope test"),
    }
}

#[test]
fn test_register_inline_struct_detects_type_vars_from_field_types() {
    // I-387: walker-only。field の TypeVar variant が type_params に伝播。
    let mut reg = SyntheticTypeRegistry::new();
    let name = reg.register_inline_struct(&[
        (
            "x".to_string(),
            RustType::TypeVar {
                name: "T".to_string(),
            },
        ),
        ("y".to_string(), RustType::F64),
    ]);
    let def = reg.get(&name).unwrap();
    match &def.item {
        Item::Struct { type_params, .. } => {
            assert_eq!(
                type_params.len(),
                1,
                "inline struct should detect T from TypeVar walk"
            );
            assert_eq!(type_params[0].name, "T");
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_register_intersection_struct_detects_type_vars_from_field_types() {
    // I-387: walker-only。
    let mut reg = SyntheticTypeRegistry::new();
    let fields = vec![
        pub_field(
            "a",
            RustType::TypeVar {
                name: "U".to_string(),
            },
        ),
        pub_field("b", RustType::String),
    ];
    let (name, _is_new) = reg.register_intersection_struct(&fields);
    let def = reg.get(&name).unwrap();
    match &def.item {
        Item::Struct { type_params, .. } => {
            assert_eq!(
                type_params.len(),
                1,
                "intersection struct should detect U from TypeVar walk"
            );
            assert_eq!(type_params[0].name, "U");
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_register_intersection_enum_detects_type_vars_from_variant_fields() {
    // I-387: walker-only。
    let mut reg = SyntheticTypeRegistry::new();
    let variants = vec![
        EnumVariant {
            name: "Variant0".to_string(),
            value: None,
            data: None,
            fields: vec![pub_field(
                "x",
                RustType::TypeVar {
                    name: "S".to_string(),
                },
            )],
        },
        EnumVariant {
            name: "Variant1".to_string(),
            value: None,
            data: None,
            fields: vec![pub_field("y", RustType::F64)],
        },
    ];
    let (name, _is_new) = reg.register_intersection_enum(None, variants);
    let def = reg.get(&name).unwrap();
    match &def.item {
        Item::Enum { type_params, .. } => {
            assert_eq!(
                type_params.len(),
                1,
                "intersection enum should detect S from TypeVar walk"
            );
            assert_eq!(type_params[0].name, "S");
        }
        _ => panic!("expected Item::Enum"),
    }
}

#[test]
fn test_register_union_no_type_params_when_scope_empty() {
    let mut reg = SyntheticTypeRegistry::new();
    // No type param scope set
    let name = reg.register_union(&[
        RustType::Named {
            name: "T".to_string(),
            type_args: vec![],
        },
        RustType::F64,
    ]);
    let def = reg.get(&name).unwrap();
    match &def.item {
        Item::Enum { type_params, .. } => {
            assert!(
                type_params.is_empty(),
                "without scope, no type params should be detected"
            );
        }
        _ => panic!("expected Item::Enum for empty scope test"),
    }
}

// ── push / restore / is_in_type_param_scope ──

// I-383 T3: push_type_param_scope の append-merge 意味論
#[test]
fn test_push_type_param_scope_appends_to_existing() {
    let mut reg = SyntheticTypeRegistry::new();
    let prev1 = reg.push_type_param_scope(vec!["S".to_string()]);
    assert!(prev1.is_empty());
    assert!(reg.is_in_type_param_scope("S"));

    // 内側の scope: T を追加すると、S と T の両方が見えるべき
    let prev2 = reg.push_type_param_scope(vec!["T".to_string()]);
    assert_eq!(prev2, vec!["S".to_string()]);
    assert!(reg.is_in_type_param_scope("S"), "outer S still visible");
    assert!(reg.is_in_type_param_scope("T"), "inner T visible");

    // restore で内側 scope を抜けると T は消える
    reg.restore_type_param_scope(prev2);
    assert!(reg.is_in_type_param_scope("S"));
    assert!(!reg.is_in_type_param_scope("T"));

    // restore で外側 scope を抜けると空
    reg.restore_type_param_scope(prev1);
    assert!(!reg.is_in_type_param_scope("S"));
}

#[test]
fn test_push_type_param_scope_idempotent_on_duplicate() {
    let mut reg = SyntheticTypeRegistry::new();
    reg.push_type_param_scope(vec!["T".to_string()]);
    let prev = reg.push_type_param_scope(vec!["T".to_string()]);
    assert_eq!(prev, vec!["T".to_string()]);
    // 重複する push でも scope に T が 1 つだけ存在する
    assert!(reg.is_in_type_param_scope("T"));
    reg.restore_type_param_scope(prev);
    assert!(reg.is_in_type_param_scope("T"));
}

// I-383 T4: is_in_type_param_scope 公開 API
#[test]
fn test_is_in_type_param_scope_empty() {
    let reg = SyntheticTypeRegistry::new();
    assert!(!reg.is_in_type_param_scope("T"));
}

#[test]
fn test_is_in_type_param_scope_multiple() {
    let mut reg = SyntheticTypeRegistry::new();
    reg.push_type_param_scope(vec!["T".to_string(), "U".to_string(), "V".to_string()]);
    assert!(reg.is_in_type_param_scope("T"));
    assert!(reg.is_in_type_param_scope("U"));
    assert!(reg.is_in_type_param_scope("V"));
    assert!(!reg.is_in_type_param_scope("W"));
}
