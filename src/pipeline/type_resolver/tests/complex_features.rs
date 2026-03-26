use super::*;

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
