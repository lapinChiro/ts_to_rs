use super::*;
use crate::ir::RustType;
use crate::parser::parse_typescript;

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
fn test_build_registry_function() {
    let module =
        parse_typescript("function draw(p: Point, color: string): boolean { return true; }")
            .unwrap();
    let reg = build_registry(&module);
    match reg.get("draw").unwrap() {
        TypeDef::Function {
            params,
            return_type,
            ..
        } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].0, "p");
            assert_eq!(
                params[0].1,
                RustType::Named {
                    name: "Point".to_string(),
                    type_args: vec![],
                }
            );
            assert_eq!(params[1].0, "color");
            assert_eq!(params[1].1, RustType::String);
            assert_eq!(*return_type, Some(RustType::Bool));
        }
        _ => panic!("expected Function"),
    }
}

#[test]
fn test_build_registry_arrow_function() {
    let module = parse_typescript("const greet = (name: string): string => name;").unwrap();
    let reg = build_registry(&module);
    match reg.get("greet").unwrap() {
        TypeDef::Function {
            params,
            return_type,
            ..
        } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].0, "name");
            assert_eq!(params[0].1, RustType::String);
            assert_eq!(*return_type, Some(RustType::String));
        }
        _ => panic!("expected Function"),
    }
}

#[test]
fn test_build_registry_fn_rest_param_sets_has_rest_true() {
    let module = parse_typescript("function sum(...nums: number[]): number { return 0; }").unwrap();
    let reg = build_registry(&module);
    match reg.get("sum").unwrap() {
        TypeDef::Function {
            params, has_rest, ..
        } => {
            assert!(has_rest, "has_rest should be true for rest param");
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].0, "nums");
            assert_eq!(params[0].1, RustType::Vec(Box::new(RustType::F64)));
        }
        _ => panic!("expected Function"),
    }
}

#[test]
fn test_build_registry_fn_mixed_and_rest_param() {
    let module =
        parse_typescript("function log(prefix: string, ...msgs: string[]): void {}").unwrap();
    let reg = build_registry(&module);
    match reg.get("log").unwrap() {
        TypeDef::Function {
            params, has_rest, ..
        } => {
            assert!(has_rest);
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].0, "prefix");
            assert_eq!(params[0].1, RustType::String);
            assert_eq!(params[1].0, "msgs");
            assert_eq!(params[1].1, RustType::Vec(Box::new(RustType::String)));
        }
        _ => panic!("expected Function"),
    }
}

