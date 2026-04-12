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

// --- is_trait_type ---

#[test]
fn test_is_trait_type_methods_only_returns_true() {
    let mut reg = TypeRegistry::new();
    let mut methods = HashMap::new();
    methods.insert(
        "greet".to_string(),
        vec![MethodSignature {
            params: vec![("msg".to_string(), RustType::String).into()],
            return_type: None,
            has_rest: false,
            type_params: vec![],
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
            vec![("x".to_string(), RustType::F64).into()],
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
            type_params: vec![],
        }],
    );
    reg.register(
        "Ctx".to_string(),
        TypeDef::new_interface(
            vec![],
            vec![("name".to_string(), RustType::String).into()],
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
            assert_eq!(
                sig.params,
                vec![("input".to_string(), RustType::String).into()]
            );
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

// ── class: mixed members (field + constructor + method) ──

#[test]
fn test_class_with_fields_constructor_and_methods() {
    let module = parse_typescript(
        r#"
        class Service {
            name: string;
            constructor(n: string) {}
            process(input: number): boolean { return true; }
        }
        "#,
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Service").expect("Service should be registered");
    if let TypeDef::Struct {
        fields,
        constructor,
        methods,
        ..
    } = def
    {
        // field
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "name");
        assert_eq!(fields[0].ty, RustType::String);

        // constructor
        let ctor = constructor.as_ref().expect("constructor should be present");
        assert_eq!(ctor[0].params.len(), 1);
        assert_eq!(ctor[0].params[0].name, "n");
        assert_eq!(ctor[0].params[0].ty, RustType::String);

        // method
        let process_sigs = methods.get("process").expect("process method");
        assert_eq!(process_sigs[0].params[0].ty, RustType::F64);
        assert_eq!(process_sigs[0].return_type, Some(RustType::Bool));
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── G5: パス C (TypeRef) ──

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
        assert!(fields
            .iter()
            .any(|f| f.name == "text" && f.ty == RustType::String));
        assert!(fields
            .iter()
            .any(|f| f.name == "json" && f.ty == RustType::Bool));
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
        assert_eq!(
            fields.len(),
            2,
            "Partial<Body> should have 2 Optional fields, got: {fields:?}"
        );
        assert!(
            fields.iter().any(|f| f.name == "text"
                && matches!(&f.ty, RustType::Option(inner) if **inner == RustType::String)),
            "text field should be Option<String>, got: {fields:?}"
        );
        assert!(
            fields.iter().any(|f| f.name == "json"
                && matches!(&f.ty, RustType::Option(inner) if **inner == RustType::Bool)),
            "json field should be Option<Bool>, got: {fields:?}"
        );
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

#[test]
fn test_type_alias_type_ref_with_pick() {
    let module = parse_typescript(
        "interface Body { text: string; json: boolean }
         type TextOnly = Pick<Body, 'text'>;",
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg.get("TextOnly").expect("TextOnly should be registered");
    if let TypeDef::Struct { fields, .. } = def {
        assert_eq!(fields.len(), 1, "Pick should have 1 field, got: {fields:?}");
        assert_eq!(fields[0].name, "text");
        assert_eq!(fields[0].ty, RustType::String);
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

#[test]
fn test_type_alias_simple_ref_copies_methods() {
    let module = parse_typescript(
        "interface Greeter { name: string; greet(msg: string): void; }
         type MyGreeter = Greeter;",
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg
        .get("MyGreeter")
        .expect("MyGreeter should be registered");
    if let TypeDef::Struct {
        fields, methods, ..
    } = def
    {
        assert_eq!(fields.len(), 1, "should copy 1 field, got: {fields:?}");
        assert_eq!(fields[0].name, "name");
        assert!(
            methods.contains_key("greet"),
            "should copy 'greet' method from Greeter"
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
            .any(|f| f.name == "x" && f.ty == RustType::F64));
        assert!(fields
            .iter()
            .any(|f| f.name == "y" && f.ty == RustType::String));
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── G11: intersection で未宣言 TypeRef は _N field として埋め込み ──

#[test]
fn test_type_alias_intersection_unresolvable_refs_embedded_as_fields() {
    // Foo, Bar は未宣言 → registry に未登録 → resolve_ts_type で Named として解決
    // → _N field として埋め込み（暗黙無視ではなく明示的に参照を保持）
    let module = parse_typescript("type X = Foo & Bar;").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("X").expect("X should be registered");
    if let TypeDef::Struct { fields, .. } = def {
        assert_eq!(
            fields.len(),
            2,
            "unresolvable TypeRefs should be embedded as _N fields, got: {fields:?}"
        );
        assert!(fields.iter().any(|f| f.name == "_0"));
        assert!(fields.iter().any(|f| f.name == "_1"));
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── G12: intersection で未宣言 TypeRef + TypeLiteral ──

#[test]
fn test_type_alias_intersection_unregistered_ref_with_literal() {
    // UnknownType は _0 field として埋め込み、TypeLiteral の y は通常フィールド
    let module = parse_typescript("type X = UnknownType & { y: string };").unwrap();
    let reg = build_registry(&module);
    let def = reg
        .get("X")
        .expect("X should be registered with merged fields");
    if let TypeDef::Struct { fields, .. } = def {
        assert_eq!(
            fields.len(),
            2,
            "should have _0 (UnknownType) + y, got: {fields:?}"
        );
        assert!(
            fields.iter().any(|f| f.name == "_0"),
            "UnknownType should be _0 field"
        );
        assert!(
            fields
                .iter()
                .any(|f| f.name == "y" && f.ty == RustType::String),
            "y should be String"
        );
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── intersection: 3+ members ──

#[test]
fn test_type_alias_intersection_three_members() {
    let module = parse_typescript(
        "interface A { x: number; }
         interface B { y: string; }
         type X = A & B & { z: boolean };",
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg.get("X").expect("X should be registered");
    if let TypeDef::Struct { fields, .. } = def {
        assert_eq!(fields.len(), 3, "expected 3 merged fields, got {fields:?}");
        assert!(fields
            .iter()
            .any(|f| f.name == "x" && f.ty == RustType::F64));
        assert!(fields
            .iter()
            .any(|f| f.name == "y" && f.ty == RustType::String));
        assert!(fields
            .iter()
            .any(|f| f.name == "z" && f.ty == RustType::Bool));
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── intersection: TypeLiteral optional field ──

#[test]
fn test_type_alias_intersection_type_literal_optional_field() {
    let module = parse_typescript(
        "interface Base { id: number; }
         type X = Base & { opt?: string; };",
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg.get("X").expect("X should be registered");
    if let TypeDef::Struct { fields, .. } = def {
        // Base から来る id フィールドの検証
        assert!(
            fields
                .iter()
                .any(|f| f.name == "id" && f.ty == RustType::F64),
            "id field should be merged from Base, got: {fields:?}"
        );
        // optional フィールドの検証
        let opt_field = fields.iter().find(|f| f.name == "opt").expect("opt field");
        assert!(
            opt_field.optional,
            "opt field should have optional=true, got: {:?}",
            opt_field
        );
        assert!(
            matches!(&opt_field.ty, RustType::Option(inner) if **inner == RustType::String),
            "opt field type should be Option<String>, got: {:?}",
            opt_field.ty
        );
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── intersection: TypeRef with type args (instantiation) ──

#[test]
fn test_type_alias_intersection_generic_type_ref_instantiated() {
    let module = parse_typescript(
        "interface Container<T> { value: T; }
         type X = Container<string> & { extra: number; };",
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg.get("X").expect("X should be registered");
    if let TypeDef::Struct { fields, .. } = def {
        assert_eq!(fields.len(), 2, "expected 2 fields, got {fields:?}");
        // Container<string> → value: String (instantiated, not TypeVar)
        let value_field = fields
            .iter()
            .find(|f| f.name == "value")
            .expect("value field");
        assert_eq!(
            value_field.ty,
            RustType::String,
            "value should be String (instantiated), got: {:?}",
            value_field.ty
        );
        assert!(
            fields
                .iter()
                .any(|f| f.name == "extra" && f.ty == RustType::F64),
            "extra field should be F64, got: {fields:?}"
        );
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── intersection: TypeRef to non-Struct (Enum) ──

#[test]
fn test_type_alias_intersection_enum_ref_embedded_as_field() {
    let module = parse_typescript(
        "type Status = 'active' | 'inactive';
         type X = { id: number } & Status;",
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg.get("X").expect("X should be registered");
    if let TypeDef::Struct { fields, .. } = def {
        // id フィールドは TypeLiteral から
        assert!(
            fields
                .iter()
                .any(|f| f.name == "id" && f.ty == RustType::F64),
            "should have id field, got: {fields:?}"
        );
        // Status (Enum) は Struct ではないため _N field として埋め込まれる
        assert!(
            fields.iter().any(|f| f.name.starts_with('_')),
            "Enum TypeRef should be embedded as _N field, got: {fields:?}"
        );
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── intersection: TypeLiteral member with method ──

#[test]
fn test_type_alias_intersection_type_literal_with_method() {
    let module = parse_typescript(
        "interface Named { name: string; }
         type X = Named & { greet(): void; };",
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg.get("X").expect("X should be registered");
    if let TypeDef::Struct {
        fields, methods, ..
    } = def
    {
        assert!(
            fields.iter().any(|f| f.name == "name"),
            "should have 'name' field from Named"
        );
        assert!(
            methods.contains_key("greet"),
            "should have 'greet' method from type literal"
        );
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

// ── type alias with method/call/construct signatures ──

#[test]
fn test_type_alias_with_method_signature() {
    let module = parse_typescript("type Handler = { handle(x: string): void; }").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Handler").expect("Handler should be registered");
    if let TypeDef::Struct { methods, .. } = def {
        let sigs = methods.get("handle").expect("handle method should exist");
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].params.len(), 1);
        assert_eq!(sigs[0].params[0].name, "x");
        assert_eq!(sigs[0].params[0].ty, RustType::String);
        assert_eq!(sigs[0].return_type, Some(RustType::Unit));
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

#[test]
fn test_type_alias_with_call_signature_and_field() {
    // call-sig-only は try_collect_fn_type_alias で Function になる。
    // call sig + field の mixed case がパス A で Struct になることを検証。
    let module =
        parse_typescript("type Callable = { (input: string): number; name: string; }").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Callable").expect("Callable should be registered");
    if let TypeDef::Struct {
        call_signatures,
        fields,
        ..
    } = def
    {
        assert_eq!(call_signatures.len(), 1, "should have 1 call signature");
        assert_eq!(call_signatures[0].params.len(), 1);
        assert_eq!(call_signatures[0].params[0].name, "input");
        assert_eq!(call_signatures[0].return_type, Some(RustType::F64));
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "name");
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

#[test]
fn test_type_alias_with_construct_signature() {
    let module = parse_typescript("type Factory = { new(config: string): Factory; }").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Factory").expect("Factory should be registered");
    if let TypeDef::Struct { constructor, .. } = def {
        let ctors = constructor.as_ref().expect("constructor should be present");
        assert_eq!(ctors.len(), 1);
        assert_eq!(ctors[0].params.len(), 1);
        assert_eq!(ctors[0].params[0].name, "config");
        assert_eq!(ctors[0].params[0].ty, RustType::String);
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── テストカバレッジ補完 (G1-G6) ──

/// G1: Index signature のみの type alias → pass-1 placeholder のまま（TypeDef::Alias 別 PRD）
#[test]
fn test_type_alias_index_sig_only_stays_placeholder() {
    let module = parse_typescript("type Dict = { [key: string]: number; }").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Dict").expect("pass-1 should register placeholder");
    if let TypeDef::Struct { fields, .. } = def {
        // index signature のみの TypeLiteral は空 fields の struct として登録される
        assert!(
            fields.is_empty(),
            "index-sig-only type alias should have empty fields (TypeDef::Alias 別 PRD)"
        );
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

/// G4: generic intersection — TypeVar 保持確認
#[test]
fn test_type_alias_generic_intersection_preserves_type_var() {
    let module = parse_typescript(
        "interface Base { id: number; }
         type WithBase<T> = Base & { extra: T; };",
    )
    .unwrap();
    let mut synthetic = crate::pipeline::SyntheticTypeRegistry::new();
    let reg = build_registry_with_synthetic(&module, &mut synthetic);
    let def = reg.get("WithBase").expect("WithBase should be registered");
    if let TypeDef::Struct {
        fields,
        type_params,
        ..
    } = def
    {
        // T は TypeVar として残る（monomorphize されない）
        let extra_field = fields
            .iter()
            .find(|f| f.name == "extra")
            .expect("extra field");
        assert!(
            matches!(&extra_field.ty, RustType::TypeVar { name } if name == "T"),
            "T should be TypeVar in registry, got: {:?}",
            extra_field.ty
        );
        assert!(type_params.iter().any(|tp| tp.name == "T"));
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

/// G5: resolve 失敗時、TypeDef 全体が登録されない（pass-1 placeholder のまま）
///
/// `type X = { x: typeof UnknownVar }` のように resolve_ts_type が失敗するフィールド型を含む場合、
/// resolve_struct_for_registry が Err を返し、Pass 1 placeholder（空 struct）が残る。
#[test]
fn test_type_alias_resolve_failure_stays_placeholder() {
    // `typeof UnknownVar` は registry に UnknownVar がないためエラー
    let module = parse_typescript("type X = { value: typeof UnknownVar; };").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("X").expect("pass-1 should register placeholder");
    if let TypeDef::Struct { fields, .. } = def {
        assert!(
            fields.is_empty(),
            "resolve failure should keep pass-1 placeholder (empty fields), got: {fields:?}"
        );
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

/// G6: mapped type alias → resolve_ts_type で HashMap に解決され、
/// パス C で Named 以外のため登録スキップ
#[test]
fn test_type_alias_mapped_type_stays_placeholder() {
    let module = parse_typescript("type Mapped<T> = { [K in keyof T]: string };").unwrap();
    let reg = build_registry(&module);
    let def = reg
        .get("Mapped")
        .expect("pass-1 should register placeholder");
    if let TypeDef::Struct { fields, .. } = def {
        // mapped type は HashMap に resolve されるが TypeDef::Struct として登録できない
        // → pass-1 placeholder (空 fields) のまま
        assert!(
            fields.is_empty(),
            "mapped type alias should have empty fields (HashMap cannot be TypeDef::Struct)"
        );
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── Pass 2a/2b split verification ──

#[test]
fn test_pass_2b_sees_forward_declared_callable_interface() {
    // const handler: Handler = ... が先に書かれ、
    // interface Handler が後方で宣言されている場合、
    // Pass 2b で handler の型注釈 "Handler" が正しく解決できる
    let source = r#"
        const handler: Handler = (req: string): string => req;
        interface Handler {
            (req: string): string;
        }
    "#;
    let module = parse_typescript(source).unwrap();
    let reg = build_registry(&module);

    // Handler interface は Pass 2a で解決済
    let handler_def = reg.get("Handler").expect("Handler should be registered");
    if let TypeDef::Struct {
        call_signatures, ..
    } = handler_def
    {
        assert_eq!(call_signatures.len(), 1, "Handler should have 1 call sig");
    } else {
        panic!("expected TypeDef::Struct for Handler, got {handler_def:?}");
    }

    // handler const は Pass 2b で ConstValue として登録される (P2.4)
    let handler_const = reg.get("handler").expect("handler should be registered");
    if let TypeDef::ConstValue {
        type_ref_name: Some(ref_name),
        ..
    } = handler_const
    {
        assert_eq!(ref_name, "Handler");
    } else {
        panic!(
            "expected TypeDef::ConstValue with type_ref_name for handler, got {handler_const:?}"
        );
    }
}

#[test]
fn test_non_callable_interface_arrow_remains_function() {
    // 非 callable interface の型注釈を持つ arrow は従来通り Function として登録
    let source = r#"
        interface Config {
            name: string;
        }
        const factory: Config = (x: string): string => x;
    "#;
    let module = parse_typescript(source).unwrap();
    let reg = build_registry(&module);

    let factory = reg.get("factory").expect("factory should be registered");
    assert!(
        matches!(factory, TypeDef::Function { .. }),
        "non-callable interface arrow should be TypeDef::Function, got {factory:?}"
    );
}
