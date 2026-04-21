mod classes;
mod enums;
mod error_handling;
mod imports_and_exports;
mod module_items;
mod variable_type_propagation;

use super::*;
use crate::ir::Stmt;
use crate::ir::{BinOp, Expr, Param, RustType, StructField, Visibility};
use crate::parser::parse_typescript;
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::TypeRegistry;
use crate::transformer::test_fixtures::TctxFixture;
use crate::transformer::Transformer;

// -- marker_struct_name tests --

#[test]
fn marker_name_pascalcase_lowercase_value() {
    assert_eq!(
        Transformer::marker_struct_name("GetCookie", "getCookie"),
        "GetCookieGetCookieImpl"
    );
}

#[test]
fn marker_name_short_value() {
    // single word → 先頭大文字化
    assert_eq!(
        Transformer::marker_struct_name("GetCookie", "g1"),
        "GetCookieG1Impl"
    );
}

#[test]
fn marker_name_pascalcase_snake_value() {
    assert_eq!(
        Transformer::marker_struct_name("Handler", "request_handler"),
        "HandlerRequestHandlerImpl"
    );
}

#[test]
fn marker_name_distinct_for_distinct_values() {
    let name1 = Transformer::marker_struct_name("I", "foo");
    let name2 = Transformer::marker_struct_name("I", "bar");
    assert_ne!(name1, name2);
}

#[test]
fn marker_name_collision_suffix_loop() {
    let f = TctxFixture::from_source("const x = 1;");
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);

    // "a" and "A" both become PascalCase "A" → same base "IAImpl"
    let base1 = Transformer::marker_struct_name("I", "a");
    let base2 = Transformer::marker_struct_name("I", "A");
    assert_eq!(base1, base2); // both "IAImpl"

    let alloc1 = t.allocate_marker_name(&base1);
    let alloc2 = t.allocate_marker_name(&base2);
    assert_eq!(alloc1, "IAImpl");
    assert_eq!(alloc2, "IAImpl1"); // collision → suffix
}

// -- spawn_nested_scope factory method tests --

#[test]
fn spawn_nested_scope_can_convert_expr() {
    let f = TctxFixture::from_source("const x = 42;");
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let mut t = Transformer::for_module(&tctx, &mut synthetic);

    let mut sub = t.spawn_nested_scope();
    let lit = swc_ecma_ast::Number {
        span: swc_common::DUMMY_SP,
        value: 42.0,
        raw: None,
    };
    let result = sub.convert_expr(&swc_ecma_ast::Expr::Lit(swc_ecma_ast::Lit::Num(lit)));
    assert!(result.is_ok());
    assert!(matches!(result.unwrap(), Expr::NumberLit(v) if v == 42.0));
}

#[test]
fn spawn_nested_scope_with_local_synthetic_uses_local() {
    let f = TctxFixture::from_source("const x = 1;");
    let tctx = f.tctx();
    let mut parent_synthetic = SyntheticTypeRegistry::new();
    let t = Transformer::for_module(&tctx, &mut parent_synthetic);

    let mut local_synthetic = SyntheticTypeRegistry::new();
    let mut sub = t.spawn_nested_scope_with_local_synthetic(&mut local_synthetic);

    // sub-Transformer が convert_expr を呼べることを確認
    let lit = swc_ecma_ast::Number {
        span: swc_common::DUMMY_SP,
        value: 1.0,
        raw: None,
    };
    let result = sub.convert_expr(&swc_ecma_ast::Expr::Lit(swc_ecma_ast::Lit::Num(lit)));
    assert!(result.is_ok());
}

// Option IR builder tests are colocated with the builders themselves in
// `src/transformer/helpers/option_builders.rs` for cohesion (moved T6-6
// 2026-04-21).
