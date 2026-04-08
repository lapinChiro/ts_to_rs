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

/// Walks a `RustType` and collects all `Named { name }` / `DynTrait(name)`
/// references where `name` is **not** a known type in `registry` and **not**
/// already in the current `known_scope`.
///
/// I-383 T2.A-iv: such names are **free type variables** — generic parameters
/// inherited from a generic interface call signature (e.g., `SSGParamsMiddleware`'s
/// `<E extends Env>(...)`) that have been flattened into a `RustType::Fn` by
/// `convert_ts_type` but lost their declaring generic context. When such a Fn
/// becomes the expected type of an arrow expression, the arrow's body resolves
/// nested expressions whose synthetic union/struct registrations would otherwise
/// leak these free variables as dangling external refs.
///
/// Pushing the extracted free variables into `synthetic.type_param_scope` for
/// the duration of the init expression's resolution causes the synthetic types
/// I-387: 型変数 (`RustType::TypeVar { name }`) を IR walk で収集する。
///
/// `TypeVar` variant は構造的に型変数を表すため、旧 `collect_free_type_vars`
/// が必要としていた heuristic (registry 未登録 / scope 外 / builtin 名リスト
/// 除外 / path 形式除外) は一切不要になった。本関数は単純な再帰 walker。
///
/// 重複は出力に含めない。`out` は挿入順 (深さ優先順) を保持する。
pub(super) fn collect_type_vars(ty: &RustType, out: &mut Vec<String>) {
    match ty {
        RustType::TypeVar { name } => {
            if !out.contains(name) {
                out.push(name.clone());
            }
        }
        RustType::Named { type_args, .. } => {
            for arg in type_args {
                collect_type_vars(arg, out);
            }
        }
        RustType::StdCollection { args, .. } => {
            for arg in args {
                collect_type_vars(arg, out);
            }
        }
        RustType::Option(inner) | RustType::Vec(inner) | RustType::Ref(inner) => {
            collect_type_vars(inner, out);
        }
        RustType::Result { ok, err } => {
            collect_type_vars(ok, out);
            collect_type_vars(err, out);
        }
        RustType::Tuple(elems) => {
            for elem in elems {
                collect_type_vars(elem, out);
            }
        }
        RustType::Fn {
            params,
            return_type,
        } => {
            for p in params {
                collect_type_vars(p, out);
            }
            collect_type_vars(return_type, out);
        }
        RustType::QSelf {
            qself, trait_ref, ..
        } => {
            collect_type_vars(qself, out);
            for arg in &trait_ref.type_args {
                collect_type_vars(arg, out);
            }
        }
        RustType::Primitive(_)
        | RustType::String
        | RustType::F64
        | RustType::Bool
        | RustType::Unit
        | RustType::Any
        | RustType::Never
        | RustType::DynTrait(_) => {}
    }
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

/// Extracts the element type from an array/tuple source type at a given index.
///
/// - `Vec<T>` → `T` for any index
/// - `Tuple([A, B, ...])` → positional type
/// - `Option<inner>` → unwraps and recurses
pub(super) fn lookup_array_element_type(source_type: &RustType, index: usize) -> Option<RustType> {
    match source_type {
        RustType::Vec(inner) => Some(inner.as_ref().clone()),
        RustType::Tuple(types) => types.get(index).cloned(),
        RustType::Option(inner) => lookup_array_element_type(inner, index),
        _ => None,
    }
}

/// Unwraps `Option<T>` → `T` for a destructuring default value context.
///
/// When a destructuring pattern has a default value (e.g., `{ x = 0 }`),
/// the default replaces `None`, so the variable's type is `T`, not `Option<T>`.
/// Non-Option types pass through unchanged.
pub(super) fn unwrap_option_for_default(ty: RustType) -> RustType {
    match ty {
        RustType::Option(inner) => *inner,
        other => other,
    }
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

/// Enters a type parameter scope: pushes the declared parameter names into
/// `synthetic.type_param_scope` AND extracts each `extends` constraint into a
/// HashMap.
///
/// Returns `(constraints, prev_scope)` where `prev_scope` must be restored by
/// the caller via `synthetic.restore_type_param_scope` when leaving the scope
/// (typically alongside the `type_param_constraints` restore).
///
/// # Lexical scope semantics (I-387)
///
/// `convert_ts_type` references `synthetic.type_param_scope` to route TS type
/// references to either `RustType::TypeVar { name }` (when in scope) or
/// `RustType::Named` (user types). Pushing the scope here makes the names
/// visible during method body / arrow expression resolution, so synthetic
/// union/struct registrations see TypeVar variants and `extract_used_type_params`
/// (TypeVar walker) correctly identifies them as type parameters.
///
/// # Constraint resolution ordering
///
/// Names are pushed to scope **before** constraint conversion so that
/// constraints referencing sibling type params (e.g., `<K, V extends Record<K, string>>`)
/// resolve `K` against the active scope.
pub(super) fn enter_type_param_scope(
    type_params: &ast::TsTypeParamDecl,
    synthetic: &mut SyntheticTypeRegistry,
    registry: &TypeRegistry,
) -> (HashMap<String, RustType>, Vec<String>) {
    let names: Vec<String> = type_params
        .params
        .iter()
        .map(|p| p.name.sym.to_string())
        .collect();
    let prev_scope = synthetic.push_type_param_scope(names);

    let mut constraints = HashMap::new();
    for param in &type_params.params {
        if let Some(constraint) = &param.constraint {
            if let Ok(rust_ty) = convert_ts_type(constraint, synthetic, registry) {
                constraints.insert(param.name.sym.to_string(), rust_ty);
            }
        }
    }
    (constraints, prev_scope)
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
        RustType::Named { name, .. } => match registry.get(name) {
            Some(TypeDef::Function {
                return_type,
                params,
                ..
            }) => (
                return_type.clone(),
                Some(params.iter().map(|p| p.ty.clone()).collect()),
            ),
            Some(TypeDef::Struct {
                call_signatures, ..
            }) if !call_signatures.is_empty() => {
                let sig = crate::registry::select_overload(call_signatures, 0, &[]);
                (
                    sig.return_type.clone(),
                    Some(sig.params.iter().map(|p| p.ty.clone()).collect()),
                )
            }
            _ => (None, None),
        },
        _ => (None, None),
    }
}
