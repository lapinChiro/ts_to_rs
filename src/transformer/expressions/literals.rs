//! Literal and string-related conversion helpers.
//!
//! Contains functions for converting SWC literal nodes to IR expressions
//! and utilities for detecting string types and format specifiers.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{BinOp, CallTarget, Expr, RustType, UserTypeRef};
use crate::pipeline::synthetic_registry::variant_name_for_type;
use crate::registry::{TypeDef, TypeRegistry};
use crate::transformer::Transformer;

impl<'a> Transformer<'a> {
    /// Converts an SWC literal to an IR expression.
    ///
    /// When `expected` is `RustType::String`, string literals are wrapped with `.to_string()`
    /// to produce an owned `String` instead of `&str`.
    pub(crate) fn convert_lit(
        &mut self,
        lit: &ast::Lit,
        expected: Option<&RustType>,
    ) -> Result<Expr> {
        match lit {
            ast::Lit::Num(n) => {
                let expr = Expr::NumberLit(n.value);
                if let Some(wrapped) = wrap_in_synthetic_union_variant(
                    self.reg(),
                    expected,
                    &RustType::F64,
                    expr.clone(),
                ) {
                    return Ok(wrapped);
                }
                Ok(expr)
            }
            ast::Lit::Str(s) => {
                let value = s.value.to_string_lossy().into_owned();
                // Check if the expected type is a string literal union enum
                if let Some(RustType::Named { name, .. }) = expected {
                    if let Some(variant) = lookup_string_enum_variant(self.reg(), name, &value) {
                        return Ok(Expr::EnumVariant {
                            enum_ty: crate::ir::UserTypeRef::new(name),
                            variant: variant.to_string(),
                        });
                    }
                }
                // I-144 T6-3 (cell-i024): synthetic `Option<Union<..., String, ...>>`
                // call-arg wrap — outer `Option` has already been peeled, so
                // `expected` is `Named { name: Union }` and we emit
                // `Union::String("<value>".to_string())`.
                let string_lit_with_owned = Expr::MethodCall {
                    object: Box::new(Expr::StringLit(value.clone())),
                    method: "to_string".to_string(),
                    args: vec![],
                };
                if let Some(wrapped) = wrap_in_synthetic_union_variant(
                    self.reg(),
                    expected,
                    &RustType::String,
                    string_lit_with_owned,
                ) {
                    return Ok(wrapped);
                }
                let expr = Expr::StringLit(value);
                if matches!(expected, Some(RustType::String)) {
                    Ok(Expr::MethodCall {
                        object: Box::new(expr),
                        method: "to_string".to_string(),
                        args: vec![],
                    })
                } else {
                    Ok(expr)
                }
            }
            ast::Lit::Bool(b) => {
                let expr = Expr::BoolLit(b.value);
                if let Some(wrapped) =
                    wrap_in_synthetic_union_variant(self.reg(), expected, &RustType::Bool, expr)
                {
                    return Ok(wrapped);
                }
                Ok(Expr::BoolLit(b.value))
            }
            ast::Lit::Null(_) => Ok(Expr::BuiltinVariantValue(crate::ir::BuiltinVariant::None)),
            ast::Lit::Regex(regex) => {
                let pattern = regex.exp.to_string();
                let flags = regex.flags.to_string();
                // Embed supported flags as inline flags in the pattern
                let mut prefix = String::new();
                if flags.contains('i') {
                    prefix.push_str("(?i)");
                }
                if flags.contains('m') {
                    prefix.push_str("(?m)");
                }
                if flags.contains('s') {
                    prefix.push_str("(?s)");
                }
                // 'u' flag: Rust regex is Unicode-aware by default — no action needed.
                let full_pattern = format!("{prefix}{pattern}");
                Ok(Expr::Regex {
                    pattern: full_pattern,
                    global: flags.contains('g'),
                    sticky: flags.contains('y'),
                })
            }
            ast::Lit::BigInt(bigint) => {
                // BigInt literals (e.g., 123n) → i128
                let value: i128 = bigint.value.to_string().parse().map_err(|_| {
                    super::super::UnsupportedSyntaxError::new(
                        format!("BigInt literal out of i128 range: {}n", bigint.value),
                        bigint.span,
                    )
                })?;
                Ok(Expr::IntLit(value))
            }
            _ => Err(anyhow!("unsupported literal: {:?}", lit)),
        }
    }
}

