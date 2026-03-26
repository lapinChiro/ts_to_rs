use super::*;

// --- declare module error propagation ---

#[test]
fn test_declare_module_inner_error_reported_in_resilient_mode() {
    // An unsupported declaration inside `declare module` should be reported, not silently dropped
    let source = r#"
        declare module 'test' {
            interface Valid { x: string; }
            using y = something;
        }
    "#;
    let module = parse_typescript(source).expect("parse failed");
    let (items, unsupported) = transform_module_collecting(&module, &TypeRegistry::new()).unwrap();
    // Valid interface should still be converted
    assert!(
        items
            .iter()
            .any(|i| matches!(i, Item::Struct { name, .. } if name == "Valid")),
        "valid interface should be converted: {items:?}"
    );
    // `using` declaration should be reported as unsupported
    assert!(
        !unsupported.is_empty(),
        "unsupported declaration inside declare module should be reported"
    );
}

#[test]
fn test_declare_module_inner_error_propagates_in_strict_mode() {
    // In strict mode (non-resilient), unsupported inside declare module should error
    let source = r#"
        declare module 'test' {
            using y = something;
        }
    "#;
    let module = parse_typescript(source).expect("parse failed");
    let result = transform_module(&module, &TypeRegistry::new());
    assert!(
        result.is_err(),
        "unsupported inside declare module should error in strict mode"
    );
}

// --- UnsupportedSyntaxError ---

#[test]
fn test_unsupported_syntax_error_new_creates_correct_instance() {
    use swc_common::{BytePos, Span};
    let span = Span::new(BytePos(42), BytePos(50));
    let err = super::super::UnsupportedSyntaxError::new("TestKind", span);
    assert_eq!(err.kind, "TestKind");
    assert_eq!(err.byte_pos, 42);
}

#[test]
fn test_unsupported_syntax_error_new_converts_to_anyhow() {
    use swc_common::{BytePos, Span};
    let span = Span::new(BytePos(10), BytePos(20));
    let err = super::super::UnsupportedSyntaxError::new("SomeError", span);
    let anyhow_err: anyhow::Error = err.into();
    let downcasted = anyhow_err
        .downcast::<super::super::UnsupportedSyntaxError>()
        .unwrap();
    assert_eq!(downcasted.kind, "SomeError");
    assert_eq!(downcasted.byte_pos, 10);
}
