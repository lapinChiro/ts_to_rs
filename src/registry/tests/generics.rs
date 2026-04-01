use super::*;
use crate::ir::TypeParam;

// --- I-100: Generics Foundation ---

#[test]
fn test_generic_interface_type_params_stored_in_registry() {
    let module = parse_typescript("interface Container<T> { value: T; }").unwrap();
    let reg = build_registry(&module);
    match reg.get("Container").unwrap() {
        TypeDef::Struct {
            type_params,
            fields,
            ..
        } => {
            assert_eq!(type_params.len(), 1);
            assert_eq!(type_params[0].name, "T");
            assert_eq!(type_params[0].constraint, None);
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].name, "value");
            assert!(
                matches!(&fields[0].ty, RustType::Named { name, .. } if name == "T"),
                "expected Named(T), got {:?}",
                fields[0].ty
            );
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

#[test]
fn test_generic_interface_constraint_stored_in_registry() {
    let module = parse_typescript(
        "interface Serializable { serialize(): string; } \
         interface Processor<T extends Serializable> { process(input: T): T; }",
    )
    .unwrap();
    let reg = build_registry(&module);
    match reg.get("Processor").unwrap() {
        TypeDef::Struct { type_params, .. } => {
            assert_eq!(type_params.len(), 1);
            assert_eq!(type_params[0].name, "T");
            assert_eq!(
                type_params[0].constraint,
                Some(RustType::Named {
                    name: "Serializable".to_string(),
                    type_args: vec![],
                })
            );
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

#[test]
fn test_instantiate_generic_type_substitutes_fields() {
    let module = parse_typescript("interface Container<T> { value: T; }").unwrap();
    let reg = build_registry(&module);
    let instantiated = reg
        .instantiate("Container", &[RustType::String])
        .expect("instantiate should succeed");
    match instantiated {
        TypeDef::Struct { fields, .. } => {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].name, "value");
            assert_eq!(fields[0].ty, RustType::String);
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

#[test]
fn test_instantiate_non_generic_returns_original() {
    let module = parse_typescript("interface Point { x: number; y: number; }").unwrap();
    let reg = build_registry(&module);
    let original = reg.get("Point").unwrap().clone();
    let instantiated = reg
        .instantiate("Point", &[RustType::String])
        .expect("instantiate should succeed");
    assert_eq!(instantiated, original);
}

#[test]
fn test_instantiate_arg_count_mismatch_returns_original() {
    let module = parse_typescript("interface Container<T> { value: T; }").unwrap();
    let reg = build_registry(&module);
    let original = reg.get("Container").unwrap().clone();
    let instantiated = reg
        .instantiate("Container", &[RustType::String, RustType::F64])
        .expect("instantiate should succeed");
    assert_eq!(instantiated, original);
}

// --- I-218: class / type alias 型パラメータ収集 ---

#[test]
fn test_collect_class_type_params_single() {
    let module = parse_typescript("class Foo<T> { value: T; }").unwrap();
    let reg = build_registry(&module);
    match reg.get("Foo").unwrap() {
        TypeDef::Struct {
            type_params,
            fields,
            ..
        } => {
            assert_eq!(type_params.len(), 1);
            assert_eq!(type_params[0].name, "T");
            assert_eq!(type_params[0].constraint, None);
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].name, "value");
            assert!(
                matches!(&fields[0].ty, RustType::Named { name, .. } if name == "T"),
                "expected Named(T), got {:?}",
                fields[0].ty
            );
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

#[test]
fn test_collect_class_type_params_multiple_with_constraint() {
    let module = parse_typescript(
        "interface Bar { name: string; } \
         class Foo<T extends Bar, U> { first: T; second: U; }",
    )
    .unwrap();
    let reg = build_registry(&module);
    match reg.get("Foo").unwrap() {
        TypeDef::Struct { type_params, .. } => {
            assert_eq!(type_params.len(), 2);
            assert_eq!(type_params[0].name, "T");
            assert_eq!(
                type_params[0].constraint,
                Some(RustType::Named {
                    name: "Bar".to_string(),
                    type_args: vec![],
                })
            );
            assert_eq!(type_params[1].name, "U");
            assert_eq!(type_params[1].constraint, None);
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

#[test]
fn test_collect_type_alias_struct_type_params() {
    let module = parse_typescript("type Pair<A, B> = { first: A; second: B; }").unwrap();
    let reg = build_registry(&module);
    match reg.get("Pair").unwrap() {
        TypeDef::Struct {
            type_params,
            fields,
            ..
        } => {
            assert_eq!(type_params.len(), 2);
            assert_eq!(type_params[0].name, "A");
            assert_eq!(type_params[1].name, "B");
            assert_eq!(fields.len(), 2);
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

#[test]
fn test_collect_type_alias_du_enum_type_params() {
    let module = parse_typescript(
        r#"type Result<T> = { kind: "ok"; value: T } | { kind: "error"; msg: string }"#,
    )
    .unwrap();
    let reg = build_registry(&module);
    match reg.get("Result").unwrap() {
        TypeDef::Enum {
            type_params,
            variants,
            tag_field,
            variant_fields,
            ..
        } => {
            assert_eq!(type_params.len(), 1);
            assert_eq!(type_params[0].name, "T");
            assert_eq!(tag_field.as_deref(), Some("kind"));
            assert_eq!(variants.len(), 2);
            let ok_fields = variant_fields.get("Ok").expect("Ok variant should exist");
            assert!(
                ok_fields.iter().any(|f| f.name == "value"
                    && matches!(f.ty, RustType::Named { ref name, .. } if name == "T")),
                "expected Ok variant to have field 'value: T', got {ok_fields:?}"
            );
        }
        other => panic!("expected Enum, got {other:?}"),
    }
}

// --- substitute_types ---

#[test]
fn test_substitute_types_enum_variant_fields() {
    let enum_def = TypeDef::Enum {
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: None,
        }],
        variants: vec!["Ok".to_string(), "Error".to_string()],
        string_values: HashMap::new(),
        tag_field: Some("kind".to_string()),
        variant_fields: HashMap::from([
            (
                "Ok".to_string(),
                vec![(
                    "value".to_string(),
                    RustType::Named {
                        name: "T".to_string(),
                        type_args: vec![],
                    },
                )
                    .into()],
            ),
            (
                "Error".to_string(),
                vec![("msg".to_string(), RustType::String).into()],
            ),
        ]),
    };
    let bindings = HashMap::from([("T".to_string(), RustType::String)]);
    let result = enum_def.substitute_types(&bindings);
    match &result {
        TypeDef::Enum { variant_fields, .. } => {
            let ok_fields = variant_fields.get("Ok").unwrap();
            assert_eq!(
                ok_fields[0].ty,
                RustType::String,
                "T should be substituted to String"
            );
            let err_fields = variant_fields.get("Error").unwrap();
            assert_eq!(
                err_fields[0].ty,
                RustType::String,
                "String should remain unchanged"
            );
        }
        other => panic!("expected Enum, got {other:?}"),
    }
}

#[test]
fn test_substitute_types_enum_multiple_params() {
    let enum_def = TypeDef::Enum {
        type_params: vec![
            TypeParam {
                name: "T".to_string(),
                constraint: None,
            },
            TypeParam {
                name: "E".to_string(),
                constraint: None,
            },
        ],
        variants: vec!["Ok".to_string(), "Err".to_string()],
        string_values: HashMap::new(),
        tag_field: Some("kind".to_string()),
        variant_fields: HashMap::from([
            (
                "Ok".to_string(),
                vec![(
                    "value".to_string(),
                    RustType::Named {
                        name: "T".to_string(),
                        type_args: vec![],
                    },
                )
                    .into()],
            ),
            (
                "Err".to_string(),
                vec![(
                    "error".to_string(),
                    RustType::Named {
                        name: "E".to_string(),
                        type_args: vec![],
                    },
                )
                    .into()],
            ),
        ]),
    };
    let bindings = HashMap::from([
        ("T".to_string(), RustType::String),
        ("E".to_string(), RustType::F64),
    ]);
    let result = enum_def.substitute_types(&bindings);
    match &result {
        TypeDef::Enum { variant_fields, .. } => {
            let ok_fields = variant_fields.get("Ok").unwrap();
            assert_eq!(ok_fields[0].ty, RustType::String);
            let err_fields = variant_fields.get("Err").unwrap();
            assert_eq!(err_fields[0].ty, RustType::F64);
        }
        other => panic!("expected Enum, got {other:?}"),
    }
}
