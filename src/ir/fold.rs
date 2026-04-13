//! IR 変換用の owning visitor trait（SWC の `Fold` 相当）。
//!
//! `IrVisitor` が read-only 走査のための trait であるのに対し、`IrFolder` は
//! IR ノードを消費して新しい IR ノードを返す変換用 trait。型パラメータ置換
//! （`Substitute`）のような **IR → IR 変換** のための共通骨格を提供する。
//!
//! # 使い方
//!
//! ```ignore
//! use crate::ir::fold::{IrFolder, walk_rust_type};
//! use crate::ir::RustType;
//! use std::collections::HashMap;
//!
//! struct Substitute<'a> {
//!     bindings: &'a HashMap<String, RustType>,
//! }
//!
//! impl<'a> IrFolder for Substitute<'a> {
//!     fn fold_rust_type(&mut self, ty: RustType) -> RustType {
//!         if let RustType::Named { ref name, ref type_args } = ty {
//!             if type_args.is_empty() {
//!                 if let Some(concrete) = self.bindings.get(name.as_str()) {
//!                     return concrete.clone();
//!                 }
//!             }
//!         }
//!         walk_rust_type(self, ty)
//!     }
//! }
//! ```

use super::{
    AssocConst, CallTarget, ClosureBody, EnumVariant, Expr, Item, MatchArm, Method, Param, Pattern,
    PatternCtor, RustType, Stmt, StructField, TraitRef, TypeParam, UserTypeRef,
};

/// IR を owned 値ベースで変換する folder trait。
///
/// 各 `fold_*` メソッドはデフォルトで同名の `walk_*` 関数に委譲する。
/// 実装者は必要なメソッドのみ override し、子ノードの再帰変換が必要な
/// 場合は明示的に `walk_*` を呼ぶ。
pub trait IrFolder {
    fn fold_item(&mut self, item: Item) -> Item {
        walk_item(self, item)
    }
    fn fold_stmt(&mut self, stmt: Stmt) -> Stmt {
        walk_stmt(self, stmt)
    }
    fn fold_expr(&mut self, expr: Expr) -> Expr {
        walk_expr(self, expr)
    }
    fn fold_rust_type(&mut self, ty: RustType) -> RustType {
        walk_rust_type(self, ty)
    }
    fn fold_pattern(&mut self, pat: Pattern) -> Pattern {
        walk_pattern(self, pat)
    }
    fn fold_match_arm(&mut self, arm: MatchArm) -> MatchArm {
        walk_match_arm(self, arm)
    }
    fn fold_type_param(&mut self, tp: TypeParam) -> TypeParam {
        walk_type_param(self, tp)
    }
    fn fold_method(&mut self, m: Method) -> Method {
        walk_method(self, m)
    }
    /// User-defined type 参照を fold する。デフォルトは恒等変換。
    ///
    /// `walk_expr` の `Expr::EnumVariant::enum_ty` および `walk_call_target` の
    /// `CallTarget::UserAssocFn` / `UserTupleCtor` / `UserEnumVariantCtor` 各
    /// variant から発火する。型パラメータ置換 (`Substitute`) は user type ref を
    /// 変換しない (`UserTypeRef` は識別子であり `RustType` ではない) ため
    /// デフォルトの恒等変換を使う。将来的に user type 名そのものを書き換える
    /// fold 実装が必要になった場合の拡張ポイントとして残す。
    fn fold_user_type_ref(&mut self, r: UserTypeRef) -> UserTypeRef {
        r
    }
    /// `TraitRef` を fold する。デフォルトは `walk_trait_ref` 経由で `type_args`
    /// のみ再帰し `name` は不変。
    ///
    /// `walk_rust_type::QSelf` および `walk_item::Trait::supertraits` /
    /// `Item::Impl::for_trait` の各構築サイトから発火する。`IrVisitor` 側の
    /// `visit_trait_ref` と対称な fold ホックで、新 IR 構造追加時の更新点を
    /// `walk_*` の単一ソースに集約する (I-380)。`TraitRef::name` を書き換える
    /// folder (例: trait リネーム) はこのフックを override する。
    fn fold_trait_ref(&mut self, tr: TraitRef) -> TraitRef {
        walk_trait_ref(self, tr)
    }
    fn fold_param(&mut self, p: Param) -> Param {
        walk_param(self, p)
    }
    fn fold_struct_field(&mut self, f: StructField) -> StructField {
        walk_struct_field(self, f)
    }
}