/// Wraps `inner` (already converted to the Rust-side representation of
/// `literal_ty`) in a synthetic union variant constructor when `expected` is
/// a `Named` type registered as a [`SyntheticTypeKind::UnionEnum`] and one
/// of its variants carries `literal_ty` as its payload.
///
/// Emits `Expr::FnCall { target: UserEnumVariantCtor, args: [inner] }`.
/// Returns `None` when:
/// - `expected` is not a synthetic union,
/// - `expected` is `None`,
/// - or no variant's `data` matches `literal_ty`.
///
/// I-144 T6-3 cell-i024 path: required so `f("hi")` on
/// `function f(x: string | number | null)` emits
/// `f(Some(F64OrString::String("hi".to_string())))` rather than
/// `f(Some("hi"))` (which mismatches `Option<F64OrString>` at the call site).
pub(super) fn wrap_in_synthetic_union_variant(
    reg: &TypeRegistry,
    expected: Option<&RustType>,
    literal_ty: &RustType,
    inner: Expr,
) -> Option<Expr> {
    let RustType::Named { name, type_args } = expected? else {
        return None;
    };
    if !type_args.is_empty() {
        return None;
    }
    let TypeDef::Enum {
        variants,
        string_values,
        tag_field,
        ..
    } = reg.get(name)?
    else {
        return None;
    };
    // Skip string-literal union enums (handled by `lookup_string_enum_variant`)
    // and discriminated unions (non-synthetic variant shape).
    if !string_values.is_empty() || tag_field.is_some() {
        return None;
    }
    // Variant names follow `variant_name_for_type` by convention for synthetic
    // union enums: `RustType::String` → `"String"`, `RustType::F64` → `"F64"`,
    // `RustType::Bool` → `"Bool"`. If the expected variant exists in the enum
    // we emit `Enum::Variant(inner)`.
    let expected_variant = variant_name_for_type(literal_ty);
    if !variants.iter().any(|v| v == &expected_variant) {
        return None;
    }
    Some(Expr::FnCall {
        target: CallTarget::UserEnumVariantCtor {
            enum_ty: UserTypeRef::new(name.clone()),
            variant: expected_variant,
        },
        args: vec![inner],
    })
}

/// 文字列リテラル値から string literal union enum のバリアント名を逆引きする。
pub(super) fn lookup_string_enum_variant<'a>(
    reg: &'a TypeRegistry,
    enum_name: &str,
    string_value: &str,
) -> Option<&'a String> {
    if let Some(TypeDef::Enum { string_values, .. }) = reg.get(enum_name) {
        string_values.get(string_value)
    } else {
        None
    }
}

/// Checks whether a RustType represents a string (including Option<String>).
pub(super) fn is_string_type(ty: &RustType) -> bool {
    matches!(ty, RustType::String)
        || matches!(ty, RustType::Option(inner) if matches!(inner.as_ref(), RustType::String))
}

/// `println!` の引数で `{:?}` (Debug) を使うべき型かどうかを判定する。
///
/// `Vec<T>`, `Tuple` は Debug フォーマットを使う。
/// プリミティブ型と Named 型（enum/struct）は Display を使う。
/// `Option<T>` は console.log 変換時に `wrap_option_for_display` で個別処理される。
pub(super) fn needs_debug_format(ty: Option<&RustType>) -> bool {
    match ty {
        None => false, // 型不明の場合は Display を試みる（コンパイルエラーで発見できる）
        Some(RustType::Vec(_)) => true,
        Some(RustType::Tuple(_)) => true,
        _ => false,
    }
}

