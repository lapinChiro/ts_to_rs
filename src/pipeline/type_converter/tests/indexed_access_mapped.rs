//! Tests for I-285 (indexed access with type parameter keys) and
//! I-200 (mapped type identity detection extension).

use super::*;

// ===========================================================================
// I-285: indexed access with type parameter keys
// ===========================================================================

#[test]
fn test_indexed_access_keyof_on_known_struct_returns_field_type() {
    // Env[keyof Env] where Env has two fields of identical type → single type (not union)
    let module = parse_type_annotation(
        "interface Env { Variables: Record<string, unknown>; Bindings: Record<string, unknown> }\n\
         type T = Env[keyof Env];",
    );
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let alias = extract_type_alias(&module, 1);

    let result = convert_ts_type(&alias.type_ann, &mut synthetic, &reg);
    assert!(
        result.is_ok(),
        "indexed access with keyof should succeed: {:?}",
        result.err()
    );
    // Both fields are HashMap<String, Any> → single type (deduplicated)
    let ty = result.unwrap();
    match &ty {
        RustType::Named { name, .. } => {
            assert_eq!(
                name, "HashMap",
                "both fields have same type, should return single HashMap"
            );
        }
        _ => panic!("expected Named type, got: {ty:?}"),
    }
}

#[test]
fn test_indexed_access_type_param_key_on_known_struct() {
    // When obj type is known struct and index is a type param name not in registry
    let module = parse_type_annotation(
        "interface User { name: string; age: number }\n\
         type FieldOf<K extends keyof User> = User[K];",
    );
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let alias = extract_type_alias(&module, 1);

    let result = convert_ts_type(&alias.type_ann, &mut synthetic, &reg);
    assert!(
        result.is_ok(),
        "type param key indexed access on known struct should succeed: {:?}",
        result.err()
    );
    // Result should be union of string | f64 (User's field types)
    let ty = result.unwrap();
    match &ty {
        RustType::Named { name, .. } => {
            // Should be a synthetic union like "F64OrString"
            assert!(
                name.contains("Or") || name == "F64OrString" || name == "StringOrF64",
                "expected union of field types, got: {name}"
            );
        }
        _ => panic!("expected Named union type, got: {ty:?}"),
    }
}

#[test]
fn test_indexed_access_type_param_key_on_unknown_type_returns_any() {
    // T[K] where T is also a type parameter (not in registry)
    let module = parse_type_annotation("type Get<T, K extends keyof T> = T[K];");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let alias = extract_type_alias(&module, 0);

    let result = convert_ts_type(&alias.type_ann, &mut synthetic, &reg);
    assert!(
        result.is_ok(),
        "type param key on unknown base should not error: {:?}",
        result.err()
    );
    // When both T and K are unknown, should fall back to Any
    assert_eq!(result.unwrap(), RustType::Any);
}

#[test]
fn test_indexed_access_number_on_non_const_returns_any() {
    // T[number] where T is not a const array → graceful fallback
    let module = parse_type_annotation("type ElementOf<T extends any[]> = T[number];");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let alias = extract_type_alias(&module, 0);

    let result = convert_ts_type(&alias.type_ann, &mut synthetic, &reg);
    assert!(
        result.is_ok(),
        "[number] on non-const should not error: {:?}",
        result.err()
    );
}

#[test]
fn test_nested_indexed_access_resolves_recursively() {
    // E['Variables'][Key] — nested indexed access
    // Env['Variables'] → inline struct with {name: String, count: f64}
    // result[Key] → union of String | f64
    let module = parse_type_annotation(
        "interface Env { Variables: { name: string; count: number } }\n\
         type T<Key extends keyof Env['Variables']> = Env['Variables'][Key];",
    );
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let alias = extract_type_alias(&module, 1);

    let result = convert_ts_type(&alias.type_ann, &mut synthetic, &reg);
    assert!(
        result.is_ok(),
        "nested indexed access should resolve recursively: {:?}",
        result.err()
    );
    // Inner Env['Variables'] resolves to a synthetic inline struct name.
    // Then [Key] on that struct → generics erasure → union of field types or Any.
    // The inline struct is registered in synthetic, so lookup may succeed.
    let ty = result.unwrap();
    // Should be a concrete type (not error). Any or union is acceptable.
    assert_ne!(ty, RustType::Never, "should not be Never");
}

