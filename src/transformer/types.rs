//! Type conversion from SWC TypeScript AST to IR.
//!
//! Handles conversion of TypeScript type declarations (interfaces, type aliases)
//! and type annotations into the IR representation.

use anyhow::{anyhow, Result};
use swc_ecma_ast::{
    Expr, TsInterfaceDecl, TsKeywordTypeKind, TsMethodSignature, TsPropertySignature, TsType,
    TsTypeAliasDecl, TsTypeElement,
};

use crate::ir::{EnumValue, EnumVariant, Item, Method, Param, RustType, StructField, Visibility};

/// Converts a SWC [`TsType`] into an IR [`RustType`].
///
/// # Supported conversions
///
/// - `string` -> `String`
/// - `number` -> `f64`
/// - `boolean` -> `bool`
/// - `T[]` -> `Vec<T>`
/// - `Array<T>` -> `Vec<T>`
/// - `T | null` / `T | undefined` -> `Option<T>`
/// - `[T, U, ...]` -> `(T, U, ...)`
///
/// # Errors
///
/// Returns an error for unsupported type constructs.
pub fn convert_ts_type(ts_type: &TsType) -> Result<RustType> {
    match ts_type {
        TsType::TsKeywordType(kw) => match kw.kind {
            TsKeywordTypeKind::TsStringKeyword => Ok(RustType::String),
            TsKeywordTypeKind::TsNumberKeyword => Ok(RustType::F64),
            TsKeywordTypeKind::TsBooleanKeyword => Ok(RustType::Bool),
            TsKeywordTypeKind::TsVoidKeyword => Ok(RustType::Unit),
            TsKeywordTypeKind::TsAnyKeyword | TsKeywordTypeKind::TsUnknownKeyword => {
                Ok(RustType::Any)
            }
            TsKeywordTypeKind::TsNeverKeyword => Ok(RustType::Never),
            other => Err(anyhow!("unsupported keyword type: {:?}", other)),
        },
        TsType::TsArrayType(arr) => {
            let inner = convert_ts_type(&arr.elem_type)?;
            Ok(RustType::Vec(Box::new(inner)))
        }
        TsType::TsTypeRef(type_ref) => convert_type_ref(type_ref),
        TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsUnionType(union),
        ) => convert_union_type(union),
        TsType::TsParenthesizedType(paren) => convert_ts_type(&paren.type_ann),
        TsType::TsFnOrConstructorType(swc_ecma_ast::TsFnOrConstructorType::TsFnType(fn_type)) => {
            convert_fn_type(fn_type)
        }
        TsType::TsTupleType(tuple) => {
            let elems = tuple
                .elem_types
                .iter()
                .map(|elem| convert_ts_type(&elem.ty))
                .collect::<Result<Vec<_>>>()?;
            Ok(RustType::Tuple(elems))
        }
        _ => Err(anyhow!("unsupported type: {:?}", ts_type)),
    }
}

/// Converts a type reference like `Array<T>`.
fn convert_type_ref(type_ref: &swc_ecma_ast::TsTypeRef) -> Result<RustType> {
    let name = match &type_ref.type_name {
        swc_ecma_ast::TsEntityName::Ident(ident) => ident.sym.to_string(),
        _ => return Err(anyhow!("unsupported qualified type name")),
    };

    match name.as_str() {
        "Array" => {
            let params = type_ref
                .type_params
                .as_ref()
                .ok_or_else(|| anyhow!("Array requires a type parameter"))?;
            if params.params.len() != 1 {
                return Err(anyhow!("Array expects exactly one type parameter"));
            }
            let inner = convert_ts_type(&params.params[0])?;
            Ok(RustType::Vec(Box::new(inner)))
        }
        // User-defined types: pass through as Named, with any generic type arguments
        other => {
            let type_args = match &type_ref.type_params {
                Some(params) => params
                    .params
                    .iter()
                    .map(|p| convert_ts_type(p))
                    .collect::<Result<Vec<_>>>()?,
                None => vec![],
            };
            Ok(RustType::Named {
                name: other.to_string(),
                type_args,
            })
        }
    }
}

/// Converts a union type. Handles `T | null` and `T | undefined` as `Option<T>`.
fn convert_union_type(union: &swc_ecma_ast::TsUnionType) -> Result<RustType> {
    let mut non_null_types: Vec<&TsType> = Vec::new();
    let mut has_null_or_undefined = false;

    for ty in &union.types {
        match ty.as_ref() {
            TsType::TsKeywordType(kw)
                if kw.kind == TsKeywordTypeKind::TsNullKeyword
                    || kw.kind == TsKeywordTypeKind::TsUndefinedKeyword =>
            {
                has_null_or_undefined = true;
            }
            other => {
                non_null_types.push(other);
            }
        }
    }

    if has_null_or_undefined && non_null_types.len() == 1 {
        let inner = convert_ts_type(non_null_types[0])?;
        Ok(RustType::Option(Box::new(inner)))
    } else if has_null_or_undefined && non_null_types.is_empty() {
        // `null | undefined` — treat as Option of unit, but we don't have unit type
        Err(anyhow!("union of only null/undefined is not supported"))
    } else if !has_null_or_undefined {
        Err(anyhow!("non-nullable union types are not supported"))
    } else {
        Err(anyhow!(
            "union with multiple non-null types is not supported"
        ))
    }
}

/// Converts a [`TsInterfaceDecl`] into an IR [`Item::Struct`] or [`Item::Trait`].
///
/// - Properties-only interface → `Item::Struct` (each property becomes a field)
/// - Interface with method signatures → `Item::Trait` (each method becomes a trait method)
///
/// # Errors
///
/// Returns an error if a member has an unsupported type or pattern.
pub fn convert_interface(decl: &TsInterfaceDecl, vis: Visibility) -> Result<Item> {
    let name = decl.id.sym.to_string();
    let type_params = extract_type_params(decl.type_params.as_deref());

    let has_methods = decl
        .body
        .body
        .iter()
        .any(|m| matches!(m, TsTypeElement::TsMethodSignature(_)));

    if has_methods {
        convert_interface_as_trait(decl, vis, &name, type_params)
    } else {
        convert_interface_as_struct(decl, vis, &name, type_params)
    }
}

