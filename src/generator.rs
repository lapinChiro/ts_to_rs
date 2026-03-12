//! Code generator: converts IR into Rust source code strings.

use crate::ir::{Expr, Item, Method, RustType, Stmt, Visibility};

/// Generates Rust source code from a list of IR items.
pub fn generate(items: &[Item]) -> String {
    items
        .iter()
        .map(generate_item)
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Generates the Rust type syntax for a [`RustType`].
pub fn generate_type(ty: &RustType) -> String {
    match ty {
        RustType::String => "String".to_string(),
        RustType::F64 => "f64".to_string(),
        RustType::Bool => "bool".to_string(),
        RustType::Option(inner) => format!("Option<{}>", generate_type(inner)),
        RustType::Vec(inner) => format!("Vec<{}>", generate_type(inner)),
        RustType::Fn {
            params,
            return_type,
        } => {
            let params_str = params
                .iter()
                .map(generate_type)
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Box<dyn Fn({params_str}) -> {}>",
                generate_type(return_type)
            )
        }
        RustType::Named { name, type_args } => {
            if type_args.is_empty() {
                name.clone()
            } else {
                let args = type_args
                    .iter()
                    .map(generate_type)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{name}<{args}>")
            }
        }
    }
}

/// Generates a single IR item as Rust source code.
fn generate_item(item: &Item) -> String {
    match item {
        Item::Use { path, names } => {
            if names.len() == 1 {
                format!("use {}::{};", path, names[0])
            } else {
                format!("use {}::{{{}}};", path, names.join(", "))
            }
        }
        Item::Struct {
            vis,
            name,
            type_params,
            fields,
        } => {
            let vis_str = generate_vis(vis);
            let generics = generate_type_params(type_params);
            let mut out = format!("{vis_str}struct {name}{generics} {{\n");
            for field in fields {
                let field_vis = match vis {
                    Visibility::Public => "pub ",
                    Visibility::Private => "",
                };
                out.push_str(&format!(
                    "    {field_vis}{}: {},\n",
                    field.name,
                    generate_type(&field.ty)
                ));
            }
            out.push('}');
            out
        }
        Item::Enum {
            vis,
            name,
            variants,
        } => {
            let vis_str = generate_vis(vis);
            let mut out = format!("{vis_str}enum {name} {{\n");
            for variant in variants {
                out.push_str(&format!("    {variant},\n"));
            }
            out.push('}');
            out
        }
        Item::Impl {
            struct_name,
            methods,
        } => {
            let mut out = format!("impl {struct_name} {{\n");
            for (i, method) in methods.iter().enumerate() {
                if i > 0 {
                    out.push('\n');
                }
                out.push_str(&generate_method(method));
            }
            out.push('}');
            out
        }
        Item::Fn {
            vis,
            name,
            type_params,
            params,
            return_type,
            body,
        } => {
            let vis_str = generate_vis(vis);
            let generics = generate_type_params(type_params);
            let params_str = params
                .iter()
                .map(|p| format!("{}: {}", p.name, generate_type(&p.ty)))
                .collect::<Vec<_>>()
                .join(", ");
            let ret_str = match return_type {
                Some(ty) => format!(" -> {}", generate_type(ty)),
                None => String::new(),
            };
            let mut out = format!("{vis_str}fn {name}{generics}({params_str}){ret_str} {{\n");
            let body_len = body.len();
            for (i, stmt) in body.iter().enumerate() {
                let is_last = i == body_len - 1;
                out.push_str(&generate_stmt(stmt, 1, is_last));
                out.push('\n');
            }
            out.push('}');
            out
        }
    }
}

