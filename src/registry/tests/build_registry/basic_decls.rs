//! Basic declaration registration tests: interface / type alias / enum /
//! export / optional field / empty module / forward reference /
//! intersection merge.
//!
//! All tests exercise the Pass 1 + Pass 2a main path of `build_registry`
//! on syntactically simple sources where resolution succeeds without
//! intersection decomposition or callable-interface classification.

use super::*;

// ── build_registry: interface / type alias / enum ──

#[test]
fn test_build_registry_interface() {
    let module = parse_typescript("interface Point { x: number; y: number; }").unwrap();
    let reg = build_registry(&module);
    assert_eq!(
        reg.get("Point").unwrap(),
        &TypeDef::new_interface(
            vec![],
            vec![
                ("x".to_string(), RustType::F64).into(),
                ("y".to_string(), RustType::F64).into(),
            ],
            HashMap::new(),
            vec![],
        )
    );
}

#[test]
fn test_build_registry_type_alias_object() {
    let module = parse_typescript("type Config = { name: string; count: number; };").unwrap();
    let reg = build_registry(&module);
    assert_eq!(
        reg.get("Config").unwrap(),
        &TypeDef::new_struct(
            vec![
                ("name".to_string(), RustType::String).into(),
                ("count".to_string(), RustType::F64).into(),
            ],
            HashMap::new(),
            vec![],
        )
    );
}

/// Generic type alias のフィールド型で型パラメータが `TypeVar` として解決されることを検証。
///
/// regression guard: パス A の `resolve_struct_for_registry` で scope push が欠落すると、
/// 型パラメータ `K` が `Named { name: "K" }` として registry に格納され、
/// downstream の `unique_field_types()` → synthetic union で dangling ref になる。
/// (Phase D probe で `P` leak として発見、D-0.5 で修正)
///
/// TypeCollector (registry 登録) は generic 定義をそのまま格納する。monomorphize は
/// TypeConverter (IR 生成) 側の責務であり、registry には TypeVar が残る。
#[test]
fn test_build_registry_generic_type_alias_fields_use_type_var() {
    let module =
        parse_typescript("type Dict<K extends string, V> = { key: K; value: V; };").unwrap();
    let mut synthetic = crate::pipeline::SyntheticTypeRegistry::new();
    let reg = build_registry_with_synthetic(&module, &mut synthetic);
    let typedef = reg.get("Dict").unwrap();
    match typedef {
        TypeDef::Struct { fields, .. } => {
            let key_field = fields.iter().find(|f| f.name == "key").expect("key field");
            let value_field = fields
                .iter()
                .find(|f| f.name == "value")
                .expect("value field");
            // Registry は generic 定義を格納。K, V ともに TypeVar として残る。
            // Named ではなく TypeVar であることが重要 (Named だと dangling ref になる)。
            assert!(
                matches!(&key_field.ty, RustType::TypeVar { name } if name == "K"),
                "K should be TypeVar in registry (not Named), got: {:?}",
                key_field.ty
            );
            assert!(
                matches!(&value_field.ty, RustType::TypeVar { name } if name == "V"),
                "V should be TypeVar in registry, got: {:?}",
                value_field.ty
            );
        }
        other => panic!("expected Struct, got: {other:?}"),
    }
}

/// Generic type alias + Record のフィールドで型パラメータが TypeVar として格納されることを検証。
///
/// `type X<P extends string> = { param: Record<P, string> }` の場合、registry は
/// generic 定義を格納するため `HashMap<TypeVar("P"), String>` になる。
/// monomorphize (P → String) は TypeConverter 側で適用される。
///
/// regression guard: scope 欠落時は `P` が `Named { "P" }` → `TypeRefCollector` が拾い
/// → synthetic union → dangling ref という連鎖が発生する。
#[test]
fn test_build_registry_generic_type_alias_record_field_has_type_var() {
    let module = parse_typescript(
        "type Targets<P extends string = string> = { param: Record<P, string>; };",
    )
    .unwrap();
    let mut synthetic = crate::pipeline::SyntheticTypeRegistry::new();
    let reg = build_registry_with_synthetic(&module, &mut synthetic);
    let typedef = reg.get("Targets").unwrap();
    match typedef {
        TypeDef::Struct { fields, .. } => {
            let param_field = fields
                .iter()
                .find(|f| f.name == "param")
                .expect("param field");
            // Registry は generic 定義を格納: Record<P, string> → HashMap<TypeVar("P"), String>
            assert_eq!(
                param_field.ty,
                RustType::StdCollection {
                    kind: crate::ir::StdCollectionKind::HashMap,
                    args: vec![
                        RustType::TypeVar {
                            name: "P".to_string()
                        },
                        RustType::String,
                    ],
                },
                "Record<P, string> should be HashMap<TypeVar(P), String> in registry"
            );
        }
        other => panic!("expected Struct, got: {other:?}"),
    }
}

#[test]
fn test_build_registry_enum() {
    let module = parse_typescript("enum Color { Red, Green, Blue }").unwrap();
    let reg = build_registry(&module);
    assert_eq!(
        reg.get("Color").unwrap(),
        &TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Red".to_string(), "Green".to_string(), "Blue".to_string(),],
            string_values: HashMap::new(),
            tag_field: None,
            variant_fields: HashMap::new(),
        }
    );
}

#[test]
fn test_build_registry_export_declarations() {
    let module =
        parse_typescript("export interface Foo { x: number; }\nexport enum Bar { A, B }").unwrap();
    let reg = build_registry(&module);
    assert!(reg.get("Foo").is_some());
    assert!(reg.get("Bar").is_some());
}

#[test]
fn test_build_registry_optional_field() {
    let module = parse_typescript("interface Config { name?: string; }").unwrap();
    let reg = build_registry(&module);
    assert_eq!(
        reg.get("Config").unwrap(),
        &TypeDef::new_interface(
            vec![],
            vec![FieldDef {
                name: "name".to_string(),
                ty: RustType::Option(Box::new(RustType::String)),
                optional: true,
            }],
            HashMap::new(),
            vec![],
        )
    );
}

#[test]
fn test_build_registry_empty_module() {
    let module = parse_typescript("").unwrap();
    let reg = build_registry(&module);
    assert!(reg.get("anything").is_none());
}

#[test]
fn test_build_registry_forward_reference_resolves_type() {
    let module = parse_typescript("interface A { b: B } interface B { x: number; }").unwrap();
    let reg = build_registry(&module);

    match reg.get("A").unwrap() {
        TypeDef::Struct { fields, .. } => {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].name, "b");
            assert!(matches!(&fields[0].ty, RustType::Named { name, .. } if name == "B"));
        }
        other => panic!("expected Struct, got: {:?}", other),
    }
    assert!(reg.get("B").is_some());
}

// --- intersection type registration ---

#[test]
fn test_build_registry_intersection_type_alias_merges_fields() {
    let module = parse_typescript(
        "interface Named { name: string; } interface Aged { age: number; } type Person = Named & Aged;",
    )
    .unwrap();
    let reg = build_registry(&module);
    let person = reg.get("Person").expect("Person should be registered");
    match person {
        TypeDef::Struct { fields, .. } => {
            assert_eq!(fields.len(), 2, "expected 2 merged fields");
            assert!(
                fields
                    .iter()
                    .any(|f| f.name == "name" && f.ty == RustType::String),
                "expected name: String"
            );
            assert!(
                fields
                    .iter()
                    .any(|f| f.name == "age" && f.ty == RustType::F64),
                "expected age: f64"
            );
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}