/// Converts an interface with only property signatures into an IR [`Item::Struct`].
fn convert_interface_as_struct(
    decl: &TsInterfaceDecl,
    vis: Visibility,
    name: &str,
    type_params: Vec<String>,
) -> Result<Item> {
    let mut fields = Vec::new();

    for member in &decl.body.body {
        match member {
            TsTypeElement::TsPropertySignature(prop) => {
                let field = convert_property_signature(prop)?;
                fields.push(field);
            }
            _ => {
                return Err(anyhow!(
                    "unsupported interface member (only property signatures are supported)"
                ));
            }
        }
    }

    Ok(Item::Struct {
        vis,
        name: name.to_string(),
        type_params,
        fields,
    })
}

/// Converts an interface with method signatures into an IR [`Item::Trait`].
fn convert_interface_as_trait(
    decl: &TsInterfaceDecl,
    vis: Visibility,
    name: &str,
    type_params: Vec<String>,
) -> Result<Item> {
    let mut methods = Vec::new();

    for member in &decl.body.body {
        match member {
            TsTypeElement::TsMethodSignature(method_sig) => {
                let method = convert_method_signature(method_sig)?;
                methods.push(method);
            }
            TsTypeElement::TsPropertySignature(_) => {
                // Properties in a trait interface are skipped for now.
                // Trait cannot have fields in Rust.
            }
            _ => {
                return Err(anyhow!(
                    "unsupported interface member (only property and method signatures are supported)"
                ));
            }
        }
    }

    // type_params are not directly on Trait in current IR, so we ignore them for now.
    // TODO: Add type_params to Item::Trait when needed.
    let _ = type_params;

    Ok(Item::Trait {
        vis,
        name: name.to_string(),
        methods,
    })
}

/// Converts a [`TsMethodSignature`] into an IR [`Method`] (signature only, no body).
fn convert_method_signature(sig: &TsMethodSignature) -> Result<Method> {
    let name = match sig.key.as_ref() {
        swc_ecma_ast::Expr::Ident(ident) => ident.sym.to_string(),
        _ => {
            return Err(anyhow!(
                "unsupported method signature key (only identifiers)"
            ))
        }
    };

    let mut params = Vec::new();
    for param in &sig.params {
        match param {
            swc_ecma_ast::TsFnParam::Ident(ident) => {
                let param_name = ident.id.sym.to_string();
                let ty = ident
                    .type_ann
                    .as_ref()
                    .map(|ann| convert_ts_type(&ann.type_ann))
                    .transpose()?;
                params.push(Param {
                    name: param_name,
                    ty,
                });
            }
            _ => return Err(anyhow!("unsupported method parameter pattern")),
        }
    }

    let return_type = sig
        .type_ann
        .as_ref()
        .map(|ann| convert_ts_type(&ann.type_ann))
        .transpose()?
        .and_then(|ty| if ty == RustType::Unit { None } else { Some(ty) });

    Ok(Method {
        vis: Visibility::Public,
        name,
        has_self: true,
        has_mut_self: false,
        params,
        return_type,
        body: vec![],
    })
}

/// Converts a [`TsTypeAliasDecl`] into an IR item.
///
/// - String literal union → `Item::Enum`
/// - Function type → `Item::TypeAlias` with `RustType::Fn`
/// - Object type literal → `Item::Struct`
///
/// # Errors
///
/// Returns an error if the type alias body is not a supported form.
pub fn convert_type_alias(decl: &TsTypeAliasDecl, vis: Visibility) -> Result<Item> {
    let name = decl.id.sym.to_string();

    // String literal union: `type X = "a" | "b" | "c"` → enum
    if let Some(item) = try_convert_string_literal_union(decl, vis.clone())? {
        return Ok(item);
    }

    // Single string literal: `type X = "only"` → enum with one variant
    if let Some(item) = try_convert_single_string_literal(decl, vis.clone())? {
        return Ok(item);
    }

    // General union type: `type X = 200 | 404` or `type X = string | number` → enum
    if let Some(item) = try_convert_general_union(decl, vis.clone())? {
        return Ok(item);
    }

    // Function type: `type Fn = (x: T) => U` → type alias
    if let Some(item) = try_convert_function_type_alias(decl, vis.clone())? {
        return Ok(item);
    }

    // Tuple type: `type Pair = [string, number]` → type alias
    if let Some(item) = try_convert_tuple_type_alias(decl, vis.clone())? {
        return Ok(item);
    }

    match decl.type_ann.as_ref() {
        TsType::TsTypeLit(lit) => {
            let mut fields = Vec::new();
            for member in &lit.members {
                match member {
                    TsTypeElement::TsPropertySignature(prop) => {
                        let field = convert_property_signature(prop)?;
                        fields.push(field);
                    }
                    _ => {
                        return Err(anyhow!(
                            "unsupported type literal member (only property signatures are supported)"
                        ));
                    }
                }
            }
            let type_params = extract_type_params(decl.type_params.as_deref());

            Ok(Item::Struct {
                vis,
                name,
                type_params,
                fields,
            })
        }
        _ => Err(anyhow!(
            "unsupported type alias body (only object type literals are supported)"
        )),
    }
}

/// Tries to convert a type alias with a function type body.
///
/// Returns `Ok(Some(Item::TypeAlias))` if the body is a `TsFnType`, `Ok(None)` otherwise.
fn try_convert_function_type_alias(
    decl: &TsTypeAliasDecl,
    vis: Visibility,
) -> Result<Option<Item>> {
    let fn_type = match decl.type_ann.as_ref() {
        TsType::TsFnOrConstructorType(swc_ecma_ast::TsFnOrConstructorType::TsFnType(f)) => f,
        _ => return Ok(None),
    };

    let mut param_types = Vec::new();
    for param in &fn_type.params {
        match param {
            swc_ecma_ast::TsFnParam::Ident(ident) => {
                let ty = ident
                    .type_ann
                    .as_ref()
                    .map(|ann| convert_ts_type(&ann.type_ann))
                    .transpose()?
                    .unwrap_or(RustType::Any);
                param_types.push(ty);
            }
            _ => return Err(anyhow!("unsupported function type parameter pattern")),
        }
    }

    let return_type = convert_ts_type(&fn_type.type_ann.type_ann)?;

    let name = decl.id.sym.to_string();
    let type_params = extract_type_params(decl.type_params.as_deref());

    Ok(Some(Item::TypeAlias {
        vis,
        name,
        type_params,
        ty: RustType::Fn {
            params: param_types,
            return_type: Box::new(return_type),
        },
    }))
}

