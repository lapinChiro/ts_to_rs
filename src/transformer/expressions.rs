//! Expression conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC expression nodes into the IR [`Expr`] representation.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{ClosureBody, Expr, Param};
use crate::transformer::statements::convert_stmt;
use crate::transformer::types::convert_ts_type;

/// Converts an SWC [`ast::Expr`] into an IR [`Expr`].
///
/// # Supported conversions
///
/// - Identifiers → `Expr::Ident`
/// - Number literals → `Expr::NumberLit`
/// - String literals → `Expr::StringLit`
/// - Boolean literals → `Expr::BoolLit`
/// - Template literals → `Expr::FormatMacro`
/// - Binary expressions → `Expr::BinaryOp`
/// - Object literals (with type hint) → `Expr::StructInit`
///
/// # Errors
///
/// Returns an error for unsupported expression types.
pub fn convert_expr(expr: &ast::Expr) -> Result<Expr> {
    convert_expr_with_type_hint(expr, None)
}

/// Converts an SWC [`ast::Expr`] into an IR [`Expr`], with an optional type hint.
///
/// The `type_hint` is used for object literals to determine the struct name.
/// When a variable declaration has a type annotation (e.g., `const p: Point = { x: 1, y: 2 }`),
/// the type name `"Point"` is passed as the hint.
pub fn convert_expr_with_type_hint(expr: &ast::Expr, type_hint: Option<&str>) -> Result<Expr> {
    match expr {
        ast::Expr::Ident(ident) => Ok(Expr::Ident(ident.sym.to_string())),
        ast::Expr::Lit(lit) => convert_lit(lit),
        ast::Expr::Bin(bin) => convert_bin_expr(bin),
        ast::Expr::Tpl(tpl) => convert_template_literal(tpl),
        ast::Expr::Paren(paren) => convert_expr_with_type_hint(&paren.expr, type_hint),
        ast::Expr::Member(member) => convert_member_expr(member),
        ast::Expr::This(_) => Ok(Expr::Ident("self".to_string())),
        ast::Expr::Assign(assign) => convert_assign_expr(assign),
        ast::Expr::Arrow(arrow) => convert_arrow_expr(arrow),
        ast::Expr::Call(call) => convert_call_expr(call),
        ast::Expr::New(new_expr) => convert_new_expr(new_expr),
        ast::Expr::Array(array_lit) => convert_array_lit(array_lit),
        ast::Expr::Object(obj_lit) => convert_object_lit(obj_lit, type_hint),
        _ => Err(anyhow!("unsupported expression: {:?}", expr)),
    }
}

/// Converts an SWC literal to an IR expression.
fn convert_lit(lit: &ast::Lit) -> Result<Expr> {
    match lit {
        ast::Lit::Num(n) => Ok(Expr::NumberLit(n.value)),
        ast::Lit::Str(s) => Ok(Expr::StringLit(s.value.to_string_lossy().into_owned())),
        ast::Lit::Bool(b) => Ok(Expr::BoolLit(b.value)),
        _ => Err(anyhow!("unsupported literal: {:?}", lit)),
    }
}

/// Converts an SWC binary expression to an IR `BinaryOp`.
fn convert_bin_expr(bin: &ast::BinExpr) -> Result<Expr> {
    let left = convert_expr(&bin.left)?;
    let right = convert_expr(&bin.right)?;
    let op = convert_binary_op(bin.op)?;
    Ok(Expr::BinaryOp {
        left: Box::new(left),
        op,
        right: Box::new(right),
    })
}

/// Converts an SWC binary operator to its Rust string representation.
fn convert_binary_op(op: ast::BinaryOp) -> Result<String> {
    let s = match op {
        ast::BinaryOp::Add => "+",
        ast::BinaryOp::Sub => "-",
        ast::BinaryOp::Mul => "*",
        ast::BinaryOp::Div => "/",
        ast::BinaryOp::Mod => "%",
        ast::BinaryOp::EqEq | ast::BinaryOp::EqEqEq => "==",
        ast::BinaryOp::NotEq | ast::BinaryOp::NotEqEq => "!=",
        ast::BinaryOp::Lt => "<",
        ast::BinaryOp::LtEq => "<=",
        ast::BinaryOp::Gt => ">",
        ast::BinaryOp::GtEq => ">=",
        ast::BinaryOp::LogicalAnd => "&&",
        ast::BinaryOp::LogicalOr => "||",
        _ => return Err(anyhow!("unsupported binary operator: {:?}", op)),
    };
    Ok(s.to_string())
}

/// Converts a member expression (`obj.field`) to `Expr::FieldAccess`.
///
/// `this.x` becomes `self.x`.
fn convert_member_expr(member: &ast::MemberExpr) -> Result<Expr> {
    let object = convert_expr(&member.obj)?;
    let field = match &member.prop {
        ast::MemberProp::Ident(ident) => ident.sym.to_string(),
        _ => return Err(anyhow!("unsupported member property (only identifiers)")),
    };
    Ok(Expr::FieldAccess {
        object: Box::new(object),
        field,
    })
}

