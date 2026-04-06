//! End-to-end integration test for I-375: lowercase class names must survive
//! a full `transpile_single` round-trip without falling back to a synthesized
//! stub or losing their reference.
//!
//! ## Why both a unit test and an integration test?
//!
//! The walker-layer unit test
//! (`test_walker_lowercase_class_name_registered_via_type_ref` in
//! `src/pipeline/external_struct_generator/tests.rs`) exercises the walker in
//! isolation with a hand-built IR that has `type_ref: Some("myClass")`. That
//! proves the walker's structural classification is correct *given* the
//! Transformer populates `type_ref` correctly.
//!
//! This integration test closes the loop by driving the full pipeline
//! (parser → transformer → walker → generator) on real TypeScript source,
//! verifying that (a) the Transformer constructs the right IR for a lowercase
//! class, (b) the walker registers `myClass` in the reference graph, and
//! (c) the final Rust output contains exactly one `struct myClass` definition
//! and a `myClass::new(...)` call referring to it. A regression in any of
//! those layers would manifest here as a missing struct, a duplicated stub,
//! or a compile error in the generated Rust.
//!
//! Before I-375, the walker used an "uppercase-head" heuristic on the joined
//! path string of `Expr::FnCall::name`, so `myClass::new(1)` was silently
//! dropped from the reference graph because its head segment was lowercase.
//! With the `CallTarget::Path { type_ref }` structural classification, the
//! Transformer records the reference at construction time based on a
//! `TypeRegistry` lookup, independent of naming conventions.

use ts_to_rs::pipeline::transpile_single;

#[test]
fn test_lowercase_class_name_is_converted_without_errors() {
    // A lowercase class name is unusual but legal TypeScript. The Transformer
    // must emit the corresponding Rust struct and the `new myClass(...)` call
    // must be wired to `myClass::new(...)`. Because the walker now consults
    // `CallTarget::type_ref` structurally, this conversion succeeds without
    // relying on uppercase-head heuristics.
    // Wrap the `new myClass(...)` call inside a function body so that the
    // Transformer actually emits the call expression (module-level `const`
    // initializers are handled separately).
    let source = r#"
class myClass {
  x: number;
  constructor(x: number) { this.x = x; }
}
function build(): myClass {
  return new myClass(1);
}
"#;

    let rust = transpile_single(source).expect("transpilation should succeed");

    // The user-defined type must appear as a Rust struct.
    assert!(
        rust.contains("struct myClass"),
        "expected `struct myClass` in output, got:\n{rust}"
    );
    // The constructor call must become `myClass::new(1)`.
    assert!(
        rust.contains("myClass::new("),
        "expected `myClass::new(` in output, got:\n{rust}"
    );
    // Crucially, the walker must not have synthesized a duplicate stub for
    // `myClass` — if the structural fix regresses to the uppercase heuristic,
    // we would either lose the reference entirely (producing a dangling
    // identifier in `let c = myClass::new(1)`) or emit a second empty struct.
    let struct_count = rust.matches("struct myClass").count();
    assert_eq!(
        struct_count, 1,
        "expected exactly one `struct myClass` definition, got {struct_count} in:\n{rust}"
    );
}
