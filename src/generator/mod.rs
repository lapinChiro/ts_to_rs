//! Code generator: converts IR into Rust source code strings.

pub mod types;

mod expressions;
mod statements;

use crate::ir::{EnumValue, EnumVariant, Item, Method, Param, RustType, TypeParam, Visibility};

use expressions::{escape_ident, generate_expr};
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
///
/// The Generator is a pure IR → text conversion. It does not perform semantic
/// analysis (e.g., scanning for `Regex::new()` to inject imports). All semantic
/// decisions (imports, type coercions, etc.) are the Transformer's responsibility
/// and must be present in the IR items.
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
        Item::Comment(text) => text
            .lines()
            .map(|line| format!("// {line}"))
            .collect::<Vec<_>>()
            .join("\n"),
        Item::Use { vis, path, names } => {
            let vis_prefix = generate_vis(vis);
            if names.len() == 1 {
                format!("{vis_prefix}use {}::{};", path, names[0])
            } else {
                format!("{vis_prefix}use {}::{{{}}};", path, names.join(", "))
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
            let derivable = fields.iter().all(|f| is_derivable_type(&f.ty));
            let mut out = if derivable {
                "#[derive(Debug, Clone, PartialEq)]\n".to_string()
            } else {
                String::new()
            };
            out.push_str(&format!("{vis_str}struct {name}{generics} {{\n"));
            for field in fields {
                let field_vis = generate_vis(field.vis.as_ref().unwrap_or(vis));
                out.push_str(&format!(
                    "    {field_vis}{}: {},\n",
                    escape_ident(&field.name),
                    generate_type(&field.ty)
                ));
            }
            // Add PhantomData for type params not used in any field
            for tp in type_params {
                let used = fields.iter().any(|f| f.ty.uses_param(&tp.name));
                if !used {
                    out.push_str(&format!(
                        "    _phantom_{}: std::marker::PhantomData<{}>,\n",
                        tp.name.to_lowercase(),
                        tp.name
                    ));
                }
            }
            out.push('}');
            out
        }
        Item::Enum {
            vis,
            name,
            serde_tag,
            variants,
        } => generate_enum(vis, name, serde_tag, variants),
        Item::TypeAlias {
            vis,
            name,
            type_params,
            ty,
        } => {
            let vis_str = generate_vis(vis);
            let generics = generate_type_params(type_params);
            format!("{vis_str}type {name}{generics} = {};", generate_type(ty))
        }
        Item::Trait {
            vis,
            name,
            type_params,
            supertraits,
            methods,
            associated_types,
        } => {
            let vis_str = generate_vis(vis);
            let generics = generate_type_params(type_params);
            let bounds = if supertraits.is_empty() {
                String::new()
            } else {
                format!(": {}", supertraits.join(" + "))
            };
            let mut out = format!("{vis_str}trait {name}{generics}{bounds} {{\n");
            for assoc_type in associated_types {
                out.push_str(&format!("    type {assoc_type};\n"));
            }
            for method in methods {
                out.push_str(&generate_trait_method_sig(method));
            }
            out.push('}');
            out
        }
        Item::Impl {
            struct_name,
            for_trait,
            consts,
            methods,
        } => {
            let header = match for_trait {
                Some(trait_name) => format!("impl {trait_name} for {struct_name}"),
                None => format!("impl {struct_name}"),
            };
            let mut out = format!("{header} {{\n");
            let mut first = true;
            for constant in consts {
                if !first {
                    out.push('\n');
                }
                first = false;
                let vis_str = generate_vis(&constant.vis);
                let ty_str = generate_type(&constant.ty);
                let val_str = generate_expr(&constant.value);
                out.push_str(&format!(
                    "    {vis_str}const {}: {ty_str} = {val_str};\n",
                    constant.name
                ));
            }
            let in_trait_impl = for_trait.is_some();
            for method in methods {
                if !first {
                    out.push('\n');
                }
                first = false;
                out.push_str(&generate_method(method, in_trait_impl));
            }
            out.push('}');
            out
        }
        Item::Fn {
            vis,
            attributes,
            is_async,
            name,
            type_params,
            params,
            return_type,
            body,
        } => {
            let vis_str = generate_vis(vis);
            let mut attr_str = String::new();
            for attr in attributes {
                attr_str.push_str(&format!("#[{attr}]\n"));
            }
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
            let name = escape_ident(name);
            let mut out = format!(
                "{attr_str}{vis_str}{async_str}fn {name}{generics}({params_str}){ret_str} {{\n"
            );
            for stmt in body {
                out.push_str(&generate_stmt(stmt, 1));
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

    match &method.body {
        None => {
            // Abstract method — signature only
            format!("    fn {}({params_str}){ret_str};\n", method.name)
        }
        Some(body) => {
            // Default implementation
            let mut out = format!("    fn {}({params_str}){ret_str} {{\n", method.name);
            for stmt in body {
                out.push_str(&generate_stmt(stmt, 2));
                out.push('\n');
            }
            out.push_str("    }\n");
            out
        }
    }
}

/// Generates a method inside an `impl` block.
///
/// When `in_trait_impl` is true, visibility qualifiers are suppressed because
/// Rust does not allow them on trait implementation methods.
/// Empty method bodies with non-unit return types get `todo!()` as a placeholder.
fn generate_method(method: &Method, in_trait_impl: bool) -> String {
    // Trait impl methods must not have visibility qualifiers
    let vis_str = if in_trait_impl {
        String::new()
    } else {
        generate_vis(&method.vis).to_string()
    };
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
    let body = method.body.as_deref().unwrap_or(&[]);
    if body.is_empty() && method.return_type.is_some() {
        // Non-unit return type with empty body: insert todo!() to avoid type mismatch
        out.push_str("        todo!()\n");
    } else {
        for stmt in body {
            out.push_str(&generate_stmt(stmt, 2));
            out.push('\n');
        }
    }
    out.push_str("    }\n");
    out
}

/// Determines whether all enum variants have numeric values (or no values).
/// Checks if any variant has data (tuple-like variant).
fn has_data_variants(variants: &[EnumVariant]) -> bool {
    variants.iter().any(|v| v.data.is_some())
}

fn is_numeric_enum(variants: &[EnumVariant]) -> bool {
    !has_data_variants(variants)
        && variants.iter().all(|v| {
            matches!(
                v.value,
                None | Some(EnumValue::Number(_)) | Some(EnumValue::Expr(_))
            )
        })
}

/// Generates a Rust enum definition from IR.
///
/// - Data enums have tuple-like variants (e.g., `String(String)`, `F64(f64)`).
/// - Numeric enums get `#[repr(i64)]` and discriminant values.
/// - String enums get an `as_str()` impl block.
/// - Enums without values are treated as numeric enums with auto-incrementing values.
fn generate_enum(
    vis: &Visibility,
    name: &str,
    serde_tag: &Option<String>,
    variants: &[EnumVariant],
) -> String {
    // Discriminated union with serde tag
    if let Some(tag) = serde_tag {
        return generate_serde_tagged_enum(vis, name, tag, variants);
    }

    let vis_str = generate_vis(vis);
    let data_enum = has_data_variants(variants);
    let numeric = is_numeric_enum(variants);

    let mut out = String::new();

    if data_enum {
        out.push_str("#[derive(Debug, Clone, PartialEq)]\n");
    } else {
        out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n");
    }
    if numeric {
        out.push_str("#[repr(i64)]\n");
    }
    out.push_str(&format!("{vis_str}enum {name} {{\n"));

    if data_enum {
        for variant in variants {
            if let Some(data_ty) = &variant.data {
                out.push_str(&format!(
                    "    {}({}),\n",
                    variant.name,
                    generate_type(data_ty)
                ));
            } else if let Some(EnumValue::Number(n)) = &variant.value {
                // Numeric literal in a mixed union — unit variant with comment
                out.push_str(&format!("    {}, // = {}\n", variant.name, n));
            } else if let Some(EnumValue::Str(s)) = &variant.value {
                out.push_str(&format!("    {}, // = \"{}\"\n", variant.name, s));
            } else {
                out.push_str(&format!("    {},\n", variant.name));
            }
        }
    } else if numeric {
        let mut next_value: i64 = 0;
        for variant in variants {
            match &variant.value {
                Some(EnumValue::Number(n)) => {
                    next_value = *n + 1;
                    out.push_str(&format!("    {} = {},\n", variant.name, n));
                }
                Some(EnumValue::Expr(expr)) => {
                    out.push_str(&format!("    {} = {},\n", variant.name, expr));
                }
                None => {
                    out.push_str(&format!("    {} = {},\n", variant.name, next_value));
                    next_value += 1;
                }
                _ => unreachable!(),
            };
        }
    } else {
        for variant in variants {
            out.push_str(&format!("    {},\n", variant.name));
        }
    }

    out.push('}');

    // Generate as_str() impl for string enums
    if !numeric && !data_enum {
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

        // Generate Display impl for string enums
        out.push_str(&format!("\n\nimpl std::fmt::Display for {name} {{\n"));
        out.push_str("    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n");
        out.push_str("        write!(f, \"{}\", self.as_str())\n");
        out.push_str("    }\n");
        out.push('}');
    }

    // Generate Display impl for numeric enums
    if numeric {
        out.push_str(&format!("\n\nimpl std::fmt::Display for {name} {{\n"));
        out.push_str("    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n");
        out.push_str("        write!(f, \"{}\", *self as i64)\n");
        out.push_str("    }\n");
        out.push('}');
    }

    out
}

/// Generates a serde-tagged enum for discriminated unions.
///
/// Produces `#[serde(tag = "...")]` on the enum and `#[serde(rename = "...")]` on each variant.
fn generate_serde_tagged_enum(
    vis: &Visibility,
    name: &str,
    tag: &str,
    variants: &[EnumVariant],
) -> String {
    let vis_str = generate_vis(vis);
    let mut out = String::new();

    out.push_str("#[derive(Debug, Clone, PartialEq)]\n");
    out.push_str(&format!("{vis_str}enum {name} {{\n"));

    for variant in variants {
        if variant.fields.is_empty() {
            out.push_str(&format!("    {},\n", variant.name));
        } else {
            out.push_str(&format!("    {} {{\n", variant.name));
            for field in &variant.fields {
                out.push_str(&format!(
                    "        {}: {},\n",
                    field.name,
                    generate_type(&field.ty)
                ));
            }
            out.push_str("    },\n");
        }
    }

    out.push('}');

    // Generate tag accessor method: fn kind(&self) -> &str { match self { ... } }
    let tag_method = escape_rust_keyword(tag);
    out.push_str(&format!("\n\nimpl {name} {{\n"));
    out.push_str(&format!("    pub fn {tag_method}(&self) -> &str {{\n"));
    out.push_str("        match self {\n");
    for variant in variants {
        if let Some(EnumValue::Str(s)) = &variant.value {
            let pattern_suffix = if variant.fields.is_empty() {
                String::new()
            } else {
                " { .. }".to_string()
            };
            out.push_str(&format!(
                "            {name}::{}{pattern_suffix} => \"{s}\",\n",
                variant.name
            ));
        }
    }
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push('}');

    out
}

/// Returns true if a type can appear in a struct with `#[derive(Debug, Clone, PartialEq)]`.
///
/// `Box<dyn Fn>` and `Box<dyn Any>` do not implement these traits, so structs
/// containing them cannot derive them.
fn is_derivable_type(ty: &RustType) -> bool {
    match ty {
        RustType::Fn { .. } | RustType::Any | RustType::DynTrait(_) => false,
        RustType::Option(inner) | RustType::Vec(inner) | RustType::Ref(inner) => {
            is_derivable_type(inner)
        }
        RustType::Result { ok, err } => is_derivable_type(ok) && is_derivable_type(err),
        RustType::Tuple(elems) => elems.iter().all(is_derivable_type),
        RustType::Named { type_args, .. } => type_args.iter().all(is_derivable_type),
        _ => true,
    }
}

/// Rust の予約語をエスケープする（`type` → `r#type`）。
fn escape_rust_keyword(name: &str) -> String {
    match name {
        "type" | "match" | "move" | "ref" | "self" | "super" | "crate" | "fn" | "let" | "mut"
        | "pub" | "return" | "static" | "struct" | "trait" | "use" | "where" | "while"
        | "async" | "await" | "dyn" | "abstract" | "become" | "box" | "do" | "final" | "macro"
        | "override" | "priv" | "typeof" | "unsized" | "virtual" | "yield" | "try" | "mod"
        | "enum" | "extern" | "const" | "continue" | "break" | "else" | "false" | "for" | "if"
        | "impl" | "in" | "loop" | "true" | "unsafe" | "as" => {
            format!("r#{name}")
        }
        _ => name.to_string(),
    }
}

/// Generates the visibility prefix string.
fn generate_vis(vis: &Visibility) -> &'static str {
    match vis {
        Visibility::Public => "pub ",
        Visibility::PubCrate => "pub(crate) ",
        Visibility::Private => "",
    }
}

/// Generates the generic type parameters string (e.g., `<T, U>` or `<T: Foo>`).
///
/// Returns an empty string if there are no type parameters.
fn generate_type_params(type_params: &[TypeParam]) -> String {
    if type_params.is_empty() {
        String::new()
    } else {
        let params: Vec<String> = type_params
            .iter()
            .map(|p| match &p.constraint {
                Some(ty) => format!("{}: {}", p.name, generate_type(ty)),
                None => p.name.clone(),
            })
            .collect();
        format!("<{}>", params.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        BinOp, EnumValue, EnumVariant, Expr, Item, Method, Param, RustType, Stmt, StructField,
        TypeParam, Visibility,
    };

    // --- Item::Use tests ---

    #[test]
    fn test_generate_use_single() {
        let item = Item::Use {
            vis: Visibility::Private,
            path: "crate::bar".to_string(),
            names: vec!["Foo".to_string()],
        };
        assert_eq!(generate(&[item]), "use crate::bar::Foo;");
    }

    #[test]
    fn test_generate_use_multiple() {
        let item = Item::Use {
            vis: Visibility::Private,
            path: "crate::bar".to_string(),
            names: vec!["A".to_string(), "B".to_string()],
        };
        assert_eq!(generate(&[item]), "use crate::bar::{A, B};");
    }

    #[test]
    fn test_generate_pub_use_single() {
        let item = Item::Use {
            vis: Visibility::Public,
            path: "crate::bar".to_string(),
            names: vec!["Foo".to_string()],
        };
        assert_eq!(generate(&[item]), "pub use crate::bar::Foo;");
    }

    #[test]
    fn test_generate_pub_use_multiple() {
        let item = Item::Use {
            vis: Visibility::Public,
            path: "crate::baz".to_string(),
            names: vec!["A".to_string(), "B".to_string()],
        };
        assert_eq!(generate(&[item]), "pub use crate::baz::{A, B};");
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
                    vis: None,
                    name: "name".to_string(),
                    ty: RustType::String,
                },
                StructField {
                    vis: None,
                    name: "age".to_string(),
                    ty: RustType::F64,
                },
            ],
        };
        let expected = "\
#[derive(Debug, Clone, PartialEq)]
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
                vis: None,
                name: "x".to_string(),
                ty: RustType::Bool,
            }],
        };
        let expected = "\
#[derive(Debug, Clone, PartialEq)]
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
            type_params: vec![TypeParam {
                name: "T".to_string(),
                constraint: None,
            }],
            fields: vec![StructField {
                vis: None,
                name: "value".to_string(),
                ty: RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![],
                },
            }],
        };
        let expected = "\
#[derive(Debug, Clone, PartialEq)]
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
            serde_tag: None,
            variants: vec![
                EnumVariant {
                    name: "Red".to_string(),
                    value: None,
                    data: None,
                    fields: vec![],
                },
                EnumVariant {
                    name: "Green".to_string(),
                    value: None,
                    data: None,
                    fields: vec![],
                },
                EnumVariant {
                    name: "Blue".to_string(),
                    value: None,
                    data: None,
                    fields: vec![],
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
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, \"{}\", *self as i64)
    }
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_enum_numeric_explicit() {
        let item = Item::Enum {
            vis: Visibility::Public,
            name: "Status".to_string(),
            serde_tag: None,
            variants: vec![
                EnumVariant {
                    name: "Active".to_string(),
                    value: Some(EnumValue::Number(1)),
                    data: None,
                    fields: vec![],
                },
                EnumVariant {
                    name: "Inactive".to_string(),
                    value: Some(EnumValue::Number(0)),
                    data: None,
                    fields: vec![],
                },
            ],
        };
        let expected = "\
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum Status {
    Active = 1,
    Inactive = 0,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, \"{}\", *self as i64)
    }
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_enum_string() {
        let item = Item::Enum {
            vis: Visibility::Public,
            name: "Direction".to_string(),
            serde_tag: None,
            variants: vec![
                EnumVariant {
                    name: "Up".to_string(),
                    value: Some(EnumValue::Str("UP".to_string())),
                    data: None,
                    fields: vec![],
                },
                EnumVariant {
                    name: "Down".to_string(),
                    value: Some(EnumValue::Str("DOWN".to_string())),
                    data: None,
                    fields: vec![],
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
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, \"{}\", self.as_str())
    }
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_enum_private() {
        let item = Item::Enum {
            vis: Visibility::Private,
            name: "Color".to_string(),
            serde_tag: None,
            variants: vec![EnumVariant {
                name: "Red".to_string(),
                value: None,
                data: None,
                fields: vec![],
            }],
        };
        let result = generate(&[item]);
        assert!(!result.contains("pub enum"));
        assert!(result.contains("enum Color"));
    }

    #[test]
    fn test_generate_enum_data_variants() {
        let item = Item::Enum {
            vis: Visibility::Public,
            name: "Value".to_string(),
            serde_tag: None,
            variants: vec![
                EnumVariant {
                    name: "String".to_string(),
                    value: None,
                    data: Some(RustType::String),
                    fields: vec![],
                },
                EnumVariant {
                    name: "F64".to_string(),
                    value: None,
                    data: Some(RustType::F64),
                    fields: vec![],
                },
                EnumVariant {
                    name: "Bool".to_string(),
                    value: None,
                    data: Some(RustType::Bool),
                    fields: vec![],
                },
            ],
        };
        let expected = "\
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    F64(f64),
    Bool(bool),
}";
        assert_eq!(generate(&[item]), expected);
    }

    // --- Item::Fn tests ---

    #[test]
    fn test_generate_fn_simple_return() {
        let item = Item::Fn {
            vis: Visibility::Public,
            attributes: vec![],
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
            body: vec![Stmt::TailExpr(Expr::BinaryOp {
                left: Box::new(Expr::Ident("a".to_string())),
                op: BinOp::Add,
                right: Box::new(Expr::Ident("b".to_string())),
            })],
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
            attributes: vec![],
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
            attributes: vec![],
            is_async: false,
            name: "get_value".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: Some(RustType::F64),
            body: vec![Stmt::TailExpr(Expr::NumberLit(42.0))],
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
            attributes: vec![],
            is_async: false,
            name: "identity".to_string(),
            type_params: vec![TypeParam {
                name: "T".to_string(),
                constraint: None,
            }],
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
            body: vec![Stmt::TailExpr(Expr::Ident("x".to_string()))],
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
            consts: vec![],
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
                body: Some(vec![Stmt::TailExpr(Expr::Ident("Self { x }".to_string()))]),
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
            consts: vec![],
            methods: vec![Method {
                vis: Visibility::Public,
                name: "get_name".to_string(),
                has_self: true,
                has_mut_self: false,
                params: vec![],
                return_type: Some(RustType::String),
                body: Some(vec![Stmt::TailExpr(Expr::FieldAccess {
                    object: Box::new(Expr::Ident("self".to_string())),
                    field: "name".to_string(),
                })]),
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
#[derive(Debug, Clone, PartialEq)]
pub struct A {
}

#[derive(Debug, Clone, PartialEq)]
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
            type_params: vec![],
            methods: vec![Method {
                vis: Visibility::Private,
                name: "speak".to_string(),
                has_self: true,
                has_mut_self: false,
                params: vec![],
                return_type: Some(RustType::String),
                body: None,
            }],
            supertraits: vec![],
            associated_types: vec![],
        };
        let expected = "\
pub trait AnimalTrait {
    fn speak(&self) -> String;
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_trait_with_supertraits_outputs_bounds() {
        let item = Item::Trait {
            vis: Visibility::Public,
            name: "Dog".to_string(),
            type_params: vec![],
            supertraits: vec!["Animal".to_string(), "Debug".to_string()],
            methods: vec![Method {
                vis: Visibility::Private,
                name: "bark".to_string(),
                has_self: true,
                has_mut_self: false,
                params: vec![],
                return_type: None,
                body: None,
            }],
            associated_types: vec![],
        };
        let expected = "\
pub trait Dog: Animal + Debug {
    fn bark(&self);
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_impl_for_trait() {
        let item = Item::Impl {
            struct_name: "Dog".to_string(),
            for_trait: Some("AnimalTrait".to_string()),
            consts: vec![],
            methods: vec![Method {
                vis: Visibility::Private,
                name: "speak".to_string(),
                has_self: true,
                has_mut_self: false,
                params: vec![],
                return_type: Some(RustType::String),
                body: Some(vec![Stmt::TailExpr(Expr::FieldAccess {
                    object: Box::new(Expr::Ident("self".to_string())),
                    field: "name".to_string(),
                })]),
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

    #[test]
    fn test_escape_ident_fn_name_reserved_word_adds_r_hash() {
        let item = Item::Fn {
            vis: Visibility::Public,
            attributes: vec![],
            is_async: false,
            name: "match".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![],
        };
        let output = generate(&[item]);
        assert!(
            output.contains("fn r#match()"),
            "expected r#match in: {output}"
        );
    }

    #[test]
    fn test_generate_fn_with_attributes_outputs_attr_lines() {
        let item = Item::Fn {
            vis: Visibility::Private,
            attributes: vec!["tokio::main".to_string()],
            is_async: true,
            name: "main".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![],
        };
        let expected = "\
#[tokio::main]
async fn main() {
}";
        assert_eq!(generate(&[item]), expected);
    }

    #[test]
    fn test_generate_fn_without_attributes_no_attr_lines() {
        let item = Item::Fn {
            vis: Visibility::Private,
            attributes: vec![],
            is_async: true,
            name: "not_main".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![],
        };
        let output = generate(&[item]);
        assert!(
            !output.contains("#["),
            "expected no attributes in: {output}"
        );
    }

    #[test]
    fn test_escape_ident_struct_field_reserved_word_adds_r_hash() {
        let item = Item::Struct {
            vis: Visibility::Public,
            name: "Foo".to_string(),
            type_params: vec![],
            fields: vec![StructField {
                vis: Some(Visibility::Public),
                name: "type".to_string(),
                ty: RustType::String,
            }],
        };
        let output = generate(&[item]);
        assert!(
            output.contains("r#type: String"),
            "expected r#type in: {output}"
        );
    }

    // --- Expr::Regex tests ---

    #[test]
    fn test_generate_regex_backslash_pattern_uses_raw_string() {
        // \d+ must use raw string to preserve backslashes
        let expr = Expr::Regex {
            pattern: r"\d+".to_string(),
            global: false,
            sticky: false,
        };
        let output = generate_expr(&expr);
        assert_eq!(output, r#"Regex::new(r"\d+").unwrap()"#);
    }

    #[test]
    fn test_generate_regex_quote_pattern_uses_raw_hash_string() {
        // Pattern containing " must use r#"..."#
        let expr = Expr::Regex {
            pattern: r#"a"b"#.to_string(),
            global: false,
            sticky: false,
        };
        let output = generate_expr(&expr);
        assert_eq!(output, r###"Regex::new(r#"a"b"#).unwrap()"###);
    }

    #[test]
    fn test_generate_regex_simple_pattern_uses_raw_string() {
        let expr = Expr::Regex {
            pattern: "pattern".to_string(),
            global: false,
            sticky: false,
        };
        let output = generate_expr(&expr);
        assert_eq!(output, r#"Regex::new(r"pattern").unwrap()"#);
    }
}
