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
            ("x".to_string(), RustType::F64),
            ("y".to_string(), RustType::F64),
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
            params: vec![(
                "p".to_string(),
                RustType::Named {
                    name: "Point".to_string(),
                    type_args: vec![],
                },
            )],
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
            assert_eq!(params[0].0, "p");
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

#[test]
fn test_registry_merge() {
    let mut reg1 = TypeRegistry::new();
    reg1.register(
        "Point".to_string(),
        TypeDef::new_struct(
            vec![("x".to_string(), RustType::F64)],
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
    // Builtin Response with constructor should not be overwritten by source-defined
    // empty Response interface (e.g., `export interface Response extends ClientResponse {}`)
    let mut builtin_reg = TypeRegistry::new();
    let ctor = MethodSignature {
        params: vec![
            ("body".to_string(), RustType::String),
            (
                "init".to_string(),
                RustType::Named {
                    name: "ResponseInit".to_string(),
                    type_args: vec![],
                },
            ),
        ],
        return_type: None,
        has_rest: false,
    };
    builtin_reg.register_external(
        "Response".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: HashMap::new(),
            constructor: Some(vec![ctor]),
            extends: vec![],
            is_interface: true,
        },
    );

    // Source-defined Response without constructor (mimics Hono's client/types.ts)
    let mut source_reg = TypeRegistry::new();
    source_reg.register(
        "Response".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: HashMap::new(),
            constructor: None,
            extends: vec!["ClientResponse".to_string()],
            is_interface: true,
        },
    );

    builtin_reg.merge(&source_reg);

    // Constructor should be preserved from builtin
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
            // Source extends should be adopted
            assert!(extends.contains(&"ClientResponse".to_string()));
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

#[test]
fn test_merge_source_constructor_overrides_builtin() {
    // When source defines its OWN constructor, it should take priority
    let mut builtin_reg = TypeRegistry::new();
    builtin_reg.register_external(
        "MyClass".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: HashMap::new(),
            constructor: Some(vec![MethodSignature {
                params: vec![("old".to_string(), RustType::String)],
                return_type: None,
                has_rest: false,
            }]),
            extends: vec![],
            is_interface: false,
        },
    );

    let mut source_reg = TypeRegistry::new();
    source_reg.register(
        "MyClass".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![("x".to_string(), RustType::F64)],
            methods: HashMap::new(),
            constructor: Some(vec![MethodSignature {
                params: vec![("new_param".to_string(), RustType::F64)],
                return_type: None,
                has_rest: false,
            }]),
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
            // Source constructor should take priority
            let ctors = constructor.as_ref().unwrap();
            assert_eq!(ctors[0].params[0].0, "new_param");
            // Source fields should be adopted
            assert_eq!(fields.len(), 1);
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

// -- build_registry tests --

#[test]
fn test_build_registry_interface() {
    let module = parse_typescript("interface Point { x: number; y: number; }").unwrap();
    let reg = build_registry(&module);
    assert_eq!(
        reg.get("Point").unwrap(),
        &TypeDef::new_interface(
            vec![],
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
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
                ("name".to_string(), RustType::String),
                ("count".to_string(), RustType::F64),
            ],
            HashMap::new(),
            vec![],
        )
    );
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
            vec![(
                "name".to_string(),
                RustType::Option(Box::new(RustType::String)),
            )],
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
            assert_eq!(fields[0].0, "b");
            assert!(matches!(&fields[0].1, RustType::Named { name, .. } if name == "B"));
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
                    .any(|(n, t)| n == "name" && *t == RustType::String),
                "expected name: String"
            );
            assert!(
                fields
                    .iter()
                    .any(|(n, t)| n == "age" && *t == RustType::F64),
                "expected age: f64"
            );
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

// --- is_trait_type ---

#[test]
fn test_is_trait_type_methods_only_returns_true() {
    let mut reg = TypeRegistry::new();
    let mut methods = HashMap::new();
    methods.insert(
        "greet".to_string(),
        vec![MethodSignature {
            params: vec![("msg".to_string(), RustType::String)],
            return_type: None,
            has_rest: false,
        }],
    );
    reg.register(
        "Greeter".to_string(),
        TypeDef::new_interface(vec![], vec![], methods, vec![]),
    );
    assert!(reg.is_trait_type("Greeter"));
}

#[test]
fn test_is_trait_type_fields_only_returns_false() {
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        TypeDef::new_interface(
            vec![],
            vec![("x".to_string(), RustType::F64)],
            HashMap::new(),
            vec![],
        ),
    );
    assert!(!reg.is_trait_type("Point"));
}

#[test]
fn test_is_trait_type_mixed_returns_true() {
    let mut reg = TypeRegistry::new();
    let mut methods = HashMap::new();
    methods.insert(
        "greet".to_string(),
        vec![MethodSignature {
            params: vec![],
            return_type: None,
            has_rest: false,
        }],
    );
    reg.register(
        "Ctx".to_string(),
        TypeDef::new_interface(
            vec![],
            vec![("name".to_string(), RustType::String)],
            methods,
            vec![],
        ),
    );
    assert!(reg.is_trait_type("Ctx"));
}

