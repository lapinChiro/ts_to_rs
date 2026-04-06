use super::*;
use crate::ir::{
    AssocConst, BinOp, ClosureBody, EnumVariant, Expr, MatchArm, Method, Param, Stmt, TraitRef,
    TypeParam,
};
use crate::pipeline::SyntheticTypeRegistry;
use std::collections::HashMap;

/// テスト用に TypeRegistry に外部型としてフィールド付き struct 型を登録するヘルパー。
fn register_external_struct(
    registry: &mut TypeRegistry,
    name: &str,
    fields: Vec<(&str, RustType)>,
    type_params: Vec<TypeParam>,
) {
    registry.register_external(
        name.to_string(),
        TypeDef::Struct {
            type_params,
            fields: fields
                .into_iter()
                .map(|(n, ty)| (n.to_string(), ty))
                .map(Into::into)
                .collect(),
            methods: HashMap::new(),
            constructor: None,
            call_signatures: vec![],
            extends: vec![],
            is_interface: true,
        },
    );
}

// =========================================================================
// T1: collect_undefined_type_references
// =========================================================================

#[test]
fn test_collect_refs_enum_variant_named_type_detected() {
    let mut registry = TypeRegistry::new();
    register_external_struct(&mut registry, "Date", vec![], vec![]);

    let items = vec![Item::Enum {
        vis: Visibility::Public,
        name: "MyEnum".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![EnumVariant {
            name: "Date".to_string(),
            value: None,
            data: Some(RustType::Named {
                name: "Date".to_string(),
                type_args: vec![],
            }),
            fields: vec![],
        }],
    }];

    let refs = collect_undefined_type_references(&items, &[], &[], &registry);
    assert_eq!(refs, HashSet::from(["Date".to_string()]));
}

#[test]
fn test_collect_refs_rust_stdlib_types_excluded() {
    let registry = TypeRegistry::new();

    let items = vec![Item::Enum {
        vis: Visibility::Public,
        name: "MyEnum".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![
            EnumVariant {
                name: "S".to_string(),
                value: None,
                data: Some(RustType::String),
                fields: vec![],
            },
            EnumVariant {
                name: "N".to_string(),
                value: None,
                data: Some(RustType::F64),
                fields: vec![],
            },
            EnumVariant {
                name: "B".to_string(),
                value: None,
                data: Some(RustType::Bool),
                fields: vec![],
            },
        ],
    }];

    let refs = collect_undefined_type_references(&items, &[], &[], &registry);
    assert!(refs.is_empty());
}

#[test]
fn test_collect_refs_serde_json_value_excluded() {
    let registry = TypeRegistry::new();

    let items = vec![Item::Enum {
        vis: Visibility::Public,
        name: "MyEnum".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![EnumVariant {
            name: "Other".to_string(),
            value: None,
            data: Some(RustType::Named {
                name: "serde_json::Value".to_string(),
                type_args: vec![],
            }),
            fields: vec![],
        }],
    }];

    let refs = collect_undefined_type_references(&items, &[], &[], &registry);
    assert!(refs.is_empty());
}

#[test]
fn test_collect_refs_defined_struct_excluded() {
    let mut registry = TypeRegistry::new();
    register_external_struct(&mut registry, "Foo", vec![], vec![]);

    let items = vec![
        Item::Struct {
            vis: Visibility::Public,
            name: "Foo".to_string(),
            type_params: vec![],
            fields: vec![],
        },
        Item::Enum {
            vis: Visibility::Public,
            name: "MyEnum".to_string(),
            type_params: vec![],
            serde_tag: None,
            variants: vec![EnumVariant {
                name: "Foo".to_string(),
                value: None,
                data: Some(RustType::Named {
                    name: "Foo".to_string(),
                    type_args: vec![],
                }),
                fields: vec![],
            }],
        },
    ];

    let refs = collect_undefined_type_references(&items, &[], &[], &registry);
    assert!(refs.is_empty());
}

#[test]
fn test_collect_refs_nested_type_args_detected() {
    let mut registry = TypeRegistry::new();
    register_external_struct(&mut registry, "ArrayBuffer", vec![], vec![]);

    let items = vec![Item::Struct {
        vis: Visibility::Public,
        name: "MyStruct".to_string(),
        type_params: vec![],
        fields: vec![StructField {
            vis: Some(Visibility::Public),
            name: "data".to_string(),
            ty: RustType::Vec(Box::new(RustType::Named {
                name: "ArrayBuffer".to_string(),
                type_args: vec![],
            })),
        }],
    }];

    let refs = collect_undefined_type_references(&items, &[], &[], &registry);
    assert_eq!(refs, HashSet::from(["ArrayBuffer".to_string()]));
}

#[test]
fn test_collect_refs_struct_field_named_type_detected() {
    let mut registry = TypeRegistry::new();
    register_external_struct(&mut registry, "Headers", vec![], vec![]);

    let items = vec![Item::Struct {
        vis: Visibility::Public,
        name: "MyStruct".to_string(),
        type_params: vec![],
        fields: vec![StructField {
            vis: Some(Visibility::Public),
            name: "headers".to_string(),
            ty: RustType::Named {
                name: "Headers".to_string(),
                type_args: vec![],
            },
        }],
    }];

    let refs = collect_undefined_type_references(&items, &[], &[], &registry);
    assert_eq!(refs, HashSet::from(["Headers".to_string()]));
}

#[test]
fn test_collect_refs_not_in_registry_excluded() {
    let registry = TypeRegistry::new();

    let items = vec![Item::Enum {
        vis: Visibility::Public,
        name: "MyEnum".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![EnumVariant {
            name: "Unknown".to_string(),
            value: None,
            data: Some(RustType::Named {
                name: "Unknown".to_string(),
                type_args: vec![],
            }),
            fields: vec![],
        }],
    }];

    let refs = collect_undefined_type_references(&items, &[], &[], &registry);
    assert!(refs.is_empty());
}

