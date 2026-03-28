//! Standalone helper functions for TypeResolver.
//!
//! Free functions that don't require `&self` access to TypeResolver. Used by
//! multiple submodules (visitors, expressions, narrowing, expected_types).

use swc_ecma_ast as ast;

use std::collections::HashMap;

use crate::ir::RustType;
use crate::pipeline::type_converter::convert_ts_type;
use crate::pipeline::ResolvedType;
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{TypeDef, TypeRegistry};

/// Returns true if the expression is a `null` literal or `undefined` identifier.
pub(super) fn is_null_or_undefined(expr: &ast::Expr) -> bool {
    matches!(expr, ast::Expr::Lit(ast::Lit::Null(..)))
        || matches!(expr, ast::Expr::Ident(id) if id.sym.as_ref() == "undefined")
}

/// Extracts the property name from an object literal key.
pub(super) fn extract_prop_name(key: &ast::PropName) -> Option<String> {
    match key {
        ast::PropName::Ident(ident) => Some(ident.sym.to_string()),
        ast::PropName::Str(s) => Some(s.value.to_string_lossy().into_owned()),
        _ => None,
    }
}

/// Extracts the string value of a named property from an object literal.
///
/// Used to identify the discriminant value in a discriminated union object literal.
pub(super) fn find_string_prop_value(obj: &ast::ObjectLit, prop_name: &str) -> Option<String> {
    for prop in &obj.props {
        if let ast::PropOrSpread::Prop(prop) = prop {
            if let ast::Prop::KeyValue(kv) = prop.as_ref() {
                let key = extract_prop_name(&kv.key);
                if key.as_deref() == Some(prop_name) {
                    if let ast::Expr::Lit(ast::Lit::Str(s)) = &*kv.value {
                        return Some(s.value.to_string_lossy().into_owned());
                    }
                }
            }
        }
    }
    None
}

/// Returns the common `RustType::Named` type if all types in the slice are identical Named types.
///
/// Used for spread merging: `{ ...a, ...b }` where both `a` and `b` are `CORSOptions`
/// → the result is `CORSOptions` (not an anonymous struct).
///
/// Requires both `name` and `type_args` to match. `Container<f64>` and `Container<String>`
/// are NOT considered the same type and will produce an anonymous struct instead.
pub(super) fn common_named_type(types: &[RustType]) -> Option<RustType> {
    if types.is_empty() {
        return None;
    }
    let first = types.first()?;
    if let RustType::Named {
        name,
        type_args: first_args,
    } = first
    {
        if types.iter().all(|t| {
            matches!(t, RustType::Named { name: n, type_args } if n == name && type_args == first_args)
        }) {
            return Some(first.clone());
        }
    }
    None
}

pub(super) fn is_null_literal(expr: &ast::Expr) -> bool {
    matches!(expr, ast::Expr::Lit(ast::Lit::Null(_)))
}

/// Returns true if the resolved type is an object type (struct, named, vec, etc.).
/// Used for const-mut detection: TypeScript's `const` allows field mutation on objects.
pub(super) fn is_object_type(ty: &ResolvedType) -> bool {
    match ty {
        ResolvedType::Known(rust_type) => matches!(
            rust_type,
            RustType::Named { .. } | RustType::Vec(_) | RustType::Tuple(_) | RustType::Any
        ),
        ResolvedType::Unknown => false,
    }
}

/// RustType から TypeRegistry ルックアップ用の (型名, 型引数) を抽出する。
///
/// `Named`, `String` に加え、trait ラッピング後の `Ref(DynTrait)`,
/// `Box<dyn Trait>` (`Named { name: "Box", type_args: [DynTrait(_)] }`),
/// `DynTrait` も trait 名に展開する。
///
/// Note: `Vec<T>` is NOT handled here. Callers (`lookup_method_sigs`,
/// `resolve_member_type`) handle Vec→Array mapping directly with
/// `registry.instantiate("Array", &[T])` to preserve the element type.
pub(super) fn extract_type_name_for_registry(ty: &RustType) -> Option<(&str, &[RustType])> {
    match ty {
        RustType::String => Some(("String", &[])),
        RustType::Named { name, type_args }
            if name == "Box"
                && type_args.len() == 1
                && matches!(&type_args[0], RustType::DynTrait(_)) =>
        {
            if let RustType::DynTrait(trait_name) = &type_args[0] {
                Some((trait_name.as_str(), &[]))
            } else {
                None
            }
        }
        RustType::Named { name, type_args } => Some((name.as_str(), type_args.as_slice())),
        RustType::Ref(inner) => match inner.as_ref() {
            RustType::DynTrait(name) => Some((name.as_str(), &[])),
            _ => None,
        },
        RustType::DynTrait(name) => Some((name.as_str(), &[])),
        _ => None,
    }
}

