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
            // I-387: `T` in field type is now represented as TypeVar (not Named).
            assert!(
                matches!(&fields[0].ty, RustType::TypeVar { name } if name == "T"),
                "expected TypeVar(T), got {:?}",
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
            // I-387: `T` in field type is now represented as TypeVar (not Named).
            assert!(
                matches!(&fields[0].ty, RustType::TypeVar { name } if name == "T"),
                "expected TypeVar(T), got {:?}",
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
            // I-387: `T` inside enum variant field is TypeVar.
            assert!(
                ok_fields.iter().any(|f| f.name == "value"
                    && matches!(f.ty, RustType::TypeVar { ref name } if name == "T")),
                "expected Ok variant to have field 'value: T' (TypeVar), got {ok_fields:?}"
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

// --- Batch 4c: 構成型 substitute メソッド ---

#[test]
fn test_field_def_substitute_replaces_type_param() {
    let field = FieldDef {
        name: "value".to_string(),
        ty: RustType::Named {
            name: "T".to_string(),
            type_args: vec![],
        },
        optional: true,
    };
    let bindings = HashMap::from([("T".to_string(), RustType::String)]);
    let result = field.substitute(&bindings);
    assert_eq!(result.ty, RustType::String);
    assert_eq!(result.name, "value");
    assert!(result.optional);
}

#[test]
fn test_field_def_substitute_empty_bindings_unchanged() {
    let field = FieldDef {
        name: "x".to_string(),
        ty: RustType::F64,
        optional: false,
    };
    let bindings = HashMap::new();
    let result = field.substitute(&bindings);
    assert_eq!(result, field);
}

#[test]
fn test_param_def_substitute_replaces_type_param() {
    let param = ParamDef {
        name: "input".to_string(),
        ty: RustType::Named {
            name: "T".to_string(),
            type_args: vec![],
        },
        optional: true,
        has_default: true,
    };
    let bindings = HashMap::from([("T".to_string(), RustType::F64)]);
    let result = param.substitute(&bindings);
    assert_eq!(result.ty, RustType::F64);
    assert_eq!(result.name, "input");
    assert!(result.optional);
    assert!(result.has_default);
}

#[test]
fn test_method_signature_substitute_replaces_params_and_return() {
    let sig = MethodSignature {
        params: vec![ParamDef {
            name: "x".to_string(),
            ty: RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            },
            optional: false,
            has_default: false,
        }],
        return_type: Some(RustType::Named {
            name: "T".to_string(),
            type_args: vec![],
        }),
        has_rest: false,
        type_params: vec![],
    };
    let bindings = HashMap::from([("T".to_string(), RustType::String)]);
    let result = sig.substitute(&bindings);
    assert_eq!(result.params[0].ty, RustType::String);
    assert_eq!(result.return_type, Some(RustType::String));
    assert!(!result.has_rest);
}

#[test]
fn test_method_signature_substitute_none_return_type_preserved() {
    let sig = MethodSignature {
        params: vec![ParamDef::new(
            "x".to_string(),
            RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            },
        )],
        return_type: None,
        has_rest: true,
        type_params: vec![],
    };
    let bindings = HashMap::from([("T".to_string(), RustType::F64)]);
    let result = sig.substitute(&bindings);
    assert_eq!(result.params[0].ty, RustType::F64);
    assert_eq!(
        result.return_type, None,
        "None return_type should remain None"
    );
    assert!(result.has_rest, "has_rest should be preserved");
}

#[test]
fn test_const_field_substitute_replaces_type_param() {
    let field = ConstField {
        name: "key".to_string(),
        ty: RustType::Named {
            name: "T".to_string(),
            type_args: vec![],
        },
        string_literal_value: Some("hello".to_string()),
    };
    let bindings = HashMap::from([("T".to_string(), RustType::Bool)]);
    let result = field.substitute(&bindings);
    assert_eq!(result.ty, RustType::Bool);
    assert_eq!(result.name, "key");
    assert_eq!(result.string_literal_value, Some("hello".to_string()));
}

#[test]
fn test_const_element_substitute_replaces_type_param() {
    let elem = ConstElement {
        ty: RustType::Named {
            name: "T".to_string(),
            type_args: vec![],
        },
        string_literal_value: Some("world".to_string()),
    };
    let bindings = HashMap::from([("T".to_string(), RustType::String)]);
    let result = elem.substitute(&bindings);
    assert_eq!(result.ty, RustType::String);
    assert_eq!(result.string_literal_value, Some("world".to_string()));
}

// --- Batch 4c: type_params() 全バリアント ---

#[test]
fn test_type_params_function_returns_params() {
    let func = TypeDef::Function {
        type_params: vec![
            TypeParam {
                name: "T".to_string(),
                constraint: None,
            },
            TypeParam {
                name: "U".to_string(),
                constraint: Some(RustType::String),
            },
        ],
        params: vec![],
        return_type: None,
        has_rest: false,
    };
    let tp = func.type_params();
    assert_eq!(tp.len(), 2);
    assert_eq!(tp[0].name, "T");
    assert_eq!(tp[1].name, "U");
    assert_eq!(tp[1].constraint, Some(RustType::String));
}

#[test]
fn test_type_params_const_value_returns_empty() {
    let cv = TypeDef::ConstValue {
        fields: vec![],
        elements: vec![],
        type_ref_name: None,
    };
    assert!(cv.type_params().is_empty());
}

// --- Batch 4c: substitute_types 全バリアント ---

#[test]
fn test_substitute_types_struct_methods() {
    let struct_def = TypeDef::Struct {
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: None,
        }],
        fields: vec![],
        methods: HashMap::from([(
            "get".to_string(),
            vec![MethodSignature {
                params: vec![ParamDef::new("key".to_string(), RustType::String)],
                return_type: Some(RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![],
                }),
                has_rest: false,
                type_params: vec![],
            }],
        )]),
        constructor: None,
        call_signatures: vec![],
        extends: vec![],
        is_interface: true,
    };
    let bindings = HashMap::from([("T".to_string(), RustType::F64)]);
    let result = struct_def.substitute_types(&bindings);
    match &result {
        TypeDef::Struct { methods, .. } => {
            let get_sigs = methods.get("get").unwrap();
            assert_eq!(
                get_sigs[0].return_type,
                Some(RustType::F64),
                "method return type T should be substituted to f64"
            );
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

#[test]
fn test_substitute_types_struct_constructor() {
    let struct_def = TypeDef::Struct {
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: None,
        }],
        fields: vec![],
        methods: HashMap::new(),
        constructor: Some(vec![MethodSignature {
            params: vec![ParamDef::new(
                "value".to_string(),
                RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![],
                },
            )],
            return_type: None,
            has_rest: false,
            type_params: vec![],
        }]),
        call_signatures: vec![],
        extends: vec![],
        is_interface: false,
    };
    let bindings = HashMap::from([("T".to_string(), RustType::String)]);
    let result = struct_def.substitute_types(&bindings);
    match &result {
        TypeDef::Struct { constructor, .. } => {
            let ctor = constructor.as_ref().unwrap();
            assert_eq!(
                ctor[0].params[0].ty,
                RustType::String,
                "constructor param T should be substituted to String"
            );
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

#[test]
fn test_substitute_types_struct_call_signatures() {
    let struct_def = TypeDef::Struct {
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: None,
        }],
        fields: vec![],
        methods: HashMap::new(),
        constructor: None,
        call_signatures: vec![MethodSignature {
            params: vec![ParamDef::new(
                "input".to_string(),
                RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![],
                },
            )],
            return_type: Some(RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            }),
            has_rest: false,
            type_params: vec![],
        }],
        extends: vec![],
        is_interface: true,
    };
    let bindings = HashMap::from([("T".to_string(), RustType::Bool)]);
    let result = struct_def.substitute_types(&bindings);
    match &result {
        TypeDef::Struct {
            call_signatures, ..
        } => {
            assert_eq!(call_signatures[0].params[0].ty, RustType::Bool);
            assert_eq!(call_signatures[0].return_type, Some(RustType::Bool));
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

#[test]
fn test_substitute_types_function() {
    let func = TypeDef::Function {
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: None,
        }],
        params: vec![
            ParamDef::new(
                "input".to_string(),
                RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![],
                },
            ),
            ParamDef::new("flag".to_string(), RustType::Bool),
        ],
        return_type: Some(RustType::Vec(Box::new(RustType::Named {
            name: "T".to_string(),
            type_args: vec![],
        }))),
        has_rest: false,
    };
    let bindings = HashMap::from([("T".to_string(), RustType::String)]);
    let result = func.substitute_types(&bindings);
    match &result {
        TypeDef::Function {
            params,
            return_type,
            has_rest,
            ..
        } => {
            assert_eq!(
                params[0].ty,
                RustType::String,
                "T param should be substituted"
            );
            assert_eq!(
                params[1].ty,
                RustType::Bool,
                "non-T param should be unchanged"
            );
            assert_eq!(
                *return_type,
                Some(RustType::Vec(Box::new(RustType::String))),
                "return type Vec<T> should become Vec<String>"
            );
            assert!(!has_rest);
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

#[test]
fn test_substitute_types_const_value() {
    let cv = TypeDef::ConstValue {
        fields: vec![ConstField {
            name: "key".to_string(),
            ty: RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            },
            string_literal_value: Some("hello".to_string()),
        }],
        elements: vec![ConstElement {
            ty: RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            },
            string_literal_value: None,
        }],
        type_ref_name: Some("Config".to_string()),
    };
    let bindings = HashMap::from([("T".to_string(), RustType::F64)]);
    let result = cv.substitute_types(&bindings);
    match &result {
        TypeDef::ConstValue {
            fields,
            elements,
            type_ref_name,
        } => {
            assert_eq!(fields[0].ty, RustType::F64, "field T should be substituted");
            assert_eq!(
                fields[0].string_literal_value,
                Some("hello".to_string()),
                "string_literal_value should be preserved"
            );
            assert_eq!(
                elements[0].ty,
                RustType::F64,
                "element T should be substituted"
            );
            assert_eq!(
                type_ref_name.as_deref(),
                Some("Config"),
                "type_ref_name should be preserved"
            );
        }
        other => panic!("expected ConstValue, got {other:?}"),
    }
}

