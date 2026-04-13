use super::*;
use crate::registry::{FieldDef, ParamDef};
use crate::ts_type_info::resolve::typedef::{resolve_field_def, resolve_param_def};

#[test]
fn resolve_keyword_types() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();

    assert_eq!(
        resolve_ts_type(&TsTypeInfo::String, &reg, &mut syn).unwrap(),
        RustType::String
    );
    assert_eq!(
        resolve_ts_type(&TsTypeInfo::Number, &reg, &mut syn).unwrap(),
        RustType::F64
    );
    assert_eq!(
        resolve_ts_type(&TsTypeInfo::Boolean, &reg, &mut syn).unwrap(),
        RustType::Bool
    );
    assert_eq!(
        resolve_ts_type(&TsTypeInfo::Void, &reg, &mut syn).unwrap(),
        RustType::Unit
    );
    assert_eq!(
        resolve_ts_type(&TsTypeInfo::Any, &reg, &mut syn).unwrap(),
        RustType::Any
    );
    assert_eq!(
        resolve_ts_type(&TsTypeInfo::Never, &reg, &mut syn).unwrap(),
        RustType::Never
    );
}

#[test]
fn resolve_array() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let info = TsTypeInfo::Array(Box::new(TsTypeInfo::String));
    assert_eq!(
        resolve_ts_type(&info, &reg, &mut syn).unwrap(),
        RustType::Vec(Box::new(RustType::String))
    );
}

#[test]
fn resolve_nullable_union() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let info = TsTypeInfo::Union(vec![TsTypeInfo::String, TsTypeInfo::Null]);
    assert_eq!(
        resolve_ts_type(&info, &reg, &mut syn).unwrap(),
        RustType::Option(Box::new(RustType::String))
    );
}

#[test]
fn resolve_type_ref_array() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let info = TsTypeInfo::TypeRef {
        name: "Array".to_string(),
        type_args: vec![TsTypeInfo::Number],
    };
    assert_eq!(
        resolve_ts_type(&info, &reg, &mut syn).unwrap(),
        RustType::Vec(Box::new(RustType::F64))
    );
}

#[test]
fn resolve_promise() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let info = TsTypeInfo::TypeRef {
        name: "Promise".to_string(),
        type_args: vec![TsTypeInfo::String],
    };
    let result = resolve_ts_type(&info, &reg, &mut syn).unwrap();
    // Promise<T> は Named("Promise", [T]) のまま返る（unwrap は transformer の責務）
    match result {
        RustType::Named { name, type_args } => {
            assert_eq!(name, "Promise");
            assert_eq!(type_args, vec![RustType::String]);
        }
        _ => panic!("expected Named(Promise)"),
    }
}

#[test]
fn resolve_all_keyword_types() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();

    // Additional keywords not in resolve_keyword_types
    assert_eq!(
        resolve_ts_type(&TsTypeInfo::Unknown, &reg, &mut syn).unwrap(),
        RustType::Any
    );
    assert_eq!(
        resolve_ts_type(&TsTypeInfo::Null, &reg, &mut syn).unwrap(),
        RustType::Unit
    );
    assert_eq!(
        resolve_ts_type(&TsTypeInfo::Undefined, &reg, &mut syn).unwrap(),
        RustType::Unit
    );
    assert_eq!(
        resolve_ts_type(&TsTypeInfo::Object, &reg, &mut syn).unwrap(),
        RustType::Named {
            name: "serde_json::Value".to_string(),
            type_args: vec![]
        }
    );
    assert_eq!(
        resolve_ts_type(&TsTypeInfo::BigInt, &reg, &mut syn).unwrap(),
        RustType::Primitive(PrimitiveIntKind::I128)
    );
}

#[test]
fn resolve_tuple() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let info = TsTypeInfo::Tuple(vec![TsTypeInfo::String, TsTypeInfo::Number]);
    assert_eq!(
        resolve_ts_type(&info, &reg, &mut syn).unwrap(),
        RustType::Tuple(vec![RustType::String, RustType::F64])
    );
}

#[test]
fn resolve_function() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let info = TsTypeInfo::Function {
        params: vec![TsTypeInfo::String],
        return_type: Box::new(TsTypeInfo::Number),
    };
    assert_eq!(
        resolve_ts_type(&info, &reg, &mut syn).unwrap(),
        RustType::Fn {
            params: vec![RustType::String],
            return_type: Box::new(RustType::F64),
        }
    );
}

