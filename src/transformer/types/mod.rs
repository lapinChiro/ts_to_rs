//! Type conversion from SWC TypeScript AST to IR.
//!
//! Handles conversion of TypeScript type declarations (interfaces, type aliases)
//! and type annotations into the IR representation.

use anyhow::{anyhow, Result};
use swc_ecma_ast::{
    Expr, TsInterfaceDecl, TsKeywordTypeKind, TsMethodSignature, TsPropertySignature, TsType,
    TsTypeAliasDecl, TsTypeElement,
};

use std::sync::atomic::{AtomicU32, Ordering};

use crate::ir::{EnumValue, EnumVariant, Item, Method, Param, RustType, StructField, Visibility};
use crate::registry::{TypeDef, TypeRegistry};

/// Counter for generating unique synthetic struct names.
static SYNTHETIC_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Returns true if the keyword type is a nullable sentinel (`null`, `undefined`, `void`).
///
/// These types are filtered from union members and cause the union to be wrapped in `Option`.
fn is_nullable_keyword(kind: TsKeywordTypeKind) -> bool {
    matches!(
        kind,
        TsKeywordTypeKind::TsNullKeyword
            | TsKeywordTypeKind::TsUndefinedKeyword
            | TsKeywordTypeKind::TsVoidKeyword
    )
}

/// Resets the synthetic name counter to zero.
///
/// Called at the start of each `transpile` invocation to ensure deterministic naming
/// across test runs. Without reset, test execution order affects generated names.
pub(crate) fn reset_synthetic_counter() {
    SYNTHETIC_COUNTER.store(0, Ordering::Relaxed);
}

/// Generates a unique synthetic struct name with the given prefix (e.g., `_TypeLit0`, `_Intersection1`).
fn generate_synthetic_name(prefix: &str) -> String {
    let id = SYNTHETIC_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("_{prefix}{id}")
}

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
pub fn convert_ts_type(
    ts_type: &TsType,
    extra_items: &mut Vec<Item>,
    reg: &TypeRegistry,
) -> Result<RustType> {
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
            TsKeywordTypeKind::TsObjectKeyword => Ok(RustType::Named {
                name: "serde_json::Value".to_string(),
                type_args: vec![],
            }),
            TsKeywordTypeKind::TsUndefinedKeyword | TsKeywordTypeKind::TsNullKeyword => {
                Ok(RustType::Unit)
            }
            TsKeywordTypeKind::TsBigIntKeyword => Ok(RustType::Named {
                name: "i64".to_string(),
                type_args: vec![],
            }),
            other => Err(anyhow!("unsupported keyword type: {:?}", other)),
        },
        TsType::TsArrayType(arr) => {
            let inner = convert_ts_type(&arr.elem_type, extra_items, reg)?;
            Ok(RustType::Vec(Box::new(inner)))
        }
        TsType::TsTypeRef(type_ref) => convert_type_ref(type_ref, extra_items, reg),
        TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsUnionType(union),
        ) => convert_union_type(union, extra_items, reg),
        TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsIntersectionType(intersection),
        ) => convert_intersection_in_annotation(intersection, extra_items, reg),
        TsType::TsParenthesizedType(paren) => convert_ts_type(&paren.type_ann, extra_items, reg),
        TsType::TsFnOrConstructorType(swc_ecma_ast::TsFnOrConstructorType::TsFnType(fn_type)) => {
            convert_fn_type(fn_type, extra_items, reg)
        }
        TsType::TsTupleType(tuple) => {
            let elems = tuple
                .elem_types
                .iter()
                .map(|elem| convert_ts_type(&elem.ty, extra_items, reg))
                .collect::<Result<Vec<_>>>()?;
            Ok(RustType::Tuple(elems))
        }
        TsType::TsIndexedAccessType(indexed) => {
            convert_indexed_access_type(indexed, extra_items, reg)
        }
        TsType::TsTypeLit(type_lit) => convert_type_lit_in_annotation(type_lit, extra_items, reg),
        TsType::TsLitType(lit) => match &lit.lit {
            swc_ecma_ast::TsLit::Str(_) | swc_ecma_ast::TsLit::Tpl(_) => Ok(RustType::String),
            swc_ecma_ast::TsLit::Bool(_) => Ok(RustType::Bool),
            swc_ecma_ast::TsLit::Number(_) => Ok(RustType::F64),
            swc_ecma_ast::TsLit::BigInt(_) => Ok(RustType::Named {
                name: "i64".to_string(),
                type_args: vec![],
            }),
        },
        TsType::TsConditionalType(cond) => convert_conditional_type(cond, extra_items, reg),
        TsType::TsMappedType(mapped) => {
            // Fallback: treat mapped types as HashMap<String, V>
            let value_type = mapped
                .type_ann
                .as_ref()
                .map(|ann| convert_ts_type(ann, extra_items, reg))
                .transpose()?
                .unwrap_or(RustType::Any);
            Ok(RustType::Named {
                name: "HashMap".to_string(),
                type_args: vec![RustType::String, value_type],
            })
        }
        TsType::TsTypePredicate(_) => {
            // `x is Type` → bool (type guard predicates are booleans at runtime)
            Ok(RustType::Bool)
        }
        TsType::TsTypeOperator(op) => {
            use swc_ecma_ast::TsTypeOperatorOp;
            match op.op {
                // `readonly T[]` → strip readonly, convert inner type
                // Rust enforces immutability through variable bindings, not types
                TsTypeOperatorOp::ReadOnly => convert_ts_type(&op.type_ann, extra_items, reg),
                _ => Err(anyhow!("unsupported type operator: {:?}", op.op)),
            }
        }
        TsType::TsTypeQuery(query) => {
            // `typeof X` → look up X in registry; if found, use that type
            let name = match &query.expr_name {
                swc_ecma_ast::TsTypeQueryExpr::TsEntityName(swc_ecma_ast::TsEntityName::Ident(
                    ident,
                )) => ident.sym.to_string(),
                _ => return Err(anyhow!("unsupported typeof expression")),
            };
            match reg.get(&name) {
                Some(crate::registry::TypeDef::Function {
                    params,
                    return_type,
                    ..
                }) => {
                    let param_types: Vec<RustType> =
                        params.iter().map(|(_, t)| t.clone()).collect();
                    let ret = return_type.clone().unwrap_or(RustType::Unit);
                    Ok(RustType::Fn {
                        params: param_types,
                        return_type: Box::new(ret),
                    })
                }
                _ => Err(anyhow!(
                    "unsupported type: TsTypeQuery for unknown identifier '{name}'"
                )),
            }
        }
        _ => Err(anyhow!("unsupported type: {:?}", ts_type)),
    }
}

