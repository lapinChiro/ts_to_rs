//! Tests for destructuring variable type registration.
//!
//! Verifies that `register_pat_vars` registers destructured variables with
//! correct types (not Unknown) when source type information is available.
//!
//! Test strategy: destructured variables are used in expression statements
//! (`x;`), causing `resolve_expr` to call `lookup_var` and store the result
//! in `expr_types`. We verify by checking for the expected type in `expr_types`.
//! Functions return `void` to avoid return-type annotations polluting `expr_types`.

use crate::ir::RustType;
use crate::pipeline::ResolvedType;
use crate::registry::TypeRegistry;

use super::{resolve, resolve_with_reg};

/// Helper: check if a variable is registered (exists in var_mutability).
fn var_is_registered(
    res: &crate::pipeline::type_resolution::FileTypeResolution,
    var_name: &str,
) -> bool {
    res.var_mutability.keys().any(|v| v.name == var_name)
}

/// Helper: count how many expr_types entries match a predicate.
fn count_expr_types(
    res: &crate::pipeline::type_resolution::FileTypeResolution,
    pred: impl Fn(&ResolvedType) -> bool,
) -> usize {
    res.expr_types.values().filter(|t| pred(t)).count()
}

fn build_point_registry() -> TypeRegistry {
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
    reg
}

fn build_options_registry() -> TypeRegistry {
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
    reg
}

fn build_nested_registry() -> TypeRegistry {
    let mut reg = TypeRegistry::new();
    reg.register(
        "Inner".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![("value".to_string(), RustType::String).into()],
            Default::default(),
            vec![],
        ),
    );
    reg.register(
        "Outer".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![(
                "inner".to_string(),
                RustType::Named {
                    name: "Inner".to_string(),
                    type_args: vec![],
                },
            )
                .into()],
            Default::default(),
            vec![],
        ),
    );
    reg
}

// =============================================================================
// Object destructuring: Assign pattern ({ x })
// =============================================================================

#[test]
fn test_destructuring_assign_gets_field_type() {
    // const { x } = p; where p: Point → x should be F64
    let reg = build_point_registry();
    let res = resolve_with_reg(
        r#"
        function f(p: Point): void {
            const { x } = p;
            x;
        }
        "#,
        &reg,
    );

    assert!(var_is_registered(&res, "x"), "x should be registered");
    // Only source of F64 in expr_types is the `x;` expression statement
    let f64_count = count_expr_types(&res, |t| matches!(t, ResolvedType::Known(RustType::F64)));
    assert!(
        f64_count >= 1,
        "x should resolve to F64 when destructured from Point, \
         found {f64_count} F64 entries in expr_types"
    );
}

#[test]
fn test_destructuring_optional_field_gets_option_type() {
    // const { color } = opts; where color?: string → color should be Option<String>
    let reg = build_options_registry();
    let res = resolve_with_reg(
        r#"
        function f(opts: Options): void {
            const { color } = opts;
            color;
        }
        "#,
        &reg,
    );

    assert!(
        var_is_registered(&res, "color"),
        "color should be registered"
    );
    let opt_count = count_expr_types(
        &res,
        |t| matches!(t, ResolvedType::Known(RustType::Option(inner)) if matches!(inner.as_ref(), RustType::String)),
    );
    assert!(
        opt_count >= 1,
        "color should resolve to Option<String>, got {opt_count} matches"
    );
}

#[test]
fn test_destructuring_assign_with_default_gets_unwrapped_type() {
    // const { color = "black" } = opts; where color?: string
    // → color should be String (unwrapped from Option<String> due to default)
    let reg = build_options_registry();
    let res = resolve_with_reg(
        r#"
        function f(opts: Options): void {
            const { color = "black" } = opts;
            color;
        }
        "#,
        &reg,
    );

    assert!(
        var_is_registered(&res, "color"),
        "color should be registered"
    );
    // color should be String (unwrapped), NOT Option<String>
    let string_count =
        count_expr_types(&res, |t| matches!(t, ResolvedType::Known(RustType::String)));
    assert!(
        string_count >= 1,
        "color with default should resolve to String (unwrapped from Option<String>)"
    );
    // Verify it's NOT still Option<String>
    let opt_count = count_expr_types(
        &res,
        |t| matches!(t, ResolvedType::Known(RustType::Option(inner)) if matches!(inner.as_ref(), RustType::String)),
    );
    assert_eq!(
        opt_count, 0,
        "color with default should NOT be Option<String>"
    );
}

