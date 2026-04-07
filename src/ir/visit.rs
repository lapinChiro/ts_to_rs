//! IR 走査用の read-only visitor trait。
//!
//! `IrVisitor` は IR 全要素（`Item`, `Stmt`, `Expr`, `RustType`, `Pattern`,
//! `MatchArm`, `TypeParam`, `Method`）への再帰走査を単一の共通骨格として提供する。
//! 従来は `collect_type_refs_*`（walker）, `expr_contains_runtime_typeof`,
//! `items_contain_regex` 等の複数の手書き再帰実装が独立に存在し、新しい IR
//! variant 追加時に全箇所を個別更新する必要があったが、本 trait により
//! 共通の `walk_*` 関数に集約される。
//!
//! # 使い方
//!
//! ```ignore
//! use crate::ir::visit::{IrVisitor, walk_expr};
//! use crate::ir::Expr;
//!
//! struct RuntimeTypeofDetector { found: bool }
//!
//! impl IrVisitor for RuntimeTypeofDetector {
//!     fn visit_expr(&mut self, expr: &Expr) {
//!         if matches!(expr, Expr::RuntimeTypeof { .. }) {
//!             self.found = true;
//!             return;
//!         }
//!         walk_expr(self, expr);
//!     }
//! }
//! ```
//!
//! 各 `visit_*` はデフォルトで対応する `walk_*` に委譲し、実装者は必要な
//! ノードだけ override する。`walk_*` は `?Sized` 境界で dyn 互換性を保つ。

use super::{
    AssocConst, CallTarget, ClosureBody, Expr, Item, MatchArm, Method, Pattern, RustType, Stmt,
    TypeParam, UserTypeRef,
};

/// IR の read-only 走査用 visitor trait。
///
/// 各 `visit_*` はデフォルトで同名の `walk_*` 関数に委譲する。実装者は
/// 必要なメソッドだけ override し、子ノードへの再帰が必要な場合は明示的に
/// `walk_*` を呼ぶ。
pub trait IrVisitor {
    fn visit_item(&mut self, item: &Item) {
        walk_item(self, item);
    }
    fn visit_stmt(&mut self, stmt: &Stmt) {
        walk_stmt(self, stmt);
    }
    fn visit_expr(&mut self, expr: &Expr) {
        walk_expr(self, expr);
    }
    fn visit_rust_type(&mut self, ty: &RustType) {
        walk_rust_type(self, ty);
    }
    fn visit_pattern(&mut self, pat: &Pattern) {
        walk_pattern(self, pat);
    }
    fn visit_match_arm(&mut self, arm: &MatchArm) {
        walk_match_arm(self, arm);
    }
    fn visit_type_param(&mut self, tp: &TypeParam) {
        walk_type_param(self, tp);
    }
    fn visit_method(&mut self, m: &Method) {
        walk_method(self, m);
    }
    /// User-defined type 参照を通知する。
    ///
    /// 以下の経路から発火する:
    /// - `walk_expr` の `Expr::EnumVariant::enum_ty`
    /// - `walk_call_target` の `CallTarget::UserAssocFn::ty`
    /// - `walk_call_target` の `CallTarget::UserTupleCtor::0`
    /// - `walk_call_target` の `CallTarget::UserEnumVariantCtor::enum_ty`
    ///
    /// walker の実装は本フックを override するだけで refs グラフを一様に
    /// 構築できる (`external_struct_generator::TypeRefCollector` がその例)。
    /// builtin variant / プリミティブ / std module path / 自由関数は型レベルで
    /// `UserTypeRef` フィールドを持たないため、本フックは構造的に発火しない。
    fn visit_user_type_ref(&mut self, _r: &UserTypeRef) {}
}

