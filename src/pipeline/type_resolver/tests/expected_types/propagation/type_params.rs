use super::*;
use swc_common::Spanned;

#[test]
fn test_fallback_expected_resolves_type_params() {
    // In `options.field || {}`, if options is generic T extends { field: Config },
    // the {} should get Config as expected type, not the type param.
    let res = resolve(
        r#"
        interface Config {
            host: string;
        }
        function f<T extends Config>(opts: T) {
            const result = opts || { host: "default" };
        }
        "#,
    );

    // The fallback {} should have expected type resolved through constraint
    let has_config = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "Config"));
    assert!(
        has_config,
        "fallback || {{}} should have Config expected type from type param constraint"
    );
}

// ── Type parameter constraint resolution — variable declaration ──

#[test]
fn test_var_decl_expected_type_resolves_type_params() {
    let res = resolve(
        r#"
        interface Options {
            debug?: boolean;
        }
        function f<T extends Options>() {
            const x: T = { debug: true } as T;
        }
        "#,
    );

    let has_options = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "Options"));
    assert!(
        has_options,
        "var decl with type param annotation should resolve to constraint type"
    );
}

// ── Type parameter constraint resolution — return statement ──

#[test]
fn test_return_expected_type_resolves_type_params() {
    let res = resolve(
        r#"
        interface Config {
            host: string;
        }
        function f<T extends Config>(): T {
            return { host: "localhost" } as T;
        }
        "#,
    );

    let has_config = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "Config"));
    assert!(
        has_config,
        "return statement should resolve type param T to constraint Config in expected type"
    );
}

// ── Method body preserves class type param constraints ──

#[test]
fn test_call_arg_expected_type_resolves_type_params() {
    // class Container<T extends Options> { add(item: T) {} }
    // container.add({}) → expected type for {} should be Options, not T
    let res = resolve(
        r#"
        interface Options {
            debug?: boolean;
        }
        class Container<T extends Options> {
            add(item: T): void {
                const x = item;
            }
            run(): void {
                this.add({ debug: true });
            }
        }
        "#,
    );

    let has_options = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Named { name, .. } if name == "Options"));
    assert!(
        has_options,
        "call argument expected type should resolve type param T to constraint Options"
    );
}

// ── Object literal field propagation with synthetic types ──

#[test]
fn test_new_expr_explicit_type_args_in_return_type() {
    // `new Container<Config>(value)` should return Named("Container", [Named("Config")])
    // (not Named("Container", []) which is the current buggy behavior).
    let source = r#"
        interface Config { host: string; }
        class Container<T> {
            value: T;
            constructor(value: T) { this.value = value; }
        }
        const c = new Container<Config>({ host: "localhost" });
    "#;
    let res = resolve(source);
    // The `new Container<Config>(...)` expression should have type
    // Named("Container", [Named("Config")])
    let has_container_with_type_args = res.expr_types.values().any(|ty| {
        matches!(ty, ResolvedType::Known(RustType::Named { name, type_args })
            if name == "Container" && !type_args.is_empty())
    });
    assert!(
        has_container_with_type_args,
        "new Container<Config>(...) should return Named('Container', [Config]), not empty type_args"
    );
}

#[test]
fn test_new_expr_explicit_type_args_resolve_constructor_params() {
    // `new Container<Config>(value)` where constructor is `(value: T)`:
    // explicit type arg `Config` should instantiate `T` → `Config`,
    // so the argument `{ host: "localhost" }` gets expected type Config.
    let source = r#"
        interface Config { host: string; }
        class Container<T> {
            value: T;
            constructor(value: T) { this.value = value; }
        }
        const c = new Container<Config>({ host: "localhost" });
    "#;
    let res = resolve(source);
    let has_config_expected = res
        .expected_types
        .values()
        .any(|ty| matches!(ty, RustType::Named { name, .. } if name == "Config"));
    assert!(
        has_config_expected,
        "argument to new Container<Config>(...) should have expected type Config"
    );
}

