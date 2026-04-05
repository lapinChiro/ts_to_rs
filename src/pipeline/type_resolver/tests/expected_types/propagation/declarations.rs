use super::*;

#[test]
fn test_propagate_expected_var_decl_object_literal_sets_struct_name() {
    // 1-2: const p: Point = { x: 1, y: 2 } → object literal gets Named("Point")
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64).into(),
                ("y".to_string(), RustType::F64).into(),
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
                ("x".to_string(), RustType::F64).into(),
                ("y".to_string(), RustType::F64).into(),
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
fn test_propagate_expected_assign_rhs_gets_lhs_type() {
    // 1-6: let x: Point; x = { x: 1, y: 2 }
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64).into(),
                ("y".to_string(), RustType::F64).into(),
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

// --- Destructuring default expected type propagation ---

#[test]
fn test_destructuring_default_string_gets_string_expected() {
    // const { color = "black" } = opts where color?: string
    // The default expr "black" should get expected type String (unwrapped from Option<String>).
    // Use a void function with no return to ensure String expected is from destructuring only.
    let mut reg = TypeRegistry::new();
    reg.register(
        "Options".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![
                ("width".to_string(), RustType::F64).into(),
                (
                    "color".to_string(),
                    RustType::Option(Box::new(RustType::String)),
                )
                    .into(),
            ],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg(
        r#"
        function f(opts: Options): void {
            const { color = "black" } = opts;
        }
        "#,
        &reg,
    );

    // The string literal "black" should have expected type String.
    // Since return type is void, the only source of String expected type is
    // the destructuring default propagation.
    let has_string_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::String));
    assert!(
        has_string_expected,
        "destructuring default \"black\" should have String as expected type, got: {:?}",
        res.expected_types.values().collect::<Vec<_>>()
    );
}

#[test]
fn test_destructuring_default_number_gets_f64_expected() {
    // const { count = 10 } = opts where count?: number
    // Use a void function to isolate the source of F64 expected type.
    let mut reg = TypeRegistry::new();
    reg.register(
        "Options".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![
                ("name".to_string(), RustType::String).into(),
                (
                    "count".to_string(),
                    RustType::Option(Box::new(RustType::F64)),
                )
                    .into(),
            ],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg(
        r#"
        function f(opts: Options): void {
            const { count = 10 } = opts;
        }
        "#,
        &reg,
    );

    let has_f64_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::F64));
    assert!(
        has_f64_expected,
        "destructuring default 10 should have F64 as expected type"
    );
}

#[test]
fn test_destructuring_default_non_optional_field_gets_expected() {
    // const { width = 100 } = opts where width: number (not optional)
    // Even for non-optional fields, the default should get the field type as expected.
    let mut reg = TypeRegistry::new();
    reg.register(
        "Config".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![("width".to_string(), RustType::F64).into()],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg(
        r#"
        function f(cfg: Config): void {
            const { width = 100 } = cfg;
        }
        "#,
        &reg,
    );

    let has_f64_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::F64));
    assert!(
        has_f64_expected,
        "destructuring default for non-optional field should have F64 as expected type"
    );
}

#[test]
fn test_destructuring_default_fn_param_string_gets_string_expected() {
    // function f({ color = "black" }: Options) — function parameter destructuring.
    // The default "black" should get String as expected type (unwrapped from Option<String>).
    let mut reg = TypeRegistry::new();
    reg.register(
        "Options".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![
                ("width".to_string(), RustType::F64).into(),
                (
                    "color".to_string(),
                    RustType::Option(Box::new(RustType::String)),
                )
                    .into(),
            ],
            Default::default(),
            vec![],
        ),
    );

    let res = resolve_with_reg(
        r#"
        function f({ color = "black" }: Options): void {
        }
        "#,
        &reg,
    );

    let has_string_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::String));
    assert!(
        has_string_expected,
        "fn param destructuring default \"black\" should have String as expected type, got: {:?}",
        res.expected_types.values().collect::<Vec<_>>()
    );
}

#[test]
fn test_destructuring_default_unknown_source_type_no_panic() {
    // const { x = 0 } = unknownObj — source type is unknown.
    // Should not panic and should not set expected types for default.
    let res = resolve(
        r#"
        function f(): void {
            const obj: any = {};
            const { x = 0 } = obj;
        }
        "#,
    );

    // Just verify no panic — we don't require expected type propagation
    // when the source type is unknown.
    assert!(
        !res.expr_types.is_empty(),
        "should resolve expressions without panic"
    );
}
