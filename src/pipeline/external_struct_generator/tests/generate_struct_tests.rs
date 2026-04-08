use super::*;

#[test]
fn test_generate_struct_error_with_fields() {
    let mut registry = TypeRegistry::new();
    register_external_struct(
        &mut registry,
        "Error",
        vec![
            ("name", RustType::String),
            ("message", RustType::String),
            ("stack", RustType::Option(Box::new(RustType::String))),
        ],
        vec![],
    );

    let item = generate_external_struct("Error", &registry, &SyntheticTypeRegistry::new()).unwrap();
    match item {
        Item::Struct {
            vis,
            name,
            fields,
            type_params,
        } => {
            assert_eq!(vis, Visibility::Public);
            assert_eq!(name, "Error");
            assert!(type_params.is_empty());
            assert_eq!(fields.len(), 3);
            assert_eq!(fields[0].name, "name");
            assert_eq!(fields[0].ty, RustType::String);
            assert_eq!(fields[1].name, "message");
            assert_eq!(fields[1].ty, RustType::String);
            assert_eq!(fields[2].name, "stack");
            assert_eq!(fields[2].ty, RustType::Option(Box::new(RustType::String)));
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_generate_struct_field_names_snake_case() {
    let mut registry = TypeRegistry::new();
    register_external_struct(
        &mut registry,
        "RegExp",
        vec![("lastIndex", RustType::F64), ("ignoreCase", RustType::Bool)],
        vec![],
    );

    let item =
        generate_external_struct("RegExp", &registry, &SyntheticTypeRegistry::new()).unwrap();
    match item {
        Item::Struct { fields, .. } => {
            assert_eq!(fields[0].name, "last_index");
            assert_eq!(fields[1].name, "ignore_case");
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_generate_struct_typedef_function_returns_none() {
    let mut registry = TypeRegistry::new();
    registry.register(
        "fetch".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![],
            return_type: None,
            has_rest: false,
        },
    );

    assert!(generate_external_struct("fetch", &registry, &SyntheticTypeRegistry::new()).is_none());
}

#[test]
fn test_generate_struct_generic_type_params_preserved() {
    let mut registry = TypeRegistry::new();
    register_external_struct(
        &mut registry,
        "ReadableStream",
        vec![("locked", RustType::Bool)],
        vec![TypeParam {
            name: "R".to_string(),
            constraint: None,
        }],
    );

    let item = generate_external_struct("ReadableStream", &registry, &SyntheticTypeRegistry::new())
        .unwrap();
    match item {
        Item::Struct { type_params, .. } => {
            assert_eq!(type_params.len(), 1);
            assert_eq!(type_params[0].name, "R");
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_generate_struct_fields_all_public() {
    let mut registry = TypeRegistry::new();
    register_external_struct(
        &mut registry,
        "URL",
        vec![("href", RustType::String), ("hostname", RustType::String)],
        vec![],
    );

    let item = generate_external_struct("URL", &registry, &SyntheticTypeRegistry::new()).unwrap();
    match item {
        Item::Struct { fields, .. } => {
            for field in &fields {
                assert_eq!(field.vis, Some(Visibility::Public));
            }
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_generate_struct_nonexistent_type_returns_none() {
    let registry = TypeRegistry::new();
    assert!(
        generate_external_struct("NonExistent", &registry, &SyntheticTypeRegistry::new()).is_none()
    );
}

#[test]
fn test_generate_struct_empty_fields() {
    let mut registry = TypeRegistry::new();
    register_external_struct(&mut registry, "Date", vec![], vec![]);

    let item = generate_external_struct("Date", &registry, &SyntheticTypeRegistry::new()).unwrap();
    match item {
        Item::Struct { name, fields, .. } => {
            assert_eq!(name, "Date");
            assert!(fields.is_empty());
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_generate_struct_typedef_enum_returns_none() {
    let mut registry = TypeRegistry::new();
    registry.register(
        "Status".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Ok".to_string(), "Error".to_string()],
            string_values: HashMap::new(),
            tag_field: None,
            variant_fields: HashMap::new(),
        },
    );

    assert!(generate_external_struct("Status", &registry, &SyntheticTypeRegistry::new()).is_none());
}

#[test]
fn test_generate_struct_optional_field_preserved() {
    let mut registry = TypeRegistry::new();
    register_external_struct(
        &mut registry,
        "Error",
        vec![("stack", RustType::Option(Box::new(RustType::String)))],
        vec![],
    );

    let item = generate_external_struct("Error", &registry, &SyntheticTypeRegistry::new()).unwrap();
    match item {
        Item::Struct { fields, .. } => {
            assert_eq!(fields[0].ty, RustType::Option(Box::new(RustType::String)));
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_generate_struct_nested_named_type_in_field() {
    let mut registry = TypeRegistry::new();
    register_external_struct(
        &mut registry,
        "Request",
        vec![(
            "headers",
            RustType::Named {
                name: "Headers".to_string(),
                type_args: vec![],
            },
        )],
        vec![],
    );

    let item =
        generate_external_struct("Request", &registry, &SyntheticTypeRegistry::new()).unwrap();
    match item {
        Item::Struct { fields, .. } => {
            assert_eq!(
                fields[0].ty,
                RustType::Named {
                    name: "Headers".to_string(),
                    type_args: vec![],
                }
            );
        }
        _ => panic!("expected Item::Struct"),
    }
}

// =========================================================================
// T2b: generate_external_struct — monomorphization
// =========================================================================

#[test]
fn test_generate_struct_monomorphizes_non_trait_constraint() {
    // ArrayBufferView<TArrayBuffer extends ArrayBufferOrSharedArrayBuffer>
    // → ArrayBufferOrSharedArrayBuffer は synthetic union enum → 非 trait → モノモーフィゼーション
    let mut registry = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();

    // ArrayBufferOrSharedArrayBuffer を synthetic union enum として登録
    let enum_name = synthetic.register_union(&[
        RustType::Named {
            name: "ArrayBuffer".to_string(),
            type_args: vec![],
        },
        RustType::Named {
            name: "SharedArrayBuffer".to_string(),
            type_args: vec![],
        },
    ]);

    // ArrayBufferView を TypeRegistry に登録（制約付き型パラメータ）
    register_external_struct(
        &mut registry,
        "ArrayBufferView",
        vec![
            (
                "buffer",
                RustType::TypeVar {
                    name: "TArrayBuffer".to_string(),
                },
            ),
            ("byteLength", RustType::F64),
        ],
        vec![TypeParam {
            name: "TArrayBuffer".to_string(),
            constraint: Some(RustType::Named {
                name: enum_name.clone(),
                type_args: vec![],
            }),
        }],
    );

    let item = generate_external_struct("ArrayBufferView", &registry, &synthetic).unwrap();
    match item {
        Item::Struct {
            type_params,
            fields,
            ..
        } => {
            // 型パラメータがモノモーフィゼーションで除去される
            assert!(
                type_params.is_empty(),
                "non-trait constraint should be monomorphized away: {type_params:?}"
            );
            // buffer フィールドの型が制約型に置換される
            assert_eq!(
                fields[0].ty,
                RustType::Named {
                    name: enum_name,
                    type_args: vec![],
                },
                "field type should be substituted with constraint type"
            );
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_generate_struct_preserves_trait_constraint() {
    // interface 制約は trait bound として有効 → モノモーフィゼーション非適用
    let mut registry = TypeRegistry::new();
    let synthetic = SyntheticTypeRegistry::new();

    // SomeTrait を interface として登録
    registry.register(
        "SomeTrait".to_string(),
        crate::registry::TypeDef::new_interface(vec![], vec![], HashMap::new(), vec![]),
    );

    register_external_struct(
        &mut registry,
        "GenericStruct",
        vec![(
            "value",
            RustType::TypeVar {
                name: "T".to_string(),
            },
        )],
        vec![TypeParam {
            name: "T".to_string(),
            constraint: Some(RustType::Named {
                name: "SomeTrait".to_string(),
                type_args: vec![],
            }),
        }],
    );

    let item = generate_external_struct("GenericStruct", &registry, &synthetic).unwrap();
    match item {
        Item::Struct {
            type_params,
            fields,
            ..
        } => {
            // trait bound は保持される
            assert_eq!(type_params.len(), 1, "trait constraint should be preserved");
            assert_eq!(type_params[0].name, "T");
            // フィールド型は置換されない
            assert_eq!(
                fields[0].ty,
                RustType::TypeVar {
                    name: "T".to_string()
                }
            );
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_generate_struct_monomorphizes_primitive_constraint() {
    // T extends number → f64 はプリミティブ → モノモーフィゼーション
    let mut registry = TypeRegistry::new();
    let synthetic = SyntheticTypeRegistry::new();

    register_external_struct(
        &mut registry,
        "NumberBox",
        vec![(
            "value",
            RustType::TypeVar {
                name: "T".to_string(),
            },
        )],
        vec![TypeParam {
            name: "T".to_string(),
            constraint: Some(RustType::F64),
        }],
    );

    let item = generate_external_struct("NumberBox", &registry, &synthetic).unwrap();
    match item {
        Item::Struct {
            type_params,
            fields,
            ..
        } => {
            assert!(
                type_params.is_empty(),
                "primitive constraint should be monomorphized: {type_params:?}"
            );
            assert_eq!(fields[0].ty, RustType::F64);
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_generate_struct_monomorphizes_chained_constraints() {
    // U extends T, T extends number → T → f64, U → f64（チェーン制約）
    let mut registry = TypeRegistry::new();
    let synthetic = SyntheticTypeRegistry::new();

    register_external_struct(
        &mut registry,
        "Pair",
        vec![
            (
                "first",
                RustType::TypeVar {
                    name: "T".to_string(),
                },
            ),
            (
                "second",
                RustType::TypeVar {
                    name: "U".to_string(),
                },
            ),
        ],
        vec![
            TypeParam {
                name: "T".to_string(),
                constraint: Some(RustType::F64),
            },
            TypeParam {
                name: "U".to_string(),
                constraint: Some(RustType::TypeVar {
                    name: "T".to_string(),
                }),
            },
        ],
    );

    let item = generate_external_struct("Pair", &registry, &synthetic).unwrap();
    match item {
        Item::Struct {
            type_params,
            fields,
            ..
        } => {
            assert!(
                type_params.is_empty(),
                "chained constraints should be fully monomorphized: {type_params:?}"
            );
            assert_eq!(fields[0].ty, RustType::F64, "first should be f64");
            assert_eq!(fields[1].ty, RustType::F64, "second should be f64");
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_generate_struct_mixed_constrained_and_unconstrained_params() {
    // <T, U extends number> → T 保持、U モノモーフィゼーション → <T>
    let mut registry = TypeRegistry::new();
    let synthetic = SyntheticTypeRegistry::new();

    register_external_struct(
        &mut registry,
        "MixedGeneric",
        vec![
            (
                "data",
                RustType::TypeVar {
                    name: "T".to_string(),
                },
            ),
            (
                "count",
                RustType::TypeVar {
                    name: "U".to_string(),
                },
            ),
        ],
        vec![
            TypeParam {
                name: "T".to_string(),
                constraint: None,
            },
            TypeParam {
                name: "U".to_string(),
                constraint: Some(RustType::F64),
            },
        ],
    );

    let item = generate_external_struct("MixedGeneric", &registry, &synthetic).unwrap();
    match item {
        Item::Struct {
            type_params,
            fields,
            ..
        } => {
            // T は保持、U は除去
            assert_eq!(type_params.len(), 1, "only unconstrained T should remain");
            assert_eq!(type_params[0].name, "T");
            // data: T（置換なし）
            assert_eq!(
                fields[0].ty,
                RustType::TypeVar {
                    name: "T".to_string()
                }
            );
            // count: f64（U → f64 に置換）
            assert_eq!(fields[1].ty, RustType::F64);
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_generate_struct_monomorphizes_nested_type_param_reference() {
    // フィールド型が Option<T> のように型パラメータをネストして参照するケース
    // T extends number → Option<T> が Option<f64> に置換される
    let mut registry = TypeRegistry::new();
    let synthetic = SyntheticTypeRegistry::new();

    register_external_struct(
        &mut registry,
        "OptionalNumber",
        vec![(
            "value",
            RustType::Option(Box::new(RustType::TypeVar {
                name: "T".to_string(),
            })),
        )],
        vec![TypeParam {
            name: "T".to_string(),
            constraint: Some(RustType::F64),
        }],
    );

    let item = generate_external_struct("OptionalNumber", &registry, &synthetic).unwrap();
    match item {
        Item::Struct {
            type_params,
            fields,
            ..
        } => {
            assert!(type_params.is_empty());
            // Option<T> → Option<f64>
            assert_eq!(fields[0].ty, RustType::Option(Box::new(RustType::F64)));
        }
        _ => panic!("expected Item::Struct"),
    }
}