#[test]
fn resolve_literal_types() {
    use super::super::TsLiteralKind;
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();

    assert_eq!(
        resolve_ts_type(
            &TsTypeInfo::Literal(TsLiteralKind::String("hi".to_string())),
            &reg,
            &mut syn
        )
        .unwrap(),
        RustType::String
    );
    assert_eq!(
        resolve_ts_type(
            &TsTypeInfo::Literal(TsLiteralKind::Number(42.0)),
            &reg,
            &mut syn
        )
        .unwrap(),
        RustType::F64
    );
    assert_eq!(
        resolve_ts_type(
            &TsTypeInfo::Literal(TsLiteralKind::Boolean(true)),
            &reg,
            &mut syn
        )
        .unwrap(),
        RustType::Bool
    );
    assert_eq!(
        resolve_ts_type(
            &TsTypeInfo::Literal(TsLiteralKind::Template),
            &reg,
            &mut syn
        )
        .unwrap(),
        RustType::String
    );
}

#[test]
fn resolve_type_predicate() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    assert_eq!(
        resolve_ts_type(&TsTypeInfo::TypePredicate, &reg, &mut syn).unwrap(),
        RustType::Bool
    );
}

#[test]
fn resolve_readonly_stripped() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let info = TsTypeInfo::Readonly(Box::new(TsTypeInfo::Array(Box::new(TsTypeInfo::String))));
    assert_eq!(
        resolve_ts_type(&info, &reg, &mut syn).unwrap(),
        RustType::Vec(Box::new(RustType::String))
    );
}

#[test]
fn resolve_record_type() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let info = TsTypeInfo::TypeRef {
        name: "Record".to_string(),
        type_args: vec![TsTypeInfo::String, TsTypeInfo::Number],
    };
    assert_eq!(
        resolve_ts_type(&info, &reg, &mut syn).unwrap(),
        RustType::StdCollection {
            kind: StdCollectionKind::HashMap,
            args: vec![RustType::String, RustType::F64],
        }
    );
}

#[test]
fn resolve_set_type() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let info = TsTypeInfo::TypeRef {
        name: "Set".to_string(),
        type_args: vec![TsTypeInfo::String],
    };
    assert_eq!(
        resolve_ts_type(&info, &reg, &mut syn).unwrap(),
        RustType::StdCollection {
            kind: StdCollectionKind::HashSet,
            args: vec![RustType::String],
        }
    );
}

#[test]
fn resolve_user_defined_type() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let info = TsTypeInfo::TypeRef {
        name: "MyStruct".to_string(),
        type_args: vec![],
    };
    assert_eq!(
        resolve_ts_type(&info, &reg, &mut syn).unwrap(),
        RustType::Named {
            name: "MyStruct".to_string(),
            type_args: vec![]
        }
    );
}

#[test]
fn resolve_mapped_type_fallback() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let info = TsTypeInfo::Mapped {
        type_param: "K".to_string(),
        constraint: Box::new(TsTypeInfo::String),
        value: Some(Box::new(TsTypeInfo::Number)),
        has_readonly: false,
        has_optional: false,
        name_type: None,
    };
    assert_eq!(
        resolve_ts_type(&info, &reg, &mut syn).unwrap(),
        RustType::StdCollection {
            kind: StdCollectionKind::HashMap,
            args: vec![RustType::String, RustType::F64],
        }
    );
}

#[test]
fn resolve_nullable_undefined_union() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let info = TsTypeInfo::Union(vec![TsTypeInfo::Number, TsTypeInfo::Undefined]);
    assert_eq!(
        resolve_ts_type(&info, &reg, &mut syn).unwrap(),
        RustType::Option(Box::new(RustType::F64))
    );
}

#[test]
fn resolve_field_optional() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let field = FieldDef {
        name: "x".to_string(),
        ty: TsTypeInfo::String,
        optional: true,
    };
    let resolved = resolve_field_def(field, &reg, &mut syn).unwrap();
    assert_eq!(resolved.ty, RustType::Option(Box::new(RustType::String)));
    assert!(resolved.optional);
}

#[test]
fn resolve_param_with_default() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let param = ParamDef {
        name: "x".to_string(),
        ty: TsTypeInfo::Number,
        optional: false,
        has_default: true,
    };
    let resolved = resolve_param_def(param, &reg, &mut syn).unwrap();
    assert_eq!(resolved.ty, RustType::Option(Box::new(RustType::F64)));
    assert!(resolved.has_default);
}