/// Converts an assignment expression (`target = value`) to `Expr::Assign`.
fn convert_assign_expr(assign: &ast::AssignExpr) -> Result<Expr> {
    let target = match &assign.left {
        ast::AssignTarget::Simple(simple) => match simple {
            ast::SimpleAssignTarget::Member(member) => convert_member_expr(member)?,
            ast::SimpleAssignTarget::Ident(ident) => Expr::Ident(ident.id.sym.to_string()),
            _ => return Err(anyhow!("unsupported assignment target")),
        },
        _ => return Err(anyhow!("unsupported assignment target pattern")),
    };
    let value = convert_expr(&assign.right)?;
    Ok(Expr::Assign {
        target: Box::new(target),
        value: Box::new(value),
    })
}

/// Converts an arrow function expression to `Expr::Closure`.
///
/// - Expression body: `(x: number) => x + 1` → `|x: f64| x + 1`
/// - Block body: `(x: number) => { return x + 1; }` → `|x: f64| { x + 1 }`
fn convert_arrow_expr(arrow: &ast::ArrowExpr) -> Result<Expr> {
    let mut params = Vec::new();
    for param in &arrow.params {
        match param {
            ast::Pat::Ident(ident) => {
                let name = ident.id.sym.to_string();
                let ty = ident
                    .type_ann
                    .as_ref()
                    .ok_or_else(|| anyhow!("arrow parameter '{}' has no type annotation", name))?;
                let rust_type = convert_ts_type(&ty.type_ann)?;
                params.push(Param {
                    name,
                    ty: rust_type,
                });
            }
            _ => return Err(anyhow!("unsupported arrow parameter pattern")),
        }
    }

    let return_type = arrow
        .return_type
        .as_ref()
        .map(|ann| convert_ts_type(&ann.type_ann))
        .transpose()?;

    let body = match arrow.body.as_ref() {
        ast::BlockStmtOrExpr::Expr(expr) => {
            let ir_expr = convert_expr(expr)?;
            ClosureBody::Expr(Box::new(ir_expr))
        }
        ast::BlockStmtOrExpr::BlockStmt(block) => {
            let mut stmts = Vec::new();
            for stmt in &block.stmts {
                stmts.push(convert_stmt(stmt)?);
            }
            ClosureBody::Block(stmts)
        }
    };

    Ok(Expr::Closure {
        params,
        return_type,
        body,
    })
}

/// Converts a function/method call expression.
///
/// - `foo(x, y)` → `Expr::FnCall { name: "foo", args }`
/// - `obj.method(x)` → `Expr::MethodCall { object, method, args }`
fn convert_call_expr(call: &ast::CallExpr) -> Result<Expr> {
    let args = convert_call_args(&call.args)?;

    match call.callee {
        ast::Callee::Expr(ref callee) => match callee.as_ref() {
            ast::Expr::Ident(ident) => Ok(Expr::FnCall {
                name: ident.sym.to_string(),
                args,
            }),
            ast::Expr::Member(member) => {
                let object = convert_expr(&member.obj)?;
                let method = match &member.prop {
                    ast::MemberProp::Ident(ident) => ident.sym.to_string(),
                    _ => return Err(anyhow!("unsupported call target member property")),
                };
                Ok(Expr::MethodCall {
                    object: Box::new(object),
                    method,
                    args,
                })
            }
            _ => Err(anyhow!("unsupported call target expression")),
        },
        _ => Err(anyhow!("unsupported callee type")),
    }
}

/// Converts a `new` expression to a `ClassName::new(args)` call.
///
/// `new Foo(x, y)` → `Expr::FnCall { name: "Foo::new", args }`
fn convert_new_expr(new_expr: &ast::NewExpr) -> Result<Expr> {
    let class_name = match new_expr.callee.as_ref() {
        ast::Expr::Ident(ident) => ident.sym.to_string(),
        _ => return Err(anyhow!("unsupported new expression target")),
    };
    let args = match &new_expr.args {
        Some(args) => convert_call_args(args)?,
        None => vec![],
    };
    Ok(Expr::FnCall {
        name: format!("{class_name}::new"),
        args,
    })
}

/// Converts call arguments from SWC `ExprOrSpread` to IR `Expr`.
fn convert_call_args(args: &[ast::ExprOrSpread]) -> Result<Vec<Expr>> {
    args.iter().map(|arg| convert_expr(&arg.expr)).collect()
}