/// `Item` の全 variant を再帰的に走査する。
pub fn walk_item<V: IrVisitor + ?Sized>(v: &mut V, item: &Item) {
    match item {
        Item::Struct {
            type_params,
            fields,
            ..
        } => {
            for tp in type_params {
                v.visit_type_param(tp);
            }
            for field in fields {
                v.visit_rust_type(&field.ty);
            }
        }
        Item::Enum {
            type_params,
            variants,
            ..
        } => {
            for tp in type_params {
                v.visit_type_param(tp);
            }
            for variant in variants {
                if let Some(data) = &variant.data {
                    v.visit_rust_type(data);
                }
                for field in &variant.fields {
                    v.visit_rust_type(&field.ty);
                }
            }
        }
        Item::Trait {
            type_params,
            supertraits,
            methods,
            ..
        } => {
            for tp in type_params {
                v.visit_type_param(tp);
            }
            for supertrait in supertraits {
                for arg in &supertrait.type_args {
                    v.visit_rust_type(arg);
                }
            }
            for method in methods {
                v.visit_method(method);
            }
        }
        Item::Impl {
            type_params,
            for_trait,
            consts,
            methods,
            ..
        } => {
            for tp in type_params {
                v.visit_type_param(tp);
            }
            if let Some(tref) = for_trait {
                for arg in &tref.type_args {
                    v.visit_rust_type(arg);
                }
            }
            for c in consts {
                walk_assoc_const(v, c);
            }
            for method in methods {
                v.visit_method(method);
            }
        }
        Item::TypeAlias {
            type_params, ty, ..
        } => {
            for tp in type_params {
                v.visit_type_param(tp);
            }
            v.visit_rust_type(ty);
        }
        Item::Fn {
            type_params,
            params,
            return_type,
            body,
            ..
        } => {
            for tp in type_params {
                v.visit_type_param(tp);
            }
            for param in params {
                if let Some(ty) = &param.ty {
                    v.visit_rust_type(ty);
                }
            }
            if let Some(rt) = return_type {
                v.visit_rust_type(rt);
            }
            for stmt in body {
                v.visit_stmt(stmt);
            }
        }
        Item::Comment(_) | Item::Use { .. } | Item::RawCode(_) => {}
    }
}

/// `AssocConst` を走査する（trait method ではないヘルパー）。
fn walk_assoc_const<V: IrVisitor + ?Sized>(v: &mut V, c: &AssocConst) {
    v.visit_rust_type(&c.ty);
    v.visit_expr(&c.value);
}

/// `Method` の全子ノード（params, return type, body）を走査する。
pub fn walk_method<V: IrVisitor + ?Sized>(v: &mut V, m: &Method) {
    for param in &m.params {
        if let Some(ty) = &param.ty {
            v.visit_rust_type(ty);
        }
    }
    if let Some(rt) = &m.return_type {
        v.visit_rust_type(rt);
    }
    if let Some(body) = &m.body {
        for stmt in body {
            v.visit_stmt(stmt);
        }
    }
}

/// `TypeParam` の制約を走査する。
pub fn walk_type_param<V: IrVisitor + ?Sized>(v: &mut V, tp: &TypeParam) {
    if let Some(constraint) = &tp.constraint {
        v.visit_rust_type(constraint);
    }
}

