//! Type conversion from SWC TypeScript AST to IR.
//!
//! Handles conversion of TypeScript type declarations (interfaces, type aliases)
//! and type annotations into the IR representation.

use anyhow::{anyhow, Result};
use swc_ecma_ast::{
    Expr, TsInterfaceDecl, TsKeywordTypeKind, TsPropertySignature, TsType, TsTypeAliasDecl,
    TsTypeElement,
};

use crate::ir::{Item, RustType, StructField, Visibility};

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

/// Converts a [`TsInterfaceDecl`] into an IR [`Item::Struct`].
///
/// Each property signature becomes a struct field. Optional properties
/// (marked with `?`) are wrapped in `Option<T>`.
///
/// # Errors
///
/// Returns an error if a property has an unsupported type or is not a
/// property signature.
pub fn convert_interface(decl: &TsInterfaceDecl, vis: Visibility) -> Result<Item> {
    let name = decl.id.sym.to_string();
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

    let type_params = extract_type_params(decl.type_params.as_deref());

    Ok(Item::Struct {
        vis,
        name,
        type_params,
        fields,
    })
}

/// Converts a [`TsTypeAliasDecl`] with an object type literal body into an IR [`Item::Struct`].
///
/// # Errors
///
/// Returns an error if the type alias body is not an object type literal.
pub fn convert_type_alias(decl: &TsTypeAliasDecl, vis: Visibility) -> Result<Item> {
    let name = decl.id.sym.to_string();

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
}
