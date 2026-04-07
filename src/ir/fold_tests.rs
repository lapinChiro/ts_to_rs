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
        ctor: PatternCtor::Builtin(crate::ir::BuiltinVariant::Some),
        fields: vec![Pattern::binding("x")],
    };
    let result = IdentityFolder.fold_pattern(pat.clone());
    assert_eq!(result, pat);
}

#[test]
fn fold_pattern_ctor_routes_user_type_ref_through_hook() {
    // I-380: walk_pattern_ctor が UserEnumVariant::enum_ty / UserStruct(_) を
    // fold_user_type_ref 経由で書き換えることを確認する。
    struct RenameUserType;
    impl IrFolder for RenameUserType {
        fn fold_user_type_ref(&mut self, r: UserTypeRef) -> UserTypeRef {
            UserTypeRef::new(format!("{}_renamed", r.as_str()))
        }
    }

    // UserEnumVariant
    let pat = Pattern::TupleStruct {
        ctor: PatternCtor::UserEnumVariant {
            enum_ty: UserTypeRef::new("Color"),
            variant: "Red".to_string(),
        },
        fields: vec![Pattern::Wildcard],
    };
    match RenameUserType.fold_pattern(pat) {
        Pattern::TupleStruct {
            ctor: PatternCtor::UserEnumVariant { enum_ty, variant },
            ..
        } => {
            assert_eq!(enum_ty.as_str(), "Color_renamed");
            assert_eq!(variant, "Red");
        }
        other => panic!("expected UserEnumVariant, got {other:?}"),
    }

    // UserStruct
    let pat = Pattern::UnitStruct {
        ctor: PatternCtor::UserStruct(UserTypeRef::new("Foo")),
    };
    match RenameUserType.fold_pattern(pat) {
        Pattern::UnitStruct {
            ctor: PatternCtor::UserStruct(ty),
        } => {
            assert_eq!(ty.as_str(), "Foo_renamed");
        }
        other => panic!("expected UserStruct, got {other:?}"),
    }

    // Builtin: 通過する (恒等)
    let pat = Pattern::UnitStruct {
        ctor: PatternCtor::Builtin(crate::ir::BuiltinVariant::None),
    };
    let result = RenameUserType.fold_pattern(pat.clone());
    assert_eq!(result, pat);
}

/// I-380: `IrFolder::fold_trait_ref` が QSelf / Impl::for_trait /
/// Trait::supertraits の各構築サイトから発火することを検証する。
/// `IrVisitor::visit_trait_ref` の fold 対称版テスト。
#[test]
fn fold_trait_ref_fires_from_qself_impl_and_trait_supertraits() {
    use crate::ir::{Method, Visibility};

    struct RenameTrait;
    impl IrFolder for RenameTrait {
        fn fold_trait_ref(&mut self, tr: TraitRef) -> TraitRef {
            let renamed = TraitRef {
                name: format!("{}_renamed", tr.name),
                type_args: tr.type_args,
            };
            walk_trait_ref(self, renamed)
        }
    }

    // 1. QSelf の trait_ref
    let ty = RustType::QSelf {
        qself: Box::new(RustType::Named {
            name: "T".to_string(),
            type_args: vec![],
        }),
        trait_ref: TraitRef {
            name: "Promise".to_string(),
            type_args: vec![],
        },
        item: "Output".to_string(),
    };
    match RenameTrait.fold_rust_type(ty) {
        RustType::QSelf { trait_ref, .. } => {
            assert_eq!(trait_ref.name, "Promise_renamed");
        }
        other => panic!("expected QSelf, got {other:?}"),
    }

    // 2. Item::Impl::for_trait
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![],
        for_trait: Some(TraitRef {
            name: "Display".to_string(),
            type_args: vec![],
        }),
        consts: vec![],
        methods: vec![],
    };
    match RenameTrait.fold_item(item) {
        Item::Impl { for_trait, .. } => {
            assert_eq!(for_trait.unwrap().name, "Display_renamed");
        }
        other => panic!("expected Impl, got {other:?}"),
    }

    // 3. Item::Trait::supertraits
    let item = Item::Trait {
        vis: Visibility::Public,
        name: "Greeter".to_string(),
        type_params: vec![],
        supertraits: vec![
            TraitRef {
                name: "Debug".to_string(),
                type_args: vec![],
            },
            TraitRef {
                name: "Clone".to_string(),
                type_args: vec![],
            },
        ],
        methods: Vec::<Method>::new(),
        associated_types: vec![],
    };
    match RenameTrait.fold_item(item) {
        Item::Trait { supertraits, .. } => {
            let names: Vec<String> = supertraits.into_iter().map(|t| t.name).collect();
            assert_eq!(names, vec!["Debug_renamed", "Clone_renamed"]);
        }
        other => panic!("expected Trait, got {other:?}"),
    }
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