/// Converts a template literal to `Expr::FormatMacro`.
///
/// `` `Hello ${name}` `` becomes `format!("Hello {}", name)`.
fn convert_template_literal(tpl: &ast::Tpl) -> Result<Expr> {
    let mut template = String::new();
    let mut args = Vec::new();

    for (i, quasi) in tpl.quasis.iter().enumerate() {
        // raw text of the quasi (the string parts between expressions)
        template.push_str(&quasi.raw);
        if i < tpl.exprs.len() {
            template.push_str("{}");
            let arg = convert_expr(&tpl.exprs[i])?;
            args.push(arg);
        }
    }

    Ok(Expr::FormatMacro { template, args })
}

/// Converts an SWC object literal to an IR `Expr::StructInit`.
///
/// Requires a type hint (struct name) from the enclosing context (e.g., a variable declaration's
/// type annotation). Without a type hint, returns an error because Rust requires a named struct.
///
/// `{ x: 1, y: 2 }` with type hint `"Point"` → `Expr::StructInit { name: "Point", fields: [...] }`
fn convert_object_lit(obj_lit: &ast::ObjectLit, type_hint: Option<&str>) -> Result<Expr> {
    let struct_name = type_hint.ok_or_else(|| {
        anyhow!("object literal requires a type annotation to determine struct name")
    })?;

    let mut fields = Vec::new();
    for prop in &obj_lit.props {
        match prop {
            ast::PropOrSpread::Prop(prop) => match prop.as_ref() {
                ast::Prop::KeyValue(kv) => {
                    let key = match &kv.key {
                        ast::PropName::Ident(ident) => ident.sym.to_string(),
                        ast::PropName::Str(s) => s.value.to_string_lossy().into_owned(),
                        _ => return Err(anyhow!("unsupported object literal key")),
                    };
                    let value = convert_expr(&kv.value)?;
                    fields.push((key, value));
                }
                _ => {
                    return Err(anyhow!(
                        "unsupported object literal property (only key-value pairs)"
                    ))
                }
            },
            ast::PropOrSpread::Spread(_) => {
                return Err(anyhow!("spread in object literal is not supported"));
            }
        }
    }

    Ok(Expr::StructInit {
        name: struct_name.to_string(),
        fields,
    })
}

