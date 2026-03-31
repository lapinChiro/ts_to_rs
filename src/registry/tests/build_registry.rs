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

// ── class with constructor ──

#[test]
fn test_class_with_only_constructor_is_registered() {
    let module = parse_typescript(
        r#"
        class Handler {
            constructor(name: string, count: number) {}
        }
        "#,
    )
    .unwrap();

    let reg = build_registry(&module);
    let def = reg.get("Handler");
    assert!(
        def.is_some(),
        "class with only a constructor should be registered in TypeRegistry"
    );
    if let Some(TypeDef::Struct { constructor, .. }) = def {
        assert!(
            constructor.is_some(),
            "constructor signature should be present"
        );
    } else {
        panic!("expected TypeDef::Struct");
    }
}

// ── G5: collect_type_alias_fields TsTypeRef ──

#[test]
fn test_type_alias_type_ref_resolves_fields() {
    let module = parse_typescript(
        "interface Body { text: string; json: boolean }
         type BodyCache = Body;",
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg
        .get("BodyCache")
        .expect("BodyCache should be registered");
    if let TypeDef::Struct { fields, .. } = def {
        assert_eq!(fields.len(), 2);
        assert!(fields.iter().any(|(n, _)| n == "text"));
        assert!(fields.iter().any(|(n, _)| n == "json"));
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

#[test]
fn test_type_alias_type_ref_with_utility_type() {
    let module = parse_typescript(
        "interface Body { text: string; json: boolean }
         type BodyCache = Partial<Body>;",
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg
        .get("BodyCache")
        .expect("BodyCache should be registered");
    if let TypeDef::Struct { fields, .. } = def {
        assert!(
            fields
                .iter()
                .all(|(_, ty)| matches!(ty, RustType::Option(_))),
            "all fields should be Option after Partial, got: {fields:?}"
        );
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── G10: intersection TsTypeLit & TsTypeLit ──

#[test]
fn test_type_alias_intersection_two_type_lits() {
    let module = parse_typescript("type Both = { x: number } & { y: string };").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Both").expect("Both should be registered");
    if let TypeDef::Struct { fields, .. } = def {
        assert_eq!(fields.len(), 2);
        assert!(fields
            .iter()
            .any(|(n, ty)| n == "x" && *ty == RustType::F64));
        assert!(fields
            .iter()
            .any(|(n, ty)| n == "y" && *ty == RustType::String));
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── G11: intersection all members have no fields → empty Struct from pass-1 ──

#[test]
fn test_type_alias_intersection_unresolvable_refs_empty_struct() {
    let module = parse_typescript("type X = Foo & Bar;").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("X").expect("pass-1 should register placeholder");
    if let TypeDef::Struct { fields, .. } = def {
        assert!(
            fields.is_empty(),
            "unresolvable intersection should have empty fields"
        );
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── G12: intersection with unregistered TsTypeRef → partial merge ──

#[test]
fn test_type_alias_intersection_unregistered_ref_partial_merge() {
    let module = parse_typescript("type X = UnknownType & { y: string };").unwrap();
    let reg = build_registry(&module);
    let def = reg
        .get("X")
        .expect("X should be registered with partial fields");
    if let TypeDef::Struct { fields, .. } = def {
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].0, "y");
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── callable interface → call_signatures registration ──

#[test]
fn test_callable_interface_registers_call_signatures() {
    let module = parse_typescript(
        "interface GetCookie { (c: string): string; (c: string, key: string): number }",
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg
        .get("GetCookie")
        .expect("GetCookie should be registered");
    if let TypeDef::Struct {
        call_signatures, ..
    } = def
    {
        assert_eq!(call_signatures.len(), 2, "should have 2 call signatures");
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

#[test]
fn test_interface_with_construct_signature_registers_constructor() {
    let module =
        parse_typescript("interface Builder { new (config: string): Builder; build(): void }")
            .unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Builder").expect("Builder should be registered");
    if let TypeDef::Struct {
        constructor,
        methods,
        ..
    } = def
    {
        assert!(constructor.is_some(), "constructor should be present");
        assert!(
            methods.contains_key("build"),
            "methods should contain build"
        );
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}