#[test]
fn test_collect_refs_user_defined_type_excluded() {
    let mut registry = TypeRegistry::new();
    // register（not register_external）で登録 → ユーザー定義型
    registry.register(
        "Bindings".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![("db_url".to_string(), RustType::String).into()],
            methods: HashMap::new(),
            constructor: None,
            call_signatures: vec![],
            extends: vec![],
            is_interface: true,
        },
    );

    let items = vec![Item::Enum {
        vis: Visibility::Public,
        name: "MyEnum".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![EnumVariant {
            name: "Bindings".to_string(),
            value: None,
            data: Some(RustType::Named {
                name: "Bindings".to_string(),
                type_args: vec![],
            }),
            fields: vec![],
        }],
    }];

    let refs = collect_undefined_type_references(&items, &[], &[], &registry);
    assert!(refs.is_empty(), "user-defined types should not be included");
}

#[test]
fn test_collect_refs_multiple_types_collected() {
    let mut registry = TypeRegistry::new();
    register_external_struct(&mut registry, "Date", vec![], vec![]);
    register_external_struct(&mut registry, "Error", vec![], vec![]);
    register_external_struct(&mut registry, "RegExp", vec![], vec![]);

    let items = vec![Item::Enum {
        vis: Visibility::Public,
        name: "MyEnum".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![
            EnumVariant {
                name: "Date".to_string(),
                value: None,
                data: Some(RustType::Named {
                    name: "Date".to_string(),
                    type_args: vec![],
                }),
                fields: vec![],
            },
            EnumVariant {
                name: "Error".to_string(),
                value: None,
                data: Some(RustType::Named {
                    name: "Error".to_string(),
                    type_args: vec![],
                }),
                fields: vec![],
            },
            EnumVariant {
                name: "RegExp".to_string(),
                value: None,
                data: Some(RustType::Named {
                    name: "RegExp".to_string(),
                    type_args: vec![],
                }),
                fields: vec![],
            },
        ],
    }];

    let refs = collect_undefined_type_references(&items, &[], &[], &registry);
    assert_eq!(
        refs,
        HashSet::from([
            "Date".to_string(),
            "Error".to_string(),
            "RegExp".to_string()
        ])
    );
}

#[test]
fn test_collect_refs_option_nested_type_detected() {
    let mut registry = TypeRegistry::new();
    register_external_struct(&mut registry, "Blob", vec![], vec![]);

    let items = vec![Item::Struct {
        vis: Visibility::Public,
        name: "MyStruct".to_string(),
        type_params: vec![],
        fields: vec![StructField {
            vis: Some(Visibility::Public),
            name: "data".to_string(),
            ty: RustType::Option(Box::new(RustType::Named {
                name: "Blob".to_string(),
                type_args: vec![],
            })),
        }],
    }];

    let refs = collect_undefined_type_references(&items, &[], &[], &registry);
    assert_eq!(refs, HashSet::from(["Blob".to_string()]));
}

#[test]
fn test_collect_refs_fn_item_params_and_return_detected() {
    let mut registry = TypeRegistry::new();
    register_external_struct(&mut registry, "Request", vec![], vec![]);
    register_external_struct(&mut registry, "Response", vec![], vec![]);

    let items = vec![Item::Fn {
        vis: Visibility::Public,
        attributes: vec![],
        is_async: false,
        name: "handle".to_string(),
        type_params: vec![],
        params: vec![crate::ir::Param {
            name: "req".to_string(),
            ty: Some(RustType::Named {
                name: "Request".to_string(),
                type_args: vec![],
            }),
        }],
        return_type: Some(RustType::Named {
            name: "Response".to_string(),
            type_args: vec![],
        }),
        body: vec![],
    }];

    let refs = collect_undefined_type_references(&items, &[], &[], &registry);
    assert_eq!(
        refs,
        HashSet::from(["Request".to_string(), "Response".to_string()])
    );
}

#[test]
fn test_collect_refs_defined_trait_excluded() {
    let mut registry = TypeRegistry::new();
    register_external_struct(&mut registry, "MyTrait", vec![], vec![]);

    let items = vec![
        Item::Trait {
            vis: Visibility::Public,
            name: "MyTrait".to_string(),
            type_params: vec![],
            supertraits: vec![],
            methods: vec![],
            associated_types: vec![],
        },
        Item::Struct {
            vis: Visibility::Public,
            name: "MyStruct".to_string(),
            type_params: vec![],
            fields: vec![StructField {
                vis: Some(Visibility::Public),
                name: "t".to_string(),
                ty: RustType::Named {
                    name: "MyTrait".to_string(),
                    type_args: vec![],
                },
            }],
        },
    ];

    let refs = collect_undefined_type_references(&items, &[], &[], &registry);
    assert!(refs.is_empty());
}

#[test]
fn test_collect_refs_defined_type_alias_excluded() {
    let mut registry = TypeRegistry::new();
    register_external_struct(&mut registry, "MyAlias", vec![], vec![]);

    let items = vec![
        Item::TypeAlias {
            vis: Visibility::Public,
            name: "MyAlias".to_string(),
            type_params: vec![],
            ty: RustType::String,
        },
        Item::Struct {
            vis: Visibility::Public,
            name: "MyStruct".to_string(),
            type_params: vec![],
            fields: vec![StructField {
                vis: Some(Visibility::Public),
                name: "a".to_string(),
                ty: RustType::Named {
                    name: "MyAlias".to_string(),
                    type_args: vec![],
                },
            }],
        },
    ];

    let refs = collect_undefined_type_references(&items, &[], &[], &registry);
    assert!(refs.is_empty());
}