/// Checks whether an IR expression is known to produce a String value.
///
/// Used to detect string concatenation (`+`) and wrap the RHS in `&`.
pub(super) fn is_string_like(expr: &Expr) -> bool {
    match expr {
        Expr::StringLit(_) | Expr::FormatMacro { .. } => true,
        Expr::MethodCall { method, .. }
            if method == "to_string"
                || method == "to_uppercase"
                || method == "to_lowercase"
                || method == "trim"
                || method == "replacen" =>
        {
            true
        }
        Expr::BinaryOp {
            op: BinOp::Add,
            left,
            ..
        } => is_string_like(left),
        _ => false,
    }
}

#[cfg(test)]
mod wrap_in_synthetic_union_variant_tests {
    //! Unit tests for [`wrap_in_synthetic_union_variant`] covering every
    //! branch of the function (testing.md: equivalence partitioning +
    //! branch coverage). Each `return None` path has a dedicated test,
    //! and the positive path is exercised per primitive variant.
    use super::*;
    use crate::ir::{BuiltinVariant, RustType};
    use crate::registry::TypeRegistry;
    use std::collections::HashMap;

    fn union_registry(enum_name: &str, variant_names: &[&str]) -> TypeRegistry {
        let mut reg = TypeRegistry::new();
        reg.register(
            enum_name.to_string(),
            TypeDef::Enum {
                type_params: vec![],
                variants: variant_names.iter().map(|s| s.to_string()).collect(),
                string_values: HashMap::new(),
                tag_field: None,
                variant_fields: HashMap::new(),
            },
        );
        reg
    }

    fn sample_inner() -> Expr {
        // Arbitrary inner expression; the function must not inspect its shape.
        Expr::BuiltinVariantValue(BuiltinVariant::None)
    }

    #[test]
    fn expected_none_returns_none() {
        let reg = TypeRegistry::new();
        assert!(
            wrap_in_synthetic_union_variant(&reg, None, &RustType::F64, sample_inner()).is_none(),
            "expected=None must short-circuit to None"
        );
    }

    #[test]
    fn expected_non_named_returns_none() {
        let reg = TypeRegistry::new();
        // Non-Named expected (e.g., Option<F64>) must not emit a ctor wrap.
        assert!(wrap_in_synthetic_union_variant(
            &reg,
            Some(&RustType::Option(Box::new(RustType::F64))),
            &RustType::F64,
            sample_inner(),
        )
        .is_none());
        assert!(wrap_in_synthetic_union_variant(
            &reg,
            Some(&RustType::String),
            &RustType::String,
            sample_inner(),
        )
        .is_none());
    }

    #[test]
    fn named_with_type_args_returns_none() {
        let reg = union_registry("Container", &["F64"]);
        let generic = RustType::Named {
            name: "Container".to_string(),
            type_args: vec![RustType::F64],
        };
        assert!(wrap_in_synthetic_union_variant(
            &reg,
            Some(&generic),
            &RustType::F64,
            sample_inner(),
        )
        .is_none());
    }

    #[test]
    fn registry_miss_returns_none() {
        let reg = TypeRegistry::new();
        let missing = RustType::Named {
            name: "Unknown".to_string(),
            type_args: vec![],
        };
        assert!(wrap_in_synthetic_union_variant(
            &reg,
            Some(&missing),
            &RustType::F64,
            sample_inner(),
        )
        .is_none());
    }

    #[test]
    fn non_enum_type_returns_none() {
        let mut reg = TypeRegistry::new();
        reg.register(
            "MyStruct".to_string(),
            TypeDef::Struct {
                type_params: vec![],
                fields: vec![],
                methods: HashMap::new(),
                constructor: None,
                call_signatures: vec![],
                extends: vec![],
                is_interface: false,
            },
        );
        let struct_ty = RustType::Named {
            name: "MyStruct".to_string(),
            type_args: vec![],
        };
        assert!(wrap_in_synthetic_union_variant(
            &reg,
            Some(&struct_ty),
            &RustType::F64,
            sample_inner(),
        )
        .is_none());
    }

