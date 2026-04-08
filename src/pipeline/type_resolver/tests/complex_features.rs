use super::*;
use swc_common::Spanned;

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
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Fn(fd))) => fd,
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

// --- Spread and anonymous struct tests ---

#[test]
fn test_resolve_spread_same_type_uses_source_type() {
    // { ...defaults, ...options } where both are CORSOptions → CORSOptions
    let mut reg = TypeRegistry::new();
    reg.register(
        "CORSOptions".to_string(),
        TypeDef::new_struct(
            vec![
                ("origin".to_string(), RustType::String).into(),
                ("methods".to_string(), RustType::String).into(),
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
                ("x".to_string(), RustType::F64).into(),
                ("y".to_string(), RustType::F64).into(),
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
    let has_struct = synthetic.all_items().iter().any(
        |item| matches!(item, crate::ir::Item::Struct { name, .. } if name.starts_with("_TypeLit")),
    );
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

#[test]
fn test_select_overload_single_sig_returns_it() {
    let sigs = vec![make_sig(vec![], Some(RustType::String))];
    let selected = crate::registry::select_overload(&sigs, 0, &[]);
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
    let selected = crate::registry::select_overload(&sigs, 1, &[Some(RustType::F64)]);
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
    let selected = crate::registry::select_overload(&sigs, 0, &[]);
    assert_eq!(selected.return_type, Some(RustType::String));
    assert_eq!(selected.params.len(), 0);
    // 1 arg → sig[1]
    let selected = crate::registry::select_overload(&sigs, 1, &[None]);
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
    let selected = crate::registry::select_overload(&sigs, 1, &[Some(RustType::F64)]);
    assert_eq!(selected.return_type, Some(RustType::F64));
    assert_eq!(selected.params[0].ty, RustType::F64);
}

#[test]
fn test_select_overload_no_match_falls_back_to_first() {
    let sigs = vec![
        make_sig(vec![], Some(RustType::String)),
        make_sig(vec![RustType::F64], Some(RustType::F64)),
    ];
    // 3 args → no match → fallback to first
    let selected = crate::registry::select_overload(&sigs, 3, &[None, None, None]);
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
    let selected = crate::registry::select_overload(&sigs, 1, &[]);
    assert_eq!(selected.params[0].ty, RustType::String);
}

#[test]
fn test_select_overload_params_and_return_from_same_sig() {
    // The core invariant: params and return type must come from the same signature
    let sigs = vec![
        make_sig(vec![RustType::String], Some(RustType::String)),
        make_sig(vec![RustType::F64], Some(RustType::F64)),
    ];
    let selected = crate::registry::select_overload(&sigs, 1, &[Some(RustType::F64)]);
    // Both params and return_type should be from sig[1] (F64 variant)
    assert_eq!(selected.params[0].ty, RustType::F64);
    assert_eq!(selected.return_type, Some(RustType::F64));
}

#[test]
fn test_this_resolves_to_class_named_type_in_method() {
    // `this` in a class method should resolve to the class's Named type.
    // `this.field` should resolve to the field's type via TypeRegistry.
    let source = r#"
        class Greeter {
            name: string;
            greet(): string {
                return this.name;
            }
        }
    "#;
    let files = parse_files(vec![(PathBuf::from("test.ts"), source.to_string())]).unwrap();
    let file = &files.files[0];
    let reg = build_registry(&file.module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut resolver = TypeResolver::new(&reg, &mut synthetic);
    let res = resolver.resolve_file(file);

    // Find the class → method → return → this.name member expr
    let class = match &file.module.body[0] {
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Class(c))) => c,
        _ => panic!("expected class decl"),
    };
    let method = match &class.class.body[1] {
        swc_ecma_ast::ClassMember::Method(m) => m,
        _ => panic!("expected method"),
    };
    let body = method.function.body.as_ref().unwrap();
    let ret_stmt = match &body.stmts[0] {
        swc_ecma_ast::Stmt::Return(r) => r,
        _ => panic!("expected return"),
    };
    // return this.name → MemberExpr
    let member = match ret_stmt.arg.as_deref() {
        Some(swc_ecma_ast::Expr::Member(m)) => m,
        _ => panic!("expected member expr"),
    };

    // Verify `this` resolves to Named("Greeter")
    let this_span = Span::from_swc(member.obj.span());
    let this_ty = res
        .expr_types
        .get(&this_span)
        .expect("this should have type");
    assert!(
        matches!(this_ty, ResolvedType::Known(RustType::Named { name, .. }) if name == "Greeter"),
        "this should resolve to Named('Greeter'), got {:?}",
        this_ty
    );

    // Verify `this.name` resolves to String
    let member_span = Span::from_swc(member.span);
    let member_ty = res
        .expr_types
        .get(&member_span)
        .expect("this.name should have type");
    assert!(
        matches!(member_ty, ResolvedType::Known(RustType::String)),
        "this.name should resolve to String, got {:?}",
        member_ty
    );
}

#[test]
fn test_this_resolves_in_constructor() {
    // `this` in a constructor should resolve to the class type.
    let source = r#"
        class Counter {
            count: number;
            constructor() {
                this.count = 0;
            }
        }
    "#;
    let files = parse_files(vec![(PathBuf::from("test.ts"), source.to_string())]).unwrap();
    let file = &files.files[0];
    let reg = build_registry(&file.module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut resolver = TypeResolver::new(&reg, &mut synthetic);
    let res = resolver.resolve_file(file);

    // Find constructor → this.count = 0 → this (the LHS of the assignment)
    let class = match &file.module.body[0] {
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Class(c))) => c,
        _ => panic!("expected class decl"),
    };
    let ctor = match &class.class.body[1] {
        swc_ecma_ast::ClassMember::Constructor(c) => c,
        _ => panic!("expected constructor"),
    };
    let body = ctor.body.as_ref().unwrap();
    let expr_stmt = match &body.stmts[0] {
        swc_ecma_ast::Stmt::Expr(e) => e,
        _ => panic!("expected expr stmt"),
    };
    // this.count = 0 → the RHS `0` should be resolved
    let assign = match expr_stmt.expr.as_ref() {
        swc_ecma_ast::Expr::Assign(a) => a,
        _ => panic!("expected assign"),
    };
    // Verify `0` is resolved as F64
    let rhs_span = Span::from_swc(assign.right.span());
    let rhs_ty = res.expr_types.get(&rhs_span).expect("RHS should have type");
    assert!(
        matches!(rhs_ty, ResolvedType::Known(RustType::F64)),
        "RHS should be F64, got {:?}",
        rhs_ty
    );
}

#[test]
fn test_this_unknown_in_static_method() {
    // `this` in a static method should resolve to Unknown, not the class type.
    let source = r#"
        class Utils {
            name: string;
            static create(): Utils {
                const x = this;
                return x;
            }
        }
    "#;
    let files = parse_files(vec![(PathBuf::from("test.ts"), source.to_string())]).unwrap();
    let file = &files.files[0];
    let reg = build_registry(&file.module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut resolver = TypeResolver::new(&reg, &mut synthetic);
    let res = resolver.resolve_file(file);

    // Find the static method → const x = this → `this` expr
    let class = match &file.module.body[0] {
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Class(c))) => c,
        _ => panic!("expected class decl"),
    };
    let method = match &class.class.body[1] {
        swc_ecma_ast::ClassMember::Method(m) => m,
        _ => panic!("expected method"),
    };
    assert!(method.is_static, "method should be static");
    let body = method.function.body.as_ref().unwrap();
    let var_decl = match &body.stmts[0] {
        swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Var(v)) => v,
        _ => panic!("expected var decl"),
    };
    let init = var_decl.decls[0].init.as_ref().unwrap();
    let this_span = Span::from_swc(init.span());
    let this_ty = res
        .expr_types
        .get(&this_span)
        .expect("this should have type");
    assert!(
        matches!(this_ty, ResolvedType::Unknown),
        "this in static method should be Unknown, got {:?}",
        this_ty
    );
}