/// Converts a type reference like `Array<T>`.
fn convert_type_ref(
    type_ref: &swc_ecma_ast::TsTypeRef,
    extra_items: &mut Vec<Item>,
    reg: &TypeRegistry,
) -> Result<RustType> {
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
            let inner = convert_ts_type(&params.params[0], extra_items, reg)?;
            Ok(RustType::Vec(Box::new(inner)))
        }
        "Record" => {
            let params = type_ref
                .type_params
                .as_ref()
                .ok_or_else(|| anyhow!("Record requires type parameters"))?;
            if params.params.len() != 2 {
                return Err(anyhow!("Record expects exactly two type parameters"));
            }
            let key = convert_ts_type(&params.params[0], extra_items, reg)?;
            let val = convert_ts_type(&params.params[1], extra_items, reg)?;
            Ok(RustType::Named {
                name: "HashMap".to_string(),
                type_args: vec![key, val],
            })
        }
        "Readonly" => {
            // Rust is immutable by default — Readonly<T> is just T
            let params = type_ref
                .type_params
                .as_ref()
                .ok_or_else(|| anyhow!("Readonly requires a type parameter"))?;
            if params.params.len() != 1 {
                return Err(anyhow!("Readonly expects exactly one type parameter"));
            }
            convert_ts_type(&params.params[0], extra_items, reg)
        }
        "Partial" => convert_utility_partial(type_ref, extra_items, reg),
        "Required" => convert_utility_required(type_ref, extra_items, reg),
        "Pick" => convert_utility_pick(type_ref, extra_items, reg),
        "Omit" => convert_utility_omit(type_ref, extra_items, reg),
        "NonNullable" => convert_utility_non_nullable(type_ref, extra_items, reg),
        // User-defined types: pass through as Named, with any generic type arguments
        other => {
            let type_args = match &type_ref.type_params {
                Some(params) => params
                    .params
                    .iter()
                    .map(|p| convert_ts_type(p, extra_items, reg))
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
fn convert_union_type(
    union: &swc_ecma_ast::TsUnionType,
    extra_items: &mut Vec<Item>,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let mut non_null_types: Vec<&TsType> = Vec::new();
    let mut has_null_or_undefined = false;

    for ty in &union.types {
        match ty.as_ref() {
            TsType::TsKeywordType(kw) if is_nullable_keyword(kw.kind) => {
                has_null_or_undefined = true;
            }
            // never is the bottom type — remove from unions (T | never = T)
            TsType::TsKeywordType(kw) if kw.kind == TsKeywordTypeKind::TsNeverKeyword => {}
            other => {
                non_null_types.push(other);
            }
        }
    }

    if has_null_or_undefined && non_null_types.len() == 1 {
        let inner = convert_ts_type(non_null_types[0], extra_items, reg)?;
        Ok(RustType::Option(Box::new(inner)))
    } else if has_null_or_undefined && non_null_types.is_empty() {
        // `null | undefined` — treat as Option of unit, but we don't have unit type
        Err(anyhow!("union of only null/undefined is not supported"))
    } else if !has_null_or_undefined {
        // Convert all members, unwrapping Promise<T> → T
        let mut rust_types = Vec::new();
        for ty in &non_null_types {
            let rust_type = convert_ts_type(ty, extra_items, reg)?;
            let unwrapped = unwrap_promise(rust_type);
            if !rust_types.contains(&unwrapped) {
                rust_types.push(unwrapped);
            }
        }

        // After dedup, if only one type remains, return it directly
        if rust_types.len() == 1 {
            return Ok(rust_types.into_iter().next().unwrap());
        }

        // Generate an enum for non-nullable union in type annotation position
        let mut variants = Vec::new();
        let mut name_parts = Vec::new();
        for rust_type in &rust_types {
            let variant_name = variant_name_from_type(rust_type);
            name_parts.push(variant_name.clone());
            variants.push(EnumVariant {
                name: variant_name,
                value: None,
                data: Some(rust_type.clone()),
                fields: vec![],
            });
        }
        let enum_name = name_parts.join("Or");
        extra_items.push(Item::Enum {
            vis: Visibility::Public,
            name: enum_name.clone(),
            serde_tag: None,
            variants,
        });
        Ok(RustType::Named {
            name: enum_name,
            type_args: vec![],
        })
    } else {
        // has_null_or_undefined && non_null_types.len() > 1
        // e.g., string | number | null → Option<StringOrF64>
        let mut rust_types = Vec::new();
        for ty in &non_null_types {
            let rust_type = convert_ts_type(ty, extra_items, reg)?;
            let unwrapped = unwrap_promise(rust_type);
            if !rust_types.contains(&unwrapped) {
                rust_types.push(unwrapped);
            }
        }

        // After dedup, if only one type remains (e.g., null | undefined | T)
        if rust_types.len() == 1 {
            return Ok(RustType::Option(Box::new(
                rust_types.into_iter().next().unwrap(),
            )));
        }

        // Generate an enum and wrap in Option
        let mut variants = Vec::new();
        let mut name_parts = Vec::new();
        for rust_type in &rust_types {
            let variant_name = variant_name_from_type(rust_type);
            name_parts.push(variant_name.clone());
            variants.push(EnumVariant {
                name: variant_name,
                value: None,
                data: Some(rust_type.clone()),
                fields: vec![],
            });
        }
        let enum_name = name_parts.join("Or");
        extra_items.push(Item::Enum {
            vis: Visibility::Public,
            name: enum_name.clone(),
            serde_tag: None,
            variants,
        });
        Ok(RustType::Option(Box::new(RustType::Named {
            name: enum_name,
            type_args: vec![],
        })))
    }
}

/// Unwraps `Promise<T>` to `T`. Returns the type unchanged for non-Promise types.
fn unwrap_promise(ty: RustType) -> RustType {
    match &ty {
        RustType::Named { name, type_args } if name == "Promise" && type_args.len() == 1 => {
            type_args[0].clone()
        }
        _ => ty,
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
pub fn convert_interface_items(
    decl: &TsInterfaceDecl,
    vis: Visibility,
    reg: &TypeRegistry,
) -> Result<Vec<Item>> {
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
        let item = convert_interface_as_fn_type(decl, vis, &name, type_params, reg)?;
        return Ok(vec![item]);
    }

    if has_methods && has_properties {
        return convert_interface_as_struct_and_trait(decl, vis, &name, type_params, reg);
    }

    if has_methods {
        let item = convert_interface_as_trait(decl, vis, &name, type_params, reg)?;
        return Ok(vec![item]);
    }

    let item = convert_interface_as_struct(decl, vis, &name, type_params, reg)?;
    Ok(vec![item])
}

/// Converts an interface into a single IR item (legacy API, delegates to `convert_interface_items`).
pub fn convert_interface(
    decl: &TsInterfaceDecl,
    vis: Visibility,
    reg: &TypeRegistry,
) -> Result<Item> {
    let items = convert_interface_items(decl, vis, reg)?;
    Ok(items.into_iter().next().unwrap())
}

/// Converts an interface with only property signatures into an IR [`Item::Struct`].
///
/// If the interface extends other interfaces, parent fields are included
/// (flattened) before the child's own fields.
fn convert_interface_as_struct(
    decl: &TsInterfaceDecl,
    vis: Visibility,
    name: &str,
    type_params: Vec<String>,
    reg: &TypeRegistry,
) -> Result<Item> {
    let mut fields = Vec::new();

    // Flatten parent fields from extends chain
    for parent_name in collect_extends_names(decl) {
        if let Some(TypeDef::Struct {
            fields: parent_fields,
            ..
        }) = reg.get(&parent_name)
        {
            for (fname, ftype) in parent_fields {
                if !fields.iter().any(|f: &StructField| f.name == *fname) {
                    fields.push(StructField {
                        vis: Some(Visibility::Public),
                        name: fname.clone(),
                        ty: ftype.clone(),
                    });
                }
            }
        }
    }

    for member in &decl.body.body {
        match member {
            TsTypeElement::TsPropertySignature(prop) => {
                let field = convert_property_signature(prop, &mut Vec::new(), reg)?;
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
    reg: &TypeRegistry,
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
                    .map(|ann| convert_ts_type(&ann.type_ann, &mut Vec::new(), reg))
                    .transpose()?
                    .unwrap_or(RustType::Any);
                param_types.push(ty);
            }
            swc_ecma_ast::TsFnParam::Rest(rest) => {
                // Rest parameter: ...args: T[] → Vec<T>
                let type_ann = rest.type_ann.as_ref().or_else(|| {
                    if let swc_ecma_ast::Pat::Ident(ident) = rest.arg.as_ref() {
                        ident.type_ann.as_ref()
                    } else {
                        None
                    }
                });
                let ty = type_ann
                    .map(|ann| convert_ts_type(&ann.type_ann, &mut Vec::new(), reg))
                    .transpose()?
                    .unwrap_or(RustType::Vec(Box::new(RustType::Any)));
                param_types.push(ty);
            }
            _ => return Err(anyhow!("unsupported call signature parameter pattern")),
        }
    }

    let return_type = sig
        .type_ann
        .as_ref()
        .map(|ann| convert_ts_type(&ann.type_ann, &mut Vec::new(), reg))
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
/// - Properties → `Item::Struct` (named `{Name}Data`)
/// - Methods → `Item::Trait` (named `{Name}` — the interface name)
/// - Impl block → `Item::Impl` (implements `{Name}` for `{Name}Data`)
fn convert_interface_as_struct_and_trait(
    decl: &TsInterfaceDecl,
    vis: Visibility,
    name: &str,
    type_params: Vec<String>,
    reg: &TypeRegistry,
) -> Result<Vec<Item>> {
    let mut fields = Vec::new();
    let mut methods = Vec::new();

    // Flatten parent fields from extends chain
    for parent_name in collect_extends_names(decl) {
        if let Some(TypeDef::Struct {
            fields: parent_fields,
            ..
        }) = reg.get(&parent_name)
        {
            for (fname, ftype) in parent_fields {
                if !fields.iter().any(|f: &StructField| f.name == *fname) {
                    fields.push(StructField {
                        vis: Some(Visibility::Public),
                        name: fname.clone(),
                        ty: ftype.clone(),
                    });
                }
            }
        }
    }

    for member in &decl.body.body {
        match member {
            TsTypeElement::TsPropertySignature(prop) => {
                fields.push(convert_property_signature(prop, &mut Vec::new(), reg)?);
            }
            TsTypeElement::TsMethodSignature(method_sig) => {
                methods.push(convert_method_signature(method_sig, reg)?);
            }
            _ => {
                // Skip unsupported members in mixed interfaces
            }
        }
    }

    let struct_name = format!("{name}Data");
    let supertraits = collect_extends_names(decl);

    let struct_item = Item::Struct {
        vis: vis.clone(),
        name: struct_name.clone(),
        type_params: type_params.clone(),
        fields,
    };

    let trait_item = Item::Trait {
        vis: vis.clone(),
        name: name.to_string(),
        supertraits,
        methods: methods.clone(),
        associated_types: vec![],
    };

    let impl_item = Item::Impl {
        struct_name,
        for_trait: Some(name.to_string()),
        consts: vec![],
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
    reg: &TypeRegistry,
) -> Result<Item> {
    let mut methods = Vec::new();

    for member in &decl.body.body {
        match member {
            TsTypeElement::TsMethodSignature(method_sig) => {
                let method = convert_method_signature(method_sig, reg)?;
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

    let supertraits = collect_extends_names(decl);

    Ok(Item::Trait {
        vis,
        name: name.to_string(),
        supertraits,
        methods,
        associated_types: vec![],
    })
}

/// Collects parent interface names from the `extends` clause of an interface declaration.
fn collect_extends_names(decl: &TsInterfaceDecl) -> Vec<String> {
    decl.extends
        .iter()
        .filter_map(|e| {
            if let swc_ecma_ast::Expr::Ident(ident) = e.expr.as_ref() {
                Some(ident.sym.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Converts a [`TsMethodSignature`] into an IR [`Method`] (signature only, no body).
fn convert_method_signature(sig: &TsMethodSignature, reg: &TypeRegistry) -> Result<Method> {
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
                    .map(|ann| convert_ts_type(&ann.type_ann, &mut Vec::new(), reg))
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
        .map(|ann| convert_ts_type(&ann.type_ann, &mut Vec::new(), reg))
        .transpose()?
        .and_then(|ty| if ty == RustType::Unit { None } else { Some(ty) });

    Ok(Method {
        vis: Visibility::Public,
        name,
        has_self: true,
        has_mut_self: false,
        params,
        return_type,
        body: None,
    })
}

/// Converts a [`TsTypeAliasDecl`] into one or more IR items.
///
/// Most type aliases produce a single item. Conditional type fallbacks produce
/// a `Comment` item followed by a placeholder `TypeAlias`.
pub fn convert_type_alias_items(
    decl: &TsTypeAliasDecl,
    vis: Visibility,
    reg: &TypeRegistry,
) -> Result<Vec<Item>> {
    // Conditional type may produce multiple items (comment + placeholder)
    if let TsType::TsConditionalType(cond) = decl.type_ann.as_ref() {
        let name = decl.id.sym.to_string();
        let type_params = extract_type_params(decl.type_params.as_deref());

        let mut extra_items = Vec::new();
        match convert_conditional_type(cond, &mut extra_items, reg) {
            Ok(ty) => {
                // Remove type params not used in the resolved type
                let used_params = type_params
                    .into_iter()
                    .filter(|p| ty.uses_param(p))
                    .collect();
                let mut items = extra_items;
                items.push(Item::TypeAlias {
                    vis,
                    name,
                    type_params: used_params,
                    ty,
                });
                return Ok(items);
            }
            Err(_) => {
                // Fallback: use the true branch type, or serde_json::Value if that also fails
                let fallback_ty =
                    convert_ts_type(&cond.true_type, &mut Vec::new(), reg).unwrap_or(RustType::Any);
                let used_params = type_params
                    .into_iter()
                    .filter(|p| fallback_ty.uses_param(p))
                    .collect();
                let comment =
                    format!("TODO: Conditional type not auto-converted\nOriginal TS: type {name}",);
                return Ok(vec![
                    Item::Comment(comment),
                    Item::TypeAlias {
                        vis,
                        name,
                        type_params: used_params,
                        ty: fallback_ty,
                    },
                ]);
            }
        }
    }

    let item = convert_type_alias(decl, vis, reg)?;
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
pub fn convert_type_alias(
    decl: &TsTypeAliasDecl,
    vis: Visibility,
    reg: &TypeRegistry,
) -> Result<Item> {
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
    if let Some(item) = try_convert_discriminated_union(decl, vis.clone(), &mut Vec::new(), reg)? {
        return Ok(item);
    }

    // General union type: `type X = 200 | 404` or `type X = string | number` → enum
    if let Some(item) = try_convert_general_union(decl, vis.clone(), &mut Vec::new(), reg)? {
        return Ok(item);
    }

    // Intersection type: `type X = { a: T } & { b: U }` → struct with merged fields
    if let Some(item) = try_convert_intersection_type(decl, vis.clone(), reg, &mut Vec::new())? {
        return Ok(item);
    }

    // Function type: `type Fn = (x: T) => U` → type alias
    if let Some(item) = try_convert_function_type_alias(decl, vis.clone(), &mut Vec::new(), reg)? {
        return Ok(item);
    }

    // Tuple type: `type Pair = [string, number]` → type alias
    if let Some(item) = try_convert_tuple_type_alias(decl, vis.clone(), &mut Vec::new(), reg)? {
        return Ok(item);
    }

    match decl.type_ann.as_ref() {
        TsType::TsTypeLit(lit) => {
            let mut fields = Vec::new();
            for member in &lit.members {
                match member {
                    TsTypeElement::TsPropertySignature(prop) => {
                        let field = convert_property_signature(prop, &mut Vec::new(), reg)?;
                        fields.push(field);
                    }
                    TsTypeElement::TsIndexSignature(idx) => {
                        // { [key: string]: T } → HashMap<String, T>
                        // Convert the entire type literal to a type alias instead of a struct
                        if let Some(type_ann) = &idx.type_ann {
                            let value_type =
                                convert_ts_type(&type_ann.type_ann, &mut Vec::new(), reg)?;
                            let type_params = extract_type_params(decl.type_params.as_deref());
                            return Ok(Item::TypeAlias {
                                vis,
                                name,
                                ty: RustType::Named {
                                    name: "HashMap".to_string(),
                                    type_args: vec![RustType::String, value_type],
                                },
                                type_params,
                            });
                        }
                        return Err(anyhow!(
                            "unsupported index signature without type annotation"
                        ));
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
        TsType::TsKeywordType(_) => {
            let ty = convert_ts_type(decl.type_ann.as_ref(), &mut Vec::new(), reg)?;
            let type_params = extract_type_params(decl.type_params.as_deref());
            Ok(Item::TypeAlias {
                vis,
                name,
                ty,
                type_params,
            })
        }
        _ => Err(anyhow!(
            "unsupported type alias body: {:?}",
            std::mem::discriminant(decl.type_ann.as_ref())
        )),
    }
}

/// Tries to convert a type alias with a function type body.
///
/// Returns `Ok(Some(Item::TypeAlias))` if the body is a `TsFnType`, `Ok(None)` otherwise.
fn try_convert_function_type_alias(
    decl: &TsTypeAliasDecl,
    vis: Visibility,
    extra_items: &mut Vec<Item>,
    reg: &TypeRegistry,
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
                    .map(|ann| convert_ts_type(&ann.type_ann, extra_items, reg))
                    .transpose()?
                    .unwrap_or(RustType::Any);
                param_types.push(ty);
            }
            _ => return Err(anyhow!("unsupported function type parameter pattern")),
        }
    }

    let return_type = convert_ts_type(&fn_type.type_ann.type_ann, extra_items, reg)?;

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
fn try_convert_tuple_type_alias(
    decl: &TsTypeAliasDecl,
    vis: Visibility,
    extra_items: &mut Vec<Item>,
    reg: &TypeRegistry,
) -> Result<Option<Item>> {
    let tuple = match decl.type_ann.as_ref() {
        TsType::TsTupleType(t) => t,
        _ => return Ok(None),
    };

    let elems = tuple
        .elem_types
        .iter()
        .map(|elem| convert_ts_type(&elem.ty, extra_items, reg))
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
fn convert_conditional_type(
    cond: &swc_ecma_ast::TsConditionalType,
    extra_items: &mut Vec<Item>,
    reg: &TypeRegistry,
) -> Result<RustType> {
    // Pattern: infer extraction — `T extends Foo<infer U> ? U : never`
    if let Some((ty, trait_name)) = try_convert_infer_pattern(cond)? {
        // Generate a stub trait for the container (e.g., `pub trait Promise { type Output; }`)
        extra_items.push(Item::Trait {
            vis: Visibility::Public,
            name: trait_name,
            supertraits: vec![],
            methods: vec![],
            associated_types: vec!["Output".to_string()],
        });
        return Ok(ty);
    }

    // Pattern: type predicate — `T extends X ? true : false`
    if is_true_false_literal(&cond.true_type, &cond.false_type) {
        return Ok(RustType::Bool);
    }

    // Default: use the true branch type
    convert_ts_type(&cond.true_type, extra_items, reg)
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
/// Returns `Some((RustType, trait_name))` if the pattern matches, `None` otherwise.
/// The `trait_name` is used to generate a stub trait definition if needed.
fn try_convert_infer_pattern(
    cond: &swc_ecma_ast::TsConditionalType,
) -> Result<Option<(RustType, String)>> {
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
    Ok(Some((
        RustType::Named {
            name: format!("<{check_name} as {container_name}>::Output"),
            type_args: vec![],
        },
        container_name,
    )))
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
    extra_items: &mut Vec<Item>,
    reg: &TypeRegistry,
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
            extract_variant_info(type_lit, &discriminant_field, extra_items, reg)?;
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
    extra_items: &mut Vec<Item>,
    reg: &TypeRegistry,
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
                    let field = convert_property_signature(prop, extra_items, reg)?;
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
            // Skip nullable members in string literal unions (they become Option wrapping)
            TsType::TsKeywordType(kw) if is_nullable_keyword(kw.kind) => {
                continue;
            }
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
pub(crate) fn string_to_pascal_case(s: &str) -> String {
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

/// Generates a PascalCase variant name from a [`RustType`].
fn variant_name_from_type(ty: &RustType) -> String {
    match ty {
        RustType::String => "String".to_string(),
        RustType::F64 => "F64".to_string(),
        RustType::Bool => "Bool".to_string(),
        RustType::Unit => "Unit".to_string(),
        RustType::Any => "Any".to_string(),
        RustType::Never => "Never".to_string(),
        RustType::Named { name, .. } => name.clone(),
        RustType::Vec(inner) => format!("Vec{}", variant_name_from_type(inner)),
        RustType::Option(inner) => format!("Option{}", variant_name_from_type(inner)),
        RustType::Tuple(_) => "Tuple".to_string(),
        RustType::Fn { .. } => "Fn".to_string(),
        RustType::Result { .. } => "Result".to_string(),
    }
}

/// Tries to convert a type alias with a union type body into an enum.
///
/// Handles numeric literal unions (`type Code = 200 | 404`),
/// primitive type unions (`type Value = string | number`), and
/// type reference unions (`type R = Success | Failure`).
/// Returns `None` if the type alias body is not a union type.
fn try_convert_general_union(
    decl: &TsTypeAliasDecl,
    vis: Visibility,
    extra_items: &mut Vec<Item>,
    reg: &TypeRegistry,
) -> Result<Option<Item>> {
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
            TsType::TsKeywordType(kw) if is_nullable_keyword(kw.kind) => {
                has_null_or_undefined = true;
            }
            other => non_null_types.push(other),
        }
    }

    // Nullable union with single non-null type: `type X = T | null` → `type X = Option<T>`
    if has_null_or_undefined && non_null_types.len() == 1 {
        let inner_type = convert_ts_type(non_null_types[0], extra_items, reg)?;
        let type_params = extract_type_params(decl.type_params.as_deref());
        return Ok(Some(Item::TypeAlias {
            vis,
            name: decl.id.sym.to_string(),
            type_params,
            ty: RustType::Option(Box::new(inner_type)),
        }));
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
                    TsKeywordTypeKind::TsBigIntKeyword => (
                        "I64".to_string(),
                        RustType::Named {
                            name: "i64".to_string(),
                            type_args: vec![],
                        },
                    ),
                    TsKeywordTypeKind::TsSymbolKeyword
                    | TsKeywordTypeKind::TsAnyKeyword
                    | TsKeywordTypeKind::TsUnknownKeyword
                    | TsKeywordTypeKind::TsObjectKeyword => ("Any".to_string(), RustType::Any),
                    TsKeywordTypeKind::TsNeverKeyword | TsKeywordTypeKind::TsVoidKeyword => {
                        continue
                    }
                    _ => continue,
                };
                variants.push(EnumVariant {
                    name: variant_name,
                    value: None,
                    data: Some(rust_type),
                    fields: vec![],
                });
            }
            TsType::TsTypeRef(type_ref) => {
                let rust_type = convert_type_ref(type_ref, extra_items, reg)?;
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
            TsType::TsTypeLit(lit) => {
                let mut fields = Vec::new();
                for member in &lit.members {
                    if let TsTypeElement::TsPropertySignature(prop) = member {
                        fields.push(convert_property_signature(prop, extra_items, reg)?);
                    }
                }
                variants.push(EnumVariant {
                    name: format!("Variant{}", variants.len()),
                    value: None,
                    data: None,
                    fields,
                });
            }
            TsType::TsUnionOrIntersectionType(
                swc_ecma_ast::TsUnionOrIntersectionType::TsIntersectionType(intersection),
            ) => {
                let mut fields = Vec::new();
                for member_ty in &intersection.types {
                    if let TsType::TsTypeLit(lit) = member_ty.as_ref() {
                        for member in &lit.members {
                            if let TsTypeElement::TsPropertySignature(prop) = member {
                                fields.push(convert_property_signature(prop, extra_items, reg)?);
                            }
                        }
                    }
                }
                variants.push(EnumVariant {
                    name: format!("Variant{}", variants.len()),
                    value: None,
                    data: None,
                    fields,
                });
            }
            _ => {
                // Fallback: unsupported union member types become Any variant
                variants.push(EnumVariant {
                    name: format!("Other{}", variants.len()),
                    value: None,
                    data: Some(RustType::Any),
                    fields: vec![],
                });
            }
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

    // For multi-type nullable unions (`type X = string | number | null`), we emit
    // the enum as-is. Single-type nullable (`T | null`) is handled above as Option<T>.
    Ok(Some(enum_item))
}

/// Tries to convert a type alias with an intersection type body into a struct.
///
/// Handles intersections of object type literals (`{ a: T } & { b: U }`) by merging
/// all fields into a single struct. Returns `None` if the type alias body is not
/// an intersection type.
fn try_convert_intersection_type(
    decl: &TsTypeAliasDecl,
    vis: Visibility,
    reg: &TypeRegistry,
    extra_items: &mut Vec<Item>,
) -> Result<Option<Item>> {
    let intersection = match decl.type_ann.as_ref() {
        TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsIntersectionType(i),
        ) => i,
        _ => return Ok(None),
    };

    let mut fields = Vec::new();
    for (i, ty) in intersection.types.iter().enumerate() {
        match ty.as_ref() {
            TsType::TsTypeLit(lit) => {
                for member in &lit.members {
                    match member {
                        TsTypeElement::TsPropertySignature(prop) => {
                            let field = convert_property_signature(prop, extra_items, reg)?;
                            if fields.iter().any(|f: &StructField| f.name == field.name) {
                                return Err(anyhow!(
                                    "duplicate field '{}' in intersection type",
                                    field.name
                                ));
                            }
                            fields.push(field);
                        }
                        // TODO: intersection 内の型リテラルにメソッドシグネチャが含まれる場合、
                        // struct フィールドではなく impl ブロックのメソッドとして変換すべき。
                        // 現時点ではスキップ。
                        _ => continue,
                    }
                }
            }
            TsType::TsTypeRef(type_ref) => {
                let type_name = match &type_ref.type_name {
                    swc_ecma_ast::TsEntityName::Ident(ident) => ident.sym.to_string(),
                    _ => return Err(anyhow!("unsupported qualified type name in intersection")),
                };
                // Try to resolve fields from TypeRegistry
                if let Some(crate::registry::TypeDef::Struct {
                    fields: resolved_fields,
                    ..
                }) = reg.get(&type_name)
                {
                    for (name, ty) in resolved_fields {
                        if fields.iter().any(|f: &StructField| f.name == *name) {
                            return Err(anyhow!("duplicate field '{}' in intersection type", name));
                        }
                        fields.push(StructField {
                            vis: None,
                            name: name.clone(),
                            ty: ty.clone(),
                        });
                    }
                } else {
                    // Unresolved type reference — embed as a named field
                    let rust_type = convert_type_ref(type_ref, extra_items, reg)?;
                    fields.push(StructField {
                        vis: None,
                        name: format!("_{i}"),
                        ty: rust_type,
                    });
                }
            }
            // Skip keyword types in intersections (e.g., `string & {}` → use object fields only).
            // This is safe for TypeScript branding patterns where the keyword is nominal.
            TsType::TsKeywordType(_) => continue,
            _ => {
                return Err(anyhow!("unsupported intersection member type"));
            }
        }
    }

    let type_params = extract_type_params(decl.type_params.as_deref());

    // If all intersection members are named type refs that resolve to method-only types
    // (traits), generate a supertrait composition instead of a struct.
    let trait_names: Vec<String> = intersection
        .types
        .iter()
        .filter_map(|ty| {
            if let TsType::TsTypeRef(type_ref) = ty.as_ref() {
                if let swc_ecma_ast::TsEntityName::Ident(ident) = &type_ref.type_name {
                    let name = ident.sym.to_string();
                    if let Some(crate::registry::TypeDef::Struct {
                        fields: f,
                        methods: m,
                        ..
                    }) = reg.get(&name)
                    {
                        if f.is_empty() && !m.is_empty() {
                            return Some(name);
                        }
                    }
                }
            }
            None
        })
        .collect();

    if trait_names.len() == intersection.types.len() && !trait_names.is_empty() {
        // All members are method-only (trait-like) → supertrait composition
        return Ok(Some(Item::Trait {
            vis,
            name: decl.id.sym.to_string(),
            supertraits: trait_names,
            methods: vec![],
            associated_types: vec![],
        }));
    }

    Ok(Some(Item::Struct {
        vis,
        name: decl.id.sym.to_string(),
        type_params,
        fields,
    }))
}

/// Converts an inline type literal in annotation position into a synthetic struct.
///
/// Example: `x: { a: string, b: number }` generates `struct _TypeLit0 { pub a: String, pub b: f64 }`
/// and returns `RustType::Named { name: "_TypeLit0" }`.
fn convert_type_lit_in_annotation(
    type_lit: &swc_ecma_ast::TsTypeLit,
    extra_items: &mut Vec<Item>,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let mut fields = Vec::new();
    for member in &type_lit.members {
        match member {
            TsTypeElement::TsPropertySignature(prop) => {
                fields.push(convert_property_signature(prop, extra_items, reg)?);
            }
            TsTypeElement::TsIndexSignature(idx) => {
                // { [key: string]: T } → HashMap<String, T>
                if let Some(type_ann) = &idx.type_ann {
                    let value_type = convert_ts_type(&type_ann.type_ann, extra_items, reg)?;
                    return Ok(RustType::Named {
                        name: "HashMap".to_string(),
                        type_args: vec![RustType::String, value_type],
                    });
                }
                return Err(anyhow!(
                    "unsupported index signature without type annotation"
                ));
            }
            _ => return Err(anyhow!("unsupported type literal member")),
        }
    }
    let struct_name = generate_synthetic_name("TypeLit");
    extra_items.push(Item::Struct {
        vis: Visibility::Public,
        name: struct_name.clone(),
        type_params: vec![],
        fields,
    });
    Ok(RustType::Named {
        name: struct_name,
        type_args: vec![],
    })
}

/// Converts an intersection type in annotation position into a synthetic merged struct.
///
/// Reuses the same merging logic as `try_convert_intersection_type` (type alias position),
/// but generates a synthetic name since no explicit name is available.
fn convert_intersection_in_annotation(
    intersection: &swc_ecma_ast::TsIntersectionType,
    extra_items: &mut Vec<Item>,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let mut fields = Vec::new();
    for (i, ty) in intersection.types.iter().enumerate() {
        match ty.as_ref() {
            TsType::TsTypeLit(lit) => {
                for member in &lit.members {
                    match member {
                        TsTypeElement::TsPropertySignature(prop) => {
                            let field = convert_property_signature(prop, extra_items, reg)?;
                            if fields.iter().any(|f: &StructField| f.name == field.name) {
                                return Err(anyhow!(
                                    "duplicate field '{}' in intersection type",
                                    field.name
                                ));
                            }
                            fields.push(field);
                        }
                        // TODO: intersection 内の型リテラルにメソッドシグネチャが含まれる場合、
                        // struct フィールドではなく impl ブロックのメソッドとして変換すべき。
                        // 現時点ではスキップ。
                        _ => continue,
                    }
                }
            }
            TsType::TsTypeRef(type_ref) => {
                let rust_type = convert_type_ref(type_ref, extra_items, reg)?;
                fields.push(StructField {
                    vis: None,
                    name: format!("_{i}"),
                    ty: rust_type,
                });
            }
            // Skip keyword types in intersections (branding patterns like `string & {}`)
            TsType::TsKeywordType(_) => continue,
            _ => {
                return Err(anyhow!("unsupported intersection member type"));
            }
        }
    }

    let struct_name = generate_synthetic_name("Intersection");
    extra_items.push(Item::Struct {
        vis: Visibility::Public,
        name: struct_name.clone(),
        type_params: vec![],
        fields,
    });
    Ok(RustType::Named {
        name: struct_name,
        type_args: vec![],
    })
}

/// Converts a TS function type (`(x: number) => string`) into `RustType::Fn`.
fn convert_fn_type(
    fn_type: &swc_ecma_ast::TsFnType,
    extra_items: &mut Vec<Item>,
    reg: &TypeRegistry,
) -> Result<RustType> {
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
            convert_ts_type(&type_ann.type_ann, extra_items, reg)
        })
        .collect::<Result<Vec<_>>>()?;

    let return_type = convert_ts_type(&fn_type.type_ann.type_ann, extra_items, reg)?;

    Ok(RustType::Fn {
        params,
        return_type: Box::new(return_type),
    })
}

/// Converts a TS indexed access type (`T['Key']`) into `RustType::Named { name: "T::Key" }`.
///
/// Only string literal keys are supported.
fn convert_indexed_access_type(
    indexed: &swc_ecma_ast::TsIndexedAccessType,
    _extra_items: &mut Vec<Item>,
    _reg: &TypeRegistry,
) -> Result<RustType> {
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
pub(crate) fn convert_property_signature(
    prop: &TsPropertySignature,
    extra_items: &mut Vec<Item>,
    reg: &TypeRegistry,
) -> Result<StructField> {
    let field_name = match prop.key.as_ref() {
        Expr::Ident(ident) => ident.sym.to_string(),
        _ => return Err(anyhow!("unsupported property key (only identifiers)")),
    };

    let type_ann = prop
        .type_ann
        .as_ref()
        .ok_or_else(|| anyhow!("property '{}' has no type annotation", field_name))?;

    let mut ty = convert_ts_type(&type_ann.type_ann, extra_items, reg)?;

    // Optional properties (`?`) become Option<T>
    if prop.optional {
        // Avoid double-wrapping if the type is already Option (e.g., `name?: string | null`)
        if !matches!(ty, RustType::Option(_)) {
            ty = RustType::Option(Box::new(ty));
        }
    }

    Ok(StructField {
        vis: None,
        name: field_name,
        ty,
    })
}

// -- Utility type helpers --

/// Resolved struct info: (type_name, fields).
type ResolvedFields = (String, Vec<(String, RustType)>);

/// Extracts the inner type name and resolves its fields from the registry.
/// Returns `(type_name, fields)` or `None` if unregistered.
fn resolve_utility_inner_fields<'a>(
    type_ref: &swc_ecma_ast::TsTypeRef,
    extra_items: &'a [Item],
    reg: &'a TypeRegistry,
) -> Option<ResolvedFields> {
    let params = type_ref.type_params.as_ref()?;
    if params.params.is_empty() {
        return None;
    }
    let inner = &params.params[0];
    // Inner type must be a type reference with an ident name
    let inner_name = match inner.as_ref() {
        swc_ecma_ast::TsType::TsTypeRef(inner_ref) => match &inner_ref.type_name {
            swc_ecma_ast::TsEntityName::Ident(ident) => ident.sym.to_string(),
            _ => return None,
        },
        _ => return None,
    };

    // Try registry first, then check extra_items for synthesized structs
    let fields = if let Some(TypeDef::Struct { fields, .. }) = reg.get(&inner_name) {
        fields.clone()
    } else {
        // Look in extra_items for a previously synthesized struct
        extra_items.iter().find_map(|item| match item {
            Item::Struct { name, fields, .. } if name == &inner_name => Some(
                fields
                    .iter()
                    .map(|f| (f.name.clone(), f.ty.clone()))
                    .collect(),
            ),
            _ => None,
        })?
    };

    Some((inner_name, fields))
}

/// Resolves the inner type of a utility type, converting it first if needed (for nesting).
/// Returns `(resolved_name, fields)` or None if no struct fields can be found.
fn resolve_utility_inner_with_conversion(
    type_ref: &swc_ecma_ast::TsTypeRef,
    extra_items: &mut Vec<Item>,
    reg: &TypeRegistry,
) -> Result<Option<ResolvedFields>> {
    // First try direct resolution from registry/extra_items
    if let Some(result) = resolve_utility_inner_fields(type_ref, extra_items, reg) {
        return Ok(Some(result));
    }

    // If not found, convert the inner type (handles nested utility types)
    let params = type_ref
        .type_params
        .as_ref()
        .ok_or_else(|| anyhow!("utility type requires a type parameter"))?;
    if params.params.is_empty() {
        return Ok(None);
    }
    let converted = convert_ts_type(&params.params[0], extra_items, reg)?;

    // If conversion produced a Named type, look for it in extra_items
    if let RustType::Named { ref name, .. } = converted {
        if let Some(fields) = extra_items.iter().find_map(|item| match item {
            Item::Struct {
                name: n, fields, ..
            } if n == name => Some(
                fields
                    .iter()
                    .map(|f| (f.name.clone(), f.ty.clone()))
                    .collect(),
            ),
            _ => None,
        }) {
            return Ok(Some((name.clone(), fields)));
        }
    }

    Ok(None)
}

/// `Partial<T>` → all fields wrapped in `Option<T>`
fn convert_utility_partial(
    type_ref: &swc_ecma_ast::TsTypeRef,
    extra_items: &mut Vec<Item>,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let Some((inner_name, fields)) =
        resolve_utility_inner_with_conversion(type_ref, extra_items, reg)?
    else {
        // Fallback: return inner type as-is
        let params = type_ref
            .type_params
            .as_ref()
            .ok_or_else(|| anyhow!("Partial requires a type parameter"))?;
        return convert_ts_type(&params.params[0], extra_items, reg);
    };

    let synth_name = format!("Partial{inner_name}");
    let synth_fields = fields
        .into_iter()
        .map(|(name, ty)| StructField {
            vis: None,
            name,
            ty: if matches!(ty, RustType::Option(_)) {
                ty
            } else {
                RustType::Option(Box::new(ty))
            },
        })
        .collect();

    extra_items.push(Item::Struct {
        name: synth_name.clone(),
        vis: Visibility::Public,
        fields: synth_fields,
        type_params: vec![],
    });

    Ok(RustType::Named {
        name: synth_name,
        type_args: vec![],
    })
}

/// `Required<T>` → all `Option` wrappers removed from fields
fn convert_utility_required(
    type_ref: &swc_ecma_ast::TsTypeRef,
    extra_items: &mut Vec<Item>,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let Some((inner_name, fields)) =
        resolve_utility_inner_with_conversion(type_ref, extra_items, reg)?
    else {
        let params = type_ref
            .type_params
            .as_ref()
            .ok_or_else(|| anyhow!("Required requires a type parameter"))?;
        return convert_ts_type(&params.params[0], extra_items, reg);
    };

    let synth_name = format!("Required{inner_name}");
    let synth_fields = fields
        .into_iter()
        .map(|(name, ty)| StructField {
            vis: None,
            name,
            ty: match ty {
                RustType::Option(inner) => *inner,
                other => other,
            },
        })
        .collect();

    extra_items.push(Item::Struct {
        name: synth_name.clone(),
        vis: Visibility::Public,
        fields: synth_fields,
        type_params: vec![],
    });

    Ok(RustType::Named {
        name: synth_name,
        type_args: vec![],
    })
}

/// Extracts string literal keys from a union type parameter (e.g., `"x" | "y"`).
fn extract_string_keys(ts_type: &swc_ecma_ast::TsType) -> Vec<String> {
    match ts_type {
        swc_ecma_ast::TsType::TsLitType(lit) => match &lit.lit {
            swc_ecma_ast::TsLit::Str(s) => vec![s.value.to_string_lossy().into_owned()],
            _ => vec![],
        },
        swc_ecma_ast::TsType::TsUnionOrIntersectionType(
            swc_ecma_ast::TsUnionOrIntersectionType::TsUnionType(union),
        ) => union
            .types
            .iter()
            .flat_map(|t| extract_string_keys(t))
            .collect(),
        _ => vec![],
    }
}

/// `Pick<T, K>` → only fields whose names are in K
fn convert_utility_pick(
    type_ref: &swc_ecma_ast::TsTypeRef,
    extra_items: &mut Vec<Item>,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let params = type_ref
        .type_params
        .as_ref()
        .ok_or_else(|| anyhow!("Pick requires type parameters"))?;
    if params.params.len() < 2 {
        return Err(anyhow!("Pick expects at least two type parameters"));
    }

    let Some((inner_name, fields)) = resolve_utility_inner_fields(type_ref, extra_items, reg)
    else {
        return convert_ts_type(&params.params[0], extra_items, reg);
    };

    let keys = extract_string_keys(&params.params[1]);
    let picked_fields: Vec<StructField> = fields
        .into_iter()
        .filter(|(name, _)| keys.contains(name))
        .map(|(name, ty)| StructField {
            vis: None,
            name,
            ty,
        })
        .collect();

    let keys_suffix = keys.iter().map(|k| capitalize_first(k)).collect::<String>();
    let synth_name = format!("Pick{inner_name}{keys_suffix}");

    extra_items.push(Item::Struct {
        name: synth_name.clone(),
        vis: Visibility::Public,
        fields: picked_fields,
        type_params: vec![],
    });

    Ok(RustType::Named {
        name: synth_name,
        type_args: vec![],
    })
}

/// `Omit<T, K>` → all fields except those in K
fn convert_utility_omit(
    type_ref: &swc_ecma_ast::TsTypeRef,
    extra_items: &mut Vec<Item>,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let params = type_ref
        .type_params
        .as_ref()
        .ok_or_else(|| anyhow!("Omit requires type parameters"))?;
    if params.params.len() < 2 {
        return Err(anyhow!("Omit expects at least two type parameters"));
    }

    let Some((inner_name, fields)) = resolve_utility_inner_fields(type_ref, extra_items, reg)
    else {
        return convert_ts_type(&params.params[0], extra_items, reg);
    };

    let keys = extract_string_keys(&params.params[1]);
    let omitted_fields: Vec<StructField> = fields
        .into_iter()
        .filter(|(name, _)| !keys.contains(name))
        .map(|(name, ty)| StructField {
            vis: None,
            name,
            ty,
        })
        .collect();

    let keys_suffix = keys.iter().map(|k| capitalize_first(k)).collect::<String>();
    let synth_name = format!("Omit{inner_name}{keys_suffix}");

    extra_items.push(Item::Struct {
        name: synth_name.clone(),
        vis: Visibility::Public,
        fields: omitted_fields,
        type_params: vec![],
    });

    Ok(RustType::Named {
        name: synth_name,
        type_args: vec![],
    })
}

/// `NonNullable<T>` → strip `Option` wrapper or remove null/undefined from union
fn convert_utility_non_nullable(
    type_ref: &swc_ecma_ast::TsTypeRef,
    extra_items: &mut Vec<Item>,
    reg: &TypeRegistry,
) -> Result<RustType> {
    let params = type_ref
        .type_params
        .as_ref()
        .ok_or_else(|| anyhow!("NonNullable requires a type parameter"))?;
    if params.params.len() != 1 {
        return Err(anyhow!("NonNullable expects exactly one type parameter"));
    }

    let inner = convert_ts_type(&params.params[0], extra_items, reg)?;
    // Strip Option wrapper
    Ok(match inner {
        RustType::Option(inner_ty) => *inner_ty,
        other => other,
    })
}

/// Capitalizes the first character of a string.
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests;
