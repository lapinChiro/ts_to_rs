//! Narrowing detection for TypeResolver.
//!
//! Detects type narrowing guards in `if` conditions (typeof, instanceof, null checks)
//! and records [`NarrowEvent::Narrow`] entries for the Transformer.
//!
//! Records both **positive** narrowing (the type the variable IS in the guarded scope)
//! and **complement** narrowing (the type the variable is in the opposite scope,
//! computed by excluding the positive type from the union).

use swc_common::Spanned;
use swc_ecma_ast as ast;

use super::*;
use crate::pipeline::narrowing_analyzer::{
    NarrowEvent, NarrowTrigger, NullCheckKind, PrimaryTrigger,
};
use crate::pipeline::narrowing_patterns;
use crate::pipeline::type_resolution::Span;

/// Classifies a binary operator + RHS shape into the [`NullCheckKind`]
/// variant that represents the check precisely.
///
/// - Loose equality (`==` / `!=`) always maps to the `EqNull` / `NotEqNull`
///   variants because JS coerces `null` and `undefined` together under loose
///   comparison.
/// - Strict equality (`===` / `!==`) distinguishes `null` from `undefined`
///   based on the RHS (caller-supplied): strict variants are populated only
///   when the RHS is the `undefined` identifier.
///
/// # Panics
///
/// Panics if `op` is not a null-check operator (`==` / `!=` / `===` / `!==`).
/// Callers must verify the operator before invoking this helper — this is a
/// structural contract and a silent wrong-value fallback would mask bugs.
fn classify_null_check(op: ast::BinaryOp, rhs_is_undefined: bool) -> NullCheckKind {
    match (op, rhs_is_undefined) {
        (ast::BinaryOp::EqEq, _) => NullCheckKind::EqNull,
        (ast::BinaryOp::NotEq, _) => NullCheckKind::NotEqNull,
        (ast::BinaryOp::EqEqEq, false) => NullCheckKind::EqEqEqNull,
        (ast::BinaryOp::EqEqEq, true) => NullCheckKind::EqEqEqUndefined,
        (ast::BinaryOp::NotEqEq, false) => NullCheckKind::NotEqEqNull,
        (ast::BinaryOp::NotEqEq, true) => NullCheckKind::NotEqEqUndefined,
        other => unreachable!(
            "classify_null_check called with non-null-check operator {:?}",
            other.0
        ),
    }
}

/// Maps a typeof string to the corresponding RustType variant name.
///
/// Used to identify which variant of a union enum corresponds to
/// a typeof check result. Returns `None` for unrecognized typeof strings.
fn typeof_to_variant_name(typeof_str: &str) -> Option<&'static str> {
    match typeof_str {
        "string" => Some("String"),
        "number" => Some("F64"),
        "boolean" => Some("Bool"),
        "object" => Some("Object"),
        "function" => Some("Function"),
        _ => None,
    }
}

/// Checks whether a variant's data type matches a typeof string.
///
/// Does NOT match `RustType::Any` — Any-typed variants (e.g., "Object" in any-narrowing enums)
/// are matched via exact variant name in [`typeof_to_variant_name`], not by data type.
fn variant_matches_typeof(data: &RustType, typeof_str: &str) -> bool {
    match typeof_str {
        "string" => matches!(data, RustType::String),
        "number" => matches!(data, RustType::F64),
        "boolean" => matches!(data, RustType::Bool),
        "object" => matches!(data, RustType::Named { .. } | RustType::Vec(_)),
        "function" => matches!(data, RustType::Fn { .. }),
        _ => false,
    }
}

