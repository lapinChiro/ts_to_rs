use super::*;

#[test]
fn test_resolve_const_with_type_annotation() {
    let res = resolve("const x: number = 42;");
    // The initializer `42` should have type f64
    assert!(
        !res.expr_types.is_empty(),
        "should have at least one expr type"
    );
    // Check that at least one entry is Known(F64)
    let has_f64 = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Known(RustType::F64)));
    assert!(has_f64, "initializer 42 should resolve to f64");
}

#[test]
fn test_resolve_let_string_literal() {
    let res = resolve(r#"let y = "hello";"#);
    let has_string = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Known(RustType::String)));
    assert!(has_string, "string literal should resolve to String");
}

#[test]
fn test_resolve_let_with_reassignment_is_mutable() {
    let res = resolve("let z = 1; z = 2;");
    let is_mut = res.var_mutability.values().any(|&m| m);
    assert!(is_mut, "reassigned variable should be mutable");
}

#[test]
fn test_resolve_const_is_not_mutable() {
    let res = resolve("const x = 1;");
    let all_immutable = res.var_mutability.values().all(|&m| !m);
    assert!(all_immutable, "const variable should not be mutable");
}

#[test]
fn test_resolve_function_param_type() {
    let res = resolve("function foo(x: string): number { return 0; }");
    // x should be in scope as String
    // return 0 should have expected type f64
    let has_expected = !res.expected_types.is_empty();
    assert!(has_expected, "return statement should have expected type");
}

#[test]
fn test_resolve_expected_type_var_decl() {
    let res = resolve("const x: number = 42;");
    let has_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::F64));
    assert!(
        has_expected,
        "initializer should have expected type from annotation"
    );
}

#[test]
fn test_resolve_expected_type_return_stmt() {
    let res = resolve("function foo(): string { return 42; }");
    let has_string_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::String));
    assert!(
        has_string_expected,
        "return expression should have expected type String"
    );
}

#[test]
fn test_narrowing_typeof_string() {
    let res = resolve(
        r#"
        function foo(x: any) {
            if (typeof x === "string") {
                console.log(x);
            }
        }
        "#,
    );
    let has_string_narrowing = res
        .narrowing_events
        .iter()
        .any(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::String));
    assert!(
        has_string_narrowing,
        "typeof guard should create String narrowing event"
    );
}

#[test]
fn test_narrowing_instanceof() {
    let res = resolve(
        r#"
        function foo(x: any) {
            if (x instanceof Error) {
                console.log(x);
            }
        }
        "#,
    );
    let has_error_narrowing = res.narrowing_events.iter().any(|e| {
        e.var_name == "x"
            && matches!(&e.narrowed_type, RustType::Named { name, .. } if name == "Error")
    });
    assert!(
        has_error_narrowing,
        "instanceof guard should create Error narrowing event"
    );
}

#[test]
fn test_narrowing_null_check() {
    let res = resolve(
        r#"
        function foo(x: string | null) {
            if (x !== null) {
                console.log(x);
            }
        }
        "#,
    );
    let has_non_null_narrowing = res
        .narrowing_events
        .iter()
        .any(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::String));
    assert!(
        has_non_null_narrowing,
        "null check should narrow Option<String> to String"
    );
}

#[test]
fn test_unknown_expr() {
    let res = resolve("const x = unknownFunc();");
    let has_unknown = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Unknown));
    assert!(has_unknown, "unknown function call should be Unknown");
}

#[test]
fn test_binary_add_string_context() {
    let res = resolve(r#"const x = "hello" + " world";"#);
    let has_string = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Known(RustType::String)));
    assert!(has_string, "string + string should resolve to String");
}

#[test]
fn test_resolve_member_access_field() {
    let res = resolve(
        r#"
        interface Foo { name: string; }
        function bar(f: Foo) { return f.name; }
        "#,
    );
    let has_string = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Known(RustType::String)));
    assert!(has_string, "f.name should resolve to String");
}

#[test]
fn test_expected_type_call_arg() {
    let res = resolve(
        r#"
        function greet(name: string): void {}
        greet("hello");
        "#,
    );
    // The argument "hello" at the call site should have expected_type = String
    let has_string_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::String));
    assert!(
        has_string_expected,
        "call argument should have expected type String from parameter"
    );
}

