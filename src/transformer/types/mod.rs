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
        TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsIntersectionType(_),
        ) => Err(anyhow!(
            "intersection types are not supported in type annotation position"
        )),
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
        TsType::TsIndexedAccessType(indexed) => convert_indexed_access_type(indexed),
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
/// Converts an interface declaration into one or more IR items.
///
/// - Properties only → `[Struct]`
/// - Methods only → `[Trait]`
/// - Call signatures only → `[TypeAlias]` (fn type)
/// - Properties + Methods mixed → `[Struct, Trait, Impl]`
pub fn convert_interface_items(decl: &TsInterfaceDecl, vis: Visibility) -> Result<Vec<Item>> {
    let name = decl.id.sym.to_string();
    let type_params = extract_type_params(decl.type_params.as_deref());

    let has_methods = decl
        .body
        .body
        .iter()
        .any(|m| matches!(m, TsTypeElement::TsMethodSignature(_)));
    let has_properties = decl
        .body
        .body
        .iter()
        .any(|m| matches!(m, TsTypeElement::TsPropertySignature(_)));
    let has_call_signatures = decl
        .body
        .body
        .iter()
        .any(|m| matches!(m, TsTypeElement::TsCallSignatureDecl(_)));

    if has_call_signatures && !has_methods && !has_properties {
        let item = convert_interface_as_fn_type(decl, vis, &name, type_params)?;
        return Ok(vec![item]);
    }

    if has_methods && has_properties {
        return convert_interface_as_struct_and_trait(decl, vis, &name, type_params);
    }

    if has_methods {
        let item = convert_interface_as_trait(decl, vis, &name, type_params)?;
        return Ok(vec![item]);
    }

    let item = convert_interface_as_struct(decl, vis, &name, type_params)?;
    Ok(vec![item])
}

/// Converts an interface into a single IR item (legacy API, delegates to `convert_interface_items`).
pub fn convert_interface(decl: &TsInterfaceDecl, vis: Visibility) -> Result<Item> {
    let items = convert_interface_items(decl, vis)?;
    Ok(items.into_iter().next().unwrap())
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

/// Converts a call-signature-only interface into a fn type alias.
///
/// `interface Foo { (x: number): string }` → `type Foo = fn(f64) -> String`
///
/// When multiple call signatures exist (overloads), uses the one with the most parameters.
fn convert_interface_as_fn_type(
    decl: &TsInterfaceDecl,
    vis: Visibility,
    name: &str,
    type_params: Vec<String>,
) -> Result<Item> {
    let call_sigs: Vec<&swc_ecma_ast::TsCallSignatureDecl> = decl
        .body
        .body
        .iter()
        .filter_map(|m| match m {
            TsTypeElement::TsCallSignatureDecl(sig) => Some(sig),
            _ => None,
        })
        .collect();

    // Pick the signature with the most parameters (for overload resolution)
    let sig = call_sigs
        .iter()
        .max_by_key(|s| s.params.len())
        .ok_or_else(|| anyhow!("no call signatures found"))?;

    let mut param_types = Vec::new();
    for param in &sig.params {
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
            _ => return Err(anyhow!("unsupported call signature parameter pattern")),
        }
    }

    let return_type = sig
        .type_ann
        .as_ref()
        .map(|ann| convert_ts_type(&ann.type_ann))
        .transpose()?
        .unwrap_or(RustType::Unit);

    Ok(Item::TypeAlias {
        vis,
        name: name.to_string(),
        type_params,
        ty: RustType::Fn {
            params: param_types,
            return_type: Box::new(return_type),
        },
    })
}

