//! Code generator: converts IR into Rust source code strings.

pub mod types;

mod expressions;
mod patterns;
mod statements;

use crate::ir::{
    EnumValue, EnumVariant, Item, Method, Param, RustType, TraitRef, TypeParam, Visibility,
};

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

/// Formats the return type annotation for a function/method signature.
///
/// - `None` → empty (implicit `()`)
/// - `Some(Unit)` → empty (Rust convention: omit `-> ()`)
/// - `Some(ty)` → `" -> {ty}"`
fn format_return_type(return_type: &Option<RustType>) -> String {
    match return_type {
        Some(ty) if *ty != RustType::Unit => format!(" -> {}", generate_type(ty)),
        _ => String::new(),
    }
}

/// Returns true if the return type requires a value (not void/unit/absent).
fn has_non_unit_return_type(return_type: &Option<RustType>) -> bool {
    matches!(return_type, Some(ty) if *ty != RustType::Unit)
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
        Item::RawCode(code) => code.clone(),
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
            is_unit_struct,
        } => {
            // Unit struct: no fields, no generics — emit `struct Name;`
            if *is_unit_struct && fields.is_empty() && type_params.is_empty() {
                let vis_str = generate_vis(vis);
                return format!(
                    "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n{vis_str}struct {name};"
                );
            }
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
            type_params,
            serde_tag,
            variants,
        } => generate_enum(vis, name, type_params, serde_tag, variants),
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
                let refs: Vec<String> = supertraits.iter().map(generate_trait_ref).collect();
                format!(": {}", refs.join(" + "))
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
            type_params,
            for_trait,
            consts,
            methods,
        } => {
            let tp = generate_type_params(type_params);
            let type_args = if type_params.is_empty() {
                String::new()
            } else {
                let args: Vec<&str> = type_params.iter().map(|p| p.name.as_str()).collect();
                format!("<{}>", args.join(", "))
            };
            let header = match for_trait {
                Some(trait_ref) => {
                    let trait_str = generate_trait_ref(trait_ref);
                    format!("impl{tp} {trait_str} for {struct_name}{type_args}")
                }
                None => format!("impl{tp} {struct_name}{type_args}"),
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
        Item::Const {
            vis,
            name,
            ty,
            value,
        } => {
            let vis_str = generate_vis(vis);
            let ty_str = generate_type(ty);
            let value_str = generate_expr(value);
            let name = escape_ident(name);
            format!("{vis_str}const {name}: {ty_str} = {value_str};")
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
            let ret_str = format_return_type(return_type);
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
    let ret_str = format_return_type(&method.return_type);

    let async_str = if method.is_async { "async " } else { "" };

    match &method.body {
        None => {
            // Abstract method — signature only
            format!(
                "    {async_str}fn {}({params_str}){ret_str};\n",
                method.name
            )
        }
        Some(body) => {
            // Default implementation
            let mut out = format!(
                "    {async_str}fn {}({params_str}){ret_str} {{\n",
                method.name
            );
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
    let ret_str = format_return_type(&method.return_type);
    let async_str = if method.is_async { "async " } else { "" };
    let name = &method.name;
    let mut out = format!("    {vis_str}{async_str}fn {name}({params_str}){ret_str} {{\n");
    let body = method.body.as_deref().unwrap_or(&[]);
    if body.is_empty() && has_non_unit_return_type(&method.return_type) {
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
    variants
        .iter()
        .any(|v| v.data.is_some() || !v.fields.is_empty())
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
    type_params: &[TypeParam],
    serde_tag: &Option<String>,
    variants: &[EnumVariant],
) -> String {
    // Discriminated union with serde tag
    if let Some(tag) = serde_tag {
        return generate_serde_tagged_enum(vis, name, type_params, tag, variants);
    }

    let vis_str = generate_vis(vis);
    let tp_str = generate_type_params(type_params);
    let data_enum = has_data_variants(variants);
    let numeric = is_numeric_enum(variants);

    let mut out = String::new();

    let all_derivable = data_enum
        && variants.iter().all(|v| {
            v.data.as_ref().is_none_or(is_derivable_type)
                && v.fields.iter().all(|f| is_derivable_type(&f.ty))
        });
    if data_enum {
        if all_derivable {
            out.push_str("#[derive(Debug, Clone, PartialEq)]\n");
        }
    } else {
        out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n");
    }
    if numeric {
        out.push_str("#[repr(i64)]\n");
    }
    out.push_str(&format!("{vis_str}enum {name}{tp_str} {{\n"));

    if data_enum {
        for variant in variants {
            if !variant.fields.is_empty() {
                // Struct variant (from intersection-with-union distribution)
                out.push_str(&format!("    {} {{\n", variant.name));
                for field in &variant.fields {
                    out.push_str(&format!(
                        "        {}: {},\n",
                        field.name,
                        generate_type(&field.ty)
                    ));
                }
                out.push_str("    },\n");
            } else if let Some(data_ty) = &variant.data {
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

    // Generate Display impl for data enums.
    // Display は Debug derive を持つ enum にのみ生成可能（non-derivable variant
    // を含む enum は Debug も持たないため、{:?} フォールバックが使えない）。
    // Struct variant は match パターンが複雑になるため除外。
    let has_struct_variants = variants.iter().any(|v| !v.fields.is_empty());
    if data_enum && all_derivable && !has_struct_variants {
        out.push_str(&format!("\n\nimpl std::fmt::Display for {name} {{\n"));
        out.push_str("    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n");
        out.push_str("        match self {\n");
        for variant in variants {
            if let Some(ty) = &variant.data {
                if is_display_formattable(ty) {
                    out.push_str(&format!(
                        "            {name}::{}(v) => write!(f, \"{{}}\", v),\n",
                        variant.name,
                    ));
                } else {
                    // Named / complex types: use Debug format
                    out.push_str(&format!(
                        "            {name}::{}(v) => write!(f, \"{{:?}}\", v),\n",
                        variant.name,
                    ));
                }
            } else {
                // Unit variant (e.g., numeric literal in mixed union)
                out.push_str(&format!(
                    "            {name}::{} => write!(f, \"{}\"),\n",
                    variant.name, variant.name,
                ));
            }
        }
        out.push_str("        }\n");
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
    type_params: &[TypeParam],
    tag: &str,
    variants: &[EnumVariant],
) -> String {
    let vis_str = generate_vis(vis);
    let tp_str = generate_type_params(type_params);
    let mut out = String::new();

    let all_derivable = variants
        .iter()
        .all(|v| v.fields.iter().all(|f| is_derivable_type(&f.ty)));
    if all_derivable {
        out.push_str("#[derive(Debug, Clone, PartialEq)]\n");
    }
    out.push_str(&format!("{vis_str}enum {name}{tp_str} {{\n"));

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
    let impl_tp = if type_params.is_empty() {
        String::new()
    } else {
        let param_names: Vec<String> = type_params.iter().map(|p| p.name.clone()).collect();
        format!("<{}>", param_names.join(", "))
    };
    out.push_str(&format!("\n\nimpl{impl_tp} {name}{tp_str} {{\n"));
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

/// Returns true if a type implements `std::fmt::Display`.
///
/// `String`, `f64`, `bool` are Display-formattable via `{}`.
/// `serde_json::Value` (Any) also implements Display.
/// All other types (Named, Tuple, Vec, Fn, etc.) return false and
/// should use `{:?}` (Debug) format if Debug is available.
fn is_display_formattable(ty: &RustType) -> bool {
    matches!(
        ty,
        RustType::String | RustType::F64 | RustType::Bool | RustType::Any
    )
}

/// Returns true if a type can appear in a struct with `#[derive(Debug, Clone, PartialEq)]`.
///
/// `Box<dyn Fn>` and `Box<dyn Any>` do not implement these traits, so structs
/// containing them cannot derive them.
fn is_derivable_type(ty: &RustType) -> bool {
    match ty {
        // Fn（クロージャ）と DynTrait（trait object）は Debug/Clone/PartialEq を derive できない。
        // Any（serde_json::Value）はこれらを全て実装しているため derivable。
        RustType::Fn { .. } | RustType::DynTrait(_) => false,
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

/// trait 参照を Rust コード文字列に変換する（例: `TraitName<T, U>`）。
fn generate_trait_ref(tr: &TraitRef) -> String {
    if tr.type_args.is_empty() {
        tr.name.clone()
    } else {
        let args: Vec<String> = tr.type_args.iter().map(generate_type).collect();
        format!("{}<{}>", tr.name, args.join(", "))
    }
}

#[cfg(test)]
mod tests;