#[test]
fn test_build_registry_fn_no_rest_param_has_rest_false() {
    let module = parse_typescript("function greet(name: string): void {}").unwrap();
    let reg = build_registry(&module);
    match reg.get("greet").unwrap() {
        TypeDef::Function { has_rest, .. } => {
            assert!(!has_rest, "has_rest should be false without rest param");
        }
        _ => panic!("expected Function"),
    }
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

// --- string literal union enum registration ---

#[test]
fn test_build_registry_string_literal_union_registers_enum() {
    let module = parse_typescript(r#"type Direction = "up" | "down" | "left" | "right";"#).unwrap();
    let reg = build_registry(&module);
    let def = reg
        .get("Direction")
        .expect("Direction should be registered");
    match def {
        TypeDef::Enum {
            variants,
            string_values,
            ..
        } => {
            assert_eq!(variants, &["Up", "Down", "Left", "Right"]);
            assert_eq!(string_values.get("up").unwrap(), "Up");
            assert_eq!(string_values.get("down").unwrap(), "Down");
            assert_eq!(string_values.get("left").unwrap(), "Left");
            assert_eq!(string_values.get("right").unwrap(), "Right");
        }
        other => panic!("expected Enum, got {other:?}"),
    }
}

#[test]
fn test_build_registry_ts_enum_has_empty_string_values() {
    let module = parse_typescript("enum Color { Red, Green, Blue }").unwrap();
    let reg = build_registry(&module);
    match reg.get("Color").unwrap() {
        TypeDef::Enum { string_values, .. } => {
            assert!(
                string_values.is_empty(),
                "TS enum should have empty string_values"
            );
        }
        other => panic!("expected Enum, got {other:?}"),
    }
}

// --- discriminated union registration ---

#[test]
fn test_build_registry_discriminated_union_registers_enum() {
    let module = parse_typescript(
        r#"type Shape = { kind: "circle", radius: number } | { kind: "square", side: number };"#,
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Shape").expect("Shape should be registered");
    match def {
        TypeDef::Enum {
            type_params: _,
            variants,
            string_values,
            tag_field,
            variant_fields,
        } => {
            assert_eq!(variants, &["Circle", "Square"]);
            assert_eq!(tag_field.as_deref(), Some("kind"));
            assert_eq!(string_values.get("circle").unwrap(), "Circle");
            assert_eq!(string_values.get("square").unwrap(), "Square");
            // Circle variant has radius: f64
            let circle_fields = variant_fields.get("Circle").expect("Circle variant");
            assert_eq!(circle_fields, &[("radius".to_string(), RustType::F64)]);
            // Square variant has side: f64
            let square_fields = variant_fields.get("Square").expect("Square variant");
            assert_eq!(square_fields, &[("side".to_string(), RustType::F64)]);
        }
        other => panic!("expected Enum, got {other:?}"),
    }
}

#[test]
fn test_build_registry_discriminated_union_unit_variant() {
    let module =
        parse_typescript(r#"type Status = { type: "active" } | { type: "inactive" };"#).unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Status").expect("Status should be registered");
    match def {
        TypeDef::Enum {
            variants,
            tag_field,
            variant_fields,
            ..
        } => {
            assert_eq!(variants, &["Active", "Inactive"]);
            assert_eq!(tag_field.as_deref(), Some("type"));
            assert!(
                variant_fields.get("Active").unwrap().is_empty(),
                "unit variant should have no fields"
            );
        }
        other => panic!("expected Enum, got {other:?}"),
    }
}

// --- Function type alias registration ---

#[test]
fn test_build_registry_fn_type_alias_with_params() {
    // type Handler = (c: string) => number;
    let module = parse_typescript("type Handler = (c: string) => number;").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Handler").expect("Handler should be registered");
    match def {
        TypeDef::Function {
            params,
            return_type,
            ..
        } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].0, "c");
            assert_eq!(params[0].1, RustType::String);
            assert_eq!(*return_type, Some(RustType::F64));
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

#[test]
fn test_build_registry_fn_type_alias_no_params() {
    // type Factory = () => string;
    let module = parse_typescript("type Factory = () => string;").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Factory").expect("Factory should be registered");
    match def {
        TypeDef::Function {
            params,
            return_type,
            ..
        } => {
            assert!(params.is_empty(), "expected no params, got {:?}", params);
            assert_eq!(*return_type, Some(RustType::String));
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

#[test]
fn test_is_trait_type_methods_only_returns_true() {
    let mut reg = TypeRegistry::new();
    let mut methods = HashMap::new();
    methods.insert(
        "greet".to_string(),
        vec![MethodSignature {
            params: vec![("msg".to_string(), RustType::String)],
            return_type: None,
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

#[test]
fn test_build_registry_forward_reference_resolves_type() {
    // Interface A references interface B, but A is declared first.
    // With 2-pass construction, B should be registered before A's fields are resolved.
    let module = parse_typescript("interface A { b: B } interface B { x: number; }").unwrap();
    let reg = build_registry(&module);

    // A should have field b with type Named { name: "B" }
    match reg.get("A").unwrap() {
        TypeDef::Struct { fields, .. } => {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].0, "b");
            assert!(matches!(&fields[0].1, RustType::Named { name, .. } if name == "B"));
        }
        other => panic!("expected Struct, got: {:?}", other),
    }
    // B should also be registered
    assert!(reg.get("B").is_some());
}

#[test]
fn test_interface_method_return_type_stored_in_registry() {
    // interface に戻り値型付きメソッドを定義すると、MethodSignature に格納される
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
    // 戻り値型アノテーションなしのメソッド → return_type が None
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
    // class メソッドの戻り値型も MethodSignature に格納される
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

// --- I-100: Generics Foundation ---

#[test]
fn test_generic_interface_type_params_stored_in_registry() {
    // interface Container<T> { value: T; } → TypeDef に type_params: ["T"] が格納される
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
            // フィールド value は型パラメータ T（Named("T")）
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].0, "value");
            assert!(
                matches!(&fields[0].1, RustType::Named { name, .. } if name == "T"),
                "expected Named(T), got {:?}",
                fields[0].1
            );
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

#[test]
fn test_generic_interface_constraint_stored_in_registry() {
    // interface Processor<T extends Serializable> { ... }
    // → type_params に constraint: Some(Named("Serializable")) が格納される
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
    // Container<T> { value: T } を instantiate("Container", [String]) →
    // fields に value: String が入る
    let module = parse_typescript("interface Container<T> { value: T; }").unwrap();
    let reg = build_registry(&module);
    let instantiated = reg
        .instantiate("Container", &[RustType::String])
        .expect("instantiate should succeed");
    match instantiated {
        TypeDef::Struct { fields, .. } => {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].0, "value");
            assert_eq!(fields[0].1, RustType::String);
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

#[test]
fn test_instantiate_non_generic_returns_original() {
    // 型パラメータなしの型 → instantiate しても元の TypeDef が返る
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
    // 型引数の数が不一致 → 元の TypeDef が返る
    let module = parse_typescript("interface Container<T> { value: T; }").unwrap();
    let reg = build_registry(&module);
    let original = reg.get("Container").unwrap().clone();
    let instantiated = reg
        .instantiate("Container", &[RustType::String, RustType::F64])
        .expect("instantiate should succeed");
    assert_eq!(instantiated, original);
}

// --- TypeDef 型パラメータ関連テスト ---

#[test]
fn test_build_registry_with_union_field() {
    let module = crate::parser::parse_typescript("interface Foo { x: string | number; }").unwrap();
    let mut synthetic = SyntheticTypeRegistry::new();
    let reg = build_registry_with_synthetic(&module, &mut synthetic);

    // Foo should be registered
    let foo = reg.get("Foo");
    assert!(foo.is_some(), "Foo should be in registry");

    // x's type should be Named (the synthetic enum)
    if let Some(TypeDef::Struct { fields, .. }) = foo {
        assert_eq!(fields.len(), 1, "Foo should have 1 field");
        let (name, ty) = &fields[0];
        assert_eq!(name, "x");
        assert!(
            matches!(ty, RustType::Named { .. }),
            "x should be a Named type (synthetic enum), got: {ty:?}"
        );
    } else {
        panic!("Foo should be a Struct");
    }

    // SyntheticTypeRegistry should have the union enum
    assert!(
        !synthetic.all_items().is_empty(),
        "SyntheticTypeRegistry should contain the union enum"
    );
}

#[test]
fn test_build_registry_union_dedup() {
    let module = crate::parser::parse_typescript(
        "interface A { x: string | number; } interface B { y: string | number; }",
    )
    .unwrap();
    let mut synthetic = SyntheticTypeRegistry::new();
    let _reg = build_registry_with_synthetic(&module, &mut synthetic);

    // Both A.x and B.y use string | number → should be 1 synthetic enum (deduplicated)
    let enum_count = synthetic
        .all_items()
        .iter()
        .filter(|item| matches!(item, crate::ir::Item::Enum { .. }))
        .count();
    assert_eq!(
        enum_count, 1,
        "same union type should produce only 1 enum (dedup)"
    );
}

#[test]
fn test_analyze_any_params_registers_enum() {
    use crate::transformer::any_narrowing::{build_any_enum_variants, collect_any_constraints};

    let module = crate::parser::parse_typescript(
        r#"function foo(x: any) { if (typeof x === "string") { return x; } }"#,
    )
    .unwrap();
    let reg = build_registry(&module);

    // Verify any-typed parameter exists
    let foo_def = reg.get("foo");
    assert!(foo_def.is_some(), "foo should be in registry");
    if let Some(TypeDef::Function { params, .. }) = foo_def {
        assert!(
            params.iter().any(|(_, ty)| matches!(ty, RustType::Any)),
            "foo should have an any-typed parameter"
        );
    }

    // Simulate AnyTypeAnalyzer: collect constraints and generate enum
    if let Some(ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl)))) =
        module.body.first()
    {
        if let Some(body) = &fn_decl.function.body {
            let constraints = collect_any_constraints(body, &["x".to_string()]);
            if let Some(constraint) = constraints.get("x") {
                let variants = build_any_enum_variants(constraint);
                assert!(
                    !variants.is_empty(),
                    "should generate variants for any-typed parameter"
                );
            }
        }
    }
}

#[test]
fn test_transpile_collecting_synthetic_output() {
    let source = "export function foo(x: string | number): void { }";
    let (output, _unsupported) = crate::transpile_collecting(source).unwrap();
    // Output should contain the synthetic enum
    assert!(
        output.contains("enum"),
        "transpile output should contain synthetic enum for union type, got: {output}"
    );
    // Output should contain the function
    assert!(
        output.contains("fn foo"),
        "transpile output should contain the function"
    );
}

// --- I-218: class / type alias 型パラメータ収集 ---

#[test]
fn test_collect_class_type_params_single() {
    // class Foo<T> { value: T } → TypeDef::Struct に type_params: [T] が格納される
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
            assert_eq!(fields[0].0, "value");
            assert!(
                matches!(&fields[0].1, RustType::Named { name, .. } if name == "T"),
                "expected Named(T), got {:?}",
                fields[0].1
            );
        }
        other => panic!("expected Struct, got {other:?}"),
    }
}

#[test]
fn test_collect_class_type_params_multiple_with_constraint() {
    // class Foo<T extends Bar, U> → type_params: [T: Bar, U]
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
    // type Pair<A, B> = { first: A, second: B } → TypeDef::Struct に type_params: [A, B]
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
    // type Result<T> = { kind: "ok", value: T } | { kind: "error", msg: string }
    // → TypeDef::Enum に type_params: [T]
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
            // "ok" variant should have field "value" of type T
            let ok_fields = variant_fields.get("Ok").expect("Ok variant should exist");
            assert!(
                ok_fields.iter().any(|(name, ty)| name == "value"
                    && matches!(ty, RustType::Named { name, .. } if name == "T")),
                "expected Ok variant to have field 'value: T', got {ok_fields:?}"
            );
        }
        other => panic!("expected Enum, got {other:?}"),
    }
}

#[test]
fn test_substitute_types_enum_variant_fields() {
    // DU Enum の instantiate: T → String で variant_fields 内の T が置換される
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
                )],
            ),
            (
                "Error".to_string(),
                vec![("msg".to_string(), RustType::String)],
            ),
        ]),
    };
    let bindings = HashMap::from([("T".to_string(), RustType::String)]);
    let result = enum_def.substitute_types(&bindings);
    match &result {
        TypeDef::Enum { variant_fields, .. } => {
            let ok_fields = variant_fields.get("Ok").unwrap();
            assert_eq!(
                ok_fields[0].1,
                RustType::String,
                "T should be substituted to String"
            );
            let err_fields = variant_fields.get("Error").unwrap();
            assert_eq!(
                err_fields[0].1,
                RustType::String,
                "String should remain unchanged"
            );
        }
        other => panic!("expected Enum, got {other:?}"),
    }
}

