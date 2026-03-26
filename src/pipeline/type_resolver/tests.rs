use super::*;
use crate::pipeline::{parse_files, SyntheticTypeRegistry};
use crate::registry::build_registry;
use std::path::PathBuf;

use crate::registry::TypeDef;

fn resolve(source: &str) -> FileTypeResolution {
    let files = parse_files(vec![(PathBuf::from("test.ts"), source.to_string())]).unwrap();
    let file = &files.files[0];
    let reg = build_registry(&file.module);
    let mut synthetic = SyntheticTypeRegistry::new();

    let mut resolver = TypeResolver::new(&reg, &mut synthetic);
    resolver.resolve_file(file)
}

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

// --- Phase 1: propagate_expected tests ---

/// Helper: resolve with a pre-built registry for struct/enum definitions.
fn resolve_with_reg(source: &str, reg: &TypeRegistry) -> FileTypeResolution {
    let files = parse_files(vec![(PathBuf::from("test.ts"), source.to_string())]).unwrap();
    let file = &files.files[0];
    let mut synthetic = SyntheticTypeRegistry::new();

    let mut resolver = TypeResolver::new(reg, &mut synthetic);
    resolver.resolve_file(file)
}

#[test]
fn test_propagate_expected_var_decl_object_literal_sets_struct_name() {
    // 1-2: const p: Point = { x: 1, y: 2 } → object literal gets Named("Point")
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg("const p: Point = { x: 1, y: 2 };", &reg);

    // The object literal span should have expected = Named("Point")
    let has_point_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "Point"));
    assert!(
        has_point_expected,
        "object literal should have Named(\"Point\") as expected type"
    );

    // Field value spans should also have expected types (propagated from struct fields)
    // Count expected_types entries — should be more than just the top-level initializer
    assert!(
        res.expected_types.len() >= 3,
        "expected at least 3 entries (initializer + 2 field values), got {}",
        res.expected_types.len()
    );
}

#[test]
fn test_propagate_expected_var_decl_array_vec_sets_element_type() {
    // VarDecl + array literal propagation: const a: number[] = [1, 2] → each element gets F64
    let res = resolve("const a: number[] = [1, 2];");

    let f64_expected_count = res
        .expected_types
        .values()
        .filter(|t| matches!(t, RustType::F64))
        .count();
    // Each array element should get F64 as expected type
    assert!(
        f64_expected_count >= 2,
        "each array element should have F64 as expected, got {} F64 entries",
        f64_expected_count
    );
}

#[test]
fn test_propagate_expected_var_decl_tuple_sets_positional_types() {
    // VarDecl + tuple propagation: const t: [string, number] = ["a", 1]
    let res = resolve(r#"const t: [string, number] = ["a", 1];"#);

    let has_string_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::String));
    let has_f64_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::F64));
    assert!(
        has_string_expected,
        "first tuple element should expect String"
    );
    assert!(has_f64_expected, "second tuple element should expect F64");
}

#[test]
fn test_propagate_expected_return_object_sets_field_types() {
    // 1-3: function f(): Point { return { x: 1, y: 2 }; }
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg("function f(): Point { return { x: 1, y: 2 }; }", &reg);

    // Return value object literal should have Named("Point") as expected
    let has_point_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "Point"));
    assert!(
        has_point_expected,
        "return object literal should have Named(\"Point\") as expected"
    );
}

#[test]
fn test_propagate_expected_call_arg_from_registry_fn() {
    // 1-4: Registry function の引数 expected が propagate される
    let mut reg = TypeRegistry::new();
    reg.register(
        "greet".to_string(),
        crate::registry::TypeDef::Function {
            params: vec![("name".to_string(), RustType::String)],
            return_type: None,
            has_rest: false,
        },
    );

    let res = resolve_with_reg(r#"greet("hello");"#, &reg);

    let has_string_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::String));
    assert!(
        has_string_expected,
        "call argument should have String as expected from registry function params"
    );
}

#[test]
fn test_propagate_expected_call_arg_from_scope_fn_type() {
    // 1-4a: scope 内の Fn 型変数から引数の expected を設定
    let res = resolve(
        r#"
        function callHandler(handler: (name: string) => void) {
            handler("hello");
        }
        "#,
    );

    let has_string_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::String));
    assert!(
        has_string_expected,
        "call argument should have String as expected from Fn type in scope"
    );
}