/// Tries to convert a type alias with a tuple type body.
///
/// Returns `Ok(Some(Item::TypeAlias))` if the body is a `TsTupleType`, `Ok(None)` otherwise.
fn try_convert_tuple_type_alias(decl: &TsTypeAliasDecl, vis: Visibility) -> Result<Option<Item>> {
    let tuple = match decl.type_ann.as_ref() {
        TsType::TsTupleType(t) => t,
        _ => return Ok(None),
    };

    let elems = tuple
        .elem_types
        .iter()
        .map(|elem| convert_ts_type(&elem.ty))
        .collect::<Result<Vec<_>>>()?;

    let name = decl.id.sym.to_string();
    let type_params = extract_type_params(decl.type_params.as_deref());

    Ok(Some(Item::TypeAlias {
        vis,
        name,
        type_params,
        ty: RustType::Tuple(elems),
    }))
}

/// Tries to convert a type alias with a union body where all members are string literals.
///
/// Returns `Ok(Some(Item::Enum))` if the union is all string literals, `Ok(None)` otherwise.
fn try_convert_string_literal_union(
    decl: &TsTypeAliasDecl,
    vis: Visibility,
) -> Result<Option<Item>> {
    let union = match decl.type_ann.as_ref() {
        TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsUnionType(u),
        ) => u,
        _ => return Ok(None),
    };

    let mut variants = Vec::new();
    for ty in &union.types {
        match ty.as_ref() {
            TsType::TsLitType(lit) => match &lit.lit {
                swc_ecma_ast::TsLit::Str(s) => {
                    let value = s.value.to_string_lossy().into_owned();
                    variants.push(EnumVariant {
                        name: string_to_pascal_case(&value),
                        value: Some(EnumValue::Str(value)),
                        data: None,
                    });
                }
                _ => return Ok(None), // Non-string literal → not a string literal union
            },
            _ => return Ok(None), // Non-literal member → not a string literal union
        }
    }

    Ok(Some(Item::Enum {
        vis,
        name: decl.id.sym.to_string(),
        variants,
    }))
}

/// Tries to convert a type alias with a single string literal body.
///
/// Handles `type X = "only"` as a single-variant enum.
fn try_convert_single_string_literal(
    decl: &TsTypeAliasDecl,
    vis: Visibility,
) -> Result<Option<Item>> {
    match decl.type_ann.as_ref() {
        TsType::TsLitType(lit) => match &lit.lit {
            swc_ecma_ast::TsLit::Str(s) => {
                let value = s.value.to_string_lossy().into_owned();
                Ok(Some(Item::Enum {
                    vis,
                    name: decl.id.sym.to_string(),
                    variants: vec![EnumVariant {
                        name: string_to_pascal_case(&value),
                        value: Some(EnumValue::Str(value)),
                        data: None,
                    }],
                }))
            }
            _ => Ok(None),
        },
        _ => Ok(None),
    }
}

