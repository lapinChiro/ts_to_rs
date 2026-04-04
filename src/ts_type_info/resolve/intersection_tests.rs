use super::*;
use crate::ir::{Item, RustType, StructField, Visibility};
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{TypeDef, TypeRegistry};
use crate::ts_type_info::{TsFieldInfo, TsLiteralKind, TsTypeInfo, TsTypeLiteralInfo};
use std::collections::HashMap;

#[test]
fn type_literal_to_inline_struct() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let lit = TsTypeLiteralInfo {
        fields: vec![TsFieldInfo {
            name: "x".to_string(),
            ty: TsTypeInfo::String,
            optional: false,
        }],
        methods: vec![],
        call_signatures: vec![],
        construct_signatures: vec![],
        index_signatures: vec![],
    };
    let result = resolve_type_literal(&lit, &reg, &mut syn).unwrap();
    match result {
        RustType::Named { name, .. } => assert!(!name.is_empty()),
        _ => panic!("expected Named"),
    }
}

#[test]
fn index_signature_to_hashmap() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let lit = TsTypeLiteralInfo {
        fields: vec![],
        methods: vec![],
        call_signatures: vec![],
        construct_signatures: vec![],
        index_signatures: vec![crate::ts_type_info::TsIndexSigInfo {
            param_name: "key".to_string(),
            param_type: TsTypeInfo::String,
            value_type: TsTypeInfo::Number,
            readonly: false,
        }],
    };
    let result = resolve_type_literal(&lit, &reg, &mut syn).unwrap();
    assert_eq!(
        result,
        RustType::Named {
            name: "HashMap".to_string(),
            type_args: vec![RustType::String, RustType::F64],
        }
    );
}

#[test]
fn intersection_merges_fields() {
    let mut reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();

    reg.register(
        "A".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![crate::registry::FieldDef {
                name: "x".to_string(),
                ty: RustType::String,
                optional: false,
            }],
            methods: HashMap::new(),
            constructor: None,
            call_signatures: vec![],
            extends: vec![],
            is_interface: false,
        },
    );

    let members = vec![
        TsTypeInfo::TypeRef {
            name: "A".to_string(),
            type_args: vec![],
        },
        TsTypeInfo::TypeLiteral(TsTypeLiteralInfo {
            fields: vec![TsFieldInfo {
                name: "y".to_string(),
                ty: TsTypeInfo::Number,
                optional: false,
            }],
            methods: vec![],
            call_signatures: vec![],
            construct_signatures: vec![],
            index_signatures: vec![],
        }),
    ];

    let result = resolve_intersection(&members, &reg, &mut syn).unwrap();
    match result {
        RustType::Named { name, .. } => assert!(
            name.starts_with("_TypeLit"),
            "intersection struct should use _TypeLit naming via struct_dedup, got {name}"
        ),
        _ => panic!("expected Named for intersection struct"),
    }
}

#[test]
fn identity_mapped_type_simplified() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let members = vec![TsTypeInfo::Mapped {
        type_param: "K".to_string(),
        constraint: Box::new(TsTypeInfo::KeyOf(Box::new(TsTypeInfo::TypeRef {
            name: "T".to_string(),
            type_args: vec![],
        }))),
        value: Some(Box::new(TsTypeInfo::IndexedAccess {
            object: Box::new(TsTypeInfo::TypeRef {
                name: "T".to_string(),
                type_args: vec![],
            }),
            index: Box::new(TsTypeInfo::TypeRef {
                name: "K".to_string(),
                type_args: vec![],
            }),
        })),
        has_readonly: false,
        has_optional: false,
        name_type: None,
    }];

    let result = resolve_intersection(&members, &reg, &mut syn).unwrap();
    assert_eq!(
        result,
        RustType::Named {
            name: "T".to_string(),
            type_args: vec![]
        }
    );
}

// --- 以下、テストカバレッジ向上のため追加 ---

fn empty_type_literal() -> TsTypeLiteralInfo {
    TsTypeLiteralInfo {
        fields: vec![],
        methods: vec![],
        call_signatures: vec![],
        construct_signatures: vec![],
        index_signatures: vec![],
    }
}

fn type_literal(fields: Vec<TsFieldInfo>) -> TsTypeLiteralInfo {
    TsTypeLiteralInfo {
        fields,
        methods: vec![],
        call_signatures: vec![],
        construct_signatures: vec![],
        index_signatures: vec![],
    }
}

