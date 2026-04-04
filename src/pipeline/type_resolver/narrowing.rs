//! Narrowing detection for TypeResolver.
//!
//! Detects type narrowing guards in `if` conditions (typeof, instanceof, null checks)
//! and records [`NarrowingEvent`]s for the Transformer.

use swc_common::Spanned;
use swc_ecma_ast as ast;

use super::*;
use crate::pipeline::narrowing_patterns;
use crate::pipeline::type_resolution::{NarrowingEvent, Span};

impl<'a> TypeResolver<'a> {
    /// Detects narrowing guards in `if` conditions and records [`NarrowingEvent`]s.
    ///
    /// NarrowingEvent always records the **positive narrowed type** (e.g., `String` for
    /// `typeof x === "string"`, unwrapped `T` for `x !== null` where `x: Option<T>`).
    /// The event is placed in whichever scope the positive type is guaranteed:
    ///
    /// - `typeof x === "T"` / `x !== null` / `instanceof` / truthy → **consequent**
    /// - `typeof x !== "T"` / `x === null` → **alternate** (positive type holds in else)
    ///
    /// Complement narrowing (e.g., "not String" in then-block of `typeof x !== "string"`)
    /// is not yet supported — tracked in I-213 (Batch 5b).
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
            // Alternate narrowing is NOT valid for individual sub-guards
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
                        // === → consequent, !== → alternate
                        let target_span = if is_eq { Some(cons_span) } else { alt_span };
                        if let Some(span) = target_span {
                            self.result.narrowing_events.push(NarrowingEvent {
                                scope_start: span.lo,
                                scope_end: span.hi,
                                var_name,
                                narrowed_type,
                            });
                        }
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
                    }
                }

                // x instanceof Foo
                if matches!(bin.op, ast::BinaryOp::InstanceOf) {
                    if let (ast::Expr::Ident(var_ident), ast::Expr::Ident(class_ident)) =
                        (bin.left.as_ref(), bin.right.as_ref())
                    {
                        self.result.narrowing_events.push(NarrowingEvent {
                            scope_start: cons_span.lo,
                            scope_end: cons_span.hi,
                            var_name: var_ident.sym.to_string(),
                            narrowed_type: RustType::Named {
                                name: class_ident.sym.to_string(),
                                type_args: vec![],
                            },
                        });
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
                }
            }
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
        let narrowed_type = match type_str.as_str() {
            "string" => RustType::String,
            "number" => RustType::F64,
            "boolean" => RustType::Bool,
            _ => return None,
        };
        Some((var_name, narrowed_type))
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
