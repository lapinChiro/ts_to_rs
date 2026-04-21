//! Member / index / await rendering when the receiver shape requires
//! explicit parens (prefix operators vs postfix `.field` / `.method()`
//! / `[index]` / `.await`). Covers:
//!
//! - `FieldAccess` / `MethodCall` / `Index` / `Await` on `Deref`/`Ref`
//!   receivers → `(*x).field`, `(&x).method()`, `(*x)[0]`, `(*x).await`
//! - `MethodCall` receiver shapes that need parens (BinaryOp, UnaryOp,
//!   Cast) vs those that don't (Ident, chained MethodCall, FnCall)
//! - `static_method_call` uses `::`, `instance_method_call` uses `.`
//! - `escape_ident` reserved-word rewrite for method names / field
//!   names / let bindings (`self` is exempt)
//! - `StructInit { base: Some(_) }` update syntax rendering

use super::*;
use crate::ir::{BinOp, CallTarget, Expr, RustType, Stmt, UnOp};

// --- Deref / Ref receiver paren tests ---

#[test]
fn test_generate_expr_field_access() {
    let expr = Expr::FieldAccess {
        object: Box::new(Expr::Ident("self".to_string())),
        field: "name".to_string(),
    };
    assert_eq!(generate_expr(&expr), "self.name");
}

#[test]
fn test_generate_expr_field_access_deref_receiver_adds_parens() {
    // `.` binds tighter than prefix `*`, so `*x.name` parses as `*(x.name)`.
    // For a dereffed receiver we must emit `(*x).name` to access the field
    // on the dereffed value. This is the generator contract that Step 2's
    // `deref_closure_params` relies on to produce compilable output.
    let expr = Expr::FieldAccess {
        object: Box::new(Expr::Deref(Box::new(Expr::Ident("x".to_string())))),
        field: "name".to_string(),
    };
    assert_eq!(generate_expr(&expr), "(*x).name");
}

#[test]
fn test_generate_expr_field_access_ref_receiver_adds_parens() {
    // `&x.name` parses as `&(x.name)` — borrow the field, not the struct.
    // For a borrow-wrapped receiver we must emit `(&x).name`.
    let expr = Expr::FieldAccess {
        object: Box::new(Expr::Ref(Box::new(Expr::Ident("x".to_string())))),
        field: "name".to_string(),
    };
    assert_eq!(generate_expr(&expr), "(&x).name");
}

#[test]
fn test_generate_expr_method_call_deref_receiver_adds_parens() {
    // `*x.method()` parses as `*(x.method())` — deref the result, not the receiver.
    // For a dereffed receiver we must emit `(*x).method()`.
    let expr = Expr::MethodCall {
        object: Box::new(Expr::Deref(Box::new(Expr::Ident("x".to_string())))),
        method: "len".to_string(),
        args: vec![],
    };
    assert_eq!(generate_expr(&expr), "(*x).len()");
}

#[test]
fn test_generate_expr_index_deref_receiver_adds_parens() {
    // `*x[0]` parses as `*(x[0])` — deref the element, not the receiver.
    // For a dereffed receiver we must emit `(*x)[0]`. This matters when
    // deref_closure_params wraps a param that is then indexed inside the body.
    let expr = Expr::Index {
        object: Box::new(Expr::Deref(Box::new(Expr::Ident("x".to_string())))),
        index: Box::new(Expr::NumberLit(0.0)),
    };
    assert_eq!(generate_expr(&expr), "(*x)[0]");
}

#[test]
fn test_generate_expr_index_ref_receiver_adds_parens() {
    let expr = Expr::Index {
        object: Box::new(Expr::Ref(Box::new(Expr::Ident("x".to_string())))),
        index: Box::new(Expr::NumberLit(0.0)),
    };
    assert_eq!(generate_expr(&expr), "(&x)[0]");
}

#[test]
fn test_generate_expr_await_deref_receiver_adds_parens() {
    // `*x.await` parses as `*(x.await)` — deref the awaited value. To await
    // a dereffed future we must emit `(*x).await`.
    let expr = Expr::Await(Box::new(Expr::Deref(Box::new(Expr::Ident(
        "x".to_string(),
    )))));
    assert_eq!(generate_expr(&expr), "(*x).await");
}

// --- MethodCall receiver shape paren rules ---

#[test]
fn test_generate_method_call_binary_op_receiver_needs_parens() {
    let expr = Expr::MethodCall {
        object: Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Ident("a".to_string())),
            op: BinOp::Add,
            right: Box::new(Expr::Ident("b".to_string())),
        }),
        method: "sqrt".to_string(),
        args: vec![],
    };
    assert_eq!(generate_expr(&expr), "(a + b).sqrt()");
}

#[test]
fn test_generate_method_call_unary_op_receiver_needs_parens() {
    let expr = Expr::MethodCall {
        object: Box::new(Expr::UnaryOp {
            op: UnOp::Neg,
            operand: Box::new(Expr::Ident("x".to_string())),
        }),
        method: "abs".to_string(),
        args: vec![],
    };
    assert_eq!(generate_expr(&expr), "(-x).abs()");
}