#[test]
fn test_collect_refs_enum_variant_struct_fields_detected() {
    let mut registry = TypeRegistry::new();
    register_external_struct(&mut registry, "FormData", vec![], vec![]);

    let items = vec![Item::Enum {
        vis: Visibility::Public,
        name: "MyEnum".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![EnumVariant {
            name: "Upload".to_string(),
            value: None,
            data: None,
            fields: vec![StructField {
                vis: Some(Visibility::Public),
                name: "form".to_string(),
                ty: RustType::Named {
                    name: "FormData".to_string(),
                    type_args: vec![],
                },
            }],
        }],
    }];

    let refs = collect_undefined_type_references(&items, &[], &[], &registry);
    assert_eq!(refs, HashSet::from(["FormData".to_string()]));
}

#[test]
fn test_collect_refs_result_type_both_ok_and_err_detected() {
    let mut registry = TypeRegistry::new();
    register_external_struct(&mut registry, "Response", vec![], vec![]);
    register_external_struct(&mut registry, "HttpError", vec![], vec![]);

    let items = vec![Item::Struct {
        vis: Visibility::Public,
        name: "MyStruct".to_string(),
        type_params: vec![],
        fields: vec![StructField {
            vis: Some(Visibility::Public),
            name: "result".to_string(),
            ty: RustType::Result {
                ok: Box::new(RustType::Named {
                    name: "Response".to_string(),
                    type_args: vec![],
                }),
                err: Box::new(RustType::Named {
                    name: "HttpError".to_string(),
                    type_args: vec![],
                }),
            },
        }],
    }];

    let refs = collect_undefined_type_references(&items, &[], &[], &registry);
    assert_eq!(
        refs,
        HashSet::from(["Response".to_string(), "HttpError".to_string()])
    );
}

#[test]
fn test_collect_refs_tuple_type_detected() {
    let mut registry = TypeRegistry::new();
    register_external_struct(&mut registry, "Headers", vec![], vec![]);

    let items = vec![Item::Struct {
        vis: Visibility::Public,
        name: "MyStruct".to_string(),
        type_params: vec![],
        fields: vec![StructField {
            vis: Some(Visibility::Public),
            name: "pair".to_string(),
            ty: RustType::Tuple(vec![
                RustType::String,
                RustType::Named {
                    name: "Headers".to_string(),
                    type_args: vec![],
                },
            ]),
        }],
    }];

    let refs = collect_undefined_type_references(&items, &[], &[], &registry);
    assert_eq!(refs, HashSet::from(["Headers".to_string()]));
}

