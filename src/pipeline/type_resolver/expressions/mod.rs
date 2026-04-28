//! Expression type resolution dispatcher.
//!
//! [`TypeResolver::resolve_expr`] is the public entry; it stores the result in
//! `expr_types` and delegates to the private [`Self::resolve_expr_inner`] dispatcher.
//! Each non-trivial AST variant is handled in a dedicated submodule:
//!
//! | Variant | Submodule |
//! |---------|-----------|
//! | `Bin` | [`binary`] |
//! | `Assign` | [`assignments`] |
//! | `Member` | [`member`] |
//! | `Object` | [`object`] |
//! | `Cond` | [`conditional`] |
//! | `TsAs` / `TsTypeAssertion` / `TsNonNull` | [`assertions`] |
//! | `OptChain` | [`opt_chain`] |
//! | `New` | [`new_expr`] |
//! | `Call` | [`super::call_resolution`] |
//! | `Arrow` / `Fn` / `Array` | [`super::fn_exprs`] |
//!
//! Trivial / transparent variants (`Ident` / `Lit` / `Tpl` / `Paren` / `Unary` /
//! `Update` / `This` / `Seq` / `Class` / `Await` / `TsConstAssertion`) stay inline
//! in this dispatcher.

use swc_common::Spanned;
use swc_ecma_ast as ast;

use super::*;
use crate::pipeline::type_resolution::Span;

mod assertions;
mod assignments;
mod binary;
mod conditional;
mod member;
mod new_expr;
mod object;
mod opt_chain;

impl<'a> TypeResolver<'a> {
    /// 式の型を解決し、Known な結果を `expr_types` に記録する。
    ///
    /// 全ての部分式の型が `expr_types` に蓄積されるため、Transformer は
    /// `get_expr_type(tctx, expr)` だけで任意の式の型を取得できる。
    pub(super) fn resolve_expr(&mut self, expr: &ast::Expr) -> ResolvedType {
        let ty = self.resolve_expr_inner(expr);
        if matches!(ty, ResolvedType::Known(_)) {
            let span = Span::from_swc(expr.span());
            self.result
                .expr_types
                .entry(span)
                .or_insert_with(|| ty.clone());
        }
        ty
    }

    fn resolve_expr_inner(&mut self, expr: &ast::Expr) -> ResolvedType {
        match expr {
            ast::Expr::Ident(ident) => self.lookup_var(ident.sym.as_ref()),
            ast::Expr::Lit(ast::Lit::Str(_)) => ResolvedType::Known(RustType::String),
            ast::Expr::Lit(ast::Lit::Num(_)) => ResolvedType::Known(RustType::F64),
            ast::Expr::Lit(ast::Lit::Bool(_)) => ResolvedType::Known(RustType::Bool),
            ast::Expr::Lit(ast::Lit::Null(_)) => {
                ResolvedType::Known(RustType::Option(Box::new(RustType::Any)))
            }
            ast::Expr::Tpl(tpl) => {
                // Template literal: recursively resolve each interpolated expression
                // so `expr_types` contains entries for inner sub-expressions.
                // Without this, downstream lookups (e.g. `is_du_field_binding`
                // checking `get_expr_type(&event)` inside `` `${event.x}` ``)
                // return Unknown and fall through to raw member access emission.
                for expr in &tpl.exprs {
                    self.resolve_expr(expr);
                }
                ResolvedType::Known(RustType::String)
            }
            ast::Expr::TaggedTpl(tagged) => {
                // Tagged template: recurse into tag and interpolated exprs for
                // consistency with Tpl. The tag's return type is not analyzed
                // here (converter treats TaggedTpl as unsupported — I-110), so
                // the overall type is Unknown.
                self.resolve_expr(&tagged.tag);
                for expr in &tagged.tpl.exprs {
                    self.resolve_expr(expr);
                }
                ResolvedType::Unknown
            }
            ast::Expr::Bin(bin) => self.resolve_bin_expr(bin),
            ast::Expr::Member(member) => self.resolve_member_expr(member),
            ast::Expr::Call(call) => self.resolve_call_expr(call),
            ast::Expr::New(new_expr) => self.resolve_new_expr(new_expr),
            ast::Expr::Paren(paren) => self.resolve_expr(&paren.expr),
            ast::Expr::TsAs(ts_as) => self.resolve_ts_as_expr(ts_as),
            ast::Expr::Array(arr) => self.resolve_array_expr(arr),
            ast::Expr::Arrow(arrow) => self.resolve_arrow_expr(arrow),
            ast::Expr::Fn(fn_expr) => self.resolve_fn_expr(fn_expr),
            ast::Expr::Assign(assign) => self.resolve_assign_expr(assign),
            ast::Expr::Cond(cond) => self.resolve_cond_expr(cond),
            ast::Expr::Unary(unary) => {
                // Resolve operand to register its expr_type (used by Transformer
                // for typeof/unary plus operand type decisions)
                self.resolve_expr(&unary.arg);
                match unary.op {
                    ast::UnaryOp::TypeOf => ResolvedType::Known(RustType::String),
                    ast::UnaryOp::Bang => ResolvedType::Known(RustType::Bool),
                    ast::UnaryOp::Minus | ast::UnaryOp::Plus => ResolvedType::Known(RustType::F64),
                    _ => ResolvedType::Unknown,
                }
            }
            ast::Expr::Await(await_expr) => self.resolve_expr(&await_expr.arg),
            ast::Expr::TsNonNull(ts_non_null) => self.resolve_ts_non_null_expr(ts_non_null),
            ast::Expr::TsTypeAssertion(assertion) => self.resolve_ts_type_assertion_expr(assertion),
            ast::Expr::TsConstAssertion(const_assertion) => {
                // x as const — return inner expression's type
                self.resolve_expr(&const_assertion.expr)
            }
            ast::Expr::Object(obj) => self.resolve_object_expr(obj),
            ast::Expr::OptChain(opt) => self.resolve_opt_chain_expr(opt),
            ast::Expr::Update(_) => {
                // i++ / i-- → f64
                ResolvedType::Known(RustType::F64)
            }
            ast::Expr::This(_) => {
                // `this` — resolve from scope (registered by visit_class_decl)
                self.lookup_var("this")
            }
            ast::Expr::Seq(seq) => {
                // Comma expression: evaluate all, return last
                let mut last = ResolvedType::Unknown;
                for expr in &seq.exprs {
                    let span = Span::from_swc(expr.span());
                    let ty = self.resolve_expr(expr);
                    self.result.expr_types.insert(span, ty.clone());
                    last = ty;
                }
                last
            }
            ast::Expr::Class(class_expr) => {
                // Class expression: `const C = class Foo { ... }` or `const C = class { ... }`
                let class_name = class_expr
                    .ident
                    .as_ref()
                    .map(|id| id.sym.to_string())
                    .unwrap_or_default();
                let class_span = class_expr
                    .ident
                    .as_ref()
                    .map(|id| Span::from_swc(id.span))
                    .unwrap_or_else(|| Span::from_swc(class_expr.class.span));
                self.visit_class_body(&class_expr.class, &class_name, class_span);
                ResolvedType::Unknown
            }
            _ => ResolvedType::Unknown,
        }
    }
}
