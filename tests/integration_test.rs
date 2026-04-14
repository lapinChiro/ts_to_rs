use std::fs;
use ts_to_rs::{transpile, transpile_collecting, transpile_with_builtins};

/// Generates a snapshot test from a fixture file name.
///
/// Derives the fixture path from the test name: `test_foo_bar` → `tests/fixtures/foo-bar.input.ts`.
///
/// Variants:
/// - `snapshot_test!(test_foo)` — uses `transpile()` (no builtins, errors on unsupported)
/// - `snapshot_test!(test_foo, collecting)` — uses `transpile_collecting()` (no builtins, collects unsupported)
/// - `snapshot_test!(test_foo, builtins)` — uses `transpile_with_builtins()` (builtins loaded, collects unsupported)
macro_rules! snapshot_test {
    ($name:ident) => {
        #[test]
        fn $name() {
            let fixture = stringify!($name)
                .strip_prefix("test_")
                .unwrap_or(stringify!($name))
                .replace('_', "-");
            let input = fs::read_to_string(format!("tests/fixtures/{fixture}.input.ts")).unwrap();
            let output = transpile(&input).unwrap();
            insta::assert_snapshot!(output);
        }
    };
    ($name:ident, collecting) => {
        #[test]
        fn $name() {
            let fixture = stringify!($name)
                .strip_prefix("test_")
                .unwrap_or(stringify!($name))
                .replace('_', "-");
            let input = fs::read_to_string(format!("tests/fixtures/{fixture}.input.ts")).unwrap();
            let (output, unsupported) = transpile_collecting(&input).unwrap();
            insta::assert_snapshot!(output);
            if !unsupported.is_empty() {
                let json = serde_json::to_string_pretty(&unsupported).unwrap();
                insta::assert_snapshot!(format!("{}_unsupported", stringify!($name)), json);
            }
        }
    };
    ($name:ident, builtins) => {
        #[test]
        fn $name() {
            let fixture = stringify!($name)
                .strip_prefix("test_")
                .unwrap_or(stringify!($name))
                .replace('_', "-");
            let input = fs::read_to_string(format!("tests/fixtures/{fixture}.input.ts")).unwrap();
            let (output, unsupported) = transpile_with_builtins(&input).unwrap();
            insta::assert_snapshot!(output);
            if !unsupported.is_empty() {
                let json = serde_json::to_string_pretty(&unsupported).unwrap();
                insta::assert_snapshot!(format!("{}_unsupported", stringify!($name)), json);
            }
        }
    };
}

// ── transpile (no builtins, errors on unsupported) ──────────────────────