// =============================================================================
// Object destructuring: KeyValue pattern ({ x: y })
// =============================================================================

#[test]
fn test_destructuring_key_value_gets_field_type() {
    // const { x: px } = p; where p: Point → px should be F64
    let reg = build_point_registry();
    let res = resolve_with_reg(
        r#"
        function f(p: Point): void {
            const { x: px } = p;
            px;
        }
        "#,
        &reg,
    );

    assert!(var_is_registered(&res, "px"), "px should be registered");
    let f64_count = count_expr_types(&res, |t| matches!(t, ResolvedType::Known(RustType::F64)));
    assert!(
        f64_count >= 1,
        "px should resolve to F64 when destructured from Point.x"
    );
}

#[test]
fn test_destructuring_key_value_with_default_gets_unwrapped_type() {
    // const { color: c = "black" } = opts; where color?: string
    // SWC: KeyValue { key: "color", value: Pat::Assign { left: Ident("c"), right: "black" } }
    // → c should be String (unwrapped from Option<String>)
    let reg = build_options_registry();
    let res = resolve_with_reg(
        r#"
        function f(opts: Options): void {
            const { color: c = "black" } = opts;
            c;
        }
        "#,
        &reg,
    );

    assert!(var_is_registered(&res, "c"), "c should be registered");
    let string_count =
        count_expr_types(&res, |t| matches!(t, ResolvedType::Known(RustType::String)));
    assert!(
        string_count >= 1,
        "c with default should resolve to String (unwrapped from Option<String>)"
    );
}

// =============================================================================
// Object destructuring: Rest pattern ({ ...rest })
// =============================================================================

#[test]
fn test_destructuring_rest_registered() {
    // const { x, ...rest } = p; rest should be registered (as Unknown)
    let reg = build_point_registry();
    let res = resolve_with_reg(
        r#"
        function f(p: Point): void {
            const { x, ...rest } = p;
        }
        "#,
        &reg,
    );

    assert!(var_is_registered(&res, "x"), "x should be registered");
    assert!(var_is_registered(&res, "rest"), "rest should be registered");
}

// =============================================================================
// Object destructuring: nested ({ a: { b } })
// =============================================================================

#[test]
fn test_destructuring_nested_gets_inner_type() {
    // const { inner: { value } } = outer; where Outer.inner: Inner, Inner.value: String
    // → value should be String
    let reg = build_nested_registry();
    let res = resolve_with_reg(
        r#"
        function f(outer: Outer): void {
            const { inner: { value } } = outer;
            value;
        }
        "#,
        &reg,
    );

    assert!(
        var_is_registered(&res, "value"),
        "value should be registered"
    );
    let string_count =
        count_expr_types(&res, |t| matches!(t, ResolvedType::Known(RustType::String)));
    assert!(
        string_count >= 1,
        "value should resolve to String from nested destructuring"
    );
}

// =============================================================================
// Variable declaration: type annotation vs init type
// =============================================================================

#[test]
fn test_var_decl_destructuring_with_annotation() {
    // const { x }: Point = p; — type from pattern annotation
    let reg = build_point_registry();
    let res = resolve_with_reg(
        r#"
        function f(): void {
            const p: Point = { x: 1, y: 2 };
            const { x }: Point = p;
            x;
        }
        "#,
        &reg,
    );

    assert!(var_is_registered(&res, "x"), "x should be registered");
    let f64_count = count_expr_types(&res, |t| matches!(t, ResolvedType::Known(RustType::F64)));
    assert!(
        f64_count >= 1,
        "x should resolve to F64 from pattern type annotation"
    );
}

#[test]
fn test_var_decl_destructuring_from_init() {
    // const { x } = p; — no annotation, type from init expression
    let reg = build_point_registry();
    let res = resolve_with_reg(
        r#"
        function f(p: Point): void {
            const { x } = p;
            x;
        }
        "#,
        &reg,
    );

    assert!(var_is_registered(&res, "x"), "x should be registered");
    let f64_count = count_expr_types(&res, |t| matches!(t, ResolvedType::Known(RustType::F64)));
    assert!(
        f64_count >= 1,
        "x should resolve to F64 from init expression type"
    );
}

