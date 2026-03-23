use super::*;
use crate::ir::{BinOp, MatchPattern, UnOp};
use crate::parser::parse_typescript;
use crate::registry::{MethodSignature, TypeDef, TypeRegistry};
use crate::transformer::test_fixtures::TctxFixture;
use crate::transformer::TypeEnv;
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
fn extract_fn_body_expr_stmt(module: &ast::Module, fn_index: usize, stmt_index: usize) -> ast::Expr {
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

#[test]
fn test_convert_expr_identifier() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("foo;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::Ident("foo".to_string()));
}

#[test]
fn test_convert_expr_number_literal() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("42;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::NumberLit(42.0));
}

// --- BigInt literal ---

#[test]
fn test_convert_expr_bigint_literal_generates_int_lit() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("123n;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::IntLit(123));
}

#[test]
fn test_convert_expr_bigint_zero_generates_int_lit() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("0n;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::IntLit(0));
}

#[test]
fn test_convert_expr_string_literal() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("\"hello\";");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::StringLit("hello".to_string()));
}

#[test]
fn test_convert_expr_bool_true() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("true;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn test_convert_expr_bool_false() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("false;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn test_convert_expr_binary_add() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const x = a + b;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::BinaryOp {
            left: Box::new(Expr::Ident("a".to_string())),
            op: BinOp::Add,
            right: Box::new(Expr::Ident("b".to_string())),
        }
    );
}

#[test]
fn test_convert_expr_binary_greater_than() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const x = a > b;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::BinaryOp {
            left: Box::new(Expr::Ident("a".to_string())),
            op: BinOp::Gt,
            right: Box::new(Expr::Ident("b".to_string())),
        }
    );
}

#[test]
fn test_convert_expr_binary_strict_equals() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const x = a === b;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::BinaryOp {
            left: Box::new(Expr::Ident("a".to_string())),
            op: BinOp::Eq,
            right: Box::new(Expr::Ident("b".to_string())),
        }
    );
}

#[test]
fn test_convert_expr_template_literal() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const x = `Hello ${name}`;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::FormatMacro {
            template: "Hello {}".to_string(),
            args: vec![Expr::Ident("name".to_string())],
        }
    );
}

#[test]
fn test_convert_expr_member_this_field() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("this.name;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::FieldAccess {
            object: Box::new(Expr::Ident("self".to_string())),
            field: "name".to_string(),
        }
    );
}

#[test]
fn test_convert_expr_member_non_this() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("obj.field;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::FieldAccess {
            object: Box::new(Expr::Ident("obj".to_string())),
            field: "field".to_string(),
        }
    );
}

// -- Arrow function (closure) tests --

#[test]
fn test_convert_expr_arrow_expr_body() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // `(x: number) => x + 1`
    let swc_expr = parse_var_init("const f = (x: number) => x + 1;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match result {
        Expr::Closure {
            params,
            return_type,
            body,
        } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
            assert_eq!(params[0].ty, Some(crate::ir::RustType::F64));
            assert!(return_type.is_none());
            assert!(matches!(body, crate::ir::ClosureBody::Expr(_)));
        }
        _ => panic!("expected Expr::Closure"),
    }
}

#[test]
fn test_convert_expr_arrow_block_body() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // `(x: number): number => { return x + 1; }`
    let swc_expr = parse_var_init("const f = (x: number): number => { return x + 1; };");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match result {
        Expr::Closure {
            params,
            return_type,
            body,
        } => {
            assert_eq!(params.len(), 1);
            assert!(return_type.is_some());
            assert_eq!(return_type.unwrap(), crate::ir::RustType::F64);
            assert!(matches!(body, crate::ir::ClosureBody::Block(_)));
        }
        _ => panic!("expected Expr::Closure"),
    }
}

#[test]
fn test_convert_expr_arrow_no_params() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const f = () => 42;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match result {
        Expr::Closure { params, body, .. } => {
            assert!(params.is_empty());
            assert!(matches!(body, crate::ir::ClosureBody::Expr(_)));
        }
        _ => panic!("expected Expr::Closure"),
    }
}

#[test]
fn test_convert_expr_arrow_no_type_annotation_param_ty_is_none() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const f = (x) => x + 1;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match result {
        Expr::Closure { params, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
            assert_eq!(params[0].ty, None);
        }
        _ => panic!("expected Expr::Closure"),
    }
}

#[test]
fn test_convert_expr_arrow_mixed_type_annotations() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Only first param has type annotation
    let swc_expr = parse_var_init("const f = (x: number, y) => x + y;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match result {
        Expr::Closure { params, .. } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].ty, Some(crate::ir::RustType::F64));
            assert_eq!(params[1].ty, None);
        }
        _ => panic!("expected Expr::Closure"),
    }
}

// -- Function call tests --

#[test]
fn test_convert_expr_call_simple() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("foo(x, y);");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::FnCall {
            name: "foo".to_string(),
            args: vec![Expr::Ident("x".to_string()), Expr::Ident("y".to_string()),],
        }
    );
}

#[test]
fn test_convert_expr_call_no_args() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("foo();");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::FnCall {
            name: "foo".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_call_nested() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("foo(bar(x));");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::FnCall {
            name: "foo".to_string(),
            args: vec![Expr::FnCall {
                name: "bar".to_string(),
                args: vec![Expr::Ident("x".to_string())],
            }],
        }
    );
}

#[test]
fn test_convert_expr_method_call() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("obj.method(x);");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("obj".to_string())),
            method: "method".to_string(),
            args: vec![Expr::Ident("x".to_string())],
        }
    );
}

#[test]
fn test_convert_expr_method_call_this() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("this.doSomething(x);");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("self".to_string())),
            method: "doSomething".to_string(),
            args: vec![Expr::Ident("x".to_string())],
        }
    );
}

#[test]
fn test_convert_expr_method_chain() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("a.b().c();");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("a".to_string())),
                method: "b".to_string(),
                args: vec![],
            }),
            method: "c".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_new() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("new Foo(x, y);");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::FnCall {
            name: "Foo::new".to_string(),
            args: vec![Expr::Ident("x".to_string()), Expr::Ident("y".to_string()),],
        }
    );
}

#[test]
fn test_convert_expr_new_no_args() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("new Foo();");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::FnCall {
            name: "Foo::new".to_string(),
            args: vec![],
        }
    );
}

// --- Constructor string arg gets .to_string() ---

#[test]
fn test_new_expr_string_arg_gets_to_string() {
    // new Foo("hello") with Foo { name: String } → Foo::new("hello".to_string())
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "Foo".to_string(),
        TypeDef::new_struct(
            vec![("name".to_string(), RustType::String)],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let source = r#"new Foo("hello");"#;
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 0);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match &result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "Foo::new");
            assert!(
                matches!(&args[0], Expr::MethodCall { method, .. } if method == "to_string"),
                "expected .to_string() on string arg, got {:?}",
                args[0]
            );
        }
        other => panic!("expected FnCall, got {other:?}"),
    }
}

#[test]
fn test_convert_expr_template_literal_no_exprs() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const x = `hello world`;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::FormatMacro {
            template: "hello world".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_array_numbers() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_var_init("const a = [1, 2, 3];");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Vec {
            elements: vec![
                Expr::NumberLit(1.0),
                Expr::NumberLit(2.0),
                Expr::NumberLit(3.0),
            ],
        }
    );
}

#[test]
fn test_convert_expr_array_strings() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_var_init(r#"const a = ["x", "y"];"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Vec {
            elements: vec![
                Expr::StringLit("x".to_string()),
                Expr::StringLit("y".to_string()),
            ],
        }
    );
}

#[test]
fn test_convert_expr_array_empty() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_var_init("const a = [];");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::Vec { elements: vec![] });
}

#[test]
fn test_convert_expr_array_single_element() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_var_init("const a = [42];");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Vec {
            elements: vec![Expr::NumberLit(42.0)],
        }
    );
}

// -- Object literal tests --

#[test]
fn test_convert_expr_object_literal_with_type_hint_basic() {
    // { x: 1, y: 2 } with expected Named("Point") from type annotation
    let f = TctxFixture::from_source("const p: Point = { x: 1, y: 2 };");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), Expr::NumberLit(1.0)),
                ("y".to_string(), Expr::NumberLit(2.0)),
            ],
            base: None,
        }
    );
}

#[test]
fn test_convert_expr_object_literal_mixed_field_types() {
    let f =
        TctxFixture::from_source(r#"const c: Config = { name: "foo", count: 42, active: true };"#);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "Config".to_string(),
            fields: vec![
                ("name".to_string(), Expr::StringLit("foo".to_string())),
                ("count".to_string(), Expr::NumberLit(42.0)),
                ("active".to_string(), Expr::BoolLit(true)),
            ],
            base: None,
        }
    );
}

#[test]
fn test_convert_expr_object_literal_single_field() {
    let f = TctxFixture::from_source("const w: Wrapper = { value: 10 };");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "Wrapper".to_string(),
            fields: vec![("value".to_string(), Expr::NumberLit(10.0))],
            base: None,
        }
    );
}

#[test]
fn test_convert_expr_object_literal_empty() {
    let f = TctxFixture::from_source("const e: Empty = {};");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "Empty".to_string(),
            fields: vec![],
            base: None,
        }
    );
}

#[test]
fn test_convert_expr_object_literal_without_type_hint_errors() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const obj = { x: 1 };");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    );
    assert!(result.is_err());
}

#[test]
fn test_convert_expr_object_spread_last_position_expands_remaining_fields() {
    // { x: 10, ...rest } → Point { x: 10.0, y: rest.y }
    use crate::registry::TypeDef;
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::from_source_with_reg("const p: Point = { x: 10, ...rest };", reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), Expr::NumberLit(10.0)),
                (
                    "y".to_string(),
                    Expr::FieldAccess {
                        object: Box::new(Expr::Ident("rest".to_string())),
                        field: "y".to_string(),
                    }
                ),
            ],
            base: None,
        }
    );
}

#[test]
fn test_convert_expr_object_spread_middle_position_expands_remaining_fields() {
    // { a: 1, ...rest, c: 3 } → S { a: 1.0, c: 3.0, b: rest.b }
    use crate::registry::TypeDef;
    let mut reg = TypeRegistry::new();
    reg.register(
        "S".to_string(),
        TypeDef::new_struct(
            vec![
                ("a".to_string(), RustType::F64),
                ("b".to_string(), RustType::F64),
                ("c".to_string(), RustType::F64),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::from_source_with_reg("const s: S = { a: 1, ...rest, c: 3 };", reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "S".to_string(),
            fields: vec![
                ("a".to_string(), Expr::NumberLit(1.0)),
                ("c".to_string(), Expr::NumberLit(3.0)),
                (
                    "b".to_string(),
                    Expr::FieldAccess {
                        object: Box::new(Expr::Ident("rest".to_string())),
                        field: "b".to_string(),
                    }
                ),
            ],
            base: None,
        }
    );
}

#[test]
fn test_convert_object_spread_unregistered_type_generates_struct_update() {
    // {...a, key: 1} — TypeRegistry 未登録 → struct update syntax
    let f = TctxFixture::from_source("const p: Point = { ...other, x: 10 };");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "Point".to_string(),
            fields: vec![("x".to_string(), Expr::NumberLit(10.0))],
            base: Some(Box::new(Expr::Ident("other".to_string()))),
        }
    );
}

#[test]
fn test_convert_object_spread_multiple_registered_generates_merged_fields() {
    // {...a, ...b} — 複数スプレッド + TypeRegistry 登録済み
    use crate::registry::TypeDef;
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::from_source_with_reg("const p: Point = { ...a, ...b };", reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // 最初の spread (a) はフィールド展開、最後の spread (b) は base
    match &result {
        Expr::StructInit { base, fields, .. } => {
            assert!(base.is_some(), "expected base for last spread");
            // a のフィールドが展開されている
            assert!(
                fields.iter().any(|(k, _)| k == "x" || k == "y"),
                "expected expanded fields from first spread, got {fields:?}"
            );
        }
        other => panic!("expected StructInit, got {other:?}"),
    }
}

#[test]
fn test_convert_expr_object_spread_with_override() {
    use crate::registry::TypeDef;
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::from_source_with_reg("const p: Point = { ...other, x: 10 };", reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // Spread expands to field-by-field access: x is overridden, y from base
    assert_eq!(
        result,
        Expr::StructInit {
            name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), Expr::NumberLit(10.0)),
                (
                    "y".to_string(),
                    Expr::FieldAccess {
                        object: Box::new(Expr::Ident("other".to_string())),
                        field: "y".to_string(),
                    }
                ),
            ],
            base: None,
        }
    );
}

#[test]
fn test_convert_expr_array_nested() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_var_init("const a = [[1, 2], [3]];");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Vec {
            elements: vec![
                Expr::Vec {
                    elements: vec![Expr::NumberLit(1.0), Expr::NumberLit(2.0)],
                },
                Expr::Vec {
                    elements: vec![Expr::NumberLit(3.0)],
                },
            ],
        }
    );
}

// -- Expected type propagation tests --

#[test]
fn test_convert_expr_string_lit_with_string_expected_adds_to_string() {
    let f = TctxFixture::from_source(r#"const s: string = "hello";"#);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::StringLit("hello".to_string())),
            method: "to_string".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_string_lit_without_expected_unchanged() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("\"hello\";");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::StringLit("hello".to_string()));
}

#[test]
fn test_convert_expr_string_lit_with_f64_expected_unchanged() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("\"hello\";");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::StringLit("hello".to_string()));
}

#[test]
fn test_convert_expr_array_string_with_vec_string_expected() {
    let f = TctxFixture::from_source(r#"const a: string[] = ["a", "b"];"#);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Vec {
            elements: vec![
                Expr::MethodCall {
                    object: Box::new(Expr::StringLit("a".to_string())),
                    method: "to_string".to_string(),
                    args: vec![],
                },
                Expr::MethodCall {
                    object: Box::new(Expr::StringLit("b".to_string())),
                    method: "to_string".to_string(),
                    args: vec![],
                },
            ],
        }
    );
}

// -- TypeRegistry-based resolution tests --

#[test]
fn test_convert_expr_member_enum_access_from_registry() {
    // enum Color { Red, Green, Blue }
    // Color.Red  →  Color::Red
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "Color".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Red".to_string(), "Green".to_string(), "Blue".to_string()],
            string_values: std::collections::HashMap::new(),
            tag_field: None,
            variant_fields: std::collections::HashMap::new(),
        },
    );

    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    let swc_expr = parse_expr("Color.Red;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::Ident("Color::Red".to_string()));
}

#[test]
fn test_convert_expr_member_non_enum_unchanged() {
    // obj.field should remain FieldAccess when obj is not an enum
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("obj.field;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::FieldAccess {
            object: Box::new(Expr::Ident("obj".to_string())),
            field: "field".to_string(),
        }
    );
}

#[test]
fn test_convert_expr_call_resolves_object_arg_from_registry() {
    // function draw(p: Point): void {}
    // draw({ x: 0, y: 0 })  →  draw(Point { x: 0.0, y: 0.0 })
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "draw".to_string(),
        TypeDef::Function {
            params: vec![(
                "p".to_string(),
                RustType::Named {
                    name: "Point".to_string(),
                    type_args: vec![],
                },
            )],
            return_type: None,
            has_rest: false,
        },
    );

    let source = "draw({ x: 0, y: 0 });";
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 0);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::FnCall {
            name: "draw".to_string(),
            args: vec![Expr::StructInit {
                name: "Point".to_string(),
                fields: vec![
                    ("x".to_string(), Expr::NumberLit(0.0)),
                    ("y".to_string(), Expr::NumberLit(0.0)),
                ],
                base: None,
            }],
        }
    );
}

#[test]
fn test_convert_expr_object_literal_nested_resolves_field_type_from_registry() {
    // interface Origin { x: number; y: number; }
    // interface Rect { origin: Origin; w: number; }
    // const r: Rect = { origin: { x: 0, y: 0 }, w: 10 }
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "Origin".to_string(),
        TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    reg.register(
        "Rect".to_string(),
        TypeDef::new_struct(
            vec![
                (
                    "origin".to_string(),
                    RustType::Named {
                        name: "Origin".to_string(),
                        type_args: vec![],
                    },
                ),
                ("w".to_string(), RustType::F64),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );

    let f = TctxFixture::from_source_with_reg(
        "const r: Rect = { origin: { x: 0, y: 0 }, w: 10 };",
        reg,
    );
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "Rect".to_string(),
            fields: vec![
                (
                    "origin".to_string(),
                    Expr::StructInit {
                        name: "Origin".to_string(),
                        fields: vec![
                            ("x".to_string(), Expr::NumberLit(0.0)),
                            ("y".to_string(), Expr::NumberLit(0.0)),
                        ],
                        base: None,
                    }
                ),
                ("w".to_string(), Expr::NumberLit(10.0)),
            ],
            base: None,
        }
    );
}