#[test]
fn test_this_unknown_outside_class() {
    // `this` outside a class should resolve to Unknown.
    let source = r#"
        function standalone(): void {
            const x = this;
        }
    "#;
    let result = resolve(source);

    // All `this` expressions outside class should be Unknown
    let this_types: Vec<_> = result
        .expr_types
        .values()
        .filter(|ty| matches!(ty, ResolvedType::Unknown))
        .collect();
    assert!(
        !this_types.is_empty(),
        "this outside class should resolve to Unknown"
    );
}

#[test]
fn test_this_in_arrow_function_inside_method() {
    // Arrow functions lexically capture `this` from the enclosing method scope.
    let source = r#"
        class Timer {
            delay: number;
            start(): void {
                const cb = () => { return this.delay; };
            }
        }
    "#;
    let files = parse_files(vec![(PathBuf::from("test.ts"), source.to_string())]).unwrap();
    let file = &files.files[0];
    let reg = build_registry(&file.module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut resolver = TypeResolver::new(&reg, &mut synthetic);
    let res = resolver.resolve_file(file);

    // `this.delay` inside the arrow function should resolve to F64
    // because `this` is lexically captured from the enclosing method
    let has_f64 = res
        .expr_types
        .values()
        .any(|ty| matches!(ty, ResolvedType::Known(RustType::F64)));
    assert!(
        has_f64,
        "this.delay in arrow function should resolve to F64 via lexical this"
    );
}

#[test]
fn test_private_field_access_resolves_type() {
    // Private fields (#field) should be resolved by resolve_member_type,
    // just like public fields (ident).
    let source = r#"
        class Store {
            #count: number;
            getCount(): number {
                return this.#count;
            }
        }
    "#;
    let files = parse_files(vec![(PathBuf::from("test.ts"), source.to_string())]).unwrap();
    let file = &files.files[0];
    let reg = build_registry(&file.module);
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut resolver = TypeResolver::new(&reg, &mut synthetic);
    let res = resolver.resolve_file(file);

    // Find this.#count in getCount() body → return this.#count
    let class = match &file.module.body[0] {
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Class(c))) => c,
        _ => panic!("expected class decl"),
    };
    // body[0] = #count field, body[1] = getCount method
    let method = match &class.class.body[1] {
        swc_ecma_ast::ClassMember::Method(m) => m,
        _ => panic!("expected method"),
    };
    let body = method.function.body.as_ref().unwrap();
    let ret_stmt = match &body.stmts[0] {
        swc_ecma_ast::Stmt::Return(r) => r,
        _ => panic!("expected return"),
    };
    // return this.#count → the member expr this.#count
    let member_expr = ret_stmt.arg.as_ref().unwrap();
    let member_span = Span::from_swc(member_expr.span());
    let member_ty = res.expr_types.get(&member_span);
    assert!(
        matches!(member_ty, Some(ResolvedType::Known(RustType::F64))),
        "this.#count should resolve to F64, got {:?}",
        member_ty
    );
}