#[test]
fn test_generate_method_call_cast_receiver_needs_parens() {
    let expr = Expr::MethodCall {
        object: Box::new(Expr::Cast {
            expr: Box::new(Expr::Ident("x".to_string())),
            target: RustType::F64,
        }),
        method: "abs".to_string(),
        args: vec![],
    };
    assert_eq!(generate_expr(&expr), "(x as f64).abs()");
}

#[test]
fn test_generate_method_call_ident_receiver_no_parens() {
    let expr = Expr::MethodCall {
        object: Box::new(Expr::Ident("x".to_string())),
        method: "abs".to_string(),
        args: vec![],
    };
    assert_eq!(generate_expr(&expr), "x.abs()");
}

#[test]
fn test_generate_method_call_chain_no_parens() {
    let expr = Expr::MethodCall {
        object: Box::new(Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "foo".to_string(),
            args: vec![],
        }),
        method: "bar".to_string(),
        args: vec![],
    };
    assert_eq!(generate_expr(&expr), "x.foo().bar()");
}

#[test]
fn test_generate_method_call_fn_call_receiver_no_parens() {
    let expr = Expr::MethodCall {
        object: Box::new(Expr::FnCall {
            target: CallTarget::Free("foo".to_string()),
            args: vec![],
        }),
        method: "bar".to_string(),
        args: vec![],
    };
    assert_eq!(generate_expr(&expr), "foo().bar()");
}

// --- I-378: static method calls are now CallTarget::UserAssocFn (FnCall) ---
// MethodCall is strictly for instance method calls (`.method()` separator).

#[test]
fn test_generate_static_method_call_via_user_assoc_fn() {
    // I-378: `Foo::create(1)` is now `FnCall { UserAssocFn { ty: "Foo", method: "create" } }`,
    // not `MethodCall { Ident("Foo"), "create" }`. The generator's `is_type_ident`
    // uppercase heuristic was removed; classification is now structural at the
    // Transformer layer.
    let expr = Expr::FnCall {
        target: CallTarget::UserAssocFn {
            ty: crate::ir::UserTypeRef::new("Foo"),
            method: "create".to_string(),
        },
        args: vec![Expr::IntLit(1)],
    };
    assert_eq!(generate_expr(&expr), "Foo::create(1)");
}

#[test]
fn test_generate_instance_method_call_uses_dot() {
    let expr = Expr::MethodCall {
        object: Box::new(Expr::Ident("foo".to_string())),
        method: "create".to_string(),
        args: vec![],
    };
    assert_eq!(generate_expr(&expr), "foo.create()");
}

// --- Rust reserved word escape (method name / field / let / self-exempt) ---

#[test]
fn test_escape_ident_method_call_reserved_word_adds_r_hash() {
    let expr = Expr::MethodCall {
        object: Box::new(Expr::Ident("obj".to_string())),
        method: "match".to_string(),
        args: vec![Expr::Ident("x".to_string())],
    };
    assert_eq!(generate_expr(&expr), "obj.r#match(x)");
}

#[test]
fn test_escape_ident_let_reserved_word_adds_r_hash() {
    let stmt = Stmt::Let {
        mutable: false,
        name: "type".to_string(),
        ty: None,
        init: Some(Expr::NumberLit(1.0)),
    };
    let result = generate_stmt(&stmt, 0);
    assert!(result.contains("r#type"), "expected r#type in: {result}");
}

#[test]
fn test_escape_ident_field_access_reserved_word_adds_r_hash() {
    let expr = Expr::FieldAccess {
        object: Box::new(Expr::Ident("obj".to_string())),
        field: "match".to_string(),
    };
    assert_eq!(generate_expr(&expr), "obj.r#match");
}

#[test]
fn test_escape_ident_non_reserved_word_unchanged() {
    let expr = Expr::MethodCall {
        object: Box::new(Expr::Ident("obj".to_string())),
        method: "foo".to_string(),
        args: vec![Expr::Ident("x".to_string())],
    };
    assert_eq!(generate_expr(&expr), "obj.foo(x)");
}

#[test]
fn test_escape_ident_self_not_escaped() {
    let expr = Expr::FieldAccess {
        object: Box::new(Expr::Ident("self".to_string())),
        field: "x".to_string(),
    };
    assert_eq!(generate_expr(&expr), "self.x");
}

// --- StructInit update syntax ---

#[test]
fn test_generate_struct_init_with_base_renders_update_syntax() {
    let expr = Expr::StructInit {
        name: "Foo".to_string(),
        fields: vec![("key".to_string(), Expr::NumberLit(1.0))],
        base: Some(Box::new(Expr::Ident("other".to_string()))),
    };
    assert_eq!(generate_expr(&expr), "Foo { key: 1.0, ..other }");
}

#[test]
fn test_generate_struct_init_base_only_renders_update_syntax() {
    let expr = Expr::StructInit {
        name: "Foo".to_string(),
        fields: vec![],
        base: Some(Box::new(Expr::Ident("other".to_string()))),
    };
    assert_eq!(generate_expr(&expr), "Foo { ..other }");
}