/// Converts an SWC array literal to an IR `Expr::Vec`.
fn convert_array_lit(array_lit: &ast::ArrayLit) -> Result<Expr> {
    let elements = array_lit
        .elems
        .iter()
        .filter_map(|elem| elem.as_ref())
        .map(|elem| convert_expr(&elem.expr))
        .collect::<Result<Vec<_>>>()?;
    Ok(Expr::Vec { elements })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_typescript;
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
        let result = convert_expr(&swc_expr).unwrap();
        assert_eq!(result, Expr::Ident("foo".to_string()));
    }

    #[test]
    fn test_convert_expr_number_literal() {
        let swc_expr = parse_expr("42;");
        let result = convert_expr(&swc_expr).unwrap();
        assert_eq!(result, Expr::NumberLit(42.0));
    }

    #[test]
    fn test_convert_expr_string_literal() {
        let swc_expr = parse_expr("\"hello\";");
        let result = convert_expr(&swc_expr).unwrap();
        assert_eq!(result, Expr::StringLit("hello".to_string()));
    }

    #[test]
    fn test_convert_expr_bool_true() {
        let swc_expr = parse_expr("true;");
        let result = convert_expr(&swc_expr).unwrap();
        assert_eq!(result, Expr::BoolLit(true));
    }

    #[test]
    fn test_convert_expr_bool_false() {
        let swc_expr = parse_expr("false;");
        let result = convert_expr(&swc_expr).unwrap();
        assert_eq!(result, Expr::BoolLit(false));
    }

    #[test]
    fn test_convert_expr_binary_add() {
        let swc_expr = parse_var_init("const x = a + b;");
        let result = convert_expr(&swc_expr).unwrap();
        assert_eq!(
            result,
            Expr::BinaryOp {
                left: Box::new(Expr::Ident("a".to_string())),
                op: "+".to_string(),
                right: Box::new(Expr::Ident("b".to_string())),
            }
        );
    }

    #[test]
    fn test_convert_expr_binary_greater_than() {
        let swc_expr = parse_var_init("const x = a > b;");
        let result = convert_expr(&swc_expr).unwrap();
        assert_eq!(
            result,
            Expr::BinaryOp {
                left: Box::new(Expr::Ident("a".to_string())),
                op: ">".to_string(),
                right: Box::new(Expr::Ident("b".to_string())),
            }
        );
    }

    #[test]
    fn test_convert_expr_binary_strict_equals() {
        let swc_expr = parse_var_init("const x = a === b;");
        let result = convert_expr(&swc_expr).unwrap();
        assert_eq!(
            result,
            Expr::BinaryOp {
                left: Box::new(Expr::Ident("a".to_string())),
                op: "==".to_string(),
                right: Box::new(Expr::Ident("b".to_string())),
            }
        );
    }

    #[test]
    fn test_convert_expr_template_literal() {
        let swc_expr = parse_var_init("const x = `Hello ${name}`;");
        let result = convert_expr(&swc_expr).unwrap();
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
        let result = convert_expr(&swc_expr).unwrap();
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
        let result = convert_expr(&swc_expr).unwrap();
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
        let result = convert_expr(&swc_expr).unwrap();
        match result {
            Expr::Closure {
                params,
                return_type,
                body,
            } => {
                assert_eq!(params.len(), 1);
                assert_eq!(params[0].name, "x");
                assert_eq!(params[0].ty, crate::ir::RustType::F64);
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
        let result = convert_expr(&swc_expr).unwrap();
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
        let result = convert_expr(&swc_expr).unwrap();
        match result {
            Expr::Closure { params, body, .. } => {
                assert!(params.is_empty());
                assert!(matches!(body, crate::ir::ClosureBody::Expr(_)));
            }
            _ => panic!("expected Expr::Closure"),
        }
    }

    // -- Function call tests --

    #[test]
    fn test_convert_expr_call_simple() {
        let swc_expr = parse_expr("foo(x, y);");
        let result = convert_expr(&swc_expr).unwrap();
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
        let result = convert_expr(&swc_expr).unwrap();
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
        let result = convert_expr(&swc_expr).unwrap();
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
        let result = convert_expr(&swc_expr).unwrap();
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
        let result = convert_expr(&swc_expr).unwrap();
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
        let result = convert_expr(&swc_expr).unwrap();
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
        let result = convert_expr(&swc_expr).unwrap();
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
        let result = convert_expr(&swc_expr).unwrap();
        assert_eq!(
            result,
            Expr::FnCall {
                name: "Foo::new".to_string(),
                args: vec![],
            }
        );
    }

    #[test]
    fn test_convert_expr_template_literal_no_exprs() {
        let swc_expr = parse_var_init("const x = `hello world`;");
        let result = convert_expr(&swc_expr).unwrap();
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
        let result = convert_expr(&expr).unwrap();
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
        let result = convert_expr(&expr).unwrap();
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
        let result = convert_expr(&expr).unwrap();
        assert_eq!(result, Expr::Vec { elements: vec![] });
    }

    #[test]
    fn test_convert_expr_array_single_element() {
        let expr = parse_var_init("const a = [42];");
        let result = convert_expr(&expr).unwrap();
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
        // { x: 1, y: 2 } with type hint "Point"
        let swc_expr = parse_var_init("const p: Point = { x: 1, y: 2 };");
        let result = convert_expr_with_type_hint(&swc_expr, Some("Point")).unwrap();
        assert_eq!(
            result,
            Expr::StructInit {
                name: "Point".to_string(),
                fields: vec![
                    ("x".to_string(), Expr::NumberLit(1.0)),
                    ("y".to_string(), Expr::NumberLit(2.0)),
                ],
            }
        );
    }

    #[test]
    fn test_convert_expr_object_literal_mixed_field_types() {
        let swc_expr =
            parse_var_init(r#"const c: Config = { name: "foo", count: 42, active: true };"#);
        let result = convert_expr_with_type_hint(&swc_expr, Some("Config")).unwrap();
        assert_eq!(
            result,
            Expr::StructInit {
                name: "Config".to_string(),
                fields: vec![
                    ("name".to_string(), Expr::StringLit("foo".to_string())),
                    ("count".to_string(), Expr::NumberLit(42.0)),
                    ("active".to_string(), Expr::BoolLit(true)),
                ],
            }
        );
    }

    #[test]
    fn test_convert_expr_object_literal_single_field() {
        let swc_expr = parse_var_init("const w: Wrapper = { value: 10 };");
        let result = convert_expr_with_type_hint(&swc_expr, Some("Wrapper")).unwrap();
        assert_eq!(
            result,
            Expr::StructInit {
                name: "Wrapper".to_string(),
                fields: vec![("value".to_string(), Expr::NumberLit(10.0))],
            }
        );
    }

    #[test]
    fn test_convert_expr_object_literal_empty() {
        let swc_expr = parse_var_init("const e: Empty = {};");
        let result = convert_expr_with_type_hint(&swc_expr, Some("Empty")).unwrap();
        assert_eq!(
            result,
            Expr::StructInit {
                name: "Empty".to_string(),
                fields: vec![],
            }
        );
    }

    #[test]
    fn test_convert_expr_object_literal_without_type_hint_errors() {
        let swc_expr = parse_var_init("const obj = { x: 1 };");
        let result = convert_expr(&swc_expr);
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_expr_array_nested() {
        let expr = parse_var_init("const a = [[1, 2], [3]];");
        let result = convert_expr(&expr).unwrap();
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
}