snapshot_test!(test_import_export);
snapshot_test!(test_basic_types);
snapshot_test!(test_optional_fields);
snapshot_test!(test_functions);
snapshot_test!(test_classes);
snapshot_test!(test_closures);
snapshot_test!(test_generics);
snapshot_test!(test_function_calls);
snapshot_test!(test_error_handling);
snapshot_test!(test_loops);
snapshot_test!(test_mixed);
snapshot_test!(test_enum);
snapshot_test!(test_array_literal);
snapshot_test!(test_object_literal);
snapshot_test!(test_type_registry);
snapshot_test!(test_string_to_string);
snapshot_test!(test_ternary);
snapshot_test!(test_class_inheritance, collecting);
snapshot_test!(test_break_continue);
snapshot_test!(test_object_destructuring);
snapshot_test!(test_console_api);
snapshot_test!(test_general_for_loop);
snapshot_test!(test_string_methods);
snapshot_test!(test_array_methods);
snapshot_test!(test_do_while);
snapshot_test!(test_array_destructuring, collecting);
snapshot_test!(test_number_parse_api);
snapshot_test!(test_math_api, collecting);
snapshot_test!(test_async_await);
snapshot_test!(test_async_class_method);
snapshot_test!(test_const_primitive);
snapshot_test!(test_callable_interface_param_rename);
snapshot_test!(test_callable_interface_inner);
snapshot_test!(test_callable_interface_async);
snapshot_test!(test_callable_interface_generic);
snapshot_test!(test_callable_interface_call);
snapshot_test!(test_callable_interface_call_generic);
snapshot_test!(test_callable_interface_generic_default);
snapshot_test!(test_type_infer_unannotated);
snapshot_test!(test_unary_operators, collecting);
snapshot_test!(test_void_type);
snapshot_test!(test_nullish_coalescing);
snapshot_test!(test_type_assertion);
snapshot_test!(test_optional_chaining);
snapshot_test!(test_keyword_types);
snapshot_test!(test_array_spread);
snapshot_test!(test_object_spread);
snapshot_test!(test_string_literal_union);
snapshot_test!(test_getter_setter);
snapshot_test!(test_default_params);
snapshot_test!(test_union_type);
snapshot_test!(test_indexed_access_type, collecting);
snapshot_test!(test_builtin_api_batch);
snapshot_test!(test_inline_type_literal_param);
snapshot_test!(test_interface_mixed);
snapshot_test!(test_discriminated_union);
snapshot_test!(test_intersection_type);
snapshot_test!(test_abstract_class);
snapshot_test!(test_conditional_type);
snapshot_test!(test_switch);
snapshot_test!(test_param_properties);
snapshot_test!(test_update_expr);
snapshot_test!(test_class_default_params);
snapshot_test!(test_fn_expr);
snapshot_test!(test_var_type_arrow);
snapshot_test!(test_var_type_alias_arrow);
snapshot_test!(test_regex_literal);
snapshot_test!(test_call_signature_rest);
snapshot_test!(test_callable_interface, collecting);
snapshot_test!(test_type_alias_utility);
snapshot_test!(test_any_type_narrowing);
snapshot_test!(test_type_narrowing);
snapshot_test!(test_union_fallback);
snapshot_test!(test_generic_class);
snapshot_test!(test_array_builtin_methods);
snapshot_test!(test_intersection_methods);
snapshot_test!(test_intersection_empty_object, collecting);
snapshot_test!(test_intersection_fallback, collecting);
snapshot_test!(test_intersection_union_distribution, collecting);
snapshot_test!(test_typeof_const);
snapshot_test!(test_assignment_expected_type);
snapshot_test!(test_as_type_expected);
snapshot_test!(test_ternary_union);
snapshot_test!(test_explicit_type_args);
snapshot_test!(test_private_member_expected_type);

// ── transpile_collecting (no builtins, collects unsupported) ────────────

snapshot_test!(test_interface_methods, collecting);
snapshot_test!(test_narrowing_truthy_instanceof, collecting);
snapshot_test!(test_trait_coercion, collecting);
snapshot_test!(test_anon_struct_inference, collecting);
snapshot_test!(test_instanceof_builtin, collecting);

// ── transpile_with_builtins ────────────────────────────────────────────

snapshot_test!(test_vec_method_expected_type, builtins);
snapshot_test!(test_external_type_struct, builtins);
// Tests below use same fixture as another test but with builtins loaded.
// Can't use macro (fixture name ≠ test name).

#[test]
fn test_string_methods_with_builtins() {
    let input = fs::read_to_string("tests/fixtures/string-methods.input.ts").unwrap();
    let (output, unsupported) = transpile_with_builtins(&input).unwrap();
    insta::assert_snapshot!(output);
    if !unsupported.is_empty() {
        let json = serde_json::to_string_pretty(&unsupported).unwrap();
        insta::assert_snapshot!("test_string_methods_with_builtins_unsupported", json);
    }
}

#[test]
fn test_instanceof_builtin_with_builtins() {
    let input = fs::read_to_string("tests/fixtures/instanceof-builtin.input.ts").unwrap();
    let (output, unsupported) = transpile_with_builtins(&input).unwrap();
    insta::assert_snapshot!(output);
    if !unsupported.is_empty() {
        let json = serde_json::to_string_pretty(&unsupported).unwrap();
        insta::assert_snapshot!("test_instanceof_builtin_with_builtins_unsupported", json);
    }
}

// ── Custom tests (non-macro: require specialized assertions) ───────────

/// Step 2 (RC-2): `throw new Error(msg)` must produce `Err(msg.to_string())`,
/// not `Err(Some(msg).to_string())`.
///
/// With strict-null-checks in the extract tool, the `Error` constructor's
/// `message?: string` param resolves to `Option<String>`, so the TypeResolver
/// propagates `Option<String>` as the expected type for `msg`. `convert_expr`
/// then wraps the string arg as `Some(msg)`. In the throw flow the message is
/// lifted out of the constructor and passed directly to `.to_string()` —
/// `Some(String)::to_string()` produces `"Some(msg)"` at runtime (silent
/// semantic change) and `Option::<String>::to_string()` is not even defined
/// (compile error). `extract_error_message` strips the outer `Some(...)`
/// specifically for this case.
#[test]
fn test_throw_new_error_strips_some_wrap_with_builtins() {
    let input = r#"
function throwError(msg: string): never {
  throw new Error(msg);
}
"#;
    let (output, _unsupported) = transpile_with_builtins(input).unwrap();
    assert!(
        output.contains("Err(msg.to_string())"),
        "expected `Err(msg.to_string())` without Some-wrap, got:\n{output}"
    );
    assert!(
        !output.contains("Some(msg)"),
        "Some-wrap must be stripped from throw new Error arg, got:\n{output}"
    );
}