#[test]
fn test_mutability_let_without_assign() {
    let res = resolve("let z = 1;");
    // z is declared with let but never reassigned
    let all_immutable = res.var_mutability.values().all(|&m| !m);
    assert!(
        all_immutable,
        "let without reassignment should not be mutable"
    );
}

#[test]
fn test_synthetic_registration_in_body() {
    let files = parse_files(vec![(
        PathBuf::from("test.ts"),
        "function foo() { const x: string | number = 42; }".to_string(),
    )])
    .unwrap();
    let file = &files.files[0];
    let reg = build_registry(&file.module);
    let mut synthetic = SyntheticTypeRegistry::new();

    let mut resolver = TypeResolver::new(&reg, &mut synthetic);
    let _res = resolver.resolve_file(file);
    // The union type string | number in the body should have registered a synthetic enum
    assert!(
        !synthetic.all_items().is_empty(),
        "body union type annotation should register synthetic enum"
    );
}

#[test]
fn test_resolve_arrow_body() {
    let res = resolve(
        r#"
        const f = (x: string) => x.length;
        "#,
    );
    // Arrow body should be walked; x.length should be in expr_types
    let has_f64 = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Known(RustType::F64)));
    assert!(
        has_f64,
        "arrow body expression x.length should resolve to f64"
    );
}

#[test]
fn test_resolve_arrow_param_type() {
    let res = resolve(
        r#"
        const greet = (name: string) => name;
        "#,
    );
    // name should be resolved to String inside the arrow body
    let has_string = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Known(RustType::String)));
    assert!(
        has_string,
        "arrow param name should resolve to String in body"
    );
}

#[test]
fn test_resolve_array_literal_numbers() {
    let res = resolve("const arr = [1, 2, 3];");
    let has_vec_f64 = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Known(RustType::Vec(inner)) if matches!(inner.as_ref(), RustType::F64)));
    assert!(has_vec_f64, "[1, 2, 3] should resolve to Vec<f64>");
}

#[test]
fn test_resolve_array_literal_strings() {
    let res = resolve(r#"const arr = ["a", "b"];"#);
    let has_vec_string = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Known(RustType::Vec(inner)) if matches!(inner.as_ref(), RustType::String)));
    assert!(
        has_vec_string,
        r#"["a", "b"] should resolve to Vec<String>"#
    );
}

#[test]
fn test_resolve_array_literal_empty() {
    let res = resolve("const arr = [];");
    let has_unknown = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Unknown));
    assert!(has_unknown, "[] should resolve to Unknown");
}

#[test]
fn test_resolve_class_method_body() {
    let res = resolve(
        r#"
        class Foo {
            bar(x: number): string {
                return "hello";
            }
        }
        "#,
    );
    // "hello" inside the class method body should be resolved
    let has_string = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Known(RustType::String)));
    assert!(has_string, "class method body should be walked");
}

#[test]
fn test_resolve_class_constructor() {
    let res = resolve(
        r#"
        class Foo {
            constructor(x: number) {
                const y = x;
            }
        }
        "#,
    );
    // x inside constructor should be f64
    let has_f64 = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Known(RustType::F64)));
    assert!(
        has_f64,
        "constructor body should be walked and params registered"
    );
}

#[test]
fn test_resolve_method_call_return_type() {
    // Register a type with a method in TypeRegistry
    let files = parse_files(vec![(
        PathBuf::from("test.ts"),
        r#"
        interface Greeter { greet(): string; }
        function use_greeter(g: Greeter) { return g.greet(); }
        "#
        .to_string(),
    )])
    .unwrap();
    let file = &files.files[0];
    let reg = build_registry(&file.module);
    let mut synthetic = SyntheticTypeRegistry::new();

    let mut resolver = TypeResolver::new(&reg, &mut synthetic);
    let res = resolver.resolve_file(file);

    // g.greet() should resolve to String (from Greeter.greet return type)
    let has_string = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Known(RustType::String)));
    assert!(has_string, "method call g.greet() should resolve to String");
}

#[test]
fn test_resolve_string_length() {
    let res = resolve(
        r#"
        function foo(s: string) { return s.length; }
        "#,
    );
    let has_f64 = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Known(RustType::F64)));
    assert!(has_f64, "s.length on String should resolve to f64");
}

