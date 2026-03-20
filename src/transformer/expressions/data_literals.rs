//! Data literal conversions: object literals, array literals, and spread arrays.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{Expr, RustType, Stmt};
use crate::registry::{TypeDef, TypeRegistry};
use crate::transformer::TypeEnv;

use super::convert_expr;

/// Converts a discriminated union object literal to an enum variant construction.
///
/// Identifies the discriminant field value to determine the correct variant,
/// then builds the variant with remaining fields (excluding the discriminant).
pub(super) fn convert_discriminated_union_object_lit(
    obj_lit: &ast::ObjectLit,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
    enum_name: &str,
    tag_field: &str,
    string_values: &std::collections::HashMap<String, String>,
    variant_fields_map: &std::collections::HashMap<String, Vec<(String, RustType)>>,
) -> Result<Expr> {
    // Find the discriminant field value
    let mut disc_value = None;
    for prop in &obj_lit.props {
        if let ast::PropOrSpread::Prop(prop) = prop {
            if let ast::Prop::KeyValue(kv) = prop.as_ref() {
                let key = match &kv.key {
                    ast::PropName::Ident(ident) => ident.sym.to_string(),
                    ast::PropName::Str(s) => s.value.to_string_lossy().into_owned(),
                    _ => continue,
                };
                if key == tag_field {
                    if let ast::Expr::Lit(ast::Lit::Str(s)) = kv.value.as_ref() {
                        disc_value = Some(s.value.to_string_lossy().into_owned());
                    }
                }
            }
        }
    }

    let disc_value = disc_value.ok_or_else(|| {
        anyhow!("discriminated union object literal missing discriminant field '{tag_field}'")
    })?;

    let variant_name = string_values.get(&disc_value).ok_or_else(|| {
        anyhow!("unknown discriminant value '{disc_value}' for enum '{enum_name}'")
    })?;

    let variant_field_types = variant_fields_map.get(variant_name);

    // Build fields (excluding the discriminant field)
    let mut fields = Vec::new();
    for prop in &obj_lit.props {
        if let ast::PropOrSpread::Prop(prop) = prop {
            match prop.as_ref() {
                ast::Prop::KeyValue(kv) => {
                    let key = match &kv.key {
                        ast::PropName::Ident(ident) => ident.sym.to_string(),
                        ast::PropName::Str(s) => s.value.to_string_lossy().into_owned(),
                        _ => continue,
                    };
                    if key == tag_field {
                        continue; // Skip discriminant field
                    }
                    let field_expected = variant_field_types
                        .and_then(|fs| fs.iter().find(|(n, _)| n == &key).map(|(_, ty)| ty));
                    let value = convert_expr(&kv.value, reg, field_expected, type_env)?;
                    fields.push((key, value));
                }
                ast::Prop::Shorthand(ident) => {
                    let key = ident.sym.to_string();
                    if key == tag_field {
                        continue;
                    }
                    let field_expected = variant_field_types
                        .and_then(|fs| fs.iter().find(|(n, _)| n == &key).map(|(_, ty)| ty));
                    let value = convert_expr(
                        &ast::Expr::Ident(ident.clone()),
                        reg,
                        field_expected,
                        type_env,
                    )?;
                    fields.push((key, value));
                }
                _ => {}
            }
        }
    }

    let full_name = format!("{enum_name}::{variant_name}");

    // Unit variant (no fields) → Ident
    if fields.is_empty() {
        return Ok(Expr::Ident(full_name));
    }

    Ok(Expr::StructInit {
        name: full_name,
        fields,
        base: None,
    })
}

/// Tries to convert an object literal with all computed keys to a `HashMap::from(...)`.
///
/// Returns `Ok(Some(expr))` if all properties use computed keys, `Ok(None)` if not
/// (mixed or no computed keys), or `Err` if a computed key value fails to convert.
pub(super) fn try_convert_as_hashmap(
    obj_lit: &ast::ObjectLit,
    reg: &TypeRegistry,
    type_env: &TypeEnv,
) -> Result<Option<Expr>> {
    if obj_lit.props.is_empty() {
        return Ok(None);
    }

    // Check that ALL properties are key-value pairs with computed keys
    let mut entries = Vec::new();
    for prop in &obj_lit.props {
        match prop {
            ast::PropOrSpread::Prop(prop) => match prop.as_ref() {
                ast::Prop::KeyValue(kv) => {
                    let computed_expr = match &kv.key {
                        ast::PropName::Computed(c) => &c.expr,
                        _ => return Ok(None), // non-computed key → not a HashMap
                    };
                    let key = convert_expr(computed_expr, reg, None, type_env)?;
                    let value = convert_expr(&kv.value, reg, None, type_env)?;
                    entries.push(Expr::Tuple {
                        elements: vec![key, value],
                    });
                }
                _ => return Ok(None),
            },
            ast::PropOrSpread::Spread(_) => return Ok(None),
        }
    }

    Ok(Some(Expr::FnCall {
        name: "HashMap::from".to_string(),
        args: vec![Expr::Vec { elements: entries }],
    }))
}

