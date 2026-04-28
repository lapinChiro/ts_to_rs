//! `ReturnWrapContext` struct + builders + variant lookup helpers.
//!
//! The context bundles the synthetic enum name with the (return-type → variant-name)
//! mapping so [`super::wrapping::wrap_leaf`] can determine the correct variant for any
//! given leaf expression without re-scanning the call signatures.

use crate::ir::RustType;
use crate::pipeline::synthetic_registry::variant_name_for_type;
use crate::registry::MethodSignature;

/// Context for wrapping return expressions in a synthetic union enum variant.
#[derive(Debug, Clone)]
pub(crate) struct ReturnWrapContext {
    /// Name of the synthetic union enum (e.g., `"CookieOrOptionString"`)
    pub enum_name: String,
    /// Mapping from return type to variant name.
    pub variant_by_type: Vec<(RustType, String)>,
}

/// Builds a `ReturnWrapContext` from the call signatures of a callable interface.
///
/// Returns `None` if all overloads have the same return type (no wrapping needed).
pub(crate) fn build_return_wrap_context(
    call_sigs: &[MethodSignature],
    enum_name: &str,
) -> Option<ReturnWrapContext> {
    // Collect unique return types
    let return_types: Vec<RustType> = call_sigs
        .iter()
        .filter_map(|s| s.return_type.clone())
        .map(|ty| ty.unwrap_promise())
        .collect();

    let mut unique = Vec::new();
    for ty in &return_types {
        if !unique.contains(ty) {
            unique.push(ty.clone());
        }
    }

    // No divergence → no wrap needed
    if unique.len() <= 1 {
        return None;
    }

    let variant_by_type: Vec<(RustType, String)> = unique
        .iter()
        .map(|ty| (ty.clone(), variant_name_for_type(ty)))
        .collect();

    Some(ReturnWrapContext {
        enum_name: enum_name.to_string(),
        variant_by_type,
    })
}

/// Builds a `ReturnWrapContext` from a synthetic union enum's variants.
///
/// Used for general functions (not callable interfaces) whose return type
/// is a synthetic union enum. The variant mapping is derived from the
/// enum's `EnumVariant` definitions.
pub(crate) fn build_return_wrap_context_from_enum(
    enum_name: &str,
    variants: &[crate::ir::EnumVariant],
) -> ReturnWrapContext {
    let variant_by_type: Vec<(RustType, String)> = variants
        .iter()
        .filter_map(|v| v.data.as_ref().map(|ty| (ty.clone(), v.name.clone())))
        .collect();
    ReturnWrapContext {
        enum_name: enum_name.to_string(),
        variant_by_type,
    }
}

impl ReturnWrapContext {
    /// Finds the variant name for the given return type.
    ///
    /// Tries exact match first, then Option<T> narrowing (T matches Option<T> variant).
    pub(crate) fn variant_for(&self, ty: &RustType) -> Option<&str> {
        // Exact match
        if let Some((_, name)) = self.variant_by_type.iter().find(|(t, _)| t == ty) {
            return Some(name);
        }

        // Option narrowing: T can match Option<T>
        for (vty, name) in &self.variant_by_type {
            if let RustType::Option(inner) = vty {
                if inner.as_ref() == ty {
                    return Some(name);
                }
            }
        }

        None
    }