// =========================================================================
// T2: generate_external_struct
// =========================================================================

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
                RustType::Named {
                    name: "TArrayBuffer".to_string(),
                    type_args: vec![],
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
            RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
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
                RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![],
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
            RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
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
                RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![],
                },
            ),
            (
                "second",
                RustType::Named {
                    name: "U".to_string(),
                    type_args: vec![],
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
                constraint: Some(RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![],
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
                RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![],
                },
            ),
            (
                "count",
                RustType::Named {
                    name: "U".to_string(),
                    type_args: vec![],
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
                RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![],
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
            RustType::Option(Box::new(RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
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

// =========================================================================
// camel_to_snake
// =========================================================================

#[test]
fn test_camel_to_snake_basic() {
    assert_eq!(camel_to_snake("byteLength"), "byte_length");
    assert_eq!(camel_to_snake("lastIndex"), "last_index");
    assert_eq!(camel_to_snake("ignoreCase"), "ignore_case");
}

#[test]
fn test_camel_to_snake_acronym() {
    assert_eq!(camel_to_snake("toISOString"), "to_iso_string");
    assert_eq!(camel_to_snake("bodyUsed"), "body_used");
}

#[test]
fn test_camel_to_snake_already_lowercase() {
    assert_eq!(camel_to_snake("name"), "name");
    assert_eq!(camel_to_snake("source"), "source");
}

#[test]
fn test_camel_to_snake_single_char() {
    assert_eq!(camel_to_snake("x"), "x");
}

#[test]
fn test_camel_to_snake_pascal_case() {
    // PascalCase は先頭を小文字にする
    assert_eq!(camel_to_snake("ByteLength"), "byte_length");
}

#[test]
fn test_camel_to_snake_all_uppercase() {
    assert_eq!(camel_to_snake("URL"), "url");
}

#[test]
fn test_camel_to_snake_consecutive_acronyms() {
    assert_eq!(camel_to_snake("XMLHTTPRequest"), "xmlhttp_request");
}

#[test]
fn test_camel_to_snake_empty() {
    assert_eq!(camel_to_snake(""), "");
}

// =========================================================================
// T5: collect_all_undefined_references / generate_stub_structs
// =========================================================================

#[test]
fn test_collect_all_undefined_refs_includes_non_external() {
    // registry に登録されていない型も検出する（is_external フィルタなし）
    let items = vec![Item::Enum {
        vis: Visibility::Public,
        name: "MyEnum".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![EnumVariant {
            name: "A".to_string(),
            data: Some(RustType::Named {
                name: "UnknownType".to_string(),
                type_args: vec![],
            }),
            fields: vec![],
            value: None,
        }],
    }];
    let refs = collect_all_undefined_references(&items, &[], &[]);
    assert!(refs.contains("UnknownType"));
}

#[test]
fn test_collect_all_undefined_refs_excludes_defined() {
    let items = vec![
        Item::Struct {
            vis: Visibility::Public,
            name: "Foo".to_string(),
            type_params: vec![],
            fields: vec![],
        },
        Item::Enum {
            vis: Visibility::Public,
            name: "MyEnum".to_string(),
            type_params: vec![],
            serde_tag: None,
            variants: vec![EnumVariant {
                name: "A".to_string(),
                data: Some(RustType::Named {
                    name: "Foo".to_string(),
                    type_args: vec![],
                }),
                fields: vec![],
                value: None,
            }],
        },
    ];
    let refs = collect_all_undefined_references(&items, &[], &[]);
    assert!(!refs.contains("Foo"), "defined types should be excluded");
}

#[test]
fn test_generate_stub_structs_creates_empty_stubs() {
    let registry = TypeRegistry::new();
    let mut items = vec![Item::Enum {
        vis: Visibility::Public,
        name: "MyEnum".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![EnumVariant {
            name: "A".to_string(),
            data: Some(RustType::Named {
                name: "MissingType".to_string(),
                type_args: vec![],
            }),
            fields: vec![],
            value: None,
        }],
    }];
    generate_stub_structs(
        &mut items,
        &[],
        &[],
        &registry,
        &SyntheticTypeRegistry::new(),
    );
    let has_stub = items.iter().any(|item| {
        matches!(item, Item::Struct { name, fields, .. } if name == "MissingType" && fields.is_empty())
    });
    assert!(has_stub, "should generate stub for MissingType");
}

// =========================================================================
// T6: collect_type_refs_from_item — Impl/Trait/TypeAlias 強化分（A-2-1）
// =========================================================================

#[test]
fn test_collect_type_refs_from_type_alias() {
    // type Foo = Bar; → Bar が refs に入る
    let item = Item::TypeAlias {
        vis: Visibility::Public,
        name: "Foo".to_string(),
        type_params: vec![],
        ty: RustType::Named {
            name: "Bar".to_string(),
            type_args: vec![],
        },
    };
    let mut refs = std::collections::HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Bar"), "TypeAlias の右辺型が refs に入るべき");
}

#[test]
fn test_collect_type_refs_from_type_alias_nested() {
    // type Foo = Vec<Bar>; → Bar が refs に入る
    let item = Item::TypeAlias {
        vis: Visibility::Public,
        name: "Foo".to_string(),
        type_params: vec![],
        ty: RustType::Vec(Box::new(RustType::Named {
            name: "Bar".to_string(),
            type_args: vec![],
        })),
    };
    let mut refs = std::collections::HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Bar"));
}

#[test]
fn test_collect_type_refs_from_impl_for_trait() {
    // impl<T> MyTrait<U> for Foo { } → MyTrait と U が refs に入る
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![],
        for_trait: Some(TraitRef {
            name: "MyTrait".to_string(),
            type_args: vec![RustType::Named {
                name: "U".to_string(),
                type_args: vec![],
            }],
        }),
        consts: vec![],
        methods: vec![],
    };
    let mut refs = std::collections::HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("MyTrait"), "trait 名が refs に入るべき");
    assert!(refs.contains("U"), "trait の type_args が refs に入るべき");
}

#[test]
fn test_collect_type_refs_from_impl_method_signature() {
    // impl Foo { fn bar(x: Baz) -> Qux { ... } } → Baz と Qux が refs に入る
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![],
        for_trait: None,
        consts: vec![],
        methods: vec![Method {
            vis: Visibility::Public,
            name: "bar".to_string(),
            has_self: true,
            has_mut_self: false,
            params: vec![Param {
                name: "x".to_string(),
                ty: Some(RustType::Named {
                    name: "Baz".to_string(),
                    type_args: vec![],
                }),
            }],
            return_type: Some(RustType::Named {
                name: "Qux".to_string(),
                type_args: vec![],
            }),
            body: Some(vec![]),
        }],
    };
    let mut refs = std::collections::HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(
        refs.contains("Baz"),
        "impl method の param 型が refs に入るべき"
    );
    assert!(
        refs.contains("Qux"),
        "impl method の return 型が refs に入るべき"
    );
}

#[test]
fn test_collect_type_refs_from_trait_method_signature() {
    // trait Foo { fn bar(x: Baz) -> Qux; } → Baz と Qux が refs に入る
    let item = Item::Trait {
        vis: Visibility::Public,
        name: "Foo".to_string(),
        type_params: vec![],
        supertraits: vec![],
        methods: vec![Method {
            vis: Visibility::Public,
            name: "bar".to_string(),
            has_self: true,
            has_mut_self: false,
            params: vec![Param {
                name: "x".to_string(),
                ty: Some(RustType::Named {
                    name: "Baz".to_string(),
                    type_args: vec![],
                }),
            }],
            return_type: Some(RustType::Named {
                name: "Qux".to_string(),
                type_args: vec![],
            }),
            body: None, // trait method signature
        }],
        associated_types: vec![],
    };
    let mut refs = std::collections::HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(
        refs.contains("Baz"),
        "trait method signature の param 型が refs に入るべき"
    );
    assert!(
        refs.contains("Qux"),
        "trait method signature の return 型が refs に入るべき"
    );
}

#[test]
fn test_collect_type_refs_from_impl_consts() {
    // impl Foo { pub const MAX: SomeType = ...; } → SomeType が refs に入る
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![],
        for_trait: None,
        consts: vec![AssocConst {
            vis: Visibility::Public,
            name: "MAX".to_string(),
            ty: RustType::Named {
                name: "SomeType".to_string(),
                type_args: vec![],
            },
            value: Expr::NumberLit(0.0),
        }],
        methods: vec![],
    };
    let mut refs = std::collections::HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(
        refs.contains("SomeType"),
        "impl の AssocConst.ty が refs に入るべき"
    );
}

#[test]
fn test_collect_type_refs_excludes_self() {
    // impl Foo { fn new(...) -> Self { ... } } で `Self` は ref に含まれない。
    // Self を含めると stub generator が `pub struct Self {}` を生成し
    // Rust の予約語衝突でコンパイル不可になるための回避（band-aid）。
    // 根本治療は IR レベルでの型名サニタイズ（I-374 参照）。
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![],
        for_trait: None,
        consts: vec![],
        methods: vec![Method {
            vis: Visibility::Public,
            name: "new".to_string(),
            has_self: false,
            has_mut_self: false,
            params: vec![],
            return_type: Some(RustType::Named {
                name: "Self".to_string(),
                type_args: vec![],
            }),
            body: Some(vec![]),
        }],
    };
    let mut refs = std::collections::HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(
        !refs.contains("Self"),
        "`Self` は ref 収集対象から除外される"
    );
}