/// Converts a string value to PascalCase for use as an enum variant name.
///
/// Examples: `"up"` → `"Up"`, `"foo-bar"` → `"FooBar"`, `"UPPER_CASE"` → `"UpperCase"`
fn string_to_pascal_case(s: &str) -> String {
    s.split(['-', '_', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let lower = part.to_lowercase();
            let mut chars = lower.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// Tries to convert a type alias with a union type body into an enum.
///
/// Handles numeric literal unions (`type Code = 200 | 404`) and
/// primitive type unions (`type Value = string | number`).
/// Returns `None` if the type alias body is not a union type.
fn try_convert_general_union(decl: &TsTypeAliasDecl, vis: Visibility) -> Result<Option<Item>> {
    let union = match decl.type_ann.as_ref() {
        TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsUnionType(u),
        ) => u,
        _ => return Ok(None),
    };

    // Filter out null/undefined members
    let mut non_null_types: Vec<&TsType> = Vec::new();
    let mut has_null_or_undefined = false;
    for ty in &union.types {
        match ty.as_ref() {
            TsType::TsKeywordType(kw)
                if kw.kind == TsKeywordTypeKind::TsNullKeyword
                    || kw.kind == TsKeywordTypeKind::TsUndefinedKeyword =>
            {
                has_null_or_undefined = true;
            }
            other => non_null_types.push(other),
        }
    }

    // If all members are string literals, `try_convert_string_literal_union` handles it
    if non_null_types.iter().all(|t| {
        matches!(
            t,
            TsType::TsLitType(lit) if matches!(lit.lit, swc_ecma_ast::TsLit::Str(_))
        )
    }) {
        return Ok(None);
    }

    let mut variants = Vec::new();
    for ty in &non_null_types {
        match *ty {
            TsType::TsLitType(lit) => match &lit.lit {
                swc_ecma_ast::TsLit::Number(n) => {
                    let value = n.value as i64;
                    variants.push(EnumVariant {
                        name: format!(
                            "V{}",
                            if value < 0 {
                                format!("Neg{}", -value)
                            } else {
                                value.to_string()
                            }
                        ),
                        value: Some(EnumValue::Number(value)),
                        data: None,
                    });
                }
                swc_ecma_ast::TsLit::Str(s) => {
                    let value = s.value.to_string_lossy().into_owned();
                    variants.push(EnumVariant {
                        name: string_to_pascal_case(&value),
                        value: Some(EnumValue::Str(value)),
                        data: None,
                    });
                }
                _ => return Err(anyhow!("unsupported literal type in union")),
            },
            TsType::TsKeywordType(kw) => {
                let (variant_name, rust_type) = match kw.kind {
                    TsKeywordTypeKind::TsStringKeyword => ("String".to_string(), RustType::String),
                    TsKeywordTypeKind::TsNumberKeyword => ("F64".to_string(), RustType::F64),
                    TsKeywordTypeKind::TsBooleanKeyword => ("Bool".to_string(), RustType::Bool),
                    _ => return Err(anyhow!("unsupported keyword type in union: {:?}", kw.kind)),
                };
                variants.push(EnumVariant {
                    name: variant_name,
                    value: None,
                    data: Some(rust_type),
                });
            }
            _ => return Err(anyhow!("unsupported type in union")),
        }
    }

    if variants.is_empty() {
        return Err(anyhow!("empty union type"));
    }

    let enum_item = Item::Enum {
        vis: vis.clone(),
        name: decl.id.sym.to_string(),
        variants,
    };

    if has_null_or_undefined {
        // Wrap in Option: `type X = string | number | null` → `Option<EnumName>`
        // We still emit the enum, but we'd need a TypeAlias wrapping it in Option.
        // For now, just return the enum (nullable wrapping is a separate concern)
        Ok(Some(enum_item))
    } else {
        Ok(Some(enum_item))
    }
}

/// Converts a TS function type (`(x: number) => string`) into `RustType::Fn`.
fn convert_fn_type(fn_type: &swc_ecma_ast::TsFnType) -> Result<RustType> {
    let params = fn_type
        .params
        .iter()
        .map(|p| {
            let type_ann = match p {
                swc_ecma_ast::TsFnParam::Ident(ident) => ident
                    .type_ann
                    .as_ref()
                    .ok_or_else(|| anyhow!("function type parameter has no type annotation"))?,
                _ => return Err(anyhow!("unsupported function type parameter pattern")),
            };
            convert_ts_type(&type_ann.type_ann)
        })
        .collect::<Result<Vec<_>>>()?;

    let return_type = convert_ts_type(&fn_type.type_ann.type_ann)?;

    Ok(RustType::Fn {
        params,
        return_type: Box::new(return_type),
    })
}

/// Extracts type parameter names from an optional [`TsTypeParamDecl`].
///
/// Returns an empty vec if there are no type parameters.
pub fn extract_type_params(type_params: Option<&swc_ecma_ast::TsTypeParamDecl>) -> Vec<String> {
    match type_params {
        Some(params) => params
            .params
            .iter()
            .map(|p| p.name.sym.to_string())
            .collect(),
        None => vec![],
    }
}

/// Converts a property signature into an IR [`StructField`].
fn convert_property_signature(prop: &TsPropertySignature) -> Result<StructField> {
    let field_name = match prop.key.as_ref() {
        Expr::Ident(ident) => ident.sym.to_string(),
        _ => return Err(anyhow!("unsupported property key (only identifiers)")),
    };

    let type_ann = prop
        .type_ann
        .as_ref()
        .ok_or_else(|| anyhow!("property '{}' has no type annotation", field_name))?;

    let mut ty = convert_ts_type(&type_ann.type_ann)?;

    // Optional properties (`?`) become Option<T>
    if prop.optional {
        // Avoid double-wrapping if the type is already Option (e.g., `name?: string | null`)
        if !matches!(ty, RustType::Option(_)) {
            ty = RustType::Option(Box::new(ty));
        }
    }

    Ok(StructField {
        name: field_name,
        ty,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_typescript;
    use swc_ecma_ast::{Decl, ModuleItem, Stmt};

    /// Helper: parse TS source and extract the first TsInterfaceDecl.
    fn parse_interface(source: &str) -> TsInterfaceDecl {
        let module = parse_typescript(source).expect("parse failed");
        match &module.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::TsInterface(decl))) => *decl.clone(),
            _ => panic!("expected TsInterfaceDecl"),
        }
    }

    /// Helper: parse TS source and extract the first TsTypeAliasDecl.
    fn parse_type_alias(source: &str) -> TsTypeAliasDecl {
        let module = parse_typescript(source).expect("parse failed");
        match &module.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::TsTypeAlias(decl))) => *decl.clone(),
            _ => panic!("expected TsTypeAliasDecl"),
        }
    }

    // -- convert_ts_type tests --

    #[test]
    fn test_convert_ts_type_string() {
        let decl = parse_interface("interface T { x: string; }");
        let prop = match &decl.body.body[0] {
            TsTypeElement::TsPropertySignature(p) => p,
            _ => panic!("expected property signature"),
        };
        let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
        assert_eq!(ty, RustType::String);
    }

    #[test]
    fn test_convert_ts_type_number() {
        let decl = parse_interface("interface T { x: number; }");
        let prop = match &decl.body.body[0] {
            TsTypeElement::TsPropertySignature(p) => p,
            _ => panic!("expected property signature"),
        };
        let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
        assert_eq!(ty, RustType::F64);
    }

    #[test]
    fn test_convert_ts_type_boolean() {
        let decl = parse_interface("interface T { x: boolean; }");
        let prop = match &decl.body.body[0] {
            TsTypeElement::TsPropertySignature(p) => p,
            _ => panic!("expected property signature"),
        };
        let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
        assert_eq!(ty, RustType::Bool);
    }

    #[test]
    fn test_convert_ts_type_array_bracket() {
        let decl = parse_interface("interface T { x: string[]; }");
        let prop = match &decl.body.body[0] {
            TsTypeElement::TsPropertySignature(p) => p,
            _ => panic!("expected property signature"),
        };
        let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
        assert_eq!(ty, RustType::Vec(Box::new(RustType::String)));
    }

    #[test]
    fn test_convert_ts_type_array_generic() {
        let decl = parse_interface("interface T { x: Array<number>; }");
        let prop = match &decl.body.body[0] {
            TsTypeElement::TsPropertySignature(p) => p,
            _ => panic!("expected property signature"),
        };
        let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
        assert_eq!(ty, RustType::Vec(Box::new(RustType::F64)));
    }

    #[test]
    fn test_convert_ts_type_union_null() {
        let decl = parse_interface("interface T { x: string | null; }");
        let prop = match &decl.body.body[0] {
            TsTypeElement::TsPropertySignature(p) => p,
            _ => panic!("expected property signature"),
        };
        let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
        assert_eq!(ty, RustType::Option(Box::new(RustType::String)));
    }

    #[test]
    fn test_convert_ts_type_union_undefined() {
        let decl = parse_interface("interface T { x: number | undefined; }");
        let prop = match &decl.body.body[0] {
            TsTypeElement::TsPropertySignature(p) => p,
            _ => panic!("expected property signature"),
        };
        let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
        assert_eq!(ty, RustType::Option(Box::new(RustType::F64)));
    }

    // -- convert_interface tests --

    #[test]
    fn test_convert_interface_basic() {
        let decl = parse_interface("interface Foo { name: string; age: number; }");
        let item = convert_interface(&decl, Visibility::Public).unwrap();

        match item {
            Item::Struct {
                vis,
                name,
                type_params,
                fields,
            } => {
                assert_eq!(vis, Visibility::Public);
                assert_eq!(name, "Foo");
                assert!(type_params.is_empty());
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].name, "name");
                assert_eq!(fields[0].ty, RustType::String);
                assert_eq!(fields[1].name, "age");
                assert_eq!(fields[1].ty, RustType::F64);
            }
            _ => panic!("expected Item::Struct"),
        }
    }

    #[test]
    fn test_convert_interface_optional_field() {
        let decl = parse_interface("interface Bar { label?: string; }");
        let item = convert_interface(&decl, Visibility::Public).unwrap();

        match item {
            Item::Struct { fields, .. } => {
                assert_eq!(fields[0].name, "label");
                assert_eq!(fields[0].ty, RustType::Option(Box::new(RustType::String)));
            }
            _ => panic!("expected Item::Struct"),
        }
    }

    #[test]
    fn test_convert_interface_optional_union_null_no_double_wrap() {
        // `name?: string | null` should be `Option<String>`, not `Option<Option<String>>`
        let decl = parse_interface("interface Baz { name?: string | null; }");
        let item = convert_interface(&decl, Visibility::Public).unwrap();

        match item {
            Item::Struct { fields, .. } => {
                assert_eq!(fields[0].ty, RustType::Option(Box::new(RustType::String)));
            }
            _ => panic!("expected Item::Struct"),
        }
    }

    #[test]
    fn test_convert_interface_vec_field() {
        let decl = parse_interface("interface Qux { items: number[]; }");
        let item = convert_interface(&decl, Visibility::Public).unwrap();

        match item {
            Item::Struct { fields, .. } => {
                assert_eq!(fields[0].ty, RustType::Vec(Box::new(RustType::F64)));
            }
            _ => panic!("expected Item::Struct"),
        }
    }

    #[test]
    fn test_convert_interface_with_type_params() {
        let decl = parse_interface("interface Container<T> { value: T; }");
        let item = convert_interface(&decl, Visibility::Public).unwrap();

        match item {
            Item::Struct { type_params, .. } => {
                assert_eq!(type_params, vec!["T".to_string()]);
            }
            _ => panic!("expected Item::Struct"),
        }
    }

    #[test]
    fn test_convert_interface_with_multiple_type_params() {
        let decl = parse_interface("interface Pair<A, B> { first: A; second: B; }");
        let item = convert_interface(&decl, Visibility::Public).unwrap();

        match item {
            Item::Struct { type_params, .. } => {
                assert_eq!(type_params, vec!["A".to_string(), "B".to_string()]);
            }
            _ => panic!("expected Item::Struct"),
        }
    }

    // -- convert_interface with method signatures --

    #[test]
    fn test_convert_interface_method_only_generates_trait() {
        let decl = parse_interface("interface Greeter { greet(name: string): string; }");
        let item = convert_interface(&decl, Visibility::Public).unwrap();

        match item {
            Item::Trait { vis, name, methods } => {
                assert_eq!(vis, Visibility::Public);
                assert_eq!(name, "Greeter");
                assert_eq!(methods.len(), 1);
                assert_eq!(methods[0].name, "greet");
                assert!(methods[0].has_self);
                assert_eq!(methods[0].params.len(), 1);
                assert_eq!(methods[0].params[0].name, "name");
                assert_eq!(methods[0].params[0].ty, Some(RustType::String));
                assert_eq!(methods[0].return_type, Some(RustType::String));
            }
            _ => panic!("expected Item::Trait, got {:?}", item),
        }
    }

    #[test]
    fn test_convert_interface_method_no_args_void_return() {
        let decl = parse_interface("interface Runner { run(): void; }");
        let item = convert_interface(&decl, Visibility::Public).unwrap();

        match item {
            Item::Trait { methods, .. } => {
                assert_eq!(methods[0].name, "run");
                assert!(methods[0].has_self);
                assert!(methods[0].params.is_empty());
                assert_eq!(methods[0].return_type, None);
            }
            _ => panic!("expected Item::Trait"),
        }
    }

    #[test]
    fn test_convert_interface_method_multiple_params() {
        let decl = parse_interface("interface Math { add(a: number, b: number): number; }");
        let item = convert_interface(&decl, Visibility::Public).unwrap();

        match item {
            Item::Trait { methods, .. } => {
                assert_eq!(methods[0].params.len(), 2);
                assert_eq!(methods[0].params[0].name, "a");
                assert_eq!(methods[0].params[1].name, "b");
                assert_eq!(methods[0].return_type, Some(RustType::F64));
            }
            _ => panic!("expected Item::Trait"),
        }
    }

    #[test]
    fn test_convert_interface_properties_only_still_struct() {
        let decl = parse_interface("interface Point { x: number; y: number; }");
        let item = convert_interface(&decl, Visibility::Public).unwrap();

        assert!(matches!(item, Item::Struct { .. }));
    }

    #[test]
    fn test_convert_interface_method_with_type_params() {
        let decl =
            parse_interface("interface Repo<T> { find(id: string): T; save(item: T): void; }");
        let item = convert_interface(&decl, Visibility::Public).unwrap();

        match item {
            Item::Trait { name, methods, .. } => {
                assert_eq!(name, "Repo");
                assert_eq!(methods.len(), 2);
                assert_eq!(methods[0].name, "find");
                assert_eq!(methods[1].name, "save");
            }
            _ => panic!("expected Item::Trait"),
        }
    }

    // -- convert_type_alias tests --

    #[test]
    fn test_convert_type_alias_object_literal() {
        let decl = parse_type_alias("type Point = { x: number; y: number; };");
        let item = convert_type_alias(&decl, Visibility::Public).unwrap();

        match item {
            Item::Struct { name, fields, .. } => {
                assert_eq!(name, "Point");
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].name, "x");
                assert_eq!(fields[0].ty, RustType::F64);
                assert_eq!(fields[1].name, "y");
                assert_eq!(fields[1].ty, RustType::F64);
            }
            _ => panic!("expected Item::Struct"),
        }
    }

    #[test]
    fn test_convert_type_alias_with_type_params() {
        let decl = parse_type_alias("type Pair<A, B> = { first: A; second: B; };");
        let item = convert_type_alias(&decl, Visibility::Public).unwrap();

        match item {
            Item::Struct { type_params, .. } => {
                assert_eq!(type_params, vec!["A".to_string(), "B".to_string()]);
            }
            _ => panic!("expected Item::Struct"),
        }
    }

    // -- convert_ts_type: generic type arguments --

    #[test]
    fn test_convert_ts_type_named_with_type_args() {
        // `Container<string>` should become Named { name: "Container", type_args: [String] }
        let decl = parse_interface("interface T { x: Container<string>; }");
        let prop = match &decl.body.body[0] {
            TsTypeElement::TsPropertySignature(p) => p,
            _ => panic!("expected property signature"),
        };
        let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
        assert_eq!(
            ty,
            RustType::Named {
                name: "Container".to_string(),
                type_args: vec![RustType::String],
            }
        );
    }

    #[test]
    fn test_convert_ts_type_named_with_multiple_type_args() {
        let decl = parse_interface("interface T { x: Pair<string, number>; }");
        let prop = match &decl.body.body[0] {
            TsTypeElement::TsPropertySignature(p) => p,
            _ => panic!("expected property signature"),
        };
        let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
        assert_eq!(
            ty,
            RustType::Named {
                name: "Pair".to_string(),
                type_args: vec![RustType::String, RustType::F64],
            }
        );
    }

    #[test]
    fn test_convert_ts_type_named_without_type_args() {
        let decl = parse_interface("interface T { x: Point; }");
        let prop = match &decl.body.body[0] {
            TsTypeElement::TsPropertySignature(p) => p,
            _ => panic!("expected property signature"),
        };
        let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
        assert_eq!(
            ty,
            RustType::Named {
                name: "Point".to_string(),
                type_args: vec![],
            }
        );
    }

    // -- convert_ts_type: function types --

    #[test]
    fn test_convert_ts_type_fn_type() {
        // `callback: (x: number) => string` → Fn { params: [F64], return_type: String }
        let decl = parse_interface("interface T { callback: (x: number) => string; }");
        let prop = match &decl.body.body[0] {
            TsTypeElement::TsPropertySignature(p) => p,
            _ => panic!("expected property signature"),
        };
        let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
        assert_eq!(
            ty,
            RustType::Fn {
                params: vec![RustType::F64],
                return_type: Box::new(RustType::String),
            }
        );
    }

    #[test]
    fn test_convert_ts_type_fn_type_no_params() {
        let decl = parse_interface("interface T { callback: () => boolean; }");
        let prop = match &decl.body.body[0] {
            TsTypeElement::TsPropertySignature(p) => p,
            _ => panic!("expected property signature"),
        };
        let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
        assert_eq!(
            ty,
            RustType::Fn {
                params: vec![],
                return_type: Box::new(RustType::Bool),
            }
        );
    }

    // -- convert_ts_type: keyword types (any, unknown, never) --

    #[test]
    fn test_convert_ts_type_any() {
        let decl = parse_interface("interface T { x: any; }");
        let prop = match &decl.body.body[0] {
            TsTypeElement::TsPropertySignature(p) => p,
            _ => panic!("expected property signature"),
        };
        let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
        assert_eq!(ty, RustType::Any);
    }

    #[test]
    fn test_convert_ts_type_unknown() {
        let decl = parse_interface("interface T { x: unknown; }");
        let prop = match &decl.body.body[0] {
            TsTypeElement::TsPropertySignature(p) => p,
            _ => panic!("expected property signature"),
        };
        let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
        assert_eq!(ty, RustType::Any);
    }

    #[test]
    fn test_convert_ts_type_never() {
        let decl = parse_interface("interface T { x: never; }");
        let prop = match &decl.body.body[0] {
            TsTypeElement::TsPropertySignature(p) => p,
            _ => panic!("expected property signature"),
        };
        let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
        assert_eq!(ty, RustType::Never);
    }

    // -- convert_type_alias: string literal union --

    #[test]
    fn test_convert_type_alias_string_literal_union_produces_enum() {
        let decl = parse_type_alias(r#"type Direction = "up" | "down" | "left" | "right";"#);
        let item = convert_type_alias(&decl, Visibility::Public).unwrap();
        match item {
            Item::Enum {
                vis,
                name,
                variants,
            } => {
                assert_eq!(vis, Visibility::Public);
                assert_eq!(name, "Direction");
                assert_eq!(variants.len(), 4);
                assert_eq!(variants[0].name, "Up");
                assert_eq!(
                    variants[0].value,
                    Some(crate::ir::EnumValue::Str("up".to_string()))
                );
                assert_eq!(variants[1].name, "Down");
                assert_eq!(variants[2].name, "Left");
                assert_eq!(variants[3].name, "Right");
            }
            _ => panic!("expected Item::Enum, got {:?}", item),
        }
    }

    #[test]
    fn test_convert_type_alias_string_literal_union_two_members() {
        let decl = parse_type_alias(r#"type Status = "active" | "inactive";"#);
        let item = convert_type_alias(&decl, Visibility::Public).unwrap();
        match item {
            Item::Enum { name, variants, .. } => {
                assert_eq!(name, "Status");
                assert_eq!(variants.len(), 2);
                assert_eq!(variants[0].name, "Active");
                assert_eq!(variants[1].name, "Inactive");
            }
            _ => panic!("expected Item::Enum"),
        }
    }

    #[test]
    fn test_convert_type_alias_string_literal_union_single_member() {
        let decl = parse_type_alias(r#"type Only = "only";"#);
        let item = convert_type_alias(&decl, Visibility::Public).unwrap();
        match item {
            Item::Enum { name, variants, .. } => {
                assert_eq!(name, "Only");
                assert_eq!(variants.len(), 1);
                assert_eq!(variants[0].name, "Only");
            }
            _ => panic!("expected Item::Enum"),
        }
    }

    #[test]
    fn test_convert_type_alias_string_literal_union_kebab_case() {
        let decl = parse_type_alias(r#"type X = "foo-bar" | "baz-qux";"#);
        let item = convert_type_alias(&decl, Visibility::Public).unwrap();
        match item {
            Item::Enum { variants, .. } => {
                assert_eq!(variants[0].name, "FooBar");
                assert_eq!(variants[1].name, "BazQux");
            }
            _ => panic!("expected Item::Enum"),
        }
    }

    #[test]
    fn test_convert_type_alias_numeric_literal_union_produces_enum() {
        let decl = parse_type_alias("type Code = 200 | 404 | 500;");
        let item = convert_type_alias(&decl, Visibility::Public).unwrap();
        match item {
            Item::Enum {
                vis,
                name,
                variants,
            } => {
                assert_eq!(vis, Visibility::Public);
                assert_eq!(name, "Code");
                assert_eq!(variants.len(), 3);
                assert_eq!(variants[0].name, "V200");
                assert_eq!(variants[0].value, Some(EnumValue::Number(200)));
                assert!(variants[0].data.is_none());
                assert_eq!(variants[1].name, "V404");
                assert_eq!(variants[1].value, Some(EnumValue::Number(404)));
                assert_eq!(variants[2].name, "V500");
                assert_eq!(variants[2].value, Some(EnumValue::Number(500)));
            }
            _ => panic!("expected Item::Enum, got {:?}", item),
        }
    }

    #[test]
    fn test_convert_type_alias_numeric_literal_union_two_members() {
        let decl = parse_type_alias("type Code = 200 | 404;");
        let item = convert_type_alias(&decl, Visibility::Public).unwrap();
        match item {
            Item::Enum { name, variants, .. } => {
                assert_eq!(name, "Code");
                assert_eq!(variants.len(), 2);
            }
            _ => panic!("expected Item::Enum"),
        }
    }

    // -- convert_type_alias: primitive union --

    #[test]
    fn test_convert_type_alias_primitive_union_two_types() {
        let decl = parse_type_alias("type Value = string | number;");
        let item = convert_type_alias(&decl, Visibility::Public).unwrap();
        match item {
            Item::Enum {
                vis,
                name,
                variants,
            } => {
                assert_eq!(vis, Visibility::Public);
                assert_eq!(name, "Value");
                assert_eq!(variants.len(), 2);
                assert_eq!(variants[0].name, "String");
                assert_eq!(variants[0].data, Some(RustType::String));
                assert!(variants[0].value.is_none());
                assert_eq!(variants[1].name, "F64");
                assert_eq!(variants[1].data, Some(RustType::F64));
            }
            _ => panic!("expected Item::Enum, got {:?}", item),
        }
    }

    #[test]
    fn test_convert_type_alias_primitive_union_three_types() {
        let decl = parse_type_alias("type Any = string | number | boolean;");
        let item = convert_type_alias(&decl, Visibility::Public).unwrap();
        match item {
            Item::Enum { name, variants, .. } => {
                assert_eq!(name, "Any");
                assert_eq!(variants.len(), 3);
                assert_eq!(variants[0].name, "String");
                assert_eq!(variants[1].name, "F64");
                assert_eq!(variants[2].name, "Bool");
            }
            _ => panic!("expected Item::Enum"),
        }
    }

    // -- convert_type_alias: mixed union --

    #[test]
    fn test_convert_type_alias_mixed_union_string_and_number_literal() {
        let decl = parse_type_alias(r#"type Mixed = "ok" | 404;"#);
        let item = convert_type_alias(&decl, Visibility::Public).unwrap();
        match item {
            Item::Enum { name, variants, .. } => {
                assert_eq!(name, "Mixed");
                assert_eq!(variants.len(), 2);
                assert_eq!(variants[0].name, "Ok");
                assert_eq!(variants[0].value, Some(EnumValue::Str("ok".to_string())));
                assert!(variants[0].data.is_none());
                assert_eq!(variants[1].name, "V404");
                assert_eq!(variants[1].value, Some(EnumValue::Number(404)));
            }
            _ => panic!("expected Item::Enum, got {:?}", item),
        }
    }

    #[test]
    fn test_convert_type_alias_nullable_union_with_multiple_types() {
        // `type Opt = string | number | null` → enum (nullable wrapping is future work)
        let decl = parse_type_alias("type Opt = string | number | null;");
        let item = convert_type_alias(&decl, Visibility::Public).unwrap();
        match item {
            Item::Enum { name, variants, .. } => {
                assert_eq!(name, "Opt");
                assert_eq!(variants.len(), 2);
                assert_eq!(variants[0].name, "String");
                assert_eq!(variants[1].name, "F64");
            }
            _ => panic!("expected Item::Enum, got {:?}", item),
        }
    }

    #[test]
    fn test_convert_type_alias_non_object_returns_error() {
        let decl = parse_type_alias("type Name = string;");
        let result = convert_type_alias(&decl, Visibility::Public);
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_ts_type_void_returns_unit() {
        let decl = parse_interface("interface T { callback: () => void; }");
        let prop = match &decl.body.body[0] {
            TsTypeElement::TsPropertySignature(p) => p,
            _ => panic!("expected property signature"),
        };
        // The callback type is `() => void`, which is a TsFnType
        // whose return type is void. We check the return type is Unit.
        let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
        assert_eq!(
            ty,
            RustType::Fn {
                params: vec![],
                return_type: Box::new(RustType::Unit),
            }
        );
    }

    // -- convert_type_alias: function type body --

    #[test]
    fn test_convert_type_alias_function_type_single_param() {
        let decl = parse_type_alias("type Handler = (req: Request) => Response;");
        let item = convert_type_alias(&decl, Visibility::Public).unwrap();

        match item {
            Item::TypeAlias {
                vis,
                name,
                type_params,
                ty,
            } => {
                assert_eq!(vis, Visibility::Public);
                assert_eq!(name, "Handler");
                assert!(type_params.is_empty());
                assert_eq!(
                    ty,
                    RustType::Fn {
                        params: vec![RustType::Named {
                            name: "Request".to_string(),
                            type_args: vec![],
                        }],
                        return_type: Box::new(RustType::Named {
                            name: "Response".to_string(),
                            type_args: vec![],
                        }),
                    }
                );
            }
            _ => panic!("expected Item::TypeAlias, got {:?}", item),
        }
    }

    #[test]
    fn test_convert_type_alias_function_type_no_params() {
        let decl = parse_type_alias("type Factory = () => Widget;");
        let item = convert_type_alias(&decl, Visibility::Public).unwrap();

        match item {
            Item::TypeAlias { ty, .. } => {
                assert_eq!(
                    ty,
                    RustType::Fn {
                        params: vec![],
                        return_type: Box::new(RustType::Named {
                            name: "Widget".to_string(),
                            type_args: vec![],
                        }),
                    }
                );
            }
            _ => panic!("expected Item::TypeAlias"),
        }
    }

    #[test]
    fn test_convert_type_alias_function_type_void_return() {
        let decl = parse_type_alias("type Callback = (x: number) => void;");
        let item = convert_type_alias(&decl, Visibility::Public).unwrap();

        match item {
            Item::TypeAlias { ty, .. } => {
                assert_eq!(
                    ty,
                    RustType::Fn {
                        params: vec![RustType::F64],
                        return_type: Box::new(RustType::Unit),
                    }
                );
            }
            _ => panic!("expected Item::TypeAlias"),
        }
    }

    #[test]
    fn test_convert_type_alias_function_type_multiple_params() {
        let decl = parse_type_alias("type ErrorHandler = (err: string, ctx: Context) => Response;");
        let item = convert_type_alias(&decl, Visibility::Public).unwrap();

        match item {
            Item::TypeAlias { ty, .. } => match ty {
                RustType::Fn { params, .. } => {
                    assert_eq!(params.len(), 2);
                    assert_eq!(params[0], RustType::String);
                }
                _ => panic!("expected RustType::Fn"),
            },
            _ => panic!("expected Item::TypeAlias"),
        }
    }

    // -- convert_ts_type: tuple types --

    #[test]
    fn test_convert_ts_type_tuple_two_elements() {
        let decl = parse_interface("interface T { x: [string, number]; }");
        let prop = match &decl.body.body[0] {
            TsTypeElement::TsPropertySignature(p) => p,
            _ => panic!("expected property signature"),
        };
        let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
        assert_eq!(ty, RustType::Tuple(vec![RustType::String, RustType::F64]));
    }

    #[test]
    fn test_convert_ts_type_tuple_single_element() {
        let decl = parse_interface("interface T { x: [boolean]; }");
        let prop = match &decl.body.body[0] {
            TsTypeElement::TsPropertySignature(p) => p,
            _ => panic!("expected property signature"),
        };
        let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
        assert_eq!(ty, RustType::Tuple(vec![RustType::Bool]));
    }

    #[test]
    fn test_convert_ts_type_tuple_empty() {
        let decl = parse_interface("interface T { x: []; }");
        let prop = match &decl.body.body[0] {
            TsTypeElement::TsPropertySignature(p) => p,
            _ => panic!("expected property signature"),
        };
        let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
        assert_eq!(ty, RustType::Tuple(vec![]));
    }

    #[test]
    fn test_convert_ts_type_tuple_nested() {
        let decl = parse_interface("interface T { x: [[string, number], boolean]; }");
        let prop = match &decl.body.body[0] {
            TsTypeElement::TsPropertySignature(p) => p,
            _ => panic!("expected property signature"),
        };
        let ty = convert_ts_type(&prop.type_ann.as_ref().unwrap().type_ann).unwrap();
        assert_eq!(
            ty,
            RustType::Tuple(vec![
                RustType::Tuple(vec![RustType::String, RustType::F64]),
                RustType::Bool,
            ])
        );
    }

    #[test]
    fn test_convert_type_alias_tuple_type() {
        let decl = parse_type_alias("type Pair = [string, number];");
        let item = convert_type_alias(&decl, Visibility::Public).unwrap();

        match item {
            Item::TypeAlias {
                vis,
                name,
                type_params,
                ty,
            } => {
                assert_eq!(vis, Visibility::Public);
                assert_eq!(name, "Pair");
                assert!(type_params.is_empty());
                assert_eq!(ty, RustType::Tuple(vec![RustType::String, RustType::F64]));
            }
            _ => panic!("expected Item::TypeAlias, got {:?}", item),
        }
    }

    #[test]
    fn test_convert_type_alias_function_type_with_generics() {
        let decl = parse_type_alias("type Mapper<T, U> = (item: T) => U;");
        let item = convert_type_alias(&decl, Visibility::Public).unwrap();

        match item {
            Item::TypeAlias { type_params, .. } => {
                assert_eq!(type_params, vec!["T".to_string(), "U".to_string()]);
            }
            _ => panic!("expected Item::TypeAlias"),
        }
    }
}
