use super::*;

/// Converts a [`TsTypeAliasDecl`] into one or more IR items.
///
/// Most type aliases produce a single item. Conditional type fallbacks produce
/// a `Comment` item followed by a placeholder `TypeAlias`.
pub fn convert_type_alias_items(
    decl: &TsTypeAliasDecl,
    vis: Visibility,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<Vec<Item>> {
    // Conditional type may produce multiple items (comment + placeholder)
    if let TsType::TsConditionalType(cond) = decl.type_ann.as_ref() {
        let name = sanitize_rust_type_name(&decl.id.sym);
        let tp_names: Vec<String> = decl
            .type_params
            .as_ref()
            .map(|tp| tp.params.iter().map(|p| p.name.sym.to_string()).collect())
            .unwrap_or_default();
        let prev_scope = synthetic.push_type_param_scope(tp_names);
        let (type_params, mono_subs) =
            extract_type_params(decl.type_params.as_deref(), synthetic, reg);

        let result = match convert_conditional_type(cond, synthetic, reg) {
            Ok(ty) => {
                let ty = ty.substitute(&mono_subs);
                let used_params = type_params
                    .into_iter()
                    .filter(|p| ty.uses_param(&p.name))
                    .collect();
                Ok(vec![Item::TypeAlias {
                    vis,
                    name,
                    type_params: used_params,
                    ty,
                }])
            }
            Err(_) => {
                let fallback_ty =
                    convert_ts_type(&cond.true_type, synthetic, reg).unwrap_or(RustType::Any);
                let fallback_ty = fallback_ty.substitute(&mono_subs);
                let used_params = type_params
                    .into_iter()
                    .filter(|p| fallback_ty.uses_param(&p.name))
                    .collect();
                let comment =
                    format!("TODO: Conditional type not auto-converted\nOriginal TS: type {name}",);
                Ok(vec![
                    Item::Comment(comment),
                    Item::TypeAlias {
                        vis,
                        name,
                        type_params: used_params,
                        ty: fallback_ty,
                    },
                ])
            }
        };

        synthetic.restore_type_param_scope(prev_scope);
        return result;
    }

    // keyof typeof X → string literal union enum from struct fields
    if let Some(items) = try_convert_keyof_typeof_alias(decl, vis, reg)? {
        return Ok(items);
    }

    let item = convert_type_alias(decl, vis, synthetic, reg)?;
    Ok(vec![item])
}

/// Tries to convert `type X = keyof typeof Y` to a string literal union enum.
///
/// Returns `Ok(Some(items))` if the type alias is `keyof typeof <name>` and the name
/// is found in the registry as a struct. Returns `Ok(None)` otherwise.
fn try_convert_keyof_typeof_alias(
    decl: &TsTypeAliasDecl,
    vis: Visibility,
    reg: &TypeRegistry,
) -> Result<Option<Vec<Item>>> {
    // Match: TsTypeOperator(KeyOf, TsTypeQuery(ident))
    let op = match decl.type_ann.as_ref() {
        TsType::TsTypeOperator(op) if op.op == swc_ecma_ast::TsTypeOperatorOp::KeyOf => op,
        _ => return Ok(None),
    };
    let query = match op.type_ann.as_ref() {
        TsType::TsTypeQuery(q) => q,
        _ => return Ok(None),
    };
    let type_name = match &query.expr_name {
        swc_ecma_ast::TsTypeQueryExpr::TsEntityName(swc_ecma_ast::TsEntityName::Ident(ident)) => {
            ident.sym.to_string()
        }
        _ => return Ok(None),
    };
    let fields = match reg.get(&type_name) {
        Some(crate::registry::TypeDef::Struct { fields, .. }) => fields,
        Some(crate::registry::TypeDef::Enum { string_values, .. }) => {
            // For enums, use variant string values as keys
            let name = sanitize_rust_type_name(&decl.id.sym);
            let variants = string_values
                .values()
                .map(|v| EnumVariant {
                    name: v.clone(),
                    value: None,
                    data: None,
                    fields: vec![],
                })
                .collect();
            return Ok(Some(vec![Item::Enum {
                vis,
                name,
                type_params: vec![],
                serde_tag: None,
                variants,
            }]));
        }
        _ => return Ok(None),
    };

    let name = sanitize_rust_type_name(&decl.id.sym);
    let variants = fields
        .iter()
        .map(|field| EnumVariant {
            name: field.name.clone(),
            value: Some(EnumValue::Str(field.name.clone())),
            data: None,
            fields: vec![],
        })
        .collect();

    Ok(Some(vec![Item::Enum {
        vis,
        name,
        type_params: vec![],
        serde_tag: None,
        variants,
    }]))
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
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<Item> {
    let name = sanitize_rust_type_name(&decl.id.sym);

    // String literal union: `type X = "a" | "b" | "c"` → enum
    // スコープ設定不要: string literal は型パラメータを参照しない
    if let Some(item) = try_convert_string_literal_union(decl, vis)? {
        return Ok(item);
    }

    // Single string literal: `type X = "only"` → enum with one variant
    if let Some(item) = try_convert_single_string_literal(decl, vis)? {
        return Ok(item);
    }

    // 以降のパスは convert_ts_type → register_union を呼ぶ可能性があるため
    // 型パラメータスコープを設定してから実行する
    let tp_names: Vec<String> = decl
        .type_params
        .as_ref()
        .map(|tp| tp.params.iter().map(|p| p.name.sym.to_string()).collect())
        .unwrap_or_default();
    let prev_scope = synthetic.push_type_param_scope(tp_names);

    let result = convert_type_alias_with_scope(decl, vis, &name, synthetic, reg);

    synthetic.restore_type_param_scope(prev_scope);

    result
}

/// Inner implementation for type alias conversion (with type_param_scope already set).
///
/// Covers discriminated union, general union, intersection, function, tuple, and
/// fallback type alias paths. String literal paths are handled before scope setup.
fn convert_type_alias_with_scope(
    decl: &TsTypeAliasDecl,
    vis: Visibility,
    name: &str,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<Item> {
    // Discriminated union: `type X = { kind: "a", ... } | { kind: "b", ... }` → serde-tagged enum
    if let Some(item) = try_convert_discriminated_union(decl, vis, synthetic, reg)? {
        return Ok(item);
    }

    // General union type: `type X = 200 | 404` or `type X = string | number` → enum
    if let Some(item) = try_convert_general_union(decl, vis, synthetic, reg)? {
        return Ok(item);
    }

    // Intersection type: `type X = { a: T } & { b: U }` → struct with merged fields
    if let Some(item) = try_convert_intersection_type(decl, vis, reg, synthetic)? {
        return Ok(item);
    }

    // Function type: `type Fn = (x: T) => U` → type alias
    if let Some(item) = try_convert_function_type_alias(decl, vis, synthetic, reg)? {
        return Ok(item);
    }

    // Tuple type: `type Pair = [string, number]` → type alias
    if let Some(item) = try_convert_tuple_type_alias(decl, vis, synthetic, reg)? {
        return Ok(item);
    }

    convert_type_alias_fallback(decl, vis, name, synthetic, reg)
}

/// Fallback type alias body conversion (type literal or generic type).
fn convert_type_alias_fallback(
    decl: &TsTypeAliasDecl,
    vis: Visibility,
    name: &str,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<Item> {
    match decl.type_ann.as_ref() {
        TsType::TsTypeLit(_) => {
            use crate::ts_type_info::resolve::intersection::{
                resolve_method_info, resolve_type_literal_fields,
            };
            use crate::ts_type_info::resolve::resolve_ts_type;

            // SWC TsTypeLit → TsTypeInfo::TypeLiteral(TsTypeLiteralInfo) に変換
            let lit_info =
                match crate::ts_type_info::convert_to_ts_type_info(decl.type_ann.as_ref())? {
                    crate::ts_type_info::TsTypeInfo::TypeLiteral(info) => info,
                    _ => unreachable!("TsTypeLit should convert to TsTypeInfo::TypeLiteral"),
                };

            let has_methods = !lit_info.methods.is_empty();
            let has_properties = !lit_info.fields.is_empty();
            let has_call_signatures = !lit_info.call_signatures.is_empty();
            let has_construct_signatures = !lit_info.construct_signatures.is_empty();

            // Construct signatures → unsupported
            if has_construct_signatures {
                return Err(anyhow!(
                    "unsupported type literal member: construct signature"
                ));
            }

            // Call signatures only → function type alias
            if has_call_signatures && !has_methods && !has_properties {
                // Pick the signature with the most parameters (overload resolution)
                let sig = lit_info
                    .call_signatures
                    .iter()
                    .max_by_key(|s| s.params.len())
                    .ok_or_else(|| anyhow!("no call signatures found"))?;
                let param_types = sig
                    .params
                    .iter()
                    .map(|p| resolve_ts_type(&p.ty, reg, synthetic))
                    .collect::<Result<Vec<_>>>()?;
                let return_type = sig
                    .return_type
                    .as_ref()
                    .map(|rt| resolve_ts_type(rt, reg, synthetic))
                    .transpose()?
                    .unwrap_or(RustType::Unit);
                let (type_params, mono_subs) =
                    extract_type_params(decl.type_params.as_deref(), synthetic, reg);
                let fn_ty = RustType::Fn {
                    params: param_types,
                    return_type: Box::new(return_type),
                };
                return Ok(Item::TypeAlias {
                    vis,
                    name: name.to_string(),
                    type_params,
                    ty: fn_ty.substitute(&mono_subs),
                });
            }

            // Methods only → trait (same logic as interface 3-way classification)
            if has_methods && !has_properties {
                let methods = lit_info
                    .methods
                    .iter()
                    .map(|m| resolve_method_info(m, reg, synthetic))
                    .collect::<Result<Vec<_>>>()?;
                return Ok(Item::Trait {
                    vis,
                    name: name.to_string(),
                    type_params: vec![],
                    supertraits: vec![],
                    methods,
                    associated_types: vec![],
                });
            }

            // Index signature → HashMap (delegate to resolve_type_literal)
            if let Some(idx) = lit_info.index_signatures.first() {
                let value_type = resolve_ts_type(&idx.value_type, reg, synthetic)?;
                let (type_params, mono_subs) =
                    extract_type_params(decl.type_params.as_deref(), synthetic, reg);
                return Ok(Item::TypeAlias {
                    vis,
                    name: name.to_string(),
                    ty: RustType::Named {
                        name: "HashMap".to_string(),
                        type_args: vec![RustType::String, value_type.substitute(&mono_subs)],
                    },
                    type_params,
                });
            }

            // Call signatures mixed with properties → unsupported
            // (Rust structs cannot be both callable and have fields)
            if has_call_signatures {
                return Err(anyhow!(
                    "unsupported type literal member: call signature mixed with properties"
                ));
            }

            // Properties (with possible mixed methods — methods are skipped in structs)
            let fields = resolve_type_literal_fields(&lit_info, reg, synthetic)?;
            let (type_params, mono_subs) =
                extract_type_params(decl.type_params.as_deref(), synthetic, reg);

            Ok(Item::Struct {
                vis,
                name: name.to_string(),
                type_params,
                fields: fields.iter().map(|f| f.substitute(&mono_subs)).collect(),
            })
        }
        // Fallback: any type that convert_ts_type can handle → type alias
        other => {
            let ty = convert_ts_type(other, synthetic, reg).map_err(|_| {
                anyhow!(
                    "unsupported type alias body: {:?}",
                    std::mem::discriminant(other)
                )
            })?;
            let (type_params, mono_subs) =
                extract_type_params(decl.type_params.as_deref(), synthetic, reg);
            Ok(Item::TypeAlias {
                vis,
                name: name.to_string(),
                ty: ty.substitute(&mono_subs),
                type_params,
            })
        }
    }
}

/// Tries to convert a type alias with a function type body.
///
/// Returns `Ok(Some(Item::TypeAlias))` if the body is a `TsFnType`, `Ok(None)` otherwise.
pub(super) fn try_convert_function_type_alias(
    decl: &TsTypeAliasDecl,
    vis: Visibility,
    synthetic: &mut SyntheticTypeRegistry,
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
                    .map(|ann| convert_ts_type(&ann.type_ann, synthetic, reg))
                    .transpose()?
                    .unwrap_or(RustType::Any);
                param_types.push(ty);
            }
            _ => return Err(anyhow!("unsupported function type parameter pattern")),
        }
    }

    let return_type = convert_ts_type(&fn_type.type_ann.type_ann, synthetic, reg)?;

    let name = sanitize_rust_type_name(&decl.id.sym);
    let (type_params, mono_subs) = extract_type_params(decl.type_params.as_deref(), synthetic, reg);

    let fn_ty = RustType::Fn {
        params: param_types,
        return_type: Box::new(return_type),
    };
    Ok(Some(Item::TypeAlias {
        vis,
        name,
        type_params,
        ty: fn_ty.substitute(&mono_subs),
    }))
}

/// Tries to convert a type alias with a tuple type body.
///
/// Returns `Ok(Some(Item::TypeAlias))` if the body is a `TsTupleType`, `Ok(None)` otherwise.
pub(super) fn try_convert_tuple_type_alias(
    decl: &TsTypeAliasDecl,
    vis: Visibility,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<Option<Item>> {
    let tuple = match decl.type_ann.as_ref() {
        TsType::TsTupleType(t) => t,
        _ => return Ok(None),
    };

    let elems = tuple
        .elem_types
        .iter()
        .map(|elem| convert_ts_type(&elem.ty, synthetic, reg))
        .collect::<Result<Vec<_>>>()?;

    let name = sanitize_rust_type_name(&decl.id.sym);
    let (type_params, mono_subs) = extract_type_params(decl.type_params.as_deref(), synthetic, reg);

    Ok(Some(Item::TypeAlias {
        vis,
        name,
        type_params,
        ty: RustType::Tuple(elems).substitute(&mono_subs),
    }))
}

/// Converts a conditional type expression to a [`RustType`].
///
/// Detects patterns and converts accordingly:
/// - `infer` extraction: `T extends Foo<infer U> ? U : never` → `<T as Foo>::Output`
/// - Type predicate (`true`/`false` branches): `T extends X ? true : false` → `bool`
/// - Other patterns: returns the true branch type
pub(super) fn convert_conditional_type(
    cond: &swc_ecma_ast::TsConditionalType,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<RustType> {
    // Pattern: infer extraction — `T extends Foo<infer U> ? U : never`
    if let Some((ty, trait_name)) = try_convert_infer_pattern(cond)? {
        // Generate a stub trait for the container (e.g., `pub trait Promise { type Output; }`)
        synthetic.push_item(
            trait_name.clone(),
            crate::pipeline::SyntheticTypeKind::Trait,
            Item::Trait {
                vis: Visibility::Public,
                name: trait_name,
                type_params: vec![],
                supertraits: vec![],
                methods: vec![],
                associated_types: vec!["Output".to_string()],
            },
        );
        return Ok(ty);
    }

    // Pattern: type predicate — `T extends X ? true : false`
    if is_true_false_literal(&cond.true_type, &cond.false_type) {
        return Ok(RustType::Bool);
    }

    // Default: use the true branch type
    convert_ts_type(&cond.true_type, synthetic, reg)
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
