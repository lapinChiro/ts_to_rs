//! SWC parser configuration contract tests.
//!
//! These tests lock the current accept/reject boundary of `parse_typescript()`
//! under `TsSyntax::default()`. They are intended as canaries for `swc_*`
//! dependency upgrades: if parser defaults or AST acceptance drift, these tests
//! should fail immediately before deeper transformer regressions become harder
//! to diagnose.

use swc_ecma_ast::{Decl, Expr, ModuleItem, Stmt};
use ts_to_rs::parser::parse_typescript;

#[test]
fn test_parser_accepts_import_type_syntax_under_current_defaults() {
    let source = "import type { Foo } from './foo';\nconst x = 1;\n";
    let module =
        parse_typescript(source).expect("import type should parse under current TsSyntax defaults");
    let Some(ModuleItem::ModuleDecl(swc_ecma_ast::ModuleDecl::Import(import))) =
        module.body.first()
    else {
        panic!("expected first module item to be ImportDecl");
    };
    assert!(
        import.type_only,
        "import type should set ImportDecl.type_only"
    );
}

#[test]
fn test_parser_accepts_satisfies_syntax_under_current_defaults() {
    let source = r#"
type Shape = { a: number };
const value = { a: 1 } satisfies Shape;
"#;
    let module =
        parse_typescript(source).expect("satisfies should parse under current TsSyntax defaults");
    let Some(ModuleItem::Stmt(Stmt::Decl(Decl::Var(var_decl)))) = module.body.get(1) else {
        panic!("expected second module item to be variable declaration");
    };
    let init = var_decl.decls[0]
        .init
        .as_deref()
        .expect("const value should have initializer");
    assert!(
        matches!(init, Expr::TsSatisfies(_)),
        "satisfies initializer should parse as Expr::TsSatisfies, got {init:?}"
    );
}

#[test]
fn test_parser_accepts_as_const_syntax_under_current_defaults() {
    let source = "const PHASE = { A: 1, B: 2 } as const;\n";
    let module =
        parse_typescript(source).expect("as const should parse under current TsSyntax defaults");
    let Some(ModuleItem::Stmt(Stmt::Decl(Decl::Var(var_decl)))) = module.body.first() else {
        panic!("expected first module item to be variable declaration");
    };
    let init = var_decl.decls[0]
        .init
        .as_deref()
        .expect("const PHASE should have initializer");
    assert!(
        matches!(init, Expr::TsAs(_) | Expr::TsConstAssertion(_)),
        "as const initializer should stay in a type-wrapper AST shape, got {init:?}"
    );
}

#[test]
fn test_parser_rejects_decorator_syntax_under_current_defaults() {
    let source = "@sealed\nclass Foo {}\n";
    let err = parse_typescript(source).expect_err("decorators should stay disabled");
    let msg = err.to_string();
    assert!(
        msg.contains("Parse error"),
        "decorator rejection should surface as parser error, got: {msg}"
    );
}

#[test]
fn test_parser_rejects_jsx_syntax_under_current_defaults() {
    let source = "const el = <div />;\n";
    let err = parse_typescript(source).expect_err("JSX should stay disabled");
    let msg = err.to_string();
    assert!(
        msg.contains("Parse error"),
        "JSX rejection should surface as parser error, got: {msg}"
    );
}
