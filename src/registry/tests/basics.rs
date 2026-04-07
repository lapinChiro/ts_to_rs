use super::*;

#[test]
fn test_registry_new_is_empty() {
    let reg = TypeRegistry::new();
    assert!(reg.get("Foo").is_none());
}

#[test]
fn test_registry_register_and_get_struct() {
    let mut reg = TypeRegistry::new();
    let point = TypeDef::new_struct(
        vec![
            ("x".to_string(), RustType::F64).into(),
            ("y".to_string(), RustType::F64).into(),
        ],
        HashMap::new(),
        vec![],
    );
    reg.register("Point".to_string(), point.clone());
    let def = reg.get("Point").unwrap();
    assert_eq!(*def, point);
}

#[test]
fn test_registry_register_and_get_enum() {
    let mut reg = TypeRegistry::new();
    reg.register(
        "Color".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Red".to_string(), "Green".to_string(), "Blue".to_string()],
            string_values: HashMap::new(),
            tag_field: None,
            variant_fields: HashMap::new(),
        },
    );
    let def = reg.get("Color").unwrap();
    assert_eq!(
        *def,
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Red".to_string(), "Green".to_string(), "Blue".to_string(),],
            string_values: HashMap::new(),
            tag_field: None,
            variant_fields: HashMap::new(),
        }
    );
}

#[test]
fn test_registry_register_and_get_function() {
    let mut reg = TypeRegistry::new();
    reg.register(
        "draw".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![(
                "p".to_string(),
                RustType::Named {
                    name: "Point".to_string(),
                    type_args: vec![],
                },
            )
                .into()],
            return_type: None,
            has_rest: false,
        },
    );
    let def = reg.get("draw").unwrap();
    match def {
        TypeDef::Function {
            params,
            return_type,
            ..
        } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "p");
            assert!(return_type.is_none());
        }
        _ => panic!("expected Function"),
    }
}

#[test]
fn test_registry_get_nonexistent_returns_none() {
    let reg = TypeRegistry::new();
    assert!(reg.get("NonExistent").is_none());
}

// ── merge ──

#[test]
fn test_registry_merge() {
    let mut reg1 = TypeRegistry::new();
    reg1.register(
        "Point".to_string(),
        TypeDef::new_struct(
            vec![("x".to_string(), RustType::F64).into()],
            HashMap::new(),
            vec![],
        ),
    );

    let mut reg2 = TypeRegistry::new();
    reg2.register(
        "Color".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Red".to_string()],
            string_values: HashMap::new(),
            tag_field: None,
            variant_fields: HashMap::new(),
        },
    );

    reg1.merge(&reg2);
    assert!(reg1.get("Point").is_some());
    assert!(reg1.get("Color").is_some());
}