#[test]
fn test_resolve_object_literal_values() {
    let res = resolve(r#"const obj = { x: 42, y: "hello" };"#);
    let has_f64 = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Known(RustType::F64)));
    let has_string = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Known(RustType::String)));
    assert!(has_f64, "object literal value 42 should be resolved");
    assert!(
        has_string,
        "object literal value 'hello' should be resolved"
    );
}

#[test]
fn test_resolve_destructuring_object() {
    let res = resolve(
        r#"
        const obj = { x: 1, y: 2 };
        const { x, y } = obj;
        "#,
    );
    // x and y should be registered as variables (Unknown type since no annotation)
    // The key test is that this doesn't crash
    assert!(
        !res.var_mutability.is_empty(),
        "destructured variables should be registered"
    );
}

#[test]
fn test_resolve_throw_stmt() {
    let res = resolve(
        r#"
        function foo() {
            throw new Error("fail");
        }
        "#,
    );
    // The throw expression should be walked
    assert!(
        !res.expr_types.is_empty(),
        "throw expression should be resolved"
    );
}

// ── resolve_member_type: synthetic inline struct field access ──

#[test]
fn test_member_access_on_inline_object_type_parameter() {
    // { verification?: string } becomes a synthetic struct _TypeLitN.
    // options.verification should resolve to Option<String> via SyntheticTypeRegistry.
    let res = resolve(
        r#"
        function f(options: { verification?: string; alg?: string }) {
            const v = options.verification;
        }
        "#,
    );
    let has_option_string = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Known(RustType::Option(inner)) if matches!(inner.as_ref(), RustType::String)));
    assert!(
        has_option_string,
        "options.verification on inline type should resolve to Option<String>"
    );
}

#[test]
fn test_member_access_on_inline_object_type_required_field() {
    // Required field (not optional) should resolve to the direct type.
    let res = resolve(
        r#"
        function f(opts: { name: string; count: number }) {
            const n = opts.name;
            const c = opts.count;
        }
        "#,
    );
    let has_string = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Known(RustType::String)));
    assert!(
        has_string,
        "opts.name on inline type should resolve to String"
    );
    let has_f64 = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Known(RustType::F64)));
    assert!(has_f64, "opts.count on inline type should resolve to F64");
}

// ── resolve_member_type: type parameter constraint field access ──

#[test]
fn test_member_access_on_type_param_with_constraint() {
    // E extends Env: env.bindings should resolve through the constraint to Env's fields.
    let mut reg = TypeRegistry::new();
    reg.register(
        "Env".to_string(),
        TypeDef::new_struct(
            vec![
                ("bindings".to_string(), RustType::Any).into(),
                ("variables".to_string(), RustType::Any).into(),
            ],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg(
        r#"
        function f<E extends Env>(env: E) {
            const b = env.bindings;
        }
        "#,
        &reg,
    );
    let has_any = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Known(RustType::Any)));
    assert!(
        has_any,
        "env.bindings on constrained type param should resolve through constraint"
    );
}

#[test]
fn test_member_access_on_type_param_nonexistent_field_returns_unknown() {
    let mut reg = TypeRegistry::new();
    reg.register(
        "Env".to_string(),
        TypeDef::new_struct(
            vec![("bindings".to_string(), RustType::String).into()],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg(
        r#"
        function f<E extends Env>(env: E) {
            const x = env.nonexistent;
        }
        "#,
        &reg,
    );
    // env.nonexistent should resolve to Unknown (field doesn't exist on Env).
    // Verify by checking that the MemberExpr produces Unknown.
    let has_unknown = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Unknown));
    assert!(
        has_unknown,
        "nonexistent field on constrained type param should resolve to Unknown"
    );
    // Also verify no String leaked from Env.bindings (which was NOT accessed).
    let has_string = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Known(RustType::String)));
    assert!(
        !has_string,
        "should not have String — env.nonexistent is not env.bindings"
    );
}