#[test]
fn test_throw_new_error_string_literal_no_double_to_string() {
    // Regression for the full convert_expr → extract_error_message → to_string
    // chain on a literal arg: expected Option<String> triggers
    // `convert_lit` to append `.to_string()` under the Some, AND the outer Some
    // wrap. Stripping only Some leaves `"x".to_string()`, then
    // convert_throw_stmt appends another `.to_string()` producing
    // `"x".to_string().to_string()`. extract_error_message now strips the
    // redundant inner `.to_string()` as well.
    let input = r#"
function fail(): never {
  throw new Error("static");
}
"#;
    let (output, _unsupported) = transpile_with_builtins(input).unwrap();
    assert!(
        output.contains("Err(\"static\".to_string())"),
        "expected single `.to_string()` on literal arg, got:\n{output}"
    );
    assert!(
        !output.contains(".to_string().to_string()"),
        "redundant double `.to_string()` must be stripped, got:\n{output}"
    );
}

#[test]
fn test_callable_interface_generic_arity_mismatch_errors() {
    let input =
        fs::read_to_string("tests/fixtures/callable-interface-generic-arity-mismatch.input.ts")
            .unwrap();
    // transpile_collecting is resilient: errors become unsupported entries, not Err
    let (output, unsupported) = transpile_collecting(&input).unwrap();
    // The arity mismatch error should be in unsupported, not in the output
    let has_arity_error = unsupported
        .iter()
        .any(|u| u.kind.contains("type parameters") || u.kind.contains("type arguments"));
    assert!(
        has_arity_error,
        "callable interface with generic arity mismatch should produce unsupported error (INV-4), \
         got unsupported: {unsupported:?}, output: {output}"
    );
    // The const declaration should NOT appear in output (conversion was aborted for this item)
    assert!(
        !output.contains("mapStr"),
        "mapStr should not appear in output when arity mismatch, got: {output}"
    );
}

#[test]
fn test_unsupported_syntax_default_errors() {
    let input = fs::read_to_string("tests/fixtures/unsupported-syntax.input.ts").unwrap();
    let result = transpile(&input);
    assert!(
        result.is_err(),
        "transpile should error on unsupported syntax by default"
    );
}

#[test]
fn test_unsupported_syntax_collecting_output() {
    let input = fs::read_to_string("tests/fixtures/unsupported-syntax.input.ts").unwrap();
    let (output, unsupported) = transpile_collecting(&input).unwrap();
    insta::assert_snapshot!("unsupported_syntax_rust_output", output);
    let json = serde_json::to_string_pretty(&unsupported).unwrap();
    insta::assert_snapshot!("unsupported_syntax_json_report", json);
}

#[test]
fn test_multi_var_decl() {
    let input = fs::read_to_string("tests/fixtures/multi-var-decl.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    assert!(
        output.contains("let a") && output.contains("let b"),
        "multi-var const should expand to separate lets: {output}"
    );
    assert!(
        output.contains("let mut x") && output.contains("let y"),
        "multi-var let: x is reassigned (let mut), y is not (let): {output}"
    );
    assert!(
        !output.contains("let mut y"),
        "y is never reassigned, should not be let mut: {output}"
    );
    insta::assert_snapshot!(output);
}

