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

/// Converts explicit type arguments from a `TsTypeParamInstantiation` to `Vec<RustType>`.
///
/// Used for `foo<string, number>()` or `new Map<string, number>()` where the
/// caller provides explicit type arguments that should instantiate the callee's
/// type parameters.
pub(super) fn convert_explicit_type_args(
    type_args: &ast::TsTypeParamInstantiation,
    synthetic: &mut SyntheticTypeRegistry,
    registry: &TypeRegistry,
) -> Vec<RustType> {
    type_args
        .params
        .iter()
        .filter_map(|ts_type| convert_ts_type(ts_type, synthetic, registry).ok())
        .collect()
}

/// Builds a type parameter name → concrete type mapping from type definition
/// type params and explicit type arguments.
///
/// Given `class Foo<T, U>` and `new Foo<string, number>()`, produces
/// `{"T": String, "U": F64}`. Extra type args without corresponding params
/// are ignored; missing type args are skipped.
pub(super) fn build_type_arg_bindings(
    type_def_params: &[crate::ir::TypeParam],
    explicit_type_args: &[RustType],
) -> HashMap<String, RustType> {
    type_def_params
        .iter()
        .zip(explicit_type_args.iter())
        .map(|(param, arg)| (param.name.clone(), arg.clone()))
        .collect()
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