/// `Stmt` の全 variant を再帰的に走査する。
pub fn walk_stmt<V: IrVisitor + ?Sized>(v: &mut V, stmt: &Stmt) {
    match stmt {
        Stmt::Let { ty, init, .. } => {
            if let Some(t) = ty {
                v.visit_rust_type(t);
            }
            if let Some(e) = init {
                v.visit_expr(e);
            }
        }
        Stmt::If {
            condition,
            then_body,
            else_body,
        } => {
            v.visit_expr(condition);
            for s in then_body {
                v.visit_stmt(s);
            }
            if let Some(eb) = else_body {
                for s in eb {
                    v.visit_stmt(s);
                }
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            v.visit_expr(condition);
            for s in body {
                v.visit_stmt(s);
            }
        }
        Stmt::WhileLet {
            pattern,
            expr,
            body,
            ..
        } => {
            v.visit_pattern(pattern);
            v.visit_expr(expr);
            for s in body {
                v.visit_stmt(s);
            }
        }
        Stmt::ForIn { iterable, body, .. } => {
            v.visit_expr(iterable);
            for s in body {
                v.visit_stmt(s);
            }
        }
        Stmt::Loop { body, .. } | Stmt::LabeledBlock { body, .. } => {
            for s in body {
                v.visit_stmt(s);
            }
        }
        Stmt::Break { value, .. } => {
            if let Some(e) = value {
                v.visit_expr(e);
            }
        }
        Stmt::Continue { .. } => {}
        Stmt::Return(opt) => {
            if let Some(e) = opt {
                v.visit_expr(e);
            }
        }
        Stmt::Expr(e) | Stmt::TailExpr(e) => v.visit_expr(e),
        Stmt::IfLet {
            pattern,
            expr,
            then_body,
            else_body,
        } => {
            v.visit_pattern(pattern);
            v.visit_expr(expr);
            for s in then_body {
                v.visit_stmt(s);
            }
            if let Some(eb) = else_body {
                for s in eb {
                    v.visit_stmt(s);
                }
            }
        }
        Stmt::Match { expr, arms } => {
            v.visit_expr(expr);
            for arm in arms {
                v.visit_match_arm(arm);
            }
        }
    }
}

/// `Expr` の全 variant を再帰的に走査する。
pub fn walk_expr<V: IrVisitor + ?Sized>(v: &mut V, expr: &Expr) {
    match expr {
        Expr::Cast { expr, target } => {
            v.visit_expr(expr);
            v.visit_rust_type(target);
        }
        Expr::StructInit { fields, base, .. } => {
            for (_, e) in fields {
                v.visit_expr(e);
            }
            if let Some(b) = base {
                v.visit_expr(b);
            }
        }
        Expr::Closure {
            params,
            return_type,
            body,
        } => {
            for p in params {
                if let Some(t) = &p.ty {
                    v.visit_rust_type(t);
                }
            }
            if let Some(rt) = return_type {
                v.visit_rust_type(rt);
            }
            match body {
                ClosureBody::Expr(e) => v.visit_expr(e),
                ClosureBody::Block(stmts) => {
                    for s in stmts {
                        v.visit_stmt(s);
                    }
                }
            }
        }
        Expr::FieldAccess { object, .. } => v.visit_expr(object),
        Expr::MethodCall { object, args, .. } => {
            v.visit_expr(object);
            for a in args {
                v.visit_expr(a);
            }
        }
        Expr::Assign { target, value } => {
            v.visit_expr(target);
            v.visit_expr(value);
        }
        Expr::UnaryOp { operand, .. } => v.visit_expr(operand),
        Expr::BinaryOp { left, right, .. } => {
            v.visit_expr(left);
            v.visit_expr(right);
        }
        Expr::Range { start, end } => {
            if let Some(s) = start {
                v.visit_expr(s);
            }
            if let Some(e) = end {
                v.visit_expr(e);
            }
        }
        Expr::FnCall { target, args } => {
            walk_call_target(v, target);
            for a in args {
                v.visit_expr(a);
            }
        }
        Expr::Vec { elements } | Expr::Tuple { elements } => {
            for e in elements {
                v.visit_expr(e);
            }
        }
        Expr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            v.visit_expr(condition);
            v.visit_expr(then_expr);
            v.visit_expr(else_expr);
        }
        Expr::IfLet {
            pattern,
            expr,
            then_expr,
            else_expr,
        } => {
            v.visit_pattern(pattern);
            v.visit_expr(expr);
            v.visit_expr(then_expr);
            v.visit_expr(else_expr);
        }
        Expr::FormatMacro { args, .. } | Expr::MacroCall { args, .. } => {
            for a in args {
                v.visit_expr(a);
            }
        }
        Expr::Await(e) | Expr::Deref(e) | Expr::Ref(e) => v.visit_expr(e),
        Expr::Index { object, index } => {
            v.visit_expr(object);
            v.visit_expr(index);
        }
        Expr::RuntimeTypeof { operand } => v.visit_expr(operand),
        Expr::Matches { expr, pattern } => {
            v.visit_expr(expr);
            v.visit_pattern(pattern);
        }
        Expr::Block(stmts) => {
            for s in stmts {
                v.visit_stmt(s);
            }
        }
        Expr::Match { expr, arms } => {
            v.visit_expr(expr);
            for arm in arms {
                v.visit_match_arm(arm);
            }
        }
        Expr::EnumVariant { enum_ty, .. } => v.visit_user_type_ref(enum_ty),
        // 型参照やサブ式を持たないリーフ
        Expr::NumberLit(_)
        | Expr::IntLit(_)
        | Expr::BoolLit(_)
        | Expr::StringLit(_)
        | Expr::Ident(_)
        | Expr::Unit
        | Expr::RawCode(_)
        | Expr::Regex { .. }
        | Expr::PrimitiveAssocConst { .. }
        | Expr::StdConst(_) => {}
    }
}