fn field(name: &str, ty: TsTypeInfo) -> TsFieldInfo {
    TsFieldInfo {
        name: name.to_string(),
        ty,
        optional: false,
    }
}

fn string_lit(s: &str) -> TsTypeInfo {
    TsTypeInfo::Literal(TsLiteralKind::String(s.to_string()))
}

#[test]
fn empty_type_literals_filtered_from_intersection() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    // {} & {} → 空メンバー → 空 struct
    let members = vec![
        TsTypeInfo::TypeLiteral(empty_type_literal()),
        TsTypeInfo::TypeLiteral(empty_type_literal()),
    ];
    let result = resolve_intersection(&members, &reg, &mut syn).unwrap();
    match result {
        RustType::Named { name, .. } => assert!(
            name.starts_with("_TypeLit"),
            "empty intersection should use _TypeLit naming via struct_dedup, got {name}"
        ),
        other => panic!("expected Named, got {other:?}"),
    }
}

#[test]
fn identity_mapped_with_readonly_not_simplified() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let members = vec![TsTypeInfo::Mapped {
        type_param: "K".to_string(),
        constraint: Box::new(TsTypeInfo::KeyOf(Box::new(TsTypeInfo::TypeRef {
            name: "T".to_string(),
            type_args: vec![],
        }))),
        value: Some(Box::new(TsTypeInfo::IndexedAccess {
            object: Box::new(TsTypeInfo::TypeRef {
                name: "T".to_string(),
                type_args: vec![],
            }),
            index: Box::new(TsTypeInfo::TypeRef {
                name: "K".to_string(),
                type_args: vec![],
            }),
        })),
        has_readonly: true,
        has_optional: false,
        name_type: None,
    }];
    // readonly 修飾子がある場合、簡約されない → HashMap フォールバック
    let result = resolve_intersection(&members, &reg, &mut syn).unwrap();
    assert_ne!(
        result,
        RustType::Named {
            name: "T".to_string(),
            type_args: vec![]
        }
    );
}

#[test]
fn identity_mapped_with_optional_not_simplified() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let members = vec![TsTypeInfo::Mapped {
        type_param: "K".to_string(),
        constraint: Box::new(TsTypeInfo::KeyOf(Box::new(TsTypeInfo::TypeRef {
            name: "T".to_string(),
            type_args: vec![],
        }))),
        value: Some(Box::new(TsTypeInfo::IndexedAccess {
            object: Box::new(TsTypeInfo::TypeRef {
                name: "T".to_string(),
                type_args: vec![],
            }),
            index: Box::new(TsTypeInfo::TypeRef {
                name: "K".to_string(),
                type_args: vec![],
            }),
        })),
        has_readonly: false,
        has_optional: true,
        name_type: None,
    }];
    let result = resolve_intersection(&members, &reg, &mut syn).unwrap();
    assert_ne!(
        result,
        RustType::Named {
            name: "T".to_string(),
            type_args: vec![]
        }
    );
}

#[test]
fn identity_mapped_value_mismatch_not_simplified() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    // value が T[K] でなく string → 簡約されない
    let members = vec![TsTypeInfo::Mapped {
        type_param: "K".to_string(),
        constraint: Box::new(TsTypeInfo::KeyOf(Box::new(TsTypeInfo::TypeRef {
            name: "T".to_string(),
            type_args: vec![],
        }))),
        value: Some(Box::new(TsTypeInfo::String)),
        has_readonly: false,
        has_optional: false,
        name_type: None,
    }];
    let result = resolve_intersection(&members, &reg, &mut syn).unwrap();
    // HashMap フォールバック
    match result {
        RustType::Named { name, .. } => assert_eq!(name, "HashMap"),
        other => panic!("expected HashMap, got {other:?}"),
    }
}

#[test]
fn identity_mapped_constraint_not_keyof_not_simplified() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    // constraint が keyof T でなく string → 簡約されない
    let members = vec![TsTypeInfo::Mapped {
        type_param: "K".to_string(),
        constraint: Box::new(TsTypeInfo::String),
        value: Some(Box::new(TsTypeInfo::Number)),
        has_readonly: false,
        has_optional: false,
        name_type: None,
    }];
    let result = resolve_intersection(&members, &reg, &mut syn).unwrap();
    match result {
        RustType::Named { name, .. } => assert_eq!(name, "HashMap"),
        other => panic!("expected HashMap, got {other:?}"),
    }
}

