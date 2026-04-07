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
fn match_arm_walker_visits_patterns_and_guard() {
    // `match x { 1 | 2 if flag => {} }`
    let arm = MatchArm {
        patterns: vec![
            Pattern::Literal(Expr::IntLit(1)),
            Pattern::Literal(Expr::IntLit(2)),
        ],
        guard: Some(Expr::Ident("flag".to_string())),
        body: vec![],
    };

    let mut counter = NodeCounter::default();
    counter.visit_match_arm(&arm);

    assert_eq!(counter.arms, 1);
    // 2 `Pattern::Literal` → walker routes through `visit_pattern` for each
    // (2 patterns) and then `walk_pattern` descends into the inner literal expr.
    assert_eq!(counter.patterns, 2);
    // 2 literal inner exprs + 1 guard = 3
    assert_eq!(counter.exprs, 3);
}

#[test]
fn fn_call_walker_descends_into_args_only() {
    // foo(x, y)
    let expr = Expr::FnCall {
        target: CallTarget::Free("foo".to_string()),
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
            Expr::EnumVariant { .. } => "expr:enumvariant",
            Expr::PrimitiveAssocConst { .. } => "expr:primitiveassocconst",
            Expr::StdConst(_) => "expr:stdconst",
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
        "expr:enumvariant",
        "expr:primitiveassocconst",
        "expr:stdconst",
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

/// `walk_expr` の `Expr::EnumVariant` 分岐が `visit_user_type_ref` フックを
/// 発火することを検証する。`external_struct_generator::TypeRefCollector` の
/// `IrVisitor` 実装が本フックの override だけで EnumVariant の親 enum 型を
/// refs に登録できる構造を保証するセーフティネット。
#[test]
fn walk_expr_enum_variant_fires_visit_user_type_ref_hook() {
    #[derive(Default)]
    struct UserTypeRefRecorder {
        seen: Vec<String>,
    }
    impl IrVisitor for UserTypeRefRecorder {
        fn visit_user_type_ref(&mut self, r: &crate::ir::UserTypeRef) {
            self.seen.push(r.as_str().to_string());
        }
    }

    let expr = Expr::EnumVariant {
        enum_ty: crate::ir::UserTypeRef::new("Color"),
        variant: "Red".to_string(),
    };

    let mut rec = UserTypeRefRecorder::default();
    rec.visit_expr(&expr);

    assert_eq!(
        rec.seen,
        vec!["Color".to_string()],
        "walk_expr must invoke visit_user_type_ref for Expr::EnumVariant::enum_ty"
    );
}

/// `walk_call_target` 経由で `CallTarget::UserAssocFn` / `UserTupleCtor` /
/// `UserEnumVariantCtor` のいずれもが `visit_user_type_ref` フックを発火する
/// ことを構造的に検証する。`walker_tests.rs` の挙動テストとは独立に、フック
/// 配線が機能していることを確認するセーフティネット。
#[test]
fn walk_call_target_user_variants_all_fire_visit_user_type_ref_hook() {
    use crate::ir::UserTypeRef;

    #[derive(Default)]
    struct UserTypeRefRecorder {
        seen: Vec<String>,
    }
    impl IrVisitor for UserTypeRefRecorder {
        fn visit_user_type_ref(&mut self, r: &UserTypeRef) {
            self.seen.push(r.as_str().to_string());
        }
    }

    // UserAssocFn
    let mut rec = UserTypeRefRecorder::default();
    rec.visit_expr(&Expr::FnCall {
        target: CallTarget::UserAssocFn {
            ty: UserTypeRef::new("MyClass"),
            method: "new".to_string(),
        },
        args: vec![],
    });
    assert_eq!(rec.seen, vec!["MyClass".to_string()]);

    // UserTupleCtor
    let mut rec = UserTypeRefRecorder::default();
    rec.visit_expr(&Expr::FnCall {
        target: CallTarget::UserTupleCtor(UserTypeRef::new("Wrapper")),
        args: vec![],
    });
    assert_eq!(rec.seen, vec!["Wrapper".to_string()]);

    // UserEnumVariantCtor
    let mut rec = UserTypeRefRecorder::default();
    rec.visit_expr(&Expr::FnCall {
        target: CallTarget::UserEnumVariantCtor {
            enum_ty: UserTypeRef::new("Color"),
            variant: "Red".to_string(),
        },
        args: vec![],
    });
    assert_eq!(rec.seen, vec!["Color".to_string()]);
}

/// `walk_call_target` の non-user variant (`Free` / `BuiltinVariant` /
/// `ExternalPath` / `Super`) は `visit_user_type_ref` フックを発火しない
/// ことを検証する。これにより walker は builtin / 外部 path を user type
/// として誤登録しない構造的保証を持つ。
#[test]
fn walk_call_target_non_user_variants_never_fire_visit_user_type_ref_hook() {
    use crate::ir::{BuiltinVariant, UserTypeRef};

    struct PanicOnUserTypeRef;
    impl IrVisitor for PanicOnUserTypeRef {
        fn visit_user_type_ref(&mut self, r: &UserTypeRef) {
            panic!(
                "non-user CallTarget variant must NOT fire visit_user_type_ref, \
                 got {:?}",
                r.as_str()
            );
        }
    }

    let cases = vec![
        Expr::FnCall {
            target: CallTarget::Free("foo".to_string()),
            args: vec![],
        },
        Expr::FnCall {
            target: CallTarget::BuiltinVariant(BuiltinVariant::Some),
            args: vec![],
        },
        Expr::FnCall {
            target: CallTarget::BuiltinVariant(BuiltinVariant::None),
            args: vec![],
        },
        Expr::FnCall {
            target: CallTarget::BuiltinVariant(BuiltinVariant::Ok),
            args: vec![],
        },
        Expr::FnCall {
            target: CallTarget::BuiltinVariant(BuiltinVariant::Err),
            args: vec![],
        },
        Expr::FnCall {
            target: CallTarget::ExternalPath(vec![
                "std".to_string(),
                "fs".to_string(),
                "write".to_string(),
            ]),
            args: vec![],
        },
        Expr::FnCall {
            target: CallTarget::Super,
            args: vec![],
        },
    ];

    for expr in cases {
        PanicOnUserTypeRef.visit_expr(&expr);
    }
}

/// プリミティブ assoc const と std const は user type ref を持たないため
/// `visit_user_type_ref` フックは発火しないことを検証する。
#[test]
fn walk_expr_primitive_and_std_const_do_not_fire_user_type_ref_hook() {
    #[derive(Default)]
    struct UserTypeRefRecorder {
        seen: Vec<String>,
    }
    impl IrVisitor for UserTypeRefRecorder {
        fn visit_user_type_ref(&mut self, r: &crate::ir::UserTypeRef) {
            self.seen.push(r.as_str().to_string());
        }
    }

    let mut rec = UserTypeRefRecorder::default();
    rec.visit_expr(&Expr::PrimitiveAssocConst {
        ty: crate::ir::PrimitiveType::F64,
        name: "NAN".to_string(),
    });
    rec.visit_expr(&Expr::StdConst(crate::ir::StdConst::F64Pi));

    assert!(
        rec.seen.is_empty(),
        "PrimitiveAssocConst / StdConst must NOT register user type refs, got {:?}",
        rec.seen
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
