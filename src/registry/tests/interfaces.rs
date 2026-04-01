use super::*;
use crate::pipeline::SyntheticTypeRegistry;

use swc_common::sync::Lrc;
use swc_common::{FileName, SourceMap};
use swc_ecma_ast as ast;
use swc_ecma_parser::{Parser, StringInput, Syntax, TsSyntax};

/// TypeScript の interface 宣言をパースして `TsInterfaceDecl` を返す。
fn parse_interface(source: &str) -> ast::TsInterfaceDecl {
    let cm: Lrc<SourceMap> = Lrc::new(SourceMap::default());
    let fm = cm.new_source_file(FileName::Anon.into(), source.to_string());
    let mut parser = Parser::new(
        Syntax::Typescript(TsSyntax::default()),
        StringInput::from(&*fm),
        None,
    );
    let module = parser.parse_module().unwrap();
    for item in module.body {
        if let ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::TsInterface(iface))) = item {
            return *iface;
        }
    }
    panic!("no TsInterfaceDecl found in source: {source}");
}

/// パースした interface から最初の `TsPropertySignature` を取り出す。
fn extract_first_property(iface: &ast::TsInterfaceDecl) -> &ast::TsPropertySignature {
    for member in &iface.body.body {
        if let ast::TsTypeElement::TsPropertySignature(prop) = member {
            return prop;
        }
    }
    panic!("no TsPropertySignature found");
}

// ── collect_interface_fields ──

#[test]
fn test_collect_interface_fields_property_signatures_collected() {
    let iface = parse_interface("interface I { x: number; y: string }");
    let reg = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();

    let fields =
        super::super::interfaces::collect_interface_fields(&iface, &reg, &mut synthetic).unwrap();

    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0], ("x".to_string(), RustType::F64).into());
    assert_eq!(fields[1], ("y".to_string(), RustType::String).into());
}

#[test]
fn test_collect_interface_fields_non_property_members_skipped() {
    let iface = parse_interface("interface I { x: number; foo(a: string): void; y: string }");
    let reg = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();

    let fields =
        super::super::interfaces::collect_interface_fields(&iface, &reg, &mut synthetic).unwrap();

    // method signature should be skipped — only property signatures collected
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name, "x");
    assert_eq!(fields[1].name, "y");
}

// ── collect_interface_signatures ──

#[test]
fn test_collect_interface_signatures_ident_param_with_type_collected() {
    let iface = parse_interface("interface I { foo(x: string): number }");
    let reg = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();

    let sigs = super::super::interfaces::collect_interface_signatures(&iface, &reg, &mut synthetic);

    let sigs = sigs.methods.get("foo").expect("foo method should exist");
    assert_eq!(sigs.len(), 1);
    assert_eq!(
        sigs[0].params,
        vec![("x".to_string(), RustType::String).into()]
    );
    assert_eq!(sigs[0].return_type, Some(RustType::F64));
    assert!(!sigs[0].has_rest);
}

#[test]
fn test_collect_interface_signatures_rest_param_collected() {
    let iface = parse_interface("interface I { foo(...args: string[]): void }");
    let reg = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();

    let sigs = super::super::interfaces::collect_interface_signatures(&iface, &reg, &mut synthetic);

    let sigs = sigs.methods.get("foo").expect("foo method should exist");
    assert_eq!(sigs.len(), 1);
    assert!(sigs[0].has_rest, "has_rest should be true for rest param");
    assert_eq!(sigs[0].params.len(), 1);
    assert_eq!(sigs[0].params[0].name, "args");
    assert_eq!(
        sigs[0].params[0].ty,
        RustType::Vec(Box::new(RustType::String))
    );
}

#[test]
fn test_collect_interface_signatures_overload_accumulates() {
    let iface = parse_interface("interface I { foo(x: string): number; foo(x: number): string; }");
    let reg = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();

    let sigs = super::super::interfaces::collect_interface_signatures(&iface, &reg, &mut synthetic);

    let sigs = sigs.methods.get("foo").expect("foo method should exist");
    assert_eq!(sigs.len(), 2, "overloaded methods should accumulate in Vec");
    // First overload: (string) -> number
    assert_eq!(sigs[0].params[0].ty, RustType::String);
    assert_eq!(sigs[0].return_type, Some(RustType::F64));
    // Second overload: (number) -> string
    assert_eq!(sigs[1].params[0].ty, RustType::F64);
    assert_eq!(sigs[1].return_type, Some(RustType::String));
}