/// `Param` の型を fold する。
pub fn walk_param<F: IrFolder + ?Sized>(f: &mut F, p: Param) -> Param {
    Param {
        name: p.name,
        ty: p.ty.map(|t| f.fold_rust_type(t)),
    }
}

/// `TraitRef` を fold する: `type_args` のみ再帰し、`name` は不変で返す。
///
/// `IrVisitor::walk_trait_ref` の fold 対称版 (I-380)。
pub fn walk_trait_ref<F: IrFolder + ?Sized>(f: &mut F, tr: TraitRef) -> TraitRef {
    TraitRef {
        name: tr.name,
        type_args: tr
            .type_args
            .into_iter()
            .map(|a| f.fold_rust_type(a))
            .collect(),
    }
}

/// `StructField` の型を fold する。visibility と name は不変。
pub fn walk_struct_field<F: IrFolder + ?Sized>(f: &mut F, field: StructField) -> StructField {
    StructField {
        vis: field.vis,
        name: field.name,
        ty: f.fold_rust_type(field.ty),
    }
}

/// `RustType` の全 variant を再帰的に fold する。
pub fn walk_rust_type<F: IrFolder + ?Sized>(f: &mut F, ty: RustType) -> RustType {
    match ty {
        RustType::Named { name, type_args } => RustType::Named {
            name,
            type_args: type_args.into_iter().map(|a| f.fold_rust_type(a)).collect(),
        },
        RustType::QSelf {
            qself,
            trait_ref,
            item,
        } => RustType::QSelf {
            qself: Box::new(f.fold_rust_type(*qself)),
            trait_ref: f.fold_trait_ref(trait_ref),
            item,
        },
        RustType::Option(inner) => RustType::Option(Box::new(f.fold_rust_type(*inner))),
        RustType::Vec(inner) => RustType::Vec(Box::new(f.fold_rust_type(*inner))),
        RustType::Ref(inner) => RustType::Ref(Box::new(f.fold_rust_type(*inner))),
        RustType::Result { ok, err } => RustType::Result {
            ok: Box::new(f.fold_rust_type(*ok)),
            err: Box::new(f.fold_rust_type(*err)),
        },
        RustType::Tuple(elems) => RustType::Tuple(
            elems
                .into_iter()
                .map(|e| f.fold_rust_type(e))
                .collect::<Vec<_>>(),
        ),
        RustType::Fn {
            params,
            return_type,
        } => RustType::Fn {
            params: params.into_iter().map(|p| f.fold_rust_type(p)).collect(),
            return_type: Box::new(f.fold_rust_type(*return_type)),
        },
        RustType::StdCollection { kind, args } => RustType::StdCollection {
            kind,
            args: args.into_iter().map(|a| f.fold_rust_type(a)).collect(),
        },
        ty @ (RustType::Unit
        | RustType::String
        | RustType::F64
        | RustType::Bool
        | RustType::Any
        | RustType::Never
        | RustType::DynTrait(_)
        | RustType::TypeVar { .. }
        | RustType::Primitive(_)) => ty,
    }
}

/// `TypeParam` の制約を fold する。
pub fn walk_type_param<F: IrFolder + ?Sized>(f: &mut F, tp: TypeParam) -> TypeParam {
    TypeParam {
        name: tp.name,
        constraint: tp.constraint.map(|c| f.fold_rust_type(c)),
        default: tp.default.map(|d| f.fold_rust_type(d)),
    }
}