// ── resolve_type_params ──

#[test]
fn resolve_type_params_empty() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let result = resolve_type_params(vec![], &reg, &mut syn).unwrap();
    assert!(result.is_empty());
}

#[test]
fn resolve_type_params_with_constraint() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let params = vec![crate::ir::TypeParam {
        name: "T".to_string(),
        constraint: Some(TsTypeInfo::String),
        default: None,
    }];
    let resolved = resolve_type_params(params, &reg, &mut syn).unwrap();
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].name, "T");
    assert_eq!(resolved[0].constraint, Some(RustType::String));
}

#[test]
fn resolve_type_params_without_constraint() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let params = vec![crate::ir::TypeParam {
        name: "T".to_string(),
        constraint: None,
        default: None,
    }];
    let resolved = resolve_type_params(params, &reg, &mut syn).unwrap();
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].name, "T");
    assert_eq!(resolved[0].constraint, None);
}

// ── resolve_typedef PascalCase ──

#[test]
fn resolve_typedef_string_literal_union_applies_pascal_case() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let def: TypeDef<TsTypeInfo> = TypeDef::Enum {
        type_params: vec![],
        variants: vec!["up".to_string(), "down".to_string()],
        string_values: [
            ("up".to_string(), "up".to_string()),
            ("down".to_string(), "down".to_string()),
        ]
        .into_iter()
        .collect(),
        tag_field: None,
        variant_fields: std::collections::HashMap::new(),
    };
    let resolved = resolve_typedef(def, &reg, &mut syn).unwrap();
    if let TypeDef::Enum {
        variants,
        string_values,
        ..
    } = resolved
    {
        assert_eq!(variants, vec!["Up".to_string(), "Down".to_string()]);
        assert_eq!(string_values.get("up"), Some(&"Up".to_string()));
        assert_eq!(string_values.get("down"), Some(&"Down".to_string()));
    } else {
        panic!("expected Enum");
    }
}

#[test]
fn resolve_typedef_regular_enum_no_pascal_case() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let def: TypeDef<TsTypeInfo> = TypeDef::Enum {
        type_params: vec![],
        variants: vec!["Red".to_string(), "Green".to_string()],
        string_values: std::collections::HashMap::new(),
        tag_field: None,
        variant_fields: std::collections::HashMap::new(),
    };
    let resolved = resolve_typedef(def, &reg, &mut syn).unwrap();
    if let TypeDef::Enum { variants, .. } = resolved {
        assert_eq!(variants, vec!["Red".to_string(), "Green".to_string()]);
    } else {
        panic!("expected Enum");
    }
}

// ── resolve_type_ref: type arg truncation ──

#[test]
fn resolve_type_ref_truncates_extra_type_args() {
    // Register a type with 0 type_params
    let mut reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    reg.register(
        "Foo".to_string(),
        TypeDef::Struct {
            type_params: vec![], // 0 type params
            fields: vec![],
            methods: std::collections::HashMap::new(),
            constructor: None,
            call_signatures: vec![],
            extends: vec![],
            is_interface: false,
        },
    );
    // Resolve a type ref with extra type args
    let info = TsTypeInfo::TypeRef {
        name: "Foo".to_string(),
        type_args: vec![TsTypeInfo::String, TsTypeInfo::Number],
    };
    let result = resolve_ts_type(&info, &reg, &mut syn).unwrap();
    // Extra args should be truncated to 0
    match result {
        RustType::Named { name, type_args } => {
            assert_eq!(name, "Foo");
            assert!(
                type_args.is_empty(),
                "type args should be truncated to match type_params count (0)"
            );
        }
        _ => panic!("expected Named"),
    }
}

// ── resolve_keyof ──

#[test]
fn resolve_keyof_type_ref_not_in_registry_falls_back_to_string() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    // keyof UnknownType → String フォールバック
    let info = TsTypeInfo::KeyOf(Box::new(TsTypeInfo::TypeRef {
        name: "UnknownType".to_string(),
        type_args: vec![],
    }));
    assert_eq!(
        resolve_ts_type(&info, &reg, &mut syn).unwrap(),
        RustType::String
    );
}

// ── resolve_type_query ──

