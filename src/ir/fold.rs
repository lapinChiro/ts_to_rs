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
    RustType, Stmt, StructField, TraitRef, TypeParam,
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
            trait_ref: TraitRef {
                name: trait_ref.name,
                type_args: trait_ref
                    .type_args
                    .into_iter()
                    .map(|a| f.fold_rust_type(a))
                    .collect(),
            },
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
        ty @ (RustType::Unit
        | RustType::String
        | RustType::F64
        | RustType::Bool
        | RustType::Any
        | RustType::Never
        | RustType::DynTrait(_)) => ty,
    }
}

/// `TypeParam` の制約を fold する。
pub fn walk_type_param<F: IrFolder + ?Sized>(f: &mut F, tp: TypeParam) -> TypeParam {
    TypeParam {
        name: tp.name,
        constraint: tp.constraint.map(|c| f.fold_rust_type(c)),
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
        Pattern::TupleStruct { path, fields } => Pattern::TupleStruct {
            path,
            fields: fields.into_iter().map(|p| f.fold_pattern(p)).collect(),
        },
        Pattern::Struct { path, fields, rest } => Pattern::Struct {
            path,
            fields: fields
                .into_iter()
                .map(|(n, p)| (n, f.fold_pattern(p)))
                .collect(),
            rest,
        },
        Pattern::UnitStruct { path } => Pattern::UnitStruct { path },
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

/// `MatchArm` の全子要素を fold する。
///
/// NOTE (Phase 1): `arm.patterns` は `Vec<MatchPattern>` のまま。Phase 2 で
/// `Vec<Pattern>` に置換後、`f.fold_pattern(pat)` を呼ぶ。現状では
/// `MatchPattern::Literal(Expr)` 内の Expr のみ fold 対象とする。
pub fn walk_match_arm<F: IrFolder + ?Sized>(f: &mut F, arm: MatchArm) -> MatchArm {
    use crate::ir::MatchPattern;
    let patterns = arm
        .patterns
        .into_iter()
        .map(|p| match p {
            MatchPattern::Literal(e) => MatchPattern::Literal(f.fold_expr(e)),
            other => other,
        })
        .collect();
    MatchArm {
        patterns,
        guard: arm.guard.map(|g| f.fold_expr(g)),
        body: arm.body.into_iter().map(|s| f.fold_stmt(s)).collect(),
    }
}

/// `Method` の全子要素を fold する。
pub fn walk_method<F: IrFolder + ?Sized>(f: &mut F, m: Method) -> Method {
    Method {
        vis: m.vis,
        name: m.name,
        has_self: m.has_self,
        has_mut_self: m.has_mut_self,
        params: m
            .params
            .into_iter()
            .map(|p| Param {
                name: p.name,
                ty: p.ty.map(|t| f.fold_rust_type(t)),
            })
            .collect(),
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
        } => Item::Struct {
            vis,
            name,
            type_params: type_params
                .into_iter()
                .map(|tp| f.fold_type_param(tp))
                .collect(),
            fields: fields
                .into_iter()
                .map(|fld| StructField {
                    vis: fld.vis,
                    name: fld.name,
                    ty: f.fold_rust_type(fld.ty),
                })
                .collect(),
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
                        .map(|fld| StructField {
                            vis: fld.vis,
                            name: fld.name,
                            ty: f.fold_rust_type(fld.ty),
                        })
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
                .map(|sup| TraitRef {
                    name: sup.name,
                    type_args: sup
                        .type_args
                        .into_iter()
                        .map(|a| f.fold_rust_type(a))
                        .collect(),
                })
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
            for_trait: for_trait.map(|tr| TraitRef {
                name: tr.name,
                type_args: tr
                    .type_args
                    .into_iter()
                    .map(|a| f.fold_rust_type(a))
                    .collect(),
            }),
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
            params: params
                .into_iter()
                .map(|p| Param {
                    name: p.name,
                    ty: p.ty.map(|t| f.fold_rust_type(t)),
                })
                .collect(),
            return_type: return_type.map(|t| f.fold_rust_type(t)),
            body: body.into_iter().map(|s| f.fold_stmt(s)).collect(),
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
            // NOTE (Phase 1): `pattern` is still `String`. Phase 2 will call
            // `f.fold_pattern(pattern)`.
            pattern,
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
            // NOTE (Phase 1): still `String`.
            pattern,
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
            params: params
                .into_iter()
                .map(|p| Param {
                    name: p.name,
                    ty: p.ty.map(|t| f.fold_rust_type(t)),
                })
                .collect(),
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
            target: fold_call_target(f, target),
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
            // NOTE (Phase 1): still `String`.
            pattern,
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
            // NOTE (Phase 1): still `String`.
            pattern,
        },
        Expr::Block(stmts) => Expr::Block(stmts.into_iter().map(|s| f.fold_stmt(s)).collect()),
        Expr::Match { expr, arms } => Expr::Match {
            expr: Box::new(f.fold_expr(*expr)),
            arms: arms.into_iter().map(|a| f.fold_match_arm(a)).collect(),
        },
        // leaf expressions without sub-expressions or types
        e @ (Expr::NumberLit(_)
        | Expr::IntLit(_)
        | Expr::BoolLit(_)
        | Expr::StringLit(_)
        | Expr::Ident(_)
        | Expr::Unit
        | Expr::RawCode(_)
        | Expr::Regex { .. }) => e,
    }
}

/// `CallTarget` は折り畳み対象外。
///
/// `CallTarget::Path` の `segments` / `type_ref` はプレーンな識別子文字列
/// （`RustType` ではない）であり、型パラメータ置換の対象にはならない。
/// 既存の `src/ir/substitute.rs::Expr::substitute` も同じ方針で `CallTarget`
/// を `.clone()` でそのまま引き継いでおり（同ファイル 400-403 行のコメント
/// と単体テスト `test_substitute_fn_call_preserves_call_target_and_substitutes_args`
/// を参照）、`IrFolder` もその不変条件を踏襲する。
///
/// 結果として本関数は恒等変換だが、意図を明示するため独立関数として分離し、
/// 将来 `CallTarget` に型情報が追加された場合の拡張ポイントとして残す。
fn fold_call_target<F: IrFolder + ?Sized>(_f: &mut F, target: CallTarget) -> CallTarget {
    target
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::test_fixtures::{all_exprs, all_items, all_patterns, all_rust_types, all_stmts};
    use crate::ir::BinOp;

    /// Identity folder: defaults return the input unchanged; used to verify that
    /// `walk_*` produces identical IR for arbitrary inputs.
    struct IdentityFolder;
    impl IrFolder for IdentityFolder {}

    #[test]
    fn identity_folder_preserves_binary_op() {
        let expr = Expr::BinaryOp {
            left: Box::new(Expr::IntLit(1)),
            op: BinOp::Add,
            right: Box::new(Expr::IntLit(2)),
        };
        let result = IdentityFolder.fold_expr(expr.clone());
        assert_eq!(result, expr);
    }

    #[test]
    fn identity_folder_preserves_pattern() {
        let pat = Pattern::TupleStruct {
            path: vec!["Some".to_string()],
            fields: vec![Pattern::binding("x")],
        };
        let result = IdentityFolder.fold_pattern(pat.clone());
        assert_eq!(result, pat);
    }

    /// Replaces `RustType::Named { name: "T", type_args: [] }` with `RustType::F64`.
    struct ReplaceTWithF64;
    impl IrFolder for ReplaceTWithF64 {
        fn fold_rust_type(&mut self, ty: RustType) -> RustType {
            if let RustType::Named {
                ref name,
                ref type_args,
            } = ty
            {
                if name == "T" && type_args.is_empty() {
                    return RustType::F64;
                }
            }
            walk_rust_type(self, ty)
        }
    }

    #[test]
    fn type_substitute_folder_replaces_named_t() {
        let ty = RustType::Option(Box::new(RustType::Named {
            name: "T".to_string(),
            type_args: vec![],
        }));
        let result = ReplaceTWithF64.fold_rust_type(ty);
        assert_eq!(result, RustType::Option(Box::new(RustType::F64)));
    }

    // ------------------------------------------------------------------
    // 全 variant 網羅 identity テスト
    //
    // `walk_*` が全 variant を正しく再構築することを確認する。identity folder
    // に各 variant を通すと入力と等しい値が返ることを検証する。これにより
    // 将来 variant 追加時に walk_* の更新漏れ（特に「pass-through 忘れ」）を
    // identity テストが検出する。
    // ------------------------------------------------------------------

    #[test]
    fn identity_folder_preserves_all_rust_type_variants() {
        for ty in all_rust_types() {
            let result = IdentityFolder.fold_rust_type(ty.clone());
            assert_eq!(result, ty, "identity fold changed RustType variant");
        }
    }

    #[test]
    fn identity_folder_preserves_all_pattern_variants() {
        for pat in all_patterns() {
            let result = IdentityFolder.fold_pattern(pat.clone());
            assert_eq!(result, pat, "identity fold changed Pattern variant");
        }
    }

    #[test]
    fn identity_folder_preserves_all_expr_variants() {
        for expr in all_exprs() {
            let result = IdentityFolder.fold_expr(expr.clone());
            assert_eq!(result, expr, "identity fold changed Expr variant");
        }
    }

    #[test]
    fn identity_folder_preserves_all_stmt_variants() {
        for stmt in all_stmts() {
            let result = IdentityFolder.fold_stmt(stmt.clone());
            assert_eq!(result, stmt, "identity fold changed Stmt variant");
        }
    }

    #[test]
    fn identity_folder_preserves_all_item_variants() {
        for item in all_items() {
            let result = IdentityFolder.fold_item(item.clone());
            assert_eq!(result, item, "identity fold changed Item variant");
        }
    }

    #[test]
    fn type_substitute_folder_descends_into_fn_type() {
        let ty = RustType::Fn {
            params: vec![RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            }],
            return_type: Box::new(RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            }),
        };
        let result = ReplaceTWithF64.fold_rust_type(ty);
        assert_eq!(
            result,
            RustType::Fn {
                params: vec![RustType::F64],
                return_type: Box::new(RustType::F64),
            }
        );
    }
}
