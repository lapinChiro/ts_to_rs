//! Code generator: converts IR into Rust source code strings.

pub mod types;

mod expressions;
mod statements;

use crate::ir::{EnumValue, EnumVariant, Item, Method, Param, Visibility};

use statements::generate_stmt;
use types::generate_type;

/// Generates a parameter as `name: Type` or just `name` if the type is `None`.
pub(super) fn generate_param(p: &Param) -> String {
    match &p.ty {
        Some(ty) => format!("{}: {}", p.name, generate_type(ty)),
        None => p.name.clone(),
    }
}

/// Generates Rust source code from a list of IR items.
pub fn generate(items: &[Item]) -> String {
    items
        .iter()
        .map(generate_item)
        .collect::<Vec<_>>()
        .join("\n\n")
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
        } => generate_enum(vis, name, variants),
        Item::Trait { vis, name, methods } => {
            let vis_str = generate_vis(vis);
            let mut out = format!("{vis_str}trait {name} {{\n");
            for method in methods {
                out.push_str(&generate_trait_method_sig(method));
            }
            out.push('}');
            out
        }
        Item::Impl {
            struct_name,
            for_trait,
            methods,
        } => {
            let header = match for_trait {
                Some(trait_name) => format!("impl {trait_name} for {struct_name}"),
                None => format!("impl {struct_name}"),
            };
            let mut out = format!("{header} {{\n");
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
            is_async,
            name,
            type_params,
            params,
            return_type,
            body,
        } => {
            let vis_str = generate_vis(vis);
            let async_str = if *is_async { "async " } else { "" };
            let generics = generate_type_params(type_params);
            let params_str = params
                .iter()
                .map(generate_param)
                .collect::<Vec<_>>()
                .join(", ");
            let ret_str = match return_type {
                Some(ty) => format!(" -> {}", generate_type(ty)),
                None => String::new(),
            };
            let mut out =
                format!("{vis_str}{async_str}fn {name}{generics}({params_str}){ret_str} {{\n");
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

/// Returns the `self` parameter string for a method.
fn self_param_str(method: &Method) -> &'static str {
    if method.has_mut_self {
        "&mut self"
    } else if method.has_self {
        "&self"
    } else {
        ""
    }
}

/// Generates a trait method signature (no body).
fn generate_trait_method_sig(method: &Method) -> String {
    let self_param = self_param_str(method);
    let other_params = method
        .params
        .iter()
        .map(generate_param)
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
    format!("    fn {}({params_str}){ret_str};\n", method.name)
}

