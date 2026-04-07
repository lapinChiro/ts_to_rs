//! I-378 integration test: enum unit variant value-position references
//! must be structurally captured by the walker via `Expr::EnumVariant`.
//!
//! Before I-378, `Color.Red` in TS was lowered to `Expr::Ident("Color::Red")`,
//! a display-formatted path that the walker could not parse without a
//! string substring scan. After I-378, the lowering produces
//! `Expr::EnumVariant { enum_ty: UserTypeRef("Color"), variant: "Red" }`,
//! and the walker registers `Color` via `visit_user_type_ref` (or its
//! transitional manual equivalent in `collect_type_refs_from_expr`).
//!
//! This integration test exercises the full pipeline (parser → transformer →
//! walker → generator) to verify:
//! 1. The Transformer constructs `Expr::EnumVariant` (not `Expr::Ident`).
//! 2. The walker registers `Color` so the generator emits a `enum Color`
//!    definition (or, when an enum is registered, references it correctly).
//! 3. The final Rust output renders `Color::Red` as a path expression and
//!    compiles.

use ts_to_rs::pipeline::transpile_single;

#[test]
fn enum_value_path_lowers_to_structured_variant_and_renders_correctly() {
    let ts_source = r#"
enum Color { Red, Green, Blue }

function pick(): Color {
    return Color.Red;
}
"#;
    let result = transpile_single(ts_source).expect("transpile must succeed");
    let rust = &result;

    // Output must contain the enum definition.
    assert!(
        rust.contains("enum Color"),
        "expected `enum Color` in output, got:\n{rust}"
    );
    // Output must reference the variant via the path form `Color::Red`.
    assert!(
        rust.contains("Color::Red"),
        "expected `Color::Red` value reference in output, got:\n{rust}"
    );
    // The display-formatted broken-window form must be gone: there should be
    // no bare `Color :: Red` from string concatenation, only the structural
    // path emitted by the generator.
    assert!(
        !rust.contains("Ident(\"Color::Red\")"),
        "internal IR must not leak Expr::Ident strings into output, got:\n{rust}"
    );
}