// =============================================================================
// Array destructuring
// =============================================================================

#[test]
fn test_array_destructuring_vec_gets_element_type() {
    // const [a, b] = arr; where arr: number[] → a, b should be F64
    // Use parameter to avoid numeric literals polluting expr_types with F64
    let res = resolve(
        r#"
        function f(arr: number[]): void {
            const [a, b] = arr;
            a;
            b;
        }
        "#,
    );

    assert!(var_is_registered(&res, "a"), "a should be registered");
    assert!(var_is_registered(&res, "b"), "b should be registered");
    // Only F64 sources: `a;` and `b;` expression statements (no numeric literals)
    let f64_count = count_expr_types(&res, |t| matches!(t, ResolvedType::Known(RustType::F64)));
    assert_eq!(
        f64_count, 2,
        "exactly a and b should resolve to F64 from Vec<F64>, found {f64_count}"
    );
}

#[test]
fn test_tuple_destructuring_gets_positional_types() {
    // const [a, b]: [string, number] = t; → a: String, b: F64
    let res = resolve(
        r#"
        function f(): void {
            const t: [string, number] = ["hello", 42];
            const [a, b] = t;
            a;
            b;
        }
        "#,
    );

    assert!(var_is_registered(&res, "a"), "a should be registered");
    assert!(var_is_registered(&res, "b"), "b should be registered");
    // a should be String, b should be F64
    let string_count =
        count_expr_types(&res, |t| matches!(t, ResolvedType::Known(RustType::String)));
    assert!(
        string_count >= 1,
        "a should resolve to String from tuple position 0"
    );
    let f64_count = count_expr_types(&res, |t| matches!(t, ResolvedType::Known(RustType::F64)));
    assert!(
        f64_count >= 1,
        "b should resolve to F64 from tuple position 1"
    );
}

// =============================================================================
// Function parameter destructuring
// =============================================================================

#[test]
fn test_fn_param_destructuring_gets_field_type() {
    // function f({ x }: Point) → x should be F64
    let reg = build_point_registry();
    let res = resolve_with_reg(
        r#"
        function f({ x }: Point): void {
            x;
        }
        "#,
        &reg,
    );

    assert!(var_is_registered(&res, "x"), "x should be registered");
    let f64_count = count_expr_types(&res, |t| matches!(t, ResolvedType::Known(RustType::F64)));
    assert!(
        f64_count >= 1,
        "x should resolve to F64 from parameter destructuring"
    );
}

#[test]
fn test_fn_param_array_destructuring_gets_element_type() {
    // function f([a, b]: [string, number]) → a: String, b: F64
    let res = resolve(
        r#"
        function f([a, b]: [string, number]): void {
            a;
            b;
        }
        "#,
    );

    assert!(var_is_registered(&res, "a"), "a should be registered");
    assert!(var_is_registered(&res, "b"), "b should be registered");
    let string_count =
        count_expr_types(&res, |t| matches!(t, ResolvedType::Known(RustType::String)));
    assert!(
        string_count >= 1,
        "a should resolve to String from array param"
    );
    let f64_count = count_expr_types(&res, |t| matches!(t, ResolvedType::Known(RustType::F64)));
    assert!(f64_count >= 1, "b should resolve to F64 from array param");
}

#[test]
fn test_fn_param_array_default_gets_expected() {
    // function f([a = 0]: [number]) → default 0 should get F64 expected type
    let res = resolve(
        r#"
        function f([a = 0]: [number]): void {
        }
        "#,
    );

    assert!(var_is_registered(&res, "a"), "a should be registered");
    // The default expression `0` should have F64 as expected type
    // (propagated from the array element type via propagate_destructuring_defaults)
    let has_f64_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::F64));
    assert!(
        has_f64_expected,
        "default value 0 in array param should have F64 expected type, got: {:?}",
        res.expected_types.values().collect::<Vec<_>>()
    );
}

// =============================================================================
// Unknown source (no regression)
// =============================================================================

#[test]
fn test_destructuring_unknown_source_registers_unknown() {
    // const { x } = unknownObj; → x registered (as Unknown, no panic)
    let res = resolve(
        r#"
        function f(): void {
            const obj: any = {};
            const { x } = obj;
        }
        "#,
    );

    assert!(
        var_is_registered(&res, "x"),
        "x should be registered even with unknown source"
    );
}
