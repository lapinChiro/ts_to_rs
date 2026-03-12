//! Expression conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC expression nodes into the IR [`Expr`] representation.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{ClosureBody, Expr, Param, RustType};
use crate::registry::{TypeDef, TypeRegistry};
use crate::transformer::statements::convert_stmt;
use crate::transformer::types::convert_ts_type;

/// Converts an SWC [`ast::Expr`] into an IR [`Expr`], with an optional expected type.
///
/// The `expected` type is used for:
/// - Object literals: determines the struct name from `RustType::Named`
/// - String literals: adds `.to_string()` when `RustType::String` is expected
/// - Array literals: propagates element type from `RustType::Vec`
///
/// # Errors
///
/// Returns an error for unsupported expression types.
pub fn convert_expr(
    expr: &ast::Expr,
    reg: &TypeRegistry,
    expected: Option<&RustType>,
) -> Result<Expr> {
    match expr {
        ast::Expr::Ident(ident) => Ok(Expr::Ident(ident.sym.to_string())),
        ast::Expr::Lit(lit) => convert_lit(lit, expected),
        ast::Expr::Bin(bin) => convert_bin_expr(bin, reg),
        ast::Expr::Tpl(tpl) => convert_template_literal(tpl, reg),
        ast::Expr::Paren(paren) => convert_expr(&paren.expr, reg, expected),
        ast::Expr::Member(member) => convert_member_expr(member, reg),
        ast::Expr::This(_) => Ok(Expr::Ident("self".to_string())),
        ast::Expr::Assign(assign) => convert_assign_expr(assign, reg),
        ast::Expr::Arrow(arrow) => convert_arrow_expr(arrow, reg),
        ast::Expr::Call(call) => convert_call_expr(call, reg),
        ast::Expr::New(new_expr) => convert_new_expr(new_expr, reg),
        ast::Expr::Array(array_lit) => convert_array_lit(array_lit, reg, expected),
        ast::Expr::Object(obj_lit) => convert_object_lit(obj_lit, reg, expected),
        ast::Expr::Cond(cond) => convert_cond_expr(cond, reg, expected),
        _ => Err(anyhow!("unsupported expression: {:?}", expr)),
    }
}

/// Converts an SWC literal to an IR expression.
///
/// When `expected` is `RustType::String`, string literals are wrapped with `.to_string()`
/// to produce an owned `String` instead of `&str`.
fn convert_lit(lit: &ast::Lit, expected: Option<&RustType>) -> Result<Expr> {
    match lit {
        ast::Lit::Num(n) => Ok(Expr::NumberLit(n.value)),
        ast::Lit::Str(s) => {
            let expr = Expr::StringLit(s.value.to_string_lossy().into_owned());
            if matches!(expected, Some(RustType::String)) {
                Ok(Expr::MethodCall {
                    object: Box::new(expr),
                    method: "to_string".to_string(),
                    args: vec![],
                })
            } else {
                Ok(expr)
            }
        }
        ast::Lit::Bool(b) => Ok(Expr::BoolLit(b.value)),
        _ => Err(anyhow!("unsupported literal: {:?}", lit)),
    }
}