/// `walk_expr` の `Expr::EnumVariant` 分岐が `fold_user_type_ref` フックを
/// 経由して enum_ty を折りたたむことを検証する。識別 fold (`r → r`) でも
/// 経路上にフックが配置されていることが Phase 2 の前提条件。
#[test]
fn walk_expr_enum_variant_routes_through_fold_user_type_ref() {
    struct PrefixFolder;
    impl IrFolder for PrefixFolder {
        fn fold_user_type_ref(&mut self, r: super::UserTypeRef) -> super::UserTypeRef {
            super::UserTypeRef::new(format!("Prefixed_{}", r.as_str()))
        }
    }

    let expr = Expr::EnumVariant {
        enum_ty: super::UserTypeRef::new("Color"),
        variant: "Red".to_string(),
    };

    let folded = PrefixFolder.fold_expr(expr);
    match folded {
        Expr::EnumVariant { enum_ty, variant } => {
            assert_eq!(enum_ty.as_str(), "Prefixed_Color");
            assert_eq!(variant, "Red");
        }
        other => panic!("expected EnumVariant, got {other:?}"),
    }
}

/// `walk_call_target` 経由で `CallTarget::UserAssocFn` / `UserTupleCtor` /
/// `UserEnumVariantCtor` のいずれもが `fold_user_type_ref` フックを経由する
/// ことを検証する (Phase 2 で追加された walk_call_target フック配線の保証)。
#[test]
fn walk_call_target_user_variants_all_route_through_fold_user_type_ref() {
    struct PrefixFolder;
    impl IrFolder for PrefixFolder {
        fn fold_user_type_ref(&mut self, r: crate::ir::UserTypeRef) -> crate::ir::UserTypeRef {
            crate::ir::UserTypeRef::new(format!("Prefixed_{}", r.as_str()))
        }
    }

    // UserAssocFn
    let folded = PrefixFolder.fold_expr(Expr::FnCall {
        target: CallTarget::UserAssocFn {
            ty: crate::ir::UserTypeRef::new("MyClass"),
            method: "new".to_string(),
        },
        args: vec![],
    });
    match folded {
        Expr::FnCall {
            target: CallTarget::UserAssocFn { ty, .. },
            ..
        } => assert_eq!(ty.as_str(), "Prefixed_MyClass"),
        _ => panic!("expected UserAssocFn"),
    }

    // UserTupleCtor
    let folded = PrefixFolder.fold_expr(Expr::FnCall {
        target: CallTarget::UserTupleCtor(crate::ir::UserTypeRef::new("Wrapper")),
        args: vec![],
    });
    match folded {
        Expr::FnCall {
            target: CallTarget::UserTupleCtor(ty),
            ..
        } => assert_eq!(ty.as_str(), "Prefixed_Wrapper"),
        _ => panic!("expected UserTupleCtor"),
    }

    // UserEnumVariantCtor
    let folded = PrefixFolder.fold_expr(Expr::FnCall {
        target: CallTarget::UserEnumVariantCtor {
            enum_ty: crate::ir::UserTypeRef::new("Color"),
            variant: "Red".to_string(),
        },
        args: vec![],
    });
    match folded {
        Expr::FnCall {
            target: CallTarget::UserEnumVariantCtor { enum_ty, .. },
            ..
        } => assert_eq!(enum_ty.as_str(), "Prefixed_Color"),
        _ => panic!("expected UserEnumVariantCtor"),
    }
}

/// `walk_call_target` の non-user variant (`Free` / `BuiltinVariant` /
/// `ExternalPath` / `Super`) は `fold_user_type_ref` フックを発火しない
/// ことを検証する。
#[test]
fn walk_call_target_non_user_variants_bypass_fold_user_type_ref() {
    struct PanicOnUserTypeRef;
    impl IrFolder for PanicOnUserTypeRef {
        fn fold_user_type_ref(&mut self, _r: crate::ir::UserTypeRef) -> crate::ir::UserTypeRef {
            panic!("non-user CallTarget variant must NOT route through fold_user_type_ref");
        }
    }

    let cases = vec![
        CallTarget::Free("foo".to_string()),
        CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Some),
        CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::None),
        CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Ok),
        CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Err),
        CallTarget::ExternalPath(vec!["std".to_string(), "fs".to_string()]),
        CallTarget::Super,
    ];

    for target in cases {
        let original = Expr::FnCall {
            target: target.clone(),
            args: vec![],
        };
        let folded = PanicOnUserTypeRef.fold_expr(original.clone());
        assert_eq!(
            folded, original,
            "non-user variant must round-trip identity"
        );
    }
}

/// PrimitiveAssocConst / StdConst は user type ref を持たないため
/// `fold_user_type_ref` フックを経由せず識別変換されることを検証する。
#[test]
fn walk_expr_primitive_and_std_const_bypass_fold_user_type_ref() {
    struct PanicOnUserTypeRef;
    impl IrFolder for PanicOnUserTypeRef {
        fn fold_user_type_ref(&mut self, _r: super::UserTypeRef) -> super::UserTypeRef {
            panic!("fold_user_type_ref must NOT be called for PrimitiveAssocConst/StdConst");
        }
    }

    let p = Expr::PrimitiveAssocConst {
        ty: crate::ir::PrimitiveType::F64,
        name: "NAN".to_string(),
    };
    let s = Expr::StdConst(crate::ir::StdConst::F64Pi);

    // Both should fold to themselves without invoking fold_user_type_ref
    assert_eq!(PanicOnUserTypeRef.fold_expr(p.clone()), p);
    assert_eq!(PanicOnUserTypeRef.fold_expr(s.clone()), s);
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