/// Generates a method inside an `impl` block.
fn generate_method(method: &Method) -> String {
    let vis_str = generate_vis(&method.vis);
    let self_param = if method.has_self { "&self" } else { "" };
    let other_params = method
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name, generate_type(&p.ty)))
        .collect::<Vec<_>>()
        .join(", ");
    let params_str = if method.has_self && !other_params.is_empty() {
        format!("{self_param}, {other_params}")
    } else if method.has_self {
        self_param.to_string()
    } else {
        other_params
    };
    let ret_str = match &method.return_type {
        Some(ty) => format!(" -> {}", generate_type(ty)),
        None => String::new(),
    };
    let name = &method.name;
    let mut out = format!("    {vis_str}fn {name}({params_str}){ret_str} {{\n");
    let body_len = method.body.len();
    for (i, stmt) in method.body.iter().enumerate() {
        let is_last = i == body_len - 1;
        out.push_str(&generate_stmt(stmt, 2, is_last));
        out.push('\n');
    }
    out.push_str("    }\n");
    out
}

/// Generates the visibility prefix string.
fn generate_vis(vis: &Visibility) -> &'static str {
    match vis {
        Visibility::Public => "pub ",
        Visibility::Private => "",
    }
}

/// Generates the generic type parameters string (e.g., `<T, U>`).
///
/// Returns an empty string if there are no type parameters.
fn generate_type_params(type_params: &[String]) -> String {
    if type_params.is_empty() {
        String::new()
    } else {
        format!("<{}>", type_params.join(", "))
    }
}

/// Generates a statement with the given indentation level.
///
/// When `is_last_in_fn` is true and the statement is `Stmt::Return(Some(expr))`,
/// it emits just the expression (idiomatic Rust tail expression).
fn generate_stmt(stmt: &Stmt, indent: usize, is_last_in_fn: bool) -> String {
    let pad = indent_str(indent);
    match stmt {
        Stmt::Let {
            mutable,
            name,
            ty,
            init,
        } => {
            let mut out = format!("{pad}let ");
            if *mutable {
                out.push_str("mut ");
            }
            out.push_str(name);
            if let Some(ty) = ty {
                out.push_str(&format!(": {}", generate_type(ty)));
            }
            if let Some(init) = init {
                out.push_str(&format!(" = {}", generate_expr(init)));
            }
            out.push(';');
            out
        }
        Stmt::If {
            condition,
            then_body,
            else_body,
        } => {
            let mut out = format!("{pad}if {} {{\n", generate_expr(condition));
            for s in then_body {
                // Statements inside if are not tail-position of a function
                out.push_str(&generate_stmt(s, indent + 1, false));
                out.push('\n');
            }
            match else_body {
                Some(stmts) => {
                    out.push_str(&format!("{pad}}} else {{\n"));
                    for s in stmts {
                        out.push_str(&generate_stmt(s, indent + 1, false));
                        out.push('\n');
                    }
                    out.push_str(&format!("{pad}}}"));
                }
                None => {
                    out.push_str(&format!("{pad}}}"));
                }
            }
            out
        }
        Stmt::Return(expr) => {
            if is_last_in_fn {
                match expr {
                    Some(e) => format!("{pad}{}", generate_expr(e)),
                    None => format!("{pad}return;"),
                }
            } else {
                match expr {
                    Some(e) => format!("{pad}return {};", generate_expr(e)),
                    None => format!("{pad}return;"),
                }
            }
        }
        Stmt::Expr(expr) => {
            format!("{pad}{};", generate_expr(expr))
        }
    }
}

/// Generates an expression as a Rust source string.
fn generate_expr(expr: &Expr) -> String {
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
    }
}

