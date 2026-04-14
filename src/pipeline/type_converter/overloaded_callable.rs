//! Widest signature computation for overloaded callable interfaces.
//!
//! Given multiple call signatures (overloads), computes a single "widest"
//! signature whose parameter list is a superset of all overloads (using
//! `Option` wrapping for absent parameters and union types for divergent
//! parameter types at the same position).
//!
//! Used by Phase 4 (trait emission), Phase 5 (inner fn emission),
//! and Phase 7 (return wrap context).

use std::collections::HashMap;

use crate::ir::{RustType, TypeParam};
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{MethodSignature, ParamDef};

/// Widest signature computed from overloaded call signatures.
#[derive(Debug, Clone, PartialEq)]
pub struct WidestSignature {
    /// Parameters of the widest signature. Each parameter's type is the union
    /// of all overloads' types at that position, wrapped in `Option` if some
    /// overloads don't have a parameter at that position.
    pub params: Vec<ParamDef>,
    /// Return type of the widest signature. If all overloads return the same
    /// type, this is that type. If overloads have divergent returns, this is
    /// the synthetic union enum type.
    pub return_type: Option<RustType>,
    /// Whether the return types diverge across overloads (requiring a synthetic
    /// union enum for the inner function's return).
    pub return_diverges: bool,
}

/// Computes the widest signature from a set of overloaded call signatures.
///
/// The widest signature has:
/// - The maximum number of parameters across all overloads
/// - Each parameter's type is the union of types at that position
/// - Parameters absent in some overloads are wrapped in `Option`
/// - Return type is unified (same type) or a synthetic union enum (divergent)
pub fn compute_widest_signature(
    overloads: &[MethodSignature],
    synthetic: &mut SyntheticTypeRegistry,
) -> WidestSignature {
    assert!(!overloads.is_empty(), "overloads must not be empty");

    let params = compute_widest_params(overloads, synthetic);
    let (return_type, return_diverges) = compute_union_return(overloads, synthetic);

    WidestSignature {
        params,
        return_type,
        return_diverges,
    }
}

/// Computes the widest parameter list from overloaded signatures.
///
/// For each parameter position:
/// - Collects all types at that position across overloads
/// - If all types are identical, uses that type directly
/// - If types differ, creates a synthetic union type
/// - If some overloads don't have a parameter at that position, wraps in `Option`
fn compute_widest_params(
    overloads: &[MethodSignature],
    synthetic: &mut SyntheticTypeRegistry,
) -> Vec<ParamDef> {
    let max_arity = overloads.iter().map(|s| s.params.len()).max().unwrap_or(0);
    let mut widest_params = Vec::with_capacity(max_arity);

    for i in 0..max_arity {
        // Collect types at position i from all overloads
        let types_at_pos: Vec<&RustType> = overloads
            .iter()
            .filter_map(|s| s.params.get(i).map(|p| &p.ty))
            .collect();

        // Use the first overload's param name that has this position, or generate a name
        let name = overloads
            .iter()
            .find_map(|s| s.params.get(i).map(|p| p.name.clone()))
            .unwrap_or_else(|| format!("arg{i}"));

        // Determine if this parameter is present in all overloads
        let present_in_all = overloads.iter().all(|s| s.params.len() > i);

        // Determine if this parameter is optional in any overload that has it
        let optional_in_any = overloads
            .iter()
            .any(|s| s.params.get(i).is_some_and(|p| p.optional));

        // Unify types at this position
        let unified_type = unify_types(&types_at_pos, synthetic);

        // Wrap in Option<T> if the param is absent or optional in any overload.
        // `wrap_if_optional` is idempotent, so `T | null` (already Option) doesn't
        // become `Option<Option<T>>`.
        let optional = !present_in_all || optional_in_any;
        let ty = unified_type.wrap_if_optional(optional);

        widest_params.push(ParamDef {
            name,
            ty,
            optional,
            has_default: false,
        });
    }

    widest_params
}