/// Selects the best matching overload from a set of method signatures.
///
/// Returns the full `MethodSignature` so callers can extract both parameter types
/// and return type from the **same** signature, avoiding inconsistency.
///
/// Resolution strategy (5 stages):
/// 1. Single signature → use it
/// 2. All signatures have the same return type → use first (selection is irrelevant for return type)
/// 3. Filter by argument count → if exactly one matches, use it
/// 4. Filter by argument type compatibility → if exactly one matches, use it
/// 5. Fallback: first signature
pub(super) fn select_overload<'a>(
    sigs: &'a [crate::registry::MethodSignature],
    arg_count: usize,
    arg_types: &[Option<RustType>],
) -> &'a crate::registry::MethodSignature {
    debug_assert!(
        !sigs.is_empty(),
        "select_overload called with empty signatures"
    );

    // Stage 1: single signature
    if sigs.len() == 1 {
        return &sigs[0];
    }

    // Stage 2: all return types identical — selection doesn't affect return type
    let first_ret = &sigs[0].return_type;
    if sigs.iter().all(|s| &s.return_type == first_ret) {
        return &sigs[0];
    }

    // Stage 3: filter by argument count
    let by_count: Vec<&crate::registry::MethodSignature> = sigs
        .iter()
        .filter(|sig| sig.params.len() == arg_count)
        .collect();
    if by_count.len() == 1 {
        return by_count[0];
    }

    // Stage 4: filter by argument type compatibility
    let candidates: Vec<&crate::registry::MethodSignature> = if by_count.is_empty() {
        sigs.iter().collect()
    } else {
        by_count
    };
    if arg_types.iter().any(|t| t.is_some()) {
        let compatible: Vec<&&crate::registry::MethodSignature> = candidates
            .iter()
            .filter(|sig| {
                sig.params.iter().zip(arg_types.iter()).all(
                    |((_, param_ty), arg_ty)| match arg_ty {
                        Some(at) => at == param_ty,
                        None => true,
                    },
                )
            })
            .collect();
        if compatible.len() == 1 {
            return compatible[0];
        }
    }

    // Stage 5: fallback to first signature
    &sigs[0]
}

/// Extracts type parameter constraints from a `TsTypeParamDecl`.
///
/// For each type parameter with an `extends` constraint, converts the constraint
/// type and adds it to the map. Unconstrained type parameters are skipped.
pub(super) fn collect_type_param_constraints(
    type_params: &ast::TsTypeParamDecl,
    synthetic: &mut SyntheticTypeRegistry,
    registry: &TypeRegistry,
) -> HashMap<String, RustType> {
    let mut constraints = HashMap::new();
    for param in &type_params.params {
        if let Some(constraint) = &param.constraint {
            if let Ok(rust_ty) = convert_ts_type(constraint, synthetic, registry) {
                constraints.insert(param.name.sym.to_string(), rust_ty);
            }
        }
    }
    constraints
}

/// Promise<T> → T に展開し、Unit（void）は None にする。
/// TypeResolver が expected_type / return_type として登録する前に適用する。
pub(super) fn unwrap_promise_and_unit(ty: RustType) -> Option<RustType> {
    let unwrapped = match &ty {
        RustType::Named { name, type_args } if name == "Promise" && type_args.len() == 1 => {
            type_args[0].clone()
        }
        _ => ty,
    };
    if matches!(unwrapped, RustType::Unit) {
        None
    } else {
        Some(unwrapped)
    }
}

/// Extracts function return type and parameter types from an expected type.
///
/// Handles two cases:
/// - `RustType::Fn { return_type, params }` — uses the types directly
/// - `RustType::Named { name }` — looks up TypeRegistry for `TypeDef::Function`
///
/// Used by `resolve_arrow_expr` and `resolve_fn_expr` to infer return type
/// from parent context (e.g., variable type annotation).
pub(super) fn resolve_fn_type_info(
    expected: &RustType,
    registry: &TypeRegistry,
) -> (Option<RustType>, Option<Vec<RustType>>) {
    match expected {
        RustType::Fn {
            return_type,
            params,
        } => {
            let ret = if matches!(return_type.as_ref(), RustType::Unit) {
                None
            } else {
                Some(return_type.as_ref().clone())
            };
            (ret, Some(params.clone()))
        }
        RustType::Named { name, .. } => {
            if let Some(TypeDef::Function {
                return_type,
                params,
                ..
            }) = registry.get(name)
            {
                (
                    return_type.clone(),
                    Some(params.iter().map(|(_, ty)| ty.clone()).collect()),
                )
            } else {
                (None, None)
            }
        }
        _ => (None, None),
    }
}
