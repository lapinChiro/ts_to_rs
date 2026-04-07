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
    AssocConst, ClosureBody, Expr, Item, MatchArm, Method, Pattern, RustType, Stmt, TypeParam,
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
            pattern: _,
            expr,
            body,
            ..
        } => {
            // NOTE (Phase 1): `pattern` is still `String` during Phase 1. It will be
            // replaced by `Pattern` in Phase 2 and `v.visit_pattern(pattern)` will be
            // wired in at that time.
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
            pattern: _,
            expr,
            then_body,
            else_body,
        } => {
            // NOTE (Phase 1): `pattern` is still `String` during Phase 1.
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
        Expr::FnCall { target: _, args } => {
            // `CallTarget::Path::type_ref` は `visit_expr` を override した
            // 実装側で検査する（walk は子の再帰だけを担当する）。
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
            pattern: _,
            expr,
            then_expr,
            else_expr,
        } => {
            // NOTE (Phase 1): `pattern` is still `String` during Phase 1.
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
        Expr::Matches { expr, pattern: _ } => {
            // NOTE (Phase 1): `pattern` is still `String` during Phase 1.
            v.visit_expr(expr);
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
        // 型参照やサブ式を持たないリーフ
        Expr::NumberLit(_)
        | Expr::IntLit(_)
        | Expr::BoolLit(_)
        | Expr::StringLit(_)
        | Expr::Ident(_)
        | Expr::Unit
        | Expr::RawCode(_)
        | Expr::Regex { .. } => {}
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
///
/// NOTE (Phase 1): `arm.patterns` は `Vec<MatchPattern>` のまま。Phase 2 で
/// `Vec<Pattern>` に置換後、`v.visit_pattern(pat)` を呼ぶ。現状は
/// `MatchPattern::Literal(Expr)` 内の Expr のみ visitor 経由で走査する。
pub fn walk_match_arm<V: IrVisitor + ?Sized>(v: &mut V, arm: &MatchArm) {
    use crate::ir::MatchPattern;
    for pat in &arm.patterns {
        if let MatchPattern::Literal(expr) = pat {
            v.visit_expr(expr);
        }
    }
    if let Some(g) = &arm.guard {
        v.visit_expr(g);
    }
    for s in &arm.body {
        v.visit_stmt(s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::test_fixtures::{all_exprs, all_items, all_patterns, all_rust_types, all_stmts};
    use crate::ir::{BinOp, CallTarget, Visibility};

    #[derive(Default)]
    struct NodeCounter {
        items: usize,
        stmts: usize,
        exprs: usize,
        types: usize,
        patterns: usize,
        arms: usize,
    }

    impl IrVisitor for NodeCounter {
        fn visit_item(&mut self, item: &Item) {
            self.items += 1;
            walk_item(self, item);
        }
        fn visit_stmt(&mut self, stmt: &Stmt) {
            self.stmts += 1;
            walk_stmt(self, stmt);
        }
        fn visit_expr(&mut self, expr: &Expr) {
            self.exprs += 1;
            walk_expr(self, expr);
        }
        fn visit_rust_type(&mut self, ty: &RustType) {
            self.types += 1;
            walk_rust_type(self, ty);
        }
        fn visit_pattern(&mut self, pat: &Pattern) {
            self.patterns += 1;
            walk_pattern(self, pat);
        }
        fn visit_match_arm(&mut self, arm: &MatchArm) {
            self.arms += 1;
            walk_match_arm(self, arm);
        }
    }

    #[test]
    fn counter_visitor_traverses_nested_fn_body() {
        // fn f(x: f64) -> f64 { let y: f64 = x + 1.0; y }
        let item = Item::Fn {
            vis: Visibility::Public,
            attributes: vec![],
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![crate::ir::Param {
                name: "x".to_string(),
                ty: Some(RustType::F64),
            }],
            return_type: Some(RustType::F64),
            body: vec![
                Stmt::Let {
                    mutable: false,
                    name: "y".to_string(),
                    ty: Some(RustType::F64),
                    init: Some(Expr::BinaryOp {
                        left: Box::new(Expr::Ident("x".to_string())),
                        op: BinOp::Add,
                        right: Box::new(Expr::NumberLit(1.0)),
                    }),
                },
                Stmt::TailExpr(Expr::Ident("y".to_string())),
            ],
        };

        let mut counter = NodeCounter::default();
        counter.visit_item(&item);

        assert_eq!(counter.items, 1);
        assert_eq!(counter.stmts, 2);
        // exprs: Let init = BinaryOp (1) + BinaryOp.left Ident x (1) + BinaryOp.right NumberLit (1)
        //        + TailExpr Ident y (1) = 4
        assert_eq!(counter.exprs, 4);
        // param x: F64, let y: F64, return type F64 → 3 visits
        assert_eq!(counter.types, 3);
    }

    #[test]
    fn pattern_walker_visits_nested_tuple_struct() {
        // Some(Color::Red(x))
        let pat = Pattern::TupleStruct {
            path: vec!["Some".to_string()],
            fields: vec![Pattern::TupleStruct {
                path: vec!["Color".to_string(), "Red".to_string()],
                fields: vec![Pattern::binding("x")],
            }],
        };

        let mut counter = NodeCounter::default();
        counter.visit_pattern(&pat);

        // Outer Some(_), inner Color::Red(_), inner binding x → 3 pattern visits
        assert_eq!(counter.patterns, 3);
    }

    #[test]
    fn match_arm_walker_visits_literals_and_guard() {
        // NOTE (Phase 1): `MatchArm.patterns` is still `Vec<MatchPattern>`.
        // Phase 2 will replace with `Vec<Pattern>` and the walker will route
        // through `visit_pattern`. For now we only verify that Literal expressions
        // and guards are visited.
        use crate::ir::MatchPattern;
        let arm = MatchArm {
            patterns: vec![
                MatchPattern::Literal(Expr::IntLit(1)),
                MatchPattern::Literal(Expr::IntLit(2)),
            ],
            guard: Some(Expr::Ident("flag".to_string())),
            body: vec![],
        };

        let mut counter = NodeCounter::default();
        counter.visit_match_arm(&arm);

        assert_eq!(counter.arms, 1);
        // During Phase 1 the walker visits Literal inner exprs directly (not through
        // visit_pattern), so pattern count stays 0.
        assert_eq!(counter.patterns, 0);
        // 2 literal inner exprs + 1 guard = 3
        assert_eq!(counter.exprs, 3);
    }

    #[test]
    fn fn_call_walker_descends_into_args_only() {
        // foo(x, y)
        let expr = Expr::FnCall {
            target: CallTarget::simple("foo"),
            args: vec![Expr::Ident("x".to_string()), Expr::Ident("y".to_string())],
        };
        let mut counter = NodeCounter::default();
        counter.visit_expr(&expr);
        // outer FnCall(1) + 2 args (2) = 3
        assert_eq!(counter.exprs, 3);
    }

    // ------------------------------------------------------------------
    // 全 variant 網羅カバレッジテスト
    //
    // IR の全 variant を 1 度ずつ含むサンプルを構築し、対応する walker が
    // その variant を**実際に訪問したか**をタグ式に記録する visitor で検証
    // する。variant 追加時に walk_* の更新漏れを検出するセーフティネット。
    // ------------------------------------------------------------------

    use std::collections::HashSet;

    #[derive(Default)]
    struct TagRecorder {
        tags: HashSet<&'static str>,
    }

    impl TagRecorder {
        fn mark(&mut self, tag: &'static str) {
            self.tags.insert(tag);
        }
    }

    impl IrVisitor for TagRecorder {
        fn visit_item(&mut self, item: &Item) {
            self.mark(match item {
                Item::Struct { .. } => "item:struct",
                Item::Enum { .. } => "item:enum",
                Item::Trait { .. } => "item:trait",
                Item::Impl { .. } => "item:impl",
                Item::TypeAlias { .. } => "item:typealias",
                Item::Fn { .. } => "item:fn",
                Item::Comment(_) => "item:comment",
                Item::Use { .. } => "item:use",
                Item::RawCode(_) => "item:rawcode",
            });
            walk_item(self, item);
        }

        fn visit_stmt(&mut self, stmt: &Stmt) {
            self.mark(match stmt {
                Stmt::Let { .. } => "stmt:let",
                Stmt::If { .. } => "stmt:if",
                Stmt::While { .. } => "stmt:while",
                Stmt::WhileLet { .. } => "stmt:whilelet",
                Stmt::ForIn { .. } => "stmt:forin",
                Stmt::Loop { .. } => "stmt:loop",
                Stmt::Break { .. } => "stmt:break",
                Stmt::Continue { .. } => "stmt:continue",
                Stmt::Return(_) => "stmt:return",
                Stmt::Expr(_) => "stmt:expr",
                Stmt::TailExpr(_) => "stmt:tailexpr",
                Stmt::IfLet { .. } => "stmt:iflet",
                Stmt::Match { .. } => "stmt:match",
                Stmt::LabeledBlock { .. } => "stmt:labeledblock",
            });
            walk_stmt(self, stmt);
        }

        fn visit_expr(&mut self, expr: &Expr) {
            self.mark(match expr {
                Expr::NumberLit(_) => "expr:numberlit",
                Expr::BoolLit(_) => "expr:boollit",
                Expr::StringLit(_) => "expr:stringlit",
                Expr::Ident(_) => "expr:ident",
                Expr::FormatMacro { .. } => "expr:formatmacro",
                Expr::FieldAccess { .. } => "expr:fieldaccess",
                Expr::MethodCall { .. } => "expr:methodcall",
                Expr::StructInit { .. } => "expr:structinit",
                Expr::Assign { .. } => "expr:assign",
                Expr::UnaryOp { .. } => "expr:unaryop",
                Expr::BinaryOp { .. } => "expr:binaryop",
                Expr::Range { .. } => "expr:range",
                Expr::FnCall { .. } => "expr:fncall",
                Expr::Closure { .. } => "expr:closure",
                Expr::Vec { .. } => "expr:vec",
                Expr::Tuple { .. } => "expr:tuple",
                Expr::If { .. } => "expr:if",
                Expr::IfLet { .. } => "expr:iflet",
                Expr::MacroCall { .. } => "expr:macrocall",
                Expr::Await(_) => "expr:await",
                Expr::Deref(_) => "expr:deref",
                Expr::Ref(_) => "expr:ref",
                Expr::Unit => "expr:unit",
                Expr::IntLit(_) => "expr:intlit",
                Expr::RawCode(_) => "expr:rawcode",
                Expr::RuntimeTypeof { .. } => "expr:runtimetypeof",
                Expr::Index { .. } => "expr:index",
                Expr::Cast { .. } => "expr:cast",
                Expr::Matches { .. } => "expr:matches",
                Expr::Block(_) => "expr:block",
                Expr::Match { .. } => "expr:match",
                Expr::Regex { .. } => "expr:regex",
            });
            walk_expr(self, expr);
        }

        fn visit_rust_type(&mut self, ty: &RustType) {
            self.mark(match ty {
                RustType::Unit => "ty:unit",
                RustType::String => "ty:string",
                RustType::F64 => "ty:f64",
                RustType::Bool => "ty:bool",
                RustType::Option(_) => "ty:option",
                RustType::Vec(_) => "ty:vec",
                RustType::Fn { .. } => "ty:fn",
                RustType::Result { .. } => "ty:result",
                RustType::Tuple(_) => "ty:tuple",
                RustType::Any => "ty:any",
                RustType::Never => "ty:never",
                RustType::Named { .. } => "ty:named",
                RustType::Ref(_) => "ty:ref",
                RustType::DynTrait(_) => "ty:dyntrait",
                RustType::QSelf { .. } => "ty:qself",
            });
            walk_rust_type(self, ty);
        }

        fn visit_pattern(&mut self, pat: &Pattern) {
            self.mark(match pat {
                Pattern::Wildcard => "pat:wildcard",
                Pattern::Literal(_) => "pat:literal",
                Pattern::Binding { .. } => "pat:binding",
                Pattern::TupleStruct { .. } => "pat:tuplestruct",
                Pattern::Struct { .. } => "pat:struct",
                Pattern::UnitStruct { .. } => "pat:unitstruct",
                Pattern::Or(_) => "pat:or",
                Pattern::Range { .. } => "pat:range",
                Pattern::Ref { .. } => "pat:ref",
                Pattern::Tuple(_) => "pat:tuple",
            });
            walk_pattern(self, pat);
        }
    }

    /// 全 `RustType` variant が walker で訪問されることを検証する。
    #[test]
    fn walker_visits_every_rust_type_variant() {
        let mut rec = TagRecorder::default();
        for ty in all_rust_types() {
            rec.visit_rust_type(&ty);
        }
        let expected: HashSet<&'static str> = [
            "ty:unit",
            "ty:string",
            "ty:f64",
            "ty:bool",
            "ty:any",
            "ty:never",
            "ty:option",
            "ty:vec",
            "ty:ref",
            "ty:result",
            "ty:tuple",
            "ty:fn",
            "ty:named",
            "ty:dyntrait",
            "ty:qself",
        ]
        .into_iter()
        .collect();
        let missing: Vec<&&str> = expected.difference(&rec.tags).collect();
        assert!(
            missing.is_empty(),
            "walker failed to visit variants: {:?}",
            missing
        );
    }

    /// 全 `Pattern` variant が walker で訪問されることを検証する。
    #[test]
    fn walker_visits_every_pattern_variant() {
        let mut rec = TagRecorder::default();
        for p in all_patterns() {
            rec.visit_pattern(&p);
        }
        let expected: HashSet<&'static str> = [
            "pat:wildcard",
            "pat:literal",
            "pat:binding",
            "pat:tuplestruct",
            "pat:struct",
            "pat:unitstruct",
            "pat:or",
            "pat:range",
            "pat:ref",
            "pat:tuple",
        ]
        .into_iter()
        .collect();
        let missing: Vec<&&str> = expected.difference(&rec.tags).collect();
        assert!(
            missing.is_empty(),
            "walker failed to visit variants: {:?}",
            missing
        );
    }

    /// 全 `Expr` variant が walker で訪問されることを検証する。
    #[test]
    fn walker_visits_every_expr_variant() {
        let mut rec = TagRecorder::default();
        for e in all_exprs() {
            rec.visit_expr(&e);
        }
        let expected: HashSet<&'static str> = [
            "expr:numberlit",
            "expr:intlit",
            "expr:boollit",
            "expr:stringlit",
            "expr:ident",
            "expr:unit",
            "expr:rawcode",
            "expr:regex",
            "expr:formatmacro",
            "expr:fieldaccess",
            "expr:methodcall",
            "expr:structinit",
            "expr:assign",
            "expr:unaryop",
            "expr:binaryop",
            "expr:range",
            "expr:fncall",
            "expr:closure",
            "expr:vec",
            "expr:tuple",
            "expr:if",
            "expr:iflet",
            "expr:macrocall",
            "expr:await",
            "expr:deref",
            "expr:ref",
            "expr:runtimetypeof",
            "expr:index",
            "expr:cast",
            "expr:matches",
            "expr:block",
            "expr:match",
        ]
        .into_iter()
        .collect();
        let missing: Vec<&&str> = expected.difference(&rec.tags).collect();
        assert!(
            missing.is_empty(),
            "walker failed to visit variants: {:?}",
            missing
        );
    }

    /// 全 `Stmt` variant が walker で訪問されることを検証する。
    #[test]
    fn walker_visits_every_stmt_variant() {
        let mut rec = TagRecorder::default();
        for s in all_stmts() {
            rec.visit_stmt(&s);
        }
        let expected: HashSet<&'static str> = [
            "stmt:let",
            "stmt:if",
            "stmt:while",
            "stmt:whilelet",
            "stmt:forin",
            "stmt:loop",
            "stmt:break",
            "stmt:continue",
            "stmt:return",
            "stmt:expr",
            "stmt:tailexpr",
            "stmt:iflet",
            "stmt:match",
            "stmt:labeledblock",
        ]
        .into_iter()
        .collect();
        let missing: Vec<&&str> = expected.difference(&rec.tags).collect();
        assert!(
            missing.is_empty(),
            "walker failed to visit variants: {:?}",
            missing
        );
    }

    /// 全 `Item` variant が walker で訪問されることを検証する。
    #[test]
    fn walker_visits_every_item_variant() {
        let mut rec = TagRecorder::default();
        for item in all_items() {
            rec.visit_item(&item);
        }
        let expected: HashSet<&'static str> = [
            "item:comment",
            "item:use",
            "item:struct",
            "item:enum",
            "item:trait",
            "item:impl",
            "item:typealias",
            "item:fn",
            "item:rawcode",
        ]
        .into_iter()
        .collect();
        let missing: Vec<&&str> = expected.difference(&rec.tags).collect();
        assert!(
            missing.is_empty(),
            "walker failed to visit variants: {:?}",
            missing
        );
    }
}