#[test]
fn test_collect_type_refs_from_trait_supertraits() {
    // trait Foo: Bar + Baz<T> { } → Bar, Baz, T が refs に入る
    let item = Item::Trait {
        vis: Visibility::Public,
        name: "Foo".to_string(),
        type_params: vec![],
        supertraits: vec![
            TraitRef {
                name: "Bar".to_string(),
                type_args: vec![],
            },
            TraitRef {
                name: "Baz".to_string(),
                type_args: vec![RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![],
                }],
            },
        ],
        methods: vec![],
        associated_types: vec![],
    };
    let mut refs = std::collections::HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Bar"));
    assert!(refs.contains("Baz"));
    assert!(refs.contains("T"));
}

// =========================================================================
// T7: collect_type_refs_from_rust_type — RustType::QSelf 強化
// =========================================================================

fn named(name: &str) -> RustType {
    RustType::Named {
        name: name.to_string(),
        type_args: vec![],
    }
}

#[test]
fn test_collect_type_refs_qself_extracts_qself_and_trait_name() {
    // <T as Promise>::Output → refs に T と Promise が入る（item 名 Output は除外）
    let ty = RustType::QSelf {
        qself: Box::new(named("T")),
        trait_ref: TraitRef {
            name: "Promise".to_string(),
            type_args: vec![],
        },
        item: "Output".to_string(),
    };
    let mut refs = HashSet::new();
    collect_type_refs_from_rust_type(&ty, &mut refs);
    assert!(refs.contains("T"));
    assert!(refs.contains("Promise"));
    assert!(!refs.contains("Output"));
}

#[test]
fn test_collect_type_refs_qself_walks_trait_type_args() {
    // <T as Container<Inner>>::Item → T, Container, Inner が入る
    let ty = RustType::QSelf {
        qself: Box::new(named("T")),
        trait_ref: TraitRef {
            name: "Container".to_string(),
            type_args: vec![named("Inner")],
        },
        item: "Item".to_string(),
    };
    let mut refs = HashSet::new();
    collect_type_refs_from_rust_type(&ty, &mut refs);
    assert!(refs.contains("T"));
    assert!(refs.contains("Container"));
    assert!(refs.contains("Inner"));
    assert!(!refs.contains("Item"));
}

#[test]
fn test_collect_type_refs_qself_walks_qself_inner() {
    // <Vec<Foo> as Promise>::Output → Vec, Foo, Promise が入る
    let ty = RustType::QSelf {
        qself: Box::new(RustType::Vec(Box::new(named("Foo")))),
        trait_ref: TraitRef {
            name: "Promise".to_string(),
            type_args: vec![],
        },
        item: "Output".to_string(),
    };
    let mut refs = HashSet::new();
    collect_type_refs_from_rust_type(&ty, &mut refs);
    assert!(refs.contains("Foo"));
    assert!(refs.contains("Promise"));
}

#[test]
fn test_collect_type_refs_dyn_trait_records_trait_name() {
    // dyn Greeter → refs に Greeter が入る
    let ty = RustType::DynTrait("Greeter".to_string());
    let mut refs = HashSet::new();
    collect_type_refs_from_rust_type(&ty, &mut refs);
    assert!(refs.contains("Greeter"));
}

// =========================================================================
// T8: collect_type_refs_from_item — fn body / impl method body / closure / cast
// =========================================================================

fn fn_with_body(name: &str, body: Vec<Stmt>) -> Item {
    Item::Fn {
        vis: Visibility::Public,
        attributes: vec![],
        is_async: false,
        name: name.to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body,
    }
}