    #[test]
    fn string_literal_union_returns_none() {
        let mut reg = TypeRegistry::new();
        let mut string_values = HashMap::new();
        string_values.insert("a".to_string(), "A".to_string());
        string_values.insert("b".to_string(), "B".to_string());
        reg.register(
            "StringLitEnum".to_string(),
            TypeDef::Enum {
                type_params: vec![],
                variants: vec!["A".to_string(), "B".to_string()],
                string_values,
                tag_field: None,
                variant_fields: HashMap::new(),
            },
        );
        let enum_ty = RustType::Named {
            name: "StringLitEnum".to_string(),
            type_args: vec![],
        };
        // String literal unions have their own handler (`lookup_string_enum_variant`).
        assert!(wrap_in_synthetic_union_variant(
            &reg,
            Some(&enum_ty),
            &RustType::String,
            sample_inner(),
        )
        .is_none());
    }

    #[test]
    fn discriminated_union_returns_none() {
        let mut reg = TypeRegistry::new();
        reg.register(
            "Tagged".to_string(),
            TypeDef::Enum {
                type_params: vec![],
                variants: vec!["A".to_string(), "B".to_string()],
                string_values: HashMap::new(),
                tag_field: Some("kind".to_string()),
                variant_fields: HashMap::new(),
            },
        );
        let tagged = RustType::Named {
            name: "Tagged".to_string(),
            type_args: vec![],
        };
        // Discriminated unions use variant-field emission, not primitive wrap.
        assert!(wrap_in_synthetic_union_variant(
            &reg,
            Some(&tagged),
            &RustType::F64,
            sample_inner(),
        )
        .is_none());
    }

    #[test]
    fn variant_missing_returns_none() {
        let reg = union_registry("OnlyF64", &["F64"]);
        let ty = RustType::Named {
            name: "OnlyF64".to_string(),
            type_args: vec![],
        };
        // Literal is `Bool` but the union only has an `F64` variant.
        assert!(
            wrap_in_synthetic_union_variant(&reg, Some(&ty), &RustType::Bool, sample_inner(),)
                .is_none()
        );
    }

    #[test]
    fn f64_variant_emits_ctor_call() {
        let reg = union_registry("F64OrString", &["F64", "String"]);
        let ty = RustType::Named {
            name: "F64OrString".to_string(),
            type_args: vec![],
        };
        let result =
            wrap_in_synthetic_union_variant(&reg, Some(&ty), &RustType::F64, Expr::NumberLit(42.0))
                .expect("F64 variant must wrap");
        assert_eq!(
            result,
            Expr::FnCall {
                target: CallTarget::UserEnumVariantCtor {
                    enum_ty: UserTypeRef::new("F64OrString".to_string()),
                    variant: "F64".to_string(),
                },
                args: vec![Expr::NumberLit(42.0)],
            }
        );
    }

    #[test]
    fn string_variant_emits_ctor_call() {
        let reg = union_registry("F64OrString", &["F64", "String"]);
        let ty = RustType::Named {
            name: "F64OrString".to_string(),
            type_args: vec![],
        };
        let inner = Expr::MethodCall {
            object: Box::new(Expr::StringLit("hi".to_string())),
            method: "to_string".to_string(),
            args: vec![],
        };
        let result =
            wrap_in_synthetic_union_variant(&reg, Some(&ty), &RustType::String, inner.clone())
                .expect("String variant must wrap");
        match result {
            Expr::FnCall {
                target:
                    CallTarget::UserEnumVariantCtor {
                        ref enum_ty,
                        ref variant,
                    },
                ref args,
            } => {
                assert_eq!(enum_ty.as_str(), "F64OrString");
                assert_eq!(variant, "String");
                assert_eq!(args, &[inner]);
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn bool_variant_emits_ctor_call() {
        let reg = union_registry("BoolOrF64", &["Bool", "F64"]);
        let ty = RustType::Named {
            name: "BoolOrF64".to_string(),
            type_args: vec![],
        };
        let result =
            wrap_in_synthetic_union_variant(&reg, Some(&ty), &RustType::Bool, Expr::BoolLit(true))
                .expect("Bool variant must wrap");
        assert_eq!(
            result,
            Expr::FnCall {
                target: CallTarget::UserEnumVariantCtor {
                    enum_ty: UserTypeRef::new("BoolOrF64".to_string()),
                    variant: "Bool".to_string(),
                },
                args: vec![Expr::BoolLit(true)],
            }
        );
    }
}
