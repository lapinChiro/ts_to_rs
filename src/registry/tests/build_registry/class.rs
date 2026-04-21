//! Class registration tests.
//!
//! Covers class-specific decl paths:
//! - Class with only a constructor → registered with `constructor: Some(...)`
//! - Class with mixed members (field + constructor + method) → all three
//!   are captured in the `TypeDef::Struct` storage

use super::*;

// ── class with constructor ──

#[test]
fn test_class_with_only_constructor_is_registered() {
    let module = parse_typescript(
        r#"
        class Handler {
            constructor(name: string, count: number) {}
        }
        "#,
    )
    .unwrap();

    let reg = build_registry(&module);
    let def = reg.get("Handler");
    assert!(
        def.is_some(),
        "class with only a constructor should be registered in TypeRegistry"
    );
    if let Some(TypeDef::Struct { constructor, .. }) = def {
        assert!(
            constructor.is_some(),
            "constructor signature should be present"
        );
    } else {
        panic!("expected TypeDef::Struct");
    }
}

// ── class: mixed members (field + constructor + method) ──

#[test]
fn test_class_with_fields_constructor_and_methods() {
    let module = parse_typescript(
        r#"
        class Service {
            name: string;
            constructor(n: string) {}
            process(input: number): boolean { return true; }
        }
        "#,
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Service").expect("Service should be registered");
    if let TypeDef::Struct {
        fields,
        constructor,
        methods,
        ..
    } = def
    {
        // field
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "name");
        assert_eq!(fields[0].ty, RustType::String);

        // constructor
        let ctor = constructor.as_ref().expect("constructor should be present");
        assert_eq!(ctor[0].params.len(), 1);
        assert_eq!(ctor[0].params[0].name, "n");
        assert_eq!(ctor[0].params[0].ty, RustType::String);

        // method
        let process_sigs = methods.get("process").expect("process method");
        assert_eq!(process_sigs[0].params[0].ty, RustType::F64);
        assert_eq!(process_sigs[0].return_type, Some(RustType::Bool));
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}