// --- Optional None completion ---

#[test]
fn test_object_lit_omitted_optional_field_gets_none() {
    // struct Item { name: String, value: Option<f64> }
    // { name: "test" } → Item { name: "test".to_string(), value: None }
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "Item".to_string(),
        TypeDef::new_struct(
            vec![
                ("name".to_string(), RustType::String),
                (
                    "value".to_string(),
                    RustType::Option(Box::new(RustType::F64)),
                ),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::from_source_with_reg(r#"const i: Item = { name: "test" };"#, reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match &result {
        Expr::StructInit { fields, .. } => {
            assert_eq!(fields.len(), 2, "expected 2 fields (name + value: None)");
            assert!(
                fields
                    .iter()
                    .any(|(k, v)| k == "value" && matches!(v, Expr::Ident(s) if s == "None")),
                "expected value: None, got {:?}",
                fields
            );
        }
        other => panic!("expected StructInit, got {other:?}"),
    }
}

// --- Number + string concatenation ---

#[test]
fn test_binary_number_plus_string_generates_format() {
    // x + " px" where x: number → format!("{}{}", x, " px")
    let f = TctxFixture::from_source(
        r#"function f(x: number): string { return x + " px"; }"#,
    );
    let tctx = f.tctx();
    // The return expression is the binary expression
    let fn_decl = match &f.module().body[0] {
        ModuleItem::Stmt(Stmt::Decl(Decl::Fn(fd))) => fd,
        _ => panic!("expected fn decl"),
    };
    let ret_stmt = &fn_decl.function.body.as_ref().unwrap().stmts[0];
    let swc_expr = match ret_stmt {
        ast::Stmt::Return(ret) => *ret.arg.as_ref().unwrap().clone(),
        _ => panic!("expected return"),
    };
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match &result {
        Expr::FormatMacro { template, args } => {
            assert_eq!(template, "{}{}");
            assert_eq!(args.len(), 2);
        }
        other => panic!("expected FormatMacro for number + string, got {other:?}"),
    }
}

// --- Box::new wrapping for Fn arguments ---

#[test]
fn test_fn_arg_box_dyn_fn_gets_box_new() {
    // applyFn(myFunc) where param is Fn type → applyFn(Box::new(my_func))
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "applyFn".to_string(),
        TypeDef::Function {
            params: vec![(
                "f".to_string(),
                RustType::Fn {
                    params: vec![RustType::F64],
                    return_type: Box::new(RustType::F64),
                },
            )],
            return_type: Some(RustType::F64),
            has_rest: false,
        },
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    let swc_expr = parse_expr("applyFn(myFunc);");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match &result {
        Expr::FnCall { args, .. } => {
            assert!(
                matches!(&args[0], Expr::FnCall { name, .. } if name == "Box::new"),
                "expected Box::new wrapping, got {:?}",
                args[0]
            );
        }
        other => panic!("expected FnCall, got {other:?}"),
    }
}

// -- Ternary (conditional) expression tests --

#[test]
fn test_convert_expr_ternary_basic_identifiers() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const x = flag ? a : b;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::If {
            condition: Box::new(Expr::Ident("flag".to_string())),
            then_expr: Box::new(Expr::Ident("a".to_string())),
            else_expr: Box::new(Expr::Ident("b".to_string())),
        }
    );
}

#[test]
fn test_convert_expr_ternary_with_comparison_condition() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const x = a > 0 ? a : b;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::If {
            condition: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("a".to_string())),
                op: BinOp::Gt,
                right: Box::new(Expr::NumberLit(0.0)),
            }),
            then_expr: Box::new(Expr::Ident("a".to_string())),
            else_expr: Box::new(Expr::Ident("b".to_string())),
        }
    );
}

#[test]
fn test_convert_expr_ternary_with_string_literals() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init(r#"const x = flag ? "yes" : "no";"#);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::If {
            condition: Box::new(Expr::Ident("flag".to_string())),
            then_expr: Box::new(Expr::StringLit("yes".to_string())),
            else_expr: Box::new(Expr::StringLit("no".to_string())),
        }
    );
}

#[test]
fn test_convert_expr_ternary_nested() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x > 0 ? "positive" : x < 0 ? "negative" : "zero"
    let swc_expr = parse_var_init(r#"const s = x > 0 ? "positive" : x < 0 ? "negative" : "zero";"#);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::If {
            condition: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Gt,
                right: Box::new(Expr::NumberLit(0.0)),
            }),
            then_expr: Box::new(Expr::StringLit("positive".to_string())),
            else_expr: Box::new(Expr::If {
                condition: Box::new(Expr::BinaryOp {
                    left: Box::new(Expr::Ident("x".to_string())),
                    op: BinOp::Lt,
                    right: Box::new(Expr::NumberLit(0.0)),
                }),
                then_expr: Box::new(Expr::StringLit("negative".to_string())),
                else_expr: Box::new(Expr::StringLit("zero".to_string())),
            }),
        }
    );
}

#[test]
fn test_convert_expr_ternary_heterogeneous_branches_produces_if() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // cond ? "a" : 1 → if-else with different types (no type coercion)
    let swc_expr = parse_var_init(r#"const x = flag ? "a" : 1;"#);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::If {
            condition: Box::new(Expr::Ident("flag".to_string())),
            then_expr: Box::new(Expr::StringLit("a".to_string())),
            else_expr: Box::new(Expr::NumberLit(1.0)),
        }
    );
}

#[test]
fn test_convert_expr_math_max_three_args_chains() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Math.max(a, b, c) → a.max(b).max(c)
    let expr = parse_expr("Math.max(a, b, c);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("a".to_string())),
                method: "max".to_string(),
                args: vec![Expr::Ident("b".to_string())],
            }),
            method: "max".to_string(),
            args: vec![Expr::Ident("c".to_string())],
        }
    );
}

// -- console.log/error/warn → MacroCall tests --

#[test]
fn test_convert_expr_console_log_single_arg() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("console.log(x);");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MacroCall {
            name: "println".to_string(),
            args: vec![Expr::Ident("x".to_string())],
            use_debug: vec![false],
        }
    );
}

#[test]
fn test_convert_expr_console_error() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("console.error(x);");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MacroCall {
            name: "eprintln".to_string(),
            args: vec![Expr::Ident("x".to_string())],
            use_debug: vec![false],
        }
    );
}

#[test]
fn test_convert_expr_console_warn() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("console.warn(x);");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MacroCall {
            name: "eprintln".to_string(),
            args: vec![Expr::Ident("x".to_string())],
            use_debug: vec![false],
        }
    );
}

#[test]
fn test_convert_expr_console_log_no_args() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("console.log();");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MacroCall {
            name: "println".to_string(),
            args: vec![],
            use_debug: vec![],
        }
    );
}

#[test]
fn test_convert_expr_console_log_multiple_args() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("console.log(x, y);");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MacroCall {
            name: "println".to_string(),
            args: vec![Expr::Ident("x".to_string()), Expr::Ident("y".to_string()),],
            use_debug: vec![false, false],
        }
    );
}

// -- Shorthand property tests --

#[test]
fn test_convert_expr_object_shorthand_single() {
    // const p: Foo = { x }  →  Foo { x: x }
    let f = TctxFixture::from_source("const p: Foo = { x };");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "Foo".to_string(),
            fields: vec![("x".to_string(), Expr::Ident("x".to_string()))],
            base: None,
        }
    );
}

#[test]
fn test_convert_expr_object_shorthand_mixed_with_key_value() {
    // const p: Foo = { x, y: 2 }  →  Foo { x: x, y: 2.0 }
    let f = TctxFixture::from_source("const p: Foo = { x, y: 2 };");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "Foo".to_string(),
            fields: vec![
                ("x".to_string(), Expr::Ident("x".to_string())),
                ("y".to_string(), Expr::NumberLit(2.0)),
            ],
            base: None,
        }
    );
}

#[test]
fn test_convert_expr_object_shorthand_with_registry_field_type() {
    // const u: User = { name }  where name: String → User { name: name }
    // (Ident values don't get .to_string() — only string literals do)
    use crate::registry::TypeDef;
    let mut reg = TypeRegistry::new();
    reg.register(
        "User".to_string(),
        TypeDef::new_struct(
            vec![("name".to_string(), RustType::String)],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::from_source_with_reg("const u: User = { name };", reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "User".to_string(),
            fields: vec![("name".to_string(), Expr::Ident("name".to_string()))],
            base: None,
        }
    );
}

#[test]
fn test_convert_expr_array_nested_vec_string_expected() {
    let f = TctxFixture::from_source(r#"const a: string[][] = [["a"]];"#);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Vec {
            elements: vec![Expr::Vec {
                elements: vec![Expr::MethodCall {
                    object: Box::new(Expr::StringLit("a".to_string())),
                    method: "to_string".to_string(),
                    args: vec![],
                }],
            }],
        }
    );
}

// -- Unary operator tests --

#[test]
fn test_convert_expr_unary_not_bool_literal() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!true;");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::BoolLit(true)),
        }
    );
}

#[test]
fn test_convert_expr_unary_not_ident() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!x;");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::Ident("x".to_string())),
        }
    );
}

#[test]
fn test_convert_expr_unary_minus_ident() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("-x;");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::UnaryOp {
            op: UnOp::Neg,
            operand: Box::new(Expr::Ident("x".to_string())),
        }
    );
}

#[test]
fn test_convert_expr_unary_minus_number_literal() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("-42;");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::UnaryOp {
            op: UnOp::Neg,
            operand: Box::new(Expr::NumberLit(42.0)),
        }
    );
}

#[test]
fn test_convert_expr_unary_not_complex_expr() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("!(a > b);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::UnaryOp {
            op: UnOp::Not,
            operand: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("a".to_string())),
                op: BinOp::Gt,
                right: Box::new(Expr::Ident("b".to_string())),
            }),
        }
    );
}

// -- Await expression tests --

#[test]
fn test_convert_expr_await_simple() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("await fetch();");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Await(Box::new(Expr::FnCall {
            name: "fetch".to_string(),
            args: vec![],
        }))
    );
}

#[test]
fn test_convert_expr_await_ident() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("await promise;");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Await(Box::new(Expr::Ident("promise".to_string())))
    );
}

// -- String method conversion tests --

#[test]
fn test_convert_expr_string_length_to_len_as_f64() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("s.length;");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Cast {
            expr: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("s".to_string())),
                method: "len".to_string(),
                args: vec![],
            }),
            target: RustType::F64,
        }
    );
}

#[test]
fn test_convert_expr_string_includes_to_contains() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr(r#"s.includes("x");"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("s".to_string())),
            method: "contains".to_string(),
            args: vec![Expr::Ref(Box::new(Expr::StringLit("x".to_string())))],
        }
    );
}

#[test]
fn test_convert_includes_to_contains_with_ref() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.includes(3) → arr.contains(&3.0)
    let expr = parse_expr("arr.includes(3);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("arr".to_string())),
            method: "contains".to_string(),
            args: vec![Expr::Ref(Box::new(Expr::NumberLit(3.0)))],
        }
    );
}

#[test]
fn test_convert_expr_string_starts_with() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr(r#"s.startsWith("a");"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("s".to_string())),
            method: "starts_with".to_string(),
            args: vec![Expr::StringLit("a".to_string())],
        }
    );
}

#[test]
fn test_convert_expr_string_ends_with() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr(r#"s.endsWith("z");"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("s".to_string())),
            method: "ends_with".to_string(),
            args: vec![Expr::StringLit("z".to_string())],
        }
    );
}

#[test]
fn test_convert_expr_string_trim_adds_to_string() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("s.trim();");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("s".to_string())),
                method: "trim".to_string(),
                args: vec![],
            }),
            method: "to_string".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_string_to_lower_case() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("s.toLowerCase();");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("s".to_string())),
            method: "to_lowercase".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_string_to_upper_case() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("s.toUpperCase();");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("s".to_string())),
            method: "to_uppercase".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_string_split_generates_vec_string() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // s.split(",") → s.split(",").map(|s| s.to_string()).collect::<Vec<String>>()
    let expr = parse_expr(r#"s.split(",");"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(Expr::Ident("s".to_string())),
                    method: "split".to_string(),
                    args: vec![Expr::StringLit(",".to_string())],
                }),
                method: "map".to_string(),
                args: vec![Expr::Closure {
                    params: vec![Param {
                        name: "s".to_string(),
                        ty: None,
                    }],
                    body: ClosureBody::Expr(Box::new(Expr::MethodCall {
                        object: Box::new(Expr::Ident("s".to_string())),
                        method: "to_string".to_string(),
                        args: vec![],
                    })),
                    return_type: None,
                }],
            }),
            method: "collect::<Vec<String>>".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_substring_two_args_generates_slice() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // s.substring(1, 3) → s[1..3].to_string()
    let expr = parse_expr("s.substring(1, 3);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Index {
                object: Box::new(Expr::Ident("s".to_string())),
                index: Box::new(Expr::Range {
                    start: Some(Box::new(Expr::NumberLit(1.0))),
                    end: Some(Box::new(Expr::NumberLit(3.0))),
                }),
            }),
            method: "to_string".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_substring_one_arg_generates_open_range() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // s.substring(1) → s[1..].to_string()
    let expr = parse_expr("s.substring(1);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Index {
                object: Box::new(Expr::Ident("s".to_string())),
                index: Box::new(Expr::Range {
                    start: Some(Box::new(Expr::NumberLit(1.0))),
                    end: None,
                }),
            }),
            method: "to_string".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_slice_one_arg_generates_open_range() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.slice(1) → arr[1..].to_vec()
    let expr = parse_expr("arr.slice(1);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Index {
                object: Box::new(Expr::Ident("arr".to_string())),
                index: Box::new(Expr::Range {
                    start: Some(Box::new(Expr::NumberLit(1.0))),
                    end: None,
                }),
            }),
            method: "to_vec".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_string_replace_generates_replacen() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // s.replace("a", "b") → s.replacen("a", "b", 1) (first occurrence only)
    let expr = parse_expr(r#"s.replace("a", "b");"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("s".to_string())),
            method: "replacen".to_string(),
            args: vec![
                Expr::StringLit("a".to_string()),
                Expr::StringLit("b".to_string()),
                Expr::IntLit(1),
            ],
        }
    );
}

#[test]
fn test_convert_expr_string_replace_all_generates_replace() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // s.replaceAll("a", "b") → s.replace("a", "b") (Rust replace replaces all)
    let expr = parse_expr(r#"s.replaceAll("a", "b");"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("s".to_string())),
            method: "replace".to_string(),
            args: vec![
                Expr::StringLit("a".to_string()),
                Expr::StringLit("b".to_string()),
            ],
        }
    );
}

// -- Array method conversion tests --

#[test]
fn test_convert_expr_array_map_to_iter_map_collect() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("arr.map((x: number) => x + 1);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // arr.map((x: number) => x + 1) → arr.iter().cloned().map(|x| x + 1).collect()
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(Expr::MethodCall {
                        object: Box::new(Expr::Ident("arr".to_string())),
                        method: "iter".to_string(),
                        args: vec![],
                    }),
                    method: "cloned".to_string(),
                    args: vec![],
                }),
                method: "map".to_string(),
                args: vec![Expr::Closure {
                    params: vec![Param {
                        name: "x".to_string(),
                        ty: None,
                    }],
                    return_type: None,
                    body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
                        left: Box::new(Expr::Ident("x".to_string())),
                        op: BinOp::Add,
                        right: Box::new(Expr::NumberLit(1.0)),
                    })),
                }],
            }),
            method: "collect::<Vec<_>>".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_array_filter_to_iter_filter_collect() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("arr.filter((x: number) => x > 0);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // arr.filter((x: number) => x > 0) → arr.iter().cloned().filter(|x| x > 0).collect()
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(Expr::MethodCall {
                        object: Box::new(Expr::Ident("arr".to_string())),
                        method: "iter".to_string(),
                        args: vec![],
                    }),
                    method: "cloned".to_string(),
                    args: vec![],
                }),
                method: "filter".to_string(),
                args: vec![Expr::Closure {
                    params: vec![Param {
                        name: "x".to_string(),
                        ty: None,
                    }],
                    return_type: None,
                    body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
                        left: Box::new(Expr::Ident("x".to_string())),
                        op: BinOp::Gt,
                        right: Box::new(Expr::NumberLit(0.0)),
                    })),
                }],
            }),
            method: "collect::<Vec<_>>".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_array_find_to_iter_find() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("arr.find((x: number) => x > 0);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // arr.find((x: number) => x > 0) → arr.iter().cloned().find(|x| x > 0)
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(Expr::Ident("arr".to_string())),
                    method: "iter".to_string(),
                    args: vec![],
                }),
                method: "cloned".to_string(),
                args: vec![],
            }),
            method: "find".to_string(),
            args: vec![Expr::Closure {
                params: vec![Param {
                    name: "x".to_string(),
                    ty: None,
                }],
                return_type: None,
                body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
                    left: Box::new(Expr::Ident("x".to_string())),
                    op: BinOp::Gt,
                    right: Box::new(Expr::NumberLit(0.0)),
                })),
            }],
        }
    );
}