/// `Pattern` の全 variant を再帰的に fold する。
pub fn walk_pattern<F: IrFolder + ?Sized>(f: &mut F, pat: Pattern) -> Pattern {
    match pat {
        Pattern::Wildcard => Pattern::Wildcard,
        Pattern::Literal(e) => Pattern::Literal(f.fold_expr(e)),
        Pattern::Binding {
            name,
            is_mut,
            subpat,
        } => Pattern::Binding {
            name,
            is_mut,
            subpat: subpat.map(|p| Box::new(f.fold_pattern(*p))),
        },
        Pattern::TupleStruct { ctor, fields } => Pattern::TupleStruct {
            ctor: walk_pattern_ctor(f, ctor),
            fields: fields.into_iter().map(|p| f.fold_pattern(p)).collect(),
        },
        Pattern::Struct { ctor, fields, rest } => Pattern::Struct {
            ctor: walk_pattern_ctor(f, ctor),
            fields: fields
                .into_iter()
                .map(|(n, p)| (n, f.fold_pattern(p)))
                .collect(),
            rest,
        },
        Pattern::UnitStruct { ctor } => Pattern::UnitStruct {
            ctor: walk_pattern_ctor(f, ctor),
        },
        Pattern::Or(pats) => Pattern::Or(pats.into_iter().map(|p| f.fold_pattern(p)).collect()),
        Pattern::Range {
            start,
            end,
            inclusive,
        } => Pattern::Range {
            start: start.map(|e| Box::new(f.fold_expr(*e))),
            end: end.map(|e| Box::new(f.fold_expr(*e))),
            inclusive,
        },
        Pattern::Ref { mutable, inner } => Pattern::Ref {
            mutable,
            inner: Box::new(f.fold_pattern(*inner)),
        },
        Pattern::Tuple(pats) => {
            Pattern::Tuple(pats.into_iter().map(|p| f.fold_pattern(p)).collect())
        }
    }
}

/// `PatternCtor` を fold する。`UserEnumVariant::enum_ty` / `UserStruct::0` の
/// [`UserTypeRef`] については `fold_user_type_ref` を経由する。
pub fn walk_pattern_ctor<F: IrFolder + ?Sized>(f: &mut F, ctor: PatternCtor) -> PatternCtor {
    match ctor {
        PatternCtor::Builtin(b) => PatternCtor::Builtin(b),
        PatternCtor::UserEnumVariant { enum_ty, variant } => PatternCtor::UserEnumVariant {
            enum_ty: f.fold_user_type_ref(enum_ty),
            variant,
        },
        PatternCtor::UserStruct(ty) => PatternCtor::UserStruct(f.fold_user_type_ref(ty)),
    }
}

/// `MatchArm` の全子要素を fold する。
pub fn walk_match_arm<F: IrFolder + ?Sized>(f: &mut F, arm: MatchArm) -> MatchArm {
    MatchArm {
        patterns: arm
            .patterns
            .into_iter()
            .map(|p| f.fold_pattern(p))
            .collect(),
        guard: arm.guard.map(|g| f.fold_expr(g)),
        body: arm.body.into_iter().map(|s| f.fold_stmt(s)).collect(),
    }
}

/// `Method` の全子要素を fold する。
pub fn walk_method<F: IrFolder + ?Sized>(f: &mut F, m: Method) -> Method {
    Method {
        vis: m.vis,
        name: m.name,
        is_async: m.is_async,
        has_self: m.has_self,
        has_mut_self: m.has_mut_self,
        params: m.params.into_iter().map(|p| f.fold_param(p)).collect(),
        return_type: m.return_type.map(|t| f.fold_rust_type(t)),
        body: m
            .body
            .map(|body| body.into_iter().map(|s| f.fold_stmt(s)).collect()),
    }
}