#[test]
fn test_nullable_return() {
    let input = fs::read_to_string("tests/fixtures/nullable-return.input.ts").unwrap();
    let output = transpile(&input).unwrap();
    assert!(
        !output.contains("Some(if"),
        "ternary return should not be wrapped in Some(): {output}"
    );
    assert!(
        output.contains(r#"Some("found".to_string())"#),
        "direct return of literal should be wrapped in Some(): {output}"
    );
    insta::assert_snapshot!(output);
}

/// I-040: TS interface method の optional param (`y?: number`) は Rust trait method で
/// `Option<f64>` にラップされ、caller からの引数不足は `None` で自動補完される。
#[test]
fn test_interface_method_optional_param_compiles() {
    let input = r#"
interface Foo {
    bar(x: number, y?: number): number;
}
function runit(f: Foo): number {
    return f.bar(1);
}
"#;
    let output = transpile(input).unwrap();
    assert!(
        output.contains("fn bar(&self, x: f64, y: Option<f64>)"),
        "optional interface param must be Option<f64>, got:\n{output}"
    );
    assert!(
        output.contains("f.bar(1.0, None)") || output.contains("f.bar(1_f64, None)"),
        "caller must fill omitted optional with None, got:\n{output}"
    );
}

/// I-040: TS class method の optional param も Rust で `Option<T>` に統一される。
#[test]
fn test_class_method_optional_param_compiles() {
    let input = r#"
class Foo {
    bar(x: number, y?: number): number {
        return x;
    }
}
function runit(f: Foo): number {
    return f.bar(1);
}
"#;
    let output = transpile(input).unwrap();
    assert!(
        output.contains("fn bar(&self, x: f64, y: Option<f64>)"),
        "optional class method param must be Option<f64>, got:\n{output}"
    );
    assert!(
        output.contains(".bar(1.0, None)") || output.contains(".bar(1_f64, None)"),
        "caller must fill omitted optional with None, got:\n{output}"
    );
}

/// I-040: TS fn type alias の optional param も `Option<T>` に統一される。
#[test]
fn test_fn_type_alias_optional_param_compiles() {
    let input = r#"
type Callback = (x: number, y?: number) => number;
function runit(f: Callback): number {
    return f(1);
}
"#;
    let output = transpile(input).unwrap();
    assert!(
        output.contains("Fn(f64, Option<f64>) -> f64"),
        "fn type alias optional param must be Option<f64>, got:\n{output}"
    );
    assert!(
        output.contains("f(1.0, None)") || output.contains("f(1_f64, None)"),
        "caller must fill omitted optional with None, got:\n{output}"
    );
}

/// I-040: インライン fn 型 param の optional も `Option<T>` + fill-None。
#[test]
fn test_inline_fn_type_optional_param_compiles() {
    let input = r#"
function runit(f: (x: number, y?: number) => number): number {
    return f(1);
}
"#;
    let output = transpile(input).unwrap();
    assert!(
        output.contains("Fn(f64, Option<f64>) -> f64"),
        "inline fn type optional param must be Option<f64>, got:\n{output}"
    );
    assert!(
        output.contains("f(1.0, None)") || output.contains("f(1_f64, None)"),
        "caller must fill omitted optional with None, got:\n{output}"
    );
}

/// I-040: `x?: T = value` (optional + default 併用) で `Option<Option<T>>` に
/// ならず、単一の `Option<T>` にラップされることを保証する。
/// class method / ctor / free fn の 3 系統すべてで検証する。
#[test]
fn test_optional_plus_default_no_double_wrap() {
    // (1) free fn: `x?: T = value`
    let input_free = r#"
function fff(x?: number = 5): number {
    return x;
}
"#;
    let output_free = transpile(input_free).unwrap();
    assert!(
        !output_free.contains("Option<Option<"),
        "free fn `x?: T = value` must not produce Option<Option<_>>, got:\n{output_free}"
    );
    assert!(
        output_free.contains("x: Option<f64>"),
        "free fn `x?: T = value` must produce a single Option<f64>, got:\n{output_free}"
    );

    // (2) class method: `m(x?: T = value)`
    let input_method = r#"
class C {
    m(x?: number = 5): number {
        return x;
    }
}
"#;
    let output_method = transpile(input_method).unwrap();
    assert!(
        !output_method.contains("Option<Option<"),
        "class method `x?: T = value` must not produce Option<Option<_>>, got:\n{output_method}"
    );
    assert!(
        output_method.contains("x: Option<f64>"),
        "class method `x?: T = value` must produce a single Option<f64>, got:\n{output_method}"
    );

    // (3) class constructor: `constructor(x?: T = value)`
    let input_ctor = r#"
class C {
    constructor(x?: number = 5) {
    }
}
"#;
    let output_ctor = transpile(input_ctor).unwrap();
    assert!(
        !output_ctor.contains("Option<Option<"),
        "ctor `x?: T = value` must not produce Option<Option<_>>, got:\n{output_ctor}"
    );
    assert!(
        output_ctor.contains("x: Option<f64>"),
        "ctor `x?: T = value` must produce a single Option<f64>, got:\n{output_ctor}"
    );
}

/// I-040 (visit_param_pat fix): TypeResolver が `x?: T` を `Option<T>` として
/// scope に登録することで、本体内の `if (x)` が `if let Some(x) = x` に narrowing
/// される。逆に `x: T = value` (default-only) では本体内で `x` は `T` として登録
/// される (default expansion stmt が unwrap するため)。
#[test]
fn test_optional_param_narrows_to_if_let_in_body() {
    let input = r#"
function f(name: string, prefix?: string): string {
    if (prefix) {
        return prefix;
    }
    return name;
}
"#;
    let output = transpile(input).unwrap();
    assert!(
        output.contains("if let Some(prefix) = prefix"),
        "optional param `prefix` must narrow via `if let Some(...)`, got:\n{output}"
    );
}

#[test]
fn test_default_param_keeps_unwrapped_type_in_body() {
    let input = r#"
function f(x: number = 5): number {
    return x + 1;
}
"#;
    let output = transpile(input).unwrap();
    // After `let x = x.unwrap_or(5.0)` expansion, body sees x as f64 not Option<f64>.
    // No `if let Some(x) = x` narrowing should be inserted; arithmetic must compile.
    assert!(
        output.contains("x + 1.0") || output.contains("x + 1_f64"),
        "default param `x` must be treated as f64 in body (post-expansion), got:\n{output}"
    );
    assert!(
        !output.contains("Some(x + 1"),
        "default param body must not double-wrap return value, got:\n{output}"
    );
}

/// I-040 S7: anonymous type literal method (`{ m(y?: number): void }`) の optional
/// param が `Option<T>` にラップされる。`resolve_method_info` 経由で IR に到達するパス
/// を end-to-end で検証する。
#[test]
fn test_type_literal_method_optional_param_compiles() {
    let input = r#"
type Container = { compute(x: number, y?: number): number };
function runit(c: Container): number {
    return c.compute(1);
}
"#;
    let output = transpile(input).unwrap();
    assert!(
        output.contains("y: Option<f64>"),
        "type literal method optional param must be Option<f64>, got:\n{output}"
    );
}

/// I-040: `Partial<{ name?: T }>` で `Option<Option<T>>` にならず単一 `Option<T>` になる。
#[test]
fn test_partial_of_optional_field_no_double_wrap() {
    let input = r#"
type Base = { name?: string };
type P = Partial<Base>;
function runit(p: P): string {
    return "hi";
}
"#;
    let output = transpile(input).unwrap();
    assert!(
        !output.contains("Option<Option<"),
        "Partial<{{ name?: T }}> must not produce Option<Option<_>>, got:\n{output}"
    );
}

/// I-040 / callable interface: `(y?: number): void` の optional param は `Option<T>`。
/// trait 宣言と const callable の caller の両方を検証する (caller は call_0 ディスパッチ
/// + None auto-fill が機能することを保証)。
#[test]
fn test_callable_interface_optional_param_compiles() {
    let input = r#"
interface Handler {
    (x: number, y?: number): void;
}
const h: Handler = (x: number, y?: number): void => {};
function runit(): void {
    h(1);
}
"#;
    let output = transpile(input).unwrap();
    assert!(
        output.contains("fn call_0(&self, x: f64, y: Option<f64>)"),
        "callable interface optional param must be Option<f64>, got:\n{output}"
    );
    assert!(
        output.contains("h.call_0(1.0, None)") || output.contains("h.call_0(1_f64, None)"),
        "callable interface caller must dispatch to call_0 and fill None, got:\n{output}"
    );
}

/// I-040: generic fn type alias (`type Callback<T> = (x: T, y?: T) => T`) でも
/// optional param が `Option<T>` 化され、caller で fill-None が機能する。
#[test]
fn test_generic_fn_type_alias_optional_param_compiles() {
    let input = r#"
type Callback<T> = (x: T, y?: T) => T;
function runit(f: Callback<number>): number {
    return f(1);
}
"#;
    let output = transpile(input).unwrap();
    assert!(
        output.contains("Option<"),
        "generic fn type alias optional param must produce Option<_>, got:\n{output}"
    );
    assert!(
        output.contains("f(1.0, None)") || output.contains("f(1_f64, None)"),
        "caller of generic fn type alias must fill None for omitted optional, got:\n{output}"
    );
}
