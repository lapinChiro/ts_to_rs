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

/// Returns a typed float literal string when the expression is a numeric literal
/// (or negated numeric literal) used as a method receiver.
///
/// Rust cannot call methods on ambiguous float literals (e.g., `3.7.floor()`),
/// so we emit `3.7_f64.floor()` or `(-5.0_f64).abs()`.
fn float_literal_for_method(expr: &Expr) -> Option<String> {
    match expr {
        Expr::NumberLit(n) => {
            let lit = if n.fract() == 0.0 {
                format!("{n:.1}_f64")
            } else {
                format!("{n}_f64")
            };
            Some(lit)
        }
        Expr::UnaryOp { op, operand } if op.as_str() == "-" => {
            if let Expr::NumberLit(n) = operand.as_ref() {
                let lit = if n.fract() == 0.0 {
                    format!("{n:.1}_f64")
                } else {
                    format!("{n}_f64")
                };
                Some(format!("(-{lit})"))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Returns `true` if the expression is an uppercase identifier (heuristic for type/class name).
///
/// Used to detect static method calls: `Foo.method()` → `Foo::method()`.
fn is_type_ident(expr: &Expr) -> bool {
    if let Expr::Ident(name) = expr {
        name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
    } else {
        false
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
            let method = escape_ident(method);
            // Float literals need _f64 suffix for method calls (e.g., 3.7_f64.floor())
            // Also handles negated literals: (-5.0_f64).abs()
            if let Some(lit) = float_literal_for_method(object) {
                return format!("{lit}.{method}({args_str})");
            }
            let obj_str = generate_expr(object);
            // Uppercase identifier receiver → static method call (Type::method)
            let sep = if is_type_ident(object) { "::" } else { "." };
            if needs_parens_as_receiver(object) {
                format!("({obj_str}){sep}{method}({args_str})")
            } else {
                format!("{obj_str}{sep}{method}({args_str})")
            }
        }
        Expr::StructInit { name, fields, base } => {
            let mut parts: Vec<String> = if fields
                .iter()
                .all(|(f, v)| matches!(v, Expr::Ident(i) if i == f))
            {
                // Shorthand: `x, y` when field name == value name
                fields.iter().map(|(f, _)| f.to_string()).collect()
            } else {
                fields
                    .iter()
                    .map(|(f, v)| format!("{f}: {}", generate_expr(v)))
                    .collect()
            };
            if let Some(base_expr) = base {
                parts.push(format!("..{}", generate_expr(base_expr)));
            }
            format!("{name} {{ {} }}", parts.join(", "))
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
            if op.is_bitwise() {
                let left_str = format!("{} as i64", generate_expr(left));
                let right_str = format!("{} as i64", generate_expr(right));
                format!("(({left_str}) {op_str} ({right_str})) as f64")
            } else {
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
        }
        Expr::Vec { elements } => {
            let elems_str = elements
                .iter()
                .map(generate_expr)
                .collect::<Vec<_>>()
                .join(", ");
            format!("vec![{elems_str}]")
        }
        Expr::Tuple { elements } => {
            let elems_str = elements
                .iter()
                .map(generate_expr)
                .collect::<Vec<_>>()
                .join(", ");
            format!("({elems_str})")
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
        Expr::MacroCall {
            name,
            args,
            use_debug,
        } => generate_macro_call(name, args, use_debug),
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
        Expr::Deref(inner) => format!("*{}", generate_expr(inner)),
        Expr::Ref(inner) => format!("&{}", generate_expr(inner)),
        Expr::Unit => "()".to_string(),
        Expr::IntLit(n) => format!("{n}"),
        Expr::Block(stmts) => {
            use super::statements::generate_stmt;
            let mut out = "{\n".to_string();
            for s in stmts {
                out.push_str(&generate_stmt(s, 1));
                out.push('\n');
            }
            out.push('}');
            out
        }
        Expr::Regex { pattern, .. } => {
            format!("Regex::new(\"{pattern}\").unwrap()")
        }
        Expr::Match { expr, arms } => {
            use crate::ir::MatchPattern;
            let match_target = generate_expr(expr);
            let mut out = format!("match {match_target} {{\n");
            for arm in arms {
                let patterns_str = arm
                    .patterns
                    .iter()
                    .map(|p| match p {
                        MatchPattern::Literal(e) => generate_expr(e),
                        MatchPattern::Wildcard => "_".to_string(),
                        MatchPattern::EnumVariant { path, bindings } => {
                            if bindings.is_empty() {
                                format!("{path} {{ .. }}")
                            } else {
                                let fields = bindings.join(", ");
                                format!("{path} {{ {fields}, .. }}")
                            }
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" | ");
                let guard_str = arm
                    .guard
                    .as_ref()
                    .map(|g| format!(" if {}", generate_expr(g)))
                    .unwrap_or_default();
                out.push_str(&format!("    {patterns_str}{guard_str} => {{\n"));
                for s in &arm.body {
                    use super::statements::generate_stmt;
                    out.push_str(&generate_stmt(s, 2));
                    out.push('\n');
                }
                out.push_str("    }\n");
            }
            out.push('}');
            out
        }
    }
}

/// Generates a macro call expression (e.g., `println!("{}", x)`).
///
/// For `println!`/`eprintln!`, constructs a format string based on argument types:
/// - No args → `name!()`
/// - Single string literal → `name!("the string")`
/// - Other args → `name!("{} {}", arg1, arg2)` using Display format
///
/// Display (`{}`) is used instead of Debug (`{:?}`) because `console.log` in TypeScript
/// outputs values without debug formatting (e.g., strings without quotes).
fn generate_macro_call(name: &str, args: &[Expr], use_debug: &[bool]) -> String {
    if args.is_empty() {
        return format!("{name}!()");
    }

    // Single string literal: output directly without format placeholders
    if args.len() == 1 {
        if let Expr::StringLit(s) = &args[0] {
            return format!("{name}!(\"{s}\")");
        }
    }

    // Build format string with per-argument Display/Debug placeholders
    let format_str = args
        .iter()
        .enumerate()
        .map(|(i, _)| {
            if use_debug.get(i).copied().unwrap_or(false) {
                "{:?}"
            } else {
                "{}"
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
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
        Expr::IntLit(n) => format!("{n}"),
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
            if return_type.is_some() {
                format!("|{params_str}|{ret_str} {{ {} }}", generate_expr(expr))
            } else {
                format!("|{params_str}|{ret_str} {}", generate_expr(expr))
            }
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
    fn test_generate_expr_tuple_literal() {
        let expr = Expr::Tuple {
            elements: vec![
                Expr::MethodCall {
                    object: Box::new(Expr::StringLit("a".to_string())),
                    method: "to_string".to_string(),
                    args: vec![],
                },
                Expr::NumberLit(1.0),
            ],
        };
        assert_eq!(generate_expr(&expr), r#"("a".to_string(), 1.0)"#);
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
    fn test_generate_expr_bitwise_and_casts_to_i64() {
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Ident("a".to_string())),
            op: BinOp::BitAnd,
            right: Box::new(Expr::Ident("b".to_string())),
        };
        assert_eq!(generate_expr(&expr), "((a as i64) & (b as i64)) as f64");
    }

    #[test]
    fn test_generate_expr_bitwise_or_casts_to_i64() {
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Ident("a".to_string())),
            op: BinOp::BitOr,
            right: Box::new(Expr::Ident("b".to_string())),
        };
        assert_eq!(generate_expr(&expr), "((a as i64) | (b as i64)) as f64");
    }

    #[test]
    fn test_generate_expr_bitwise_xor_casts_to_i64() {
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Ident("a".to_string())),
            op: BinOp::BitXor,
            right: Box::new(Expr::Ident("b".to_string())),
        };
        assert_eq!(generate_expr(&expr), "((a as i64) ^ (b as i64)) as f64");
    }

    #[test]
    fn test_generate_expr_bitwise_shl_casts_to_i64() {
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Ident("a".to_string())),
            op: BinOp::Shl,
            right: Box::new(Expr::Ident("b".to_string())),
        };
        assert_eq!(generate_expr(&expr), "((a as i64) << (b as i64)) as f64");
    }

    #[test]
    fn test_generate_expr_bitwise_shr_casts_to_i64() {
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Ident("a".to_string())),
            op: BinOp::Shr,
            right: Box::new(Expr::Ident("b".to_string())),
        };
        assert_eq!(generate_expr(&expr), "((a as i64) >> (b as i64)) as f64");
    }

    #[test]
    fn test_generate_expr_bitwise_nested_or_and() {
        // (a & b) | c — inner bitwise should also be cast
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("a".to_string())),
                op: BinOp::BitAnd,
                right: Box::new(Expr::Ident("b".to_string())),
            }),
            op: BinOp::BitOr,
            right: Box::new(Expr::Ident("c".to_string())),
        };
        assert_eq!(
            generate_expr(&expr),
            "((((a as i64) & (b as i64)) as f64 as i64) | (c as i64)) as f64"
        );
    }

    #[test]
    fn test_generate_expr_arithmetic_with_bitwise_no_cast_on_arithmetic() {
        // a + (b & c) — only bitwise part gets cast
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::Ident("a".to_string())),
            op: BinOp::Add,
            right: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("b".to_string())),
                op: BinOp::BitAnd,
                right: Box::new(Expr::Ident("c".to_string())),
            }),
        };
        assert_eq!(
            generate_expr(&expr),
            "a + (((b as i64) & (c as i64)) as f64)"
        );
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
    fn test_generate_closure_expr_body_with_return_type_has_braces() {
        let expr = Expr::Closure {
            params: vec![Param {
                name: "x".to_string(),
                ty: Some(RustType::F64),
            }],
            return_type: Some(RustType::F64),
            body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Mul,
                right: Box::new(Expr::NumberLit(2.0)),
            })),
        };
        assert_eq!(generate_expr(&expr), "|x: f64| -> f64 { x * 2.0 }");
    }

    #[test]
    fn test_generate_closure_expr_body_without_return_type_no_braces() {
        let expr = Expr::Closure {
            params: vec![Param {
                name: "x".to_string(),
                ty: Some(RustType::F64),
            }],
            return_type: None,
            body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Mul,
                right: Box::new(Expr::NumberLit(2.0)),
            })),
        };
        assert_eq!(generate_expr(&expr), "|x: f64| x * 2.0");
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
            use_debug: vec![],
        };
        assert_eq!(generate_expr(&expr), "println!()");
    }

    #[test]
    fn test_generate_expr_macro_call_single_string_literal() {
        let expr = Expr::MacroCall {
            name: "println".to_string(),
            args: vec![Expr::StringLit("hello".to_string())],
            use_debug: vec![false],
        };
        assert_eq!(generate_expr(&expr), "println!(\"hello\")");
    }

    #[test]
    fn test_generate_expr_macro_call_single_ident() {
        let expr = Expr::MacroCall {
            name: "println".to_string(),
            args: vec![Expr::Ident("x".to_string())],
            use_debug: vec![false],
        };
        assert_eq!(generate_expr(&expr), "println!(\"{}\", x)");
    }

    #[test]
    fn test_generate_expr_macro_call_multiple_args() {
        let expr = Expr::MacroCall {
            name: "println".to_string(),
            args: vec![
                Expr::StringLit("value:".to_string()),
                Expr::Ident("x".to_string()),
            ],
            use_debug: vec![false, false],
        };
        assert_eq!(generate_expr(&expr), "println!(\"{} {}\", \"value:\", x)");
    }

    #[test]
    fn test_generate_expr_macro_call_eprintln() {
        let expr = Expr::MacroCall {
            name: "eprintln".to_string(),
            args: vec![Expr::Ident("err".to_string())],
            use_debug: vec![false],
        };
        assert_eq!(generate_expr(&expr), "eprintln!(\"{}\", err)");
    }

    #[test]
    fn test_generate_expr_macro_call_use_debug_single() {
        let expr = Expr::MacroCall {
            name: "println".to_string(),
            args: vec![Expr::Ident("arr".to_string())],
            use_debug: vec![true],
        };
        assert_eq!(generate_expr(&expr), "println!(\"{:?}\", arr)");
    }

    #[test]
    fn test_generate_expr_macro_call_use_debug_mixed() {
        let expr = Expr::MacroCall {
            name: "println".to_string(),
            args: vec![
                Expr::StringLit("items:".to_string()),
                Expr::Ident("arr".to_string()),
            ],
            use_debug: vec![false, true],
        };
        assert_eq!(
            generate_expr(&expr),
            "println!(\"{} {:?}\", \"items:\", arr)"
        );
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

    // --- I-84: static method :: separator ---

    #[test]
    fn test_generate_static_method_call_uses_double_colon() {
        // Foo.method() → Foo::method() (uppercase receiver = type name = static call)
        let expr = Expr::MethodCall {
            object: Box::new(Expr::Ident("Foo".to_string())),
            method: "create".to_string(),
            args: vec![Expr::IntLit(1)],
        };
        assert_eq!(generate_expr(&expr), "Foo::create(1)");
    }

    #[test]
    fn test_generate_instance_method_call_uses_dot() {
        // foo.method() → foo.method() (lowercase = instance, no change)
        let expr = Expr::MethodCall {
            object: Box::new(Expr::Ident("foo".to_string())),
            method: "create".to_string(),
            args: vec![],
        };
        assert_eq!(generate_expr(&expr), "foo.create()");
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
    fn test_generate_expr_deref_renders_star() {
        let expr = Expr::Deref(Box::new(Expr::Ident("x".to_string())));
        assert_eq!(generate_expr(&expr), "*x");
    }

    #[test]
    fn test_generate_expr_ref_renders_ampersand() {
        let expr = Expr::Ref(Box::new(Expr::Ident("sep".to_string())));
        assert_eq!(generate_expr(&expr), "&sep");
    }

    #[test]
    fn test_generate_expr_ref_number_renders_ampersand_literal() {
        let expr = Expr::Ref(Box::new(Expr::NumberLit(0.0)));
        assert_eq!(generate_expr(&expr), "&0.0");
    }

    #[test]
    fn test_generate_expr_unit_renders_parens() {
        assert_eq!(generate_expr(&Expr::Unit), "()");
    }

    #[test]
    fn test_generate_expr_int_lit_positive_renders_number() {
        assert_eq!(generate_expr(&Expr::IntLit(42)), "42");
    }

    #[test]
    fn test_generate_expr_int_lit_negative_renders_negative() {
        assert_eq!(generate_expr(&Expr::IntLit(-1)), "-1");
    }

    #[test]
    fn test_generate_expr_int_lit_zero_renders_zero() {
        assert_eq!(generate_expr(&Expr::IntLit(0)), "0");
    }

    #[test]
    fn test_escape_ident_self_not_escaped() {
        let expr = Expr::FieldAccess {
            object: Box::new(Expr::Ident("self".to_string())),
            field: "x".to_string(),
        };
        assert_eq!(generate_expr(&expr), "self.x");
    }

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

    #[test]
    fn test_generate_expr_block_renders_block_expression() {
        let expr = Expr::Block(vec![
            Stmt::Let {
                mutable: true,
                name: "_v".to_string(),
                ty: None,
                init: Some(Expr::Vec {
                    elements: vec![Expr::NumberLit(1.0)],
                }),
            },
            Stmt::TailExpr(Expr::Ident("_v".to_string())),
        ]);
        let expected = "{\n    let mut _v = vec![1.0];\n    _v\n}";
        assert_eq!(generate_expr(&expr), expected);
    }

    #[test]
    fn test_generate_expr_match_with_enum_variant_bindings() {
        use crate::ir::MatchArm;
        let expr = Expr::Match {
            expr: Box::new(Expr::Ref(Box::new(Expr::Ident("s".to_string())))),
            arms: vec![
                MatchArm {
                    patterns: vec![crate::ir::MatchPattern::EnumVariant {
                        path: "Shape::Circle".to_string(),
                        bindings: vec!["radius".to_string()],
                    }],
                    guard: None,
                    body: vec![Stmt::TailExpr(Expr::MethodCall {
                        object: Box::new(Expr::Ident("radius".to_string())),
                        method: "clone".to_string(),
                        args: vec![],
                    })],
                },
                MatchArm {
                    patterns: vec![crate::ir::MatchPattern::Wildcard],
                    guard: None,
                    body: vec![Stmt::TailExpr(Expr::MacroCall {
                        name: "panic".to_string(),
                        args: vec![Expr::StringLit("unexpected variant".to_string())],
                        use_debug: vec![false],
                    })],
                },
            ],
        };
        let expected = "match &s {\n    Shape::Circle { radius, .. } => {\n        radius.clone()\n    }\n    _ => {\n        panic!(\"unexpected variant\")\n    }\n}";
        assert_eq!(generate_expr(&expr), expected);
    }
}
