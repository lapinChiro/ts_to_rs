//! IR 全 variant 網羅フィクスチャ（テスト専用）。
//!
//! `visit.rs` / `fold.rs` / 将来追加される visitor モジュールの全 variant
//! カバレッジテストで共有する。variant 追加時は対応する fixture 関数に
//! 新 variant を追加するだけで、全 visitor の網羅テストが自動的に新 variant
//! をカバーするようになる。
//!
//! 各 `all_*()` は対応する enum の全 variant を最小構成で 1 つずつ含む
//! `Vec<T>` を返す。ネストした enum（`Pattern::TupleStruct` 内の `Pattern` 等）
//! は `Wildcard` のような最小 variant を使い、組み合わせ爆発を避ける。

#![cfg(test)]

use super::{
    BinOp, CallTarget, ClosureBody, EnumVariant, Expr, Item, MatchArm, Method, Param, Pattern,
    RustType, Stmt, StructField, TraitRef, Visibility,
};

/// 全 `RustType` variant を含む type の束。
pub(crate) fn all_rust_types() -> Vec<RustType> {
    vec![
        RustType::Unit,
        RustType::String,
        RustType::F64,
        RustType::Bool,
        RustType::Any,
        RustType::Never,
        RustType::Option(Box::new(RustType::String)),
        RustType::Vec(Box::new(RustType::F64)),
        RustType::Ref(Box::new(RustType::Bool)),
        RustType::Result {
            ok: Box::new(RustType::Unit),
            err: Box::new(RustType::String),
        },
        RustType::Tuple(vec![RustType::F64, RustType::Bool]),
        RustType::Fn {
            params: vec![RustType::F64],
            return_type: Box::new(RustType::Bool),
        },
        RustType::Named {
            name: "Foo".to_string(),
            type_args: vec![RustType::F64],
        },
        RustType::DynTrait("Animal".to_string()),
        RustType::QSelf {
            qself: Box::new(RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            }),
            trait_ref: TraitRef {
                name: "Iterator".to_string(),
                type_args: vec![RustType::F64],
            },
            item: "Item".to_string(),
        },
    ]
}

/// 全 `Pattern` variant を含む pattern の束。
pub(crate) fn all_patterns() -> Vec<Pattern> {
    vec![
        Pattern::Wildcard,
        Pattern::Literal(Expr::IntLit(1)),
        Pattern::Binding {
            name: "x".to_string(),
            is_mut: true,
            subpat: Some(Box::new(Pattern::Wildcard)),
        },
        Pattern::TupleStruct {
            ctor: super::PatternCtor::UserEnumVariant {
                enum_ty: super::UserTypeRef::new("Color"),
                variant: "Red".to_string(),
            },
            fields: vec![Pattern::Wildcard],
        },
        Pattern::Struct {
            ctor: super::PatternCtor::UserStruct(super::UserTypeRef::new("Foo")),
            fields: vec![("x".to_string(), Pattern::Wildcard)],
            rest: true,
        },
        Pattern::UnitStruct {
            ctor: super::PatternCtor::Builtin(super::BuiltinVariant::None),
        },
        Pattern::Or(vec![Pattern::Wildcard]),
        Pattern::Range {
            start: Some(Box::new(Expr::IntLit(1))),
            end: Some(Box::new(Expr::IntLit(5))),
            inclusive: true,
        },
        Pattern::Ref {
            mutable: true,
            inner: Box::new(Pattern::Wildcard),
        },
        Pattern::Tuple(vec![Pattern::Wildcard]),
    ]
}

/// 全 `Expr` variant を含む expr の束。
pub(crate) fn all_exprs() -> Vec<Expr> {
    vec![
        Expr::NumberLit(1.0),
        Expr::IntLit(1),
        Expr::BoolLit(true),
        Expr::StringLit("s".to_string()),
        Expr::Ident("x".to_string()),
        Expr::Unit,
        Expr::RawCode("raw".to_string()),
        Expr::Regex {
            pattern: "p".to_string(),
            global: false,
            sticky: false,
        },
        Expr::FormatMacro {
            template: "{}".to_string(),
            args: vec![Expr::Unit],
        },
        Expr::FieldAccess {
            object: Box::new(Expr::Unit),
            field: "f".to_string(),
        },
        Expr::MethodCall {
            object: Box::new(Expr::Unit),
            method: "m".to_string(),
            args: vec![Expr::Unit],
        },
        Expr::StructInit {
            name: "S".to_string(),
            fields: vec![("f".to_string(), Expr::Unit)],
            base: Some(Box::new(Expr::Unit)),
        },
        Expr::Assign {
            target: Box::new(Expr::Unit),
            value: Box::new(Expr::Unit),
        },
        Expr::UnaryOp {
            op: super::UnOp::Not,
            operand: Box::new(Expr::Unit),
        },
        Expr::BinaryOp {
            left: Box::new(Expr::Unit),
            op: BinOp::Add,
            right: Box::new(Expr::Unit),
        },
        Expr::Range {
            start: Some(Box::new(Expr::Unit)),
            end: Some(Box::new(Expr::Unit)),
        },
        Expr::FnCall {
            target: CallTarget::Free("f".to_string()),
            args: vec![Expr::Unit],
        },
        Expr::Closure {
            params: vec![],
            return_type: Some(RustType::Unit),
            body: ClosureBody::Expr(Box::new(Expr::Unit)),
        },
        Expr::Vec {
            elements: vec![Expr::Unit],
        },
        Expr::Tuple {
            elements: vec![Expr::Unit],
        },
        Expr::If {
            condition: Box::new(Expr::Unit),
            then_expr: Box::new(Expr::Unit),
            else_expr: Box::new(Expr::Unit),
        },
        Expr::IfLet {
            pattern: Box::new(Pattern::some_binding("x")),
            expr: Box::new(Expr::Unit),
            then_expr: Box::new(Expr::Unit),
            else_expr: Box::new(Expr::Unit),
        },
        Expr::MacroCall {
            name: "println".to_string(),
            args: vec![Expr::Unit],
            use_debug: vec![false],
        },
        Expr::Await(Box::new(Expr::Unit)),
        Expr::Deref(Box::new(Expr::Unit)),
        Expr::Ref(Box::new(Expr::Unit)),
        Expr::RuntimeTypeof {
            operand: Box::new(Expr::Unit),
        },
        Expr::Index {
            object: Box::new(Expr::Unit),
            index: Box::new(Expr::Unit),
        },
        Expr::Cast {
            expr: Box::new(Expr::Unit),
            target: RustType::F64,
        },
        Expr::Matches {
            expr: Box::new(Expr::Unit),
            pattern: Box::new(Pattern::TupleStruct {
                ctor: super::PatternCtor::Builtin(super::BuiltinVariant::Some),
                fields: vec![Pattern::Wildcard],
            }),
        },
        Expr::Block(vec![Stmt::Expr(Expr::Unit)]),
        Expr::Match {
            expr: Box::new(Expr::Unit),
            arms: vec![MatchArm {
                patterns: vec![Pattern::Wildcard],
                guard: None,
                body: vec![],
            }],
        },
        Expr::EnumVariant {
            enum_ty: super::UserTypeRef::new("Color"),
            variant: "Red".to_string(),
        },
        Expr::PrimitiveAssocConst {
            ty: super::PrimitiveType::F64,
            name: "NAN".to_string(),
        },
        Expr::StdConst(super::StdConst::F64Pi),
        Expr::BuiltinVariantValue(super::BuiltinVariant::None),
    ]
}