#[test]
fn test_substitute_types_enum_multiple_params() {
    // 複数型パラメータの DU Enum: T → String, E → i64
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
                )],
            ),
            (
                "Err".to_string(),
                vec![(
                    "error".to_string(),
                    RustType::Named {
                        name: "E".to_string(),
                        type_args: vec![],
                    },
                )],
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
            assert_eq!(ok_fields[0].1, RustType::String);
            let err_fields = variant_fields.get("Err").unwrap();
            assert_eq!(err_fields[0].1, RustType::F64);
        }
        other => panic!("expected Enum, got {other:?}"),
    }
}

#[test]
fn test_build_registry_call_signature_type_alias_registers_as_function() {
    // type Handler = { (c: string): number }
    let module = parse_typescript("type Handler = { (c: string): number };").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Handler").expect("Handler should be registered");
    match def {
        TypeDef::Function {
            params,
            return_type,
            ..
        } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].0, "c");
            assert_eq!(params[0].1, RustType::String);
            assert_eq!(*return_type, Some(RustType::F64));
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

#[test]
fn test_build_registry_call_signature_type_alias_multiple_params() {
    // type Callback = { (a: string, b: number): boolean }
    let module = parse_typescript("type Callback = { (a: string, b: number): boolean };").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Callback").expect("Callback should be registered");
    match def {
        TypeDef::Function {
            params,
            return_type,
            ..
        } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0], ("a".to_string(), RustType::String));
            assert_eq!(params[1], ("b".to_string(), RustType::F64));
            assert_eq!(*return_type, Some(RustType::Bool));
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

