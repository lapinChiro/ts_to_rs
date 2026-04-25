//! Narrow guard detection: typeof / instanceof / null check / OptChain /
//! truthy + early-return complement.
//!
//! These were originally methods on `TypeResolver`
//! (`pipeline::type_resolver::narrowing`) and are now the canonical
//! home for narrowing-trigger AST matching, giving the
//! [`narrowing_analyzer`](super) module its "single source of truth"
//! status for narrow-related analysis. The resolver retains only a
//! thin [`super::NarrowTypeContext`] implementation that supplies
//! declared-type lookup and synthetic-enum introspection.
//!
//! # Responsibilities
//!
//! - **Positive narrowing**: record the type `x` IS inside a guarded
//!   scope (e.g. `if (typeof x === "string") { /* x is String */ }`).
//! - **Complement narrowing**: record the type `x` is in the *opposite*
//!   scope, computed by excluding the positive variant from the union
//!   (e.g. the `else` arm of `if (x instanceof Foo)`).
//! - **Early-return complement**: if an `if` whose then-block always
//!   exits narrows `x`, the fall-through scope enjoys the complement
//!   (T11 in the PRD).
//!
//! # Design notes
//!
//! - **No complement for `&&` sub-guards**: `!(A && B) == !A || !B`
//!   does not imply either side is false, so
//!   [`detect_narrowing_guard`] recurses with `alternate = None` on
//!   both legs of a compound guard.
//! - **Null-check complement asymmetry**: `x !== null` narrowing in
//!   the opposite (`else`) scope is deliberately skipped — the
//!   resolver already exposes the original `Option<T>` there, which
//!   `if-let` / `match` emission consumes naturally.
//! - **Truthy complement asymmetry**: analogous to null-check — the
//!   `else` of `if (x)` for `Option<T>` leaves the original
//!   `Option<T>`; no explicit complement is recorded.

use swc_common::Spanned;
use swc_ecma_ast as ast;

use crate::ir::RustType;
use crate::pipeline::narrowing_patterns;
use crate::pipeline::type_resolution::Span;
use crate::pipeline::ResolvedType;
use crate::transformer::helpers::peek_through::peek_through_type_assertions;

use super::events::{NarrowEvent, NarrowTrigger, NullCheckKind, PrimaryTrigger};
use super::type_context::NarrowTypeContext;