impl<'a> TypeResolver<'a> {
    /// Detects narrowing guards in `if` conditions and records
    /// [`NarrowEvent::Narrow`] entries with the corresponding
    /// [`NarrowTrigger`].
    ///
    /// Records both positive and complement narrowing events:
    /// - **Positive**: The type that the variable IS in the guarded scope
    /// - **Complement**: The type that the variable is in the opposite scope
    ///   (computed by excluding the positive type's variant from the union)
    ///
    /// For compound `&&` guards, complement narrowing is NOT recorded
    /// (De Morgan: `!(A && B) = !A || !B`, so neither is guaranteed).
    pub(super) fn detect_narrowing_guard(
        &mut self,
        test: &ast::Expr,
        consequent: &ast::Stmt,
        alternate: Option<&ast::Stmt>,
    ) {
        let cons_span = Span::from_swc(consequent.span());
        let alt_span = alternate.map(|s| Span::from_swc(s.span()));

        match test {
            // Compound: a && b → detect narrowing from both sides.
            // Consequent narrowing is valid (both conditions are true in then-block).
            // Alternate/complement narrowing is NOT valid for individual sub-guards
            // (else means !(A && B) = !A || !B, so neither A nor B is guaranteed false).
            ast::Expr::Bin(bin) if matches!(bin.op, ast::BinaryOp::LogicalAnd) => {
                self.detect_narrowing_guard(&bin.left, consequent, None);
                self.detect_narrowing_guard(&bin.right, consequent, None);
            }
            ast::Expr::Bin(bin) => {
                let is_eq = matches!(bin.op, ast::BinaryOp::EqEqEq | ast::BinaryOp::EqEq);
                let is_neq = matches!(bin.op, ast::BinaryOp::NotEqEq | ast::BinaryOp::NotEq);

                // typeof narrowing
                if is_eq || is_neq {
                    if let Some((var_name, narrowed_type, type_str)) =
                        self.extract_typeof_narrowing(bin)
                    {
                        // === → positive in consequent, !== → positive in alternate
                        let positive_span = if is_eq { Some(cons_span) } else { alt_span };
                        // Complement goes to the opposite scope
                        let complement_span = if is_eq { alt_span } else { Some(cons_span) };

                        if let Some(span) = positive_span {
                            self.result.narrow_events.push(NarrowEvent::Narrow {
                                scope_start: span.lo,
                                scope_end: span.hi,
                                var_name: var_name.clone(),
                                narrowed_type: narrowed_type.clone(),
                                trigger: NarrowTrigger::Primary(PrimaryTrigger::TypeofGuard(
                                    type_str.clone(),
                                )),
                            });
                        }

                        // Record complement narrowing in the opposite scope
                        if let Some(span) = complement_span {
                            if let Some(complement) =
                                self.compute_complement_type(&var_name, &narrowed_type)
                            {
                                self.result.narrow_events.push(NarrowEvent::Narrow {
                                    scope_start: span.lo,
                                    scope_end: span.hi,
                                    var_name,
                                    narrowed_type: complement,
                                    trigger: NarrowTrigger::Primary(PrimaryTrigger::TypeofGuard(
                                        type_str,
                                    )),
                                });
                            }
                        }
                        // typeof was handled; skip null check below to avoid double-processing
                        return;
                    }
                }

                // null/undefined narrowing
                if is_eq || is_neq {
                    if let Some((var_name, narrowed_type, null_kind)) =
                        self.extract_null_check_narrowing(bin)
                    {
                        // !== null → consequent, === null → alternate
                        let target_span = if is_neq { Some(cons_span) } else { alt_span };
                        if let Some(span) = target_span {
                            self.result.narrow_events.push(NarrowEvent::Narrow {
                                scope_start: span.lo,
                                scope_end: span.hi,
                                var_name,
                                narrowed_type,
                                trigger: NarrowTrigger::Primary(PrimaryTrigger::NullCheck(
                                    null_kind,
                                )),
                            });
                        }
                        // No complement for null check: the opposite scope has Option<T> which
                        // is correct in Rust (if-let else naturally handles None).
                    }
                }

                // x instanceof Foo
                if matches!(bin.op, ast::BinaryOp::InstanceOf) {
                    if let (ast::Expr::Ident(var_ident), ast::Expr::Ident(class_ident)) =
                        (bin.left.as_ref(), bin.right.as_ref())
                    {
                        let var_name = var_ident.sym.to_string();
                        let class_name = class_ident.sym.to_string();
                        let narrowed_type = RustType::Named {
                            name: class_name.clone(),
                            type_args: vec![],
                        };

                        self.result.narrow_events.push(NarrowEvent::Narrow {
                            scope_start: cons_span.lo,
                            scope_end: cons_span.hi,
                            var_name: var_name.clone(),
                            narrowed_type: narrowed_type.clone(),
                            trigger: NarrowTrigger::Primary(PrimaryTrigger::InstanceofGuard(
                                class_name.clone(),
                            )),
                        });

                        // Complement narrowing in else
                        if let Some(span) = alt_span {
                            if let Some(complement) =
                                self.compute_complement_type(&var_name, &narrowed_type)
                            {
                                self.result.narrow_events.push(NarrowEvent::Narrow {
                                    scope_start: span.lo,
                                    scope_end: span.hi,
                                    var_name,
                                    narrowed_type: complement,
                                    trigger: NarrowTrigger::Primary(
                                        PrimaryTrigger::InstanceofGuard(class_name),
                                    ),
                                });
                            }
                        }
                    }
                }
            }
            // Truthy check: if (x) where x is Option<T> → narrow to T
            ast::Expr::Ident(ident) => {
                let var_name = ident.sym.to_string();
                if let ResolvedType::Known(RustType::Option(inner)) = self.lookup_var(&var_name) {
                    self.result.narrow_events.push(NarrowEvent::Narrow {
                        scope_start: cons_span.lo,
                        scope_end: cons_span.hi,
                        var_name,
                        narrowed_type: inner.as_ref().clone(),
                        trigger: NarrowTrigger::Primary(PrimaryTrigger::Truthy),
                    });
                    // No complement for truthy: else has Option<T> which is correct
                }
            }
            _ => {}
        }
    }

