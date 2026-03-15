//! Expression generation: converts IR expressions into Rust source strings.

use crate::ir::{BinOp, ClosureBody, Expr, Param, RustType};

use super::generate_param;
use super::statements::generate_stmt;
use super::types::generate_type;

/// Rust の予約語一覧（strict + reserved keywords）。
const RUST_KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for",
    "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while",
    "async", "await", "dyn", "abstract", "become", "box", "do", "final", "macro", "override",
    "priv", "typeof", "unsized", "virtual", "yield", "try",
];

/// 識別子が Rust の予約語と衝突する場合に `r#` プレフィックスを付ける。
///
/// `self` / `Self` は Rust で有効な識別子として使われるためエスケープしない。
pub(crate) fn escape_ident(name: &str) -> String {
    if name == "self" || name == "Self" {
        return name.to_string();
    }
    if RUST_KEYWORDS.contains(&name) {
        format!("r#{name}")
    } else {
        name.to_string()
    }
}

/// Returns `true` if the expression needs parentheses when used as the receiver
/// of a method call or field access (i.e., before `.method()` or `.field`).
fn needs_parens_as_receiver(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::BinaryOp { .. }
            | Expr::UnaryOp { .. }
            | Expr::Cast { .. }
            | Expr::Assign { .. }
            | Expr::If { .. }
    )
}

/// Returns `true` if a child expression needs parentheses when used as an operand
/// of a binary expression with the given parent operator.
///
/// Parentheses are needed when the child is also a `BinaryOp` with lower precedence
/// than the parent operator.
fn needs_parens_in_binop(child: &Expr, parent_op: BinOp) -> bool {
    if let Expr::BinaryOp { op: child_op, .. } = child {
        child_op.precedence() < parent_op.precedence()
    } else {
        false
    }
}

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
        Expr::Ident(name) => escape_ident(name),
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
            let obj_str = generate_expr(object);
            let method = escape_ident(method);
            if needs_parens_as_receiver(object) {
                format!("({obj_str}).{method}({args_str})")
            } else {
                format!("{obj_str}.{method}({args_str})")
            }
        }
        Expr::StructInit { name, fields, .. } => {
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
        Expr::Range { start, end } => match (start, end) {
            (Some(s), Some(e)) => {
                format!("{}..{}", generate_range_bound(s), generate_range_bound(e))
            }
            (Some(s), None) => format!("{}..", generate_range_bound(s)),
            (None, Some(e)) => format!("..{}", generate_range_bound(e)),
            (None, None) => "..".to_string(),
        },
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
            let obj_str = generate_expr(object);
            let field = escape_ident(field);
            if needs_parens_as_receiver(object) {
                format!("({obj_str}).{field}")
            } else {
                format!("{obj_str}.{field}")
            }
        }
        Expr::UnaryOp { op, operand } => {
            let op_str = op.as_str();
            let needs_parens = matches!(
                operand.as_ref(),
                Expr::BinaryOp { .. } | Expr::Assign { .. } | Expr::UnaryOp { .. }
            );
            if needs_parens {
                format!("{op_str}({})", generate_expr(operand))
            } else {
                format!("{op_str}{}", generate_expr(operand))
            }
        }
        Expr::BinaryOp { left, op, right } => {
            let op_str = op.as_str();
            let left_str = if needs_parens_in_binop(left, *op) {
                format!("({})", generate_expr(left))
            } else {
                generate_expr(left)
            };
            let right_str = if needs_parens_in_binop(right, *op) {
                format!("({})", generate_expr(right))
            } else {
                generate_expr(right)
            };
            format!("{left_str} {op_str} {right_str}")
        }
        Expr::Vec { elements } => {
            let elems_str = elements
                .iter()
                .map(generate_expr)
                .collect::<Vec<_>>()
                .join(", ");
            format!("vec![{elems_str}]")
        }
        Expr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            format!(
                "if {} {{ {} }} else {{ {} }}",
                generate_expr(condition),
                generate_expr(then_expr),
                generate_expr(else_expr)
            )
        }
        Expr::MacroCall { name, args } => generate_macro_call(name, args),
        Expr::Await(expr) => format!("{}.await", generate_expr(expr)),
        Expr::Index { object, index } => {
            // Index values must be usize in Rust; emit integer literals without .0,
            // and cast variable expressions with `as usize`.
            // Range expressions (for slicing) are passed through unchanged.
            let index_str = match index.as_ref() {
                Expr::NumberLit(n) if n.fract() == 0.0 => format!("{}", *n as usize),
                Expr::NumberLit(n) => format!("{n} as usize"),
                Expr::Range { .. } => generate_expr(index),
                _ => format!("{} as usize", generate_expr(index)),
            };
            format!("{}[{index_str}]", generate_expr(object))
        }
        Expr::Cast { expr, target } => {
            format!("{} as {}", generate_expr(expr), generate_type(target))
        }
    }
}