#[test]
fn test_call_explicit_type_args_resolve_param_expected() {
    // `identity<Config>({ host: "localhost" })` where identity is `function identity<T>(x: T): T`
    // explicit type arg `Config` should instantiate `T` → `Config` for the param.
    let source = r#"
        interface Config { host: string; }
        function identity<T>(x: T): T { return x; }
        const c = identity<Config>({ host: "localhost" });
    "#;
    let res = resolve(source);
    let has_config_expected = res
        .expected_types
        .values()
        .any(|ty| matches!(ty, RustType::Named { name, .. } if name == "Config"));
    assert!(
        has_config_expected,
        "argument to identity<Config>(...) should have expected type Config"
    );
}

#[test]
fn test_call_explicit_type_args_resolve_return_type() {
    // `create<Config>()` where create is `function create<T>(): T`
    // explicit type arg `Config` should resolve return type to Config.
    let source = r#"
        interface Config { host: string; }
        function create<T>(): T { return {} as T; }
        const c: Config = create<Config>();
    "#;
    let res = resolve(source);
    let has_config_return = res.expr_types.values().any(
        |ty| matches!(ty, ResolvedType::Known(RustType::Named { name, .. }) if name == "Config"),
    );
    assert!(
        has_config_return,
        "create<Config>() should return type Config"
    );
}

// --- Type argument inference (I-286c S3) ---

#[test]
fn test_infer_type_arg_from_literal_arg() {
    // `id("hello")` where `function id<T>(x: T): T` should infer T = String,
    // so the call expression's return type is String (not Named("T")).
    let source = r#"
        function id<T>(x: T): T { return x; }
        const result = id("hello");
    "#;
    let files = parse_files(vec![(PathBuf::from("test.ts"), source.to_string())]).unwrap();
    let file = &files.files[0];
    let reg = build_registry(&file.module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut resolver = TypeResolver::new(&reg, &mut synthetic);
    let res = resolver.resolve_file(file);

    // Find the call expression `id("hello")` AST node
    let var_decl = match &file.module.body[1] {
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Var(v))) => v,
        _ => panic!("expected var decl"),
    };
    let init = var_decl.decls[0].init.as_ref().unwrap();
    let call_span = Span::from_swc(init.span());
    let call_type = res
        .expr_types
        .get(&call_span)
        .expect("call expression should have resolved type");
    // The call `id("hello")` should resolve to String, not Named("T")
    assert!(
        matches!(call_type, ResolvedType::Known(RustType::String)),
        "id('hello') should infer T=String and return String, got {:?}",
        call_type
    );
}

#[test]
fn test_constrained_type_param_resolves_to_constraint_in_expected() {
    // `fn<T extends Config>(x: T): T` called with `({ host: "localhost", port: 80 })`:
    // The constraint `Config` should be used as expected type for the argument
    // (separate from inference feedback — this is constraint-based resolution).
    let source = r#"
        interface Config { host: string; port: number; }
        function identity<T extends Config>(x: T): T { return x; }
        identity({ host: "localhost", port: 80 });
    "#;
    let res = resolve(source);
    // The argument should have expected type Config (from T's constraint)
    let has_config_expected = res
        .expected_types
        .values()
        .any(|ty| matches!(ty, RustType::Named { name, .. } if name == "Config"));
    assert!(
        has_config_expected,
        "arg to identity<T extends Config>(...) should have expected type Config from constraint"
    );
}

#[test]
fn test_infer_type_arg_new_expr_from_arg() {
    // `new Box("hello")` where `class Box<T> { constructor(v: T) }` should
    // infer T = String from the argument, so the return type includes type_args.
    let source = r#"
        class Box<T> {
            value: T;
            constructor(v: T) { this.value = v; }
        }
        const b = new Box("hello");
    "#;
    let res = resolve(source);
    // The `new Box("hello")` expression should have type Named("Box", [String])
    let has_box_with_string = res.expr_types.values().any(|ty| {
        matches!(ty, ResolvedType::Known(RustType::Named { name, type_args })
            if name == "Box" && type_args.iter().any(|t| matches!(t, RustType::String)))
    });
    assert!(
        has_box_with_string,
        "new Box('hello') should infer T=String and return Named('Box', [String])"
    );
}

