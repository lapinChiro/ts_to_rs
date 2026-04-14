//! Expression generation: converts IR expressions into Rust source strings.

use crate::ir::{BinOp, CallTarget, ClosureBody, Expr, Param, RustType};

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

/// Returns `true` if the expression needs parentheses when used as the subject
/// of a postfix operator.
///
/// Rust's postfix operators — `.method()`, `.field`, `[index]`, `.await`, `?` —
/// all bind tighter than every prefix / infix operator. When the subject is an
/// expression that parses looser (prefix `*`/`&`, binary ops, casts, assignment,
/// `if`/`if let` blocks), explicit parens are required to bind the postfix op
/// to the whole subject rather than to just a sub-expression.
///
/// Examples:
/// - `*x.field` parses as `*(x.field)`; `(*x).field` is needed to deref first.
/// - `&x[0]` parses as `&(x[0])`; `(&x)[0]` is needed to borrow first.
/// - `*x.await` parses as `*(x.await)`; `(*x).await` is needed.
/// - `-5.0.abs()` parses as `-(5.0.abs())`; `(-5.0).abs()` is needed.
fn needs_parens_before_postfix(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::BinaryOp { .. }
            | Expr::UnaryOp { .. }
            | Expr::Cast { .. }
            | Expr::Assign { .. }
            | Expr::If { .. }
            | Expr::IfLet { .. }
            | Expr::Deref(..)
            | Expr::Ref(..)
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

/// Escapes a string value for use in a Rust string literal.
///
/// SWC's `Str.value` contains the decoded value (e.g., a literal newline character),
/// so this function re-encodes it into Rust source escape sequences.
fn escape_rust_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            c if c.is_control() => {
                // Other control characters as \xNN
                for byte in c.to_string().bytes() {
                    out.push_str(&format!("\\x{byte:02x}"));
                }
            }
            c => out.push(c),
        }
    }
    out
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
        Expr::StringLit(s) => format!("\"{}\"", escape_rust_string(s)),
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
            // I-378: static method calls (`Type.method()` → `Type::method()`) are
            // structurally classified by the Transformer as `Expr::FnCall { target:
            // CallTarget::UserAssocFn { .. } }`. The generator no longer needs to
            // detect uppercase receivers; all `Expr::MethodCall` here are guaranteed
            // to be value-receiver method calls.
            if needs_parens_before_postfix(object) {
                format!("({obj_str}).{method}({args_str})")
            } else {
                format!("{obj_str}.{method}({args_str})")
            }
        }
        Expr::StructInit { name, fields, base } => {
            // Unit struct: empty fields + no base → `Name` (unit syntax)
            if fields.is_empty() && base.is_none() {
                return name.clone();
            }
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
        Expr::FnCall { target, args } => {
            let args_str = args
                .iter()
                .map(generate_expr)
                .collect::<Vec<_>>()
                .join(", ");
            let callee = match target {
                CallTarget::Free(name) => name.clone(),
                CallTarget::BuiltinVariant(v) => v.as_rust_str().to_string(),
                CallTarget::ExternalPath(segments) => segments.join("::"),
                CallTarget::UserAssocFn { ty, method } => {
                    format!("{}::{}", ty.as_str(), method)
                }
                CallTarget::UserTupleCtor(ty) => ty.as_str().to_string(),
                CallTarget::UserEnumVariantCtor { enum_ty, variant } => {
                    format!("{}::{}", enum_ty.as_str(), variant)
                }
                CallTarget::Super => "super".to_string(),
            };
            format!("{callee}({args_str})")
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
            if needs_parens_before_postfix(object) {
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
                if *op == BinOp::UShr {
                    // UShr (>>>): convert via i32 → u32 to match JS ToUint32 semantics
                    let left_str = format!("({} as i32 as u32)", generate_expr(left));
                    let right_str = format!("({} as u32)", generate_expr(right));
                    return format!("({left_str} {op_str} {right_str}) as f64");
                }
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
        Expr::IfLet {
            pattern,
            expr,
            then_expr,
            else_expr,
        } => {
            let pat_str = crate::generator::patterns::render_pattern(pattern);
            format!(
                "if let {pat_str} = {} {{ {} }} else {{ {} }}",
                generate_expr(expr),
                generate_expr(then_expr),
                generate_expr(else_expr)
            )
        }
        Expr::MacroCall {
            name,
            args,
            use_debug,
        } => generate_macro_call(name, args, use_debug),
        Expr::Await(inner) => {
            let inner_str = generate_expr(inner);
            if needs_parens_before_postfix(inner) {
                format!("({inner_str}).await")
            } else {
                format!("{inner_str}.await")
            }
        }
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
            let obj_str = generate_expr(object);
            if needs_parens_before_postfix(object) {
                format!("({obj_str})[{index_str}]")
            } else {
                format!("{obj_str}[{index_str}]")
            }
        }
        Expr::Cast { expr, target } => {
            format!("{} as {}", generate_expr(expr), generate_type(target))
        }
        Expr::Deref(inner) => format!("*{}", generate_expr(inner)),
        Expr::Ref(inner) => format!("&{}", generate_expr(inner)),
        Expr::Matches { expr, pattern } => {
            let pat_str = crate::generator::patterns::render_pattern(pattern);
            format!("matches!({}, {pat_str})", generate_expr(expr))
        }
        Expr::Unit => "()".to_string(),
        Expr::IntLit(n) => format!("{n}"),
        Expr::RuntimeTypeof { operand } => format!("js_typeof(&{})", generate_expr(operand)),
        Expr::RawCode(code) => code.clone(),
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
            if pattern.contains('"') {
                format!("Regex::new(r#\"{pattern}\"#).unwrap()")
            } else {
                format!("Regex::new(r\"{pattern}\").unwrap()")
            }
        }
        Expr::EnumVariant { enum_ty, variant } => {
            format!("{}::{}", enum_ty.as_str(), variant)
        }
        Expr::PrimitiveAssocConst { ty, name } => {
            format!("{}::{}", ty.as_rust_str(), name)
        }
        Expr::StdConst(c) => c.rust_path().to_string(),
        Expr::BuiltinVariantValue(v) => v.as_rust_str().to_string(),
        Expr::Match { expr, arms } => generate_match_expr(expr, arms, 0),
    }
}