/// Generates a macro call expression (e.g., `println!("{:?}", x)`).
///
/// For `println!`/`eprintln!`, constructs a format string based on argument types:
/// - No args → `name!()`
/// - Single string literal → `name!("the string")`
/// - Other args → `name!("{:?} {:?}", arg1, arg2)` (string literals use `{}`)
fn generate_macro_call(name: &str, args: &[Expr]) -> String {
    if args.is_empty() {
        return format!("{name}!()");
    }

    // Single string literal: output directly without format placeholders
    if args.len() == 1 {
        if let Expr::StringLit(s) = &args[0] {
            return format!("{name}!(\"{s}\")");
        }
    }

    // Build format string with placeholders
    let placeholders: Vec<&str> = args
        .iter()
        .map(|arg| match arg {
            Expr::StringLit(_) => "{}",
            _ => "{:?}",
        })
        .collect();
    let format_str = placeholders.join(" ");
    let args_str = args
        .iter()
        .map(generate_expr)
        .collect::<Vec<_>>()
        .join(", ");
    format!("{name}!(\"{format_str}\", {args_str})")
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
        .map(generate_param)
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
            for stmt in stmts {
                out.push_str(&generate_stmt(stmt, 1));
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
    use crate::ir::{BinOp, ClosureBody, Expr, Param, RustType, Stmt, UnOp};

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
            op: BinOp::Add,
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
                ty: Some(RustType::F64),
            }],
            return_type: None,
            body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Add,
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
                ty: Some(RustType::F64),
            }],
            return_type: Some(RustType::F64),
            body: ClosureBody::Block(vec![Stmt::TailExpr(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Add,
                right: Box::new(Expr::NumberLit(1.0)),
            })]),
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
    fn test_generate_closure_param_no_type_annotation() {
        let expr = Expr::Closure {
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
        };
        assert_eq!(generate_expr(&expr), "|x| x + 1.0");
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

    // -- If expression tests --

    #[test]
    fn test_generate_expr_if_basic() {
        let expr = Expr::If {
            condition: Box::new(Expr::Ident("flag".to_string())),
            then_expr: Box::new(Expr::Ident("x".to_string())),
            else_expr: Box::new(Expr::Ident("y".to_string())),
        };
        assert_eq!(generate_expr(&expr), "if flag { x } else { y }");
    }

    #[test]
    fn test_generate_expr_if_with_literals() {
        let expr = Expr::If {
            condition: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("a".to_string())),
                op: BinOp::Gt,
                right: Box::new(Expr::NumberLit(0.0)),
            }),
            then_expr: Box::new(Expr::NumberLit(1.0)),
            else_expr: Box::new(Expr::NumberLit(2.0)),
        };
        assert_eq!(generate_expr(&expr), "if a > 0.0 { 1.0 } else { 2.0 }");
    }

    #[test]
    fn test_generate_expr_if_nested() {
        let expr = Expr::If {
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
        };
        assert_eq!(
            generate_expr(&expr),
            "if x > 0.0 { \"positive\" } else { if x < 0.0 { \"negative\" } else { \"zero\" } }"
        );
    }

    // -- MacroCall tests --

    #[test]
    fn test_generate_expr_macro_call_no_args() {
        let expr = Expr::MacroCall {
            name: "println".to_string(),
            args: vec![],
        };
        assert_eq!(generate_expr(&expr), "println!()");
    }

    #[test]
    fn test_generate_expr_macro_call_single_string_literal() {
        let expr = Expr::MacroCall {
            name: "println".to_string(),
            args: vec![Expr::StringLit("hello".to_string())],
        };
        assert_eq!(generate_expr(&expr), "println!(\"hello\")");
    }

    #[test]
    fn test_generate_expr_macro_call_single_ident() {
        let expr = Expr::MacroCall {
            name: "println".to_string(),
            args: vec![Expr::Ident("x".to_string())],
        };
        assert_eq!(generate_expr(&expr), "println!(\"{:?}\", x)");
    }

    #[test]
    fn test_generate_expr_macro_call_multiple_args() {
        let expr = Expr::MacroCall {
            name: "println".to_string(),
            args: vec![
                Expr::StringLit("value:".to_string()),
                Expr::Ident("x".to_string()),
            ],
        };
        assert_eq!(generate_expr(&expr), "println!(\"{} {:?}\", \"value:\", x)");
    }

    #[test]
    fn test_generate_expr_macro_call_eprintln() {
        let expr = Expr::MacroCall {
            name: "eprintln".to_string(),
            args: vec![Expr::Ident("err".to_string())],
        };
        assert_eq!(generate_expr(&expr), "eprintln!(\"{:?}\", err)");
    }

    #[test]
    fn test_generate_method_call_binary_op_receiver_needs_parens() {
        // (a + b).sqrt() — BinaryOp needs parens
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
        // (-x).abs() — UnaryOp needs parens
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
        // (x as f64).abs() — Cast needs parens
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
        // x.foo().bar() — MethodCall chain, no parens needed
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
        // foo().bar() — FnCall, no parens needed
        let expr = Expr::MethodCall {
            object: Box::new(Expr::FnCall {
                name: "foo".to_string(),
                args: vec![],
            }),
            method: "bar".to_string(),
            args: vec![],
        };
        assert_eq!(generate_expr(&expr), "foo().bar()");
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

    // --- Rust reserved word escape tests ---

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
}