#[test]
fn resolve_type_query_function_variant() {
    let mut reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    reg.register(
        "myFunc".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![ParamDef {
                name: "x".to_string(),
                ty: RustType::String,
                optional: false,
                has_default: false,
            }],
            return_type: Some(RustType::F64),
            has_rest: false,
        },
    );
    let info = TsTypeInfo::TypeQuery("myFunc".to_string());
    assert_eq!(
        resolve_ts_type(&info, &reg, &mut syn).unwrap(),
        RustType::Fn {
            params: vec![RustType::String],
            return_type: Box::new(RustType::F64),
        }
    );
}

#[test]
fn resolve_type_query_enum_variant() {
    let mut reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    reg.register(
        "Color".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Red".to_string()],
            string_values: std::collections::HashMap::new(),
            tag_field: None,
            variant_fields: std::collections::HashMap::new(),
        },
    );
    let info = TsTypeInfo::TypeQuery("Color".to_string());
    assert_eq!(
        resolve_ts_type(&info, &reg, &mut syn).unwrap(),
        RustType::Named {
            name: "Color".to_string(),
            type_args: vec![],
        }
    );
}

// ── is_symbol_filter_noop (tested indirectly via resolve_ts_type + Mapped) ──

#[test]
fn is_symbol_filter_noop_valid_pattern_allows_identity() {
    // { [K in keyof T as K extends symbol ? never : K]: T[K] } → T
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let info = TsTypeInfo::Mapped {
        type_param: "K".to_string(),
        constraint: Box::new(TsTypeInfo::KeyOf(Box::new(TsTypeInfo::TypeRef {
            name: "Foo".to_string(),
            type_args: vec![],
        }))),
        value: Some(Box::new(TsTypeInfo::IndexedAccess {
            object: Box::new(TsTypeInfo::TypeRef {
                name: "Foo".to_string(),
                type_args: vec![],
            }),
            index: Box::new(TsTypeInfo::TypeRef {
                name: "K".to_string(),
                type_args: vec![],
            }),
        })),
        has_readonly: false,
        has_optional: false,
        name_type: Some(Box::new(TsTypeInfo::Conditional {
            check: Box::new(TsTypeInfo::TypeRef {
                name: "K".to_string(),
                type_args: vec![],
            }),
            extends: Box::new(TsTypeInfo::Symbol),
            true_type: Box::new(TsTypeInfo::Never),
            false_type: Box::new(TsTypeInfo::TypeRef {
                name: "K".to_string(),
                type_args: vec![],
            }),
        })),
    };
    // noop symbol filter → identity 簡約成功 → Named("Foo")
    assert_eq!(
        resolve_ts_type(&info, &reg, &mut syn).unwrap(),
        RustType::Named {
            name: "Foo".to_string(),
            type_args: vec![],
        }
    );
}

#[test]
fn is_symbol_filter_noop_check_type_mismatch_blocks_identity() {
    // check が K ではなく X → noop ではない → identity 簡約されず HashMap フォールバック
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let info = TsTypeInfo::Mapped {
        type_param: "K".to_string(),
        constraint: Box::new(TsTypeInfo::String),
        value: Some(Box::new(TsTypeInfo::Number)),
        has_readonly: false,
        has_optional: false,
        name_type: Some(Box::new(TsTypeInfo::Conditional {
            check: Box::new(TsTypeInfo::TypeRef {
                name: "X".to_string(), // K ではない
                type_args: vec![],
            }),
            extends: Box::new(TsTypeInfo::Symbol),
            true_type: Box::new(TsTypeInfo::Never),
            false_type: Box::new(TsTypeInfo::TypeRef {
                name: "K".to_string(),
                type_args: vec![],
            }),
        })),
    };
    assert_eq!(
        resolve_ts_type(&info, &reg, &mut syn).unwrap(),
        RustType::StdCollection {
            kind: crate::ir::StdCollectionKind::HashMap,
            args: vec![RustType::String, RustType::F64],
        }
    );
}

