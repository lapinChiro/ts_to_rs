//! Expression generation: converts IR expressions into Rust source strings.

use crate::ir::{ClosureBody, Expr, Param, RustType};

use super::statements::generate_stmt;
use super::types::generate_type;

/// Generates an expression as a Rust source string.
pub(super) fn generate_expr(expr: &Expr) -> String {
    match expr {
        Expr::NumberLit(n) => {
            // Ensure whole numbers keep the .0 suffix
            if n.fract() == 0.0 {
                format!("{n:.1}")
            } else {
                format!("{n}")
            }
        }
        Expr::BoolLit(b) => format!("{b}"),
        Expr::StringLit(s) => format!("\"{s}\""),
        Expr::Ident(name) => name.clone(),
        Expr::FormatMacro { template, args } => {
            if args.is_empty() {
                format!("format!(\"{template}\")")
            } else {
                let args_str = args
                    .iter()
                    .map(generate_expr)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("format!(\"{template}\", {args_str})")
            }
        }
        Expr::MethodCall {
            object,
            method,
            args,
        } => {
            let args_str = args
                .iter()
                .map(generate_expr)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}.{method}({args_str})", generate_expr(object))
        }
        Expr::StructInit { name, fields } => {
            if fields
                .iter()
                .all(|(f, v)| matches!(v, Expr::Ident(i) if i == f))
            {
                // Shorthand: `Self { x, y }` when field name == value name
                let fields_str = fields
                    .iter()
                    .map(|(f, _)| f.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{name} {{ {fields_str} }}")
            } else {
                let fields_str = fields
                    .iter()
                    .map(|(f, v)| format!("{f}: {}", generate_expr(v)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{name} {{ {fields_str} }}")
            }
        }
        Expr::Range { start, end } => {
            format!(
                "{}..{}",
                generate_range_bound(start),
                generate_range_bound(end)
            )
        }
        Expr::FnCall { name, args } => {
            let args_str = args
                .iter()
                .map(generate_expr)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{name}({args_str})")
        }
        Expr::Closure {
            params,
            return_type,
            body,
        } => generate_closure(params, return_type.as_ref(), body),
        Expr::Assign { target, value } => {
            format!("{} = {}", generate_expr(target), generate_expr(value))
        }
        Expr::FieldAccess { object, field } => {
            format!("{}.{field}", generate_expr(object))
        }
        Expr::BinaryOp { left, op, right } => {
            format!("{} {op} {}", generate_expr(left), generate_expr(right))
        }
        Expr::Vec { elements } => {
            let elems_str = elements
                .iter()
                .map(generate_expr)
                .collect::<Vec<_>>()
                .join(", ");
            format!("vec![{elems_str}]")
        }
    }
}

/// Generates a range bound expression, outputting integer literals without `.0` suffix.
///
/// Rust's `Range<f64>` does not implement `Iterator`, so numeric literals in ranges
/// must be emitted as integers (e.g., `0..n` instead of `0.0..n`).
fn generate_range_bound(expr: &Expr) -> String {
    match expr {
        Expr::NumberLit(n) if n.fract() == 0.0 => format!("{}", *n as i64),
        _ => format!("{} as i64", generate_expr(expr)),
    }
}

/// Generates a closure expression.
fn generate_closure(
    params: &[Param],
    return_type: Option<&RustType>,
    body: &ClosureBody,
) -> String {
    let params_str = params
        .iter()
        .map(|p| format!("{}: {}", p.name, generate_type(&p.ty)))
        .collect::<Vec<_>>()
        .join(", ");
    let ret_str = match return_type {
        Some(ty) => format!(" -> {}", generate_type(ty)),
        None => String::new(),
    };
    match body {
        ClosureBody::Expr(expr) => {
            format!("|{params_str}|{ret_str} {}", generate_expr(expr))
        }
        ClosureBody::Block(stmts) => {
            let mut out = format!("|{params_str}|{ret_str} {{\n");
            let body_len = stmts.len();
            for (i, stmt) in stmts.iter().enumerate() {
                let is_last = i == body_len - 1;
                out.push_str(&generate_stmt(stmt, 1, is_last));
                out.push('\n');
            }
            out.push('}');
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{ClosureBody, Expr, Param, RustType, Stmt};

    #[test]
    fn test_generate_expr_number_whole() {
        assert_eq!(generate_expr(&Expr::NumberLit(42.0)), "42.0");
    }

    #[test]
    fn test_generate_expr_number_fractional() {
        assert_eq!(generate_expr(&Expr::NumberLit(2.71)), "2.71");
    }

    #[test]
    fn test_generate_expr_bool_true() {
        assert_eq!(generate_expr(&Expr::BoolLit(true)), "true");
    }

    #[test]
    fn test_generate_expr_bool_false() {
        assert_eq!(generate_expr(&Expr::BoolLit(false)), "false");
    }

    #[test]
    fn test_generate_expr_string_lit() {
        assert_eq!(
            generate_expr(&Expr::StringLit("hello".to_string())),
            "\"hello\""
        );
    }

    #[test]
    fn test_generate_expr_ident() {
        assert_eq!(generate_expr(&Expr::Ident("foo".to_string())), "foo");
    }

    #[test]
    fn test_generate_expr_binary_op() {
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Ident("a".to_string())),
            op: "+".to_string(),
            right: Box::new(Expr::Ident("b".to_string())),
        };
        assert_eq!(generate_expr(&expr), "a + b");
    }

    #[test]
    fn test_generate_expr_field_access() {
        let expr = Expr::FieldAccess {
            object: Box::new(Expr::Ident("self".to_string())),
            field: "name".to_string(),
        };
        assert_eq!(generate_expr(&expr), "self.name");
    }

    #[test]
    fn test_generate_expr_format_macro_no_args() {
        let expr = Expr::FormatMacro {
            template: "hello".to_string(),
            args: vec![],
        };
        assert_eq!(generate_expr(&expr), "format!(\"hello\")");
    }

    #[test]
    fn test_generate_expr_format_macro_with_args() {
        let expr = Expr::FormatMacro {
            template: "Hello, {}!".to_string(),
            args: vec![Expr::Ident("name".to_string())],
        };
        assert_eq!(generate_expr(&expr), "format!(\"Hello, {}!\", name)");
    }

    #[test]
    fn test_generate_expr_fn_call_err() {
        let expr = Expr::FnCall {
            name: "Err".to_string(),
            args: vec![Expr::StringLit("error".to_string())],
        };
        assert_eq!(generate_expr(&expr), "Err(\"error\")");
    }

    #[test]
    fn test_generate_expr_fn_call_ok() {
        let expr = Expr::FnCall {
            name: "Ok".to_string(),
            args: vec![Expr::NumberLit(42.0)],
        };
        assert_eq!(generate_expr(&expr), "Ok(42.0)");
    }

    #[test]
    fn test_generate_closure_expr_body() {
        let expr = Expr::Closure {
            params: vec![Param {
                name: "x".to_string(),
                ty: RustType::F64,
            }],
            return_type: None,
            body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: "+".to_string(),
                right: Box::new(Expr::NumberLit(1.0)),
            })),
        };
        assert_eq!(generate_expr(&expr), "|x: f64| x + 1.0");
    }

    #[test]
    fn test_generate_closure_block_body() {
        let expr = Expr::Closure {
            params: vec![Param {
                name: "x".to_string(),
                ty: RustType::F64,
            }],
            return_type: Some(RustType::F64),
            body: ClosureBody::Block(vec![Stmt::Return(Some(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: "+".to_string(),
                right: Box::new(Expr::NumberLit(1.0)),
            }))]),
        };
        let expected = "|x: f64| -> f64 {\n    x + 1.0\n}";
        assert_eq!(generate_expr(&expr), expected);
    }

    #[test]
    fn test_generate_closure_no_params() {
        let expr = Expr::Closure {
            params: vec![],
            return_type: None,
            body: ClosureBody::Expr(Box::new(Expr::NumberLit(42.0))),
        };
        assert_eq!(generate_expr(&expr), "|| 42.0");
    }

    #[test]
    fn test_generate_expr_vec_numbers() {
        let expr = Expr::Vec {
            elements: vec![
                Expr::NumberLit(1.0),
                Expr::NumberLit(2.0),
                Expr::NumberLit(3.0),
            ],
        };
        assert_eq!(generate_expr(&expr), "vec![1.0, 2.0, 3.0]");
    }

    #[test]
    fn test_generate_expr_vec_empty() {
        let expr = Expr::Vec { elements: vec![] };
        assert_eq!(generate_expr(&expr), "vec![]");
    }

    #[test]
    fn test_generate_expr_vec_single() {
        let expr = Expr::Vec {
            elements: vec![Expr::StringLit("hello".to_string())],
        };
        assert_eq!(generate_expr(&expr), "vec![\"hello\"]");
    }

    #[test]
    fn test_generate_expr_vec_nested() {
        let expr = Expr::Vec {
            elements: vec![
                Expr::Vec {
                    elements: vec![Expr::NumberLit(1.0)],
                },
                Expr::Vec {
                    elements: vec![Expr::NumberLit(2.0)],
                },
            ],
        };
        assert_eq!(generate_expr(&expr), "vec![vec![1.0], vec![2.0]]");
    }
}