#[test]
fn test_convert_expr_array_some_to_iter_any() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("arr.some((x: number) => x > 0);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // arr.some((x: number) => x > 0) → arr.iter().cloned().any(|x| x > 0)
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(Expr::Ident("arr".to_string())),
                    method: "iter".to_string(),
                    args: vec![],
                }),
                method: "cloned".to_string(),
                args: vec![],
            }),
            method: "any".to_string(),
            args: vec![Expr::Closure {
                params: vec![Param {
                    name: "x".to_string(),
                    ty: None,
                }],
                return_type: None,
                body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
                    left: Box::new(Expr::Ident("x".to_string())),
                    op: BinOp::Gt,
                    right: Box::new(Expr::NumberLit(0.0)),
                })),
            }],
        }
    );
}

#[test]
fn test_convert_expr_array_every_to_iter_all() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("arr.every((x: number) => x > 0);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // arr.every((x: number) => x > 0) → arr.iter().cloned().all(|x| x > 0)
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(Expr::Ident("arr".to_string())),
                    method: "iter".to_string(),
                    args: vec![],
                }),
                method: "cloned".to_string(),
                args: vec![],
            }),
            method: "all".to_string(),
            args: vec![Expr::Closure {
                params: vec![Param {
                    name: "x".to_string(),
                    ty: None,
                }],
                return_type: None,
                body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
                    left: Box::new(Expr::Ident("x".to_string())),
                    op: BinOp::Gt,
                    right: Box::new(Expr::NumberLit(0.0)),
                })),
            }],
        }
    );
}

#[test]
fn test_convert_expr_array_foreach_to_for_loop() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // forEach は式→文の変換なので、statement レベルで別途テストする
    // ここではメソッド呼び出しとしての変換を確認
    let expr = parse_expr("arr.forEach((x: number) => console.log(x));");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // forEach は map_method_call で ForEach 用の IR に変換される
    // 初版: arr.iter().cloned().for_each(|x| ...) に変換
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(Expr::Ident("arr".to_string())),
                    method: "iter".to_string(),
                    args: vec![],
                }),
                method: "cloned".to_string(),
                args: vec![],
            }),
            method: "for_each".to_string(),
            args: vec![Expr::Closure {
                params: vec![Param {
                    name: "x".to_string(),
                    ty: None,
                }],
                return_type: None,
                body: ClosureBody::Expr(Box::new(Expr::MacroCall {
                    name: "println".to_string(),
                    args: vec![Expr::Ident("x".to_string())],
                    use_debug: vec![false],
                })),
            }],
        }
    );
}

// -- Math API conversion tests --

#[test]
fn test_convert_expr_math_floor() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Math.floor(3.7);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::NumberLit(3.7)),
            method: "floor".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_math_ceil() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Math.ceil(x);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "ceil".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_math_round() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Math.round(x);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "round".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_math_abs() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Math.abs(x);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "abs".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_math_sqrt() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Math.sqrt(x);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "sqrt".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_math_max() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Math.max(a, b);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("a".to_string())),
            method: "max".to_string(),
            args: vec![Expr::Ident("b".to_string())],
        }
    );
}

#[test]
fn test_convert_expr_math_min() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Math.min(a, b);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("a".to_string())),
            method: "min".to_string(),
            args: vec![Expr::Ident("b".to_string())],
        }
    );
}

#[test]
fn test_convert_expr_math_pow() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Math.pow(x, 2);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "powf".to_string(),
            args: vec![Expr::NumberLit(2.0)],
        }
    );
}

#[test]
fn test_convert_expr_math_nested() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Math.floor(Math.sqrt(x));");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("x".to_string())),
                method: "sqrt".to_string(),
                args: vec![],
            }),
            method: "floor".to_string(),
            args: vec![],
        }
    );
}

// -- Number/parse API conversion tests --

#[test]
fn test_convert_expr_parse_int() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr(r#"parseInt("42");"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // parseInt("42") → "42".parse::<f64>().unwrap_or(f64::NAN)
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::StringLit("42".to_string())),
                method: "parse::<f64>".to_string(),
                args: vec![],
            }),
            method: "unwrap_or".to_string(),
            args: vec![Expr::Ident("f64::NAN".to_string())],
        }
    );
}

#[test]
fn test_convert_expr_parse_float() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr(r#"parseFloat("3.14");"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // parseFloat("3.14") → "3.14".parse::<f64>().unwrap_or(f64::NAN)
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::StringLit("3.14".to_string())),
                method: "parse::<f64>".to_string(),
                args: vec![],
            }),
            method: "unwrap_or".to_string(),
            args: vec![Expr::Ident("f64::NAN".to_string())],
        }
    );
}

#[test]
fn test_convert_expr_is_nan_global() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("isNaN(x);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // isNaN(x) → x.is_nan()
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "is_nan".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_number_is_nan() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Number.isNaN(x);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // Number.isNaN(x) → x.is_nan()
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "is_nan".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_number_is_finite() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Number.isFinite(x);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // Number.isFinite(x) → x.is_finite()
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "is_finite".to_string(),
            args: vec![],
        }
    );
}

// -- Nullish coalescing tests --

#[test]
fn test_convert_expr_nullish_coalescing_basic() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // `a ?? b` → `a.unwrap_or_else(|| b)`
    let expr = parse_expr("a ?? b;");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("a".to_string())),
            method: "unwrap_or_else".to_string(),
            args: vec![Expr::Closure {
                params: vec![],
                return_type: None,
                body: ClosureBody::Expr(Box::new(Expr::Ident("b".to_string()))),
            }],
        }
    );
}

// -- Type assertion tests --

#[test]
fn test_convert_expr_type_assertion_primitive_generates_cast() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // `x as number` → `x as f64` (primitive cast preserved)
    let expr = parse_expr("x as number;");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Cast {
            expr: Box::new(Expr::Ident("x".to_string())),
            target: RustType::F64,
        }
    );
}

#[test]
fn test_convert_expr_type_assertion_nested() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // `(obj as Foo).bar` → `obj.bar`
    let expr = parse_expr("(obj as Foo).bar;");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::FieldAccess {
            object: Box::new(Expr::Ident("obj".to_string())),
            field: "bar".to_string(),
        }
    );
}

#[test]
fn test_convert_opt_chain_length_returns_len_as_f64() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("x?.length;");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // x?.length → x.as_ref().map(|_v| _v.len() as f64)
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("x".to_string())),
                method: "as_ref".to_string(),
                args: vec![],
            }),
            method: "map".to_string(),
            args: vec![Expr::Closure {
                params: vec![Param {
                    name: "_v".to_string(),
                    ty: None,
                }],
                return_type: None,
                body: ClosureBody::Expr(Box::new(Expr::Cast {
                    expr: Box::new(Expr::MethodCall {
                        object: Box::new(Expr::Ident("_v".to_string())),
                        method: "len".to_string(),
                        args: vec![],
                    }),
                    target: RustType::F64,
                })),
            }],
        }
    );
}

#[test]
fn test_convert_expr_number_is_integer_to_fract() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Number.isInteger(x) → x.fract() == 0.0
    let expr = parse_expr("Number.isInteger(x);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::BinaryOp {
            left: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("x".to_string())),
                method: "fract".to_string(),
                args: vec![],
            }),
            op: BinOp::Eq,
            right: Box::new(Expr::NumberLit(0.0)),
        }
    );
}

#[test]
fn test_convert_expr_math_sign_to_signum() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Math.sign(x) → x.signum()
    let expr = parse_expr("Math.sign(x);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "signum".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_math_trunc() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Math.trunc(x) → x.trunc()
    let expr = parse_expr("Math.trunc(x);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "trunc".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_math_log_to_ln() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Math.log(x) → x.ln()
    let expr = parse_expr("Math.log(x);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "ln".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_math_pi_to_consts() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Math.PI → std::f64::consts::PI
    let expr = parse_expr("Math.PI;");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::Ident("std::f64::consts::PI".to_string()));
}

#[test]
fn test_convert_expr_math_e_to_consts() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Math.E → std::f64::consts::E
    let expr = parse_expr("Math.E;");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::Ident("std::f64::consts::E".to_string()));
}

// --- NaN / Infinity ---

#[test]
fn test_convert_expr_nan_to_f64_nan() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("NaN;");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::Ident("f64::NAN".to_string()));
}

#[test]
fn test_convert_expr_infinity_to_f64_infinity() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("Infinity;");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::Ident("f64::INFINITY".to_string()));
}

#[test]
fn test_convert_expr_slice_to_range_to_vec() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.slice(1, 3) → arr[1..3].to_vec()
    let expr = parse_expr("arr.slice(1, 3);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Index {
                object: Box::new(Expr::Ident("arr".to_string())),
                index: Box::new(Expr::Range {
                    start: Some(Box::new(Expr::NumberLit(1.0))),
                    end: Some(Box::new(Expr::NumberLit(3.0))),
                }),
            }),
            method: "to_vec".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_splice_to_drain_collect() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.splice(1, 2) → arr.drain(1..3).collect::<Vec<_>>()
    let expr = parse_expr("arr.splice(1, 2);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("arr".to_string())),
                method: "drain".to_string(),
                args: vec![Expr::Range {
                    start: Some(Box::new(Expr::NumberLit(1.0))),
                    end: Some(Box::new(Expr::NumberLit(3.0))),
                }],
            }),
            method: "collect::<Vec<_>>".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_reverse_unchanged() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.reverse() → arr.reverse() (same name, in-place)
    let expr = parse_expr("arr.reverse();");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("arr".to_string())),
            method: "reverse".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_sort_no_args_generates_sort_by_partial_cmp() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.sort() → arr.sort_by(|a, b| a.partial_cmp(b).unwrap())
    let expr = parse_expr("arr.sort();");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("arr".to_string())),
            method: "sort_by".to_string(),
            args: vec![Expr::Closure {
                params: vec![
                    Param {
                        name: "a".to_string(),
                        ty: None,
                    },
                    Param {
                        name: "b".to_string(),
                        ty: None,
                    },
                ],
                return_type: None,
                body: ClosureBody::Expr(Box::new(Expr::MethodCall {
                    object: Box::new(Expr::MethodCall {
                        object: Box::new(Expr::Ident("a".to_string())),
                        method: "partial_cmp".to_string(),
                        args: vec![Expr::Ident("b".to_string())],
                    }),
                    method: "unwrap".to_string(),
                    args: vec![],
                })),
            }],
        }
    );
}

#[test]
fn test_convert_expr_sort_with_comparator_to_sort_by() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.sort((a, b) => a - b) → arr.sort_by(|a, b| (a - b).partial_cmp(&0.0).unwrap())
    let expr = parse_expr("arr.sort((a, b) => a - b);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    if let Expr::MethodCall { method, args, .. } = &result {
        assert_eq!(method, "sort_by");
        if let Some(Expr::Closure { params, body, .. }) = args.first() {
            assert_eq!(params.len(), 2);
            assert!(params[0].ty.is_none());
            // Body should be (a - b).partial_cmp(&0.0).unwrap()
            if let ClosureBody::Expr(body_expr) = body {
                assert!(
                    matches!(body_expr.as_ref(), Expr::MethodCall { method, .. } if method == "unwrap"),
                    "expected .unwrap() at top level, got: {body_expr:?}"
                );
                return;
            }
        }
    }
    panic!("expected sort_by with partial_cmp closure, got: {result:?}");
}

#[test]
fn test_convert_expr_index_of_to_iter_position() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.indexOf(x) → arr.iter().position(|item| *item == x).map(|i| i as f64).unwrap_or(-1.0)
    let expr = parse_expr("arr.indexOf(x);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::MethodCall {
                    object: Box::new(Expr::MethodCall {
                        object: Box::new(Expr::Ident("arr".to_string())),
                        method: "iter".to_string(),
                        args: vec![],
                    }),
                    method: "position".to_string(),
                    args: vec![Expr::Closure {
                        params: vec![Param {
                            name: "item".to_string(),
                            ty: None,
                        }],
                        return_type: None,
                        body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
                            left: Box::new(Expr::Deref(Box::new(Expr::Ident("item".to_string(),)))),
                            op: BinOp::Eq,
                            right: Box::new(Expr::Ident("x".to_string())),
                        })),
                    }],
                }),
                method: "map".to_string(),
                args: vec![Expr::Closure {
                    params: vec![Param {
                        name: "i".to_string(),
                        ty: None,
                    }],
                    return_type: None,
                    body: ClosureBody::Expr(Box::new(Expr::Cast {
                        expr: Box::new(Expr::Ident("i".to_string())),
                        target: RustType::F64,
                    })),
                }],
            }),
            method: "unwrap_or".to_string(),
            args: vec![Expr::NumberLit(-1.0)],
        }
    );
}

#[test]
fn test_convert_expr_join_string_literal_passes_through() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.join(",") → arr.join(",") — string literals are already &str in Rust
    let expr = parse_expr("arr.join(\",\");");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("arr".to_string())),
            method: "join".to_string(),
            args: vec![Expr::StringLit(",".to_string())],
        }
    );
}

#[test]
fn test_convert_expr_reduce_with_init_to_iter_fold() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.reduce((acc, x) => acc + x, 0) → arr.iter().fold(0, |acc, x| acc + x)
    let expr = parse_expr("arr.reduce((acc, x) => acc + x, 0);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("arr".to_string())),
                method: "iter".to_string(),
                args: vec![],
            }),
            method: "fold".to_string(),
            args: vec![
                Expr::NumberLit(0.0),
                Expr::Closure {
                    params: vec![
                        Param {
                            name: "acc".to_string(),
                            ty: None,
                        },
                        Param {
                            name: "x".to_string(),
                            ty: None,
                        },
                    ],
                    return_type: None,
                    body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
                        left: Box::new(Expr::Ident("acc".to_string())),
                        op: BinOp::Add,
                        right: Box::new(Expr::Ident("x".to_string())),
                    })),
                },
            ],
        }
    );
}

#[test]
fn test_map_method_reduce_typed_closure_strips_type_annotations() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.reduce((acc: number, x: number) => acc + x, 0)
    // → fold closure params should have NO type annotation (Rust infers &T from iter())
    let expr = parse_expr("arr.reduce((acc: number, x: number) => acc + x, 0);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // Extract the closure from fold args
    if let Expr::MethodCall { args, .. } = &result {
        if let Some(Expr::Closure { params, .. }) = args.get(1) {
            assert!(
                params[0].ty.is_none(),
                "fold closure param 'acc' should have no type annotation, got: {:?}",
                params[0].ty
            );
            assert!(
                params[1].ty.is_none(),
                "fold closure param 'x' should have no type annotation, got: {:?}",
                params[1].ty
            );
            return;
        }
    }
    panic!("expected MethodCall with fold closure, got: {result:?}");
}

#[test]
fn test_map_method_indexof_position_returns_f64_with_unwrap() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.indexOf(target) → arr.iter().position(...).map(|i| i as f64).unwrap_or(-1.0)
    let expr = parse_expr("arr.indexOf(target);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // Should end with .unwrap_or(-1.0)
    if let Expr::MethodCall { method, args, .. } = &result {
        assert_eq!(method, "unwrap_or", "expected unwrap_or, got: {result:?}");
        assert_eq!(
            args,
            &[Expr::NumberLit(-1.0)],
            "expected unwrap_or(-1.0), got: {args:?}"
        );
        return;
    }
    panic!("expected MethodCall with unwrap_or, got: {result:?}");
}

#[test]
fn test_map_method_join_passes_borrowed_arg() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.join(sep) → arr.join(&sep)
    let expr = parse_expr("arr.join(sep);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    if let Expr::MethodCall { method, args, .. } = &result {
        assert_eq!(method, "join");
        // The argument should be a reference: &sep
        assert_eq!(
            args,
            &[Expr::Ref(Box::new(Expr::Ident("sep".to_string())))],
            "expected &sep, got: {args:?}"
        );
        return;
    }
    panic!("expected MethodCall join, got: {result:?}");
}

#[test]
fn test_map_method_sort_no_args_uses_partial_cmp() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.sort() → arr.sort_by(|a, b| a.partial_cmp(b).unwrap())
    let expr = parse_expr("arr.sort();");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    if let Expr::MethodCall { method, .. } = &result {
        assert_eq!(
            method, "sort_by",
            "expected sort_by for no-arg sort, got: {result:?}"
        );
        return;
    }
    panic!("expected sort_by, got: {result:?}");
}

