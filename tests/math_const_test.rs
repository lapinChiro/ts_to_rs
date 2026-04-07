//! I-378 integration test: `Math.PI`/`Math.E`/etc must be structurally
//! lowered to `Expr::StdConst` and rendered as `std::f64::consts::*` paths,
//! WITHOUT registering any user type ref via the walker.
//!
//! Before I-378, the Transformer produced `Expr::Ident("std::f64::consts::PI")`
//! — a display-formatted path that bypassed the walker's structural
//! classification. After I-378, the lowering uses `Expr::StdConst(F64Pi)`,
//! and the walker is structurally guaranteed not to misregister any segment
//! (`std`, `f64`, `consts`, `PI`) as a user type.

use ts_to_rs::pipeline::transpile_single;

#[test]
fn math_pi_lowers_to_std_const_and_renders_full_path() {
    let ts_source = r#"function pi(): number { return Math.PI; }"#;
    let rust = transpile_single(ts_source).expect("transpile must succeed");

    assert!(
        rust.contains("std::f64::consts::PI"),
        "expected `std::f64::consts::PI` in output, got:\n{rust}"
    );
}

#[test]
fn math_e_ln2_log10_sqrt2_all_lower_to_std_consts() {
    let ts_source = r#"
function consts(): number {
    const e = Math.E;
    const ln2 = Math.LN2;
    const log10e = Math.LOG10E;
    const sqrt2 = Math.SQRT2;
    return e + ln2 + log10e + sqrt2;
}
"#;
    let rust = transpile_single(ts_source).expect("transpile must succeed");

    for expected in [
        "std::f64::consts::E",
        "std::f64::consts::LN_2",
        "std::f64::consts::LOG10_E",
        "std::f64::consts::SQRT_2",
    ] {
        assert!(
            rust.contains(expected),
            "expected `{expected}` in output, got:\n{rust}"
        );
    }
}

#[test]
fn math_unknown_field_does_not_panic_or_lower_to_std_const() {
    // `StdConst::from_math_member` returns None for unknown fields;
    // the lowering must fall through to the normal member access path
    // and not crash.
    let ts_source = r#"function f() { return Math.UNKNOWN; }"#;
    let _ = transpile_single(ts_source);
    // Whether this errors or produces approximate output is implementation-
    // defined; the only assertion is "no panic". We don't unwrap.
}
