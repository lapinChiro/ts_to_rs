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
            target: CallTarget::simple("compute_default"),
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