/// Detects narrowing guards inside an `if` condition and records
/// [`NarrowEvent::Narrow`] events through `ctx`.
///
/// The `consequent` span carries the scope for positive narrowing; the
/// `alternate` (if present) carries the scope for complement narrowing.
///
/// Handles these trigger shapes:
///
/// - `typeof x === "string"` (positive in cons, complement in alt)
/// - `typeof x !== "string"` (positive in alt, complement in cons)
/// - `x instanceof Foo` (positive in cons, complement in alt)
/// - `x == null` / `x != null` / `x === null` / ... (only positive side
///   recorded; the negative side's `Option<T>` is correct as-is)
/// - `x?.v !== undefined` / `x?.v !== null` — OptChain non-null invariant:
///   narrows the **base** (`x`) from `Option<T>` to `T` (positive side
///   only, same asymmetry as bare null check)
/// - Truthy `if (x)` on `Option<T>` (positive in cons only)
/// - Compound `a && b` (recurses on both legs; no complement, per
///   De Morgan)
pub fn detect_narrowing_guard<C: NarrowTypeContext>(
    test: &ast::Expr,
    consequent: &ast::Stmt,
    alternate: Option<&ast::Stmt>,
    ctx: &mut C,
) {
    let cons_span = Span::from_swc(consequent.span());
    let alt_span = alternate.map(|s| Span::from_swc(s.span()));

    match test {
        // Compound: a && b → detect narrowing from both sides.
        // Consequent narrowing is valid (both conditions are true in then-block).
        // Alternate/complement narrowing is NOT valid for individual sub-guards
        // (else means !(A && B) = !A || !B, so neither A nor B is guaranteed false).
        ast::Expr::Bin(bin) if matches!(bin.op, ast::BinaryOp::LogicalAnd) => {
            detect_narrowing_guard(&bin.left, consequent, None, ctx);
            detect_narrowing_guard(&bin.right, consequent, None, ctx);
        }
        ast::Expr::Bin(bin) => {
            let is_eq = matches!(bin.op, ast::BinaryOp::EqEqEq | ast::BinaryOp::EqEq);
            let is_neq = matches!(bin.op, ast::BinaryOp::NotEqEq | ast::BinaryOp::NotEq);

            // typeof narrowing
            if is_eq || is_neq {
                if let Some((var_name, narrowed_type, type_str)) =
                    extract_typeof_narrowing(bin, ctx)
                {
                    // === → positive in consequent, !== → positive in alternate
                    let positive_span = if is_eq { Some(cons_span) } else { alt_span };
                    // Complement goes to the opposite scope
                    let complement_span = if is_eq { alt_span } else { Some(cons_span) };

                    if let Some(span) = positive_span {
                        ctx.push_narrow_event(NarrowEvent::Narrow {
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
                            compute_complement_type(&var_name, &narrowed_type, ctx)
                        {
                            ctx.push_narrow_event(NarrowEvent::Narrow {
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
                    extract_null_check_narrowing(bin, ctx)
                {
                    // !== null → consequent, === null → alternate
                    let target_span = if is_neq { Some(cons_span) } else { alt_span };
                    if let Some(span) = target_span {
                        ctx.push_narrow_event(NarrowEvent::Narrow {
                            scope_start: span.lo,
                            scope_end: span.hi,
                            var_name,
                            narrowed_type,
                            trigger: NarrowTrigger::Primary(PrimaryTrigger::NullCheck(null_kind)),
                        });
                    }
                    // No complement for null check: the opposite scope has Option<T> which
                    // is correct in Rust (if-let else naturally handles None).
                }
                // OptChain null check: x?.v !== undefined → narrow base (x) to non-null.
                // Checked after bare-ident null check to avoid double-processing when
                // the condition is simply `x !== undefined` (bare Ident, not OptChain).
                else if let Some((var_name, narrowed_type)) =
                    extract_optchain_null_check_narrowing(bin, ctx)
                {
                    let target_span = if is_neq { Some(cons_span) } else { alt_span };
                    if let Some(span) = target_span {
                        ctx.push_narrow_event(NarrowEvent::Narrow {
                            scope_start: span.lo,
                            scope_end: span.hi,
                            var_name,
                            narrowed_type,
                            trigger: NarrowTrigger::Primary(PrimaryTrigger::OptChainInvariant),
                        });
                    }
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

                    ctx.push_narrow_event(NarrowEvent::Narrow {
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
                            compute_complement_type(&var_name, &narrowed_type, ctx)
                        {
                            ctx.push_narrow_event(NarrowEvent::Narrow {
                                scope_start: span.lo,
                                scope_end: span.hi,
                                var_name,
                                narrowed_type: complement,
                                trigger: NarrowTrigger::Primary(PrimaryTrigger::InstanceofGuard(
                                    class_name,
                                )),
                            });
                        }
                    }
                }
            }
        }
        // Truthy check: if (x) where x is Option<T> → narrow to T
        ast::Expr::Ident(ident) => {
            let var_name = ident.sym.to_string();
            if let ResolvedType::Known(RustType::Option(inner)) = ctx.lookup_var(&var_name) {
                ctx.push_narrow_event(NarrowEvent::Narrow {
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

/// Detects complement narrowing in the fall-through scope after an
/// always-exiting `if` block (early-return pattern, T11).
///
/// When `if (guard) { return / throw / break / continue; }` is followed
/// by more code in the enclosing block, the code after the `if`
/// benefits from the *complement* of `guard`. The complement scope is
/// `[if_end, block_end)`.
///
/// Handled shapes mirror [`detect_narrowing_guard`]'s positive side:
///
/// - `typeof x === "..."` → opposite-variant complement
/// - `x === null` → `x` is `T` (non-null) after the exit
/// - `x?.v === undefined` → `x` is `T` (non-null) after the exit
///   (OptChain invariant: if `x` were null, `x?.v === undefined`
///   would be true, triggering the exit)
/// - `x instanceof Foo` → complement variants after the exit
/// - `!x` → `x` is `T` (non-null) after the exit
pub fn detect_early_return_narrowing<C: NarrowTypeContext>(
    test: &ast::Expr,
    if_end: u32,
    block_end: u32,
    ctx: &mut C,
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
                    extract_typeof_narrowing(bin, ctx)
                {
                    let complement_after = if is_eq {
                        compute_complement_type(&var_name, &positive_type, ctx)
                    } else {
                        Some(positive_type)
                    };
                    if let Some(narrowed_type) = complement_after {
                        ctx.push_narrow_event(NarrowEvent::Narrow {
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
                    extract_null_check_narrowing(bin, ctx)
                {
                    ctx.push_narrow_event(NarrowEvent::Narrow {
                        scope_start: if_end,
                        scope_end: block_end,
                        var_name,
                        narrowed_type: unwrapped_type,
                        trigger: NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::NullCheck(
                            null_kind,
                        )),
                    });
                    return;
                }
                // OptChain early return: if (x?.v === undefined) { return; }
                // → x is non-null after (if x were null, chain → undefined → exited)
                if let Some((var_name, unwrapped_type)) =
                    extract_optchain_null_check_narrowing(bin, ctx)
                {
                    ctx.push_narrow_event(NarrowEvent::Narrow {
                        scope_start: if_end,
                        scope_end: block_end,
                        var_name,
                        narrowed_type: unwrapped_type,
                        trigger: NarrowTrigger::EarlyReturnComplement(
                            PrimaryTrigger::OptChainInvariant,
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
                        compute_complement_type(&var_name, &positive_type, ctx)
                    {
                        ctx.push_narrow_event(NarrowEvent::Narrow {
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
        //
        // Peek-through (P3a): outer wrappers like `as` / `!` / `<T>` /
        // `as const` / parens are runtime-no-op. `if (!(x as T))` must
        // narrow `x` identically to `if (!x)`.
        //
        // OptChain (P3b): `if (!x?.v) { return; }` also narrows the base
        // `x` from `Option<T>` to `T` after the exit. The invariant is
        // symmetric to `x?.v !== undefined` (T6-4): if `x` were null /
        // undefined, `x?.v` short-circuits to `undefined`, `!undefined`
        // is true, so the early-exit fires; reaching the fall-through
        // therefore proves `x` is non-null.
        ast::Expr::Unary(unary) if unary.op == ast::UnaryOp::Bang => {
            let peeled = peek_through_type_assertions(unary.arg.as_ref());
            if let ast::Expr::Ident(ident) = peeled {
                let var_name = ident.sym.to_string();
                if let ResolvedType::Known(RustType::Option(inner)) = ctx.lookup_var(&var_name) {
                    ctx.push_narrow_event(NarrowEvent::Narrow {
                        scope_start: if_end,
                        scope_end: block_end,
                        var_name,
                        narrowed_type: inner.as_ref().clone(),
                        trigger: NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::Truthy),
                    });
                }
            } else if let Some(base_ident) = narrowing_patterns::extract_optchain_base_ident(peeled)
            {
                let var_name = base_ident.sym.to_string();
                if let Some(inner) = unwrap_option_type(&var_name, ctx) {
                    ctx.push_narrow_event(NarrowEvent::Narrow {
                        scope_start: if_end,
                        scope_end: block_end,
                        var_name,
                        narrowed_type: inner,
                        trigger: NarrowTrigger::EarlyReturnComplement(
                            PrimaryTrigger::OptChainInvariant,
                        ),
                    });
                }
            }
        }
        // Truthy: if (x) { return; } → x is null/None after (no useful narrowing)
        // The variable stays as Option<T> which is correct.
        _ => {}
    }
}

// -----------------------------------------------------------------------------
// Helpers (all pure over `NarrowTypeContext`)
// -----------------------------------------------------------------------------

/// Classifies a binary operator + RHS shape into the [`NullCheckKind`]
/// variant that represents the check precisely.
///
/// - Loose equality (`==` / `!=`) always maps to the `EqNull` /
///   `NotEqNull` variants because JS coerces `null` and `undefined`
///   together under loose comparison.
/// - Strict equality (`===` / `!==`) distinguishes `null` from
///   `undefined` based on the RHS (caller-supplied): strict variants
///   are populated only when the RHS is the `undefined` identifier.
///
/// # Panics
///
/// Panics if `op` is not a null-check operator (`==` / `!=` / `===` /
/// `!==`). Callers must verify the operator before invoking this
/// helper — this is a structural contract and a silent wrong-value
/// fallback would mask bugs.
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
/// Does NOT match `RustType::Any` — Any-typed variants (e.g., "Object"
/// in any-narrowing enums) are matched via exact variant name in
/// [`typeof_to_variant_name`], not by data type.
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

/// Extracts a typeof-narrowing triple `(var_name, narrowed_type,
/// typeof_string)` from a binary comparison, if the binary matches
/// `typeof x === "T"` / `typeof x !== "T"` / reversed orderings.
///
/// Returns `None` if the expression is not a typeof comparison or the
/// typeof operand is not a bare identifier.
fn extract_typeof_narrowing<C: NarrowTypeContext>(
    bin: &ast::BinExpr,
    ctx: &C,
) -> Option<(String, RustType, String)> {
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
            let (name, ty) = resolve_typeof_narrowed_type_from_var(&var_name, &type_str, ctx)?;
            return Some((name, ty, type_str));
        }
        _ => return None,
    };
    Some((var_name, narrowed_type, type_str))
}

/// Resolves the narrowed type for typeof "object"/"function" by looking
/// up the variable's union enum variants.
fn resolve_typeof_narrowed_type_from_var<C: NarrowTypeContext>(
    var_name: &str,
    type_str: &str,
    ctx: &C,
) -> Option<(String, RustType)> {
    let var_type = ctx.lookup_var(var_name);
    let enum_name = match &var_type {
        ResolvedType::Known(RustType::Named { name, .. }) => name.clone(),
        _ => return None,
    };
    let variants = ctx.synthetic_enum_variants(&enum_name)?;
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
/// returns the type(s) remaining after excluding the positive type's
/// variant.
///
/// - 2-variant union: returns the other variant's data type
/// - 3+ variant union: generates a sub-union enum from remaining variants
/// - Non-union or non-enum types: returns `None`
fn compute_complement_type<C: NarrowTypeContext>(
    var_name: &str,
    positive_type: &RustType,
    ctx: &mut C,
) -> Option<RustType> {
    let var_type = ctx.lookup_var(var_name);
    let enum_name = match &var_type {
        ResolvedType::Known(RustType::Named { name, .. }) => name.clone(),
        _ => return None,
    };

    let variants = ctx.synthetic_enum_variants(&enum_name)?;

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
            let sub_union_name = ctx.register_sub_union(&remaining_types);
            Some(RustType::Named {
                name: sub_union_name,
                type_args: vec![],
            })
        }
    }
}

/// Extracts the non-nullish side and `undefined` discriminant from a
/// binary comparison that has `null` or `undefined` on one side.
///
/// Returns `(non_null_expr, rhs_is_undefined)` where:
/// - `non_null_expr` is the expression on the other side of
///   `null`/`undefined`
/// - `rhs_is_undefined` is `true` when the null-side is the
///   `undefined` identifier (not `null` literal)
///
/// Handles both orderings: `x !== null` and `null !== x`.
fn extract_non_nullish_side(bin: &ast::BinExpr) -> Option<(&ast::Expr, bool)> {
    if narrowing_patterns::is_null_or_undefined(&bin.right) {
        Some((
            bin.left.as_ref(),
            narrowing_patterns::is_undefined_ident(&bin.right),
        ))
    } else if narrowing_patterns::is_null_or_undefined(&bin.left) {
        Some((
            bin.right.as_ref(),
            narrowing_patterns::is_undefined_ident(&bin.left),
        ))
    } else {
        None
    }
}

/// Looks up a variable in `ctx` and unwraps `Option<T>` to `T`.
///
/// Returns `None` if the variable is not `Option<_>`.
fn unwrap_option_type<C: NarrowTypeContext>(var_name: &str, ctx: &C) -> Option<RustType> {
    match ctx.lookup_var(var_name) {
        ResolvedType::Known(RustType::Option(inner)) => Some(inner.as_ref().clone()),
        _ => None,
    }
}

/// Extracts a null-check narrowing triple from a binary comparison.
///
/// Handles both `x === null` / `x !== null` and the reversed
/// `null === x`, plus `undefined` on either side. Returns the
/// unwrapped `Option<T>` payload type so callers can emit a narrow
/// targeting `T`.
fn extract_null_check_narrowing<C: NarrowTypeContext>(
    bin: &ast::BinExpr,
    ctx: &C,
) -> Option<(String, RustType, NullCheckKind)> {
    let (var_expr, rhs_is_undefined) = extract_non_nullish_side(bin)?;

    let var_name = match var_expr {
        ast::Expr::Ident(ident) => ident.sym.to_string(),
        _ => return None,
    };

    let inner = unwrap_option_type(&var_name, ctx)?;
    Some((
        var_name,
        inner,
        classify_null_check(bin.op, rhs_is_undefined),
    ))
}

/// Extracts an OptChain-based null-check narrowing from a binary
/// comparison.
///
/// Matches `x?.prop !== undefined` (and reversed `undefined !== x?.prop`)
/// and returns the **base** identifier of the OptChain (e.g., `x` from
/// `x?.v`) and the unwrapped `Option<T>` payload type.
///
/// **Invariant**: if `x?.prop !== undefined` is true, `x` must be
/// non-null — when `x` is null/undefined the optional chain
/// short-circuits to `undefined`, which fails the `!==` check. This
/// allows narrowing the base from `Option<T>` to `T` in the positive
/// scope.
///
/// Does **not** return [`NullCheckKind`] — callers always use
/// [`PrimaryTrigger::OptChainInvariant`] which carries no null-check
/// classification payload (the distinction between strict/loose and
/// null/undefined is irrelevant for OptChain base narrowing).
fn extract_optchain_null_check_narrowing<C: NarrowTypeContext>(
    bin: &ast::BinExpr,
    ctx: &C,
) -> Option<(String, RustType)> {
    let (chain_expr, _rhs_is_undefined) = extract_non_nullish_side(bin)?;

    let base_ident = narrowing_patterns::extract_optchain_base_ident(chain_expr)?;
    let var_name = base_ident.sym.to_string();

    let inner = unwrap_option_type(&var_name, ctx)?;
    Some((var_name, inner))
}
