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

// I-205 T3: method.kind propagation tests (Method / Getter / Setter 区別が registry に
// 正しく反映されることを verify、call site dispatch の prerequisite)

#[test]
fn test_class_method_kind_default_method_is_propagated() {
    let module = parse_typescript(
        r#"
        class Box {
            do_work(): void {}
        }
        "#,
    )
    .unwrap();
    let reg = build_registry(&module);
    if let Some(TypeDef::Struct { methods, .. }) = reg.get("Box") {
        let sigs = methods.get("do_work").expect("do_work method");
        assert_eq!(sigs.len(), 1);
        assert_eq!(
            sigs[0].kind,
            crate::registry::MethodKind::Method,
            "regular method should have MethodKind::Method"
        );
    } else {
        panic!("expected TypeDef::Struct for Box");
    }
}

#[test]
fn test_class_method_kind_getter_is_propagated() {
    let module = parse_typescript(
        r#"
        class Box {
            _value: number = 0;
            get value(): number { return this._value; }
        }
        "#,
    )
    .unwrap();
    let reg = build_registry(&module);
    if let Some(TypeDef::Struct { methods, .. }) = reg.get("Box") {
        let sigs = methods.get("value").expect("value getter");
        assert_eq!(sigs.len(), 1);
        assert_eq!(
            sigs[0].kind,
            crate::registry::MethodKind::Getter,
            "`get value()` should have MethodKind::Getter"
        );
    } else {
        panic!("expected TypeDef::Struct for Box");
    }
}

#[test]
fn test_class_method_kind_setter_is_propagated() {
    let module = parse_typescript(
        r#"
        class Box {
            _value: number = 0;
            set value(v: number) { this._value = v; }
        }
        "#,
    )
    .unwrap();
    let reg = build_registry(&module);
    if let Some(TypeDef::Struct { methods, .. }) = reg.get("Box") {
        let sigs = methods.get("value").expect("value setter");
        assert_eq!(sigs.len(), 1);
        assert_eq!(
            sigs[0].kind,
            crate::registry::MethodKind::Setter,
            "`set value(v)` should have MethodKind::Setter"
        );
    } else {
        panic!("expected TypeDef::Struct for Box");
    }
}

#[test]
fn test_class_method_kind_getter_and_setter_pair_distinguished() {
    let module = parse_typescript(
        r#"
        class Box {
            _value: number = 0;
            get value(): number { return this._value; }
            set value(v: number) { this._value = v; }
        }
        "#,
    )
    .unwrap();
    let reg = build_registry(&module);
    if let Some(TypeDef::Struct { methods, .. }) = reg.get("Box") {
        let sigs = methods.get("value").expect("value accessors");
        assert_eq!(sigs.len(), 2, "getter + setter pair = 2 signatures");
        let kinds: Vec<crate::registry::MethodKind> = sigs.iter().map(|s| s.kind).collect();
        assert!(
            kinds.contains(&crate::registry::MethodKind::Getter),
            "getter signature must be present, got {kinds:?}"
        );
        assert!(
            kinds.contains(&crate::registry::MethodKind::Setter),
            "setter signature must be present, got {kinds:?}"
        );
    } else {
        panic!("expected TypeDef::Struct for Box");
    }
}
