//! Expression conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC expression nodes into the IR [`Expr`] representation.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::Expr;

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
///
/// # Errors
///
/// Returns an error for unsupported expression types.
pub fn convert_expr(expr: &ast::Expr) -> Result<Expr> {
    match expr {
        ast::Expr::Ident(ident) => Ok(Expr::Ident(ident.sym.to_string())),
        ast::Expr::Lit(lit) => convert_lit(lit),
        ast::Expr::Bin(bin) => convert_bin_expr(bin),
        ast::Expr::Tpl(tpl) => convert_template_literal(tpl),
        ast::Expr::Paren(paren) => convert_expr(&paren.expr),
        ast::Expr::Member(member) => convert_member_expr(member),
        ast::Expr::This(_) => Ok(Expr::Ident("self".to_string())),
        ast::Expr::Assign(assign) => convert_assign_expr(assign),
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
}