#[test]
fn merge_fields_duplicate_error() {
    let mut base = vec![StructField {
        name: "x".to_string(),
        ty: RustType::String,
        vis: Some(Visibility::Public),
    }];
    let new = vec![StructField {
        name: "x".to_string(),
        ty: RustType::F64,
        vis: Some(Visibility::Public),
    }];
    let err = merge_fields_into(&mut base, new).unwrap_err();
    assert!(err.to_string().contains("duplicate field 'x'"));
}

#[test]
fn discriminated_union_with_intersection() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();

    // { base: string } & ({ kind: "click", x: number } | { kind: "hover", y: number })
    let members = vec![
        TsTypeInfo::TypeLiteral(type_literal(vec![field("base", TsTypeInfo::String)])),
        TsTypeInfo::Union(vec![
            TsTypeInfo::TypeLiteral(type_literal(vec![
                field("kind", string_lit("click")),
                field("x", TsTypeInfo::Number),
            ])),
            TsTypeInfo::TypeLiteral(type_literal(vec![
                field("kind", string_lit("hover")),
                field("y", TsTypeInfo::Number),
            ])),
        ]),
    ];

    let result = resolve_intersection(&members, &reg, &mut syn).unwrap();
    match &result {
        RustType::Named { name, .. } => assert!(name.contains("Intersection")),
        other => panic!("expected Named, got {other:?}"),
    }

    // synthetic に登録された enum を確認
    let items = syn.all_items();
    let enum_found = items.iter().any(|item| {
        if let Item::Enum {
            serde_tag,
            variants,
            ..
        } = item
        {
            assert_eq!(serde_tag.as_deref(), Some("kind"));
            assert_eq!(variants.len(), 2);
            let names: Vec<&str> = variants.iter().map(|v| v.name.as_str()).collect();
            assert!(
                names.contains(&"Click"),
                "expected Click variant, got {names:?}"
            );
            assert!(
                names.contains(&"Hover"),
                "expected Hover variant, got {names:?}"
            );
            for v in variants {
                let field_names: Vec<&str> = v.fields.iter().map(|f| f.name.as_str()).collect();
                assert!(
                    field_names.contains(&"base"),
                    "variant {} should have base field, got {field_names:?}",
                    v.name
                );
            }
            true
        } else {
            false
        }
    });
    assert!(enum_found, "discriminated union enum should be registered");
}

#[test]
fn non_discriminated_union_with_intersection() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();

    // { base: string } & ({ x: number } | { y: string })
    // discriminant なし → Variant0, Variant1
    let members = vec![
        TsTypeInfo::TypeLiteral(type_literal(vec![field("base", TsTypeInfo::String)])),
        TsTypeInfo::Union(vec![
            TsTypeInfo::TypeLiteral(type_literal(vec![field("x", TsTypeInfo::Number)])),
            TsTypeInfo::TypeLiteral(type_literal(vec![field("y", TsTypeInfo::String)])),
        ]),
    ];

    let result = resolve_intersection(&members, &reg, &mut syn).unwrap();
    assert!(matches!(result, RustType::Named { .. }));

    let items = syn.all_items();
    let enum_found = items.iter().any(|item| {
        if let Item::Enum {
            serde_tag,
            variants,
            ..
        } = item
        {
            assert_eq!(*serde_tag, None, "no discriminant → no serde_tag");
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name, "Variant0");
            assert_eq!(variants[1].name, "Variant1");
            true
        } else {
            false
        }
    });
    assert!(enum_found);
}

#[test]
fn find_discriminant_duplicate_values_returns_none() {
    // { kind: "a" } | { kind: "a" } → 重複 → None
    let variants = vec![
        TsTypeInfo::TypeLiteral(type_literal(vec![field("kind", string_lit("a"))])),
        TsTypeInfo::TypeLiteral(type_literal(vec![field("kind", string_lit("a"))])),
    ];
    assert_eq!(find_discriminant_field(&variants), None);
}

#[test]
fn find_discriminant_no_common_field_returns_none() {
    let variants = vec![
        TsTypeInfo::TypeLiteral(type_literal(vec![field("x", TsTypeInfo::Number)])),
        TsTypeInfo::TypeLiteral(type_literal(vec![field("y", TsTypeInfo::String)])),
    ];
    assert_eq!(find_discriminant_field(&variants), None);
}