/// Computes the unified return type from overloaded signatures.
///
/// Returns `(return_type, diverges)`:
/// - If all overloads return the same type (after dedup), `diverges = false`
/// - If overloads return different types, a synthetic union enum is registered
///   and `diverges = true`
fn compute_union_return(
    overloads: &[MethodSignature],
    synthetic: &mut SyntheticTypeRegistry,
) -> (Option<RustType>, bool) {
    // Collect return types, treating None (void) as RustType::Unit for uniformity.
    // Promise<T> is unwrapped to T because trait methods already unwrap Promise
    // (async fn -> T), and the union must match the unwrapped return types.
    let return_types: Vec<RustType> = overloads
        .iter()
        .map(|s| {
            s.return_type
                .clone()
                .unwrap_or(RustType::Unit)
                .unwrap_promise()
        })
        .collect();

    // Dedup: if all return types are identical, no divergence
    let unique: Vec<&RustType> = {
        let mut seen = Vec::new();
        for ty in &return_types {
            if !seen.contains(&ty) {
                seen.push(ty);
            }
        }
        seen
    };

    if unique.len() == 1 {
        let ty = unique[0];
        if matches!(ty, RustType::Unit) {
            // All overloads return void
            (None, false)
        } else {
            (Some(ty.clone()), false)
        }
    } else {
        // Divergent returns: create a synthetic union enum
        // Filter out Unit (void) — Rust has no Unit variant in enums
        let member_types: Vec<RustType> = unique
            .into_iter()
            .filter(|ty| !matches!(ty, RustType::Unit))
            .cloned()
            .collect();

        if member_types.is_empty() {
            // All were Unit after filtering (shouldn't happen since unique.len() > 1)
            (None, false)
        } else if member_types.len() == 1 {
            // Only one non-void type → wrap in Option (void overload means "might not return").
            // Use `wrap_optional` for idempotency so an already-Option return (rare) stays single-wrapped.
            (Some(member_types[0].clone().wrap_optional()), false)
        } else {
            let enum_name = synthetic.register_union(&member_types);
            (
                Some(RustType::Named {
                    name: enum_name,
                    type_args: vec![],
                }),
                true,
            )
        }
    }
}

/// Unifies multiple types into a single type.
///
/// - If all types are identical, returns that type
/// - If types differ, creates a synthetic union type
fn unify_types(types: &[&RustType], synthetic: &mut SyntheticTypeRegistry) -> RustType {
    if types.is_empty() {
        return RustType::Any;
    }

    // Dedup
    let unique: Vec<&RustType> = {
        let mut seen: Vec<&RustType> = Vec::new();
        for ty in types {
            if !seen.contains(ty) {
                seen.push(ty);
            }
        }
        seen
    };

    if unique.len() == 1 {
        unique[0].clone()
    } else {
        let member_types: Vec<RustType> = unique.into_iter().cloned().collect();
        let enum_name = synthetic.register_union(&member_types);
        RustType::Named {
            name: enum_name,
            type_args: vec![],
        }
    }
}

/// Fills missing type args with defaults from type params.
///
/// When `type_args` is shorter than `type_params`, fills in default values from
/// `TypeParam::default`. Falls back to `Any` if no default is available.
/// Returns `type_args` unchanged when it already covers all params.
pub fn fill_type_arg_defaults(type_params: &[TypeParam], type_args: &[RustType]) -> Vec<RustType> {
    type_params
        .iter()
        .enumerate()
        .map(|(i, param)| {
            type_args
                .get(i)
                .cloned()
                .or_else(|| param.default.clone())
                .unwrap_or(RustType::Any)
        })
        .collect()
}