/// Converts an SWC object literal to an IR `Expr::StructInit`.
///
/// Requires an expected type (`RustType::Named`) from the enclosing context (e.g., a variable
/// declaration's type annotation). Without a named type, returns an error because Rust requires
/// a named struct.
///
/// `{ x: 1, y: 2 }` with expected `RustType::Named { name: "Point" }` →
/// `Expr::StructInit { name: "Point", fields: [...] }`
pub(super) fn convert_object_lit(
    obj_lit: &ast::ObjectLit,
    reg: &TypeRegistry,
    expected: Option<&RustType>,
    type_env: &TypeEnv,
) -> Result<Expr> {
    // Check if all properties use computed keys → HashMap::from(vec![(k, v), ...])
    if let Some(hashmap) = try_convert_as_hashmap(obj_lit, reg, type_env)? {
        return Ok(hashmap);
    }

    let struct_name = match expected {
        Some(RustType::Named { name, .. }) => name.as_str(),
        _ => {
            return Err(anyhow!(
                "object literal requires a type annotation to determine struct name"
            ))
        }
    };

    // Check if this is a discriminated union enum
    if let Some(TypeDef::Enum {
        tag_field: Some(tag),
        string_values,
        variant_fields,
        ..
    }) = reg.get(struct_name)
    {
        return convert_discriminated_union_object_lit(
            obj_lit,
            reg,
            type_env,
            struct_name,
            tag,
            string_values,
            variant_fields,
        );
    }

    // Look up field types from the registry to propagate expected types to nested values
    let struct_fields = reg.get(struct_name).and_then(|def| match def {
        TypeDef::Struct { fields, .. } => Some(fields.as_slice()),
        _ => None,
    });

    let mut fields = Vec::new();
    let mut spreads: Vec<Expr> = Vec::new();

    for prop in &obj_lit.props {
        match prop {
            ast::PropOrSpread::Prop(prop) => match prop.as_ref() {
                ast::Prop::KeyValue(kv) => {
                    let key = match &kv.key {
                        ast::PropName::Ident(ident) => ident.sym.to_string(),
                        ast::PropName::Str(s) => s.value.to_string_lossy().into_owned(),
                        _ => return Err(anyhow!("unsupported object literal key")),
                    };
                    // Resolve the expected type for this field from the registry
                    let field_expected = struct_fields
                        .and_then(|fs| fs.iter().find(|(name, _)| name == &key).map(|(_, ty)| ty));
                    let value = convert_expr(&kv.value, reg, field_expected, type_env)?;
                    fields.push((key, value));
                }
                ast::Prop::Shorthand(ident) => {
                    let key = ident.sym.to_string();
                    let field_expected = struct_fields
                        .and_then(|fs| fs.iter().find(|(name, _)| name == &key).map(|(_, ty)| ty));
                    let value = convert_expr(
                        &ast::Expr::Ident(ident.clone()),
                        reg,
                        field_expected,
                        type_env,
                    )?;
                    fields.push((key, value));
                }
                _ => {
                    return Err(anyhow!(
                        "unsupported object literal property (only key-value pairs and shorthand)"
                    ))
                }
            },
            ast::PropOrSpread::Spread(spread_elem) => {
                let spread_expr = convert_expr(&spread_elem.expr, reg, None, type_env)?;
                spreads.push(spread_expr);
            }
        }
    }

    // Resolve spreads into field expansion + optional struct update base
    let struct_update_base = if spreads.is_empty() {
        None
    } else if spreads.len() == 1 && struct_fields.is_some() {
        // Single spread + TypeRegistry registered → expand fields (preserves type propagation)
        let base_expr = &spreads[0];
        let all_fields = struct_fields.unwrap();
        let explicit_keys: Vec<String> = fields.iter().map(|(k, _)| k.clone()).collect();
        for (field_name, _) in all_fields {
            if !explicit_keys.iter().any(|k| k == field_name) {
                fields.push((
                    field_name.clone(),
                    Expr::FieldAccess {
                        object: Box::new(base_expr.clone()),
                        field: field_name.clone(),
                    },
                ));
            }
        }
        None
    } else if spreads.len() == 1 {
        // Single spread + TypeRegistry unregistered → struct update syntax
        Some(Box::new(spreads.into_iter().next().unwrap()))
    } else {
        // Multiple spreads: expand all but last via TypeRegistry, last becomes base
        let (earlier, last) = spreads.split_at(spreads.len() - 1);
        if let Some(all_fields) = struct_fields {
            let explicit_keys: Vec<String> = fields.iter().map(|(k, _)| k.clone()).collect();
            for spread_expr in earlier {
                for (field_name, _) in all_fields {
                    if !explicit_keys.iter().any(|k| k == field_name)
                        && !fields.iter().any(|(k, _)| k == field_name)
                    {
                        fields.push((
                            field_name.clone(),
                            Expr::FieldAccess {
                                object: Box::new(spread_expr.clone()),
                                field: field_name.clone(),
                            },
                        ));
                    }
                }
            }
        } else {
            return Err(anyhow!(
                "multiple spreads with unregistered type '{}' — TypeRegistry required for field expansion",
                struct_name
            ));
        }
        Some(Box::new(last[0].clone()))
    };

    // Auto-fill omitted Option<T> fields with None (when no struct update base)
    if struct_update_base.is_none() {
        if let Some(all_fields) = struct_fields {
            let explicit_keys: std::collections::HashSet<String> =
                fields.iter().map(|(k, _)| k.clone()).collect();
            for (field_name, field_ty) in all_fields {
                if !explicit_keys.contains(field_name) && matches!(field_ty, RustType::Option(_)) {
                    fields.push((field_name.clone(), Expr::Ident("None".to_string())));
                }
            }
        }
    }

    Ok(Expr::StructInit {
        name: struct_name.to_string(),
        fields,
        base: struct_update_base,
    })
}