#[test]
fn test_propagate_expected_switch_case_gets_discriminant_type() {
    // 1-5: switch(dir) { case "up": } where dir: Direction
    let mut reg = TypeRegistry::new();
    reg.register(
        "Direction".to_string(),
        crate::registry::TypeDef::Enum {
            type_params: vec![],
            variants: vec![],
            tag_field: None,
            variant_fields: Default::default(),
            string_values: Default::default(),
        },
    );

    let res = resolve_with_reg(
        r#"
        function f(dir: Direction) {
            switch (dir) {
                case "up":
                    break;
            }
        }
        "#,
        &reg,
    );

    let has_direction_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "Direction"));
    assert!(
        has_direction_expected,
        "switch case value should have Direction as expected type"
    );
}

#[test]
fn test_propagate_expected_assign_rhs_gets_lhs_type() {
    // 1-6: let x: Point; x = { x: 1, y: 2 }
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg(
        r#"
        function f() {
            let x: Point = { x: 0, y: 0 };
            x = { x: 1, y: 2 };
        }
        "#,
        &reg,
    );

    // Count Named("Point") expected entries — should include both var decl init AND assignment RHS
    let point_expected_count = res
        .expected_types
        .values()
        .filter(|t| matches!(t, RustType::Named { name, .. } if name == "Point"))
        .count();
    assert!(
        point_expected_count >= 2,
        "both var decl init and assignment RHS should have Named(\"Point\") as expected, got {}",
        point_expected_count
    );
}

#[test]
fn test_propagate_expected_nullish_coalescing_rhs_gets_inner_type() {
    // 1-7: opt ?? "default" where opt: string | null (Option<String>)
    let res = resolve(
        r#"
        function f(opt: string | null) {
            const result = opt ?? "default";
        }
        "#,
    );

    // The RHS "default" should have String as expected (inner of Option<String>)
    let has_string_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::String));
    assert!(
        has_string_expected,
        "nullish coalescing RHS should have String as expected (inner of Option<String>)"
    );
}

#[test]
fn test_propagate_expected_ternary_branches_get_expected() {
    // 1-10: const s: string = c ? "a" : "b" → both branches get String
    let res = resolve(r#"const s: string = true ? "a" : "b";"#);

    // Count String expected entries
    let string_expected_count = res
        .expected_types
        .values()
        .filter(|t| matches!(t, RustType::String))
        .count();
    // At minimum: "a" and "b" should both have String expected
    assert!(
        string_expected_count >= 2,
        "both ternary branches should have String expected, got {}",
        string_expected_count
    );
}

#[test]
fn test_propagate_expected_class_prop_initializer_gets_annotation_type() {
    // 1-8: class C { static x: string = "hi" }
    let res = resolve(
        r#"
        class C {
            static x: string = "hello";
        }
        "#,
    );

    let has_string_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::String));
    assert!(
        has_string_expected,
        "class property initializer should have String as expected from annotation"
    );
}

#[test]
fn test_propagate_expected_du_object_lit_fields() {
    let res = resolve(
        r#"
        type Shape = { kind: "circle"; radius: number } | { kind: "square"; side: number };
        const s: Shape = { kind: "circle", radius: 42 };
        "#,
    );

    let has_f64_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::F64));
    assert!(
        has_f64_expected,
        "DU variant field 'radius' should have expected type f64"
    );
}

#[test]
fn test_propagate_expected_hashmap_value() {
    let res = resolve(
        r#"
        const m: Record<string, number> = { [key]: 42 };
        "#,
    );

    let has_f64_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::F64));
    assert!(
        has_f64_expected,
        "HashMap value should have expected type f64"
    );
}

#[test]
fn test_propagate_expected_arrow_expr_body() {
    let res = resolve(
        r#"
        const f = (): string => "hello";
        "#,
    );

    let has_string_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::String));
    assert!(
        has_string_expected,
        "arrow expression body should have expected type String from return annotation"
    );
}

#[test]
fn test_propagate_expected_rest_param_args() {
    let res = resolve(
        r#"
        function foo(a: number, ...rest: string[]): void {}
        foo(1, "hello", "world");
        "#,
    );

    let string_expected_count = res
        .expected_types
        .values()
        .filter(|t| matches!(t, RustType::String))
        .count();
    assert!(
        string_expected_count >= 2,
        "rest args 'hello' and 'world' should have expected type String, got {string_expected_count}"
    );
}