/// Converts an SWC binary expression to an IR `BinaryOp`.
fn convert_bin_expr(bin: &ast::BinExpr, reg: &TypeRegistry) -> Result<Expr> {
    let left = convert_expr(&bin.left, reg, None)?;
    let right = convert_expr(&bin.right, reg, None)?;
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
fn convert_member_expr(member: &ast::MemberExpr, reg: &TypeRegistry) -> Result<Expr> {
    let field = match &member.prop {
        ast::MemberProp::Ident(ident) => ident.sym.to_string(),
        _ => return Err(anyhow!("unsupported member property (only identifiers)")),
    };

    // Check if the object is an identifier referring to an enum in the registry
    if let ast::Expr::Ident(ident) = member.obj.as_ref() {
        let name = ident.sym.as_ref();
        if let Some(TypeDef::Enum { .. }) = reg.get(name) {
            return Ok(Expr::Ident(format!("{name}::{field}")));
        }
    }

    let object = convert_expr(&member.obj, reg, None)?;
    Ok(Expr::FieldAccess {
        object: Box::new(object),
        field,
    })
}

/// Converts an assignment expression (`target = value`) to `Expr::Assign`.
fn convert_assign_expr(assign: &ast::AssignExpr, reg: &TypeRegistry) -> Result<Expr> {
    let target = match &assign.left {
        ast::AssignTarget::Simple(simple) => match simple {
            ast::SimpleAssignTarget::Member(member) => convert_member_expr(member, reg)?,
            ast::SimpleAssignTarget::Ident(ident) => Expr::Ident(ident.id.sym.to_string()),
            _ => return Err(anyhow!("unsupported assignment target")),
        },
        _ => return Err(anyhow!("unsupported assignment target pattern")),
    };
    let value = convert_expr(&assign.right, reg, None)?;
    Ok(Expr::Assign {
        target: Box::new(target),
        value: Box::new(value),
    })
}

/// Converts an arrow function expression to `Expr::Closure`.
///
/// - Expression body: `(x: number) => x + 1` → `|x: f64| x + 1`
/// - Block body: `(x: number) => { return x + 1; }` → `|x: f64| { x + 1 }`
fn convert_arrow_expr(arrow: &ast::ArrowExpr, reg: &TypeRegistry) -> Result<Expr> {
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
            let ir_expr = convert_expr(expr, reg, return_type.as_ref())?;
            ClosureBody::Expr(Box::new(ir_expr))
        }
        ast::BlockStmtOrExpr::BlockStmt(block) => {
            let mut stmts = Vec::new();
            for stmt in &block.stmts {
                stmts.push(convert_stmt(stmt, reg, return_type.as_ref())?);
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
fn convert_call_expr(call: &ast::CallExpr, reg: &TypeRegistry) -> Result<Expr> {
    match call.callee {
        ast::Callee::Expr(ref callee) => match callee.as_ref() {
            ast::Expr::Ident(ident) => {
                let fn_name = ident.sym.to_string();
                // Look up function parameter types from the registry
                let param_types = reg.get(&fn_name).and_then(|def| match def {
                    TypeDef::Function { params, .. } => Some(params.as_slice()),
                    _ => None,
                });
                let args = convert_call_args_with_types(&call.args, reg, param_types)?;
                Ok(Expr::FnCall {
                    name: fn_name,
                    args,
                })
            }
            ast::Expr::Member(member) => {
                let object = convert_expr(&member.obj, reg, None)?;
                let method = match &member.prop {
                    ast::MemberProp::Ident(ident) => ident.sym.to_string(),
                    _ => return Err(anyhow!("unsupported call target member property")),
                };
                let args = convert_call_args(&call.args, reg)?;
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
fn convert_new_expr(new_expr: &ast::NewExpr, reg: &TypeRegistry) -> Result<Expr> {
    let class_name = match new_expr.callee.as_ref() {
        ast::Expr::Ident(ident) => ident.sym.to_string(),
        _ => return Err(anyhow!("unsupported new expression target")),
    };
    let args = match &new_expr.args {
        Some(args) => convert_call_args(args, reg)?,
        None => vec![],
    };
    Ok(Expr::FnCall {
        name: format!("{class_name}::new"),
        args,
    })
}

/// Converts call arguments from SWC `ExprOrSpread` to IR `Expr`.
fn convert_call_args(args: &[ast::ExprOrSpread], reg: &TypeRegistry) -> Result<Vec<Expr>> {
    convert_call_args_with_types(args, reg, None)
}

/// Converts call arguments with optional parameter type information from the registry.
///
/// When `param_types` is provided, each argument gets the corresponding parameter's type
/// as its expected type. This enables object literal arguments to resolve their struct name.
fn convert_call_args_with_types(
    args: &[ast::ExprOrSpread],
    reg: &TypeRegistry,
    param_types: Option<&[(String, RustType)]>,
) -> Result<Vec<Expr>> {
    args.iter()
        .enumerate()
        .map(|(i, arg)| {
            let expected = param_types.and_then(|params| params.get(i).map(|(_, ty)| ty));
            convert_expr(&arg.expr, reg, expected)
        })
        .collect()
}

/// Converts a template literal to `Expr::FormatMacro`.
///
/// `` `Hello ${name}` `` becomes `format!("Hello {}", name)`.
fn convert_template_literal(tpl: &ast::Tpl, reg: &TypeRegistry) -> Result<Expr> {
    let mut template = String::new();
    let mut args = Vec::new();

    for (i, quasi) in tpl.quasis.iter().enumerate() {
        // raw text of the quasi (the string parts between expressions)
        template.push_str(&quasi.raw);
        if i < tpl.exprs.len() {
            template.push_str("{}");
            let arg = convert_expr(&tpl.exprs[i], reg, None)?;
            args.push(arg);
        }
    }

    Ok(Expr::FormatMacro { template, args })
}

/// Converts an SWC conditional (ternary) expression to `Expr::If`.
///
/// `condition ? consequent : alternate` → `if condition { consequent } else { alternate }`
fn convert_cond_expr(
    cond: &ast::CondExpr,
    reg: &TypeRegistry,
    expected: Option<&RustType>,
) -> Result<Expr> {
    let condition = convert_expr(&cond.test, reg, None)?;
    let then_expr = convert_expr(&cond.cons, reg, expected)?;
    let else_expr = convert_expr(&cond.alt, reg, expected)?;
    Ok(Expr::If {
        condition: Box::new(condition),
        then_expr: Box::new(then_expr),
        else_expr: Box::new(else_expr),
    })
}

/// Converts an SWC object literal to an IR `Expr::StructInit`.
///
/// Requires an expected type (`RustType::Named`) from the enclosing context (e.g., a variable
/// declaration's type annotation). Without a named type, returns an error because Rust requires
/// a named struct.
///
/// `{ x: 1, y: 2 }` with expected `RustType::Named { name: "Point" }` →
/// `Expr::StructInit { name: "Point", fields: [...] }`
fn convert_object_lit(
    obj_lit: &ast::ObjectLit,
    reg: &TypeRegistry,
    expected: Option<&RustType>,
) -> Result<Expr> {
    let struct_name = match expected {
        Some(RustType::Named { name, .. }) => name.as_str(),
        _ => {
            return Err(anyhow!(
                "object literal requires a type annotation to determine struct name"
            ))
        }
    };

    // Look up field types from the registry to propagate expected types to nested values
    let struct_fields = reg.get(struct_name).and_then(|def| match def {
        TypeDef::Struct { fields } => Some(fields.as_slice()),
        _ => None,
    });

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
                    // Resolve the expected type for this field from the registry
                    let field_expected = struct_fields
                        .and_then(|fs| fs.iter().find(|(name, _)| name == &key).map(|(_, ty)| ty));
                    let value = convert_expr(&kv.value, reg, field_expected)?;
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
///
/// When `expected` is `RustType::Vec(inner)`, the inner type is propagated to each element.
fn convert_array_lit(
    array_lit: &ast::ArrayLit,
    reg: &TypeRegistry,
    expected: Option<&RustType>,
) -> Result<Expr> {
    let element_type = match expected {
        Some(RustType::Vec(inner)) => Some(inner.as_ref()),
        _ => None,
    };
    let elements = array_lit
        .elems
        .iter()
        .filter_map(|elem| elem.as_ref())
        .map(|elem| convert_expr(&elem.expr, reg, element_type))
        .collect::<Result<Vec<_>>>()?;
    Ok(Expr::Vec { elements })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_typescript;
    use crate::registry::TypeRegistry;
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
        assert_eq!(result, Expr::Ident("foo".to_string()));
    }

    #[test]
    fn test_convert_expr_number_literal() {
        let swc_expr = parse_expr("42;");
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
        assert_eq!(result, Expr::NumberLit(42.0));
    }

    #[test]
    fn test_convert_expr_string_literal() {
        let swc_expr = parse_expr("\"hello\";");
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
        assert_eq!(result, Expr::StringLit("hello".to_string()));
    }

    #[test]
    fn test_convert_expr_bool_true() {
        let swc_expr = parse_expr("true;");
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
        assert_eq!(result, Expr::BoolLit(true));
    }

    #[test]
    fn test_convert_expr_bool_false() {
        let swc_expr = parse_expr("false;");
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
        assert_eq!(result, Expr::BoolLit(false));
    }

    #[test]
    fn test_convert_expr_binary_add() {
        let swc_expr = parse_var_init("const x = a + b;");
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
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
        let result = convert_expr(&expr, &TypeRegistry::new(), None).unwrap();
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
        let result = convert_expr(&expr, &TypeRegistry::new(), None).unwrap();
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
        let result = convert_expr(&expr, &TypeRegistry::new(), None).unwrap();
        assert_eq!(result, Expr::Vec { elements: vec![] });
    }

    #[test]
    fn test_convert_expr_array_single_element() {
        let expr = parse_var_init("const a = [42];");
        let result = convert_expr(&expr, &TypeRegistry::new(), None).unwrap();
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), Some(&expected)).unwrap();
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
        let expected = RustType::Named {
            name: "Config".to_string(),
            type_args: vec![],
        };
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), Some(&expected)).unwrap();
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
        let expected = RustType::Named {
            name: "Wrapper".to_string(),
            type_args: vec![],
        };
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), Some(&expected)).unwrap();
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
        let expected = RustType::Named {
            name: "Empty".to_string(),
            type_args: vec![],
        };
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), Some(&expected)).unwrap();
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_expr_array_nested() {
        let expr = parse_var_init("const a = [[1, 2], [3]];");
        let result = convert_expr(&expr, &TypeRegistry::new(), None).unwrap();
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
        let result =
            convert_expr(&swc_expr, &TypeRegistry::new(), Some(&RustType::String)).unwrap();
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
        assert_eq!(result, Expr::StringLit("hello".to_string()));
    }

    #[test]
    fn test_convert_expr_string_lit_with_f64_expected_unchanged() {
        let swc_expr = parse_expr("\"hello\";");
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), Some(&RustType::F64)).unwrap();
        assert_eq!(result, Expr::StringLit("hello".to_string()));
    }

    #[test]
    fn test_convert_expr_array_string_with_vec_string_expected() {
        let expr = parse_var_init(r#"const a = ["a", "b"];"#);
        let expected = RustType::Vec(Box::new(RustType::String));
        let result = convert_expr(&expr, &TypeRegistry::new(), Some(&expected)).unwrap();
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
            },
        );

        let swc_expr = parse_expr("Color.Red;");
        let result = convert_expr(&swc_expr, &reg, None).unwrap();
        assert_eq!(result, Expr::Ident("Color::Red".to_string()));
    }

    #[test]
    fn test_convert_expr_member_non_enum_unchanged() {
        // obj.field should remain FieldAccess when obj is not an enum
        let reg = TypeRegistry::new();
        let swc_expr = parse_expr("obj.field;");
        let result = convert_expr(&swc_expr, &reg, None).unwrap();
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
        let result = convert_expr(&swc_expr, &reg, None).unwrap();
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
            },
        );

        let swc_expr = parse_var_init("const r: Rect = { origin: { x: 0, y: 0 }, w: 10 };");
        let expected = RustType::Named {
            name: "Rect".to_string(),
            type_args: vec![],
        };
        let result = convert_expr(&swc_expr, &reg, Some(&expected)).unwrap();
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
                        }
                    ),
                    ("w".to_string(), Expr::NumberLit(10.0)),
                ],
            }
        );
    }

    // -- Ternary (conditional) expression tests --

    #[test]
    fn test_convert_expr_ternary_basic_identifiers() {
        let swc_expr = parse_var_init("const x = flag ? a : b;");
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
        assert_eq!(
            result,
            Expr::If {
                condition: Box::new(Expr::BinaryOp {
                    left: Box::new(Expr::Ident("a".to_string())),
                    op: ">".to_string(),
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
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
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
        let swc_expr =
            parse_var_init(r#"const s = x > 0 ? "positive" : x < 0 ? "negative" : "zero";"#);
        let result = convert_expr(&swc_expr, &TypeRegistry::new(), None).unwrap();
        assert_eq!(
            result,
            Expr::If {
                condition: Box::new(Expr::BinaryOp {
                    left: Box::new(Expr::Ident("x".to_string())),
                    op: ">".to_string(),
                    right: Box::new(Expr::NumberLit(0.0)),
                }),
                then_expr: Box::new(Expr::StringLit("positive".to_string())),
                else_expr: Box::new(Expr::If {
                    condition: Box::new(Expr::BinaryOp {
                        left: Box::new(Expr::Ident("x".to_string())),
                        op: "<".to_string(),
                        right: Box::new(Expr::NumberLit(0.0)),
                    }),
                    then_expr: Box::new(Expr::StringLit("negative".to_string())),
                    else_expr: Box::new(Expr::StringLit("zero".to_string())),
                }),
            }
        );
    }

    #[test]
    fn test_convert_expr_array_nested_vec_string_expected() {
        let expr = parse_var_init(r#"const a = [["a"]];"#);
        let expected = RustType::Vec(Box::new(RustType::Vec(Box::new(RustType::String))));
        let result = convert_expr(&expr, &TypeRegistry::new(), Some(&expected)).unwrap();
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
}