#[test]
fn test_private_field_assignment_propagates_expected_type() {
    // When assigning to this.#field = {}, the RHS should get the field's
    // type annotation as expected type.
    let source = r#"
        interface Config { host: string; port: number; }
        class Server {
            #config: Config;
            constructor() {
                this.#config = { host: "localhost", port: 8080 };
            }
        }
    "#;
    let res = resolve(source);
    // The object literal { host: "localhost", port: 8080 } should have
    // expected type Config (Named).
    let has_config_expected = res
        .expected_types
        .values()
        .any(|ty| matches!(ty, RustType::Named { name, .. } if name == "Config"));
    assert!(
        has_config_expected,
        "RHS of this.#config = {{...}} should have expected type Config"
    );
}

#[test]
fn test_hashmap_computed_access_resolves_value_type() {
    // m[key] where m: Record<string, T> (HashMap<String, T>) should
    // resolve to T (the value type).
    let source = r#"
        function getItem(m: Record<string, number>, key: string): number {
            return m[key];
        }
    "#;
    let res = resolve(source);
    // m[key] should resolve to F64 (the value type of Record<string, number>)
    let has_f64_for_computed = res
        .expr_types
        .values()
        .any(|ty| matches!(ty, ResolvedType::Known(RustType::F64)));
    assert!(
        has_f64_for_computed,
        "m[key] on Record<string, number> should resolve to F64"
    );
}