#[test]
fn test_collect_type_refs_fn_body_let_binding_type() {
    // fn f() { let x: Foo = ...; } → Foo が refs
    let item = fn_with_body(
        "f",
        vec![Stmt::Let {
            mutable: false,
            name: "x".to_string(),
            ty: Some(named("Foo")),
            init: None,
        }],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Foo"));
}

#[test]
fn test_collect_type_refs_fn_body_struct_init() {
    // fn f() { Wrapper { x: 1 } } → Wrapper が refs
    let item = fn_with_body(
        "f",
        vec![Stmt::TailExpr(Expr::StructInit {
            name: "Wrapper".to_string(),
            fields: vec![("x".to_string(), Expr::IntLit(1))],
            base: None,
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Wrapper"));
}

#[test]
fn test_collect_type_refs_fn_body_struct_init_self_excluded() {
    // fn f() { Self { x: 1 } } → Self は除外
    let item = fn_with_body(
        "f",
        vec![Stmt::TailExpr(Expr::StructInit {
            name: "Self".to_string(),
            fields: vec![],
            base: None,
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(!refs.contains("Self"));
}

#[test]
fn test_collect_type_refs_fn_body_cast_target() {
    // fn f() { x as Foo } → Foo が refs
    let item = fn_with_body(
        "f",
        vec![Stmt::TailExpr(Expr::Cast {
            expr: Box::new(Expr::Ident("x".to_string())),
            target: named("Foo"),
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Foo"));
}

#[test]
fn test_collect_type_refs_fn_body_fncall_uppercase_extracted() {
    // fn f() { Color::Red(x) } → 先頭が大文字なので Color が refs に入る
    let item = fn_with_body(
        "f",
        vec![Stmt::Expr(Expr::FnCall {
            name: "Color::Red".to_string(),
            args: vec![],
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Color"));
}

#[test]
fn test_collect_type_refs_fn_body_fncall_lowercase_skipped() {
    // fn f() { scopeguard::guard(x) } → 先頭が小文字なので登録しない
    let item = fn_with_body(
        "f",
        vec![Stmt::Expr(Expr::FnCall {
            name: "scopeguard::guard".to_string(),
            args: vec![],
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(!refs.contains("scopeguard"));
    assert!(!refs.contains("guard"));
}

#[test]
fn test_collect_type_refs_fn_body_fncall_walks_args() {
    // fn f() { foo(Bar { x: 1 }) } → 小文字 foo は登録されないが args の Bar は登録される
    let item = fn_with_body(
        "f",
        vec![Stmt::Expr(Expr::FnCall {
            name: "foo".to_string(),
            args: vec![Expr::StructInit {
                name: "Bar".to_string(),
                fields: vec![],
                base: None,
            }],
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Bar"));
}

#[test]
fn test_collect_type_refs_fn_body_closure_param_and_return() {
    // fn f() { |x: Foo| -> Bar { ... } } → Foo, Bar が refs
    let item = fn_with_body(
        "f",
        vec![Stmt::TailExpr(Expr::Closure {
            params: vec![Param {
                name: "x".to_string(),
                ty: Some(named("Foo")),
            }],
            return_type: Some(named("Bar")),
            body: ClosureBody::Expr(Box::new(Expr::Ident("x".to_string()))),
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Foo"));
    assert!(refs.contains("Bar"));
}

#[test]
fn test_collect_type_refs_fn_body_match_arm_body_walked() {
    // fn f() { match x { _ => { let y: Foo = ...; } } } → Foo
    let item = fn_with_body(
        "f",
        vec![Stmt::Match {
            expr: Expr::Ident("x".to_string()),
            arms: vec![MatchArm {
                patterns: vec![],
                guard: None,
                body: vec![Stmt::Let {
                    mutable: false,
                    name: "y".to_string(),
                    ty: Some(named("Foo")),
                    init: None,
                }],
            }],
        }],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Foo"));
}

#[test]
fn test_collect_type_refs_impl_method_body_walked() {
    // impl Foo { fn m(&self) { Bar { x: 1 } } } → Bar が refs
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![],
        for_trait: None,
        consts: vec![],
        methods: vec![Method {
            vis: Visibility::Public,
            name: "m".to_string(),
            has_self: true,
            has_mut_self: false,
            params: vec![],
            return_type: None,
            body: Some(vec![Stmt::TailExpr(Expr::StructInit {
                name: "Bar".to_string(),
                fields: vec![],
                base: None,
            })]),
        }],
    };
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Bar"));
}

#[test]
fn test_collect_type_refs_impl_assoc_const_value_walked() {
    // impl Foo { const X: f64 = SomeFn(); } → SomeFn の返値式から refs を拾う
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![],
        for_trait: None,
        consts: vec![AssocConst {
            vis: Visibility::Public,
            name: "X".to_string(),
            ty: RustType::F64,
            value: Expr::FnCall {
                name: "SomeFn".to_string(),
                args: vec![],
            },
        }],
        methods: vec![],
    };
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("SomeFn"));
}

#[test]
fn test_collect_type_refs_fn_body_binary_op_walks_both_sides() {
    // fn f() { Wrapper{x:1} + Wrapper2{x:1} } — 両辺の StructInit を拾う
    let item = fn_with_body(
        "f",
        vec![Stmt::TailExpr(Expr::BinaryOp {
            left: Box::new(Expr::StructInit {
                name: "Wrapper".to_string(),
                fields: vec![],
                base: None,
            }),
            op: BinOp::Add,
            right: Box::new(Expr::StructInit {
                name: "Wrapper2".to_string(),
                fields: vec![],
                base: None,
            }),
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Wrapper"));
    assert!(refs.contains("Wrapper2"));
}

// =========================================================================
// T8b: collect_type_refs_from_item — type_params constraint walking
// =========================================================================

#[test]
fn test_collect_type_refs_struct_type_param_constraint() {
    // struct S<T: SomeTrait> { f: T } → SomeTrait が refs に入る
    let item = Item::Struct {
        vis: Visibility::Public,
        name: "S".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: Some(named("SomeTrait")),
        }],
        fields: vec![],
    };
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("SomeTrait"));
}

#[test]
fn test_collect_type_refs_fn_type_param_constraint_with_generics() {
    // fn f<T: Container<Inner>>() → Container, Inner が refs に入る
    let item = fn_with_body_and_type_params(
        "f",
        vec![TypeParam {
            name: "T".to_string(),
            constraint: Some(RustType::Named {
                name: "Container".to_string(),
                type_args: vec![named("Inner")],
            }),
        }],
        vec![],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Container"));
    assert!(refs.contains("Inner"));
}

#[test]
fn test_collect_type_refs_impl_type_param_constraint() {
    // impl<T: Bar> Foo<T> { } → Bar が refs
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: Some(named("Bar")),
        }],
        for_trait: None,
        consts: vec![],
        methods: vec![],
    };
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Bar"));
}

#[test]
fn test_collect_type_refs_trait_type_param_constraint() {
    // trait Foo<T: Bar> { } → Bar が refs
    let item = Item::Trait {
        vis: Visibility::Public,
        name: "Foo".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: Some(named("Bar")),
        }],
        supertraits: vec![],
        methods: vec![],
        associated_types: vec![],
    };
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Bar"));
}

#[test]
fn test_collect_type_refs_type_alias_type_param_constraint() {
    // type Foo<T: Bar> = T → Bar が refs
    let item = Item::TypeAlias {
        vis: Visibility::Public,
        name: "Foo".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: Some(named("Bar")),
        }],
        ty: named("T"),
    };
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Bar"));
}

#[test]
fn test_collect_type_refs_enum_type_param_constraint() {
    // enum E<T: Bar> { Variant(T) } → Bar が refs
    let item = Item::Enum {
        vis: Visibility::Public,
        name: "E".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: Some(named("Bar")),
        }],
        serde_tag: None,
        variants: vec![],
    };
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Bar"));
}

fn fn_with_body_and_type_params(name: &str, type_params: Vec<TypeParam>, body: Vec<Stmt>) -> Item {
    Item::Fn {
        vis: Visibility::Public,
        attributes: vec![],
        is_async: false,
        name: name.to_string(),
        type_params,
        params: vec![],
        return_type: None,
        body,
    }
}

// =========================================================================
// T8c: MatchArm pattern walking — EnumVariant.path uppercase extraction
// =========================================================================

#[test]
fn test_collect_type_refs_match_arm_enum_variant_pattern() {
    // match x { Color::Red { .. } => ... } → Color が refs
    let item = fn_with_body(
        "f",
        vec![Stmt::Match {
            expr: Expr::Ident("x".to_string()),
            arms: vec![MatchArm {
                patterns: vec![MatchPattern::EnumVariant {
                    path: "Color::Red".to_string(),
                    bindings: vec![],
                }],
                guard: None,
                body: vec![],
            }],
        }],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Color"));
}

#[test]
fn test_collect_type_refs_match_arm_lowercase_path_skipped() {
    // match x { foo::bar => ... } → 先頭が小文字なのでスキップ
    let item = fn_with_body(
        "f",
        vec![Stmt::Match {
            expr: Expr::Ident("x".to_string()),
            arms: vec![MatchArm {
                patterns: vec![MatchPattern::EnumVariant {
                    path: "foo::bar".to_string(),
                    bindings: vec![],
                }],
                guard: None,
                body: vec![],
            }],
        }],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(!refs.contains("foo"));
}

#[test]
fn test_collect_type_refs_match_arm_literal_walks_expr() {
    // match x { 1 => Wrapper { } => ... } の本体に StructInit が含まれる場合、Wrapper を拾う
    let item = fn_with_body(
        "f",
        vec![Stmt::Match {
            expr: Expr::Ident("x".to_string()),
            arms: vec![MatchArm {
                patterns: vec![MatchPattern::Literal(Expr::IntLit(1))],
                guard: None,
                body: vec![Stmt::TailExpr(Expr::StructInit {
                    name: "Wrapper".to_string(),
                    fields: vec![],
                    base: None,
                })],
            }],
        }],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Wrapper"));
}

#[test]
fn test_collect_type_refs_match_arm_guard_walked() {
    // match x { _ if Wrapper { }.is_valid() => ... } の guard 内 StructInit を拾う
    let item = fn_with_body(
        "f",
        vec![Stmt::Match {
            expr: Expr::Ident("x".to_string()),
            arms: vec![MatchArm {
                patterns: vec![MatchPattern::Wildcard],
                guard: Some(Expr::StructInit {
                    name: "Wrapper".to_string(),
                    fields: vec![],
                    base: None,
                }),
                body: vec![],
            }],
        }],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Wrapper"));
}

// =========================================================================
// T8d: Verbatim pattern walking — Stmt::IfLet / Expr::Matches
// =========================================================================

#[test]
fn test_collect_type_refs_stmt_iflet_pattern() {
    // if let Color::Red = x { ... } → Color が refs
    let item = fn_with_body(
        "f",
        vec![Stmt::IfLet {
            pattern: "Color::Red".to_string(),
            expr: Expr::Ident("x".to_string()),
            then_body: vec![],
            else_body: None,
        }],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Color"));
}

#[test]
fn test_collect_type_refs_stmt_whilelet_pattern() {
    // while let Color::Red(x) = it { ... } → Color が refs
    let item = fn_with_body(
        "f",
        vec![Stmt::WhileLet {
            label: None,
            pattern: "Color::Red(x)".to_string(),
            expr: Expr::Ident("it".to_string()),
            body: vec![],
        }],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Color"));
}

#[test]
fn test_collect_type_refs_expr_matches_pattern() {
    // matches!(x, Color::Red(_)) → Color が refs
    let item = fn_with_body(
        "f",
        vec![Stmt::TailExpr(Expr::Matches {
            expr: Box::new(Expr::Ident("x".to_string())),
            pattern: "Color::Red(_)".to_string(),
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Color"));
}

#[test]
fn test_collect_type_refs_pattern_lowercase_skipped() {
    // if let foo::bar = x { ... } → 先頭が小文字なのでスキップ
    let item = fn_with_body(
        "f",
        vec![Stmt::IfLet {
            pattern: "foo::bar".to_string(),
            expr: Expr::Ident("x".to_string()),
            then_body: vec![],
            else_body: None,
        }],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(!refs.contains("foo"));
}

#[test]
fn test_collect_type_refs_pattern_struct_form() {
    // if let Foo { x } = ... → Foo が refs
    let item = fn_with_body(
        "f",
        vec![Stmt::IfLet {
            pattern: "Foo { x }".to_string(),
            expr: Expr::Ident("y".to_string()),
            then_body: vec![],
            else_body: None,
        }],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Foo"));
}

#[test]
fn test_collect_type_refs_pattern_wildcard_no_extraction() {
    // if let _ = x { ... } → wildcard、何も抽出しない
    let item = fn_with_body(
        "f",
        vec![Stmt::IfLet {
            pattern: "_".to_string(),
            expr: Expr::Ident("x".to_string()),
            then_body: vec![],
            else_body: None,
        }],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.is_empty());
}

// =========================================================================
// T9: UndefinedRefScope behavior — defined_only / imports / type params
// =========================================================================

#[test]
fn test_undefined_refs_imported_types_excluded() {
    // use foo::Imported; struct S { f: Imported } → Imported は filtered out
    let items = vec![
        Item::Use {
            vis: Visibility::Private,
            path: "foo".to_string(),
            names: vec!["Imported".to_string()],
        },
        Item::Struct {
            vis: Visibility::Public,
            name: "S".to_string(),
            type_params: vec![],
            fields: vec![StructField {
                vis: Some(Visibility::Public),
                name: "f".to_string(),
                ty: named("Imported"),
            }],
        },
    ];
    let refs = collect_all_undefined_references(&items, &[], &[]);
    assert!(!refs.contains("Imported"));
}

#[test]
fn test_undefined_refs_type_params_excluded() {
    // struct S<T> { f: T } → T は型パラメータなので除外
    let items = vec![Item::Struct {
        vis: Visibility::Public,
        name: "S".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: None,
        }],
        fields: vec![StructField {
            vis: Some(Visibility::Public),
            name: "f".to_string(),
            ty: named("T"),
        }],
    }];
    let refs = collect_all_undefined_references(&items, &[], &[]);
    assert!(!refs.contains("T"));
}

#[test]
fn test_undefined_refs_defined_only_excluded() {
    // defined_only に Foo がある場合、items 内の Foo 参照は undefined と見なさない
    let items = vec![Item::Struct {
        vis: Visibility::Public,
        name: "Bar".to_string(),
        type_params: vec![],
        fields: vec![StructField {
            vis: Some(Visibility::Public),
            name: "f".to_string(),
            ty: named("Foo"),
        }],
    }];
    let defined_only = vec![Item::Struct {
        vis: Visibility::Public,
        name: "Foo".to_string(),
        type_params: vec![],
        fields: vec![],
    }];
    let refs = collect_all_undefined_references(&items, &[], &defined_only);
    assert!(!refs.contains("Foo"));
}

#[test]
fn test_undefined_refs_path_qualified_excluded() {
    // serde_json::Value が refs にあっても "::" を含むので除外
    let items = vec![Item::Struct {
        vis: Visibility::Public,
        name: "S".to_string(),
        type_params: vec![],
        fields: vec![StructField {
            vis: Some(Visibility::Public),
            name: "f".to_string(),
            ty: named("foo::Bar"),
        }],
    }];
    let refs = collect_all_undefined_references(&items, &[], &[]);
    assert!(!refs.contains("foo::Bar"));
}

#[test]
fn test_undefined_refs_collect_undefined_applies_external_filter() {
    // is_external のみ追加で適用される（registry に登録されている外部型のみ通す）
    let mut registry = TypeRegistry::new();
    register_external_struct(&mut registry, "External", vec![], vec![]);
    let items = vec![Item::Struct {
        vis: Visibility::Public,
        name: "S".to_string(),
        type_params: vec![],
        fields: vec![
            StructField {
                vis: Some(Visibility::Public),
                name: "a".to_string(),
                ty: named("External"),
            },
            StructField {
                vis: Some(Visibility::Public),
                name: "b".to_string(),
                ty: named("NotExternal"),
            },
        ],
    }];
    let refs = collect_undefined_type_references(&items, &[], &[], &registry);
    assert!(refs.contains("External"));
    assert!(!refs.contains("NotExternal"));
}

#[test]
fn test_undefined_refs_some_none_ok_err_excluded_via_builtin_set() {
    // FnCall の Some/None/Ok/Err はそれぞれ Option / Result の variant constructor。
    // walker は uppercase prefix で型名候補として ref に登録するが、最終フィルタの
    // RUST_BUILTIN_TYPES に含まれているため stub 生成対象から除外される。
    //
    // この dual-layer 防御の両方の動作をテストする:
    //  1. walker が `Some` を refs に登録する（lowercase スキップでないこと）
    //  2. UndefinedRefScope が `Some` を builtin として除外する
    let items = vec![fn_with_body(
        "f",
        vec![
            Stmt::Expr(Expr::FnCall {
                name: "Some".to_string(),
                args: vec![],
            }),
            Stmt::Expr(Expr::FnCall {
                name: "None".to_string(),
                args: vec![],
            }),
            Stmt::Expr(Expr::FnCall {
                name: "Ok".to_string(),
                args: vec![],
            }),
            Stmt::Expr(Expr::FnCall {
                name: "Err".to_string(),
                args: vec![],
            }),
        ],
    )];

    // Layer 1: walker は uppercase なので refs に登録する
    let mut walker_refs = HashSet::new();
    collect_type_refs_from_item(&items[0], &mut walker_refs);
    assert!(walker_refs.contains("Some"));
    assert!(walker_refs.contains("None"));
    assert!(walker_refs.contains("Ok"));
    assert!(walker_refs.contains("Err"));

    // Layer 2: UndefinedRefScope が builtin として除外する
    let refs = collect_all_undefined_references(&items, &[], &[]);
    assert!(!refs.contains("Some"));
    assert!(!refs.contains("None"));
    assert!(!refs.contains("Ok"));
    assert!(!refs.contains("Err"));
}