#[test]
fn is_symbol_filter_noop_extends_not_symbol_blocks_identity() {
    // extends が Symbol ではなく String → noop ではない
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let info = TsTypeInfo::Mapped {
        type_param: "K".to_string(),
        constraint: Box::new(TsTypeInfo::String),
        value: Some(Box::new(TsTypeInfo::Number)),
        has_readonly: false,
        has_optional: false,
        name_type: Some(Box::new(TsTypeInfo::Conditional {
            check: Box::new(TsTypeInfo::TypeRef {
                name: "K".to_string(),
                type_args: vec![],
            }),
            extends: Box::new(TsTypeInfo::String), // Symbol ではない
            true_type: Box::new(TsTypeInfo::Never),
            false_type: Box::new(TsTypeInfo::TypeRef {
                name: "K".to_string(),
                type_args: vec![],
            }),
        })),
    };
    assert_eq!(
        resolve_ts_type(&info, &reg, &mut syn).unwrap(),
        RustType::StdCollection {
            kind: crate::ir::StdCollectionKind::HashMap,
            args: vec![RustType::String, RustType::F64],
        }
    );
}

#[test]
fn is_symbol_filter_noop_true_type_not_never_blocks_identity() {
    // true_type が Never ではなく String → noop ではない
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let info = TsTypeInfo::Mapped {
        type_param: "K".to_string(),
        constraint: Box::new(TsTypeInfo::String),
        value: Some(Box::new(TsTypeInfo::Number)),
        has_readonly: false,
        has_optional: false,
        name_type: Some(Box::new(TsTypeInfo::Conditional {
            check: Box::new(TsTypeInfo::TypeRef {
                name: "K".to_string(),
                type_args: vec![],
            }),
            extends: Box::new(TsTypeInfo::Symbol),
            true_type: Box::new(TsTypeInfo::String), // Never ではない
            false_type: Box::new(TsTypeInfo::TypeRef {
                name: "K".to_string(),
                type_args: vec![],
            }),
        })),
    };
    assert_eq!(
        resolve_ts_type(&info, &reg, &mut syn).unwrap(),
        RustType::StdCollection {
            kind: crate::ir::StdCollectionKind::HashMap,
            args: vec![RustType::String, RustType::F64],
        }
    );
}

// ── resolve_mapped with value None ──

#[test]
fn resolve_mapped_value_none_falls_back_to_hashmap_any() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let info = TsTypeInfo::Mapped {
        type_param: "K".to_string(),
        constraint: Box::new(TsTypeInfo::String),
        value: None,
        has_readonly: false,
        has_optional: false,
        name_type: None,
    };
    assert_eq!(
        resolve_ts_type(&info, &reg, &mut syn).unwrap(),
        RustType::StdCollection {
            kind: crate::ir::StdCollectionKind::HashMap,
            args: vec![RustType::String, RustType::Any],
        }
    );
}

#[test]
fn resolve_typedef_discriminated_union_pascal_case_with_fields() {
    let reg = TypeRegistry::new();
    let mut syn = SyntheticTypeRegistry::new();
    let mut variant_fields = std::collections::HashMap::new();
    variant_fields.insert(
        "circle".to_string(),
        vec![FieldDef {
            name: "radius".to_string(),
            ty: TsTypeInfo::Number,
            optional: false,
        }],
    );
    variant_fields.insert(
        "square".to_string(),
        vec![FieldDef {
            name: "side".to_string(),
            ty: TsTypeInfo::Number,
            optional: false,
        }],
    );
    let def: TypeDef<TsTypeInfo> = TypeDef::Enum {
        type_params: vec![],
        variants: vec!["circle".to_string(), "square".to_string()],
        string_values: [
            ("circle".to_string(), "circle".to_string()),
            ("square".to_string(), "square".to_string()),
        ]
        .into_iter()
        .collect(),
        tag_field: Some("kind".to_string()),
        variant_fields,
    };
    let resolved = resolve_typedef(def, &reg, &mut syn).unwrap();
    if let TypeDef::Enum {
        variants,
        string_values,
        variant_fields,
        tag_field,
        ..
    } = resolved
    {
        assert_eq!(variants, vec!["Circle".to_string(), "Square".to_string()]);
        assert_eq!(string_values.get("circle"), Some(&"Circle".to_string()));
        assert_eq!(tag_field, Some("kind".to_string()));
        // variant_fields keys should be PascalCase
        assert!(variant_fields.contains_key("Circle"));
        assert!(variant_fields.contains_key("Square"));
        assert!(!variant_fields.contains_key("circle"));
        // Field types should be resolved
        let circle_fields = &variant_fields["Circle"];
        assert_eq!(circle_fields.len(), 1);
        assert_eq!(circle_fields[0].name, "radius");
        assert_eq!(circle_fields[0].ty, RustType::F64);
    } else {
        panic!("expected Enum");
    }
}

mod mod_tests_errors;