#[test]
fn test_map_method_sort_with_comparator_strips_type_annotations() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.sort((a: number, b: number) => b - a) → sort_by closure params have no type annotation
    let expr = parse_expr("arr.sort((a: number, b: number) => b - a);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    if let Expr::MethodCall { method, args, .. } = &result {
        assert_eq!(method, "sort_by");
        if let Some(Expr::Closure { params, .. }) = args.first() {
            assert!(
                params[0].ty.is_none(),
                "sort_by closure param should have no type, got: {:?}",
                params[0].ty
            );
            return;
        }
    }
    panic!("expected sort_by with untyped closure, got: {result:?}");
}

#[test]
fn test_map_method_splice_generates_integer_range() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr.splice(1, 2) → arr.drain(1..3).collect::<Vec<_>>()
    // The range should use integer literals, not float
    let expr = parse_expr("arr.splice(1, 2);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // Drill into: MethodCall { object: MethodCall { method: "drain", args: [Range { start, end }] }, method: "collect..." }
    if let Expr::MethodCall {
        object,
        method: collect_method,
        ..
    } = &result
    {
        assert!(
            collect_method.starts_with("collect"),
            "expected collect, got: {result:?}"
        );
        if let Expr::MethodCall { method, args, .. } = object.as_ref() {
            assert_eq!(method, "drain");
            if let Some(Expr::Range {
                start: Some(s),
                end: Some(e),
            }) = args.first()
            {
                // Start should be integer-like (NumberLit 1.0 is ok, generator handles it)
                // End should be 3 (1+2), not a BinaryOp
                assert!(
                    matches!(e.as_ref(), Expr::NumberLit(n) if *n == 3.0),
                    "expected end=3.0 (pre-computed), got: {e:?}"
                );
                assert!(
                    matches!(s.as_ref(), Expr::NumberLit(n) if *n == 1.0),
                    "expected start=1.0, got: {s:?}"
                );
                return;
            }
        }
    }
    panic!("expected drain(1..3).collect(), got: {result:?}");
}

#[test]
fn test_convert_opt_chain_normal_field_unchanged() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("x?.y;");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // x?.y → x.as_ref().map(|_v| _v.y) — 既存動作が壊れないこと
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("x".to_string())),
                method: "as_ref".to_string(),
                args: vec![],
            }),
            method: "map".to_string(),
            args: vec![Expr::Closure {
                params: vec![Param {
                    name: "_v".to_string(),
                    ty: None,
                }],
                return_type: None,
                body: ClosureBody::Expr(Box::new(Expr::FieldAccess {
                    object: Box::new(Expr::Ident("_v".to_string())),
                    field: "y".to_string(),
                })),
            }],
        }
    );
}

// --- resolve_expr_type tests ---

use super::resolve_expr_type;

/// Helper: parse a single expression from a statement
fn parse_single_expr(source: &str) -> swc_ecma_ast::Expr {
    parse_expr(source)
}

#[test]
fn test_resolve_expr_type_ident_registered_returns_type() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_single_expr("x;");
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::String);

    assert_eq!(
        resolve_expr_type(&expr, &env, &tctx, f.reg()),
        Some(RustType::String)
    );
}

#[test]
fn test_resolve_expr_type_ident_unregistered_returns_none() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_single_expr("y;");
    let env = TypeEnv::new();

    assert_eq!(resolve_expr_type(&expr, &env, &tctx, f.reg()), None);
}

#[test]
fn test_resolve_expr_type_member_field_found_returns_field_type() {
    let expr = parse_single_expr("x.field;");
    let mut env = TypeEnv::new();
    env.insert(
        "x".to_string(),
        RustType::Named {
            name: "Foo".to_string(),
            type_args: vec![],
        },
    );
    let mut reg = TypeRegistry::new();
    reg.register(
        "Foo".to_string(),
        TypeDef::new_struct(
            vec![("field".to_string(), RustType::String)],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();

    assert_eq!(
        resolve_expr_type(&expr, &env, &tctx, f.reg()),
        Some(RustType::String)
    );
}

#[test]
fn test_resolve_expr_type_member_field_not_found_returns_none() {
    let expr = parse_single_expr("x.missing;");
    let mut env = TypeEnv::new();
    env.insert(
        "x".to_string(),
        RustType::Named {
            name: "Foo".to_string(),
            type_args: vec![],
        },
    );
    let mut reg = TypeRegistry::new();
    reg.register(
        "Foo".to_string(),
        TypeDef::new_struct(
            vec![("other".to_string(), RustType::F64)],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();

    assert_eq!(resolve_expr_type(&expr, &env, &tctx, f.reg()), None);
}

#[test]
fn test_resolve_expr_type_member_option_named_returns_field_type() {
    let expr = parse_single_expr("x.field;");
    let mut env = TypeEnv::new();
    env.insert(
        "x".to_string(),
        RustType::Option(Box::new(RustType::Named {
            name: "Foo".to_string(),
            type_args: vec![],
        })),
    );
    let mut reg = TypeRegistry::new();
    reg.register(
        "Foo".to_string(),
        TypeDef::new_struct(
            vec![("field".to_string(), RustType::String)],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();

    assert_eq!(
        resolve_expr_type(&expr, &env, &tctx, f.reg()),
        Some(RustType::String)
    );
}

#[test]
fn test_resolve_expr_type_member_obj_unresolvable_returns_none() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_single_expr("y.field;");
    let env = TypeEnv::new();

    assert_eq!(resolve_expr_type(&expr, &env, &tctx, f.reg()), None);
}

#[test]
fn test_resolve_expr_type_paren_delegates_to_inner() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_single_expr("(x);");
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::F64);

    assert_eq!(
        resolve_expr_type(&expr, &env, &tctx, f.reg()),
        Some(RustType::F64)
    );
}

#[test]
fn test_resolve_expr_type_ts_as_returns_target_type() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_single_expr("x as string;");
    let env = TypeEnv::new();

    assert_eq!(
        resolve_expr_type(&expr, &env, &tctx, f.reg()),
        Some(RustType::String)
    );
}

#[test]
fn test_resolve_expr_type_number_literal_returns_f64() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_single_expr("42;");
    let env = TypeEnv::new();

    assert_eq!(
        resolve_expr_type(&expr, &env, &tctx, f.reg()),
        Some(RustType::F64)
    );
}

#[test]
fn test_resolve_expr_type_member_chain_returns_nested_type() {
    let expr = parse_single_expr("x.inner.name;");
    let mut env = TypeEnv::new();
    env.insert(
        "x".to_string(),
        RustType::Named {
            name: "Outer".to_string(),
            type_args: vec![],
        },
    );
    let mut reg = TypeRegistry::new();
    reg.register(
        "Outer".to_string(),
        TypeDef::new_struct(
            vec![(
                "inner".to_string(),
                RustType::Named {
                    name: "Inner".to_string(),
                    type_args: vec![],
                },
            )],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    reg.register(
        "Inner".to_string(),
        TypeDef::new_struct(
            vec![("name".to_string(), RustType::String)],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();

    assert_eq!(
        resolve_expr_type(&expr, &env, &tctx, f.reg()),
        Some(RustType::String)
    );
}

// --- TypeEnv-aware optional chaining tests ---

#[test]
fn test_convert_opt_chain_non_option_type_returns_plain_access() {
    let mut reg = TypeRegistry::new();
    reg.register(
        "Foo".to_string(),
        TypeDef::new_struct(
            vec![("y".to_string(), RustType::String)],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::from_source_with_reg(
        "function f(x: Foo) { x?.y; }",
        reg,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);

    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::FieldAccess {
            object: Box::new(Expr::Ident("x".to_string())),
            field: "y".to_string(),
        }
    );
}

#[test]
fn test_convert_opt_chain_option_type_returns_map_pattern() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_single_expr("x?.y;");
    let mut env = TypeEnv::new();
    env.insert(
        "x".to_string(),
        RustType::Option(Box::new(RustType::Named {
            name: "Foo".to_string(),
            type_args: vec![],
        })),
    );

    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert!(matches!(
        &result,
        Expr::MethodCall { method, .. } if method == "map"
    ));
}

#[test]
fn test_convert_opt_chain_unknown_type_returns_map_pattern() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_single_expr("x?.y;");
    let env = TypeEnv::new();

    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert!(matches!(
        &result,
        Expr::MethodCall { method, .. } if method == "map"
    ));
}

// --- Optional chaining method name mapping ---

#[test]
fn test_opt_chain_method_call_maps_to_rust_name() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // s?.toUpperCase() → s.as_ref().map(|_v| _v.to_uppercase())
    let expr = parse_single_expr("s?.toUpperCase();");
    let mut env = TypeEnv::new();
    env.insert(
        "s".to_string(),
        RustType::Option(Box::new(RustType::String)),
    );
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // Dig into the map closure body and verify method name is to_uppercase
    if let Expr::MethodCall {
        method: outer_method,
        args,
        ..
    } = &result
    {
        assert_eq!(outer_method, "map");
        if let Some(Expr::Closure {
            body: ClosureBody::Expr(body_expr),
            ..
        }) = args.first()
        {
            if let Expr::MethodCall { method, .. } = body_expr.as_ref() {
                assert_eq!(
                    method, "to_uppercase",
                    "expected to_uppercase, got {method}"
                );
                return;
            }
        }
    }
    panic!("unexpected IR structure: {result:?}");
}

// --- TypeEnv-aware nullish coalescing tests ---

#[test]
fn test_convert_nullish_coalescing_non_option_returns_left() {
    let f = TctxFixture::from_source("function f(x: string, y: string) { x ?? y; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);

    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::Ident("x".to_string()));
}

#[test]
fn test_convert_nullish_coalescing_option_returns_unwrap_or_else() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_single_expr("x ?? y;");
    let mut env = TypeEnv::new();
    env.insert(
        "x".to_string(),
        RustType::Option(Box::new(RustType::String)),
    );

    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert!(matches!(
        &result,
        Expr::MethodCall { method, .. } if method == "unwrap_or_else"
    ));
}

#[test]
fn test_convert_nullish_coalescing_unknown_type_returns_unwrap_or_else() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_single_expr("x ?? y;");
    let env = TypeEnv::new();

    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert!(matches!(
        &result,
        Expr::MethodCall { method, .. } if method == "unwrap_or_else"
    ));
}

// --- Nested optional chaining tests ---

#[test]
fn test_convert_opt_chain_nested_option_uses_and_then() {
    // x?.y?.z where x: Foo | null, Foo.y: Bar | null, Bar.z: String
    // Should use .and_then() for the inner chain to avoid Option<Option<T>>
    let mut reg = TypeRegistry::new();
    reg.register(
        "Foo".to_string(),
        TypeDef::new_struct(
            vec![(
                "y".to_string(),
                RustType::Option(Box::new(RustType::Named {
                    name: "Bar".to_string(),
                    type_args: vec![],
                })),
            )],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    reg.register(
        "Bar".to_string(),
        TypeDef::new_struct(
            vec![("z".to_string(), RustType::String)],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::from_source_with_reg(
        "function f(x: Foo | null) { x?.y?.z; }",
        reg,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);

    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // The outermost should use and_then (not map) to avoid Option<Option<T>>
    let result_str = format!("{result:?}");
    assert!(
        result_str.contains("and_then"),
        "nested optional chaining should use and_then, got: {result:?}"
    );
}

// -- array spread in expression position tests --

use crate::ir::Stmt as IrStmt;

#[test]
fn test_convert_expr_array_spread_in_expression_generates_block() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // foo([...arr, 1]) — spread in function arg position
    let expr = parse_expr("foo([...arr, 1]);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // The argument should be an Expr::Block
    match &result {
        Expr::FnCall { args, .. } => {
            assert_eq!(args.len(), 1);
            assert!(
                matches!(&args[0], Expr::Block(_)),
                "expected Block for spread array arg, got: {:?}",
                args[0]
            );
        }
        other => panic!("expected FnCall, got: {other:?}"),
    }
}

#[test]
fn test_convert_expr_array_spread_prefix_and_suffix_generates_block() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // [1, ...arr, 2] in expression position (as function arg)
    let expr = parse_expr("foo([1, ...arr, 2]);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match &result {
        Expr::FnCall { args, .. } => {
            assert_eq!(args.len(), 1);
            match &args[0] {
                Expr::Block(stmts) => {
                    // Should contain: let mut _v = vec![1.0]; + extend + push + tail
                    assert!(
                        stmts.len() >= 3,
                        "expected at least 3 stmts in block, got {stmts:?}"
                    );
                    // First: let mut _v = vec![1.0];
                    assert!(
                        matches!(&stmts[0], IrStmt::Let { mutable: true, name, .. } if name == "_v"),
                        "expected let mut _v, got: {:?}",
                        stmts[0]
                    );
                    // Last: tail expr _v
                    assert!(
                        matches!(stmts.last(), Some(IrStmt::TailExpr(Expr::Ident(n))) if n == "_v"),
                        "expected tail _v, got: {:?}",
                        stmts.last()
                    );
                }
                other => panic!("expected Block, got: {other:?}"),
            }
        }
        other => panic!("expected FnCall, got: {other:?}"),
    }
}

// -- String concatenation with & on RHS tests --

#[test]
fn test_string_concat_rhs_ident_gets_ref() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // "Hello " + name → BinaryOp { left: StringLit, op: Add, right: Ref(Ident) }
    let swc_expr = parse_expr(r#""Hello " + name"#);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match result {
        Expr::BinaryOp { right, op, .. } => {
            assert_eq!(op, BinOp::Add);
            assert!(
                matches!(*right, Expr::Ref(_)),
                "expected RHS to be Ref(...), got: {right:?}"
            );
        }
        other => panic!("expected BinaryOp, got: {other:?}"),
    }
}

#[test]
fn test_string_concat_chain_rhs_gets_ref() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // "Hello " + name + "!" → outer Add: LHS is Add(StringLit, Ref(Ident)), RHS should be Ref(StringLit("!"))
    // Actually "!" is a literal, so it gets .to_string() in Rust, which is already &str-compatible
    // But the pattern is: greeting + " " + name
    let swc_expr = parse_expr(r#"greeting + " " + name"#);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // The outer BinaryOp's left is also a BinaryOp with Add
    // We just verify the structure doesn't panic and produces BinaryOp
    assert!(matches!(result, Expr::BinaryOp { op: BinOp::Add, .. }));
}

#[test]
fn test_numeric_add_no_ref() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // a + b (numeric) should NOT get Ref
    let swc_expr = parse_expr("a + b");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match result {
        Expr::BinaryOp { right, op, .. } => {
            assert_eq!(op, BinOp::Add);
            assert!(
                !matches!(*right, Expr::Ref(_)),
                "numeric add should NOT have Ref on RHS"
            );
        }
        other => panic!("expected BinaryOp, got: {other:?}"),
    }
}

// -- Default argument (Option<T>) call site completion tests --

#[test]
fn test_call_with_missing_default_arg_appends_none() {
    // greet("World") where greet has params: (name: String, greeting: Option<String>)
    // Should produce: greet("World".to_string(), None)
    let reg = greet_registry();
    let f = TctxFixture::from_source_with_reg(r#"greet("World");"#, reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 0);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();

    match result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "greet");
            assert_eq!(
                args.len(),
                2,
                "expected 2 args (with None appended), got {args:?}"
            );
            // Second arg should be None (Ident("None"))
            assert_eq!(args[1], Expr::Ident("None".to_string()));
        }
        other => panic!("expected FnCall, got: {other:?}"),
    }
}

#[test]
fn test_call_with_option_arg_wraps_some() {
    // greet("World", "Hi") where greeting is Option<String>
    // Should produce: greet("World".to_string(), Some("Hi".to_string()))
    let reg = greet_registry();
    let f = TctxFixture::from_source_with_reg(r#"greet("World", "Hi");"#, reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 0);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();

    match result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "greet");
            assert_eq!(args.len(), 2);
            // Second arg should be Some(...)
            assert!(
                matches!(&args[1], Expr::FnCall { name, args: inner } if name == "Some" && inner.len() == 1),
                "expected Some(...), got: {:?}",
                args[1]
            );
        }
        other => panic!("expected FnCall, got: {other:?}"),
    }
}

// --- resolve_expr_type: function call return type ---

#[test]
fn test_resolve_expr_type_call_registry_fn_returns_return_type() {
    let expr = parse_single_expr("getValue();");
    let env = TypeEnv::new();
    let mut reg = TypeRegistry::new();
    reg.register(
        "getValue".to_string(),
        TypeDef::Function {
            params: vec![],
            return_type: Some(RustType::String),
            has_rest: false,
        },
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();

    assert_eq!(
        resolve_expr_type(&expr, &env, &tctx, f.reg()),
        Some(RustType::String)
    );
}

#[test]
fn test_resolve_expr_type_call_unregistered_fn_returns_none() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_single_expr("unknown();");
    let env = TypeEnv::new();

    assert_eq!(resolve_expr_type(&expr, &env, &tctx, f.reg()), None);
}