#[test]
fn test_propagate_expected_opt_chain_method_args() {
    let res = resolve(
        r#"
        interface Obj {
            greet(name: string): void;
        }
        declare const obj: Obj | undefined;
        obj?.greet("hello");
        "#,
    );

    let has_string_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::String));
    assert!(
        has_string_expected,
        "opt chain method arg should have expected type String"
    );
}

#[test]
fn test_resolve_arrow_return_type_from_fn_type_alias() {
    // Variable type annotation with function type alias should propagate
    // return type to arrow body, enabling nested object literal struct resolution
    let res = resolve(
        r#"
        interface ConnInfo { remote: RemoteInfo; }
        interface RemoteInfo { address: string; }
        type GetConnInfo = (host: string) => ConnInfo;
        const getConnInfo: GetConnInfo = (host: string) => ({
            remote: { address: host },
        });
        "#,
    );

    // The nested object literal { address: host } should have expected type
    // Named("RemoteInfo") — propagated through: GetConnInfo → ConnInfo → remote field
    let has_remote_info_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "RemoteInfo"));
    assert!(
        has_remote_info_expected,
        "nested object literal should have expected type RemoteInfo from fn type alias return type"
    );
}

#[test]
fn test_resolve_arrow_explicit_annotation_takes_priority_over_expected() {
    // Arrow's own return type annotation should take priority over expected type
    let res = resolve(
        r#"
        const f: (x: number) => string = (x: number): number => 42;
        "#,
    );

    // The return value `42` should have expected type f64 (from arrow's own annotation),
    // not String (from variable annotation)
    let has_f64_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::F64));
    assert!(
        has_f64_expected,
        "arrow's own return annotation (number) should take priority"
    );
}

#[test]
fn test_cond_expr_test_subexpressions_resolved() {
    // TypeResolver should resolve CondExpr's test sub-expressions
    // so that variable types in conditions are available in expr_types.
    let source = r#"
        function f(x: string | null): string {
            return x !== null ? x : "";
        }
    "#;
    let files = parse_files(vec![(PathBuf::from("test.ts"), source.to_string())]).unwrap();
    let file = &files.files[0];
    let reg = build_registry(&file.module);
    let mut synthetic = SyntheticTypeRegistry::new();

    let mut resolver = TypeResolver::new(&reg, &mut synthetic);
    let res = resolver.resolve_file(file);

    // Find x's Ident span in the condition `x !== null`
    let fn_decl = match &file.module.body[0] {
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Fn(
            fd,
        ))) => fd,
        _ => panic!("expected fn decl"),
    };
    let return_stmt = &fn_decl.function.body.as_ref().unwrap().stmts[0];
    let cond_expr = match return_stmt {
        swc_ecma_ast::Stmt::Return(ret) => match ret.arg.as_deref() {
            Some(swc_ecma_ast::Expr::Cond(cond)) => cond,
            _ => panic!("expected cond expr"),
        },
        _ => panic!("expected return stmt"),
    };
    // test is `x !== null`, left is `x`
    let x_ident = match cond_expr.test.as_ref() {
        swc_ecma_ast::Expr::Bin(bin) => match bin.left.as_ref() {
            swc_ecma_ast::Expr::Ident(ident) => ident,
            _ => panic!("expected ident"),
        },
        _ => panic!("expected bin expr"),
    };
    let x_span = Span::from_swc(x_ident.span);

    // x in the condition should have its type resolved
    let x_type = res.expr_type(x_span);
    assert!(
        matches!(x_type, ResolvedType::Known(RustType::Option(_))),
        "x in condition should be resolved to Option<String>, got: {:?}",
        x_type
    );
}

// --- Fn type registration on variable Ident ---

#[test]
fn test_fn_type_registered_on_variable_ident_span() {
    // const add = (x: number, y: number): number => x + y;
    // get_expr_type for the "add" Ident should return Fn type
    let source = r#"const add = (x: number, y: number): number => x + y;"#;
    let res = resolve(source);

    // Find the "add" ident span — it's the variable declaration name
    // The source starts at position 0. "const " = 6 chars, "add" starts at 6
    // But SWC byte positions may differ — let's find it by looking for Fn type entries
    let fn_type_entries: Vec<_> = res
        .expr_types
        .iter()
        .filter(|(_, ty)| matches!(ty, ResolvedType::Known(RustType::Fn { .. })))
        .collect();
    assert!(
        fn_type_entries.len() >= 2,
        "should have Fn type for both the arrow expr AND the variable ident, got {} entries: {:?}",
        fn_type_entries.len(),
        fn_type_entries
    );
}