    /// Finds the unique Option<_> variant for polymorphic None wrapping.
    ///
    /// Returns `Some(variant_name)` if exactly one variant is `Option<_>`.
    /// Returns `None` if zero or multiple Option variants exist.
    ///
    /// `pub(super)` so [`super::wrapping::wrap_leaf`] can dispatch the polymorphic
    /// None wrap path without re-implementing the Option-variant scan.
    pub(super) fn unique_option_variant(&self) -> Option<&str> {
        let options: Vec<&str> = self
            .variant_by_type
            .iter()
            .filter(|(ty, _)| matches!(ty, RustType::Option(_)))
            .map(|(_, name)| name.as_str())
            .collect();
        if options.len() == 1 {
            Some(options[0])
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::ParamDef;

    fn make_sig(return_type: Option<RustType>) -> MethodSignature {
        MethodSignature {
            params: vec![ParamDef {
                name: "x".to_string(),
                ty: RustType::String,
                optional: false,
                has_default: false,
            }],
            return_type,
            ..Default::default()
        }
    }

    // --- build_return_wrap_context ---

    #[test]
    fn build_context_returns_none_for_identical_returns() {
        let sigs = vec![
            make_sig(Some(RustType::String)),
            make_sig(Some(RustType::String)),
        ];
        assert!(build_return_wrap_context(&sigs, "Unused").is_none());
    }

    #[test]
    fn build_context_collects_unique_variants() {
        let cookie = RustType::Named {
            name: "Cookie".to_string(),
            type_args: vec![],
        };
        let sigs = vec![
            make_sig(Some(cookie.clone())),
            make_sig(Some(RustType::Option(Box::new(RustType::String)))),
        ];
        let ctx = build_return_wrap_context(&sigs, "CookieOrOptionString").unwrap();
        assert_eq!(ctx.enum_name, "CookieOrOptionString");
        assert_eq!(ctx.variant_by_type.len(), 2);
        assert_eq!(ctx.variant_by_type[0].1, "Cookie");
        assert_eq!(ctx.variant_by_type[1].1, "OptionString");
    }

    #[test]
    fn build_context_dedupes_identical_returns() {
        let sigs = vec![
            make_sig(Some(RustType::String)),
            make_sig(Some(RustType::String)),
            make_sig(Some(RustType::F64)),
        ];
        let ctx = build_return_wrap_context(&sigs, "F64OrString").unwrap();
        assert_eq!(ctx.variant_by_type.len(), 2);
    }

    #[test]
    fn build_context_unwraps_promise_in_variants() {
        let promise_string = RustType::Named {
            name: "Promise".to_string(),
            type_args: vec![RustType::String],
        };
        let promise_f64 = RustType::Named {
            name: "Promise".to_string(),
            type_args: vec![RustType::F64],
        };
        let sigs = vec![make_sig(Some(promise_string)), make_sig(Some(promise_f64))];
        let ctx = build_return_wrap_context(&sigs, "F64OrString").unwrap();
        // Should unwrap Promise<String> → String, Promise<f64> → f64
        assert_eq!(ctx.variant_by_type.len(), 2);
        assert!(ctx
            .variant_by_type
            .iter()
            .any(|(ty, _)| *ty == RustType::String));
        assert!(ctx
            .variant_by_type
            .iter()
            .any(|(ty, _)| *ty == RustType::F64));
    }

    // --- variant_for ---

    #[test]
    fn variant_for_exact_match() {
        let ctx = ReturnWrapContext {
            enum_name: "Test".to_string(),
            variant_by_type: vec![
                (RustType::String, "String".to_string()),
                (RustType::F64, "F64".to_string()),
            ],
        };
        assert_eq!(ctx.variant_for(&RustType::String), Some("String"));
        assert_eq!(ctx.variant_for(&RustType::F64), Some("F64"));
    }

    #[test]
    fn variant_for_option_narrowing() {
        let ctx = ReturnWrapContext {
            enum_name: "Test".to_string(),
            variant_by_type: vec![
                (RustType::String, "String".to_string()),
                (
                    RustType::Option(Box::new(RustType::String)),
                    "OptionString".to_string(),
                ),
            ],
        };
        // String matches both exact String and Option<String> → exact match wins
        assert_eq!(ctx.variant_for(&RustType::String), Some("String"));
    }

    #[test]
    fn variant_for_returns_none_when_no_match() {
        let ctx = ReturnWrapContext {
            enum_name: "Test".to_string(),
            variant_by_type: vec![(RustType::String, "String".to_string())],
        };
        assert_eq!(ctx.variant_for(&RustType::Bool), None);
    }

    // --- unique_option_variant ---

    #[test]
    fn unique_option_variant_picks_single() {
        let ctx = ReturnWrapContext {
            enum_name: "Test".to_string(),
            variant_by_type: vec![
                (RustType::String, "String".to_string()),
                (
                    RustType::Option(Box::new(RustType::String)),
                    "OptionString".to_string(),
                ),
            ],
        };
        assert_eq!(ctx.unique_option_variant(), Some("OptionString"));
    }

    #[test]
    fn unique_option_variant_none_when_zero() {
        let ctx = ReturnWrapContext {
            enum_name: "Test".to_string(),
            variant_by_type: vec![
                (RustType::String, "String".to_string()),
                (RustType::F64, "F64".to_string()),
            ],
        };
        assert_eq!(ctx.unique_option_variant(), None);
    }

    #[test]
    fn unique_option_variant_none_when_multiple() {
        let ctx = ReturnWrapContext {
            enum_name: "Test".to_string(),
            variant_by_type: vec![
                (
                    RustType::Option(Box::new(RustType::String)),
                    "OptionString".to_string(),
                ),
                (
                    RustType::Option(Box::new(RustType::F64)),
                    "OptionF64".to_string(),
                ),
            ],
        };
        assert_eq!(ctx.unique_option_variant(), None);
    }
}