/// Generates a method inside an `impl` block.
fn generate_method(method: &Method) -> String {
    let vis_str = generate_vis(&method.vis);
    let self_param = self_param_str(method);
    let other_params = method
        .params
        .iter()
        .map(generate_param)
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

/// Determines whether all enum variants have numeric values (or no values).
fn is_numeric_enum(variants: &[EnumVariant]) -> bool {
    variants
        .iter()
        .all(|v| matches!(v.value, None | Some(EnumValue::Number(_))))
}

/// Generates a Rust enum definition from IR.
///
/// - Numeric enums get `#[repr(i64)]` and discriminant values.
/// - String enums get an `as_str()` impl block.
/// - Enums without values are treated as numeric enums with auto-incrementing values.
fn generate_enum(vis: &Visibility, name: &str, variants: &[EnumVariant]) -> String {
    let vis_str = generate_vis(vis);
    let numeric = is_numeric_enum(variants);

    let mut out = String::new();
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n");
    if numeric {
        out.push_str("#[repr(i64)]\n");
    }
    out.push_str(&format!("{vis_str}enum {name} {{\n"));

    if numeric {
        let mut next_value: i64 = 0;
        for variant in variants {
            let value = match &variant.value {
                Some(EnumValue::Number(n)) => {
                    next_value = *n + 1;
                    *n
                }
                None => {
                    let v = next_value;
                    next_value += 1;
                    v
                }
                _ => unreachable!(),
            };
            out.push_str(&format!("    {} = {},\n", variant.name, value));
        }
    } else {
        for variant in variants {
            out.push_str(&format!("    {},\n", variant.name));
        }
    }

    out.push('}');

    // Generate as_str() impl for string enums
    if !numeric {
        out.push_str(&format!("\n\nimpl {name} {{\n"));
        out.push_str("    pub fn as_str(&self) -> &str {\n");
        out.push_str("        match self {\n");
        for variant in variants {
            if let Some(EnumValue::Str(s)) = &variant.value {
                out.push_str(&format!(
                    "            {name}::{} => \"{s}\",\n",
                    variant.name
                ));
            }
        }
        out.push_str("        }\n");
        out.push_str("    }\n");
        out.push('}');
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        EnumValue, EnumVariant, Expr, Item, Method, Param, RustType, Stmt, StructField, Visibility,
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

    // --- Item::Enum tests ---

    #[test]
    fn test_generate_enum_numeric_auto() {
        let item = Item::Enum {
            vis: Visibility::Public,
            name: "Color".to_string(),
            variants: vec![
                EnumVariant {
                    name: "Red".to_string(),
                    value: None,
                },
                EnumVariant {
                    name: "Green".to_string(),
                    value: None,
                },
                EnumVariant {
                    name: "Blue".to_string(),
                    value: None,
                },
            ],
        };
        let expected = "\
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum Color {
    Red = 0,
    Green = 1,
    Blue = 2,
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_enum_numeric_explicit() {
        let item = Item::Enum {
            vis: Visibility::Public,
            name: "Status".to_string(),
            variants: vec![
                EnumVariant {
                    name: "Active".to_string(),
                    value: Some(EnumValue::Number(1)),
                },
                EnumVariant {
                    name: "Inactive".to_string(),
                    value: Some(EnumValue::Number(0)),
                },
            ],
        };
        let expected = "\
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum Status {
    Active = 1,
    Inactive = 0,
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_enum_string() {
        let item = Item::Enum {
            vis: Visibility::Public,
            name: "Direction".to_string(),
            variants: vec![
                EnumVariant {
                    name: "Up".to_string(),
                    value: Some(EnumValue::Str("UP".to_string())),
                },
                EnumVariant {
                    name: "Down".to_string(),
                    value: Some(EnumValue::Str("DOWN".to_string())),
                },
            ],
        };
        let expected = "\
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
}

impl Direction {
    pub fn as_str(&self) -> &str {
        match self {
            Direction::Up => \"UP\",
            Direction::Down => \"DOWN\",
        }
    }
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_enum_private() {
        let item = Item::Enum {
            vis: Visibility::Private,
            name: "Color".to_string(),
            variants: vec![EnumVariant {
                name: "Red".to_string(),
                value: None,
            }],
        };
        let result = generate(&[item]);
        assert!(!result.contains("pub enum"));
        assert!(result.contains("enum Color"));
    }

    // --- Item::Fn tests ---

    #[test]
    fn test_generate_fn_simple_return() {
        let item = Item::Fn {
            vis: Visibility::Public,
            is_async: false,
            name: "add".to_string(),
            type_params: vec![],
            params: vec![
                Param {
                    name: "a".to_string(),
                    ty: Some(RustType::F64),
                },
                Param {
                    name: "b".to_string(),
                    ty: Some(RustType::F64),
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
            is_async: false,
            name: "greet".to_string(),
            type_params: vec![],
            params: vec![Param {
                name: "name".to_string(),
                ty: Some(RustType::String),
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
            is_async: false,
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
    fn test_generate_fn_with_type_params() {
        let item = Item::Fn {
            vis: Visibility::Public,
            is_async: false,
            name: "identity".to_string(),
            type_params: vec!["T".to_string()],
            params: vec![Param {
                name: "x".to_string(),
                ty: Some(RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![],
                }),
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

    // --- Item::Impl tests ---

    #[test]
    fn test_generate_impl_new() {
        let item = Item::Impl {
            struct_name: "Foo".to_string(),
            for_trait: None,
            methods: vec![Method {
                vis: Visibility::Public,
                name: "new".to_string(),
                has_self: false,
                has_mut_self: false,
                params: vec![Param {
                    name: "x".to_string(),
                    ty: Some(RustType::F64),
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
            for_trait: None,
            methods: vec![Method {
                vis: Visibility::Public,
                name: "get_name".to_string(),
                has_self: true,
                has_mut_self: false,
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

    // --- Multiple items ---

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

    // --- Item::Trait tests ---

    #[test]
    fn test_generate_trait() {
        let item = Item::Trait {
            vis: Visibility::Public,
            name: "AnimalTrait".to_string(),
            methods: vec![Method {
                vis: Visibility::Private,
                name: "speak".to_string(),
                has_self: true,
                has_mut_self: false,
                params: vec![],
                return_type: Some(RustType::String),
                body: vec![],
            }],
        };
        let expected = "\
pub trait AnimalTrait {
    fn speak(&self) -> String;
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_impl_for_trait() {
        let item = Item::Impl {
            struct_name: "Dog".to_string(),
            for_trait: Some("AnimalTrait".to_string()),
            methods: vec![Method {
                vis: Visibility::Private,
                name: "speak".to_string(),
                has_self: true,
                has_mut_self: false,
                params: vec![],
                return_type: Some(RustType::String),
                body: vec![Stmt::Return(Some(Expr::FieldAccess {
                    object: Box::new(Expr::Ident("self".to_string())),
                    field: "name".to_string(),
                }))],
            }],
        };
        let expected = "\
impl AnimalTrait for Dog {
    fn speak(&self) -> String {
        self.name
    }
}";
        assert_eq!(generate(&[item]), expected);
    }
}
