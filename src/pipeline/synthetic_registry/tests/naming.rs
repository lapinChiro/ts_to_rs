//! [`super::super::variant_name_for_type`] + [`super::super::to_pascal_case`]
//! formatting tests.
//!
//! Covers Named (with / without paths, with type args) / DynTrait / Tuple / Result /
//! Fn variants and the `_TypeLit{N}` / `Or` concatenation conventions.

use super::super::*;

// ── to_pascal_case ──

#[test]
fn test_to_pascal_case() {
    assert_eq!(to_pascal_case("process_data"), "ProcessData");
    assert_eq!(to_pascal_case("processData"), "ProcessData");
    assert_eq!(to_pascal_case("hono-base"), "HonoBase");
}

// ── variant_name_for_type: path extraction ──

#[test]
fn test_variant_name_named_with_path_uses_last_segment() {
    let ty = RustType::Named {
        name: "serde_json::Value".to_string(),
        type_args: vec![],
    };
    assert_eq!(variant_name_for_type(&ty), "Value");
}

#[test]
fn test_variant_name_named_without_path_unchanged() {
    let ty = RustType::Named {
        name: "String".to_string(),
        type_args: vec![],
    };
    assert_eq!(variant_name_for_type(&ty), "String");
}

#[test]
fn test_variant_name_dyn_trait_with_path_uses_last_segment() {
    let ty = RustType::DynTrait("std::fmt::Display".to_string());
    assert_eq!(variant_name_for_type(&ty), "Display");
}

#[test]
fn test_variant_name_dyn_trait_without_path_unchanged() {
    let ty = RustType::DynTrait("Fn".to_string());
    assert_eq!(variant_name_for_type(&ty), "Fn");
}

// ── variant_name_for_type: type args ──

#[test]
fn test_variant_name_named_with_type_args() {
    let ty = RustType::Named {
        name: "Foo".to_string(),
        type_args: vec![RustType::String],
    };
    assert_eq!(variant_name_for_type(&ty), "FooString");
}

#[test]
fn test_variant_name_named_with_multiple_type_args() {
    let ty = RustType::Named {
        name: "Map".to_string(),
        type_args: vec![RustType::String, RustType::F64],
    };
    assert_eq!(variant_name_for_type(&ty), "MapStringF64");
}

#[test]
fn test_variant_name_named_different_type_args_differ() {
    let ty1 = RustType::Named {
        name: "Foo".to_string(),
        type_args: vec![RustType::String],
    };
    let ty2 = RustType::Named {
        name: "Foo".to_string(),
        type_args: vec![RustType::F64],
    };
    assert_ne!(
        variant_name_for_type(&ty1),
        variant_name_for_type(&ty2),
        "different type_args should produce different variant names"
    );
}

// ── variant_name_for_type: Tuple ──

#[test]
fn test_variant_name_tuple_with_elements() {
    let ty = RustType::Tuple(vec![RustType::String, RustType::F64]);
    assert_eq!(variant_name_for_type(&ty), "TupleStringF64");
}

#[test]
fn test_variant_name_tuple_empty() {
    let ty = RustType::Tuple(vec![]);
    assert_eq!(variant_name_for_type(&ty), "Tuple");
}

#[test]
fn test_variant_name_tuple_different_elements_differ() {
    let ty1 = RustType::Tuple(vec![RustType::String, RustType::F64]);
    let ty2 = RustType::Tuple(vec![RustType::Bool]);
    assert_ne!(variant_name_for_type(&ty1), variant_name_for_type(&ty2));
}

// ── variant_name_for_type: Result ──

#[test]
fn test_variant_name_result() {
    let ty = RustType::Result {
        ok: Box::new(RustType::String),
        err: Box::new(RustType::Any),
    };
    assert_eq!(variant_name_for_type(&ty), "ResultStringAny");
}

#[test]
fn test_variant_name_result_different_types_differ() {
    let ty1 = RustType::Result {
        ok: Box::new(RustType::String),
        err: Box::new(RustType::Any),
    };
    let ty2 = RustType::Result {
        ok: Box::new(RustType::F64),
        err: Box::new(RustType::Any),
    };
    assert_ne!(variant_name_for_type(&ty1), variant_name_for_type(&ty2));
}

// ── variant_name_for_type: Fn ──

#[test]
fn test_variant_name_fn_includes_return_type() {
    let ty = RustType::Fn {
        params: vec![RustType::String],
        return_type: Box::new(RustType::Bool),
    };
    assert_eq!(variant_name_for_type(&ty), "FnBool");
}

#[test]
fn test_variant_name_fn_different_return_types_differ() {
    let ty1 = RustType::Fn {
        params: vec![],
        return_type: Box::new(RustType::Bool),
    };
    let ty2 = RustType::Fn {
        params: vec![],
        return_type: Box::new(RustType::String),
    };
    assert_ne!(variant_name_for_type(&ty1), variant_name_for_type(&ty2));
}
