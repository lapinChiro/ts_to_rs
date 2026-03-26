mod arrays;
mod arrows;
mod binary_unary;
mod builtins;
mod calls;
mod enums;
mod expected_type;
mod fn_exprs;
mod literals;
mod math_number;
mod member_access;
mod objects;
mod optional_chaining;
mod optional_semantics;
mod regex;
mod strings;
mod ternary;
mod type_guards;
mod update_exprs;

use super::*;
use crate::ir::{BinOp, MatchPattern, UnOp};
use crate::parser::parse_typescript;
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{MethodSignature, TypeDef, TypeRegistry};
use crate::transformer::test_fixtures::TctxFixture;
use crate::transformer::Transformer;
use swc_ecma_ast::{Decl, ModuleItem, Stmt};

/// Helper: parse a TS expression statement and return the SWC Expr.
fn parse_expr(source: &str) -> ast::Expr {
    let module = parse_typescript(source).expect("parse failed");
    match &module.body[0] {
        ModuleItem::Stmt(Stmt::Expr(expr_stmt)) => *expr_stmt.expr.clone(),
        _ => panic!("expected expression statement"),
    }
}

/// Helper: parse a variable declaration initializer expression.
fn parse_var_init(source: &str) -> ast::Expr {
    let module = parse_typescript(source).expect("parse failed");
    extract_var_init(&module)
}

/// Extract variable declaration initializer from a pre-parsed Module.
///
/// `TctxFixture::from_source()` で構築した fixture の `module()` と組み合わせることで、
/// TypeResolver が設定した expected type と同じ span を持つ式を取得できる。
fn extract_var_init(module: &ast::Module) -> ast::Expr {
    extract_var_init_at(module, 0)
}

/// Extract variable declaration initializer at a given statement index.
fn extract_var_init_at(module: &ast::Module, index: usize) -> ast::Expr {
    match &module.body[index] {
        ModuleItem::Stmt(Stmt::Decl(Decl::Var(var_decl))) => {
            let init = var_decl.decls[0].init.as_ref().expect("no initializer");
            *init.clone()
        }
        _ => panic!("expected variable declaration at index {index}"),
    }
}

/// Extract an expression statement from the module at a given index.
fn extract_expr_stmt(module: &ast::Module, index: usize) -> ast::Expr {
    match &module.body[index] {
        ModuleItem::Stmt(Stmt::Expr(expr_stmt)) => *expr_stmt.expr.clone(),
        _ => panic!("expected expression statement at index {index}"),
    }
}

/// Extract an expression statement from a function body.
///
/// `fn_index` selects which function in the module, `stmt_index` selects which
/// statement in the function body. The statement must be an expression statement.
fn extract_fn_body_expr_stmt(
    module: &ast::Module,
    fn_index: usize,
    stmt_index: usize,
) -> ast::Expr {
    let fn_decl = match &module.body[fn_index] {
        ModuleItem::Stmt(Stmt::Decl(Decl::Fn(f))) => f,
        _ => panic!("expected function declaration at module index {fn_index}"),
    };
    let body = fn_decl
        .function
        .body
        .as_ref()
        .expect("function has no body");
    match &body.stmts[stmt_index] {
        ast::Stmt::Expr(expr_stmt) => *expr_stmt.expr.clone(),
        _ => panic!("expected expression statement at stmt index {stmt_index}"),
    }
}

/// Extract a variable initializer from a function body.
fn extract_fn_body_var_init(module: &ast::Module, fn_index: usize, stmt_index: usize) -> ast::Expr {
    let fn_decl = match &module.body[fn_index] {
        ModuleItem::Stmt(Stmt::Decl(Decl::Fn(f))) => f,
        _ => panic!("expected function declaration at module index {fn_index}"),
    };
    let body = fn_decl
        .function
        .body
        .as_ref()
        .expect("function has no body");
    match &body.stmts[stmt_index] {
        ast::Stmt::Decl(Decl::Var(var_decl)) => {
            let init = var_decl.decls[0].init.as_ref().expect("no initializer");
            *init.clone()
        }
        _ => panic!("expected variable declaration at stmt index {stmt_index}"),
    }
}

/// Build a TypeRegistry with a `greet(name: String, greeting: Option<String>) -> String` function.
fn greet_registry() -> TypeRegistry {
    let mut reg = TypeRegistry::new();
    reg.register(
        "greet".to_string(),
        TypeDef::Function {
            params: vec![
                ("name".to_string(), RustType::String),
                (
                    "greeting".to_string(),
                    RustType::Option(Box::new(RustType::String)),
                ),
            ],
            return_type: Some(RustType::String),
            has_rest: false,
        },
    );
    reg
}