/// `Item` の全 variant を再帰的に fold する。
pub fn walk_item<F: IrFolder + ?Sized>(f: &mut F, item: Item) -> Item {
    match item {
        Item::Struct {
            vis,
            name,
            type_params,
            fields,
            is_unit_struct,
        } => Item::Struct {
            vis,
            name,
            type_params: type_params
                .into_iter()
                .map(|tp| f.fold_type_param(tp))
                .collect(),
            fields: fields
                .into_iter()
                .map(|fld| f.fold_struct_field(fld))
                .collect(),
            is_unit_struct,
        },
        Item::Enum {
            vis,
            name,
            type_params,
            serde_tag,
            variants,
        } => Item::Enum {
            vis,
            name,
            type_params: type_params
                .into_iter()
                .map(|tp| f.fold_type_param(tp))
                .collect(),
            serde_tag,
            variants: variants
                .into_iter()
                .map(|v| EnumVariant {
                    name: v.name,
                    value: v.value,
                    data: v.data.map(|d| f.fold_rust_type(d)),
                    fields: v
                        .fields
                        .into_iter()
                        .map(|fld| f.fold_struct_field(fld))
                        .collect(),
                })
                .collect(),
        },
        Item::Trait {
            vis,
            name,
            type_params,
            supertraits,
            methods,
            associated_types,
        } => Item::Trait {
            vis,
            name,
            type_params: type_params
                .into_iter()
                .map(|tp| f.fold_type_param(tp))
                .collect(),
            supertraits: supertraits
                .into_iter()
                .map(|sup| f.fold_trait_ref(sup))
                .collect(),
            methods: methods.into_iter().map(|m| f.fold_method(m)).collect(),
            associated_types,
        },
        Item::Impl {
            struct_name,
            type_params,
            for_trait,
            consts,
            methods,
        } => Item::Impl {
            struct_name,
            type_params: type_params
                .into_iter()
                .map(|tp| f.fold_type_param(tp))
                .collect(),
            for_trait: for_trait.map(|tr| f.fold_trait_ref(tr)),
            consts: consts
                .into_iter()
                .map(|c| AssocConst {
                    vis: c.vis,
                    name: c.name,
                    ty: f.fold_rust_type(c.ty),
                    value: f.fold_expr(c.value),
                })
                .collect(),
            methods: methods.into_iter().map(|m| f.fold_method(m)).collect(),
        },
        Item::TypeAlias {
            vis,
            name,
            type_params,
            ty,
        } => Item::TypeAlias {
            vis,
            name,
            type_params: type_params
                .into_iter()
                .map(|tp| f.fold_type_param(tp))
                .collect(),
            ty: f.fold_rust_type(ty),
        },
        Item::Fn {
            vis,
            attributes,
            is_async,
            name,
            type_params,
            params,
            return_type,
            body,
        } => Item::Fn {
            vis,
            attributes,
            is_async,
            name,
            type_params: type_params
                .into_iter()
                .map(|tp| f.fold_type_param(tp))
                .collect(),
            params: params.into_iter().map(|p| f.fold_param(p)).collect(),
            return_type: return_type.map(|t| f.fold_rust_type(t)),
            body: body.into_iter().map(|s| f.fold_stmt(s)).collect(),
        },
        Item::Const {
            vis,
            name,
            ty,
            value,
        } => Item::Const {
            vis,
            name,
            ty: f.fold_rust_type(ty),
            value: f.fold_expr(value),
        },
        item @ (Item::Comment(_) | Item::Use { .. } | Item::RawCode(_)) => item,
    }
}

/// `Stmt` の全 variant を再帰的に fold する。
pub fn walk_stmt<F: IrFolder + ?Sized>(f: &mut F, stmt: Stmt) -> Stmt {
    match stmt {
        Stmt::Let {
            mutable,
            name,
            ty,
            init,
        } => Stmt::Let {
            mutable,
            name,
            ty: ty.map(|t| f.fold_rust_type(t)),
            init: init.map(|e| f.fold_expr(e)),
        },
        Stmt::If {
            condition,
            then_body,
            else_body,
        } => Stmt::If {
            condition: f.fold_expr(condition),
            then_body: then_body.into_iter().map(|s| f.fold_stmt(s)).collect(),
            else_body: else_body.map(|eb| eb.into_iter().map(|s| f.fold_stmt(s)).collect()),
        },
        Stmt::While {
            label,
            condition,
            body,
        } => Stmt::While {
            label,
            condition: f.fold_expr(condition),
            body: body.into_iter().map(|s| f.fold_stmt(s)).collect(),
        },
        Stmt::WhileLet {
            label,
            pattern,
            expr,
            body,
        } => Stmt::WhileLet {
            label,
            pattern: f.fold_pattern(pattern),
            expr: f.fold_expr(expr),
            body: body.into_iter().map(|s| f.fold_stmt(s)).collect(),
        },
        Stmt::ForIn {
            label,
            var,
            iterable,
            body,
        } => Stmt::ForIn {
            label,
            var,
            iterable: f.fold_expr(iterable),
            body: body.into_iter().map(|s| f.fold_stmt(s)).collect(),
        },
        Stmt::Loop { label, body } => Stmt::Loop {
            label,
            body: body.into_iter().map(|s| f.fold_stmt(s)).collect(),
        },
        Stmt::Break { label, value } => Stmt::Break {
            label,
            value: value.map(|e| f.fold_expr(e)),
        },
        Stmt::Continue { label } => Stmt::Continue { label },
        Stmt::Return(opt) => Stmt::Return(opt.map(|e| f.fold_expr(e))),
        Stmt::Expr(e) => Stmt::Expr(f.fold_expr(e)),
        Stmt::TailExpr(e) => Stmt::TailExpr(f.fold_expr(e)),
        Stmt::IfLet {
            pattern,
            expr,
            then_body,
            else_body,
        } => Stmt::IfLet {
            pattern: f.fold_pattern(pattern),
            expr: f.fold_expr(expr),
            then_body: then_body.into_iter().map(|s| f.fold_stmt(s)).collect(),
            else_body: else_body.map(|eb| eb.into_iter().map(|s| f.fold_stmt(s)).collect()),
        },
        Stmt::Match { expr, arms } => Stmt::Match {
            expr: f.fold_expr(expr),
            arms: arms.into_iter().map(|a| f.fold_match_arm(a)).collect(),
        },
        Stmt::LabeledBlock { label, body } => Stmt::LabeledBlock {
            label,
            body: body.into_iter().map(|s| f.fold_stmt(s)).collect(),
        },
    }
}