#[test]
fn test_member_access_on_chained_type_param_constraints() {
    // T extends Base, Base extends Root: obj.field should resolve through 2 levels.
    let mut reg = TypeRegistry::new();
    reg.register(
        "Root".to_string(),
        TypeDef::new_struct(
            vec![("field".to_string(), RustType::F64).into()],
            Default::default(),
            vec![],
        ),
    );
    // Base extends Root — Base is in TypeRegistry as a struct inheriting Root's fields
    reg.register(
        "Base".to_string(),
        TypeDef::new_struct(
            vec![("field".to_string(), RustType::F64).into()],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg(
        r#"
        function f<T extends Base>(obj: T) {
            const v = obj.field;
        }
        "#,
        &reg,
    );
    let has_f64 = res
        .expr_types
        .values()
        .any(|t| matches!(t, ResolvedType::Known(RustType::F64)));
    assert!(
        has_f64,
        "obj.field on chained constraint (T → Base → fields) should resolve to F64"
    );
}

// ── resolve_type_params_in_type unit tests ──

#[test]
fn test_resolve_type_params_in_type_bare_param() {
    // T with constraint Named("Options") → Named("Options")
    let mut reg = TypeRegistry::new();
    reg.register(
        "Options".to_string(),
        TypeDef::new_struct(vec![], Default::default(), vec![]),
    );
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut resolver = TypeResolver::new(&reg, &mut synthetic);
    resolver.type_param_constraints.insert(
        "T".to_string(),
        RustType::Named {
            name: "Options".to_string(),
            type_args: vec![],
        },
    );

    let input = RustType::Named {
        name: "T".to_string(),
        type_args: vec![],
    };
    let result = resolver.resolve_type_params_in_type(&input);
    assert_eq!(
        result,
        RustType::Named {
            name: "Options".to_string(),
            type_args: vec![],
        }
    );
}

#[test]
fn test_resolve_type_params_in_type_in_type_args() {
    // Container<E> where E → Env → Container<Env>
    let reg = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut resolver = TypeResolver::new(&reg, &mut synthetic);
    resolver.type_param_constraints.insert(
        "E".to_string(),
        RustType::Named {
            name: "Env".to_string(),
            type_args: vec![],
        },
    );

    let input = RustType::Named {
        name: "Container".to_string(),
        type_args: vec![RustType::Named {
            name: "E".to_string(),
            type_args: vec![],
        }],
    };
    let result = resolver.resolve_type_params_in_type(&input);
    assert_eq!(
        result,
        RustType::Named {
            name: "Container".to_string(),
            type_args: vec![RustType::Named {
                name: "Env".to_string(),
                type_args: vec![],
            }],
        }
    );
}

#[test]
fn test_resolve_type_params_in_type_no_constraint() {
    // T without constraint → unchanged
    let reg = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();
    let resolver = TypeResolver::new(&reg, &mut synthetic);

    let input = RustType::Named {
        name: "T".to_string(),
        type_args: vec![],
    };
    let result = resolver.resolve_type_params_in_type(&input);
    assert_eq!(
        result, input,
        "unconstrained type param should remain unchanged"
    );
}

#[test]
fn test_resolve_type_params_in_type_option() {
    // Option<T> where T → String → Option<String>
    let reg = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut resolver = TypeResolver::new(&reg, &mut synthetic);
    resolver
        .type_param_constraints
        .insert("T".to_string(), RustType::String);

    let input = RustType::Option(Box::new(RustType::Named {
        name: "T".to_string(),
        type_args: vec![],
    }));
    let result = resolver.resolve_type_params_in_type(&input);
    assert_eq!(result, RustType::Option(Box::new(RustType::String)));
}

#[test]
fn test_resolve_type_params_in_type_non_named_unchanged() {
    // Primitive types like String, F64, Bool should pass through unchanged
    let reg = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();
    let resolver = TypeResolver::new(&reg, &mut synthetic);

    assert_eq!(
        resolver.resolve_type_params_in_type(&RustType::String),
        RustType::String
    );
    assert_eq!(
        resolver.resolve_type_params_in_type(&RustType::F64),
        RustType::F64
    );
    assert_eq!(
        resolver.resolve_type_params_in_type(&RustType::Bool),
        RustType::Bool
    );
    assert_eq!(
        resolver.resolve_type_params_in_type(&RustType::Any),
        RustType::Any
    );
}
