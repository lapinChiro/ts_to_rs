//! `const` value registration tests.
//!
//! Covers the Pass 1 `is_registrable_const_decl` filter + Pass 2 value
//! extraction for:
//! - `as const` arrays (string / number) and objects (number / string values)
//! - Explicit type annotations (`const x: Interface = ...` → `type_ref_name`)
//! - Inline type annotations (`const x: { ... } = ...` → struct fields)
//! - Non-registrable cases (`let` declarations, un-annotated `const` without
//!   `as const`)

use super::*;

// --- const value registration ---

#[test]
fn test_build_registry_const_string_array_as_const() {
    let module = parse_typescript("const TYPES = ['a', 'b', 'c'] as const;").unwrap();
    let reg = build_registry(&module);
    match reg.get("TYPES").unwrap() {
        TypeDef::ConstValue {
            elements, fields, ..
        } => {
            assert_eq!(elements.len(), 3);
            assert_eq!(elements[0].ty, RustType::String);
            assert_eq!(elements[0].string_literal_value, Some("a".to_string()));
            assert_eq!(elements[1].string_literal_value, Some("b".to_string()));
            assert_eq!(elements[2].string_literal_value, Some("c".to_string()));
            assert!(fields.is_empty());
        }
        other => panic!("expected ConstValue, got {other:?}"),
    }
}

#[test]
fn test_build_registry_const_number_array_as_const() {
    let module = parse_typescript("const NUMS = [1, 2, 3] as const;").unwrap();
    let reg = build_registry(&module);
    match reg.get("NUMS").unwrap() {
        TypeDef::ConstValue { elements, .. } => {
            assert_eq!(elements.len(), 3);
            assert_eq!(elements[0].ty, RustType::F64);
            assert!(elements[0].string_literal_value.is_none());
        }
        other => panic!("expected ConstValue, got {other:?}"),
    }
}

#[test]
fn test_build_registry_const_object_number_values_as_const() {
    let module = parse_typescript("const PHASE = { A: 1, B: 2, C: 3 } as const;").unwrap();
    let reg = build_registry(&module);
    match reg.get("PHASE").unwrap() {
        TypeDef::ConstValue {
            fields, elements, ..
        } => {
            assert_eq!(fields.len(), 3);
            assert_eq!(fields[0].name, "A");
            assert_eq!(fields[0].ty, RustType::F64);
            assert!(fields[0].string_literal_value.is_none());
            assert_eq!(fields[1].name, "B");
            assert_eq!(fields[2].name, "C");
            assert!(elements.is_empty());
        }
        other => panic!("expected ConstValue, got {other:?}"),
    }
}

#[test]
fn test_build_registry_const_object_string_values_as_const() {
    let module =
        parse_typescript("const MIMES = { aac: 'audio/aac', avi: 'video/avi' } as const;").unwrap();
    let reg = build_registry(&module);
    match reg.get("MIMES").unwrap() {
        TypeDef::ConstValue { fields, .. } => {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "aac");
            assert_eq!(fields[0].ty, RustType::String);
            assert_eq!(
                fields[0].string_literal_value,
                Some("audio/aac".to_string())
            );
            assert_eq!(fields[1].name, "avi");
            assert_eq!(fields[1].ty, RustType::String);
            assert_eq!(
                fields[1].string_literal_value,
                Some("video/avi".to_string())
            );
        }
        other => panic!("expected ConstValue, got {other:?}"),
    }
}

#[test]
fn test_build_registry_const_with_type_annotation_stores_ref_name() {
    let module = parse_typescript(
        "interface Config { x: number; y: string; }\nconst cfg: Config = { x: 1, y: 'hi' };",
    )
    .unwrap();
    let reg = build_registry(&module);
    match reg.get("cfg").unwrap() {
        TypeDef::ConstValue { type_ref_name, .. } => {
            assert_eq!(type_ref_name.as_deref(), Some("Config"));
        }
        other => panic!("expected ConstValue, got {other:?}"),
    }
}

#[test]
fn test_build_registry_const_with_inline_type_annotation() {
    let module =
        parse_typescript("const cfg: { x: number; y: string } = { x: 1, y: 'hi' };").unwrap();
    let reg = build_registry(&module);
    match reg.get("cfg").unwrap() {
        TypeDef::ConstValue { fields, .. } => {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "x");
            assert_eq!(fields[0].ty, RustType::F64);
            assert_eq!(fields[1].name, "y");
            assert_eq!(fields[1].ty, RustType::String);
        }
        other => panic!("expected ConstValue, got {other:?}"),
    }
}

#[test]
fn test_build_registry_let_var_not_registered() {
    let module = parse_typescript("let x = [1, 2, 3];").unwrap();
    let reg = build_registry(&module);
    assert!(reg.get("x").is_none());
}

#[test]
fn test_build_registry_const_no_as_const_no_annotation_not_registered() {
    let module = parse_typescript("const x = [1, 2, 3];").unwrap();
    let reg = build_registry(&module);
    assert!(reg.get("x").is_none());
}