#[test]
fn test_build_registry_call_signature_type_alias_no_params() {
    // type Factory = { (): string }
    let module = parse_typescript("type Factory = { (): string };").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Factory").expect("Factory should be registered");
    match def {
        TypeDef::Function {
            params,
            return_type,
            ..
        } => {
            assert!(params.is_empty());
            assert_eq!(*return_type, Some(RustType::String));
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

#[test]
fn test_build_registry_call_signature_overload_picks_longest() {
    // type GetCookie = { (c: string): string; (c: string, key: string): number }
    let module = parse_typescript(
        "type GetCookie = { (c: string): string; (c: string, key: string): number };",
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg
        .get("GetCookie")
        .expect("GetCookie should be registered");
    match def {
        TypeDef::Function {
            params,
            return_type,
            ..
        } => {
            // Picks the overload with the most params: (c: string, key: string): number
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].0, "c");
            assert_eq!(params[1].0, "key");
            assert_eq!(*return_type, Some(RustType::F64));
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

#[test]
fn test_build_registry_call_signature_with_properties_stays_struct() {
    // type Mixed = { name: string; (x: number): void }
    // Properties + call signature → should stay as Struct (not Function)
    let module = parse_typescript("type Mixed = { name: string; (x: number): void };").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Mixed").expect("Mixed should be registered");
    match def {
        TypeDef::Struct { fields, .. } => {
            assert!(
                fields.iter().any(|(n, _)| n == "name"),
                "should have 'name' field"
            );
        }
        other => panic!("expected Struct (mixed call sig + properties), got {other:?}"),
    }
}
