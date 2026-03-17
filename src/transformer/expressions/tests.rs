use super::*;
use crate::ir::MatchPattern;
use crate::parser::parse_typescript;
use crate::registry::{TypeDef, TypeRegistry};
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
    match &module.body[0] {
        ModuleItem::Stmt(Stmt::Decl(Decl::Var(var_decl))) => {
            let init = var_decl.decls[0].init.as_ref().expect("no initializer");
            *init.clone()
        }
        _ => panic!("expected variable declaration"),
    }
}

#[test]
fn test_convert_expr_identifier() {
    let swc_expr = parse_expr("foo;");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
    assert_eq!(result, Expr::Ident("foo".to_string()));
}

#[test]
fn test_convert_expr_number_literal() {
    let swc_expr = parse_expr("42;");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
    assert_eq!(result, Expr::NumberLit(42.0));
}

#[test]
fn test_convert_expr_string_literal() {
    let swc_expr = parse_expr("\"hello\";");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
    assert_eq!(result, Expr::StringLit("hello".to_string()));
}

#[test]
fn test_convert_expr_bool_true() {
    let swc_expr = parse_expr("true;");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn test_convert_expr_bool_false() {
    let swc_expr = parse_expr("false;");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn test_convert_expr_binary_add() {
    let swc_expr = parse_var_init("const x = a + b;");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_var_init("const x = a > b;");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_var_init("const x = a === b;");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_var_init("const x = `Hello ${name}`;");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_expr("this.name;");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_expr("obj.field;");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // `(x: number) => x + 1`
    let swc_expr = parse_var_init("const f = (x: number) => x + 1;");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // `(x: number): number => { return x + 1; }`
    let swc_expr = parse_var_init("const f = (x: number): number => { return x + 1; };");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_var_init("const f = () => 42;");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_var_init("const f = (x) => x + 1;");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // Only first param has type annotation
    let swc_expr = parse_var_init("const f = (x: number, y) => x + y;");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_expr("foo(x, y);");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_expr("foo();");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_expr("foo(bar(x));");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_expr("obj.method(x);");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_expr("this.doSomething(x);");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_expr("a.b().c();");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_expr("new Foo(x, y);");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_expr("new Foo();");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
    assert_eq!(
        result,
        Expr::FnCall {
            name: "Foo::new".to_string(),
            args: vec![],
        }
    );
}

// --- I-88: Constructor string arg gets .to_string() ---

#[test]
fn test_new_expr_string_arg_gets_to_string() {
    // new Foo("hello") with Foo { name: String } → Foo::new("hello".to_string())
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "Foo".to_string(),
        TypeDef::Struct {
            fields: vec![("name".to_string(), RustType::String)],
            methods: std::collections::HashMap::new(),
        },
    );
    let swc_expr = parse_expr(r#"new Foo("hello");"#);
    let result = convert_expr(&swc_expr, &reg, None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_var_init("const x = `hello world`;");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_var_init("const a = [1, 2, 3];");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_var_init(r#"const a = ["x", "y"];"#);
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_var_init("const a = [];");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
    assert_eq!(result, Expr::Vec { elements: vec![] });
}

#[test]
fn test_convert_expr_array_single_element() {
    let expr = parse_var_init("const a = [42];");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // { x: 1, y: 2 } with expected Named("Point")
    let swc_expr = parse_var_init("const p: Point = { x: 1, y: 2 };");
    let expected = RustType::Named {
        name: "Point".to_string(),
        type_args: vec![],
    };
    let result = convert_expr(
        &swc_expr,
        &TypeRegistry::new(),
        Some(&expected),
        &TypeEnv::new(),
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
    let swc_expr = parse_var_init(r#"const c: Config = { name: "foo", count: 42, active: true };"#);
    let expected = RustType::Named {
        name: "Config".to_string(),
        type_args: vec![],
    };
    let result = convert_expr(
        &swc_expr,
        &TypeRegistry::new(),
        Some(&expected),
        &TypeEnv::new(),
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
    let swc_expr = parse_var_init("const w: Wrapper = { value: 10 };");
    let expected = RustType::Named {
        name: "Wrapper".to_string(),
        type_args: vec![],
    };
    let result = convert_expr(
        &swc_expr,
        &TypeRegistry::new(),
        Some(&expected),
        &TypeEnv::new(),
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
    let swc_expr = parse_var_init("const e: Empty = {};");
    let expected = RustType::Named {
        name: "Empty".to_string(),
        type_args: vec![],
    };
    let result = convert_expr(
        &swc_expr,
        &TypeRegistry::new(),
        Some(&expected),
        &TypeEnv::new(),
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
    let swc_expr = parse_var_init("const obj = { x: 1 };");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new());
    assert!(result.is_err());
}

#[test]
fn test_convert_expr_object_spread_last_position_expands_remaining_fields() {
    // { x: 10, ...rest } → Point { x: 10.0, y: rest.y }
    let swc_expr = parse_var_init("const p: Point = { x: 10, ...rest };");
    let expected = RustType::Named {
        name: "Point".to_string(),
        type_args: vec![],
    };
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "Point".to_string(),
        TypeDef::Struct {
            fields: vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            methods: std::collections::HashMap::new(),
        },
    );
    let result = convert_expr(&swc_expr, &reg, Some(&expected), &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_var_init("const s: S = { a: 1, ...rest, c: 3 };");
    let expected = RustType::Named {
        name: "S".to_string(),
        type_args: vec![],
    };
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "S".to_string(),
        TypeDef::Struct {
            fields: vec![
                ("a".to_string(), RustType::F64),
                ("b".to_string(), RustType::F64),
                ("c".to_string(), RustType::F64),
            ],
            methods: std::collections::HashMap::new(),
        },
    );
    let result = convert_expr(&swc_expr, &reg, Some(&expected), &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_var_init("const p: Point = { ...other, x: 10 };");
    let expected = RustType::Named {
        name: "Point".to_string(),
        type_args: vec![],
    };
    let result = convert_expr(
        &swc_expr,
        &TypeRegistry::new(),
        Some(&expected),
        &TypeEnv::new(),
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
    let swc_expr = parse_var_init("const p: Point = { ...a, ...b };");
    let expected = RustType::Named {
        name: "Point".to_string(),
        type_args: vec![],
    };
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "Point".to_string(),
        TypeDef::Struct {
            fields: vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            methods: std::collections::HashMap::new(),
        },
    );
    let result = convert_expr(&swc_expr, &reg, Some(&expected), &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_var_init("const p: Point = { ...other, x: 10 };");
    let expected = RustType::Named {
        name: "Point".to_string(),
        type_args: vec![],
    };
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "Point".to_string(),
        TypeDef::Struct {
            fields: vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            methods: std::collections::HashMap::new(),
        },
    );
    let result = convert_expr(&swc_expr, &reg, Some(&expected), &TypeEnv::new()).unwrap();
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
    let expr = parse_var_init("const a = [[1, 2], [3]];");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_expr("\"hello\";");
    let result = convert_expr(
        &swc_expr,
        &TypeRegistry::new(),
        Some(&RustType::String),
        &TypeEnv::new(),
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
    let swc_expr = parse_expr("\"hello\";");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
    assert_eq!(result, Expr::StringLit("hello".to_string()));
}

#[test]
fn test_convert_expr_string_lit_with_f64_expected_unchanged() {
    let swc_expr = parse_expr("\"hello\";");
    let result = convert_expr(
        &swc_expr,
        &TypeRegistry::new(),
        Some(&RustType::F64),
        &TypeEnv::new(),
    )
    .unwrap();
    assert_eq!(result, Expr::StringLit("hello".to_string()));
}

#[test]
fn test_convert_expr_array_string_with_vec_string_expected() {
    let expr = parse_var_init(r#"const a = ["a", "b"];"#);
    let expected = RustType::Vec(Box::new(RustType::String));
    let result = convert_expr(
        &expr,
        &TypeRegistry::new(),
        Some(&expected),
        &TypeEnv::new(),
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
            variants: vec!["Red".to_string(), "Green".to_string(), "Blue".to_string()],
            string_values: std::collections::HashMap::new(),
            tag_field: None,
            variant_fields: std::collections::HashMap::new(),
        },
    );

    let swc_expr = parse_expr("Color.Red;");
    let result = convert_expr(&swc_expr, &reg, None, &TypeEnv::new()).unwrap();
    assert_eq!(result, Expr::Ident("Color::Red".to_string()));
}

#[test]
fn test_convert_expr_member_non_enum_unchanged() {
    // obj.field should remain FieldAccess when obj is not an enum
    let reg = TypeRegistry::new();
    let swc_expr = parse_expr("obj.field;");
    let result = convert_expr(&swc_expr, &reg, None, &TypeEnv::new()).unwrap();
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
        },
    );

    let swc_expr = parse_expr("draw({ x: 0, y: 0 });");
    let result = convert_expr(&swc_expr, &reg, None, &TypeEnv::new()).unwrap();
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
        TypeDef::Struct {
            fields: vec![
                ("x".to_string(), RustType::F64),
                ("y".to_string(), RustType::F64),
            ],
            methods: std::collections::HashMap::new(),
        },
    );
    reg.register(
        "Rect".to_string(),
        TypeDef::Struct {
            fields: vec![
                (
                    "origin".to_string(),
                    RustType::Named {
                        name: "Origin".to_string(),
                        type_args: vec![],
                    },
                ),
                ("w".to_string(), RustType::F64),
            ],
            methods: std::collections::HashMap::new(),
        },
    );

    let swc_expr = parse_var_init("const r: Rect = { origin: { x: 0, y: 0 }, w: 10 };");
    let expected = RustType::Named {
        name: "Rect".to_string(),
        type_args: vec![],
    };
    let result = convert_expr(&swc_expr, &reg, Some(&expected), &TypeEnv::new()).unwrap();
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

// --- I-86: Optional None completion ---

#[test]
fn test_object_lit_omitted_optional_field_gets_none() {
    // struct Item { name: String, value: Option<f64> }
    // { name: "test" } → Item { name: "test".to_string(), value: None }
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "Item".to_string(),
        TypeDef::Struct {
            fields: vec![
                ("name".to_string(), RustType::String),
                (
                    "value".to_string(),
                    RustType::Option(Box::new(RustType::F64)),
                ),
            ],
            methods: std::collections::HashMap::new(),
        },
    );
    let swc_expr = parse_var_init(r#"const i: Item = { name: "test" };"#);
    let expected = RustType::Named {
        name: "Item".to_string(),
        type_args: vec![],
    };
    let result = convert_expr(&swc_expr, &reg, Some(&expected), &TypeEnv::new()).unwrap();
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

// --- I-89: Number + string concatenation ---

#[test]
fn test_binary_number_plus_string_generates_format() {
    // x + " px" where x: number → format!("{}{}", x, " px")
    let swc_expr = parse_var_init(r#"const s: string = x + " px";"#);
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::F64);
    let result = convert_expr(
        &swc_expr,
        &TypeRegistry::new(),
        Some(&RustType::String),
        &env,
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

// --- I-82: Box::new wrapping for Fn arguments ---

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
        },
    );
    let swc_expr = parse_expr("applyFn(myFunc);");
    let result = convert_expr(&swc_expr, &reg, None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_var_init("const x = flag ? a : b;");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_var_init("const x = a > 0 ? a : b;");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_var_init(r#"const x = flag ? "yes" : "no";"#);
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // x > 0 ? "positive" : x < 0 ? "negative" : "zero"
    let swc_expr = parse_var_init(r#"const s = x > 0 ? "positive" : x < 0 ? "negative" : "zero";"#);
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // cond ? "a" : 1 → if-else with different types (no type coercion)
    let swc_expr = parse_var_init(r#"const x = flag ? "a" : 1;"#);
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // Math.max(a, b, c) → a.max(b).max(c)
    let expr = parse_expr("Math.max(a, b, c);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_expr("console.log(x);");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_expr("console.error(x);");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_expr("console.warn(x);");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_expr("console.log();");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_expr("console.log(x, y);");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_var_init("const p: Foo = { x };");
    let expected = RustType::Named {
        name: "Foo".to_string(),
        type_args: vec![],
    };
    let result = convert_expr(
        &swc_expr,
        &TypeRegistry::new(),
        Some(&expected),
        &TypeEnv::new(),
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
    let swc_expr = parse_var_init("const p: Foo = { x, y: 2 };");
    let expected = RustType::Named {
        name: "Foo".to_string(),
        type_args: vec![],
    };
    let result = convert_expr(
        &swc_expr,
        &TypeRegistry::new(),
        Some(&expected),
        &TypeEnv::new(),
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
    let swc_expr = parse_var_init("const u: User = { name };");
    let expected = RustType::Named {
        name: "User".to_string(),
        type_args: vec![],
    };
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "User".to_string(),
        TypeDef::Struct {
            fields: vec![("name".to_string(), RustType::String)],
            methods: std::collections::HashMap::new(),
        },
    );
    let result = convert_expr(&swc_expr, &reg, Some(&expected), &TypeEnv::new()).unwrap();
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
    let expr = parse_var_init(r#"const a = [["a"]];"#);
    let expected = RustType::Vec(Box::new(RustType::Vec(Box::new(RustType::String))));
    let result = convert_expr(
        &expr,
        &TypeRegistry::new(),
        Some(&expected),
        &TypeEnv::new(),
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
    let expr = parse_expr("!true;");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("!x;");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("-x;");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("-42;");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("!(a > b);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("await fetch();");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("await promise;");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
    assert_eq!(
        result,
        Expr::Await(Box::new(Expr::Ident("promise".to_string())))
    );
}

// -- String method conversion tests --

#[test]
fn test_convert_expr_string_length_to_len_as_f64() {
    let expr = parse_expr("s.length;");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr(r#"s.includes("x");"#);
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // arr.includes(3) → arr.contains(&3.0)
    let expr = parse_expr("arr.includes(3);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr(r#"s.startsWith("a");"#);
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr(r#"s.endsWith("z");"#);
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("s.trim();");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("s.toLowerCase();");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("s.toUpperCase();");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
fn test_convert_expr_string_split() {
    let expr = parse_expr(r#"s.split(",");"#);
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
    // s.split(",") → s.split(",").collect::<Vec<&str>>()
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("s".to_string())),
                method: "split".to_string(),
                args: vec![Expr::StringLit(",".to_string())],
            }),
            method: "collect::<Vec<&str>>".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn test_convert_expr_string_replace() {
    let expr = parse_expr(r#"s.replace("a", "b");"#);
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("arr.map((x: number) => x + 1);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("arr.filter((x: number) => x > 0);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("arr.find((x: number) => x > 0);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("arr.some((x: number) => x > 0);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("arr.every((x: number) => x > 0);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // forEach は式→文の変換なので、statement レベルで別途テストする
    // ここではメソッド呼び出しとしての変換を確認
    let expr = parse_expr("arr.forEach((x: number) => console.log(x));");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("Math.floor(3.7);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("Math.ceil(x);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("Math.round(x);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("Math.abs(x);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("Math.sqrt(x);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("Math.max(a, b);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("Math.min(a, b);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("Math.pow(x, 2);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("Math.floor(Math.sqrt(x));");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr(r#"parseInt("42");"#);
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr(r#"parseFloat("3.14");"#);
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("isNaN(x);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("Number.isNaN(x);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("Number.isFinite(x);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // `a ?? b` → `a.unwrap_or_else(|| b)`
    let expr = parse_expr("a ?? b;");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // `x as number` → `x as f64` (primitive cast preserved)
    let expr = parse_expr("x as number;");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // `(obj as Foo).bar` → `obj.bar`
    let expr = parse_expr("(obj as Foo).bar;");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("x?.length;");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // Number.isInteger(x) → x.fract() == 0.0
    let expr = parse_expr("Number.isInteger(x);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // Math.sign(x) → x.signum()
    let expr = parse_expr("Math.sign(x);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // Math.trunc(x) → x.trunc()
    let expr = parse_expr("Math.trunc(x);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // Math.log(x) → x.ln()
    let expr = parse_expr("Math.log(x);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // Math.PI → std::f64::consts::PI
    let expr = parse_expr("Math.PI;");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
    assert_eq!(result, Expr::Ident("std::f64::consts::PI".to_string()));
}

#[test]
fn test_convert_expr_math_e_to_consts() {
    // Math.E → std::f64::consts::E
    let expr = parse_expr("Math.E;");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
    assert_eq!(result, Expr::Ident("std::f64::consts::E".to_string()));
}

// --- I-85: NaN / Infinity ---

#[test]
fn test_convert_expr_nan_to_f64_nan() {
    let expr = parse_expr("NaN;");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
    assert_eq!(result, Expr::Ident("f64::NAN".to_string()));
}

#[test]
fn test_convert_expr_infinity_to_f64_infinity() {
    let expr = parse_expr("Infinity;");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
    assert_eq!(result, Expr::Ident("f64::INFINITY".to_string()));
}

#[test]
fn test_convert_expr_slice_to_range_to_vec() {
    // arr.slice(1, 3) → arr[1..3].to_vec()
    let expr = parse_expr("arr.slice(1, 3);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // arr.splice(1, 2) → arr.drain(1..3).collect::<Vec<_>>()
    let expr = parse_expr("arr.splice(1, 2);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // arr.reverse() → arr.reverse() (same name, in-place)
    let expr = parse_expr("arr.reverse();");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // arr.sort() → arr.sort_by(|a, b| a.partial_cmp(b).unwrap())
    let expr = parse_expr("arr.sort();");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // arr.sort((a, b) => a - b) → arr.sort_by(|a, b| (a - b).partial_cmp(&0.0).unwrap())
    let expr = parse_expr("arr.sort((a, b) => a - b);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // arr.indexOf(x) → arr.iter().position(|item| *item == x).map(|i| i as f64).unwrap_or(-1.0)
    let expr = parse_expr("arr.indexOf(x);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // arr.join(",") → arr.join(",") — string literals are already &str in Rust
    let expr = parse_expr("arr.join(\",\");");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // arr.reduce((acc, x) => acc + x, 0) → arr.iter().fold(0, |acc, x| acc + x)
    let expr = parse_expr("arr.reduce((acc, x) => acc + x, 0);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // arr.reduce((acc: number, x: number) => acc + x, 0)
    // → fold closure params should have NO type annotation (Rust infers &T from iter())
    let expr = parse_expr("arr.reduce((acc: number, x: number) => acc + x, 0);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // arr.indexOf(target) → arr.iter().position(...).map(|i| i as f64).unwrap_or(-1.0)
    let expr = parse_expr("arr.indexOf(target);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // arr.join(sep) → arr.join(&sep)
    let expr = parse_expr("arr.join(sep);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // arr.sort() → arr.sort_by(|a, b| a.partial_cmp(b).unwrap())
    let expr = parse_expr("arr.sort();");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // arr.sort((a: number, b: number) => b - a) → sort_by closure params have no type annotation
    let expr = parse_expr("arr.sort((a: number, b: number) => b - a);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // arr.splice(1, 2) → arr.drain(1..3).collect::<Vec<_>>()
    // The range should use integer literals, not float
    let expr = parse_expr("arr.splice(1, 2);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_expr("x?.y;");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let expr = parse_single_expr("x;");
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::String);

    assert_eq!(
        resolve_expr_type(&expr, &env, &TypeRegistry::new()),
        Some(RustType::String)
    );
}

#[test]
fn test_resolve_expr_type_ident_unregistered_returns_none() {
    let expr = parse_single_expr("y;");
    let env = TypeEnv::new();

    assert_eq!(resolve_expr_type(&expr, &env, &TypeRegistry::new()), None);
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
        TypeDef::Struct {
            fields: vec![("field".to_string(), RustType::String)],
            methods: std::collections::HashMap::new(),
        },
    );

    assert_eq!(resolve_expr_type(&expr, &env, &reg), Some(RustType::String));
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
        TypeDef::Struct {
            fields: vec![("other".to_string(), RustType::F64)],
            methods: std::collections::HashMap::new(),
        },
    );

    assert_eq!(resolve_expr_type(&expr, &env, &reg), None);
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
        TypeDef::Struct {
            fields: vec![("field".to_string(), RustType::String)],
            methods: std::collections::HashMap::new(),
        },
    );

    assert_eq!(resolve_expr_type(&expr, &env, &reg), Some(RustType::String));
}

#[test]
fn test_resolve_expr_type_member_obj_unresolvable_returns_none() {
    let expr = parse_single_expr("y.field;");
    let env = TypeEnv::new();

    assert_eq!(resolve_expr_type(&expr, &env, &TypeRegistry::new()), None);
}

#[test]
fn test_resolve_expr_type_paren_delegates_to_inner() {
    let expr = parse_single_expr("(x);");
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::F64);

    assert_eq!(
        resolve_expr_type(&expr, &env, &TypeRegistry::new()),
        Some(RustType::F64)
    );
}

#[test]
fn test_resolve_expr_type_ts_as_returns_target_type() {
    let expr = parse_single_expr("x as string;");
    let env = TypeEnv::new();

    assert_eq!(
        resolve_expr_type(&expr, &env, &TypeRegistry::new()),
        Some(RustType::String)
    );
}

#[test]
fn test_resolve_expr_type_number_literal_returns_f64() {
    let expr = parse_single_expr("42;");
    let env = TypeEnv::new();

    assert_eq!(
        resolve_expr_type(&expr, &env, &TypeRegistry::new()),
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
        TypeDef::Struct {
            fields: vec![(
                "inner".to_string(),
                RustType::Named {
                    name: "Inner".to_string(),
                    type_args: vec![],
                },
            )],
            methods: std::collections::HashMap::new(),
        },
    );
    reg.register(
        "Inner".to_string(),
        TypeDef::Struct {
            fields: vec![("name".to_string(), RustType::String)],
            methods: std::collections::HashMap::new(),
        },
    );

    assert_eq!(resolve_expr_type(&expr, &env, &reg), Some(RustType::String));
}

// --- TypeEnv-aware optional chaining tests ---

#[test]
fn test_convert_opt_chain_non_option_type_returns_plain_access() {
    let expr = parse_single_expr("x?.y;");
    let mut env = TypeEnv::new();
    env.insert(
        "x".to_string(),
        RustType::Named {
            name: "Foo".to_string(),
            type_args: vec![],
        },
    );

    let result = convert_expr(&expr, &TypeRegistry::new(), None, &env).unwrap();
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
    let expr = parse_single_expr("x?.y;");
    let mut env = TypeEnv::new();
    env.insert(
        "x".to_string(),
        RustType::Option(Box::new(RustType::Named {
            name: "Foo".to_string(),
            type_args: vec![],
        })),
    );

    let result = convert_expr(&expr, &TypeRegistry::new(), None, &env).unwrap();
    assert!(matches!(
        &result,
        Expr::MethodCall { method, .. } if method == "map"
    ));
}

#[test]
fn test_convert_opt_chain_unknown_type_returns_map_pattern() {
    let expr = parse_single_expr("x?.y;");
    let env = TypeEnv::new();

    let result = convert_expr(&expr, &TypeRegistry::new(), None, &env).unwrap();
    assert!(matches!(
        &result,
        Expr::MethodCall { method, .. } if method == "map"
    ));
}

// --- I-81: Optional chaining method name mapping ---

#[test]
fn test_opt_chain_method_call_maps_to_rust_name() {
    // s?.toUpperCase() → s.as_ref().map(|_v| _v.to_uppercase())
    let expr = parse_single_expr("s?.toUpperCase();");
    let mut env = TypeEnv::new();
    env.insert(
        "s".to_string(),
        RustType::Option(Box::new(RustType::String)),
    );
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &env).unwrap();
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
    let expr = parse_single_expr("x ?? y;");
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::String);

    let result = convert_expr(&expr, &TypeRegistry::new(), None, &env).unwrap();
    assert_eq!(result, Expr::Ident("x".to_string()));
}

#[test]
fn test_convert_nullish_coalescing_option_returns_unwrap_or_else() {
    let expr = parse_single_expr("x ?? y;");
    let mut env = TypeEnv::new();
    env.insert(
        "x".to_string(),
        RustType::Option(Box::new(RustType::String)),
    );

    let result = convert_expr(&expr, &TypeRegistry::new(), None, &env).unwrap();
    assert!(matches!(
        &result,
        Expr::MethodCall { method, .. } if method == "unwrap_or_else"
    ));
}

#[test]
fn test_convert_nullish_coalescing_unknown_type_returns_unwrap_or_else() {
    let expr = parse_single_expr("x ?? y;");
    let env = TypeEnv::new();

    let result = convert_expr(&expr, &TypeRegistry::new(), None, &env).unwrap();
    assert!(matches!(
        &result,
        Expr::MethodCall { method, .. } if method == "unwrap_or_else"
    ));
}

// --- Nested optional chaining tests ---

#[test]
fn test_convert_opt_chain_nested_option_uses_and_then() {
    // x?.y?.z where x: Option<Foo>, Foo.y: Option<Bar>, Bar.z: String
    // Should use .and_then() for the inner chain to avoid Option<Option<T>>
    let expr = parse_single_expr("x?.y?.z;");
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
        TypeDef::Struct {
            fields: vec![(
                "y".to_string(),
                RustType::Option(Box::new(RustType::Named {
                    name: "Bar".to_string(),
                    type_args: vec![],
                })),
            )],
            methods: std::collections::HashMap::new(),
        },
    );
    reg.register(
        "Bar".to_string(),
        TypeDef::Struct {
            fields: vec![("z".to_string(), RustType::String)],
            methods: std::collections::HashMap::new(),
        },
    );

    let result = convert_expr(&expr, &reg, None, &env).unwrap();
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
    // foo([...arr, 1]) — spread in function arg position
    let expr = parse_expr("foo([...arr, 1]);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // [1, ...arr, 2] in expression position (as function arg)
    let expr = parse_expr("foo([1, ...arr, 2]);");
    let result = convert_expr(&expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // "Hello " + name → BinaryOp { left: StringLit, op: Add, right: Ref(Ident) }
    let swc_expr = parse_expr(r#""Hello " + name"#);
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    // "Hello " + name + "!" → outer Add: LHS is Add(StringLit, Ref(Ident)), RHS should be Ref(StringLit("!"))
    // Actually "!" is a literal, so it gets .to_string() in Rust, which is already &str-compatible
    // But the pattern is: greeting + " " + name
    let swc_expr = parse_expr(r#"greeting + " " + name"#);
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
    // The outer BinaryOp's left is also a BinaryOp with Add
    // We just verify the structure doesn't panic and produces BinaryOp
    assert!(matches!(result, Expr::BinaryOp { op: BinOp::Add, .. }));
}

#[test]
fn test_numeric_add_no_ref() {
    // a + b (numeric) should NOT get Ref
    let swc_expr = parse_expr("a + b");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
        },
    );

    let swc_expr = parse_expr("greet(\"World\")");
    let result = convert_expr(&swc_expr, &reg, None, &TypeEnv::new()).unwrap();

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
        },
    );

    let swc_expr = parse_expr("greet(\"World\", \"Hi\")");
    let result = convert_expr(&swc_expr, &reg, None, &TypeEnv::new()).unwrap();

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
        },
    );

    assert_eq!(resolve_expr_type(&expr, &env, &reg), Some(RustType::String));
}

#[test]
fn test_resolve_expr_type_call_unregistered_fn_returns_none() {
    let expr = parse_single_expr("unknown();");
    let env = TypeEnv::new();

    assert_eq!(resolve_expr_type(&expr, &env, &TypeRegistry::new()), None);
}

#[test]
fn test_resolve_expr_type_call_fn_type_in_env_returns_return_type() {
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
        resolve_expr_type(&expr, &env, &TypeRegistry::new()),
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
        },
    );

    assert_eq!(resolve_expr_type(&expr, &env, &reg), Some(RustType::Unit));
}

// --- resolve_expr_type: array index ---

#[test]
fn test_resolve_expr_type_index_vec_returns_element_type() {
    let expr = parse_single_expr("arr[0];");
    let mut env = TypeEnv::new();
    env.insert("arr".to_string(), RustType::Vec(Box::new(RustType::String)));

    assert_eq!(
        resolve_expr_type(&expr, &env, &TypeRegistry::new()),
        Some(RustType::String)
    );
}

#[test]
fn test_resolve_expr_type_index_non_vec_returns_none() {
    let expr = parse_single_expr("x[0];");
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::String);

    assert_eq!(resolve_expr_type(&expr, &env, &TypeRegistry::new()), None);
}

// --- resolve_expr_type: binary operations ---

#[test]
fn test_resolve_expr_type_comparison_returns_bool() {
    let expr = parse_single_expr("x > y;");
    let env = TypeEnv::new();

    assert_eq!(
        resolve_expr_type(&expr, &env, &TypeRegistry::new()),
        Some(RustType::Bool)
    );
}

#[test]
fn test_resolve_expr_type_equality_returns_bool() {
    let expr = parse_single_expr("x === y;");
    let env = TypeEnv::new();

    assert_eq!(
        resolve_expr_type(&expr, &env, &TypeRegistry::new()),
        Some(RustType::Bool)
    );
}

#[test]
fn test_resolve_expr_type_logical_and_returns_operand_type() {
    let expr = parse_single_expr("a && b;");
    let mut env = TypeEnv::new();
    env.insert("a".to_string(), RustType::String);
    env.insert("b".to_string(), RustType::String);

    assert_eq!(
        resolve_expr_type(&expr, &env, &TypeRegistry::new()),
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
        TypeDef::Struct {
            fields: vec![],
            methods: std::collections::HashMap::new(),
        },
    );

    assert_eq!(
        resolve_expr_type(&expr, &env, &reg),
        Some(RustType::Named {
            name: "Foo".to_string(),
            type_args: vec![],
        })
    );
}

#[test]
fn test_resolve_expr_type_new_unregistered_returns_none() {
    let expr = parse_single_expr("new Unknown();");
    let env = TypeEnv::new();

    assert_eq!(resolve_expr_type(&expr, &env, &TypeRegistry::new()), None);
}

// --- Step 5: expected 型伝搬テスト ---

#[test]
fn test_convert_bin_expr_expected_string_enables_concat() {
    // a + b with expected=String → string concat context (RHS wrapped in Ref)
    let swc_expr = parse_expr("a + b;");
    let reg = TypeRegistry::new();
    let env = TypeEnv::new(); // a, b not registered → types unknown

    let result = convert_expr(&swc_expr, &reg, Some(&RustType::String), &env).unwrap();

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
    let reg = TypeRegistry::new();
    let env = TypeEnv::new();

    let result = convert_expr(&swc_expr, &reg, None, &env).unwrap();

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
    // f("hello") where TypeEnv has f: Fn { params: [String], return_type: Bool }
    // → "hello" should become "hello".to_string() because expected=String
    let swc_expr = parse_expr("f(\"hello\");");
    let reg = TypeRegistry::new();
    let mut env = TypeEnv::new();
    env.insert(
        "f".to_string(),
        RustType::Fn {
            params: vec![RustType::String],
            return_type: Box::new(RustType::Bool),
        },
    );

    let result = convert_expr(&swc_expr, &reg, None, &env).unwrap();

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
    let reg = TypeRegistry::new();
    let env = TypeEnv::new();

    let result = convert_expr(&swc_expr, &reg, None, &env).unwrap();

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

// --- Step 8: I-16 空配列の型推論テスト ---

#[test]
fn test_convert_array_lit_empty_with_expected_vec_string() {
    // [] with expected=Vec<String> → Expr::Vec with no elements (type comes from context)
    let swc_expr = parse_var_init("const x = [];");
    let reg = TypeRegistry::new();
    let env = TypeEnv::new();
    let expected = RustType::Vec(Box::new(RustType::String));

    let result = convert_expr(&swc_expr, &reg, Some(&expected), &env).unwrap();

    assert_eq!(result, Expr::Vec { elements: vec![] });
}

#[test]
fn test_convert_array_lit_elements_get_expected_element_type() {
    // ["a", "b"] with expected=Vec<String> → elements get .to_string()
    let swc_expr = parse_var_init("const x = [\"a\", \"b\"];");
    let reg = TypeRegistry::new();
    let env = TypeEnv::new();
    let expected = RustType::Vec(Box::new(RustType::String));

    let result = convert_expr(&swc_expr, &reg, Some(&expected), &env).unwrap();

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

// --- I-45: typeof / instanceof type guard expressions ---

#[test]
fn test_typeof_equals_string_known_type_resolves_true() {
    let swc_expr = parse_expr("typeof x === \"string\";");
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::String);
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &env).unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn test_typeof_equals_string_mismatched_type_resolves_false() {
    let swc_expr = parse_expr("typeof x === \"string\";");
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::F64);
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &env).unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn test_typeof_equals_number_known_type_resolves_true() {
    let swc_expr = parse_expr("typeof x === \"number\";");
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::F64);
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &env).unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn test_typeof_not_equals_string_known_type_resolves_false() {
    let swc_expr = parse_expr("typeof x !== \"string\";");
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::String);
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &env).unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn test_typeof_equals_string_unknown_type_resolves_placeholder() {
    let swc_expr = parse_expr("typeof x === \"string\";");
    let env = TypeEnv::new(); // x not registered
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &env).unwrap();
    // Unknown type → placeholder true
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn test_typeof_equals_undefined_option_resolves_is_none() {
    let swc_expr = parse_expr("typeof x === \"undefined\";");
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::Option(Box::new(RustType::F64)));
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &env).unwrap();
    assert!(
        matches!(&result, Expr::MethodCall { method, .. } if method == "is_none"),
        "expected is_none call, got: {:?}",
        result
    );
}

#[test]
fn test_typeof_standalone_known_type_resolves_string_lit() {
    let swc_expr = parse_expr("typeof x;");
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::String);
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &env).unwrap();
    assert_eq!(result, Expr::StringLit("string".to_string()));
}

#[test]
fn test_instanceof_known_type_match_resolves_true() {
    let swc_expr = parse_expr("x instanceof Foo;");
    let mut env = TypeEnv::new();
    env.insert(
        "x".to_string(),
        RustType::Named {
            name: "Foo".to_string(),
            type_args: vec![],
        },
    );
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &env).unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

#[test]
fn test_instanceof_known_type_mismatch_resolves_false() {
    let swc_expr = parse_expr("x instanceof Foo;");
    let mut env = TypeEnv::new();
    env.insert(
        "x".to_string(),
        RustType::Named {
            name: "Bar".to_string(),
            type_args: vec![],
        },
    );
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &env).unwrap();
    assert_eq!(result, Expr::BoolLit(false));
}

#[test]
fn test_instanceof_unknown_type_resolves_placeholder() {
    let swc_expr = parse_expr("x instanceof Foo;");
    let env = TypeEnv::new();
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &env).unwrap();
    assert_eq!(result, Expr::BoolLit(true));
}

// --- I-68: self.field string concat clone ---

#[test]
fn test_self_field_string_concat_gets_clone() {
    // this.name + " suffix" → self.name.clone() + &" suffix"
    let swc_expr = parse_expr("this.name + \" suffix\";");
    let reg = TypeRegistry::new();
    let mut env = TypeEnv::new();
    // Mark "this" as having a string field to trigger string concat context
    env.insert(
        "this".to_string(),
        RustType::Named {
            name: "Self".to_string(),
            type_args: vec![],
        },
    );
    let result = convert_expr(&swc_expr, &reg, Some(&RustType::String), &env).unwrap();
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

// --- I-65: undefined / Option semantics ---

#[test]
fn test_undefined_literal_converts_to_none() {
    let swc_expr = parse_expr("undefined;");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
    assert_eq!(result, Expr::Ident("None".to_string()));
}

#[test]
fn test_equals_undefined_converts_to_is_none() {
    let swc_expr = parse_expr("x === undefined;");
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::Option(Box::new(RustType::F64)));
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &env).unwrap();
    assert!(
        matches!(&result, Expr::MethodCall { method, .. } if method == "is_none"),
        "expected is_none, got: {:?}",
        result
    );
}

#[test]
fn test_not_equals_undefined_converts_to_is_some() {
    let swc_expr = parse_expr("x !== undefined;");
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::Option(Box::new(RustType::F64)));
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &env).unwrap();
    assert!(
        matches!(&result, Expr::MethodCall { method, .. } if method == "is_some"),
        "expected is_some, got: {:?}",
        result
    );
}

#[test]
fn test_option_expected_wraps_literal_in_some() {
    let swc_expr = parse_expr("42;");
    let expected = RustType::Option(Box::new(RustType::F64));
    let result = convert_expr(
        &swc_expr,
        &TypeRegistry::new(),
        Some(&expected),
        &TypeEnv::new(),
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
    let swc_expr = parse_expr("undefined;");
    let expected = RustType::Option(Box::new(RustType::F64));
    let result = convert_expr(
        &swc_expr,
        &TypeRegistry::new(),
        Some(&expected),
        &TypeEnv::new(),
    )
    .unwrap();
    // Should be None, not Some(None)
    assert_eq!(result, Expr::Ident("None".to_string()));
}

// --- I-90: string literal → enum variant conversion ---

#[test]
fn test_convert_lit_string_to_enum_variant_when_expected_is_string_literal_union() {
    let mut reg = TypeRegistry::new();
    let mut string_values = std::collections::HashMap::new();
    string_values.insert("up".to_string(), "Up".to_string());
    string_values.insert("down".to_string(), "Down".to_string());
    reg.register(
        "Direction".to_string(),
        TypeDef::Enum {
            variants: vec!["Up".to_string(), "Down".to_string()],
            string_values,
            tag_field: None,
            variant_fields: std::collections::HashMap::new(),
        },
    );

    let expected = RustType::Named {
        name: "Direction".to_string(),
        type_args: vec![],
    };
    let swc_expr = parse_expr(r#""up";"#);
    let result = convert_expr(&swc_expr, &reg, Some(&expected), &TypeEnv::new()).unwrap();
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
            variants: vec!["Up".to_string()],
            string_values,
            tag_field: None,
            variant_fields: std::collections::HashMap::new(),
        },
    );

    let expected = RustType::Named {
        name: "Direction".to_string(),
        type_args: vec![],
    };
    let swc_expr = parse_expr(r#""unknown";"#);
    let result = convert_expr(&swc_expr, &reg, Some(&expected), &TypeEnv::new()).unwrap();
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
            variants: vec!["Up".to_string(), "Down".to_string()],
            string_values,
            tag_field: None,
            variant_fields: std::collections::HashMap::new(),
        },
    );

    let mut type_env = TypeEnv::new();
    type_env.insert(
        "d".to_string(),
        RustType::Named {
            name: "Direction".to_string(),
            type_args: vec![],
        },
    );

    let swc_expr = parse_expr(r#"d == "up";"#);
    let result = convert_expr(&swc_expr, &reg, None, &type_env).unwrap();
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
            variants: vec!["Up".to_string()],
            string_values,
            tag_field: None,
            variant_fields: std::collections::HashMap::new(),
        },
    );

    let mut type_env = TypeEnv::new();
    type_env.insert(
        "d".to_string(),
        RustType::Named {
            name: "Direction".to_string(),
            type_args: vec![],
        },
    );

    let swc_expr = parse_expr(r#""up" != d;"#);
    let result = convert_expr(&swc_expr, &reg, None, &type_env).unwrap();
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
        },
    );

    let swc_expr = parse_expr(r#"move_dir("up");"#);
    let result = convert_expr(&swc_expr, &reg, None, &TypeEnv::new()).unwrap();
    assert_eq!(
        result,
        Expr::FnCall {
            name: "move_dir".to_string(),
            args: vec![Expr::Ident("Direction::Up".to_string())],
        }
    );
}

// --- I-91: discriminated union object literal → enum variant ---

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
            variants: vec!["Circle".to_string(), "Square".to_string()],
            string_values,
            tag_field: Some("kind".to_string()),
            variant_fields,
        },
    );

    let expected = RustType::Named {
        name: "Shape".to_string(),
        type_args: vec![],
    };
    let swc_expr = parse_var_init(r#"const s: Shape = { kind: "circle", radius: 5 };"#);
    let result = convert_expr(&swc_expr, &reg, Some(&expected), &TypeEnv::new()).unwrap();
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
            variants: vec!["Active".to_string()],
            string_values,
            tag_field: Some("type".to_string()),
            variant_fields,
        },
    );

    let expected = RustType::Named {
        name: "Status".to_string(),
        type_args: vec![],
    };
    let swc_expr = parse_var_init(r#"const s: Status = { type: "active" };"#);
    let result = convert_expr(&swc_expr, &reg, Some(&expected), &TypeEnv::new()).unwrap();
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
            variants: vec!["Circle".to_string()],
            string_values,
            tag_field: Some("kind".to_string()),
            variant_fields,
        },
    );

    let mut type_env = TypeEnv::new();
    type_env.insert(
        "s".to_string(),
        RustType::Named {
            name: "Shape".to_string(),
            type_args: vec![],
        },
    );

    let swc_expr = parse_expr("s.kind;");
    let result = convert_expr(&swc_expr, &reg, None, &type_env).unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::Ident("s".to_string())),
            method: "kind".to_string(),
            args: vec![],
        }
    );
}

// --- I-46: computed property (index access) ---

#[test]
fn test_convert_member_expr_array_index_literal_generates_index() {
    let swc_expr = parse_expr("arr[0];");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
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
    let swc_expr = parse_expr("arr[i];");
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &TypeEnv::new()).unwrap();
    assert_eq!(
        result,
        Expr::Index {
            object: Box::new(Expr::Ident("arr".to_string())),
            index: Box::new(Expr::Ident("i".to_string())),
        }
    );
}

// --- I-95: nullish coalescing expected type propagation ---

#[test]
fn test_convert_nullish_coalescing_rhs_string_gets_to_string_when_lhs_is_option_string() {
    let mut type_env = TypeEnv::new();
    type_env.insert(
        "s".to_string(),
        RustType::Option(Box::new(RustType::String)),
    );

    let swc_expr = parse_expr(r#"s ?? "default";"#);
    let result = convert_expr(&swc_expr, &TypeRegistry::new(), None, &type_env).unwrap();

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

// --- I-94: method argument type lookup ---

#[test]
fn test_convert_method_call_string_arg_gets_to_string_with_registry() {
    let mut reg = TypeRegistry::new();
    let mut methods = std::collections::HashMap::new();
    methods.insert(
        "greet".to_string(),
        vec![("name".to_string(), RustType::String)],
    );
    reg.register(
        "Greeter".to_string(),
        TypeDef::Struct {
            fields: vec![],
            methods,
        },
    );

    let mut type_env = TypeEnv::new();
    type_env.insert(
        "g".to_string(),
        RustType::Named {
            name: "Greeter".to_string(),
            type_args: vec![],
        },
    );

    let swc_expr = parse_expr(r#"g.greet("world");"#);
    let result = convert_expr(&swc_expr, &reg, None, &type_env).unwrap();

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

// --- I-97: discriminated union standalone field access ---

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
    let mut type_env = TypeEnv::new();
    type_env.insert(
        "s".to_string(),
        RustType::Named {
            name: "Shape".to_string(),
            type_args: vec![],
        },
    );

    // s.radius → match expression
    let swc_expr = parse_var_init("const x = s.radius;");
    let result = convert_expr(&swc_expr, &reg, None, &type_env).unwrap();

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
