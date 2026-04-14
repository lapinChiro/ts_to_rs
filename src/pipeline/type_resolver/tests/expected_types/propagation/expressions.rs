use super::*;

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

// ── I-022: resolve_bin_expr NC arm ──

#[test]
fn test_resolve_nc_lhs_non_option_propagates_lhs_option_wrap() {
    // `arr[i] ?? "m"` where arr: string[] (TS static type of arr[i] is `string`,
    // not `string | undefined`). LHS span must receive `Option<String>` expected
    // so convert_member_expr emits `.get(i).cloned()` (Option-preserving).
    let res = resolve(
        r#"
        function f(arr: string[], i: number): string {
            return arr[i] ?? "m";
        }
        "#,
    );

    let has_option_string_expected = res.expected_types.values().any(
        |t| matches!(t, RustType::Option(inner) if matches!(inner.as_ref(), RustType::String)),
    );
    assert!(
        has_option_string_expected,
        "NC LHS with non-Option TS type should have Option<String> expected propagated \
         (expected_types: {:?})",
        res.expected_types
    );
}

#[test]
fn test_resolve_nc_lhs_non_option_propagates_rhs_same() {
    // `arr[i] ?? d` — RHS has expected = `String` (the NC result type),
    // not `Option<String>`. Confirms the asymmetric LHS/RHS propagation.
    let res = resolve(
        r#"
        function f(arr: string[], i: number, d: string): string {
            return arr[i] ?? d;
        }
        "#,
    );

    let string_count = res
        .expected_types
        .values()
        .filter(|t| matches!(t, RustType::String))
        .count();
    // At least: the `arr[i] ?? d` span (return-propagated) + RHS `d` span.
    assert!(
        string_count >= 2,
        "NC RHS should have String expected (final NC result type); \
         string count was {string_count}, expected_types: {:?}",
        res.expected_types
    );
}

#[test]
fn test_resolve_nc_lhs_unknown_propagates_nothing() {
    // `a ?? b` with both operands untyped (no annotations, no inference anchor).
    // Neither LHS nor RHS span should receive expected types from the NC arm.
    let res = resolve("const r = a ?? b;");

    // The key assertion: no String/Option<String> is injected purely from NC
    // propagation. (Other propagations from `const` decls may add types, so we
    // check the absence of NC-specific Option<T> expected.)
    let has_option_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::Option(_)));
    assert!(
        !has_option_expected,
        "NC with untyped LHS should not propagate Option<_> expected (got {:?})",
        res.expected_types
    );
}

#[test]
fn test_resolve_nc_lhs_option_preserves_existing_behavior() {
    // LHS already Option<f64>: legacy behavior preserved — RHS gets `f64` expected
    // (inner unwrap), and LHS span is NOT forcibly rewrapped to Option<Option<f64>>.
    let res = resolve(
        r#"
        function f(x: number | null): number {
            return x ?? 0;
        }
        "#,
    );

    // RHS 0 should have F64 expected
    let has_f64_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::F64));
    assert!(
        has_f64_expected,
        "NC with LHS Option<f64> should propagate f64 to RHS (got {:?})",
        res.expected_types
    );

    // Must NOT have Option<Option<f64>> (no double-wrap regression)
    let has_double_option = res.expected_types.values().any(
        |t| matches!(t, RustType::Option(outer) if matches!(outer.as_ref(), RustType::Option(_))),
    );
    assert!(
        !has_double_option,
        "NC must not double-wrap LHS when already Option (got {:?})",
        res.expected_types
    );
}

#[test]
fn test_propagate_expected_nc_chain_propagates_option_to_inner_rhs() {
    // I-022 /check_job deep review finding: chain `a ?? b ?? c` in outer
    // Option<T>-expected context (or chain-LHS-of-outer) must propagate
    // Option<T> to inner NC's RHS span. Without this, inner RHS
    // (e.g., `items[j]` for Vec<Option<T>>) would receive String expected
    // → convert_member_expr emits `.unwrap()` form → runtime panic.
    //
    // Propagation mechanism: `propagate_expected` Bin(NullishCoalescing) arm
    // detects outer Option<T> context and overrides inner LHS/RHS spans.
    // `resolve_bin_expr` NC arm then preserves the already-set Option RHS
    // rather than overwriting with inner_val.
    let res = resolve(
        r#"
        function f(items: (string | null)[], i: number, j: number): string {
            return items[i] ?? items[j] ?? "default";
        }
        "#,
    );

    // Every operand in the chain (items[i], items[j]) must have Option<String>
    // expected. We expect at least 2 Option<String> entries (LHS + inner RHS).
    let option_string_count = res
        .expected_types
        .values()
        .filter(
            |t| matches!(t, RustType::Option(inner) if matches!(inner.as_ref(), RustType::String)),
        )
        .count();
    assert!(
        option_string_count >= 2,
        "chain NC must propagate Option<String> to all inner operands, got {option_string_count} \
         Option<String> entries. expected_types: {:?}",
        res.expected_types
    );
}

#[test]
fn test_resolve_nc_lhs_option_also_propagates_lhs_span_option() {
    // I-022 unified arm: for LHS already Option<T>, LHS span receives
    // Option<T> expected (preserving Option for convert_member_expr to emit
    // Option-preserving forms like `.get().cloned().flatten()` for Vec<Option<T>>).
    //
    // Regression test for Vec<Option<T>> + NC silent bug: before unification,
    // the Option LHS arm only propagated RHS expected, leaving LHS span empty
    // → convert_member_expr emitted `.unwrap()` form → panic on empty array.
    let res = resolve(
        r#"
        function f(items: (string | null)[], i: number): string {
            return items[i] ?? "default";
        }
        "#,
    );

    let has_option_string_expected = res.expected_types.values().any(
        |t| matches!(t, RustType::Option(inner) if matches!(inner.as_ref(), RustType::String)),
    );
    assert!(
        has_option_string_expected,
        "NC with LHS `Vec<Option<String>>` index must have `Option<String>` expected \
         propagated to LHS span so convert_member_expr emits `.flatten()` form \
         (expected_types: {:?})",
        res.expected_types
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
fn test_propagate_expected_object_lit_fields_from_synthetic_type() {
    // const x: { name: string; count: number } = { name: "hello", count: 42 }
    // The inline type becomes _TypeLitN. propagate_expected with Named("_TypeLitN")
    // should resolve fields via resolve_object_lit_fields → resolve_struct_fields_by_name,
    // setting String expected on "hello" and F64 expected on 42.
    let res = resolve(
        r#"
        const x: { name: string; count: number } = { name: "hello", count: 42 };
        "#,
    );

    // "hello" should have expected type String
    let has_string_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::String));
    assert!(
        has_string_expected,
        "field 'name' value should have String expected type from synthetic struct"
    );

    // 42 should have expected type F64
    let has_f64_expected = res
        .expected_types
        .values()
        .any(|t| matches!(t, RustType::F64));
    assert!(
        has_f64_expected,
        "field 'count' value should have F64 expected type from synthetic struct"
    );
}