#[test]
fn test_resolve_expr_type_call_fn_type_in_env_returns_return_type() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_single_expr("f();");
    let mut env = TypeEnv::new();
    env.insert(
        "f".to_string(),
        RustType::Fn {
            params: vec![],
            return_type: Box::new(RustType::Bool),
        },
    );

    assert_eq!(
        resolve_expr_type(&expr, &env, &tctx, f.reg()),
        Some(RustType::Bool)
    );
}

#[test]
fn test_resolve_expr_type_call_registry_fn_no_return_type_returns_unit() {
    let expr = parse_single_expr("doSomething();");
    let env = TypeEnv::new();
    let mut reg = TypeRegistry::new();
    reg.register(
        "doSomething".to_string(),
        TypeDef::Function {
            params: vec![],
            return_type: None,
            has_rest: false,
        },
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();

    assert_eq!(
        resolve_expr_type(&expr, &env, &tctx, f.reg()),
        Some(RustType::Unit)
    );
}

// --- resolve_expr_type: array index ---

#[test]
fn test_resolve_expr_type_index_vec_returns_element_type() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_single_expr("arr[0];");
    let mut env = TypeEnv::new();
    env.insert("arr".to_string(), RustType::Vec(Box::new(RustType::String)));

    assert_eq!(
        resolve_expr_type(&expr, &env, &tctx, f.reg()),
        Some(RustType::String)
    );
}

#[test]
fn test_resolve_expr_type_index_non_vec_returns_none() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_single_expr("x[0];");
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::String);

    assert_eq!(resolve_expr_type(&expr, &env, &tctx, f.reg()), None);
}

// --- resolve_expr_type: binary operations ---

#[test]
fn test_resolve_expr_type_comparison_returns_bool() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_single_expr("x > y;");
    let env = TypeEnv::new();

    assert_eq!(
        resolve_expr_type(&expr, &env, &tctx, f.reg()),
        Some(RustType::Bool)
    );
}

#[test]
fn test_resolve_expr_type_equality_returns_bool() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_single_expr("x === y;");
    let env = TypeEnv::new();

    assert_eq!(
        resolve_expr_type(&expr, &env, &tctx, f.reg()),
        Some(RustType::Bool)
    );
}

#[test]
fn test_resolve_expr_type_logical_and_returns_operand_type() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_single_expr("a && b;");
    let mut env = TypeEnv::new();
    env.insert("a".to_string(), RustType::String);
    env.insert("b".to_string(), RustType::String);

    assert_eq!(
        resolve_expr_type(&expr, &env, &tctx, f.reg()),
        Some(RustType::String)
    );
}

// --- resolve_expr_type: new expression ---

#[test]
fn test_resolve_expr_type_new_registered_returns_named_type() {
    let expr = parse_single_expr("new Foo();");
    let env = TypeEnv::new();
    let mut reg = TypeRegistry::new();
    reg.register(
        "Foo".to_string(),
        TypeDef::new_struct(vec![], std::collections::HashMap::new(), vec![]),
    );

    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    assert_eq!(
        resolve_expr_type(&expr, &env, &tctx, f.reg()),
        Some(RustType::Named {
            name: "Foo".to_string(),
            type_args: vec![],
        })
    );
}

#[test]
fn test_resolve_expr_type_new_unregistered_returns_none() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_single_expr("new Unknown();");
    let env = TypeEnv::new();

    assert_eq!(resolve_expr_type(&expr, &env, &tctx, f.reg()), None);
}

// --- Step 5: expected 型伝搬テスト ---

#[test]
fn test_convert_bin_expr_expected_string_enables_concat() {
    // a + b with expected=String → string concat context (RHS wrapped in Ref)
    let f = TctxFixture::from_source("const s: string = a + b;");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let env = TypeEnv::new(); // a, b not registered → types unknown

    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();

    // In string concat context, RHS should be wrapped in Ref
    match &result {
        Expr::BinaryOp { op, right, .. } => {
            assert_eq!(*op, BinOp::Add);
            assert!(
                matches!(right.as_ref(), Expr::Ref(_)),
                "RHS should be Ref in string concat context, got: {:?}",
                right
            );
        }
        _ => panic!("expected BinaryOp, got: {:?}", result),
    }
}

#[test]
fn test_convert_bin_expr_no_expected_numeric_add() {
    // a + b with expected=None → numeric addition (no Ref wrapping)
    let swc_expr = parse_expr("a + b;");
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let env = TypeEnv::new();

    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();

    match &result {
        Expr::BinaryOp { op, right, .. } => {
            assert_eq!(*op, BinOp::Add);
            assert!(
                !matches!(right.as_ref(), Expr::Ref(_)),
                "RHS should NOT be Ref in numeric context"
            );
        }
        _ => panic!("expected BinaryOp, got: {:?}", result),
    }
}

#[test]
fn test_convert_call_expr_typeenv_fn_provides_param_expected() {
    // f("hello") where f: (s: string) => boolean is declared
    // → "hello" should become "hello".to_string() because expected=String
    let source = r#"
        function f(s: string): boolean { return true; }
        f("hello");
    "#;
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();
    let env = TypeEnv::new();

    let swc_expr = extract_expr_stmt(f.module(), 1);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();

    match &result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "f");
            assert_eq!(args.len(), 1);
            // The string literal should have .to_string() because param type is String
            assert!(
                matches!(
                    &args[0],
                    Expr::MethodCall { method, .. } if method == "to_string"
                ),
                "arg should be .to_string() call, got: {:?}",
                args[0]
            );
        }
        _ => panic!("expected FnCall, got: {:?}", result),
    }
}

#[test]
fn test_convert_call_expr_no_typeenv_fn_no_expected() {
    // f("hello") where TypeEnv is empty → "hello" stays as StringLit (no .to_string())
    let swc_expr = parse_expr("f(\"hello\");");
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let env = TypeEnv::new();

    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();

    match &result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "f");
            assert_eq!(args.len(), 1);
            assert!(
                matches!(&args[0], Expr::StringLit(s) if s == "hello"),
                "arg should be plain StringLit, got: {:?}",
                args[0]
            );
        }
        _ => panic!("expected FnCall, got: {:?}", result),
    }
}

// --- Rest parameter call-site tests ---

#[test]
fn test_convert_call_expr_rest_param_packs_args_into_vec() {
    // sum(1, 2, 3) where sum(...nums: number[]) → sum(vec![1.0, 2.0, 3.0])
    let swc_expr = parse_expr("sum(1, 2, 3);");
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "sum".to_string(),
        TypeDef::Function {
            params: vec![("nums".to_string(), RustType::Vec(Box::new(RustType::F64)))],
            return_type: Some(RustType::F64),
            has_rest: true,
        },
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();

    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();

    match &result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "sum");
            assert_eq!(args.len(), 1, "all args should be packed into one vec");
            match &args[0] {
                Expr::Vec { elements } => {
                    assert_eq!(elements.len(), 3);
                }
                other => panic!("expected Vec, got: {other:?}"),
            }
        }
        _ => panic!("expected FnCall, got: {result:?}"),
    }
}

#[test]
fn test_convert_call_expr_rest_param_mixed_regular_and_rest() {
    // log("hello", 1, 2) where log(prefix: string, ...nums: number[])
    // → log("hello".to_string(), vec![1.0, 2.0])
    let swc_expr = parse_expr(r#"log("hello", 1, 2);"#);
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "log".to_string(),
        TypeDef::Function {
            params: vec![
                ("prefix".to_string(), RustType::String),
                ("nums".to_string(), RustType::Vec(Box::new(RustType::F64))),
            ],
            return_type: None,
            has_rest: true,
        },
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();

    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();

    match &result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "log");
            assert_eq!(args.len(), 2, "prefix + packed rest");
            match &args[1] {
                Expr::Vec { elements } => {
                    assert_eq!(elements.len(), 2);
                }
                other => panic!("expected Vec for rest args, got: {other:?}"),
            }
        }
        _ => panic!("expected FnCall, got: {result:?}"),
    }
}

#[test]
fn test_convert_call_expr_rest_param_no_rest_args() {
    // sum() where sum(...nums: number[]) → sum(vec![])
    let swc_expr = parse_expr("sum();");
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "sum".to_string(),
        TypeDef::Function {
            params: vec![("nums".to_string(), RustType::Vec(Box::new(RustType::F64)))],
            return_type: Some(RustType::F64),
            has_rest: true,
        },
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();

    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();

    match &result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "sum");
            assert_eq!(args.len(), 1);
            match &args[0] {
                Expr::Vec { elements } => {
                    assert_eq!(elements.len(), 0, "no rest args → empty vec");
                }
                other => panic!("expected empty Vec, got: {other:?}"),
            }
        }
        _ => panic!("expected FnCall, got: {result:?}"),
    }
}

#[test]
fn test_convert_call_expr_rest_param_spread_single_array() {
    // sum(...arr) where sum(...nums: number[]) → sum(arr)
    let swc_expr = parse_expr("sum(...arr);");
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "sum".to_string(),
        TypeDef::Function {
            params: vec![("nums".to_string(), RustType::Vec(Box::new(RustType::F64)))],
            return_type: Some(RustType::F64),
            has_rest: true,
        },
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();

    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();

    match &result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "sum");
            assert_eq!(args.len(), 1);
            // Should pass arr directly, not wrap in vec!
            assert!(
                matches!(&args[0], Expr::Ident(name) if name == "arr"),
                "spread arg should be passed directly, got: {:?}",
                args[0]
            );
        }
        _ => panic!("expected FnCall, got: {result:?}"),
    }
}

#[test]
fn test_convert_call_expr_rest_param_mixed_literal_and_spread() {
    // sum(1, ...arr) where sum(...nums: number[]) → sum([vec![1.0], arr].concat())
    let swc_expr = parse_expr("sum(1, ...arr);");
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "sum".to_string(),
        TypeDef::Function {
            params: vec![("nums".to_string(), RustType::Vec(Box::new(RustType::F64)))],
            return_type: Some(RustType::F64),
            has_rest: true,
        },
    );
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();

    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();

    match &result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "sum");
            assert_eq!(args.len(), 1);
            // Should be [vec![1.0], arr].concat()
            match &args[0] {
                Expr::MethodCall { method, .. } => {
                    assert_eq!(method, "concat");
                }
                other => panic!("expected MethodCall(.concat()), got: {other:?}"),
            }
        }
        _ => panic!("expected FnCall, got: {result:?}"),
    }
}

// --- Step 8: 空配列の型推論テスト ---

#[test]
fn test_convert_array_lit_empty_with_expected_vec_string() {
    // [] with expected=Vec<String> → Expr::Vec with no elements (type comes from context)
    let f = TctxFixture::from_source("const x: string[] = [];");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let env = TypeEnv::new();

    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();

    assert_eq!(result, Expr::Vec { elements: vec![] });
}

#[test]
fn test_convert_array_lit_elements_get_expected_element_type() {
    // ["a", "b"] with expected=Vec<String> → elements get .to_string()
    let f = TctxFixture::from_source(r#"const x: string[] = ["a", "b"];"#);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let env = TypeEnv::new();

    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();

    match &result {
        Expr::Vec { elements } => {
            assert_eq!(elements.len(), 2);
            // Each element should have .to_string() because element expected type is String
            for elem in elements {
                assert!(
                    matches!(elem, Expr::MethodCall { method, .. } if method == "to_string"),
                    "element should be .to_string() call, got: {:?}",
                    elem
                );
            }
        }
        _ => panic!("expected Vec, got: {:?}", result),
    }
}

// --- typeof / instanceof type guard expressions ---

#[test]
fn test_typeof_equals_string_known_type_resolves_true() {
    let f = TctxFixture::from_source(r#"function f(x: string) { typeof x === "string"; }"#);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn test_typeof_equals_string_mismatched_type_resolves_false() {
    let f = TctxFixture::from_source(r#"function f(x: number) { typeof x === "string"; }"#);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn test_typeof_equals_number_known_type_resolves_true() {
    let f = TctxFixture::from_source(r#"function f(x: number) { typeof x === "number"; }"#);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn test_typeof_not_equals_string_known_type_resolves_false() {
    let f = TctxFixture::from_source(r#"function f(x: string) { typeof x !== "string"; }"#);
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn test_typeof_equals_string_unknown_type_generates_todo() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("typeof x === \"string\";");
    let env = TypeEnv::new(); // x not registered
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // Unknown type → todo!() (compile error, not silent true)
    assert!(matches!(&result, Expr::FnCall { name, .. } if name == "todo!"));
}

#[test]
fn test_typeof_equals_string_any_type_generates_todo() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Any type → todo!() (compile error, not silent true).
    // For function params, any_narrowing generates enum and if-let instead.
    let swc_expr = parse_expr("typeof x === \"string\";");
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::Any);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert!(matches!(&result, Expr::FnCall { name, .. } if name == "todo!"));
}

#[test]
fn test_typeof_equals_number_any_type_generates_todo() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("typeof x === \"number\";");
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::Any);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert!(matches!(&result, Expr::FnCall { name, .. } if name == "todo!"));
}

#[test]
fn test_typeof_not_equals_string_any_type_generates_todo() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // !== with Any → todo!() (compile error, not silent true).
    let swc_expr = parse_expr("typeof x !== \"string\";");
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::Any);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert!(matches!(&result, Expr::FnCall { name, .. } if name == "todo!"));
}

#[test]
fn test_instanceof_any_type_generates_todo() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Any type → todo!() (compile error, not silent true).
    // For function params, any_narrowing generates enum and if-let instead.
    let swc_expr = parse_expr("x instanceof Foo;");
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::Any);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert!(matches!(&result, Expr::FnCall { name, .. } if name == "todo!"));
}

#[test]
fn test_typeof_equals_undefined_option_resolves_is_none() {
    let f = TctxFixture::from_source(
        r#"function f(x: number | null) { typeof x === "undefined"; }"#,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert!(
        matches!(&result, Expr::MethodCall { method, .. } if method == "is_none"),
        "expected is_none call, got: {:?}",
        result
    );
}

#[test]
fn test_typeof_standalone_known_type_resolves_string_lit() {
    let f = TctxFixture::from_source("function f(x: string) { typeof x; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::StringLit("string".to_string()));
}

#[test]
fn test_instanceof_known_type_match_resolves_true() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("x instanceof Foo;");
    let mut env = TypeEnv::new();
    env.insert(
        "x".to_string(),
        RustType::Named {
            name: "Foo".to_string(),
            type_args: vec![],
        },
    );
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn test_instanceof_known_type_mismatch_resolves_false() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("x instanceof Foo;");
    let mut env = TypeEnv::new();
    env.insert(
        "x".to_string(),
        RustType::Named {
            name: "Bar".to_string(),
            type_args: vec![],
        },
    );
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn test_instanceof_unknown_type_generates_todo() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Unknown type → todo!() (compile error, not silent true).
    let swc_expr = parse_expr("x instanceof Foo;");
    let env = TypeEnv::new();
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert!(matches!(&result, Expr::FnCall { name, .. } if name == "todo!"));
}

// --- self.field string concat clone ---

#[test]
fn test_self_field_string_concat_gets_clone() {
    // this.name + " suffix" → self.name.clone() + &" suffix"
    let f = TctxFixture::from_source(r#"const s: string = this.name + " suffix";"#);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let mut env = TypeEnv::new();
    // Mark "this" as having a string field to trigger string concat context
    env.insert(
        "this".to_string(),
        RustType::Named {
            name: "Self".to_string(),
            type_args: vec![],
        },
    );
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match &result {
        Expr::BinaryOp { left, op, .. } => {
            assert_eq!(*op, BinOp::Add);
            // LHS should be self.name.clone()
            assert!(
                matches!(left.as_ref(), Expr::MethodCall { method, .. } if method == "clone"),
                "expected .clone() on self.field, got: {:?}",
                left
            );
        }
        _ => panic!("expected BinaryOp, got: {:?}", result),
    }
}

// --- undefined / Option semantics ---

#[test]
fn test_undefined_literal_converts_to_none() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("undefined;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::Ident("None".to_string()));
}

#[test]
fn test_equals_undefined_converts_to_is_none() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("x === undefined;");
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::Option(Box::new(RustType::F64)));
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert!(
        matches!(&result, Expr::MethodCall { method, .. } if method == "is_none"),
        "expected is_none, got: {:?}",
        result
    );
}

#[test]
fn test_not_equals_undefined_converts_to_is_some() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("x !== undefined;");
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::Option(Box::new(RustType::F64)));
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert!(
        matches!(&result, Expr::MethodCall { method, .. } if method == "is_some"),
        "expected is_some, got: {:?}",
        result
    );
}

#[test]
fn test_option_expected_wraps_literal_in_some() {
    // Literals with Option expected are wrapped in Some() (for array elements etc.)
    let f = TctxFixture::from_source("const x: number | undefined = 42;");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::FnCall {
            name: "Some".to_string(),
            args: vec![Expr::NumberLit(42.0)],
        }
    );
}

