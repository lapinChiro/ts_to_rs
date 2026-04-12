mod classes;
mod enums;
mod error_handling;
mod imports_and_exports;
mod module_items;
mod variable_type_propagation;

use super::*;
use crate::ir::CallTarget;
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

// -- build_option_unwrap_with_default tests --

/// Asserts the result is `unwrap_or` with the default as a direct argument (no closure).
fn assert_unwrap_or(result: &Expr) {
    match result {
        Expr::MethodCall { method, args, .. } => {
            assert_eq!(method, "unwrap_or");
            assert_eq!(args.len(), 1);
            assert!(
                !matches!(&args[0], Expr::Closure { .. }),
                "unwrap_or should receive the value directly, not a closure"
            );
        }
        other => panic!("expected MethodCall, got {:?}", other),
    }
}

/// Asserts the result is `unwrap_or_else` with a zero-arg closure wrapping the default.
fn assert_unwrap_or_else(result: &Expr) {
    match result {
        Expr::MethodCall { method, args, .. } => {
            assert_eq!(method, "unwrap_or_else");
            assert_eq!(args.len(), 1);
            match &args[0] {
                Expr::Closure { params, .. } => {
                    assert!(params.is_empty(), "closure should have no parameters");
                }
                other => panic!("expected Closure argument, got {:?}", other),
            }
        }
        other => panic!("expected MethodCall, got {:?}", other),
    }
}

#[test]
fn test_build_option_unwrap_number_lit_uses_unwrap_or() {
    let result =
        build_option_unwrap_with_default(Expr::Ident("x".to_string()), Expr::NumberLit(0.0));
    assert_unwrap_or(&result);
}

#[test]
fn test_build_option_unwrap_int_lit_uses_unwrap_or() {
    let result = build_option_unwrap_with_default(Expr::Ident("x".to_string()), Expr::IntLit(42));
    assert_unwrap_or(&result);
}

#[test]
fn test_build_option_unwrap_bool_lit_uses_unwrap_or() {
    let result =
        build_option_unwrap_with_default(Expr::Ident("x".to_string()), Expr::BoolLit(false));
    assert_unwrap_or(&result);
}

#[test]
fn test_build_option_unwrap_string_lit_uses_unwrap_or_else() {
    let result = build_option_unwrap_with_default(
        Expr::Ident("x".to_string()),
        Expr::StringLit("hello".to_string()),
    );
    assert_unwrap_or_else(&result);
}

#[test]
fn test_build_option_unwrap_fn_call_uses_unwrap_or_else() {
    let result = build_option_unwrap_with_default(
        Expr::Ident("x".to_string()),
        Expr::FnCall {
            target: CallTarget::Free("compute_default".to_string()),
            args: vec![],
        },
    );
    assert_unwrap_or_else(&result);
}

#[test]
fn test_build_option_unwrap_struct_init_uses_unwrap_or_else() {
    let result = build_option_unwrap_with_default(
        Expr::Ident("config".to_string()),
        Expr::StructInit {
            name: "Config".to_string(),
            fields: vec![("port".to_string(), Expr::NumberLit(8080.0))],
            base: None,
        },
    );
    assert_unwrap_or_else(&result);
}

#[test]
fn test_build_option_unwrap_method_call_uses_unwrap_or_else() {
    let result = build_option_unwrap_with_default(
        Expr::Ident("x".to_string()),
        Expr::MethodCall {
            object: Box::new(Expr::Ident("obj".to_string())),
            method: "get_default".to_string(),
            args: vec![],
        },
    );
    assert_unwrap_or_else(&result);
}

#[test]
fn test_build_option_unwrap_ident_uses_unwrap_or_else() {
    let result = build_option_unwrap_with_default(
        Expr::Ident("x".to_string()),
        Expr::Ident("fallback".to_string()),
    );
    assert_unwrap_or_else(&result);
}