#[test]
fn find_discriminant_valid() {
    let variants = vec![
        TsTypeInfo::TypeLiteral(type_literal(vec![
            field("type", string_lit("text")),
            field("content", TsTypeInfo::String),
        ])),
        TsTypeInfo::TypeLiteral(type_literal(vec![
            field("type", string_lit("image")),
            field("url", TsTypeInfo::String),
        ])),
        TsTypeInfo::TypeLiteral(type_literal(vec![
            field("type", string_lit("video")),
            field("src", TsTypeInfo::String),
        ])),
    ];
    assert_eq!(find_discriminant_field(&variants), Some("type".to_string()));
}

#[test]
fn find_discriminant_non_type_literal_returns_none() {
    // TypeLiteral 以外が含まれる → None
    let variants = vec![
        TsTypeInfo::TypeLiteral(type_literal(vec![field("kind", string_lit("a"))])),
        TsTypeInfo::String,
    ];
    assert_eq!(find_discriminant_field(&variants), None);
}

#[test]
fn unresolvable_typeref_becomes_embed_field() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();

    // A & { y: number } where A is not in registry
    let members = vec![
        TsTypeInfo::TypeRef {
            name: "Unknown".to_string(),
            type_args: vec![],
        },
        TsTypeInfo::TypeLiteral(type_literal(vec![field("y", TsTypeInfo::Number)])),
    ];

    let result = resolve_intersection(&members, &reg, &mut syn).unwrap();
    assert!(matches!(result, RustType::Named { .. }));
}

#[test]
fn extract_variant_fields_type_literal() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let lit = TsTypeInfo::TypeLiteral(type_literal(vec![
        field("a", TsTypeInfo::String),
        field("b", TsTypeInfo::Number),
    ]));
    let fields = extract_variant_fields(&lit, &reg, &mut syn).unwrap();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name, "a");
    assert_eq!(fields[1].name, "b");
}

#[test]
fn extract_variant_fields_typeref_in_registry() {
    let mut reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    reg.register(
        "Point".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![
                crate::registry::FieldDef {
                    name: "x".to_string(),
                    ty: RustType::F64,
                    optional: false,
                },
                crate::registry::FieldDef {
                    name: "y".to_string(),
                    ty: RustType::F64,
                    optional: false,
                },
            ],
            methods: HashMap::new(),
            constructor: None,
            call_signatures: vec![],
            extends: vec![],
            is_interface: false,
        },
    );
    let ty = TsTypeInfo::TypeRef {
        name: "Point".to_string(),
        type_args: vec![],
    };
    let fields = extract_variant_fields(&ty, &reg, &mut syn).unwrap();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name, "x");
}

#[test]
fn extract_variant_fields_unknown_type() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let ty = TsTypeInfo::String;
    let fields = extract_variant_fields(&ty, &reg, &mut syn).unwrap();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "_data");
}

#[test]
fn resolve_method_info_basic() {
    use crate::ts_type_info::TsParamInfo;
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let method = crate::ts_type_info::TsMethodInfo {
        name: "greet".to_string(),
        params: vec![TsParamInfo {
            name: "name".to_string(),
            ty: TsTypeInfo::String,
            optional: false,
        }],
        return_type: Some(TsTypeInfo::String),
        type_params: vec![],
        optional: false,
    };
    let result = resolve_method_info(&method, &reg, &mut syn).unwrap();
    assert_eq!(result.name, "greet");
    assert_eq!(result.params.len(), 1);
    assert_eq!(result.params[0].name, "name");
    assert_eq!(result.params[0].ty, Some(RustType::String));
    assert_eq!(result.return_type, Some(RustType::String));
    assert!(result.has_self);
    assert!(!result.has_mut_self);
}

#[test]
fn resolve_method_info_no_return_type() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let method = crate::ts_type_info::TsMethodInfo {
        name: "doSomething".to_string(),
        params: vec![],
        return_type: None,
        type_params: vec![],
        optional: false,
    };
    let result = resolve_method_info(&method, &reg, &mut syn).unwrap();
    assert_eq!(result.name, "doSomething");
    assert!(result.params.is_empty());
    assert_eq!(result.return_type, None);
}

#[test]
fn extract_discriminated_variant_returns_raw_value_and_pascal_name() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let variant = TsTypeInfo::TypeLiteral(type_literal(vec![
        field("kind", string_lit("my-event")),
        field("data", TsTypeInfo::String),
    ]));
    let (raw, pascal, fields) =
        extract_discriminated_variant(&variant, "kind", &reg, &mut syn).unwrap();
    assert_eq!(raw, "my-event");
    assert_eq!(pascal, "MyEvent");
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "data");
}