#[test]
fn test_option_expected_undefined_stays_none() {
    let f = TctxFixture::from_source("const x: number | undefined = undefined;");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // Should be None, not Some(None)
    assert_eq!(result, Expr::Ident("None".to_string()));
}

// --- string literal → enum variant conversion ---

#[test]
fn test_convert_lit_string_to_enum_variant_when_expected_is_string_literal_union() {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("up".to_string(), "Up".to_string());
    string_values.insert("down".to_string(), "Down".to_string());
    reg.register(
        "Direction".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Up".to_string(), "Down".to_string()],
            string_values,
            tag_field: None,
            variant_fields: std::collections::HashMap::new(),
        },
    );

    let f = TctxFixture::from_source_with_reg(r#"const d: Direction = "up";"#, reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::Ident("Direction::Up".to_string()));
}

#[test]
fn test_convert_lit_string_no_match_falls_back_to_string_lit() {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("up".to_string(), "Up".to_string());
    reg.register(
        "Direction".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Up".to_string()],
            string_values,
            tag_field: None,
            variant_fields: std::collections::HashMap::new(),
        },
    );

    let f = TctxFixture::from_source_with_reg(r#"const d: Direction = "unknown";"#, reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::StringLit("unknown".to_string()));
}

#[test]
fn test_convert_bin_expr_enum_var_eq_string_literal_converts_rhs() {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("up".to_string(), "Up".to_string());
    string_values.insert("down".to_string(), "Down".to_string());
    reg.register(
        "Direction".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Up".to_string(), "Down".to_string()],
            string_values,
            tag_field: None,
            variant_fields: std::collections::HashMap::new(),
        },
    );

    let f = TctxFixture::from_source_with_reg(
        r#"function f(d: Direction) { d == "up"; }"#,
        reg,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::BinaryOp {
            left: Box::new(Expr::Ident("d".to_string())),
            op: BinOp::Eq,
            right: Box::new(Expr::Ident("Direction::Up".to_string())),
        }
    );
}

#[test]
fn test_convert_bin_expr_string_literal_ne_enum_var_converts_lhs() {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("up".to_string(), "Up".to_string());
    reg.register(
        "Direction".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Up".to_string()],
            string_values,
            tag_field: None,
            variant_fields: std::collections::HashMap::new(),
        },
    );

    let f = TctxFixture::from_source_with_reg(
        r#"function f(d: Direction) { "up" != d; }"#,
        reg,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::BinaryOp {
            left: Box::new(Expr::Ident("Direction::Up".to_string())),
            op: BinOp::NotEq,
            right: Box::new(Expr::Ident("d".to_string())),
        }
    );
}

#[test]
fn test_convert_call_args_string_literal_to_enum_variant() {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("up".to_string(), "Up".to_string());
    string_values.insert("down".to_string(), "Down".to_string());
    reg.register(
        "Direction".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Up".to_string(), "Down".to_string()],
            string_values,
            tag_field: None,
            variant_fields: std::collections::HashMap::new(),
        },
    );
    reg.register(
        "move_dir".to_string(),
        TypeDef::Function {
            params: vec![(
                "d".to_string(),
                RustType::Named {
                    name: "Direction".to_string(),
                    type_args: vec![],
                },
            )],
            return_type: None,
            has_rest: false,
        },
    );

    let source = r#"move_dir("up");"#;
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 0);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::FnCall {
            name: "move_dir".to_string(),
            args: vec![Expr::Ident("Direction::Up".to_string())],
        }
    );
}

// --- discriminated union object literal → enum variant ---

#[test]
fn test_convert_object_lit_discriminated_union_to_enum_variant() {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("circle".to_string(), "Circle".to_string());
    string_values.insert("square".to_string(), "Square".to_string());
    let mut variant_fields = std::collections::HashMap::new();
    variant_fields.insert(
        "Circle".to_string(),
        vec![("radius".to_string(), RustType::F64)],
    );
    variant_fields.insert(
        "Square".to_string(),
        vec![("side".to_string(), RustType::F64)],
    );
    reg.register(
        "Shape".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Circle".to_string(), "Square".to_string()],
            string_values,
            tag_field: Some("kind".to_string()),
            variant_fields,
        },
    );

    let f = TctxFixture::from_source_with_reg(
        r#"const s: Shape = { kind: "circle", radius: 5 };"#,
        reg,
    );
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::StructInit {
            name: "Shape::Circle".to_string(),
            fields: vec![("radius".to_string(), Expr::NumberLit(5.0))],
            base: None,
        }
    );
}

#[test]
fn test_convert_object_lit_discriminated_union_unit_variant() {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("active".to_string(), "Active".to_string());
    let mut variant_fields = std::collections::HashMap::new();
    variant_fields.insert("Active".to_string(), vec![]);
    reg.register(
        "Status".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Active".to_string()],
            string_values,
            tag_field: Some("type".to_string()),
            variant_fields,
        },
    );

    let f = TctxFixture::from_source_with_reg(r#"const s: Status = { type: "active" };"#, reg);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // Unit variant: no fields → Ident
    assert_eq!(result, Expr::Ident("Status::Active".to_string()));
}

#[test]
fn test_convert_member_expr_discriminant_field_to_method_call() {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("circle".to_string(), "Circle".to_string());
    let mut variant_fields = std::collections::HashMap::new();
    variant_fields.insert(
        "Circle".to_string(),
        vec![("radius".to_string(), RustType::F64)],
    );
    reg.register(
        "Shape".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Circle".to_string()],
            string_values,
            tag_field: Some("kind".to_string()),
            variant_fields,
        },
    );

    let f = TctxFixture::from_source_with_reg(
        "function f(s: Shape) { s.kind; }",
        reg,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("s".to_string())),
            method: "kind".to_string(),
            args: vec![],
        }
    );
}

// --- computed property (index access) ---

#[test]
fn test_convert_member_expr_array_index_literal_generates_index() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("arr[0];");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Index {
            object: Box::new(Expr::Ident("arr".to_string())),
            index: Box::new(Expr::NumberLit(0.0)),
        }
    );
}

#[test]
fn test_convert_member_expr_array_index_variable_generates_index() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("arr[i];");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Index {
            object: Box::new(Expr::Ident("arr".to_string())),
            index: Box::new(Expr::Ident("i".to_string())),
        }
    );
}

// --- tuple index access ---

#[test]
fn test_convert_member_expr_tuple_literal_index_generates_field_access() {
    let f = TctxFixture::from_source("function f(pair: [string, number]) { pair[0]; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::FieldAccess {
            object: Box::new(Expr::Ident("pair".to_string())),
            field: "0".to_string(),
        }
    );
}

#[test]
fn test_convert_member_expr_tuple_second_index_generates_field_access() {
    let f = TctxFixture::from_source("function f(pair: [string, number]) { pair[1]; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::FieldAccess {
            object: Box::new(Expr::Ident("pair".to_string())),
            field: "1".to_string(),
        }
    );
}

#[test]
fn test_convert_member_expr_non_tuple_index_unchanged() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let mut type_env = TypeEnv::new();
    type_env.insert("arr".to_string(), RustType::Vec(Box::new(RustType::F64)));

    let swc_expr = parse_expr("arr[0];");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &type_env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Index {
            object: Box::new(Expr::Ident("arr".to_string())),
            index: Box::new(Expr::NumberLit(0.0)),
        }
    );
}

#[test]
fn test_resolve_expr_type_tuple_index_returns_element_type() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let mut type_env = TypeEnv::new();
    type_env.insert(
        "pair".to_string(),
        RustType::Tuple(vec![RustType::String, RustType::F64]),
    );

    let swc_expr = parse_expr("pair[0];");
    let result = resolve_expr_type(&swc_expr, &type_env, &tctx, f.reg());
    assert_eq!(result, Some(RustType::String));
}

// --- nullish coalescing expected type propagation ---

#[test]
fn test_convert_nullish_coalescing_rhs_string_gets_to_string_when_lhs_is_option_string() {
    let source = r#"
        const s: string | undefined = undefined;
        const r = s ?? "default";
    "#;
    let f = TctxFixture::from_source(source);
    let tctx = f.tctx();
    let type_env = TypeEnv::new();

    let swc_expr = extract_var_init_at(f.module(), 1);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &type_env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();

    // Should be s.unwrap_or_else(|| "default".to_string())
    if let Expr::MethodCall { method, args, .. } = &result {
        assert_eq!(method, "unwrap_or_else");
        if let Expr::Closure { body, .. } = &args[0] {
            if let ClosureBody::Expr(expr) = body {
                assert!(
                    matches!(
                        expr.as_ref(),
                        Expr::MethodCall { method, .. } if method == "to_string"
                    ),
                    "expected .to_string() on rhs, got: {expr:?}"
                );
            } else {
                panic!("expected ClosureBody::Expr");
            }
        } else {
            panic!("expected Closure");
        }
    } else {
        panic!("expected MethodCall, got: {result:?}");
    }
}

// --- method argument type lookup ---

#[test]
fn test_convert_method_call_string_arg_gets_to_string_with_registry() {
    let mut reg = TypeRegistry::new();
    let mut methods = std::collections::HashMap::new();
    methods.insert(
        "greet".to_string(),
        MethodSignature {
            params: vec![("name".to_string(), RustType::String)],
            return_type: None,
        },
    );
    reg.register(
        "Greeter".to_string(),
        TypeDef::new_struct(vec![], methods, vec![]),
    );

    let source = r#"
        const g: Greeter = new Greeter();
        g.greet("world");
    "#;
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 1);
    let type_env = TypeEnv::new();
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &type_env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();

    // Should have .to_string() on the string arg
    if let Expr::MethodCall { args, .. } = &result {
        assert!(
            matches!(
                &args[0],
                Expr::MethodCall { method, .. } if method == "to_string"
            ),
            "expected .to_string() on method arg, got: {:?}",
            args[0]
        );
    } else {
        panic!("expected MethodCall, got: {result:?}");
    }
}

// --- discriminated union standalone field access ---

fn build_shape_registry_for_expr() -> TypeRegistry {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("circle".to_string(), "Circle".to_string());
    string_values.insert("square".to_string(), "Square".to_string());
    let mut variant_fields = std::collections::HashMap::new();
    variant_fields.insert(
        "Circle".to_string(),
        vec![("radius".to_string(), RustType::F64)],
    );
    variant_fields.insert(
        "Square".to_string(),
        vec![("side".to_string(), RustType::F64)],
    );
    reg.register(
        "Shape".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Circle".to_string(), "Square".to_string()],
            string_values,
            tag_field: Some("kind".to_string()),
            variant_fields,
        },
    );
    reg
}

#[test]
fn test_convert_du_standalone_field_access_generates_match_expr() {
    let reg = build_shape_registry_for_expr();

    // s.radius → match expression
    let f = TctxFixture::from_source_with_reg(
        "function f(s: Shape) { const x = s.radius; }",
        reg,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_var_init(f.module(), 0, 0);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();

    // Should be Expr::Match
    if let Expr::Match { expr, arms } = &result {
        // Match on &s
        assert_eq!(**expr, Expr::Ref(Box::new(Expr::Ident("s".to_string()))));
        // One arm for Circle (which has radius) + wildcard
        assert!(
            arms.len() >= 2,
            "expected at least 2 arms, got: {}",
            arms.len()
        );
        // First arm should bind radius
        assert!(
            arms[0].patterns.iter().any(|p| {
                matches!(p, MatchPattern::EnumVariant { path, bindings }
                    if path == "Shape::Circle" && bindings == &["radius"])
            }),
            "expected Circle arm with radius binding, got: {:?}",
            arms[0].patterns
        );
    } else {
        panic!("expected Expr::Match, got: {result:?}");
    }
}

// --- `in` operator tests ---

#[test]
fn test_in_operator_struct_field_exists_generates_true() {
    // "x" in point → true (Point has field x)
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::from_source_with_reg(
        r#"function f(point: Point) { "x" in point; }"#,
        reg,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn test_in_operator_struct_field_missing_generates_false() {
    // "z" in point → false (Point has no field z)
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let f = TctxFixture::from_source_with_reg(
        r#"function f(point: Point) { "z" in point; }"#,
        reg,
    );
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn test_in_operator_unknown_type_generates_todo() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // "x" in unknown → todo!() (not silent true)
    let expr = parse_expr(r#""x" in unknown"#);
    let env = TypeEnv::new();
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match &result {
        Expr::FnCall { name, .. } => assert_eq!(name, "todo!"),
        other => panic!("expected todo!() for unknown in operator, got: {other:?}"),
    }
}

// ---- Arrow function destructuring parameters ----

#[test]
fn test_convert_expr_arrow_object_destructuring_generates_expansion() {
    // ({ x, y }: Point) => x + y → closure with synthetic param + expansion stmts
    let reg = {
        let mut r = TypeRegistry::new();
        r.register(
            "Point".to_string(),
            TypeDef::new_struct(
                vec![
                    ("x".to_string(), crate::ir::RustType::F64),
                    ("y".to_string(), crate::ir::RustType::F64),
                ],
                std::collections::HashMap::new(),
                vec![],
            ),
        );
        r
    };
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const f = ({ x, y }: Point) => x + y;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match result {
        Expr::Closure { params, body, .. } => {
            // Should have a synthetic parameter named after the type
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "point");
            assert_eq!(
                params[0].ty,
                Some(crate::ir::RustType::Named {
                    name: "Point".to_string(),
                    type_args: vec![],
                })
            );
            // Body should be a Block with expansion stmts + the expression
            match body {
                crate::ir::ClosureBody::Block(stmts) => {
                    // At least 2 expansion stmts (let x = point.x; let y = point.y;) + return
                    assert!(
                        stmts.len() >= 3,
                        "expected at least 3 stmts, got {}",
                        stmts.len()
                    );
                }
                _ => panic!("expected Block body with expansion stmts"),
            }
        }
        _ => panic!("expected Expr::Closure, got {:?}", result),
    }
}

// --- array destructuring in arrow params ---

#[test]
fn test_convert_expr_arrow_array_destructuring_param_generates_tuple() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // ([k, v]: [string, number]) => ... → closure with (k, v) tuple param
    let swc_expr = parse_var_init("const f = ([k, v]: [string, number]) => k;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    );
    assert!(
        result.is_ok(),
        "array destructuring with type should not error: {:?}",
        result.err()
    );
    if let Ok(Expr::Closure { params, .. }) = &result {
        assert_eq!(params.len(), 1, "should have 1 tuple param");
        assert_eq!(params[0].name, "(k, v)");
    } else {
        panic!("expected Closure, got: {:?}", result);
    }
}

#[test]
fn test_convert_expr_arrow_array_destructuring_no_type_generates_param() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // ([a, b]) => ... → should not crash (fallback to untyped)
    let swc_expr = parse_var_init("const f = ([a, b]) => a;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    );
    assert!(
        result.is_ok(),
        "array destructuring without type should not error: {:?}",
        result.err()
    );
}

// --- object destructuring without type annotation ---

#[test]
fn test_convert_expr_arrow_object_destructuring_no_type_generates_value_param() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // ({ x, y }) => ... → should not crash (fallback to serde_json::Value)
    let swc_expr = parse_var_init("const f = ({ x, y }) => x;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    );
    assert!(
        result.is_ok(),
        "object destructuring without type should not error: {:?}",
        result.err()
    );
}

#[test]
fn test_convert_expr_arrow_default_param_generates_option() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // (x: number = 0) => x + 1 → closure with Option<f64> param + unwrap_or
    let swc_expr = parse_var_init("const f = (x: number = 0) => x + 1;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match result {
        Expr::Closure { params, body, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
            // Should be Option<f64>
            assert_eq!(
                params[0].ty,
                Some(crate::ir::RustType::Option(Box::new(
                    crate::ir::RustType::F64
                )))
            );
            // Body should be Block with unwrap_or expansion + expression
            match body {
                crate::ir::ClosureBody::Block(stmts) => {
                    assert!(
                        stmts.len() >= 2,
                        "expected at least 2 stmts, got {}",
                        stmts.len()
                    );
                }
                _ => panic!("expected Block body with default expansion"),
            }
        }
        _ => panic!("expected Expr::Closure, got {:?}", result),
    }
}

// ---- Update expressions in convert_expr ----

#[test]
fn test_convert_expr_postfix_increment_returns_old_value() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // i++ → { let _old = i; i = i + 1.0; _old }
    use crate::ir::Stmt as IrStmt;
    let expr = parse_expr("i++");
    let env = TypeEnv::new();
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match &result {
        Expr::Block(stmts) => {
            assert_eq!(stmts.len(), 3);
            assert!(matches!(&stmts[0], IrStmt::Let { name, .. } if name == "_old"));
            assert!(matches!(&stmts[2], IrStmt::TailExpr(Expr::Ident(n)) if n == "_old"));
        }
        other => panic!("expected Block for postfix i++, got: {other:?}"),
    }
}

#[test]
fn test_convert_expr_prefix_increment_returns_new_value() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // ++i → { i = i + 1.0; i }
    use crate::ir::Stmt as IrStmt;
    let expr = parse_expr("++i");
    let env = TypeEnv::new();
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match &result {
        Expr::Block(stmts) => {
            assert_eq!(stmts.len(), 2);
            assert!(matches!(&stmts[0], IrStmt::Expr(Expr::Assign { .. })));
            assert!(matches!(&stmts[1], IrStmt::TailExpr(Expr::Ident(n)) if n == "i"));
        }
        other => panic!("expected Block for prefix ++i, got: {other:?}"),
    }
}

