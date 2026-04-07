use super::*;

#[test]
fn test_collect_type_refs_fn_body_let_binding_type() {
    // fn f() { let x: Foo = ...; } → Foo が refs
    let item = fn_with_body(
        "f",
        vec![Stmt::Let {
            mutable: false,
            name: "x".to_string(),
            ty: Some(named("Foo")),
            init: None,
        }],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Foo"));
}

#[test]
fn test_collect_type_refs_fn_body_struct_init() {
    // fn f() { Wrapper { x: 1 } } → Wrapper が refs
    let item = fn_with_body(
        "f",
        vec![Stmt::TailExpr(Expr::StructInit {
            name: "Wrapper".to_string(),
            fields: vec![("x".to_string(), Expr::IntLit(1))],
            base: None,
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Wrapper"));
}

#[test]
fn test_collect_type_refs_fn_body_struct_init_self_excluded() {
    // fn f() { Self { x: 1 } } → Self は除外
    let item = fn_with_body(
        "f",
        vec![Stmt::TailExpr(Expr::StructInit {
            name: "Self".to_string(),
            fields: vec![],
            base: None,
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(!refs.contains("Self"));
}

#[test]
fn test_collect_type_refs_fn_body_cast_target() {
    // fn f() { x as Foo } → Foo が refs
    let item = fn_with_body(
        "f",
        vec![Stmt::TailExpr(Expr::Cast {
            expr: Box::new(Expr::Ident("x".to_string())),
            target: named("Foo"),
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Foo"));
}

#[test]
fn test_collect_type_refs_fn_body_fncall_uppercase_extracted() {
    // fn f() { Color::Red(x) } — a synthetic enum variant constructor.
    // The Transformer constructs this as `CallTarget::UserAssocFn { ty: crate::ir::UserTypeRef::new("Color"), method: "Red".to_string() }`
    // with `type_ref = Some("Color")`, so the walker registers `Color` in refs.
    let item = fn_with_body(
        "f",
        vec![Stmt::Expr(Expr::FnCall {
            target: CallTarget::UserAssocFn {
                ty: crate::ir::UserTypeRef::new("Color"),
                method: "Red".to_string(),
            },
            args: vec![],
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Color"));
}

#[test]
fn test_collect_type_refs_fn_body_fncall_module_qualified_not_registered() {
    // fn f() { scopeguard::guard(x) } — a module-qualified free function call.
    // `type_ref = None`, so nothing is registered.
    let item = fn_with_body(
        "f",
        vec![Stmt::Expr(Expr::FnCall {
            target: CallTarget::ExternalPath(vec!["scopeguard".to_string(), "guard".to_string()]),
            args: vec![],
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(!refs.contains("scopeguard"));
    assert!(!refs.contains("guard"));
}

#[test]
fn test_collect_type_refs_fn_body_fncall_walks_args() {
    // fn f() { foo(Bar { x: 1 }) } → 小文字 foo は登録されないが args の Bar は登録される
    let item = fn_with_body(
        "f",
        vec![Stmt::Expr(Expr::FnCall {
            target: CallTarget::Free("foo".to_string()),
            args: vec![Expr::StructInit {
                name: "Bar".to_string(),
                fields: vec![],
                base: None,
            }],
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Bar"));
}

#[test]
fn test_collect_type_refs_fn_body_closure_param_and_return() {
    // fn f() { |x: Foo| -> Bar { ... } } → Foo, Bar が refs
    let item = fn_with_body(
        "f",
        vec![Stmt::TailExpr(Expr::Closure {
            params: vec![Param {
                name: "x".to_string(),
                ty: Some(named("Foo")),
            }],
            return_type: Some(named("Bar")),
            body: ClosureBody::Expr(Box::new(Expr::Ident("x".to_string()))),
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Foo"));
    assert!(refs.contains("Bar"));
}

#[test]
fn test_collect_type_refs_fn_body_match_arm_body_walked() {
    // fn f() { match x { _ => { let y: Foo = ...; } } } → Foo
    let item = fn_with_body(
        "f",
        vec![Stmt::Match {
            expr: Expr::Ident("x".to_string()),
            arms: vec![MatchArm {
                patterns: vec![],
                guard: None,
                body: vec![Stmt::Let {
                    mutable: false,
                    name: "y".to_string(),
                    ty: Some(named("Foo")),
                    init: None,
                }],
            }],
        }],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Foo"));
}

#[test]
fn test_collect_type_refs_impl_method_body_walked() {
    // impl Foo { fn m(&self) { Bar { x: 1 } } } → Bar が refs
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![],
        for_trait: None,
        consts: vec![],
        methods: vec![Method {
            vis: Visibility::Public,
            name: "m".to_string(),
            has_self: true,
            has_mut_self: false,
            params: vec![],
            return_type: None,
            body: Some(vec![Stmt::TailExpr(Expr::StructInit {
                name: "Bar".to_string(),
                fields: vec![],
                base: None,
            })]),
        }],
    };
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Bar"));
}

#[test]
fn test_collect_type_refs_impl_assoc_const_value_walked() {
    // impl Foo { const X: Bar = SomeFn(Baz { }); } — the walker must recurse
    // through the const initializer expression. The call target here is a
    // plain free function (`type_ref: None`, so not registered), but its
    // argument is a `StructInit { name: "Baz" }` which *must* be walked and
    // registered. This asserts the walker traverses `AssocConst::value`.
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![],
        for_trait: None,
        consts: vec![AssocConst {
            vis: Visibility::Public,
            name: "X".to_string(),
            ty: RustType::Named {
                name: "Bar".to_string(),
                type_args: vec![],
            },
            value: Expr::FnCall {
                target: CallTarget::Free("some_fn".to_string()),
                args: vec![Expr::StructInit {
                    name: "Baz".to_string(),
                    fields: vec![],
                    base: None,
                }],
            },
        }],
        methods: vec![],
    };
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Bar"), "type annotation should register Bar");
    assert!(
        refs.contains("Baz"),
        "walker must recurse into FnCall args → StructInit and register Baz"
    );
    assert!(
        !refs.contains("some_fn"),
        "free function call without `type_ref` must not register the call name"
    );
}

#[test]
fn test_collect_type_refs_fn_body_binary_op_walks_both_sides() {
    // fn f() { Wrapper{x:1} + Wrapper2{x:1} } — 両辺の StructInit を拾う
    let item = fn_with_body(
        "f",
        vec![Stmt::TailExpr(Expr::BinaryOp {
            left: Box::new(Expr::StructInit {
                name: "Wrapper".to_string(),
                fields: vec![],
                base: None,
            }),
            op: BinOp::Add,
            right: Box::new(Expr::StructInit {
                name: "Wrapper2".to_string(),
                fields: vec![],
                base: None,
            }),
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Wrapper"));
    assert!(refs.contains("Wrapper2"));
}

// =========================================================================
// T8b: collect_type_refs_from_item — type_params constraint walking
// =========================================================================

#[test]
fn test_collect_type_refs_struct_type_param_constraint() {
    // struct S<T: SomeTrait> { f: T } → SomeTrait が refs に入る
    let item = Item::Struct {
        vis: Visibility::Public,
        name: "S".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: Some(named("SomeTrait")),
        }],
        fields: vec![],
    };
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("SomeTrait"));
}

#[test]
fn test_collect_type_refs_fn_type_param_constraint_with_generics() {
    // fn f<T: Container<Inner>>() → Container, Inner が refs に入る
    let item = fn_with_body_and_type_params(
        "f",
        vec![TypeParam {
            name: "T".to_string(),
            constraint: Some(RustType::Named {
                name: "Container".to_string(),
                type_args: vec![named("Inner")],
            }),
        }],
        vec![],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Container"));
    assert!(refs.contains("Inner"));
}

#[test]
fn test_collect_type_refs_impl_type_param_constraint() {
    // impl<T: Bar> Foo<T> { } → Bar が refs
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: Some(named("Bar")),
        }],
        for_trait: None,
        consts: vec![],
        methods: vec![],
    };
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Bar"));
}

#[test]
fn test_collect_type_refs_trait_type_param_constraint() {
    // trait Foo<T: Bar> { } → Bar が refs
    let item = Item::Trait {
        vis: Visibility::Public,
        name: "Foo".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: Some(named("Bar")),
        }],
        supertraits: vec![],
        methods: vec![],
        associated_types: vec![],
    };
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Bar"));
}

#[test]
fn test_collect_type_refs_type_alias_type_param_constraint() {
    // type Foo<T: Bar> = T → Bar が refs
    let item = Item::TypeAlias {
        vis: Visibility::Public,
        name: "Foo".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: Some(named("Bar")),
        }],
        ty: named("T"),
    };
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Bar"));
}

#[test]
fn test_collect_type_refs_enum_type_param_constraint() {
    // enum E<T: Bar> { Variant(T) } → Bar が refs
    let item = Item::Enum {
        vis: Visibility::Public,
        name: "E".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: Some(named("Bar")),
        }],
        serde_tag: None,
        variants: vec![],
    };
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Bar"));
}