    /// Detects complement narrowing after an always-exiting if-block (early return pattern).
    ///
    /// When `if (guard) { return/throw; }` is followed by more code,
    /// the code after the if benefits from complement narrowing.
    /// The complement scope is `[if_end, block_end)`.
    pub(super) fn detect_early_return_narrowing(
        &mut self,
        test: &ast::Expr,
        if_end: u32,
        block_end: u32,
    ) {
        if if_end >= block_end {
            return;
        }

        match test {
            ast::Expr::Bin(bin) => {
                let is_eq = matches!(bin.op, ast::BinaryOp::EqEqEq | ast::BinaryOp::EqEq);
                let is_neq = matches!(bin.op, ast::BinaryOp::NotEqEq | ast::BinaryOp::NotEq);

                // typeof early return: if (typeof x === "string") { return; }
                // → x is NOT string after → complement type
                if is_eq || is_neq {
                    if let Some((var_name, positive_type, type_str)) =
                        self.extract_typeof_narrowing(bin)
                    {
                        let complement_after = if is_eq {
                            self.compute_complement_type(&var_name, &positive_type)
                        } else {
                            Some(positive_type)
                        };
                        if let Some(narrowed_type) = complement_after {
                            self.result.narrow_events.push(NarrowEvent::Narrow {
                                scope_start: if_end,
                                scope_end: block_end,
                                var_name,
                                narrowed_type,
                                trigger: NarrowTrigger::EarlyReturnComplement(
                                    PrimaryTrigger::TypeofGuard(type_str),
                                ),
                            });
                        }
                        return;
                    }
                }

                // null check early return: if (x === null) { return; }
                // → x is non-null after → unwrapped Option
                if is_eq {
                    if let Some((var_name, unwrapped_type, null_kind)) =
                        self.extract_null_check_narrowing(bin)
                    {
                        self.result.narrow_events.push(NarrowEvent::Narrow {
                            scope_start: if_end,
                            scope_end: block_end,
                            var_name,
                            narrowed_type: unwrapped_type,
                            trigger: NarrowTrigger::EarlyReturnComplement(
                                PrimaryTrigger::NullCheck(null_kind),
                            ),
                        });
                        return;
                    }
                }

                // instanceof early return: if (x instanceof Foo) { return; }
                // → x is NOT Foo after → complement type
                if matches!(bin.op, ast::BinaryOp::InstanceOf) {
                    if let (ast::Expr::Ident(var_ident), ast::Expr::Ident(class_ident)) =
                        (bin.left.as_ref(), bin.right.as_ref())
                    {
                        let var_name = var_ident.sym.to_string();
                        let class_name = class_ident.sym.to_string();
                        let positive_type = RustType::Named {
                            name: class_name.clone(),
                            type_args: vec![],
                        };
                        if let Some(complement) =
                            self.compute_complement_type(&var_name, &positive_type)
                        {
                            self.result.narrow_events.push(NarrowEvent::Narrow {
                                scope_start: if_end,
                                scope_end: block_end,
                                var_name,
                                narrowed_type: complement,
                                trigger: NarrowTrigger::EarlyReturnComplement(
                                    PrimaryTrigger::InstanceofGuard(class_name),
                                ),
                            });
                        }
                    }
                }
            }
            // Negated truthy: if (!x) { return; } → x is non-null after
            ast::Expr::Unary(unary) if unary.op == ast::UnaryOp::Bang => {
                if let ast::Expr::Ident(ident) = unary.arg.as_ref() {
                    let var_name = ident.sym.to_string();
                    if let ResolvedType::Known(RustType::Option(inner)) = self.lookup_var(&var_name)
                    {
                        self.result.narrow_events.push(NarrowEvent::Narrow {
                            scope_start: if_end,
                            scope_end: block_end,
                            var_name,
                            narrowed_type: inner.as_ref().clone(),
                            trigger: NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::Truthy),
                        });
                    }
                }
            }
            // Truthy: if (x) { return; } → x is null/None after (no useful narrowing)
            // The variable stays as Option<T> which is correct.
            _ => {}
        }
    }

    fn extract_typeof_narrowing(&self, bin: &ast::BinExpr) -> Option<(String, RustType, String)> {
        // typeof x === "string" → (x, String, "string")
        let (typeof_expr, type_str) = narrowing_patterns::extract_typeof_and_string(bin)?;
        let var_name = match typeof_expr {
            ast::Expr::Ident(ident) => ident.sym.to_string(),
            _ => return None,
        };
        // Primitive types: statically known narrowed type
        let narrowed_type = match type_str.as_str() {
            "string" => RustType::String,
            "number" => RustType::F64,
            "boolean" => RustType::Bool,
            // "object"/"function": need to look up the variable's type to find the
            // matching variant's data type in the union enum
            "object" | "function" => {
                let (name, ty) =
                    self.resolve_typeof_narrowed_type_from_var(&var_name, &type_str)?;
                return Some((name, ty, type_str));
            }
            _ => return None,
        };
        Some((var_name, narrowed_type, type_str))
    }

    /// Resolves the narrowed type for typeof "object"/"function" by looking up
    /// the variable's union enum variants.
    fn resolve_typeof_narrowed_type_from_var(
        &self,
        var_name: &str,
        type_str: &str,
    ) -> Option<(String, RustType)> {
        let var_type = self.lookup_var(var_name);
        let enum_name = match &var_type {
            ResolvedType::Known(RustType::Named { name, .. }) => name.clone(),
            _ => return None,
        };
        let syn_def = self.synthetic.get(&enum_name)?;
        let variants = match &syn_def.item {
            crate::ir::Item::Enum { variants, .. } => variants,
            _ => return None,
        };
        // Find variant whose data type matches the typeof string.
        // For any-narrowing enums: "Object" variant has RustType::Any
        // For standard unions: find variant by data type matching
        let expected_variant_name = typeof_to_variant_name(type_str);
        let matching_variant = variants.iter().find(|v| {
            let Some(ref data) = v.data else {
                return false;
            };
            // First try exact variant name match (any-narrowing enums)
            if let Some(expected) = expected_variant_name {
                if v.name == expected {
                    return true;
                }
            }
            // Then try data type matching (standard union enums)
            variant_matches_typeof(data, type_str)
                && v.name != "Other"
                && !["String", "F64", "Bool", "Object", "Function"].contains(&v.name.as_str())
        });
        matching_variant
            .and_then(|v| v.data.clone())
            .map(|ty| (var_name.to_string(), ty))
    }

    /// Computes the complement type for a variable's narrowed type.
    ///
    /// Given a variable of union enum type and a positive narrowed type,
    /// returns the type(s) remaining after excluding the positive type's variant.
    ///
    /// - 2-variant union: returns the other variant's data type
    /// - 3+ variant union: generates a sub-union enum from remaining variants
    /// - Non-union or non-enum types: returns `None`
    fn compute_complement_type(
        &mut self,
        var_name: &str,
        positive_type: &RustType,
    ) -> Option<RustType> {
        let var_type = self.lookup_var(var_name);
        let enum_name = match &var_type {
            ResolvedType::Known(RustType::Named { name, .. }) => name.clone(),
            _ => return None,
        };

        let syn_def = self.synthetic.get(&enum_name)?;
        let variants = match &syn_def.item {
            crate::ir::Item::Enum { variants, .. } => variants.clone(),
            _ => return None,
        };

        // Find which variant corresponds to the positive type.
        // Use variant name matching first (robust), then fall back to data type matching.
        let positive_variant_name = variants
            .iter()
            .find(|v| {
                // For primitive types, match by the canonical variant name
                let expected_name = match positive_type {
                    RustType::String => Some("String"),
                    RustType::F64 => Some("F64"),
                    RustType::Bool => Some("Bool"),
                    _ => None,
                };
                if let Some(name) = expected_name {
                    return v.name == name;
                }
                // For Named/Fn/other types, match by data type equality
                v.data.as_ref() == Some(positive_type) && v.name != "Other"
            })
            .map(|v| v.name.clone())?;

        // Collect remaining variants (excluding the positive one and "Other")
        let remaining: Vec<_> = variants
            .iter()
            .filter(|v| v.name != positive_variant_name && v.name != "Other")
            .collect();

        match remaining.len() {
            0 => None,
            1 => {
                // 2-variant union: return the other variant's data type directly
                remaining[0].data.clone()
            }
            _ => {
                // 3+ variant union: generate a sub-union from remaining data types
                let remaining_types: Vec<RustType> =
                    remaining.iter().filter_map(|v| v.data.clone()).collect();
                let sub_union_name = self.synthetic.register_union(&remaining_types);
                Some(RustType::Named {
                    name: sub_union_name,
                    type_args: vec![],
                })
            }
        }
    }

    fn extract_null_check_narrowing(
        &self,
        bin: &ast::BinExpr,
    ) -> Option<(String, RustType, NullCheckKind)> {
        // x !== null / x !== undefined → remove Option wrapper from x's type
        let (var_expr, rhs_is_undefined) = if narrowing_patterns::is_null_or_undefined(&bin.right) {
            (
                bin.left.as_ref(),
                narrowing_patterns::is_undefined_ident(&bin.right),
            )
        } else if narrowing_patterns::is_null_or_undefined(&bin.left) {
            (
                bin.right.as_ref(),
                narrowing_patterns::is_undefined_ident(&bin.left),
            )
        } else {
            return None;
        };

        let var_name = match var_expr {
            ast::Expr::Ident(ident) => ident.sym.to_string(),
            _ => return None,
        };

        // Get current type and unwrap Option
        let current_type = self.lookup_var(&var_name);
        match current_type {
            ResolvedType::Known(RustType::Option(inner)) => Some((
                var_name,
                inner.as_ref().clone(),
                classify_null_check(bin.op, rhs_is_undefined),
            )),
            _ => None,
        }
    }
}