#[test]
fn test_fn_type_not_registered_for_non_fn_var() {
    // const x: number = 42; — should not register Fn type on variable
    let source = r#"const x: number = 42;"#;
    let res = resolve(source);

    let fn_type_entries: Vec<_> = res
        .expr_types
        .iter()
        .filter(|(_, ty)| matches!(ty, ResolvedType::Known(RustType::Fn { .. })))
        .collect();
    assert!(
        fn_type_entries.is_empty(),
        "should not have Fn type entries for non-fn var, got: {:?}",
        fn_type_entries
    );
}

// --- DU field binding detection ---

fn build_shape_registry() -> TypeRegistry {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("circle".to_string(), "Circle".to_string());
    string_values.insert("square".to_string(), "Square".to_string());
    let mut variant_fields = std::collections::HashMap::new();
    variant_fields.insert(
        "Circle".to_string(),
        vec![("radius".to_string(), RustType::F64)],
    );
    variant_fields.insert(
        "Square".to_string(),
        vec![
            ("width".to_string(), RustType::F64),
            ("height".to_string(), RustType::F64),
        ],
    );
    reg.register(
        "Shape".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Circle".to_string(), "Square".to_string()],
            string_values,
            tag_field: Some("kind".to_string()),
            variant_fields,
        },
    );
    reg
}

#[test]
fn test_du_field_binding_detected_in_switch_case() {
    let source = r#"
function describe(s: Shape): number {
switch (s.kind) {
    case "circle":
        return s.radius;
    case "square":
        return s.width;
}
}
"#;
    let reg = build_shape_registry();
    let res = resolve_with_reg(source, &reg);

    // Should have bindings for "radius" and "width"
    assert!(
        !res.du_field_bindings.is_empty(),
        "should detect DU field bindings, got: {:?}",
        res.du_field_bindings
    );

    let radius_bindings: Vec<_> = res
        .du_field_bindings
        .iter()
        .filter(|b| b.var_name == "radius")
        .collect();
    assert_eq!(
        radius_bindings.len(),
        1,
        "should have exactly one 'radius' binding"
    );

    let width_bindings: Vec<_> = res
        .du_field_bindings
        .iter()
        .filter(|b| b.var_name == "width")
        .collect();
    assert_eq!(
        width_bindings.len(),
        1,
        "should have exactly one 'width' binding"
    );
}

#[test]
fn test_du_field_binding_outside_scope_returns_false() {
    let source = r#"
function describe(s: Shape): number {
switch (s.kind) {
    case "circle":
        return s.radius;
    case "square":
        return 0;
}
}
"#;
    let reg = build_shape_registry();
    let res = resolve_with_reg(source, &reg);

    let radius_binding = res
        .du_field_bindings
        .iter()
        .find(|b| b.var_name == "radius")
        .expect("should have radius binding");

    // Inside scope: true
    assert!(res.is_du_field_binding("radius", radius_binding.scope_start));
    assert!(res.is_du_field_binding("radius", radius_binding.scope_start + 1));

    // Outside scope: false
    assert!(!res.is_du_field_binding("radius", radius_binding.scope_end));
    assert!(!res.is_du_field_binding("radius", 0));

    // Non-bound field: false
    assert!(!res.is_du_field_binding("width", radius_binding.scope_start));
}

fn resolve_with_reg_and_synthetic(
    source: &str,
    reg: &TypeRegistry,
) -> (FileTypeResolution, SyntheticTypeRegistry) {
    let files = parse_files(vec![(PathBuf::from("test.ts"), source.to_string())]).unwrap();
    let file = &files.files[0];
    let mut synthetic = SyntheticTypeRegistry::new();

    let mut resolver = TypeResolver::new(reg, &mut synthetic);
    let result = resolver.resolve_file(file);
    (result, synthetic)
}

#[test]
fn test_resolve_spread_same_type_uses_source_type() {
    // { ...defaults, ...options } where both are CORSOptions → CORSOptions
    let mut reg = TypeRegistry::new();
    reg.register(
        "CORSOptions".to_string(),
        TypeDef::new_struct(
            vec![
                ("origin".to_string(), RustType::String),
                ("methods".to_string(), RustType::String),
            ],
            Default::default(),
            vec![],
        ),
    );
    let (res, _) = resolve_with_reg_and_synthetic(
        r#"
        function cors(defaults: CORSOptions, options: CORSOptions) {
            const opts = { ...defaults, ...options };
        }
        "#,
        &reg,
    );
    let has_cors_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "CORSOptions"));
    assert!(
        has_cors_expected,
        "spread of same-type sources should use source type as expected type"
    );
}