#[test]
fn test_convert_expr_postfix_decrement_returns_old_value() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // i-- → { let _old = i; i = i - 1.0; _old }
    use crate::ir::Stmt as IrStmt;
    let expr = parse_expr("i--");
    let env = TypeEnv::new();
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match &result {
        Expr::Block(stmts) => {
            assert_eq!(stmts.len(), 3);
            assert!(matches!(&stmts[2], IrStmt::TailExpr(Expr::Ident(n)) if n == "_old"));
        }
        other => panic!("expected Block for postfix i--, got: {other:?}"),
    }
}

#[test]
fn test_convert_expr_prefix_decrement_returns_new_value() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // --i → { i = i - 1.0; i }
    use crate::ir::Stmt as IrStmt;
    let expr = parse_expr("--i");
    let env = TypeEnv::new();
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match &result {
        Expr::Block(stmts) => {
            assert_eq!(stmts.len(), 2);
            assert!(matches!(&stmts[1], IrStmt::TailExpr(Expr::Ident(n)) if n == "i"));
        }
        other => panic!("expected Block for prefix --i, got: {other:?}"),
    }
}

// ---- Function expressions ----

#[test]
fn test_convert_expr_fn_expr_anonymous_generates_closure() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // function(x: number): number { return x + 1; } → Closure
    let swc_expr = parse_var_init("const f = function(x: number): number { return x + 1; };");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match result {
        Expr::Closure {
            params,
            return_type,
            body,
        } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
            assert_eq!(params[0].ty, Some(crate::ir::RustType::F64));
            assert_eq!(return_type, Some(crate::ir::RustType::F64));
            assert!(matches!(body, crate::ir::ClosureBody::Block(_)));
        }
        _ => panic!("expected Expr::Closure, got {:?}", result),
    }
}

#[test]
fn test_convert_expr_fn_expr_named_generates_closure() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // function foo(x: number) { return x; } → Closure (name ignored)
    let swc_expr = parse_var_init("const f = function foo(x: number): number { return x; };");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match result {
        Expr::Closure { params, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
        }
        _ => panic!("expected Expr::Closure, got {:?}", result),
    }
}

#[test]
fn test_convert_expr_fn_expr_no_params_generates_closure() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const f = function(): void {};");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match result {
        Expr::Closure { params, .. } => {
            assert!(params.is_empty());
        }
        _ => panic!("expected Expr::Closure, got {:?}", result),
    }
}

// ---- Regex literal ----

#[test]
fn test_convert_expr_regex_no_flags_generates_regex_new() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // /pattern/ → Expr::Regex { global: false, sticky: false }
    let expr = parse_var_init(r#"const r = /pattern/;"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Regex {
            pattern: "pattern".to_string(),
            global: false,
            sticky: false,
        }
    );
}

#[test]
fn test_convert_expr_regex_global_flag_preserved() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // /pattern/g → Expr::Regex { global: true }
    let expr = parse_var_init(r#"const r = /pattern/g;"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Regex {
            pattern: "pattern".to_string(),
            global: true,
            sticky: false,
        }
    );
}

#[test]
fn test_convert_expr_regex_case_insensitive_flag_inlined() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // /pattern/i → Expr::Regex with (?i) prefix
    let expr = parse_var_init(r#"const r = /pattern/i;"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Regex {
            pattern: "(?i)pattern".to_string(),
            global: false,
            sticky: false,
        }
    );
}

#[test]
fn test_convert_expr_regex_multiple_flags_inlined() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // /pattern/gim → Expr::Regex with (?i)(?m) prefix and global: true
    let expr = parse_var_init(r#"const r = /pattern/gim;"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Regex {
            pattern: "(?i)(?m)pattern".to_string(),
            global: true,
            sticky: false,
        }
    );
}

// ---- Regex flag semantics ----

#[test]
fn test_convert_expr_regex_no_flags_generates_regex_ir() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // /pattern/ → Expr::Regex { global: false, sticky: false }
    let expr = parse_var_init(r#"const r = /pattern/;"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Regex {
            pattern: "pattern".to_string(),
            global: false,
            sticky: false,
        }
    );
}

#[test]
fn test_convert_expr_regex_global_flag_preserved_in_ir() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // /pattern/g → Expr::Regex { global: true, sticky: false }
    let expr = parse_var_init(r#"const r = /pattern/g;"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Regex {
            pattern: "pattern".to_string(),
            global: true,
            sticky: false,
        }
    );
}

#[test]
fn test_convert_expr_regex_sticky_flag_preserved_in_ir() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // /pattern/y → Expr::Regex { global: false, sticky: true }
    let expr = parse_var_init(r#"const r = /pattern/y;"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Regex {
            pattern: "pattern".to_string(),
            global: false,
            sticky: true,
        }
    );
}

#[test]
fn test_convert_expr_regex_multiple_flags_preserved_in_ir() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // /pattern/gims → Expr::Regex { global: true, sticky: false } with (?i)(?m)(?s) prefix
    let expr = parse_var_init(r#"const r = /pattern/gims;"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Regex {
            pattern: "(?i)(?m)(?s)pattern".to_string(),
            global: true,
            sticky: false,
        }
    );
}

#[test]
fn test_convert_expr_replace_with_global_regex_generates_replace_all() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // s.replace(/p/g, "r") → Regex::new(r"p").unwrap().replace_all(&s, "r").to_string()
    let expr = parse_expr(r#"s.replace(/p/g, "r");"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Regex {
                    pattern: "p".to_string(),
                    global: false,
                    sticky: false,
                }),
                method: "replace_all".to_string(),
                args: vec![
                    Expr::Ref(Box::new(Expr::Ident("s".to_string()))),
                    Expr::StringLit("r".to_string()),
                ],
            }),
            method: "to_string".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_replace_with_non_global_regex_generates_replace() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // s.replace(/p/, "r") → Regex::new(r"p").unwrap().replace(&s, "r").to_string()
    let expr = parse_expr(r#"s.replace(/p/, "r");"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Regex {
                    pattern: "p".to_string(),
                    global: false,
                    sticky: false,
                }),
                method: "replace".to_string(),
                args: vec![
                    Expr::Ref(Box::new(Expr::Ident("s".to_string()))),
                    Expr::StringLit("r".to_string()),
                ],
            }),
            method: "to_string".to_string(),
            args: vec![],
        }
    );
}

// ---- Regex method conversion ----

#[test]
fn test_convert_expr_regex_test_generates_is_match() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // /p/.test(s) → Regex::new(r"p").unwrap().is_match(&s)
    let expr = parse_expr(r#"/p/.test(s);"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Regex {
                pattern: "p".to_string(),
                global: false,
                sticky: false,
            }),
            method: "is_match".to_string(),
            args: vec![Expr::Ref(Box::new(Expr::Ident("s".to_string())))],
        }
    );
}

#[test]
fn test_convert_expr_string_match_regex_generates_find() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // s.match(/p/) → Regex::new(r"p").unwrap().find(&s)
    let expr = parse_expr(r#"s.match(/p/);"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Regex {
                pattern: "p".to_string(),
                global: false,
                sticky: false,
            }),
            method: "find".to_string(),
            args: vec![Expr::Ref(Box::new(Expr::Ident("s".to_string())))],
        }
    );
}

#[test]
fn test_convert_expr_string_match_global_regex_generates_find_iter() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // s.match(/p/g) → Regex::new(r"p").unwrap().find_iter(&s)
    let expr = parse_expr(r#"s.match(/p/g);"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Regex {
                pattern: "p".to_string(),
                global: false,
                sticky: false,
            }),
            method: "find_iter".to_string(),
            args: vec![Expr::Ref(Box::new(Expr::Ident("s".to_string())))],
        }
    );
}

#[test]
fn test_convert_expr_regex_exec_generates_captures() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // /p/.exec(s) → Regex::new(r"p").unwrap().captures(&s)
    let expr = parse_expr(r#"/p/.exec(s);"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Regex {
                pattern: "p".to_string(),
                global: false,
                sticky: false,
            }),
            method: "captures".to_string(),
            args: vec![Expr::Ref(Box::new(Expr::Ident("s".to_string())))],
        }
    );
}

// ---- Non-null assertion ----

#[test]
fn test_convert_expr_non_null_assertion_strips_assertion() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x! → x (non-null assertion is type-level only, stripped)
    let expr = parse_expr("x!;");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::Ident("x".to_string()));
}

// ---- Null literal ----

#[test]
fn test_convert_expr_null_literal_generates_none() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("null");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::Ident("None".to_string()));
}

#[test]
fn test_convert_expr_null_with_option_expected_returns_none_not_some_none() {
    // null with expected=Option<f64> should be None, NOT Some(None)
    let f = TctxFixture::from_source("const x: number | undefined = null;");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Ident("None".to_string()),
        "null with Option expected should be None, got: {:?}",
        result
    );
}

// ---- Return value Option wrapping ----

#[test]
fn test_convert_expr_ident_with_option_expected_passes_through() {
    // x with expected=Option<String> and unknown type → Some(x) (centralized wrapping)
    let f = TctxFixture::from_source("const y: string | undefined = x;");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::FnCall {
            name: "Some".to_string(),
            args: vec![Expr::Ident("x".to_string())],
        }
    );
}

#[test]
fn test_convert_expr_undefined_with_option_expected_returns_none() {
    // undefined with expected=Option<T> → None (no wrapping)
    let f = TctxFixture::from_source("const y: string | undefined = undefined;");
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::Ident("None".to_string()));
}

// ---- Tuple literal conversion ----

#[test]
fn test_convert_expr_array_with_tuple_expected_generates_tuple() {
    // ["a", 1] with expected=Tuple([String, F64]) → Expr::Tuple
    let f = TctxFixture::from_source(r#"const t: [string, number] = ["a", 1];"#);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match &result {
        Expr::Tuple { elements } => {
            assert_eq!(elements.len(), 2);
        }
        other => panic!("expected Tuple, got: {other:?}"),
    }
}

#[test]
fn test_convert_expr_nested_array_with_vec_tuple_expected() {
    // [["a", 1], ["b", 2]] with expected=Vec<Tuple([String, F64])>
    // → Expr::Vec { elements: [Expr::Tuple, Expr::Tuple] }
    let f = TctxFixture::from_source(r#"const t: [string, number][] = [["a", 1], ["b", 2]];"#);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match &result {
        Expr::Vec { elements } => {
            assert_eq!(elements.len(), 2);
            assert!(matches!(&elements[0], Expr::Tuple { .. }));
            assert!(matches!(&elements[1], Expr::Tuple { .. }));
        }
        other => panic!("expected Vec of Tuples, got: {other:?}"),
    }
}

// ---- Private field (PrivateName) access ----

#[test]
fn test_convert_expr_private_field_access_generates_field_access() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // this.#field → self._field
    let expr = parse_expr("this.#routes");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match &result {
        Expr::FieldAccess { object, field } => {
            assert!(matches!(object.as_ref(), Expr::Ident(name) if name == "self"));
            assert_eq!(field, "_routes");
        }
        other => panic!("expected FieldAccess, got: {other:?}"),
    }
}

// ---- Bitwise operators ----

#[test]
fn test_convert_expr_bitwise_xor() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("a ^ b");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert!(matches!(
        result,
        Expr::BinaryOp {
            op: BinOp::BitXor,
            ..
        }
    ));
}

#[test]
fn test_convert_expr_bitwise_and() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("a & b");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert!(matches!(
        result,
        Expr::BinaryOp {
            op: BinOp::BitAnd,
            ..
        }
    ));
}

#[test]
fn test_convert_expr_bitwise_or() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("a | b");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert!(matches!(
        result,
        Expr::BinaryOp {
            op: BinOp::BitOr,
            ..
        }
    ));
}

#[test]
fn test_convert_expr_shift_left() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("a << b");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert!(matches!(result, Expr::BinaryOp { op: BinOp::Shl, .. }));
}

#[test]
fn test_convert_expr_shift_right() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("a >> b");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert!(matches!(result, Expr::BinaryOp { op: BinOp::Shr, .. }));
}

// --- unsigned right shift ---

#[test]
fn test_convert_expr_unsigned_right_shift_generates_ushr() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("a >>> b");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert!(matches!(
        result,
        Expr::BinaryOp {
            op: BinOp::UShr,
            ..
        }
    ));
}

#[test]
fn test_convert_expr_compound_assign_ushr_generates_desugar() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let expr = parse_expr("x >>>= 2");
    let mut type_env = TypeEnv::new();
    type_env.insert("x".to_string(), RustType::F64);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &type_env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // Should be Assign { target: x, value: BinaryOp { op: UShr, ... } }
    if let Expr::Assign { value, .. } = &result {
        assert!(
            matches!(
                value.as_ref(),
                Expr::BinaryOp {
                    op: BinOp::UShr,
                    ..
                }
            ),
            "expected UShr binary op in assignment, got: {value:?}"
        );
    } else {
        panic!("expected Assign, got: {result:?}");
    }
}

// ---- Rest parameter in arrow ----

#[test]
fn test_convert_expr_arrow_rest_param_generates_vec() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // (...args: number[]) => args → rest param becomes Vec<f64>
    let swc_expr = parse_var_init("const f = (...args: number[]) => args;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match result {
        Expr::Closure { params, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "args");
            assert_eq!(
                params[0].ty,
                Some(crate::ir::RustType::Vec(Box::new(crate::ir::RustType::F64)))
            );
        }
        _ => panic!("expected Expr::Closure, got {:?}", result),
    }
}

// ---- Compound assignment operators ----

#[test]
fn test_convert_expr_compound_assign_mod() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x %= 3 → x = x % 3
    let expr = parse_expr("x %= 3");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::Ident("x".to_string())),
            value: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Mod,
                right: Box::new(Expr::NumberLit(3.0)),
            }),
        }
    );
}

#[test]
fn test_convert_expr_compound_assign_bitand() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x &= mask → x = x & mask
    let expr = parse_expr("x &= mask");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::Ident("x".to_string())),
            value: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::BitAnd,
                right: Box::new(Expr::Ident("mask".to_string())),
            }),
        }
    );
}

// ---- Compound assignment operators (remaining) ----

#[test]
fn test_convert_expr_compound_assign_add() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x += 1 → x = x + 1
    let expr = parse_expr("x += 1");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::Ident("x".to_string())),
            value: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Add,
                right: Box::new(Expr::NumberLit(1.0)),
            }),
        }
    );
}

#[test]
fn test_convert_expr_compound_assign_sub() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x -= 1 → x = x - 1
    let expr = parse_expr("x -= 1");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::Ident("x".to_string())),
            value: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Sub,
                right: Box::new(Expr::NumberLit(1.0)),
            }),
        }
    );
}

#[test]
fn test_convert_expr_compound_assign_mul() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x *= 2 → x = x * 2
    let expr = parse_expr("x *= 2");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::Ident("x".to_string())),
            value: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Mul,
                right: Box::new(Expr::NumberLit(2.0)),
            }),
        }
    );
}

#[test]
fn test_convert_expr_compound_assign_div() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x /= 2 → x = x / 2
    let expr = parse_expr("x /= 2");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::Ident("x".to_string())),
            value: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Div,
                right: Box::new(Expr::NumberLit(2.0)),
            }),
        }
    );
}

#[test]
fn test_convert_expr_compound_assign_bitor() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x |= mask → x = x | mask
    let expr = parse_expr("x |= mask");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::Ident("x".to_string())),
            value: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::BitOr,
                right: Box::new(Expr::Ident("mask".to_string())),
            }),
        }
    );
}

#[test]
fn test_convert_expr_compound_assign_bitxor() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x ^= mask → x = x ^ mask
    let expr = parse_expr("x ^= mask");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::Ident("x".to_string())),
            value: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::BitXor,
                right: Box::new(Expr::Ident("mask".to_string())),
            }),
        }
    );
}

#[test]
fn test_convert_expr_compound_assign_shl() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x <<= 2 → x = x << 2
    let expr = parse_expr("x <<= 2");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::Ident("x".to_string())),
            value: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Shl,
                right: Box::new(Expr::NumberLit(2.0)),
            }),
        }
    );
}