// ── collect_property_signature ──

#[test]
fn test_collect_property_signature_optional_wraps_in_option() {
    let iface = parse_interface("interface I { x?: number }");
    let prop = extract_first_property(&iface);
    let reg = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();

    let result = super::super::interfaces::collect_property_signature(prop, &reg, &mut synthetic);

    let field = result.expect("should return Some for optional property");
    assert_eq!(field.name, "x");
    assert_eq!(field.ty, RustType::Option(Box::new(RustType::F64)));
}

#[test]
fn test_collect_property_signature_non_ident_key_returns_none() {
    // Numeric literal key — SWC parses as Lit(Num), not Ident
    let iface_decl = parse_interface("interface I { 0: number }");
    let prop = extract_first_property(&iface_decl);
    let reg = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();

    let result = super::super::interfaces::collect_property_signature(prop, &reg, &mut synthetic);

    assert!(
        result.is_none(),
        "non-ident key should return None, got {result:?}"
    );
}

// ── is_callable_only ──

#[test]
fn test_is_callable_only_call_sigs_only_returns_true() {
    let iface =
        parse_interface("interface I { (x: string): number; (x: string, y: number): string }");
    assert!(super::super::interfaces::is_callable_only(&iface.body.body));
}

#[test]
fn test_is_callable_only_empty_returns_false() {
    // G8: 0 call signatures → false
    let iface = parse_interface("interface I {}");
    assert!(!super::super::interfaces::is_callable_only(
        &iface.body.body
    ));
}

#[test]
fn test_is_callable_only_mixed_call_sig_and_property_returns_false() {
    // G9: call sig + property → false
    let iface = parse_interface("interface I { (x: string): number; name: string }");
    assert!(!super::super::interfaces::is_callable_only(
        &iface.body.body
    ));
}

// ── call signature collection (G1) ──

#[test]
fn test_collect_interface_signatures_call_signature() {
    let iface = parse_interface("interface I { (x: string): number }");
    let reg = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();

    let sigs = super::super::interfaces::collect_interface_signatures(&iface, &reg, &mut synthetic);

    assert!(sigs.methods.is_empty());
    assert_eq!(sigs.call_signatures.len(), 1);
    assert_eq!(
        sigs.call_signatures[0].params,
        vec![("x".to_string(), RustType::String).into()]
    );
    assert_eq!(sigs.call_signatures[0].return_type, Some(RustType::F64));
}

#[test]
fn test_collect_interface_signatures_multiple_call_signatures() {
    let iface =
        parse_interface("interface I { (x: string): number; (x: string, y: number): string }");
    let reg = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();

    let sigs = super::super::interfaces::collect_interface_signatures(&iface, &reg, &mut synthetic);

    assert_eq!(sigs.call_signatures.len(), 2);
}

// ── construct signature collection (G2) ──

#[test]
fn test_collect_interface_signatures_construct_signature() {
    let iface = parse_interface("interface I { new (x: string): I }");
    let reg = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();

    let sigs = super::super::interfaces::collect_interface_signatures(&iface, &reg, &mut synthetic);

    assert!(sigs.methods.is_empty());
    assert!(sigs.call_signatures.is_empty());
    let ctor = sigs.constructor.expect("constructor should be Some");
    assert_eq!(ctor.len(), 1);
    assert_eq!(
        ctor[0].params,
        vec![("x".to_string(), RustType::String).into()]
    );
}

#[test]
fn test_collect_interface_signatures_no_construct_returns_none() {
    let iface = parse_interface("interface I { foo(): void }");
    let reg = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();

    let sigs = super::super::interfaces::collect_interface_signatures(&iface, &reg, &mut synthetic);

    assert!(sigs.constructor.is_none());
}