#[test]
fn test_resolve_spread_with_extra_field_creates_anon_struct() {
    // { ...base, extra: 1 } where base is Point → anonymous struct with merged fields
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
    let (res, synthetic) = resolve_with_reg_and_synthetic(
        r#"
        function make(base: Point) {
            const extended = { ...base, z: 1 };
        }
        "#,
        &reg,
    );
    // Should create an anonymous struct with x, y (from Point) + z (explicit)
    let has_anon = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name.starts_with("_TypeLit")));
    assert!(
        has_anon,
        "spread with extra fields should create anonymous struct"
    );
    // Verify the struct has 3 fields
    let struct_items: Vec<_> = synthetic
        .all_items()
        .iter()
        .filter_map(|item| match item {
            crate::ir::Item::Struct { fields, .. } if fields.len() == 3 => Some(fields),
            _ => None,
        })
        .collect();
    assert!(
        !struct_items.is_empty(),
        "anonymous struct should have 3 fields (x, y from Point + z)"
    );
}

fn resolve_with_synthetic(source: &str) -> (FileTypeResolution, SyntheticTypeRegistry) {
    let files = parse_files(vec![(PathBuf::from("test.ts"), source.to_string())]).unwrap();
    let file = &files.files[0];
    let reg = build_registry(&file.module);
    let mut synthetic = SyntheticTypeRegistry::new();

    let mut resolver = TypeResolver::new(&reg, &mut synthetic);
    let result = resolver.resolve_file(file);
    (result, synthetic)
}

#[test]
fn test_resolve_anon_struct_generated_for_untyped_object_literal() {
    let (res, synthetic) = resolve_with_synthetic("const obj = { x: 1, y: 'hello' };");
    // The object literal should get an expected type (anonymous struct)
    let has_anon_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name.starts_with("_TypeLit")));
    assert!(
        has_anon_expected,
        "untyped object literal should generate anonymous struct as expected type"
    );
    // The synthetic registry should have the anonymous struct registered
    let has_struct = synthetic
        .all_items()
        .iter()
        .any(|item| matches!(item, crate::ir::Item::Struct { name, .. } if name.starts_with("_TypeLit")));
    assert!(
        has_struct,
        "synthetic registry should contain the anonymous struct"
    );
}

#[test]
fn test_resolve_anon_struct_dedup_same_fields() {
    let (_, synthetic) = resolve_with_synthetic(
        r#"
        const a = { x: 1, y: 2 };
        const b = { x: 3, y: 4 };
        "#,
    );
    // Both objects have the same field structure (x: f64, y: f64)
    // → should share one anonymous struct, not two
    let struct_count = synthetic
        .all_items()
        .iter()
        .filter(|item| matches!(item, crate::ir::Item::Struct { name, .. } if name.starts_with("_TypeLit")))
        .count();
    assert_eq!(
        struct_count, 1,
        "same field structure should be deduped to one anonymous struct"
    );
}

#[test]
fn test_resolve_anon_struct_nested_object_literal() {
    let (res, synthetic) = resolve_with_synthetic("const obj = { inner: { a: 1 } };");
    // Both the outer and inner object should get anonymous struct expected types
    let anon_count = res
        .expected_types
        .values()
        .filter(|t| matches!(t, RustType::Named { name, .. } if name.starts_with("_TypeLit")))
        .count();
    assert!(
        anon_count >= 2,
        "nested objects should each get an anonymous struct expected type, got {anon_count}"
    );
    let struct_count = synthetic
        .all_items()
        .iter()
        .filter(|item| matches!(item, crate::ir::Item::Struct { .. }))
        .count();
    assert!(
        struct_count >= 2,
        "should generate at least 2 anonymous structs (outer + inner)"
    );
}

#[test]
fn test_resolve_named_fn_variable_propagates_arg_expected_type() {
    // When a variable has a Named type that resolves to TypeDef::Function,
    // calling it should propagate parameter types as expected types on arguments
    let res = resolve(
        r#"
        type Encoder = { (payload: Record<string, unknown>): string };
        function run(encode: Encoder) {
            encode({ alg: "HS256", typ: "JWT" });
        }
        "#,
    );
    // The object literal argument should have expected type from Encoder's first param
    let has_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "HashMap"));
    assert!(
        has_expected,
        "object literal argument should have expected type from Named fn variable's param type"
    );
}