#[test]
fn test_is_trait_type_unknown_returns_false() {
    let reg = TypeRegistry::new();
    assert!(!reg.is_trait_type("Unknown"));
}

// --- method signatures ---

#[test]
fn test_interface_method_return_type_stored_in_registry() {
    let module =
        parse_typescript("interface Formatter { format(input: string): string; }").unwrap();
    let reg = build_registry(&module);
    match reg.get("Formatter").unwrap() {
        TypeDef::Struct { methods, .. } => {
            let sigs = methods.get("format").expect("format method should exist");
            let sig = sigs.first().expect("should have at least one signature");
            assert_eq!(sig.params, vec![("input".to_string(), RustType::String)]);
            assert_eq!(sig.return_type, Some(RustType::String));
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

#[test]
fn test_interface_method_without_return_type_stores_none() {
    let module = parse_typescript("interface Logger { log(msg: string); }").unwrap();
    let reg = build_registry(&module);
    match reg.get("Logger").unwrap() {
        TypeDef::Struct { methods, .. } => {
            let sigs = methods.get("log").expect("log method should exist");
            let sig = sigs.first().expect("should have at least one signature");
            assert_eq!(sig.return_type, None);
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

#[test]
fn test_class_method_return_type_stored_in_registry() {
    let module =
        parse_typescript("class Parser { parse(input: string): number { return 0; } }").unwrap();
    let reg = build_registry(&module);
    match reg.get("Parser").unwrap() {
        TypeDef::Struct { methods, .. } => {
            let sigs = methods.get("parse").expect("parse method should exist");
            let sig = sigs.first().expect("should have at least one signature");
            assert_eq!(sig.return_type, Some(RustType::F64));
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

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

// ── TypeRegistry::lookup_method_sigs / lookup_field_type / resolve_type_def ──

#[test]
fn test_lookup_method_sigs_named_type() {
    let mut reg = TypeRegistry::new();
    let mut methods = HashMap::new();
    methods.insert(
        "greet".to_string(),
        vec![MethodSignature {
            params: vec![("msg".to_string(), RustType::String)],
            return_type: None,
            has_rest: false,
        }],
    );
    reg.register(
        "Greeter".to_string(),
        TypeDef::new_interface(vec![], vec![], methods, vec![]),
    );

    let result = reg.lookup_method_sigs(
        &RustType::Named {
            name: "Greeter".to_string(),
            type_args: vec![],
        },
        "greet",
    );
    assert!(result.is_some(), "should find method on Named type");
    assert_eq!(result.unwrap().len(), 1);
}

#[test]
fn test_lookup_method_sigs_string_type() {
    let mut reg = TypeRegistry::new();
    let mut methods = HashMap::new();
    methods.insert(
        "charAt".to_string(),
        vec![MethodSignature {
            params: vec![("pos".to_string(), RustType::F64)],
            return_type: Some(RustType::String),
            has_rest: false,
        }],
    );
    reg.register(
        "String".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods,
            constructor: None,
            extends: vec![],
            is_interface: true,
        },
    );

    let result = reg.lookup_method_sigs(&RustType::String, "charAt");
    assert!(
        result.is_some(),
        "should find method on RustType::String via String interface"
    );
}

#[test]
fn test_lookup_method_sigs_vec_type() {
    let mut reg = TypeRegistry::new();
    let mut methods = HashMap::new();
    methods.insert(
        "push".to_string(),
        vec![MethodSignature {
            params: vec![(
                "items".to_string(),
                RustType::Vec(Box::new(RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![],
                })),
            )],
            return_type: Some(RustType::F64),
            has_rest: true,
        }],
    );
    reg.register(
        "Array".to_string(),
        TypeDef::Struct {
            type_params: vec![crate::ir::TypeParam {
                name: "T".to_string(),
                constraint: None,
            }],
            fields: vec![],
            methods,
            constructor: None,
            extends: vec![],
            is_interface: true,
        },
    );

    let result = reg.lookup_method_sigs(&RustType::Vec(Box::new(RustType::F64)), "push");
    assert!(
        result.is_some(),
        "should find method on Vec via Array→instantiate"
    );
}

#[test]
fn test_lookup_method_sigs_unknown_type_returns_none() {
    let reg = TypeRegistry::new();
    // F64, Bool etc. have no methods
    assert!(reg.lookup_method_sigs(&RustType::F64, "foo").is_none());
    assert!(reg.lookup_method_sigs(&RustType::Bool, "foo").is_none());
}

#[test]
fn test_lookup_field_type_named_struct() {
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            Default::default(),
            vec![],
        ),
    );

    let result = reg.lookup_field_type(
        &RustType::Named {
            name: "Point".to_string(),
            type_args: vec![],
        },
        "x",
    );
    assert_eq!(result, Some(RustType::F64));

    // Non-existent field
    assert!(reg
        .lookup_field_type(
            &RustType::Named {
                name: "Point".to_string(),
                type_args: vec![],
            },
            "z",
        )
        .is_none());
}

#[test]
fn test_lookup_field_type_unknown_type_returns_none() {
    let reg = TypeRegistry::new();
    assert!(reg.lookup_field_type(&RustType::F64, "length").is_none());
}
