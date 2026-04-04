//! Narrowing detection for TypeResolver.
//!
//! Detects type narrowing guards in `if` conditions (typeof, instanceof, null checks)
//! and records [`NarrowingEvent`]s for the Transformer.
//!
//! Records both **positive** narrowing (the type the variable IS in the guarded scope)
//! and **complement** narrowing (the type the variable is in the opposite scope,
//! computed by excluding the positive type from the union).

use swc_common::Spanned;
use swc_ecma_ast as ast;

use super::*;
use crate::pipeline::narrowing_patterns;
use crate::pipeline::type_resolution::{NarrowingEvent, Span};

/// Returns true if the statement always exits the enclosing scope
/// (return, throw, break, continue).
///
/// For block statements, checks the last statement recursively.
/// For if statements, both branches must always exit.
pub(super) fn block_always_exits(stmt: &ast::Stmt) -> bool {
    match stmt {
        ast::Stmt::Return(_) | ast::Stmt::Throw(_) => true,
        ast::Stmt::Break(_) | ast::Stmt::Continue(_) => true,
        ast::Stmt::Block(block) => block.stmts.last().is_some_and(block_always_exits),
        ast::Stmt::If(if_stmt) => {
            let cons_exits = block_always_exits(&if_stmt.cons);
            let alt_exits = if_stmt.alt.as_ref().is_some_and(|s| block_always_exits(s));
            cons_exits && alt_exits
        }
        _ => false,
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
    /// Detects narrowing guards in `if` conditions and records [`NarrowingEvent`]s.
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
                    if let Some((var_name, narrowed_type)) = self.extract_typeof_narrowing(bin) {
                        // === → positive in consequent, !== → positive in alternate
                        let positive_span = if is_eq { Some(cons_span) } else { alt_span };
                        // Complement goes to the opposite scope
                        let complement_span = if is_eq { alt_span } else { Some(cons_span) };

                        if let Some(span) = positive_span {
                            self.result.narrowing_events.push(NarrowingEvent {
                                scope_start: span.lo,
                                scope_end: span.hi,
                                var_name: var_name.clone(),
                                narrowed_type: narrowed_type.clone(),
                            });
                        }

                        // Record complement narrowing in the opposite scope
                        if let Some(span) = complement_span {
                            if let Some(complement) =
                                self.compute_complement_type(&var_name, &narrowed_type)
                            {
                                self.result.narrowing_events.push(NarrowingEvent {
                                    scope_start: span.lo,
                                    scope_end: span.hi,
                                    var_name,
                                    narrowed_type: complement,
                                });
                            }
                        }
                        // typeof was handled; skip null check below to avoid double-processing
                        return;
                    }
                }

                // null/undefined narrowing
                if is_eq || is_neq {
                    if let Some((var_name, narrowed_type)) = self.extract_null_check_narrowing(bin)
                    {
                        // !== null → consequent, === null → alternate
                        let target_span = if is_neq { Some(cons_span) } else { alt_span };
                        if let Some(span) = target_span {
                            self.result.narrowing_events.push(NarrowingEvent {
                                scope_start: span.lo,
                                scope_end: span.hi,
                                var_name,
                                narrowed_type,
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
                        let narrowed_type = RustType::Named {
                            name: class_ident.sym.to_string(),
                            type_args: vec![],
                        };

                        self.result.narrowing_events.push(NarrowingEvent {
                            scope_start: cons_span.lo,
                            scope_end: cons_span.hi,
                            var_name: var_name.clone(),
                            narrowed_type: narrowed_type.clone(),
                        });

                        // Complement narrowing in else
                        if let Some(span) = alt_span {
                            if let Some(complement) =
                                self.compute_complement_type(&var_name, &narrowed_type)
                            {
                                self.result.narrowing_events.push(NarrowingEvent {
                                    scope_start: span.lo,
                                    scope_end: span.hi,
                                    var_name,
                                    narrowed_type: complement,
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
                    self.result.narrowing_events.push(NarrowingEvent {
                        scope_start: cons_span.lo,
                        scope_end: cons_span.hi,
                        var_name,
                        narrowed_type: inner.as_ref().clone(),
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
                    if let Some((var_name, positive_type)) = self.extract_typeof_narrowing(bin) {
                        let complement_after = if is_eq {
                            self.compute_complement_type(&var_name, &positive_type)
                        } else {
                            Some(positive_type)
                        };
                        if let Some(narrowed_type) = complement_after {
                            self.result.narrowing_events.push(NarrowingEvent {
                                scope_start: if_end,
                                scope_end: block_end,
                                var_name,
                                narrowed_type,
                            });
                        }
                        return;
                    }
                }

                // null check early return: if (x === null) { return; }
                // → x is non-null after → unwrapped Option
                if is_eq {
                    if let Some((var_name, unwrapped_type)) = self.extract_null_check_narrowing(bin)
                    {
                        self.result.narrowing_events.push(NarrowingEvent {
                            scope_start: if_end,
                            scope_end: block_end,
                            var_name,
                            narrowed_type: unwrapped_type,
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
                        let positive_type = RustType::Named {
                            name: class_ident.sym.to_string(),
                            type_args: vec![],
                        };
                        if let Some(complement) =
                            self.compute_complement_type(&var_name, &positive_type)
                        {
                            self.result.narrowing_events.push(NarrowingEvent {
                                scope_start: if_end,
                                scope_end: block_end,
                                var_name,
                                narrowed_type: complement,
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
                        self.result.narrowing_events.push(NarrowingEvent {
                            scope_start: if_end,
                            scope_end: block_end,
                            var_name,
                            narrowed_type: inner.as_ref().clone(),
                        });
                    }
                }
            }
            // Truthy: if (x) { return; } → x is null/None after (no useful narrowing)
            // The variable stays as Option<T> which is correct.
            _ => {}
        }
    }

    fn extract_typeof_narrowing(&self, bin: &ast::BinExpr) -> Option<(String, RustType)> {
        // typeof x === "string" → (x, String)
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
                return self.resolve_typeof_narrowed_type_from_var(&var_name, &type_str);
            }
            _ => return None,
        };
        Some((var_name, narrowed_type))
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

    fn extract_null_check_narrowing(&self, bin: &ast::BinExpr) -> Option<(String, RustType)> {
        // x !== null / x !== undefined → remove Option wrapper from x's type
        let var_expr = if narrowing_patterns::is_null_or_undefined(&bin.right) {
            bin.left.as_ref()
        } else if narrowing_patterns::is_null_or_undefined(&bin.left) {
            bin.right.as_ref()
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
            ResolvedType::Known(RustType::Option(inner)) => {
                Some((var_name, inner.as_ref().clone()))
            }
            _ => None,
        }
    }
}