#[test]
fn discriminated_union_intersection_sets_enum_value() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();

    // { base: string } & ({ kind: "click" } | { kind: "hover" })
    let members = vec![
        TsTypeInfo::TypeLiteral(type_literal(vec![field("base", TsTypeInfo::String)])),
        TsTypeInfo::Union(vec![
            TsTypeInfo::TypeLiteral(type_literal(vec![field("kind", string_lit("click"))])),
            TsTypeInfo::TypeLiteral(type_literal(vec![field("kind", string_lit("hover"))])),
        ]),
    ];

    resolve_intersection(&members, &reg, &mut syn).unwrap();

    // synthetic enum の EnumVariant::value が Some(Str(raw)) であることを確認
    let items = syn.all_items();
    let enum_item = items
        .iter()
        .find(|item| matches!(item, Item::Enum { .. }))
        .expect("should have enum");
    if let Item::Enum { variants, .. } = enum_item {
        for v in variants {
            assert!(v.value.is_some(), "variant {} should have value", v.name);
            match v.value.as_ref().unwrap() {
                crate::ir::EnumValue::Str(s) => {
                    assert!(s == "click" || s == "hover", "unexpected raw value: {s}");
                }
                other => panic!("expected Str, got {other:?}"),
            }
        }
    }
}

#[test]
fn cross_origin_dedup_preserves_methods() {
    // TypeLit 先行登録 → 同一フィールド intersection（メソッド付き）→ メソッドが消失しないこと
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();

    // 1. TypeLit { x: number } を先に登録
    let lit = TsTypeLiteralInfo {
        fields: vec![field("x", TsTypeInfo::Number)],
        methods: vec![],
        call_signatures: vec![],
        construct_signatures: vec![],
        index_signatures: vec![],
    };
    let _ = resolve_type_literal(&lit, &reg, &mut syn).unwrap();

    // 2. { x: number, greet(name: string): string } を intersection として登録
    let method_lit = TsTypeLiteralInfo {
        fields: vec![field("x", TsTypeInfo::Number)],
        methods: vec![crate::ts_type_info::TsMethodInfo {
            name: "greet".to_string(),
            params: vec![crate::ts_type_info::TsParamInfo {
                name: "name".to_string(),
                ty: TsTypeInfo::String,
                optional: false,
            }],
            return_type: Some(TsTypeInfo::String),
            type_params: vec![],
            optional: false,
        }],
        call_signatures: vec![],
        construct_signatures: vec![],
        index_signatures: vec![],
    };
    let members = vec![TsTypeInfo::TypeLiteral(method_lit)];
    let result = resolve_intersection(&members, &reg, &mut syn).unwrap();

    // single member → resolve_ts_type に委譲されるため、直接 TypeLit 解決になる。
    // 2 メンバーの intersection でテストし直す。
    assert!(matches!(result, RustType::Named { .. }));

    // 2 メンバー版: { x: number } & { y: string } でメソッドは TsTypeLiteral のメソッドから取得
    let mut syn2 = SyntheticTypeRegistry::new();

    // TypeLit { x: number, y: string } を先に登録（メソッドなし）
    let _ = syn2.register_inline_struct(&[
        ("x".to_string(), RustType::F64),
        ("y".to_string(), RustType::String),
    ]);

    // { x: number, greet(): void } & { y: string } を intersection として登録
    let method_lit2 = TsTypeLiteralInfo {
        fields: vec![field("x", TsTypeInfo::Number)],
        methods: vec![crate::ts_type_info::TsMethodInfo {
            name: "greet".to_string(),
            params: vec![],
            return_type: None,
            type_params: vec![],
            optional: false,
        }],
        call_signatures: vec![],
        construct_signatures: vec![],
        index_signatures: vec![],
    };
    let members2 = vec![
        TsTypeInfo::TypeLiteral(method_lit2),
        TsTypeInfo::TypeLiteral(type_literal(vec![field("y", TsTypeInfo::String)])),
    ];
    let result2 = resolve_intersection(&members2, &reg, &mut syn2).unwrap();
    assert!(matches!(result2, RustType::Named { .. }));

    // impl ブロックが登録されていること（dedup ヒット後でもメソッドが消えない）
    let has_impl = syn2
        .all_items()
        .iter()
        .any(|item| matches!(item, Item::Impl { .. }));
    assert!(
        has_impl,
        "impl block should be registered even after cross-origin struct dedup"
    );
}