#[test]
fn test_hashmap_computed_assignment_propagates_expected() {
    // m[key] = { ... } where m: Record<string, SomeStruct> should
    // propagate the value type as expected type for the RHS.
    let source = r#"
        interface Entry { name: string; value: number; }
        function setItem(m: Record<string, Entry>, key: string): void {
            m[key] = { name: "test", value: 42 };
        }
    "#;
    let res = resolve(source);
    // The RHS object literal should have expected type Entry
    let has_entry_expected = res
        .expected_types
        .values()
        .any(|ty| matches!(ty, RustType::Named { name, .. } if name == "Entry"));
    assert!(
        has_entry_expected,
        "RHS of m[key] = {{...}} should have expected type Entry"
    );
}

// ── I-383 T2.A-ii: TypeResolver pushes type param scope into synthetic registry ──

/// Class methods that return synthetic union types containing class-level
/// generics must produce a synthetic enum carrying that generic in its
/// `type_params`. Without scope push in `visit_class_body` /
/// `visit_method_function`, the synthetic enum is generated with `type_params:
/// vec![]` and the generic name (`S`) leaks as a dangling external type ref.
#[test]
fn test_class_method_generic_propagates_to_synthetic_union_via_type_resolver() {
    let source = r#"
        type Wrap<T> = { wrapped: T };
        class Holder<S> {
            transform(x: S | Wrap<S>): S {
                return (x as Wrap<S>).wrapped ?? (x as S);
            }
        }
    "#;
    let (_, synthetic) = resolve_with_synthetic(source);

    // Find the synthetic union enum for `S | Wrap<S>` (or `Wrap<S> | S`).
    // After T2.A-ii it must declare `S` in its type_params.
    let union_with_s = synthetic.all_items().into_iter().find(|item| {
        matches!(
            item,
            crate::ir::Item::Enum { name, type_params, .. }
                if name.contains("Wrap") && type_params.iter().any(|tp| tp.name == "S")
        )
    });
    assert!(
        union_with_s.is_some(),
        "expected synthetic union enum for `S | Wrap<S>` to declare S in type_params \
         (without TypeResolver scope push, S would leak as a dangling external ref). \
         all_items: {:?}",
        synthetic
            .all_items()
            .iter()
            .filter_map(|i| match i {
                crate::ir::Item::Enum {
                    name, type_params, ..
                } => Some((name.clone(), type_params.clone())),
                _ => None,
            })
            .collect::<Vec<_>>()
    );
}

/// I-383 T2.A-iv: When a const variable is annotated with a generic interface
/// call signature (e.g., `SSGParamsMiddleware: <E extends Env>(...)`),
/// `convert_ts_type` flattens it into `RustType::Fn { ...with E... }` and the
/// `<E>` binding is lost. Without expected-type free-var extraction in
/// `resolve_arrow_expr`, the inner arrow body would register synthetic union
/// types containing free `E` references and leak them as dangling external refs.
///
/// This test verifies that no synthetic enum is generated with `E` referenced
/// from a member type but absent from `type_params`.
#[test]
fn test_arrow_inheriting_generic_interface_does_not_leak_free_type_var() {
    let source = r#"
        type Ctx<E> = { env: E };
        interface GenericMiddleware {
            <E extends string = string>(handler: (c: Ctx<E>) => string): (c: Ctx<E>) => string;
        }
        const wrap: GenericMiddleware = (handler) => (c) => handler(c);
    "#;
    let (_, synthetic) = resolve_with_synthetic(source);

    // No synthetic enum should reference `E` in its members without also
    // declaring `E` in `type_params`. (E leaking would mean an Item::Enum exists
    // whose variant data uses E but type_params doesn't list E.)
    let leaked = synthetic.all_items().into_iter().find(|item| {
        if let crate::ir::Item::Enum {
            type_params,
            variants,
            ..
        } = item
        {
            let declares_e = type_params.iter().any(|tp| tp.name == "E");
            let uses_e_in_variant = variants.iter().any(|v| {
                v.data.as_ref().is_some_and(|d| d.uses_param("E"))
                    || v.fields.iter().any(|f| f.ty.uses_param("E"))
            });
            uses_e_in_variant && !declares_e
        } else {
            false
        }
    });
    assert!(
        leaked.is_none(),
        "synthetic enum leaked free type var E (uses E in variant but absent in type_params): {leaked:?}"
    );
}