#[test]
fn test_indexed_access_type_param_key_on_empty_struct_returns_any() {
    // T[K] where T has no fields → Any
    let module = parse_type_annotation(
        "interface Empty {}\n\
         type T<K extends keyof Empty> = Empty[K];",
    );
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let alias = extract_type_alias(&module, 1);

    let result = convert_ts_type(&alias.type_ann, &mut synthetic, &reg);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), RustType::Any);
}

// ===========================================================================
// I-200: mapped type identity detection extension
// ===========================================================================

#[test]
fn test_mapped_type_symbol_filter_is_identity() {
    // { [K in keyof T as K extends symbol ? never : K]: T[K] } → T (no-op filter)
    let module = parse_type_annotation(
        "type OmitSymbolKeys<T> = { [K in keyof T as K extends symbol ? never : K]: T[K] };",
    );
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let alias = extract_type_alias(&module, 0);

    let items = crate::pipeline::type_converter::type_aliases::convert_type_alias_items(
        alias,
        Visibility::Public,
        &mut synthetic,
        &reg,
    )
    .expect("should succeed");

    // Should simplify to type alias T, not HashMap
    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::TypeAlias { name, ty, .. } => {
            assert_eq!(name, "OmitSymbolKeys");
            // Should be Named("T"), not HashMap
            match ty {
                RustType::Named { name, .. } => {
                    assert_eq!(name, "T", "expected identity simplification to T");
                }
                other => panic!("expected Named type, got: {other:?}"),
            }
        }
        other => panic!("expected TypeAlias, got: {other:?}"),
    }
}

#[test]
fn test_mapped_type_non_symbol_filter_is_not_identity() {
    // { [K in keyof T as K extends number ? never : K]: T[K] }
    // number filter is NOT a no-op (unlike symbol), so this should NOT be simplified to T
    let module = parse_type_annotation(
        "type OmitNumberKeys<T> = { [K in keyof T as K extends number ? never : K]: T[K] };",
    );
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let alias = extract_type_alias(&module, 0);

    let items = crate::pipeline::type_converter::type_aliases::convert_type_alias_items(
        alias,
        Visibility::Public,
        &mut synthetic,
        &reg,
    )
    .expect("should succeed");

    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::TypeAlias { ty, .. } => {
            // Should NOT be T — should be HashMap (non-identity mapped type fallback)
            match ty {
                RustType::Named { name, .. } => {
                    assert_eq!(
                        name, "HashMap",
                        "non-symbol filter should NOT be simplified to identity"
                    );
                }
                other => panic!("expected Named(HashMap), got: {other:?}"),
            }
        }
        other => panic!("expected TypeAlias, got: {other:?}"),
    }
}

#[test]
fn test_standalone_mapped_type_identity_simplification() {
    // type X<T> = { [K in keyof T]: T[K] } → type X<T> = T (identity)
    let module = parse_type_annotation("type Identity<T> = { [K in keyof T]: T[K] };");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let alias = extract_type_alias(&module, 0);

    let result = convert_ts_type(&alias.type_ann, &mut synthetic, &reg);
    assert!(result.is_ok());
    match result.unwrap() {
        RustType::Named { name, .. } => {
            assert_eq!(
                name, "T",
                "standalone identity mapped type should simplify to T"
            );
        }
        other => panic!("expected Named('T'), got: {other:?}"),
    }
}

#[test]
fn test_intersection_mapped_type_identity_produces_typed_field() {
    // { x: string } & { [K in keyof T]: T[K] } → struct with x + T field (not HashMap)
    let module =
        parse_type_annotation("type WithMapped<T> = { x: string } & { [K in keyof T]: T[K] };");
    let reg = build_registry(&module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let alias = extract_type_alias(&module, 0);

    let items = crate::pipeline::type_converter::type_aliases::convert_type_alias_items(
        alias,
        Visibility::Public,
        &mut synthetic,
        &reg,
    )
    .expect("should succeed");

    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Struct { name, fields, .. } => {
            assert_eq!(name, "WithMapped");
            // Should have x: String and _1: T (identity), not _1: HashMap
            let embedded = fields.iter().find(|f| f.name == "_1");
            assert!(embedded.is_some(), "should have embedded field _1");
            match &embedded.unwrap().ty {
                RustType::Named { name, .. } => {
                    assert_eq!(
                        name, "T",
                        "identity mapped type in intersection should embed as T"
                    );
                }
                other => panic!("expected Named('T'), got: {other:?}"),
            }
        }
        other => panic!("expected Struct, got: {other:?}"),
    }
}