/// `CallTarget` の全 variant を走査し、内部に [`UserTypeRef`] を持つ variant
/// については `visit_user_type_ref` フックを発火する。
///
/// I-378 で導入された走査ポイント。これにより walker は `Expr::FnCall::target`
/// 内の user type 参照（`UserAssocFn::ty` / `UserTupleCtor::0` /
/// `UserEnumVariantCtor::enum_ty`）を構造的に拾えるようになり、文字列 path 解析
/// や uppercase ヒューリスティックが不要になる。
pub fn walk_call_target<V: IrVisitor + ?Sized>(v: &mut V, target: &CallTarget) {
    match target {
        CallTarget::UserAssocFn { ty, .. } => v.visit_user_type_ref(ty),
        CallTarget::UserTupleCtor(ty) => v.visit_user_type_ref(ty),
        CallTarget::UserEnumVariantCtor { enum_ty, .. } => v.visit_user_type_ref(enum_ty),
        // user type 参照を持たない variant
        CallTarget::Free(_)
        | CallTarget::BuiltinVariant(_)
        | CallTarget::ExternalPath(_)
        | CallTarget::Super => {}
    }
}

/// `RustType` の全 variant を再帰的に走査する。
pub fn walk_rust_type<V: IrVisitor + ?Sized>(v: &mut V, ty: &RustType) {
    match ty {
        RustType::Named { type_args, .. } => {
            for arg in type_args {
                v.visit_rust_type(arg);
            }
        }
        RustType::QSelf {
            qself, trait_ref, ..
        } => {
            v.visit_rust_type(qself);
            for arg in &trait_ref.type_args {
                v.visit_rust_type(arg);
            }
        }
        RustType::Option(inner) | RustType::Vec(inner) | RustType::Ref(inner) => {
            v.visit_rust_type(inner);
        }
        RustType::Result { ok, err } => {
            v.visit_rust_type(ok);
            v.visit_rust_type(err);
        }
        RustType::Tuple(elems) => {
            for elem in elems {
                v.visit_rust_type(elem);
            }
        }
        RustType::Fn {
            params,
            return_type,
        } => {
            for p in params {
                v.visit_rust_type(p);
            }
            v.visit_rust_type(return_type);
        }
        RustType::Unit
        | RustType::String
        | RustType::F64
        | RustType::Bool
        | RustType::Any
        | RustType::Never
        | RustType::DynTrait(_) => {}
    }
}

/// `Pattern` の全 variant を再帰的に走査する。
pub fn walk_pattern<V: IrVisitor + ?Sized>(v: &mut V, pat: &Pattern) {
    match pat {
        Pattern::Wildcard => {}
        Pattern::Literal(e) => v.visit_expr(e),
        Pattern::Binding { subpat, .. } => {
            if let Some(sub) = subpat {
                v.visit_pattern(sub);
            }
        }
        Pattern::TupleStruct { fields, .. } => {
            for f in fields {
                v.visit_pattern(f);
            }
        }
        Pattern::Struct { fields, .. } => {
            for (_, p) in fields {
                v.visit_pattern(p);
            }
        }
        Pattern::UnitStruct { .. } => {}
        Pattern::Or(pats) => {
            for p in pats {
                v.visit_pattern(p);
            }
        }
        Pattern::Range { start, end, .. } => {
            if let Some(s) = start {
                v.visit_expr(s);
            }
            if let Some(e) = end {
                v.visit_expr(e);
            }
        }
        Pattern::Ref { inner, .. } => v.visit_pattern(inner),
        Pattern::Tuple(pats) => {
            for p in pats {
                v.visit_pattern(p);
            }
        }
    }
}

/// `MatchArm` を走査する（patterns + guard + body）。
pub fn walk_match_arm<V: IrVisitor + ?Sized>(v: &mut V, arm: &MatchArm) {
    for pat in &arm.patterns {
        v.visit_pattern(pat);
    }
    if let Some(g) = &arm.guard {
        v.visit_expr(g);
    }
    for s in &arm.body {
        v.visit_stmt(s);
    }
}

#[cfg(test)]
#[path = "visit_tests.rs"]
mod tests;