/// 全 `Stmt` variant を含む stmt の束。
pub(crate) fn all_stmts() -> Vec<Stmt> {
    vec![
        Stmt::Let {
            mutable: false,
            name: "x".to_string(),
            ty: Some(RustType::F64),
            init: Some(Expr::Unit),
        },
        Stmt::If {
            condition: Expr::Unit,
            then_body: vec![],
            else_body: Some(vec![]),
        },
        Stmt::While {
            label: None,
            condition: Expr::Unit,
            body: vec![],
        },
        Stmt::WhileLet {
            label: None,
            pattern: Pattern::some_binding("x"),
            expr: Expr::Unit,
            body: vec![],
        },
        Stmt::ForIn {
            label: None,
            var: "x".to_string(),
            iterable: Expr::Unit,
            body: vec![],
        },
        Stmt::Loop {
            label: None,
            body: vec![],
        },
        Stmt::Break {
            label: None,
            value: Some(Expr::Unit),
        },
        Stmt::Continue { label: None },
        Stmt::Return(Some(Expr::Unit)),
        Stmt::Expr(Expr::Unit),
        Stmt::TailExpr(Expr::Unit),
        Stmt::IfLet {
            pattern: Pattern::some_binding("x"),
            expr: Expr::Unit,
            then_body: vec![],
            else_body: Some(vec![]),
        },
        Stmt::Match {
            expr: Expr::Unit,
            arms: vec![],
        },
        Stmt::LabeledBlock {
            label: "lbl".to_string(),
            body: vec![],
        },
    ]
}

/// 全 `Item` variant を含む item の束。
pub(crate) fn all_items() -> Vec<Item> {
    vec![
        Item::Comment("c".to_string()),
        Item::Use {
            vis: Visibility::Private,
            path: "crate::foo".to_string(),
            names: vec!["Bar".to_string()],
        },
        Item::Struct {
            vis: Visibility::Public,
            name: "S".to_string(),
            type_params: vec![],
            fields: vec![StructField {
                vis: None,
                name: "x".to_string(),
                ty: RustType::F64,
            }],
        },
        Item::Enum {
            vis: Visibility::Public,
            name: "E".to_string(),
            type_params: vec![],
            serde_tag: None,
            variants: vec![EnumVariant {
                name: "A".to_string(),
                value: None,
                data: Some(RustType::F64),
                fields: vec![],
            }],
        },
        Item::Trait {
            vis: Visibility::Public,
            name: "T".to_string(),
            type_params: vec![],
            supertraits: vec![],
            methods: vec![],
            associated_types: vec![],
        },
        Item::Impl {
            struct_name: "S".to_string(),
            type_params: vec![],
            for_trait: None,
            consts: vec![],
            methods: vec![Method {
                vis: Visibility::Public,
                name: "m".to_string(),
                is_async: false,
                has_self: true,
                has_mut_self: false,
                params: vec![Param {
                    name: "x".to_string(),
                    ty: Some(RustType::F64),
                }],
                return_type: Some(RustType::Bool),
                body: Some(vec![]),
            }],
        },
        Item::TypeAlias {
            vis: Visibility::Public,
            name: "A".to_string(),
            type_params: vec![],
            ty: RustType::F64,
        },
        Item::Fn {
            vis: Visibility::Public,
            attributes: vec![],
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![],
        },
        Item::Const {
            vis: Visibility::Private,
            name: "MY_CONST".to_string(),
            ty: RustType::F64,
            value: Expr::NumberLit(42.0),
        },
        Item::RawCode("rc".to_string()),
    ]
}
