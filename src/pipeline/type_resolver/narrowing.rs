//! Narrowing detection for TypeResolver.
//!
//! Detects type narrowing guards in `if` conditions (typeof, instanceof, null checks)
//! and records [`NarrowingEvent`]s for the Transformer.

use swc_common::Spanned;
use swc_ecma_ast as ast;

use super::*;
use crate::pipeline::type_resolution::{NarrowingEvent, Span};

impl<'a> TypeResolver<'a> {
    pub(super) fn detect_narrowing_guard(&mut self, test: &ast::Expr, consequent: &ast::Stmt) {
        let scope_span = Span::from_swc(consequent.span());

        match test {
            // Compound: a && b → detect narrowing from both sides
            ast::Expr::Bin(bin) if matches!(bin.op, ast::BinaryOp::LogicalAnd) => {
                self.detect_narrowing_guard(&bin.left, consequent);
                self.detect_narrowing_guard(&bin.right, consequent);
            }
            ast::Expr::Bin(bin) => {
                // typeof x === "string"
                if matches!(bin.op, ast::BinaryOp::EqEqEq | ast::BinaryOp::EqEq) {
                    if let Some((var_name, narrowed_type)) = self.extract_typeof_narrowing(bin) {
                        self.result.narrowing_events.push(NarrowingEvent {
                            scope_start: scope_span.lo,
                            scope_end: scope_span.hi,
                            var_name,
                            narrowed_type,
                        });
                    }
                }
                // x !== null
                if matches!(bin.op, ast::BinaryOp::NotEqEq | ast::BinaryOp::NotEq) {
                    if let Some((var_name, narrowed_type)) = self.extract_null_check_narrowing(bin)
                    {
                        self.result.narrowing_events.push(NarrowingEvent {
                            scope_start: scope_span.lo,
                            scope_end: scope_span.hi,
                            var_name,
                            narrowed_type,
                        });
                    }
                }
                // x instanceof Foo
                if matches!(bin.op, ast::BinaryOp::InstanceOf) {
                    if let (ast::Expr::Ident(var_ident), ast::Expr::Ident(class_ident)) =
                        (bin.left.as_ref(), bin.right.as_ref())
                    {
                        self.result.narrowing_events.push(NarrowingEvent {
                            scope_start: scope_span.lo,
                            scope_end: scope_span.hi,
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
                        scope_start: scope_span.lo,
                        scope_end: scope_span.hi,
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
        let (typeof_expr, type_str) = self.extract_typeof_and_string(bin)?;
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

    fn extract_typeof_and_string<'b>(
        &self,
        bin: &'b ast::BinExpr,
    ) -> Option<(&'b ast::Expr, String)> {
        // typeof x === "string"
        if let ast::Expr::Unary(unary) = bin.left.as_ref() {
            if matches!(unary.op, ast::UnaryOp::TypeOf) {
                if let ast::Expr::Lit(ast::Lit::Str(s)) = bin.right.as_ref() {
                    return Some((&unary.arg, s.value.to_string_lossy().into_owned()));
                }
            }
        }
        // "string" === typeof x
        if let ast::Expr::Unary(unary) = bin.right.as_ref() {
            if matches!(unary.op, ast::UnaryOp::TypeOf) {
                if let ast::Expr::Lit(ast::Lit::Str(s)) = bin.left.as_ref() {
                    return Some((&unary.arg, s.value.to_string_lossy().into_owned()));
                }
            }
        }
        None
    }

    fn extract_null_check_narrowing(&self, bin: &ast::BinExpr) -> Option<(String, RustType)> {
        // x !== null → remove Option wrapper from x's type
        let (var_expr, is_null) = if is_null_literal(&bin.right) {
            (bin.left.as_ref(), true)
        } else if is_null_literal(&bin.left) {
            (bin.right.as_ref(), true)
        } else {
            return None;
        };

        if !is_null {
            return None;
        }

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