/// `Expr` の全 variant を再帰的に fold する。
pub fn walk_expr<F: IrFolder + ?Sized>(f: &mut F, expr: Expr) -> Expr {
    match expr {
        Expr::Cast { expr, target } => Expr::Cast {
            expr: Box::new(f.fold_expr(*expr)),
            target: f.fold_rust_type(target),
        },
        Expr::StructInit { name, fields, base } => Expr::StructInit {
            name,
            fields: fields
                .into_iter()
                .map(|(n, e)| (n, f.fold_expr(e)))
                .collect(),
            base: base.map(|b| Box::new(f.fold_expr(*b))),
        },
        Expr::Closure {
            params,
            return_type,
            body,
        } => Expr::Closure {
            params: params.into_iter().map(|p| f.fold_param(p)).collect(),
            return_type: return_type.map(|t| f.fold_rust_type(t)),
            body: match body {
                ClosureBody::Expr(e) => ClosureBody::Expr(Box::new(f.fold_expr(*e))),
                ClosureBody::Block(stmts) => {
                    ClosureBody::Block(stmts.into_iter().map(|s| f.fold_stmt(s)).collect())
                }
            },
        },
        Expr::FieldAccess { object, field } => Expr::FieldAccess {
            object: Box::new(f.fold_expr(*object)),
            field,
        },
        Expr::MethodCall {
            object,
            method,
            args,
        } => Expr::MethodCall {
            object: Box::new(f.fold_expr(*object)),
            method,
            args: args.into_iter().map(|a| f.fold_expr(a)).collect(),
        },
        Expr::Assign { target, value } => Expr::Assign {
            target: Box::new(f.fold_expr(*target)),
            value: Box::new(f.fold_expr(*value)),
        },
        Expr::UnaryOp { op, operand } => Expr::UnaryOp {
            op,
            operand: Box::new(f.fold_expr(*operand)),
        },
        Expr::BinaryOp { left, op, right } => Expr::BinaryOp {
            left: Box::new(f.fold_expr(*left)),
            op,
            right: Box::new(f.fold_expr(*right)),
        },
        Expr::Range { start, end } => Expr::Range {
            start: start.map(|s| Box::new(f.fold_expr(*s))),
            end: end.map(|e| Box::new(f.fold_expr(*e))),
        },
        Expr::FnCall { target, args } => Expr::FnCall {
            target: walk_call_target(f, target),
            args: args.into_iter().map(|a| f.fold_expr(a)).collect(),
        },
        Expr::Vec { elements } => Expr::Vec {
            elements: elements.into_iter().map(|e| f.fold_expr(e)).collect(),
        },
        Expr::Tuple { elements } => Expr::Tuple {
            elements: elements.into_iter().map(|e| f.fold_expr(e)).collect(),
        },
        Expr::If {
            condition,
            then_expr,
            else_expr,
        } => Expr::If {
            condition: Box::new(f.fold_expr(*condition)),
            then_expr: Box::new(f.fold_expr(*then_expr)),
            else_expr: Box::new(f.fold_expr(*else_expr)),
        },
        Expr::IfLet {
            pattern,
            expr,
            then_expr,
            else_expr,
        } => Expr::IfLet {
            pattern: Box::new(f.fold_pattern(*pattern)),
            expr: Box::new(f.fold_expr(*expr)),
            then_expr: Box::new(f.fold_expr(*then_expr)),
            else_expr: Box::new(f.fold_expr(*else_expr)),
        },
        Expr::FormatMacro { template, args } => Expr::FormatMacro {
            template,
            args: args.into_iter().map(|a| f.fold_expr(a)).collect(),
        },
        Expr::MacroCall {
            name,
            args,
            use_debug,
        } => Expr::MacroCall {
            name,
            args: args.into_iter().map(|a| f.fold_expr(a)).collect(),
            use_debug,
        },
        Expr::Await(e) => Expr::Await(Box::new(f.fold_expr(*e))),
        Expr::Deref(e) => Expr::Deref(Box::new(f.fold_expr(*e))),
        Expr::Ref(e) => Expr::Ref(Box::new(f.fold_expr(*e))),
        Expr::Index { object, index } => Expr::Index {
            object: Box::new(f.fold_expr(*object)),
            index: Box::new(f.fold_expr(*index)),
        },
        Expr::RuntimeTypeof { operand } => Expr::RuntimeTypeof {
            operand: Box::new(f.fold_expr(*operand)),
        },
        Expr::Matches { expr, pattern } => Expr::Matches {
            expr: Box::new(f.fold_expr(*expr)),
            pattern: Box::new(f.fold_pattern(*pattern)),
        },
        Expr::Block(stmts) => Expr::Block(stmts.into_iter().map(|s| f.fold_stmt(s)).collect()),
        Expr::Match { expr, arms } => Expr::Match {
            expr: Box::new(f.fold_expr(*expr)),
            arms: arms.into_iter().map(|a| f.fold_match_arm(a)).collect(),
        },
        Expr::EnumVariant { enum_ty, variant } => Expr::EnumVariant {
            enum_ty: f.fold_user_type_ref(enum_ty),
            variant,
        },
        // leaf expressions without sub-expressions or types
        e @ (Expr::NumberLit(_)
        | Expr::IntLit(_)
        | Expr::BoolLit(_)
        | Expr::StringLit(_)
        | Expr::Ident(_)
        | Expr::Unit
        | Expr::RawCode(_)
        | Expr::Regex { .. }
        | Expr::PrimitiveAssocConst { .. }
        | Expr::StdConst(_)
        | Expr::BuiltinVariantValue(_)) => e,
    }
}

