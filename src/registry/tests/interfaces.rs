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
    assert_eq!(fields[0], ("x".to_string(), RustType::F64));
    assert_eq!(fields[1], ("y".to_string(), RustType::String));
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
    assert_eq!(fields[0].0, "x");
    assert_eq!(fields[1].0, "y");
}

// ── collect_interface_methods ──

#[test]
fn test_collect_interface_methods_ident_param_with_type_collected() {
    let iface = parse_interface("interface I { foo(x: string): number }");
    let reg = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();

    let methods = super::super::interfaces::collect_interface_methods(&iface, &reg, &mut synthetic);

    let sigs = methods.get("foo").expect("foo method should exist");
    assert_eq!(sigs.len(), 1);
    assert_eq!(sigs[0].params, vec![("x".to_string(), RustType::String)]);
    assert_eq!(sigs[0].return_type, Some(RustType::F64));
    assert!(!sigs[0].has_rest);
}

#[test]
fn test_collect_interface_methods_rest_param_collected() {
    let iface = parse_interface("interface I { foo(...args: string[]): void }");
    let reg = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();

    let methods = super::super::interfaces::collect_interface_methods(&iface, &reg, &mut synthetic);

    let sigs = methods.get("foo").expect("foo method should exist");
    assert_eq!(sigs.len(), 1);
    assert!(sigs[0].has_rest, "has_rest should be true for rest param");
    assert_eq!(sigs[0].params.len(), 1);
    assert_eq!(sigs[0].params[0].0, "args");
    assert_eq!(
        sigs[0].params[0].1,
        RustType::Vec(Box::new(RustType::String))
    );
}

#[test]
fn test_collect_interface_methods_overload_accumulates() {
    let iface = parse_interface("interface I { foo(x: string): number; foo(x: number): string; }");
    let reg = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();

    let methods = super::super::interfaces::collect_interface_methods(&iface, &reg, &mut synthetic);

    let sigs = methods.get("foo").expect("foo method should exist");
    assert_eq!(sigs.len(), 2, "overloaded methods should accumulate in Vec");
    // First overload: (string) -> number
    assert_eq!(sigs[0].params[0].1, RustType::String);
    assert_eq!(sigs[0].return_type, Some(RustType::F64));
    // Second overload: (number) -> string
    assert_eq!(sigs[1].params[0].1, RustType::F64);
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

    let (name, ty) = result.expect("should return Some for optional property");
    assert_eq!(name, "x");
    assert_eq!(ty, RustType::Option(Box::new(RustType::F64)));
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