#[test]
fn test_resolve_type_params_terminates_on_unconstrained_param() {
    // Regression test: `resolve_type_params_in_type` must not infinite-loop
    // when called on a return type containing an unconstrained type parameter.
    // e.g., `function id<T>(x: T): T` → return type Named("T") with no constraint.
    // This triggered an infinite loop in directory mode when `resolve_type_params_in_type`
    // was applied to return types in `resolve_call_expr`.
    let source = r#"
        function id<T>(x: T): T { return x; }
        function wrap<A, B>(a: A, b: B): A { return a; }
        const x = id(42);
        const y = wrap("hello", true);
    "#;
    // If this completes without hanging, the test passes
    let _res = resolve(source);
}

#[test]
fn test_resolve_type_params_terminates_on_self_referential_constraint() {
    // Pathological case: a type param whose constraint references itself.
    // This can happen with generic defaults like `<T extends T>` (invalid TS but
    // our constraint map could theoretically contain it via bugs).
    // The depth limit in resolve_type_params_impl should prevent infinite recursion.
    let source = r#"
        function complex<T extends Record<string, T>>(x: T): T { return x; }
        const r = complex({ key: {} });
    "#;
    // Must complete without hanging
    let _res = resolve(source);
}

// ── "::" compound name resolution (I-308) ──

#[test]
fn test_resolve_type_params_compound_name_indexed_access() {
    // E['Bindings'] generates Named("E::Bindings") in the IR.
    // When E extends Env { bindings: Bindings }, resolve_type_params_impl should
    // resolve "E::Bindings" → look up Env.bindings field type.
    let res = resolve(
        r#"
        interface Bindings {
            DB: string;
        }
        interface Env {
            bindings: Bindings;
        }
        function handler<E extends Env>(env: E) {
            const b: E['Bindings'] = { DB: "test" };
        }
        "#,
    );

    // The object literal { DB: "test" } should have some expected type
    // (even if the exact resolution depends on indexed access handling)
    // This test verifies that resolve_type_params_impl doesn't panic on "::" names
    let _expected_types = &res.expected_types;
}

// ── Inference feedback: 2nd pass re-propagation (I-001) ──

#[test]
fn test_infer_type_arg_feedback_to_later_arg() {
    // `fn<T>(x: T, y: T)` called with `(42, someVar)`:
    // T should be inferred as F64 from the first arg (numeric literal),
    // and the second arg should get F64 as its expected type.
    let source = r#"
        function pair<T>(x: T, y: T): T { return x; }
        const a = 42;
        pair(100, a);
    "#;
    let res = resolve(source);

    // The second argument `a` should have expected type F64 (inferred from first arg).
    // `a` is the Ident at the second arg position of the call.
    let has_f64_expected = res
        .expected_types
        .values()
        .any(|ty| matches!(ty, RustType::F64));
    assert!(
        has_f64_expected,
        "second arg to pair<T>(100, a) should have expected type F64 from inferred T=F64"
    );
}

#[test]
fn test_infer_type_arg_feedback_object_literal() {
    // `fn<T>(x: T, y: T)` called with `(configInstance, { host: "test" })`:
    // T should be inferred as Config from the first arg,
    // and the object literal's `host` field should get expected type String.
    let source = r#"
        interface Config { host: string; port: number; }
        function pair<T>(x: T, y: T): T { return x; }
        declare const cfg: Config;
        pair(cfg, { host: "test", port: 80 });
    "#;
    let res = resolve(source);

    // The object literal fields should have expected types from Config's fields.
    // `host` field value ("test") should have expected type String.
    let has_string_field_expected = res
        .expected_types
        .values()
        .any(|ty| matches!(ty, RustType::String));
    assert!(
        has_string_field_expected,
        "object literal field 'host' in pair(cfg, {{...}}) should have expected type String from inferred T=Config"
    );
}

#[test]
fn test_infer_type_arg_feedback_void_return() {
    // `fn<T>(x: T, y: T): void` (return_type = Some(Unit)) called with `(42, someVar)`:
    // Even though return type is void, the 2nd pass should still re-propagate
    // expected types so the second arg gets F64.
    let source = r#"
        function process<T>(x: T, y: T): void {}
        const a = 42;
        process(100, a);
    "#;
    let res = resolve(source);

    let has_f64_expected = res
        .expected_types
        .values()
        .any(|ty| matches!(ty, RustType::F64));
    assert!(
        has_f64_expected,
        "second arg to process<T>(100, a): void should have expected type F64 even with void return"
    );
}