/// `CallTarget` の全 variant を fold する。
///
/// I-378 で 7 variant に分解されたあと、内部の [`UserTypeRef`] は
/// `fold_user_type_ref` フック経由で変換される。`Substitute` folder は user type
/// ref を識別変換するため実質 no-op だが、将来 user type 名そのものを書き換える
/// fold 実装が必要になった場合の拡張ポイントとして配線済み。
///
/// `Free` / `BuiltinVariant` / `ExternalPath` / `Super` は `UserTypeRef` を持た
/// ないため恒等変換する。
pub fn walk_call_target<F: IrFolder + ?Sized>(f: &mut F, target: CallTarget) -> CallTarget {
    match target {
        CallTarget::UserAssocFn { ty, method } => CallTarget::UserAssocFn {
            ty: f.fold_user_type_ref(ty),
            method,
        },
        CallTarget::UserTupleCtor(ty) => CallTarget::UserTupleCtor(f.fold_user_type_ref(ty)),
        CallTarget::UserEnumVariantCtor { enum_ty, variant } => CallTarget::UserEnumVariantCtor {
            enum_ty: f.fold_user_type_ref(enum_ty),
            variant,
        },
        t @ (CallTarget::Free(_)
        | CallTarget::BuiltinVariant(_)
        | CallTarget::ExternalPath(_)
        | CallTarget::Super) => t,
    }
}

#[cfg(test)]
#[path = "fold_tests.rs"]
mod tests;