/// Converts an SWC array literal to an IR `Expr::Vec` or `Expr::VecSpread`.
///
/// When `expected` is `RustType::Vec(inner)`, the inner type is propagated to each element.
///
/// Spread arrays (`[...arr, 1]`) are handled at the statement level by `try_expand_spread_*`
/// in `convert_stmt`, so only non-spread arrays should reach here. If a spread array reaches
/// here (e.g., nested in a function call argument), an error is returned.
pub(super) fn convert_array_lit(
    array_lit: &ast::ArrayLit,
    reg: &TypeRegistry,
    expected: Option<&RustType>,
    type_env: &TypeEnv,
) -> Result<Expr> {
    let has_spread = array_lit
        .elems
        .iter()
        .filter_map(|e| e.as_ref())
        .any(|e| e.spread.is_some());

    // When expected is a Tuple type, convert to Expr::Tuple
    if let Some(RustType::Tuple(tuple_types)) = expected {
        let elements = array_lit
            .elems
            .iter()
            .filter_map(|elem| elem.as_ref())
            .enumerate()
            .map(|(i, elem)| {
                let elem_expected = tuple_types.get(i);
                convert_expr(&elem.expr, reg, elem_expected, type_env)
            })
            .collect::<Result<Vec<_>>>()?;
        return Ok(Expr::Tuple { elements });
    }

    let element_type = match expected {
        Some(RustType::Vec(inner)) => Some(inner.as_ref()),
        _ => None,
    };

    if has_spread {
        return convert_spread_array_to_block(array_lit, reg, element_type, type_env);
    }

    let elements = array_lit
        .elems
        .iter()
        .filter_map(|elem| elem.as_ref())
        .map(|elem| convert_expr(&elem.expr, reg, element_type, type_env))
        .collect::<Result<Vec<_>>>()?;
    Ok(Expr::Vec { elements })
}

/// Converts a spread array literal to an `Expr::Block` that builds the vec at runtime.
///
/// `[1, ...arr, 2]` becomes:
/// ```text
/// {
///     let mut _v = vec![1.0];
///     _v.extend(arr.iter().cloned());
///     _v.push(2.0);
///     _v
/// }
/// ```
pub(super) fn convert_spread_array_to_block(
    array_lit: &ast::ArrayLit,
    reg: &TypeRegistry,
    element_type: Option<&RustType>,
    type_env: &TypeEnv,
) -> Result<Expr> {
    let mut stmts: Vec<Stmt> = Vec::new();

    // Collect initial non-spread elements for vec![...] initialization
    let mut init_elements: Vec<Expr> = Vec::new();
    let mut initialized = false;

    for elem_opt in &array_lit.elems {
        let elem = match elem_opt {
            Some(e) => e,
            None => continue,
        };

        if elem.spread.is_some() {
            // Emit initialization if not yet done
            if !initialized {
                stmts.push(Stmt::Let {
                    mutable: true,
                    name: "_v".to_string(),
                    ty: None,
                    init: Some(Expr::Vec {
                        elements: std::mem::take(&mut init_elements),
                    }),
                });
                initialized = true;
            }
            // _v.extend(arr.iter().cloned())
            let spread_expr = convert_expr(&elem.expr, reg, None, type_env)?;
            stmts.push(Stmt::Expr(Expr::MethodCall {
                object: Box::new(Expr::Ident("_v".to_string())),
                method: "extend".to_string(),
                args: vec![Expr::MethodCall {
                    object: Box::new(Expr::MethodCall {
                        object: Box::new(spread_expr),
                        method: "iter".to_string(),
                        args: vec![],
                    }),
                    method: "cloned".to_string(),
                    args: vec![],
                }],
            }));
        } else {
            let value = convert_expr(&elem.expr, reg, element_type, type_env)?;
            if initialized {
                // _v.push(value)
                stmts.push(Stmt::Expr(Expr::MethodCall {
                    object: Box::new(Expr::Ident("_v".to_string())),
                    method: "push".to_string(),
                    args: vec![value],
                }));
            } else {
                init_elements.push(value);
            }
        }
    }

    // If no spread was encountered (shouldn't happen), fall back
    if !initialized {
        return Ok(Expr::Vec {
            elements: init_elements,
        });
    }

    stmts.push(Stmt::TailExpr(Expr::Ident("_v".to_string())));
    Ok(Expr::Block(stmts))
}