/// Generates a match expression with correct indentation at the given depth.
///
/// `base_indent` is the depth of the `match` keyword itself. Arms are indented
/// at `base_indent + 1`, arm body statements at `base_indent + 2`, and the
/// closing `}` at `base_indent`.
pub(super) fn generate_match_expr(
    expr: &Expr,
    arms: &[crate::ir::MatchArm],
    base_indent: usize,
) -> String {
    let arm_pad = "    ".repeat(base_indent + 1);
    let close_pad = "    ".repeat(base_indent);
    let match_target = generate_expr(expr);
    let mut out = format!("match {match_target} {{\n");
    for arm in arms {
        let patterns_str = arm
            .patterns
            .iter()
            .map(crate::generator::patterns::render_pattern)
            .collect::<Vec<_>>()
            .join(" | ");
        let guard_str = arm
            .guard
            .as_ref()
            .map(|g| format!(" if {}", generate_expr(g)))
            .unwrap_or_default();
        out.push_str(&format!("{arm_pad}{patterns_str}{guard_str} => {{\n"));
        for s in &arm.body {
            out.push_str(&generate_stmt(s, base_indent + 2));
            out.push('\n');
        }
        out.push_str(&format!("{arm_pad}}}\n"));
    }
    out.push_str(&close_pad);
    out.push('}');
    out
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
            return format!("{name}!(\"{}\")", escape_rust_string(s));
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
    let has_return_type = matches!(return_type, Some(ty) if *ty != RustType::Unit);
    let ret_str = if has_return_type {
        format!(" -> {}", generate_type(return_type.unwrap()))
    } else {
        String::new()
    };
    match body {
        ClosureBody::Expr(expr) => {
            if has_return_type {
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
mod tests;