#[test]
fn test_resolve_call_signature_type_alias_sets_return_expected_type() {
    // Arrow function assigned to a call-signature type alias variable
    // should propagate the return type to the return statement
    let res = resolve(
        r#"
        type Handler = { (c: string): number };
        const handler: Handler = (c) => { return 42; };
        "#,
    );
    let has_f64_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::F64));
    assert!(
        has_f64_expected,
        "return expression should have expected type f64 from call-signature type alias"
    );
}

// ── select_overload tests ─────────────────────────────────────

use crate::registry::MethodSignature;

fn make_sig(param_types: Vec<RustType>, ret: Option<RustType>) -> MethodSignature {
    MethodSignature {
        params: param_types
            .into_iter()
            .enumerate()
            .map(|(i, ty)| (format!("p{i}"), ty))
            .collect(),
        return_type: ret,
    }
}

#[test]
fn test_select_overload_single_sig_returns_it() {
    let sigs = vec![make_sig(vec![], Some(RustType::String))];
    let selected = super::select_overload(&sigs, 0, &[]);
    assert_eq!(selected.return_type, Some(RustType::String));
    assert_eq!(selected.params.len(), 0);
}

#[test]
fn test_select_overload_all_same_return_skips_to_first() {
    let sigs = vec![
        make_sig(vec![], Some(RustType::String)),
        make_sig(vec![RustType::F64], Some(RustType::String)),
        make_sig(vec![RustType::F64, RustType::Bool], Some(RustType::String)),
    ];
    // All return types identical → returns first signature
    let selected = super::select_overload(&sigs, 1, &[Some(RustType::F64)]);
    assert_eq!(selected.return_type, Some(RustType::String));
    // First signature is selected (0-arg)
    assert_eq!(selected.params.len(), 0);
}

#[test]
fn test_select_overload_arg_count_selects_match() {
    let sigs = vec![
        make_sig(vec![], Some(RustType::String)),
        make_sig(vec![RustType::F64], Some(RustType::F64)),
    ];
    // 0 args → sig[0]
    let selected = super::select_overload(&sigs, 0, &[]);
    assert_eq!(selected.return_type, Some(RustType::String));
    assert_eq!(selected.params.len(), 0);
    // 1 arg → sig[1]
    let selected = super::select_overload(&sigs, 1, &[None]);
    assert_eq!(selected.return_type, Some(RustType::F64));
    assert_eq!(selected.params.len(), 1);
}

#[test]
fn test_select_overload_arg_type_selects_compatible() {
    let sigs = vec![
        make_sig(vec![RustType::String], Some(RustType::String)),
        make_sig(vec![RustType::F64], Some(RustType::F64)),
    ];
    // arg_type=F64 → sig[1]
    let selected = super::select_overload(&sigs, 1, &[Some(RustType::F64)]);
    assert_eq!(selected.return_type, Some(RustType::F64));
    assert_eq!(selected.params[0].1, RustType::F64);
}

#[test]
fn test_select_overload_no_match_falls_back_to_first() {
    let sigs = vec![
        make_sig(vec![], Some(RustType::String)),
        make_sig(vec![RustType::F64], Some(RustType::F64)),
    ];
    // 3 args → no match → fallback to first
    let selected = super::select_overload(&sigs, 3, &[None, None, None]);
    assert_eq!(selected.return_type, Some(RustType::String));
    assert_eq!(selected.params.len(), 0);
}

#[test]
fn test_select_overload_arg_types_empty_uses_arg_count_only() {
    let sigs = vec![
        make_sig(vec![RustType::String], Some(RustType::String)),
        make_sig(vec![RustType::F64], Some(RustType::F64)),
    ];
    // Same arg_count, empty arg_types → Stage 4 skipped → first of count-matched (sig[0])
    let selected = super::select_overload(&sigs, 1, &[]);
    assert_eq!(selected.params[0].1, RustType::String);
}

#[test]
fn test_select_overload_params_and_return_from_same_sig() {
    // The core invariant: params and return type must come from the same signature
    let sigs = vec![
        make_sig(vec![RustType::String], Some(RustType::String)),
        make_sig(vec![RustType::F64], Some(RustType::F64)),
    ];
    let selected = super::select_overload(&sigs, 1, &[Some(RustType::F64)]);
    // Both params and return_type should be from sig[1] (F64 variant)
    assert_eq!(selected.params[0].1, RustType::F64);
    assert_eq!(selected.return_type, Some(RustType::F64));
}