/// Applies type parameter → concrete type substitution to a method signature.
///
/// Builds a `{ T → String, U → f64 }` binding map from `type_params` and `type_args`,
/// then substitutes all type variables in the signature's params and return type.
/// Missing type args are filled with defaults via [`fill_type_arg_defaults`].
///
/// Returns the original signature unchanged when `type_params` is empty.
pub fn apply_type_substitution(
    sig: &MethodSignature,
    type_params: &[TypeParam],
    type_args: &[RustType],
) -> MethodSignature {
    if type_params.is_empty() {
        return sig.clone();
    }
    let effective_args = fill_type_arg_defaults(type_params, type_args);
    let bindings: HashMap<String, RustType> = type_params
        .iter()
        .zip(effective_args.iter())
        .map(|(param, arg)| (param.name.clone(), arg.clone()))
        .collect();
    sig.substitute(&bindings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sig(params: Vec<(&str, RustType)>, return_type: Option<RustType>) -> MethodSignature {
        MethodSignature {
            params: params
                .into_iter()
                .map(|(name, ty)| ParamDef {
                    name: name.to_string(),
                    ty,
                    optional: false,
                    has_default: false,
                })
                .collect(),
            return_type,
            ..Default::default()
        }
    }

    fn make_sig_with_optional(
        params: Vec<(&str, RustType, bool)>,
        return_type: Option<RustType>,
    ) -> MethodSignature {
        MethodSignature {
            params: params
                .into_iter()
                .map(|(name, ty, optional)| ParamDef {
                    name: name.to_string(),
                    ty,
                    optional,
                    has_default: false,
                })
                .collect(),
            return_type,
            ..Default::default()
        }
    }

    // --- compute_widest_params tests ---

    #[test]
    fn same_arity_different_types() {
        // (c: string): string
        // (c: string, key: number): number
        // → widest: (c: string, key: Option<number>)
        let sigs = vec![
            make_sig(vec![("c", RustType::String)], Some(RustType::String)),
            make_sig(
                vec![("c", RustType::String), ("key", RustType::F64)],
                Some(RustType::F64),
            ),
        ];
        let mut synthetic = SyntheticTypeRegistry::new();
        let widest = compute_widest_signature(&sigs, &mut synthetic);

        assert_eq!(widest.params.len(), 2);
        assert_eq!(widest.params[0].name, "c");
        assert_eq!(widest.params[0].ty, RustType::String);
        assert!(!widest.params[0].optional);
        assert_eq!(widest.params[1].name, "key");
        assert_eq!(
            widest.params[1].ty,
            RustType::Option(Box::new(RustType::F64))
        );
        assert!(widest.params[1].optional);
    }

    #[test]
    fn different_arity_params_option_wrapped() {
        // (c: Context): Cookie
        // (c: Context, key: string): Option<string>
        // (c: Context, key: string, prefix: PrefixOpts): Option<string>
        let context = RustType::Named {
            name: "Context".to_string(),
            type_args: vec![],
        };
        let cookie = RustType::Named {
            name: "Cookie".to_string(),
            type_args: vec![],
        };
        let sigs = vec![
            make_sig(vec![("c", context.clone())], Some(cookie.clone())),
            make_sig(
                vec![("c", context.clone()), ("key", RustType::String)],
                Some(RustType::Option(Box::new(RustType::String))),
            ),
            make_sig_with_optional(
                vec![
                    ("c", context.clone(), false),
                    ("key", RustType::String, false),
                    (
                        "prefix",
                        RustType::Named {
                            name: "PrefixOpts".to_string(),
                            type_args: vec![],
                        },
                        true,
                    ),
                ],
                Some(RustType::Option(Box::new(RustType::String))),
            ),
        ];
        let mut synthetic = SyntheticTypeRegistry::new();
        let widest = compute_widest_signature(&sigs, &mut synthetic);

        assert_eq!(widest.params.len(), 3);
        // c: present in all → not Option
        assert_eq!(widest.params[0].ty, context);
        assert!(!widest.params[0].optional);
        // key: absent in overload 1 → Option<String>
        assert_eq!(
            widest.params[1].ty,
            RustType::Option(Box::new(RustType::String))
        );
        assert!(widest.params[1].optional);
        // prefix: absent in overloads 1,2 + optional in overload 3 → Option<PrefixOpts>
        assert!(
            matches!(&widest.params[2].ty, RustType::Option(inner) if matches!(inner.as_ref(), RustType::Named { name, .. } if name == "PrefixOpts"))
        );
        assert!(widest.params[2].optional);
    }

    #[test]
    fn divergent_param_types_create_union() {
        // (x: string): void
        // (x: number): void
        // → widest: (x: StringOrF64)
        let sigs = vec![
            make_sig(vec![("x", RustType::String)], None),
            make_sig(vec![("x", RustType::F64)], None),
        ];
        let mut synthetic = SyntheticTypeRegistry::new();
        let widest = compute_widest_signature(&sigs, &mut synthetic);

        assert_eq!(widest.params.len(), 1);
        // Should be a synthetic union type
        assert!(
            matches!(&widest.params[0].ty, RustType::Named { name, .. } if !name.is_empty()),
            "expected Named union type, got {:?}",
            widest.params[0].ty
        );
    }

    // --- compute_union_return tests ---

    #[test]
    fn non_divergent_return_same_type() {
        let sigs = vec![
            make_sig(vec![("x", RustType::String)], Some(RustType::String)),
            make_sig(
                vec![("x", RustType::String), ("y", RustType::F64)],
                Some(RustType::String),
            ),
        ];
        let mut synthetic = SyntheticTypeRegistry::new();
        let widest = compute_widest_signature(&sigs, &mut synthetic);

        assert_eq!(widest.return_type, Some(RustType::String));
        assert!(!widest.return_diverges);
    }

    #[test]
    fn divergent_return_creates_union() {
        let cookie = RustType::Named {
            name: "Cookie".to_string(),
            type_args: vec![],
        };
        let sigs = vec![
            make_sig(vec![("c", RustType::String)], Some(cookie.clone())),
            make_sig(
                vec![("c", RustType::String), ("key", RustType::String)],
                Some(RustType::Option(Box::new(RustType::String))),
            ),
        ];
        let mut synthetic = SyntheticTypeRegistry::new();
        let widest = compute_widest_signature(&sigs, &mut synthetic);

        assert!(widest.return_diverges);
        assert!(
            matches!(&widest.return_type, Some(RustType::Named { name, .. }) if !name.is_empty()),
            "expected Named union return, got {:?}",
            widest.return_type
        );
    }

    #[test]
    fn all_void_returns() {
        let sigs = vec![
            make_sig(vec![("x", RustType::String)], None),
            make_sig(vec![("x", RustType::String), ("y", RustType::F64)], None),
        ];
        let mut synthetic = SyntheticTypeRegistry::new();
        let widest = compute_widest_signature(&sigs, &mut synthetic);

        assert_eq!(widest.return_type, None);
        assert!(!widest.return_diverges);
    }

    #[test]
    fn option_param_not_double_wrapped() {
        // (c: string): string
        // (c: string, key: Option<string>): string
        // → key is absent in overload 1, so it gets Option wrapped.
        //   But key is already Option<string> in overload 2.
        //   Should NOT become Option<Option<string>>.
        let sigs = vec![
            make_sig(vec![("c", RustType::String)], Some(RustType::String)),
            make_sig(
                vec![
                    ("c", RustType::String),
                    ("key", RustType::Option(Box::new(RustType::String))),
                ],
                Some(RustType::String),
            ),
        ];
        let mut synthetic = SyntheticTypeRegistry::new();
        let widest = compute_widest_signature(&sigs, &mut synthetic);

        assert_eq!(widest.params.len(), 2);
        // key should be Option<String>, NOT Option<Option<String>>
        assert_eq!(
            widest.params[1].ty,
            RustType::Option(Box::new(RustType::String))
        );
    }

    #[test]
    fn mixed_void_and_non_void_return() {
        // (x: string): void
        // (x: string, y: number): string
        // → return type: Option<String> (void means "might not return a value")
        let sigs = vec![
            make_sig(vec![("x", RustType::String)], None),
            make_sig(
                vec![("x", RustType::String), ("y", RustType::F64)],
                Some(RustType::String),
            ),
        ];
        let mut synthetic = SyntheticTypeRegistry::new();
        let widest = compute_widest_signature(&sigs, &mut synthetic);

        assert_eq!(
            widest.return_type,
            Some(RustType::Option(Box::new(RustType::String)))
        );
        assert!(!widest.return_diverges);
    }

    #[test]
    fn single_overload_passthrough() {
        let sigs = vec![make_sig(vec![("x", RustType::String)], Some(RustType::F64))];
        let mut synthetic = SyntheticTypeRegistry::new();
        let widest = compute_widest_signature(&sigs, &mut synthetic);

        assert_eq!(widest.params.len(), 1);
        assert_eq!(widest.params[0].ty, RustType::String);
        assert_eq!(widest.return_type, Some(RustType::F64));
        assert!(!widest.return_diverges);
    }

    #[test]
    fn test_apply_type_substitution_fills_defaults_for_missing_args() {
        // interface Foo<T, U = string> { (x: T): U }
        // const f: Foo<number> = ... → type_args = [F64], U defaults to String
        let sig = make_sig(
            vec![(
                "x",
                RustType::TypeVar {
                    name: "T".to_string(),
                },
            )],
            Some(RustType::TypeVar {
                name: "U".to_string(),
            }),
        );
        let type_params = vec![
            TypeParam {
                name: "T".to_string(),
                constraint: None,
                default: None,
            },
            TypeParam {
                name: "U".to_string(),
                constraint: None,
                default: Some(RustType::String), // default = string
            },
        ];
        let type_args = vec![RustType::F64]; // only T provided, U omitted

        let result = apply_type_substitution(&sig, &type_params, &type_args);

        // T → F64 (from explicit arg), U → String (from default)
        assert_eq!(result.params[0].ty, RustType::F64);
        assert_eq!(result.return_type, Some(RustType::String));
    }

    #[test]
    fn test_apply_type_substitution_explicit_args_override_defaults() {
        // interface Foo<T = string> { (x: T): T }
        // const f: Foo<number> = ... → explicit F64 overrides default String
        let sig = make_sig(
            vec![(
                "x",
                RustType::TypeVar {
                    name: "T".to_string(),
                },
            )],
            Some(RustType::TypeVar {
                name: "T".to_string(),
            }),
        );
        let type_params = vec![TypeParam {
            name: "T".to_string(),
            constraint: None,
            default: Some(RustType::String),
        }];
        let type_args = vec![RustType::F64]; // explicit arg overrides default

        let result = apply_type_substitution(&sig, &type_params, &type_args);

        assert_eq!(result.params[0].ty, RustType::F64);
        assert_eq!(result.return_type, Some(RustType::F64));
    }

    #[test]
    fn test_apply_type_substitution_all_defaulted_no_args() {
        // interface Foo<T = number, U = string> { (x: T): U }
        // const f: Foo = ... → all defaults applied
        let sig = make_sig(
            vec![(
                "x",
                RustType::TypeVar {
                    name: "T".to_string(),
                },
            )],
            Some(RustType::TypeVar {
                name: "U".to_string(),
            }),
        );
        let type_params = vec![
            TypeParam {
                name: "T".to_string(),
                constraint: None,
                default: Some(RustType::F64),
            },
            TypeParam {
                name: "U".to_string(),
                constraint: None,
                default: Some(RustType::String),
            },
        ];
        let type_args: Vec<RustType> = vec![]; // no args, all defaults

        let result = apply_type_substitution(&sig, &type_params, &type_args);

        assert_eq!(result.params[0].ty, RustType::F64);
        assert_eq!(result.return_type, Some(RustType::String));
    }

    #[test]
    fn test_apply_type_substitution_mixed_required_and_defaulted() {
        // interface Foo<T, U = string, V = number> { (x: T, y: U): V }
        // const f: Foo<boolean> = ... → T=Bool (explicit), U=String (default), V=F64 (default)
        let sig = make_sig(
            vec![
                (
                    "x",
                    RustType::TypeVar {
                        name: "T".to_string(),
                    },
                ),
                (
                    "y",
                    RustType::TypeVar {
                        name: "U".to_string(),
                    },
                ),
            ],
            Some(RustType::TypeVar {
                name: "V".to_string(),
            }),
        );
        let type_params = vec![
            TypeParam {
                name: "T".to_string(),
                constraint: None,
                default: None, // required
            },
            TypeParam {
                name: "U".to_string(),
                constraint: None,
                default: Some(RustType::String), // defaulted
            },
            TypeParam {
                name: "V".to_string(),
                constraint: None,
                default: Some(RustType::F64), // defaulted
            },
        ];
        let type_args = vec![RustType::Bool]; // only T provided

        let result = apply_type_substitution(&sig, &type_params, &type_args);

        assert_eq!(result.params[0].ty, RustType::Bool, "T → Bool (explicit)");
        assert_eq!(
            result.params[1].ty,
            RustType::String,
            "U → String (default)"
        );
        assert_eq!(result.return_type, Some(RustType::F64), "V → F64 (default)");
    }
}