/// Generates a closure expression.
fn generate_closure(
    params: &[crate::ir::Param],
    return_type: Option<&RustType>,
    body: &crate::ir::ClosureBody,
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
        crate::ir::ClosureBody::Expr(expr) => {
            format!("|{params_str}|{ret_str} {}", generate_expr(expr))
        }
        crate::ir::ClosureBody::Block(stmts) => {
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

/// Returns the indentation string for the given level (4 spaces per level).
fn indent_str(level: usize) -> String {
    "    ".repeat(level)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        ClosureBody, Expr, Item, Method, Param, RustType, Stmt, StructField, Visibility,
    };

    // --- Item::Use tests ---

    #[test]
    fn test_generate_use_single() {
        let item = Item::Use {
            path: "crate::bar".to_string(),
            names: vec!["Foo".to_string()],
        };
        assert_eq!(generate(&[item]), "use crate::bar::Foo;");
    }

    #[test]
    fn test_generate_use_multiple() {
        let item = Item::Use {
            path: "crate::bar".to_string(),
            names: vec!["A".to_string(), "B".to_string()],
        };
        assert_eq!(generate(&[item]), "use crate::bar::{A, B};");
    }

    // --- RustType tests ---

    #[test]
    fn test_generate_type_string() {
        assert_eq!(generate_type(&RustType::String), "String");
    }

    #[test]
    fn test_generate_type_f64() {
        assert_eq!(generate_type(&RustType::F64), "f64");
    }

    #[test]
    fn test_generate_type_bool() {
        assert_eq!(generate_type(&RustType::Bool), "bool");
    }

    #[test]
    fn test_generate_type_option() {
        let ty = RustType::Option(Box::new(RustType::String));
        assert_eq!(generate_type(&ty), "Option<String>");
    }

    #[test]
    fn test_generate_type_vec() {
        let ty = RustType::Vec(Box::new(RustType::F64));
        assert_eq!(generate_type(&ty), "Vec<f64>");
    }

    #[test]
    fn test_generate_type_nested() {
        let ty = RustType::Option(Box::new(RustType::Vec(Box::new(RustType::Bool))));
        assert_eq!(generate_type(&ty), "Option<Vec<bool>>");
    }

    #[test]
    fn test_generate_type_fn_single_param() {
        let ty = RustType::Fn {
            params: vec![RustType::F64],
            return_type: Box::new(RustType::F64),
        };
        assert_eq!(generate_type(&ty), "Box<dyn Fn(f64) -> f64>");
    }

    #[test]
    fn test_generate_type_fn_multiple_params() {
        let ty = RustType::Fn {
            params: vec![RustType::String, RustType::F64],
            return_type: Box::new(RustType::Bool),
        };
        assert_eq!(generate_type(&ty), "Box<dyn Fn(String, f64) -> bool>");
    }

    #[test]
    fn test_generate_type_fn_no_params() {
        let ty = RustType::Fn {
            params: vec![],
            return_type: Box::new(RustType::F64),
        };
        assert_eq!(generate_type(&ty), "Box<dyn Fn() -> f64>");
    }

    #[test]
    fn test_generate_type_named_no_type_args() {
        let ty = RustType::Named {
            name: "Point".to_string(),
            type_args: vec![],
        };
        assert_eq!(generate_type(&ty), "Point");
    }

    #[test]
    fn test_generate_type_named_with_single_type_arg() {
        let ty = RustType::Named {
            name: "Box".to_string(),
            type_args: vec![RustType::String],
        };
        assert_eq!(generate_type(&ty), "Box<String>");
    }

    #[test]
    fn test_generate_type_named_with_multiple_type_args() {
        let ty = RustType::Named {
            name: "HashMap".to_string(),
            type_args: vec![RustType::String, RustType::F64],
        };
        assert_eq!(generate_type(&ty), "HashMap<String, f64>");
    }

    // --- Closure Expr tests ---

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

    // --- Expr tests ---

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

    // --- Item::Struct tests ---

    #[test]
    fn test_generate_struct_public() {
        let item = Item::Struct {
            vis: Visibility::Public,
            name: "Foo".to_string(),
            type_params: vec![],
            fields: vec![
                StructField {
                    name: "name".to_string(),
                    ty: RustType::String,
                },
                StructField {
                    name: "age".to_string(),
                    ty: RustType::F64,
                },
            ],
        };
        let expected = "\
pub struct Foo {
    pub name: String,
    pub age: f64,
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_struct_private() {
        let item = Item::Struct {
            vis: Visibility::Private,
            name: "Bar".to_string(),
            type_params: vec![],
            fields: vec![StructField {
                name: "x".to_string(),
                ty: RustType::Bool,
            }],
        };
        let expected = "\
struct Bar {
    x: bool,
}";
        assert_eq!(generate(&[item]), expected);
    }

    // --- Item::Enum tests ---

    #[test]
    fn test_generate_enum_public() {
        let item = Item::Enum {
            vis: Visibility::Public,
            name: "Direction".to_string(),
            variants: vec!["North".to_string(), "South".to_string()],
        };
        let expected = "\
pub enum Direction {
    North,
    South,
}";
        assert_eq!(generate(&[item]), expected);
    }

    // --- Item::Fn tests ---

    #[test]
    fn test_generate_fn_simple_return() {
        let item = Item::Fn {
            vis: Visibility::Public,
            name: "add".to_string(),
            type_params: vec![],
            params: vec![
                Param {
                    name: "a".to_string(),
                    ty: RustType::F64,
                },
                Param {
                    name: "b".to_string(),
                    ty: RustType::F64,
                },
            ],
            return_type: Some(RustType::F64),
            body: vec![Stmt::Return(Some(Expr::BinaryOp {
                left: Box::new(Expr::Ident("a".to_string())),
                op: "+".to_string(),
                right: Box::new(Expr::Ident("b".to_string())),
            }))],
        };
        let expected = "\
pub fn add(a: f64, b: f64) -> f64 {
    a + b
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_fn_no_return_type() {
        let item = Item::Fn {
            vis: Visibility::Private,
            name: "greet".to_string(),
            type_params: vec![],
            params: vec![Param {
                name: "name".to_string(),
                ty: RustType::String,
            }],
            return_type: None,
            body: vec![Stmt::Expr(Expr::Ident("println!".to_string()))],
        };
        let expected = "\
fn greet(name: String) {
    println!;
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_fn_no_params() {
        let item = Item::Fn {
            vis: Visibility::Public,
            name: "get_value".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: Some(RustType::F64),
            body: vec![Stmt::Return(Some(Expr::NumberLit(42.0)))],
        };
        let expected = "\
pub fn get_value() -> f64 {
    42.0
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_struct_with_type_params() {
        let item = Item::Struct {
            vis: Visibility::Public,
            name: "Container".to_string(),
            type_params: vec!["T".to_string()],
            fields: vec![StructField {
                name: "value".to_string(),
                ty: RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![],
                },
            }],
        };
        let expected = "\
pub struct Container<T> {
    pub value: T,
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_fn_with_type_params() {
        let item = Item::Fn {
            vis: Visibility::Public,
            name: "identity".to_string(),
            type_params: vec!["T".to_string()],
            params: vec![Param {
                name: "x".to_string(),
                ty: RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![],
                },
            }],
            return_type: Some(RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            }),
            body: vec![Stmt::Return(Some(Expr::Ident("x".to_string())))],
        };
        let expected = "\
pub fn identity<T>(x: T) -> T {
    x
}";
        assert_eq!(generate(&[item]), expected);
    }

    // --- Stmt::Let tests ---

    #[test]
    fn test_generate_let_simple() {
        let item = Item::Fn {
            vis: Visibility::Private,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::Let {
                mutable: false,
                name: "x".to_string(),
                ty: None,
                init: Some(Expr::NumberLit(42.0)),
            }],
        };
        let expected = "\
fn f() {
    let x = 42.0;
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_let_mut_with_type() {
        let item = Item::Fn {
            vis: Visibility::Private,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::Let {
                mutable: true,
                name: "count".to_string(),
                ty: Some(RustType::F64),
                init: Some(Expr::NumberLit(0.0)),
            }],
        };
        let expected = "\
fn f() {
    let mut count: f64 = 0.0;
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_let_no_init() {
        let item = Item::Fn {
            vis: Visibility::Private,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::Let {
                mutable: false,
                name: "x".to_string(),
                ty: Some(RustType::String),
                init: None,
            }],
        };
        let expected = "\
fn f() {
    let x: String;
}";
        assert_eq!(generate(&[item]), expected);
    }

    // --- Stmt::If tests ---

    #[test]
    fn test_generate_if_no_else() {
        let item = Item::Fn {
            vis: Visibility::Private,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::If {
                condition: Expr::BoolLit(true),
                then_body: vec![Stmt::Return(None)],
                else_body: None,
            }],
        };
        let expected = "\
fn f() {
    if true {
        return;
    }
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_if_with_else() {
        let item = Item::Fn {
            vis: Visibility::Private,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::If {
                condition: Expr::Ident("x".to_string()),
                then_body: vec![Stmt::Expr(Expr::Ident("a".to_string()))],
                else_body: Some(vec![Stmt::Expr(Expr::Ident("b".to_string()))]),
            }],
        };
        let expected = "\
fn f() {
    if x {
        a;
    } else {
        b;
    }
}";
        assert_eq!(generate(&[item]), expected);
    }

    // --- Stmt::Return tests ---

    #[test]
    fn test_generate_return_bare() {
        let item = Item::Fn {
            vis: Visibility::Private,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![
                Stmt::Expr(Expr::Ident("something".to_string())),
                Stmt::Return(None),
            ],
        };
        let expected = "\
fn f() {
    something;
    return;
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_return_not_last_uses_return_keyword() {
        let item = Item::Fn {
            vis: Visibility::Private,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: Some(RustType::F64),
            body: vec![
                Stmt::Return(Some(Expr::NumberLit(1.0))),
                Stmt::Return(Some(Expr::NumberLit(2.0))),
            ],
        };
        let expected = "\
fn f() -> f64 {
    return 1.0;
    2.0
}";
        assert_eq!(generate(&[item]), expected);
    }

    // --- Multiple items ---

    // --- Item::Impl tests ---

    #[test]
    fn test_generate_impl_new() {
        let item = Item::Impl {
            struct_name: "Foo".to_string(),
            methods: vec![Method {
                vis: Visibility::Public,
                name: "new".to_string(),
                has_self: false,
                params: vec![Param {
                    name: "x".to_string(),
                    ty: RustType::F64,
                }],
                return_type: Some(RustType::Named {
                    name: "Self".to_string(),
                    type_args: vec![],
                }),
                body: vec![Stmt::Return(Some(Expr::Ident("Self { x }".to_string())))],
            }],
        };
        let expected = "\
impl Foo {
    pub fn new(x: f64) -> Self {
        Self { x }
    }
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_impl_self_method() {
        let item = Item::Impl {
            struct_name: "Foo".to_string(),
            methods: vec![Method {
                vis: Visibility::Public,
                name: "get_name".to_string(),
                has_self: true,
                params: vec![],
                return_type: Some(RustType::String),
                body: vec![Stmt::Return(Some(Expr::FieldAccess {
                    object: Box::new(Expr::Ident("self".to_string())),
                    field: "name".to_string(),
                }))],
            }],
        };
        let expected = "\
impl Foo {
    pub fn get_name(&self) -> String {
        self.name
    }
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_multiple_items_separated_by_blank_line() {
        let items = vec![
            Item::Struct {
                vis: Visibility::Public,
                name: "A".to_string(),
                type_params: vec![],
                fields: vec![],
            },
            Item::Struct {
                vis: Visibility::Public,
                name: "B".to_string(),
                type_params: vec![],
                fields: vec![],
            },
        ];
        let expected = "\
pub struct A {
}

pub struct B {
}";
        assert_eq!(generate(&items), expected);
    }
}