#[test]
fn test_infer_type_arg_feedback_no_return_annotation() {
    // `fn<T>(x: T, y: T)` (no return type annotation → return_type = None):
    // The 2nd pass re-propagation must still run even when return_type is None.
    // This exercises the `None => current_result` branch in `infer_type_args_and_feedback`.
    let source = r#"
        function accept<T>(x: T, y: T) {}
        const a = 42;
        accept(100, a);
    "#;
    let res = resolve(source);

    let has_f64_expected = res
        .expected_types
        .values()
        .any(|ty| matches!(ty, RustType::F64));
    assert!(
        has_f64_expected,
        "second arg to accept<T>(100, a) (no return annotation) should have expected type F64"
    );
}

#[test]
fn test_infer_no_bindings_skips_repropagation() {
    // When all arguments are unknown (unresolvable), no bindings are inferred
    // and expected types remain as unresolved type parameters.
    let source = r#"
        function pair<T>(x: T, y: T): T { return x; }
        declare const a: unknown;
        declare const b: unknown;
        pair(a, b);
    "#;
    let res = resolve(source);

    // No concrete type (String, F64, Bool, etc.) should appear in expected types
    // since both args are unknown → no bindings → no 2nd pass.
    let has_concrete_expected = res.expected_types.values().any(|ty| {
        matches!(
            ty,
            RustType::String | RustType::F64 | RustType::Bool | RustType::Unit
        )
    });
    assert!(
        !has_concrete_expected,
        "with all-unknown args, no concrete expected types should be set"
    );
}

#[test]
fn test_infer_non_ident_callee_skips_inference() {
    // Non-Ident callees (e.g., IIFE) should not trigger type argument inference.
    // The `infer_type_args_and_feedback` guard
    // `let Expr::Ident = callee else { return }` must be exercised.
    // Note: `fn_ref(42)` would be an Ident callee, so we use an IIFE instead.
    let source = r#"
        const result = ((x: number): number => x)(42);
    "#;
    let res = resolve(source);

    // The IIFE callee is Paren(Arrow(...)), not Ident, so infer_type_args_and_feedback
    // returns immediately. Verify no panic and the call resolves without error.
    let has_some_type = res
        .expr_types
        .values()
        .any(|ty| matches!(ty, ResolvedType::Known(RustType::F64)));
    assert!(
        has_some_type,
        "IIFE ((x) => x)(42) should resolve to F64 via arrow return type"
    );
}

#[test]
fn test_infer_type_arg_feedback_with_rest_param() {
    // `fn<T>(first: T, ...rest: T[])` called with `("hello", someVar)`:
    // T should be inferred as String from the first (non-rest) arg,
    // and the rest arg should get String as expected type via 2nd pass rest propagation.
    // Note: rest-only `fn<T>(...args: T[])` cannot infer T because `infer_type_args`
    // zips param_types with arg_types (Vec<T> vs String doesn't unify).
    let source = r#"
        function collect<T>(first: T, ...rest: T[]): T[] { return [first]; }
        const a = "world";
        collect("hello", a);
    "#;
    let res = resolve(source);

    // Count String expected types: both first and rest arg should have String.
    let string_expected_count = res
        .expected_types
        .values()
        .filter(|ty| matches!(ty, RustType::String))
        .count();
    assert!(
        string_expected_count >= 2,
        "both args to collect<T>('hello', a) should have expected type String, found {}",
        string_expected_count
    );
}

#[test]
fn test_infer_partial_type_args_feedback() {
    // `fn<T, U>(x: T, y: U, z: T)` called with `("hello", unknownVar, someVar)`:
    // T should be inferred as String from arg 0, fed back to arg 2.
    // U remains unresolved since arg 1 is unknown.
    let source = r#"
        function mix<T, U>(x: T, y: U, z: T): T { return x; }
        declare const u: unknown;
        const s = "world";
        mix("hello", u, s);
    "#;
    let res = resolve(source);

    // Third arg `s` should have expected type String (from inferred T=String)
    let string_count = res
        .expected_types
        .values()
        .filter(|ty| matches!(ty, RustType::String))
        .count();
    // At least 2 String expected types: arg 0 and arg 2 (both have param type T=String)
    assert!(
        string_count >= 2,
        "args 0 and 2 to mix<T,U>('hello', u, s) should have expected type String, found {}",
        string_count
    );
}
