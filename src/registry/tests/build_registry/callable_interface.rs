//! Callable interface classification + Pass 2a/2b split verification.
//!
//! Covers:
//! - Interface with call signatures → `call_signatures` populated
//! - Interface with construct signature → `constructor` populated
//! - Pass 2b forward-declared callable interface resolution (`const
//!   handler: Handler = ...` before `interface Handler { (...): ...; }`)
//! - Non-callable interface arrow const remains `TypeDef::Function`

use super::*;

// ── callable interface → call_signatures registration ──

#[test]
fn test_callable_interface_registers_call_signatures() {
    let module = parse_typescript(
        "interface GetCookie { (c: string): string; (c: string, key: string): number }",
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg
        .get("GetCookie")
        .expect("GetCookie should be registered");
    if let TypeDef::Struct {
        call_signatures, ..
    } = def
    {
        assert_eq!(call_signatures.len(), 2, "should have 2 call signatures");
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

#[test]
fn test_interface_with_construct_signature_registers_constructor() {
    let module =
        parse_typescript("interface Builder { new (config: string): Builder; build(): void }")
            .unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Builder").expect("Builder should be registered");
    if let TypeDef::Struct {
        constructor,
        methods,
        ..
    } = def
    {
        assert!(constructor.is_some(), "constructor should be present");
        assert!(
            methods.contains_key("build"),
            "methods should contain build"
        );
    } else {
        panic!("expected TypeDef::Struct, got {def:?}");
    }
}

// ── Pass 2a/2b split verification ──

#[test]
fn test_pass_2b_sees_forward_declared_callable_interface() {
    // const handler: Handler = ... が先に書かれ、
    // interface Handler が後方で宣言されている場合、
    // Pass 2b で handler の型注釈 "Handler" が正しく解決できる
    let source = r#"
        const handler: Handler = (req: string): string => req;
        interface Handler {
            (req: string): string;
        }
    "#;
    let module = parse_typescript(source).unwrap();
    let reg = build_registry(&module);

    // Handler interface は Pass 2a で解決済
    let handler_def = reg.get("Handler").expect("Handler should be registered");
    if let TypeDef::Struct {
        call_signatures, ..
    } = handler_def
    {
        assert_eq!(call_signatures.len(), 1, "Handler should have 1 call sig");
    } else {
        panic!("expected TypeDef::Struct for Handler, got {handler_def:?}");
    }

    // handler const は Pass 2b で ConstValue として登録される (P2.4)
    let handler_const = reg.get("handler").expect("handler should be registered");
    if let TypeDef::ConstValue {
        type_ref_name: Some(ref_name),
        ..
    } = handler_const
    {
        assert_eq!(ref_name, "Handler");
    } else {
        panic!(
            "expected TypeDef::ConstValue with type_ref_name for handler, got {handler_const:?}"
        );
    }
}

#[test]
fn test_non_callable_interface_arrow_remains_function() {
    // 非 callable interface の型注釈を持つ arrow は従来通り Function として登録
    let source = r#"
        interface Config {
            name: string;
        }
        const factory: Config = (x: string): string => x;
    "#;
    let module = parse_typescript(source).unwrap();
    let reg = build_registry(&module);

    let factory = reg.get("factory").expect("factory should be registered");
    assert!(
        matches!(factory, TypeDef::Function { .. }),
        "non-callable interface arrow should be TypeDef::Function, got {factory:?}"
    );
}