#[test]
fn test_merge_preserves_builtin_constructor_when_source_has_none() {
    let mut builtin_reg = TypeRegistry::new();
    let ctor = MethodSignature {
        params: vec![
            ("body".to_string(), RustType::String).into(),
            (
                "init".to_string(),
                RustType::Named {
                    name: "ResponseInit".to_string(),
                    type_args: vec![],
                },
            )
                .into(),
        ],
        return_type: None,
        has_rest: false,
        type_params: vec![],
    };
    builtin_reg.register_external(
        "Response".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: HashMap::new(),
            constructor: Some(vec![ctor]),
            call_signatures: vec![],
            extends: vec![],
            is_interface: true,
        },
    );

    let mut source_reg = TypeRegistry::new();
    source_reg.register(
        "Response".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: HashMap::new(),
            constructor: None,
            call_signatures: vec![],
            extends: vec!["ClientResponse".to_string()],
            is_interface: true,
        },
    );

    builtin_reg.merge(&source_reg);

    match builtin_reg.get("Response").unwrap() {
        TypeDef::Struct {
            constructor,
            extends,
            ..
        } => {
            assert!(
                constructor.is_some(),
                "builtin constructor should be preserved after merge"
            );
            let ctors = constructor.as_ref().unwrap();
            assert_eq!(ctors.len(), 1);
            assert_eq!(ctors[0].params.len(), 2);
            assert!(extends.contains(&"ClientResponse".to_string()));
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

#[test]
fn test_merge_source_constructor_overrides_builtin() {
    let mut builtin_reg = TypeRegistry::new();
    builtin_reg.register_external(
        "MyClass".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: HashMap::new(),
            constructor: Some(vec![MethodSignature {
                params: vec![("old".to_string(), RustType::String).into()],
                return_type: None,
                has_rest: false,
                type_params: vec![],
            }]),
            call_signatures: vec![],
            extends: vec![],
            is_interface: false,
        },
    );

    let mut source_reg = TypeRegistry::new();
    source_reg.register(
        "MyClass".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![("x".to_string(), RustType::F64).into()],
            methods: HashMap::new(),
            constructor: Some(vec![MethodSignature {
                params: vec![("new_param".to_string(), RustType::F64).into()],
                return_type: None,
                has_rest: false,
                type_params: vec![],
            }]),
            call_signatures: vec![],
            extends: vec![],
            is_interface: false,
        },
    );

    builtin_reg.merge(&source_reg);

    match builtin_reg.get("MyClass").unwrap() {
        TypeDef::Struct {
            constructor,
            fields,
            ..
        } => {
            let ctors = constructor.as_ref().unwrap();
            assert_eq!(ctors[0].params[0].name, "new_param");
            assert_eq!(fields.len(), 1);
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

// ── merge_with_builtin_preservation: call_signatures ──

#[test]
fn test_merge_preserves_builtin_call_signatures() {
    let mut builtin_reg = TypeRegistry::new();
    builtin_reg.register_external(
        "Handler".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: HashMap::new(),
            constructor: None,
            call_signatures: vec![MethodSignature {
                params: vec![("x".to_string(), RustType::String).into()],
                return_type: Some(RustType::F64),
                has_rest: false,
                type_params: vec![],
            }],
            extends: vec![],
            is_interface: true,
        },
    );

    let mut source_reg = TypeRegistry::new();
    source_reg.register(
        "Handler".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![("name".to_string(), RustType::String).into()],
            methods: HashMap::new(),
            constructor: None,
            call_signatures: vec![],
            extends: vec![],
            is_interface: true,
        },
    );

    builtin_reg.merge(&source_reg);

    let def = builtin_reg.get("Handler").expect("Handler should exist");
    if let TypeDef::Struct {
        call_signatures,
        fields,
        ..
    } = def
    {
        assert_eq!(
            call_signatures.len(),
            1,
            "builtin call_signatures should be preserved"
        );
        assert_eq!(fields.len(), 1, "source fields should be used");
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

#[test]
fn test_merge_source_call_signatures_take_priority() {
    let mut builtin_reg = TypeRegistry::new();
    builtin_reg.register_external(
        "Handler".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: HashMap::new(),
            constructor: None,
            call_signatures: vec![MethodSignature {
                params: vec![],
                return_type: Some(RustType::Bool),
                has_rest: false,
                type_params: vec![],
            }],
            extends: vec![],
            is_interface: true,
        },
    );

    let mut source_reg = TypeRegistry::new();
    source_reg.register(
        "Handler".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: HashMap::new(),
            constructor: None,
            call_signatures: vec![
                MethodSignature {
                    params: vec![("a".to_string(), RustType::String).into()],
                    return_type: Some(RustType::F64),
                    has_rest: false,
                    type_params: vec![],
                },
                MethodSignature {
                    params: vec![
                        ("a".to_string(), RustType::String).into(),
                        ("b".to_string(), RustType::F64).into(),
                    ],
                    return_type: Some(RustType::String),
                    has_rest: false,
                    type_params: vec![],
                },
            ],
            extends: vec![],
            is_interface: true,
        },
    );

    builtin_reg.merge(&source_reg);

    let def = builtin_reg.get("Handler").expect("Handler should exist");
    if let TypeDef::Struct {
        call_signatures, ..
    } = def
    {
        assert_eq!(
            call_signatures.len(),
            2,
            "source call_signatures should take priority"
        );
        assert_eq!(call_signatures[0].return_type, Some(RustType::F64));
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}