#[test]
fn test_convert_expr_compound_assign_shr() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x >>= 2 → x = x >> 2
    let expr = parse_expr("x >>= 2");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::Assign {
            target: Box::new(Expr::Ident("x".to_string())),
            value: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Shr,
                right: Box::new(Expr::NumberLit(2.0)),
            }),
        }
    );
}

// ---- Function expression parameter patterns ----

#[test]
fn test_convert_expr_fn_expr_object_destructuring_param() {
    // const f = function({ x, y }: Point) { return x; }; → Closure with 1 param of type Point
    let reg = {
        let mut r = TypeRegistry::new();
        r.register(
            "Point".to_string(),
            TypeDef::new_struct(
                vec![
                    ("x".to_string(), crate::ir::RustType::F64),
                    ("y".to_string(), crate::ir::RustType::F64),
                ],
                std::collections::HashMap::new(),
                vec![],
            ),
        );
        r
    };
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    let swc_expr = parse_var_init("const f = function({ x, y }: Point) { return x; };");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match result {
        Expr::Closure { params, body, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(
                params[0].ty,
                Some(crate::ir::RustType::Named {
                    name: "Point".to_string(),
                    type_args: vec![],
                })
            );
            // Body should be a Block with expansion stmts
            match body {
                crate::ir::ClosureBody::Block(stmts) => {
                    assert!(
                        stmts.len() >= 2,
                        "expected at least 2 stmts, got {}",
                        stmts.len()
                    );
                }
                _ => panic!("expected Block body with expansion stmts"),
            }
        }
        _ => panic!("expected Expr::Closure, got {:?}", result),
    }
}

#[test]
fn test_convert_expr_fn_expr_default_param() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // const f = function(x: number = 0) { return x; }; → Closure with Option<f64> param
    let swc_expr = parse_var_init("const f = function(x: number = 0) { return x; };");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match result {
        Expr::Closure { params, body, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
            assert_eq!(
                params[0].ty,
                Some(crate::ir::RustType::Option(Box::new(
                    crate::ir::RustType::F64
                )))
            );
            // Body should be Block with unwrap_or expansion + return
            match body {
                crate::ir::ClosureBody::Block(stmts) => {
                    assert!(
                        stmts.len() >= 2,
                        "expected at least 2 stmts, got {}",
                        stmts.len()
                    );
                }
                _ => panic!("expected Block body with default expansion"),
            }
        }
        _ => panic!("expected Expr::Closure, got {:?}", result),
    }
}

// ---- Update expression error path ----

#[test]
fn test_convert_expr_update_non_ident_target_errors() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // arr[0]++ should error because the target is not an identifier
    let expr = parse_expr("arr[0]++");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    );
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("unsupported update expression target"),
        "expected 'unsupported update expression target', got: {err_msg}"
    );
}

// ---- Function expression with rest param ----

#[test]
fn test_convert_expr_fn_expr_rest_param_generates_closure() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // const f = function(...args: number[]): void {};
    let swc_expr = parse_var_init("const f = function(...args: number[]): void {};");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match result {
        Expr::Closure { params, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "args");
            assert_eq!(
                params[0].ty,
                Some(crate::ir::RustType::Vec(Box::new(crate::ir::RustType::F64)))
            );
        }
        _ => panic!("expected Expr::Closure, got {:?}", result),
    }
}

// ---- Arrow with rest param no type ----

#[test]
fn test_convert_expr_arrow_rest_param_no_type() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // const f = (...args) => args; → rest param with no type annotation
    let swc_expr = parse_var_init("const f = (...args) => args;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match result {
        Expr::Closure { params, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "args");
            assert_eq!(
                params[0].ty, None,
                "rest param without type annotation should have ty=None"
            );
        }
        _ => panic!("expected Expr::Closure, got {:?}", result),
    }
}

// ---- Call target expansion ----

#[test]
fn test_convert_call_expr_paren_ident_unwraps_to_fn_call() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // (foo)(1) → foo(1.0)
    let expr = parse_expr("(foo)(1);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match &result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "foo");
            assert_eq!(args.len(), 1);
        }
        other => panic!("expected FnCall, got: {other:?}"),
    }
}

#[test]
fn test_convert_call_expr_paren_member_unwraps_to_method_call() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // (obj.method)() → obj.method()
    let expr = parse_expr("(obj.method)();");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match &result {
        Expr::MethodCall { method, .. } => {
            assert_eq!(method, "method");
        }
        other => panic!("expected MethodCall, got: {other:?}"),
    }
}

#[test]
fn test_convert_call_expr_chained_call_does_not_error() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // f(x)(y) — chained call should not error
    let expr = parse_expr("f(1)(2);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    );
    assert!(
        result.is_ok(),
        "chained call should not error: {:?}",
        result.err()
    );
}

// --- IIFE (immediately invoked function expression) ---

#[test]
fn test_convert_call_expr_arrow_iife_generates_closure_call() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // (() => 42)() — arrow IIFE should produce a closure call
    let expr = parse_expr("(() => 42)();");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    );
    assert!(
        result.is_ok(),
        "arrow IIFE should not error: {:?}",
        result.err()
    );
}

#[test]
fn test_convert_call_expr_arrow_iife_with_args_generates_closure_call() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // ((x: number) => x + 1)(5) — arrow IIFE with args
    let expr = parse_expr("((x: number): number => x + 1)(5);");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    );
    assert!(
        result.is_ok(),
        "arrow IIFE with args should not error: {:?}",
        result.err()
    );
}

// ---- instanceof runtime resolution ----

#[test]
fn test_convert_instanceof_unknown_type_generates_todo() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // Unknown type → todo!() (compile error, not silent true).
    let expr = parse_expr("x instanceof Foo");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert!(matches!(&result, Expr::FnCall { name, .. } if name == "todo!"));
}

#[test]
fn test_convert_instanceof_known_matching_type_returns_true() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x instanceof Foo where x: Foo → true (correct static resolution)
    let expr = parse_expr("x instanceof Foo");
    let mut env = TypeEnv::new();
    env.insert(
        "x".to_string(),
        RustType::Named {
            name: "Foo".to_string(),
            type_args: vec![],
        },
    );
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn test_convert_instanceof_option_type_returns_is_some() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // x instanceof Foo where x: Option<Foo> → x.is_some()
    let expr = parse_expr("x instanceof Foo");
    let mut env = TypeEnv::new();
    env.insert(
        "x".to_string(),
        RustType::Option(Box::new(RustType::Named {
            name: "Foo".to_string(),
            type_args: vec![],
        })),
    );
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    match &result {
        Expr::MethodCall { method, .. } => {
            assert_eq!(method, "is_some");
        }
        other => panic!("expected MethodCall(is_some), got: {other:?}"),
    }
}

// ---- typeof runtime resolution ----

#[test]
fn test_convert_typeof_static_number_returns_string_lit() {
    // typeof 42 → "number" (static, no change needed)
    let f = TctxFixture::from_source("function f() { typeof 42; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::StringLit("number".to_string()));
}

#[test]
fn test_convert_typeof_option_type_returns_runtime_if() {
    // typeof x where x: number | null → runtime branch
    let f = TctxFixture::from_source("function f(x: number | null) { typeof x; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // Should be an If expression, NOT a static StringLit("undefined")
    match &result {
        Expr::If { .. } => {} // runtime branch — correct
        Expr::StringLit(s) if s == "undefined" => {
            panic!("typeof Option should NOT be static 'undefined' — must be runtime branch")
        }
        other => panic!("expected If for typeof Option, got: {other:?}"),
    }
}

#[test]
fn test_convert_typeof_unknown_type_returns_object() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // typeof x where x: unknown → "object" (JS default, not "unknown")
    let expr = parse_expr("typeof x");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::StringLit("object".to_string()));
}

// --- process.env ---

#[test]
fn test_process_env_access_converts_to_env_var() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // process.env.HOME → std::env::var("HOME").unwrap()
    let expr = parse_expr("process.env.HOME;");
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::FnCall {
                name: "std::env::var".to_string(),
                args: vec![Expr::StringLit("HOME".to_string())],
            }),
            method: "unwrap".to_string(),
            args: vec![],
        }
    );
}

// --- fs module ---

#[test]
fn test_fs_read_file_sync_converts_to_read_to_string() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // fs.readFileSync("a.txt", "utf8") → std::fs::read_to_string(&"a.txt").unwrap()
    let expr = parse_expr(r#"fs.readFileSync("a.txt", "utf8");"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::FnCall {
                name: "std::fs::read_to_string".to_string(),
                args: vec![Expr::Ref(Box::new(Expr::StringLit("a.txt".to_string())))],
            }),
            method: "unwrap".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_fs_write_file_sync_converts_to_fs_write() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // fs.writeFileSync("a.txt", data) → std::fs::write(&"a.txt", &data).unwrap()
    let expr = parse_expr(r#"fs.writeFileSync("a.txt", data);"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::FnCall {
                name: "std::fs::write".to_string(),
                args: vec![
                    Expr::Ref(Box::new(Expr::StringLit("a.txt".to_string()))),
                    Expr::Ref(Box::new(Expr::Ident("data".to_string()))),
                ],
            }),
            method: "unwrap".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_fs_exists_sync_converts_to_path_exists() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // fs.existsSync("a.txt") → std::path::Path::new("a.txt").exists()
    let expr = parse_expr(r#"fs.existsSync("a.txt");"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::FnCall {
                name: "std::path::Path::new".to_string(),
                args: vec![Expr::Ref(Box::new(Expr::StringLit("a.txt".to_string())))],
            }),
            method: "exists".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_fs_read_file_sync_stdin_converts_to_stdin_read() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // fs.readFileSync("/dev/stdin", "utf8") → std::io::read_to_string(std::io::stdin()).unwrap()
    let expr = parse_expr(r#"fs.readFileSync("/dev/stdin", "utf8");"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::FnCall {
                name: "std::io::read_to_string".to_string(),
                args: vec![Expr::FnCall {
                    name: "std::io::stdin".to_string(),
                    args: vec![],
                }],
            }),
            method: "unwrap".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_fs_read_file_sync_fd0_converts_to_stdin_read() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // fs.readFileSync(0, "utf8") → same as /dev/stdin
    let expr = parse_expr(r#"fs.readFileSync(0, "utf8");"#);
    let result = convert_expr(
        &expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::FnCall {
                name: "std::io::read_to_string".to_string(),
                args: vec![Expr::FnCall {
                    name: "std::io::stdin".to_string(),
                    args: vec![],
                }],
            }),
            method: "unwrap".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_object_lit_all_computed_keys_generates_hashmap() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // { [key]: "val" } (no type hint) → HashMap::from(vec![(key, "val".to_string())])
    let module =
        crate::parser::parse_typescript(r#"const x: Record<string, string> = { [key]: "val" };"#)
            .unwrap();
    let stmt = match &module.body[0] {
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Var(v))) => {
            &v.decls[0]
        }
        _ => panic!("expected var decl"),
    };
    let init = stmt.init.as_ref().unwrap();
    let result = convert_expr(
        init,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        result,
        Expr::FnCall {
            name: "HashMap::from".to_string(),
            args: vec![Expr::Vec {
                elements: vec![Expr::Tuple {
                    elements: vec![
                        Expr::Ident("key".to_string()),
                        Expr::StringLit("val".to_string()),
                    ],
                }],
            }],
        }
    );
}

// --- Expected type propagation (Category B improvements) ---

/// Step 5: Assignment RHS should propagate type from TypeEnv.
/// `x = { name: "test" }` where x: Config → `Config { name: "test".to_string() }`
#[test]
fn test_convert_assign_expr_propagates_type_from_type_env() {
    let mut reg = TypeRegistry::new();
    reg.register(
        "Config".to_string(),
        TypeDef::new_struct(
            vec![("name".to_string(), RustType::String)],
            std::collections::HashMap::new(),
            vec![],
        ),
    );

    let source = r#"
        let x: Config = { name: "" };
        x = { name: "test" };
    "#;
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 1);
    let type_env = TypeEnv::new();
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &type_env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();

    // Expected: x = Config { name: "test".to_string() }
    match &result {
        Expr::Assign { value, .. } => match value.as_ref() {
            Expr::StructInit { name, fields, .. } => {
                assert_eq!(name, "Config");
                assert_eq!(fields[0].0, "name");
                assert!(
                    matches!(&fields[0].1, Expr::MethodCall { method, .. } if method == "to_string"),
                    "expected .to_string() on string field, got {:?}",
                    fields[0].1
                );
            }
            other => panic!("expected StructInit, got {other:?}"),
        },
        other => panic!("expected Assign, got {other:?}"),
    }
}

/// Step 8: HashMap value should propagate expected value type.
/// `{ [key]: "value" }` with expected `HashMap<String, String>` → values get `.to_string()`
#[test]
fn test_convert_hashmap_propagates_value_type() {
    let f = TctxFixture::from_source(r#"const m: { [key: string]: string } = { [key]: "val" };"#);
    let tctx = f.tctx();
    let swc_expr = extract_var_init(f.module());
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();

    // Expected: HashMap::from(vec![(key, "val".to_string())])
    match &result {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "HashMap::from");
            match &args[0] {
                Expr::Vec { elements } => match &elements[0] {
                    Expr::Tuple { elements } => {
                        assert!(
                            matches!(&elements[1], Expr::MethodCall { method, .. } if method == "to_string"),
                            "expected .to_string() on HashMap value, got {:?}",
                            elements[1]
                        );
                    }
                    other => panic!("expected Tuple, got {other:?}"),
                },
                other => panic!("expected Vec, got {other:?}"),
            }
        }
        other => panic!("expected FnCall(HashMap::from), got {other:?}"),
    }
}

/// Step 9: Optional chaining method args should propagate parameter types.
/// `obj?.greet("hello")` where obj: Option<Greeter> and greet takes String → "hello".to_string()
#[test]
fn test_convert_opt_chain_method_call_propagates_param_types() {
    use crate::registry::TypeDef;

    let mut reg = TypeRegistry::new();
    let mut methods = std::collections::HashMap::new();
    methods.insert(
        "greet".to_string(),
        MethodSignature {
            params: vec![("name".to_string(), RustType::String)],
            return_type: None,
        },
    );
    reg.register(
        "Greeter".to_string(),
        TypeDef::new_struct(vec![], methods, vec![]),
    );

    let source = r#"
        const obj: Greeter | undefined = undefined;
        obj?.greet("hello");
    "#;
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();
    let swc_expr = extract_expr_stmt(f.module(), 1);
    let type_env = TypeEnv::new();
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &type_env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();

    // Expected: obj.as_ref().map(|_v| _v.greet("hello".to_string()))
    // The "hello" arg should have .to_string() because greet's param type is String
    match &result {
        Expr::MethodCall { method, args, .. } if method == "map" => {
            // The closure body should contain a method call with to_string on the arg
            match &args[0] {
                Expr::Closure { body, .. } => match body {
                    ClosureBody::Expr(expr) => match expr.as_ref() {
                        Expr::MethodCall {
                            args: inner_args, ..
                        } => {
                            assert!(
                                matches!(&inner_args[0], Expr::MethodCall { method, .. } if method == "to_string"),
                                "expected .to_string() on string arg, got {:?}",
                                inner_args[0]
                            );
                        }
                        other => panic!("expected MethodCall inside closure, got {other:?}"),
                    },
                    _ => panic!("expected ClosureBody::Expr"),
                },
                other => panic!("expected Closure, got {other:?}"),
            }
        }
        other => panic!("expected MethodCall(map), got {other:?}"),
    }
}

// --- Unary plus (I-15) ---

/// +x where x: number → x (identity, no-op)
#[test]
fn test_convert_expr_unary_plus_number_returns_identity() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let mut type_env = TypeEnv::new();
    type_env.insert("x".to_string(), crate::ir::RustType::F64);
    let swc_expr = parse_expr("+x;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &type_env,
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::Ident("x".to_string()));
}

/// +x where x: string → x.parse::<f64>().unwrap()
#[test]
fn test_convert_expr_unary_plus_string_returns_parse() {
    let f = TctxFixture::from_source("function f(x: string) { +x; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    // x.parse::<f64>().unwrap()
    match &result {
        Expr::MethodCall { method, object, .. } if method == "unwrap" => match object.as_ref() {
            Expr::MethodCall { method, .. } if method == "parse::<f64>" => {}
            other => panic!("expected parse::<f64>(), got {other:?}"),
        },
        other => panic!("expected .unwrap(), got {other:?}"),
    }
}

/// +x where x: unknown → x (fallback, let compiler catch type errors)
#[test]
fn test_convert_expr_unary_plus_unknown_returns_identity() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("+x;");
    let result = convert_expr(
        &swc_expr,
        &tctx,
        f.reg(),
        &TypeEnv::new(),
        &mut SyntheticTypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::Ident("x".to_string()));
}