// --- Batch 4c: instantiate 統合テスト ---

#[test]
fn test_instantiate_generic_function_substitutes_params_and_return() {
    let module = parse_typescript("type Transform<T> = (input: T) => T;").unwrap();
    let reg = build_registry(&module);
    let instantiated = reg
        .instantiate("Transform", &[RustType::String])
        .expect("instantiate should succeed");
    match instantiated {
        TypeDef::Function {
            params,
            return_type,
            ..
        } => {
            assert_eq!(
                params[0].ty,
                RustType::String,
                "param T should be substituted to String"
            );
            assert_eq!(
                return_type,
                Some(RustType::String),
                "return type T should be substituted to String"
            );
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

#[test]
fn test_instantiate_not_found_returns_none() {
    let reg = TypeRegistry::new();
    assert!(
        reg.instantiate("NonExistent", &[RustType::String])
            .is_none(),
        "instantiate on unknown name should return None"
    );
}

// --- C-1: TypeParam::substitute ---

#[test]
fn test_type_param_substitute_replaces_constraint() {
    let tp = TypeParam {
        name: "U".to_string(),
        constraint: Some(RustType::Named {
            name: "Container".to_string(),
            type_args: vec![RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            }],
        }),
    };
    let bindings = HashMap::from([("T".to_string(), RustType::String)]);
    let result = tp.substitute(&bindings);
    assert_eq!(result.name, "U", "name should not be changed");
    assert_eq!(
        result.constraint,
        Some(RustType::Named {
            name: "Container".to_string(),
            type_args: vec![RustType::String],
        }),
        "constraint Container<T> should become Container<String>"
    );
}

#[test]
fn test_type_param_substitute_none_constraint_unchanged() {
    let tp = TypeParam {
        name: "T".to_string(),
        constraint: None,
    };
    let bindings = HashMap::from([("T".to_string(), RustType::String)]);
    let result = tp.substitute(&bindings);
    assert_eq!(result.name, "T");
    assert_eq!(
        result.constraint, None,
        "None constraint should remain None"
    );
}

#[test]
fn test_substitute_types_function_substitutes_type_param_constraints() {
    let func = TypeDef::Function {
        type_params: vec![
            TypeParam {
                name: "T".to_string(),
                constraint: None,
            },
            TypeParam {
                name: "U".to_string(),
                constraint: Some(RustType::Named {
                    name: "Container".to_string(),
                    type_args: vec![RustType::Named {
                        name: "T".to_string(),
                        type_args: vec![],
                    }],
                }),
            },
        ],
        params: vec![ParamDef::new(
            "x".to_string(),
            RustType::Named {
                name: "U".to_string(),
                type_args: vec![],
            },
        )],
        return_type: Some(RustType::Named {
            name: "T".to_string(),
            type_args: vec![],
        }),
        has_rest: false,
    };
    let bindings = HashMap::from([
        ("T".to_string(), RustType::String),
        (
            "U".to_string(),
            RustType::Named {
                name: "Container".to_string(),
                type_args: vec![RustType::String],
            },
        ),
    ]);
    let result = func.substitute_types(&bindings);
    match &result {
        TypeDef::Function { type_params, .. } => {
            assert_eq!(
                type_params[1].constraint,
                Some(RustType::Named {
                    name: "Container".to_string(),
                    type_args: vec![RustType::String],
                }),
                "type_param U's constraint Container<T> should become Container<String>"
            );
        }
        other => panic!("expected Function, got {other:?}"),
    }
}
