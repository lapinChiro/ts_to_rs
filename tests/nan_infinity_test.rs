//! I-378 integration test: TS `NaN` / `Infinity` must be structurally lowered
//! to `Expr::PrimitiveAssocConst { F64, "NAN"|"INFINITY" }` and rendered as
//! `f64::NAN` / `f64::INFINITY` paths, WITHOUT registering `f64` as a user type.

use ts_to_rs::pipeline::transpile_single;

#[test]
fn ts_nan_lowers_to_f64_nan_path() {
    let ts_source = r#"function f(): number { return NaN; }"#;
    let rust = transpile_single(ts_source).expect("transpile must succeed");
    assert!(
        rust.contains("f64::NAN"),
        "expected `f64::NAN` in output, got:\n{rust}"
    );
}

#[test]
fn ts_infinity_lowers_to_f64_infinity_path() {
    let ts_source = r#"function g(): number { return Infinity; }"#;
    let rust = transpile_single(ts_source).expect("transpile must succeed");
    assert!(
        rust.contains("f64::INFINITY"),
        "expected `f64::INFINITY` in output, got:\n{rust}"
    );
}

#[test]
fn parse_int_fallback_uses_f64_nan() {
    // `parseInt(s)` lowers to `s.parse::<f64>().unwrap_or(f64::NAN)`. The
    // `f64::NAN` argument is the I-378 PrimitiveAssocConst form.
    let ts_source = r#"function h(): number { return parseInt("42"); }"#;
    let rust = transpile_single(ts_source).expect("transpile must succeed");
    assert!(
        rust.contains("f64::NAN"),
        "expected `f64::NAN` fallback in parseInt output, got:\n{rust}"
    );
}