/// Converts a mixed interface (properties + methods) into struct + trait + impl.
///
/// - Properties → `Item::Struct`
/// - Methods → `Item::Trait` (named `{Name}Trait`)
/// - Impl block → `Item::Impl` (implements `{Name}Trait` for `{Name}`)
fn convert_interface_as_struct_and_trait(
    decl: &TsInterfaceDecl,
    vis: Visibility,
    name: &str,
    type_params: Vec<String>,
) -> Result<Vec<Item>> {
    let mut fields = Vec::new();
    let mut methods = Vec::new();

    for member in &decl.body.body {
        match member {
            TsTypeElement::TsPropertySignature(prop) => {
                fields.push(convert_property_signature(prop)?);
            }
            TsTypeElement::TsMethodSignature(method_sig) => {
                methods.push(convert_method_signature(method_sig)?);
            }
            _ => {
                // Skip unsupported members in mixed interfaces
            }
        }
    }

    let trait_name = format!("{name}Trait");

    let struct_item = Item::Struct {
        vis: vis.clone(),
        name: name.to_string(),
        type_params: type_params.clone(),
        fields,
    };

    let trait_item = Item::Trait {
        vis: vis.clone(),
        name: trait_name.clone(),
        methods: methods.clone(),
    };

    let impl_item = Item::Impl {
        struct_name: name.to_string(),
        for_trait: Some(trait_name),
        methods,
    };

    Ok(vec![struct_item, trait_item, impl_item])
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

/// Converts a [`TsTypeAliasDecl`] into one or more IR items.
///
/// Most type aliases produce a single item. Conditional type fallbacks produce
/// a `Comment` item followed by a placeholder `TypeAlias`.
pub fn convert_type_alias_items(decl: &TsTypeAliasDecl, vis: Visibility) -> Result<Vec<Item>> {
    // Conditional type may produce multiple items (comment + placeholder)
    if let TsType::TsConditionalType(cond) = decl.type_ann.as_ref() {
        let name = decl.id.sym.to_string();
        let type_params = extract_type_params(decl.type_params.as_deref());

        match convert_conditional_type(cond) {
            Ok(ty) => {
                return Ok(vec![Item::TypeAlias {
                    vis,
                    name,
                    type_params,
                    ty,
                }]);
            }
            Err(_) => {
                let comment =
                    format!("TODO: Conditional type not auto-converted\nOriginal TS: type {name}",);
                return Ok(vec![
                    Item::Comment(comment),
                    Item::TypeAlias {
                        vis,
                        name,
                        type_params,
                        ty: RustType::Unit,
                    },
                ]);
            }
        }
    }

    let item = convert_type_alias(decl, vis)?;
    Ok(vec![item])
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

    // Discriminated union: `type X = { kind: "a", ... } | { kind: "b", ... }` → serde-tagged enum
    if let Some(item) = try_convert_discriminated_union(decl, vis.clone())? {
        return Ok(item);
    }

    // General union type: `type X = 200 | 404` or `type X = string | number` → enum
    if let Some(item) = try_convert_general_union(decl, vis.clone())? {
        return Ok(item);
    }

    // Intersection type: `type X = { a: T } & { b: U }` → struct with merged fields
    if let Some(item) = try_convert_intersection_type(decl, vis.clone())? {
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

/// Converts a conditional type expression to a [`RustType`].
///
/// Detects patterns and converts accordingly:
/// - `infer` extraction: `T extends Foo<infer U> ? U : never` → `<T as Foo>::Output`
/// - Type predicate (`true`/`false` branches): `T extends X ? true : false` → `bool`
/// - Other patterns: returns the true branch type
fn convert_conditional_type(cond: &swc_ecma_ast::TsConditionalType) -> Result<RustType> {
    // Pattern: infer extraction — `T extends Foo<infer U> ? U : never`
    if let Some(ty) = try_convert_infer_pattern(cond)? {
        return Ok(ty);
    }

    // Pattern: type predicate — `T extends X ? true : false`
    if is_true_false_literal(&cond.true_type, &cond.false_type) {
        return Ok(RustType::Bool);
    }

    // Default: use the true branch type
    convert_ts_type(&cond.true_type)
}

/// Checks if the true/false branches are `true` and `false` literal types.
fn is_true_false_literal(true_type: &TsType, false_type: &TsType) -> bool {
    let is_true_lit = matches!(
        true_type,
        TsType::TsLitType(lit) if matches!(&lit.lit, swc_ecma_ast::TsLit::Bool(b) if b.value)
    );
    let is_false_lit = matches!(
        false_type,
        TsType::TsLitType(lit) if matches!(&lit.lit, swc_ecma_ast::TsLit::Bool(b) if !b.value)
    );
    is_true_lit && is_false_lit
}

/// Tries to detect the `infer` extraction pattern:
/// `T extends Foo<infer U> ? U : never` → `<T as Foo>::Output`
///
/// Returns `Some(RustType)` if the pattern matches, `None` otherwise.
fn try_convert_infer_pattern(cond: &swc_ecma_ast::TsConditionalType) -> Result<Option<RustType>> {
    // false_type must be `never`
    if !matches!(
        cond.false_type.as_ref(),
        TsType::TsKeywordType(kw) if kw.kind == TsKeywordTypeKind::TsNeverKeyword
    ) {
        return Ok(None);
    }

    // extends_type must contain an `infer` type parameter
    let (container_name, _infer_param) = match extract_infer_info(&cond.extends_type) {
        Some(info) => info,
        None => return Ok(None),
    };

    // check_type should be a type reference (T)
    let check_name = match cond.check_type.as_ref() {
        TsType::TsTypeRef(type_ref) => match &type_ref.type_name {
            swc_ecma_ast::TsEntityName::Ident(ident) => ident.sym.to_string(),
            _ => return Ok(None),
        },
        _ => return Ok(None),
    };

    // Generate `<T as Foo>::Output`
    Ok(Some(RustType::Named {
        name: format!("<{check_name} as {container_name}>::Output"),
        type_args: vec![],
    }))
}

/// Extracts container name and infer parameter name from a type like `Foo<infer U>`.
///
/// Returns `Some((container_name, infer_param_name))` if the pattern matches.
fn extract_infer_info(extends_type: &TsType) -> Option<(String, String)> {
    let type_ref = match extends_type {
        TsType::TsTypeRef(r) => r,
        _ => return None,
    };
    let container_name = match &type_ref.type_name {
        swc_ecma_ast::TsEntityName::Ident(ident) => ident.sym.to_string(),
        _ => return None,
    };
    let params = type_ref.type_params.as_ref()?;
    for param in &params.params {
        if let TsType::TsInferType(infer) = param.as_ref() {
            let infer_name = infer.type_param.name.sym.to_string();
            return Some((container_name, infer_name));
        }
    }
    None
}

/// Tries to convert a type alias with a union body where all members are string literals.
///
/// Returns `Ok(Some(Item::Enum))` if the union is all string literals, `Ok(None)` otherwise.
/// Tries to convert a discriminated union type alias.
///
/// A discriminated union is a union of object types that share a common field
/// with string literal types. Example:
///
/// ```typescript
/// type Event = { kind: "click", x: number } | { kind: "hover", y: number }
/// ```
///
/// Produces a `#[serde(tag = "kind")]` enum with struct variants.
fn try_convert_discriminated_union(
    decl: &TsTypeAliasDecl,
    vis: Visibility,
) -> Result<Option<Item>> {
    let union = match decl.type_ann.as_ref() {
        TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsUnionType(u),
        ) => u,
        _ => return Ok(None),
    };

    // All members must be object type literals
    let type_lits: Vec<&swc_ecma_ast::TsTypeLit> = union
        .types
        .iter()
        .filter_map(|ty| match ty.as_ref() {
            TsType::TsTypeLit(lit) => Some(lit),
            _ => None,
        })
        .collect();

    if type_lits.len() != union.types.len() || type_lits.len() < 2 {
        return Ok(None);
    }

    // Find a common field that has string literal types in all members
    let discriminant_field = find_discriminant_field(&type_lits);
    let discriminant_field = match discriminant_field {
        Some(f) => f,
        None => return Ok(None),
    };

    // Build enum variants
    let mut variants = Vec::new();
    for type_lit in &type_lits {
        let (discriminant_value, other_fields) =
            extract_variant_info(type_lit, &discriminant_field)?;
        variants.push(EnumVariant {
            name: string_to_pascal_case(&discriminant_value),
            value: Some(EnumValue::Str(discriminant_value)),
            data: None,
            fields: other_fields,
        });
    }

    Ok(Some(Item::Enum {
        vis,
        name: decl.id.sym.to_string(),
        serde_tag: Some(discriminant_field),
        variants,
    }))
}

/// Finds a field name that is present in all type literals with a string literal type.
fn find_discriminant_field(type_lits: &[&swc_ecma_ast::TsTypeLit]) -> Option<String> {
    // Collect field names from the first member
    let first = type_lits[0];
    for member in &first.members {
        if let TsTypeElement::TsPropertySignature(prop) = member {
            let field_name = match prop.key.as_ref() {
                Expr::Ident(ident) => ident.sym.to_string(),
                _ => continue,
            };

            // Check if this field has a string literal type
            let has_str_lit = prop
                .type_ann
                .as_ref()
                .is_some_and(|ann| is_string_literal_type(&ann.type_ann));

            if !has_str_lit {
                continue;
            }

            // Check if all other members have this field with a string literal type
            let all_have = type_lits[1..].iter().all(|lit| {
                lit.members.iter().any(|m| {
                    if let TsTypeElement::TsPropertySignature(p) = m {
                        let name = match p.key.as_ref() {
                            Expr::Ident(id) => id.sym.to_string(),
                            _ => return false,
                        };
                        name == field_name
                            && p.type_ann
                                .as_ref()
                                .is_some_and(|ann| is_string_literal_type(&ann.type_ann))
                    } else {
                        false
                    }
                })
            });

            if all_have {
                return Some(field_name);
            }
        }
    }
    None
}

/// Checks if a type is a string literal type (e.g., `"click"`).
fn is_string_literal_type(ty: &TsType) -> bool {
    matches!(
        ty,
        TsType::TsLitType(lit) if matches!(&lit.lit, swc_ecma_ast::TsLit::Str(_))
    )
}

/// Extracts the discriminant value and non-discriminant fields from a type literal.
fn extract_variant_info(
    type_lit: &swc_ecma_ast::TsTypeLit,
    discriminant_field: &str,
) -> Result<(String, Vec<StructField>)> {
    let mut discriminant_value = None;
    let mut fields = Vec::new();

    for member in &type_lit.members {
        match member {
            TsTypeElement::TsPropertySignature(prop) => {
                let field_name = match prop.key.as_ref() {
                    Expr::Ident(ident) => ident.sym.to_string(),
                    _ => return Err(anyhow!("unsupported property key in discriminated union")),
                };

                if field_name == discriminant_field {
                    // Extract discriminant value
                    let ann = prop
                        .type_ann
                        .as_ref()
                        .ok_or_else(|| anyhow!("discriminant field has no type annotation"))?;
                    match ann.type_ann.as_ref() {
                        TsType::TsLitType(lit) => match &lit.lit {
                            swc_ecma_ast::TsLit::Str(s) => {
                                discriminant_value = Some(s.value.to_string_lossy().into_owned());
                            }
                            _ => return Err(anyhow!("discriminant must be a string literal")),
                        },
                        _ => return Err(anyhow!("discriminant must be a string literal type")),
                    }
                } else {
                    // Regular field
                    let field = convert_property_signature(prop)?;
                    fields.push(field);
                }
            }
            _ => return Err(anyhow!("unsupported member in discriminated union variant")),
        }
    }

    let value = discriminant_value.ok_or_else(|| anyhow!("discriminant value not found"))?;
    Ok((value, fields))
}

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
                        fields: vec![],
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
        serde_tag: None,
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
                    serde_tag: None,
                    variants: vec![EnumVariant {
                        name: string_to_pascal_case(&value),
                        value: Some(EnumValue::Str(value)),
                        data: None,
                        fields: vec![],
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
/// Handles numeric literal unions (`type Code = 200 | 404`),
/// primitive type unions (`type Value = string | number`), and
/// type reference unions (`type R = Success | Failure`).
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
                        fields: vec![],
                    });
                }
                swc_ecma_ast::TsLit::Str(s) => {
                    let value = s.value.to_string_lossy().into_owned();
                    variants.push(EnumVariant {
                        name: string_to_pascal_case(&value),
                        value: Some(EnumValue::Str(value)),
                        data: None,
                        fields: vec![],
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
                    fields: vec![],
                });
            }
            TsType::TsTypeRef(type_ref) => {
                let rust_type = convert_type_ref(type_ref)?;
                let variant_name = match &type_ref.type_name {
                    swc_ecma_ast::TsEntityName::Ident(ident) => ident.sym.to_string(),
                    _ => return Err(anyhow!("unsupported qualified type name in union")),
                };
                variants.push(EnumVariant {
                    name: variant_name,
                    value: None,
                    data: Some(rust_type),
                    fields: vec![],
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
        serde_tag: None,
        variants,
    };

    // TODO: nullable union (`type X = string | number | null`) should wrap in Option.
    // Currently we just emit the enum; nullable wrapping is a separate concern.
    let _ = has_null_or_undefined;
    Ok(Some(enum_item))
}

/// Tries to convert a type alias with an intersection type body into a struct.
///
/// Handles intersections of object type literals (`{ a: T } & { b: U }`) by merging
/// all fields into a single struct. Returns `None` if the type alias body is not
/// an intersection type.
fn try_convert_intersection_type(decl: &TsTypeAliasDecl, vis: Visibility) -> Result<Option<Item>> {
    let intersection = match decl.type_ann.as_ref() {
        TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsIntersectionType(i),
        ) => i,
        _ => return Ok(None),
    };

    let mut fields = Vec::new();
    for ty in &intersection.types {
        match ty.as_ref() {
            TsType::TsTypeLit(lit) => {
                for member in &lit.members {
                    match member {
                        TsTypeElement::TsPropertySignature(prop) => {
                            let field = convert_property_signature(prop)?;
                            // Check for duplicate field names
                            if fields.iter().any(|f: &StructField| f.name == field.name) {
                                return Err(anyhow!(
                                    "duplicate field '{}' in intersection type",
                                    field.name
                                ));
                            }
                            fields.push(field);
                        }
                        _ => {
                            return Err(anyhow!(
                                "unsupported intersection member (only property signatures are supported)"
                            ));
                        }
                    }
                }
            }
            _ => {
                return Err(anyhow!(
                    "unsupported intersection member type (only object type literals are supported)"
                ));
            }
        }
    }

    let type_params = extract_type_params(decl.type_params.as_deref());

    Ok(Some(Item::Struct {
        vis,
        name: decl.id.sym.to_string(),
        type_params,
        fields,
    }))
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

/// Converts a TS indexed access type (`T['Key']`) into `RustType::Named { name: "T::Key" }`.
///
/// Only string literal keys are supported.
fn convert_indexed_access_type(indexed: &swc_ecma_ast::TsIndexedAccessType) -> Result<RustType> {
    // Extract the base type name
    let obj_name = match indexed.obj_type.as_ref() {
        TsType::TsTypeRef(type_ref) => match &type_ref.type_name {
            swc_ecma_ast::TsEntityName::Ident(ident) => ident.sym.to_string(),
            _ => return Err(anyhow!("unsupported indexed access base type")),
        },
        _ => return Err(anyhow!("unsupported indexed access base type")),
    };

    // Extract the string literal key
    let key = match indexed.index_type.as_ref() {
        TsType::TsLitType(lit) => match &lit.lit {
            swc_ecma_ast::TsLit::Str(s) => s.value.to_string_lossy().into_owned(),
            _ => {
                return Err(anyhow!(
                    "unsupported indexed access key: only string literals are supported"
                ))
            }
        },
        _ => {
            return Err(anyhow!(
                "unsupported indexed access key: only string literals are supported"
            ))
        }
    };

    Ok(RustType::Named {
        name: format!("{obj_name}::{key}"),
        type_args: vec![],
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
pub(crate) fn convert_property_signature(prop: &TsPropertySignature) -> Result<StructField> {
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
mod tests;
