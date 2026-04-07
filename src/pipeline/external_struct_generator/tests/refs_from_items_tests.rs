use super::*;

#[test]
fn test_collect_type_refs_from_type_alias() {
    // type Foo = Bar; → Bar が refs に入る
    let item = Item::TypeAlias {
        vis: Visibility::Public,
        name: "Foo".to_string(),
        type_params: vec![],
        ty: RustType::Named {
            name: "Bar".to_string(),
            type_args: vec![],
        },
    };
    let mut refs = std::collections::HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Bar"), "TypeAlias の右辺型が refs に入るべき");
}

#[test]
fn test_collect_type_refs_from_type_alias_nested() {
    // type Foo = Vec<Bar>; → Bar が refs に入る
    let item = Item::TypeAlias {
        vis: Visibility::Public,
        name: "Foo".to_string(),
        type_params: vec![],
        ty: RustType::Vec(Box::new(RustType::Named {
            name: "Bar".to_string(),
            type_args: vec![],
        })),
    };
    let mut refs = std::collections::HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Bar"));
}

#[test]
fn test_collect_type_refs_from_impl_for_trait() {
    // impl<T> MyTrait<U> for Foo { } → MyTrait と U が refs に入る
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![],
        for_trait: Some(TraitRef {
            name: "MyTrait".to_string(),
            type_args: vec![RustType::Named {
                name: "U".to_string(),
                type_args: vec![],
            }],
        }),
        consts: vec![],
        methods: vec![],
    };
    let mut refs = std::collections::HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("MyTrait"), "trait 名が refs に入るべき");
    assert!(refs.contains("U"), "trait の type_args が refs に入るべき");
}

#[test]
fn test_collect_type_refs_from_impl_method_signature() {
    // impl Foo { fn bar(x: Baz) -> Qux { ... } } → Baz と Qux が refs に入る
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![],
        for_trait: None,
        consts: vec![],
        methods: vec![Method {
            vis: Visibility::Public,
            name: "bar".to_string(),
            has_self: true,
            has_mut_self: false,
            params: vec![Param {
                name: "x".to_string(),
                ty: Some(RustType::Named {
                    name: "Baz".to_string(),
                    type_args: vec![],
                }),
            }],
            return_type: Some(RustType::Named {
                name: "Qux".to_string(),
                type_args: vec![],
            }),
            body: Some(vec![]),
        }],
    };
    let mut refs = std::collections::HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(
        refs.contains("Baz"),
        "impl method の param 型が refs に入るべき"
    );
    assert!(
        refs.contains("Qux"),
        "impl method の return 型が refs に入るべき"
    );
}

#[test]
fn test_collect_type_refs_from_trait_method_signature() {
    // trait Foo { fn bar(x: Baz) -> Qux; } → Baz と Qux が refs に入る
    let item = Item::Trait {
        vis: Visibility::Public,
        name: "Foo".to_string(),
        type_params: vec![],
        supertraits: vec![],
        methods: vec![Method {
            vis: Visibility::Public,
            name: "bar".to_string(),
            has_self: true,
            has_mut_self: false,
            params: vec![Param {
                name: "x".to_string(),
                ty: Some(RustType::Named {
                    name: "Baz".to_string(),
                    type_args: vec![],
                }),
            }],
            return_type: Some(RustType::Named {
                name: "Qux".to_string(),
                type_args: vec![],
            }),
            body: None, // trait method signature
        }],
        associated_types: vec![],
    };
    let mut refs = std::collections::HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(
        refs.contains("Baz"),
        "trait method signature の param 型が refs に入るべき"
    );
    assert!(
        refs.contains("Qux"),
        "trait method signature の return 型が refs に入るべき"
    );
}

#[test]
fn test_collect_type_refs_from_impl_consts() {
    // impl Foo { pub const MAX: SomeType = ...; } → SomeType が refs に入る
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![],
        for_trait: None,
        consts: vec![AssocConst {
            vis: Visibility::Public,
            name: "MAX".to_string(),
            ty: RustType::Named {
                name: "SomeType".to_string(),
                type_args: vec![],
            },
            value: Expr::NumberLit(0.0),
        }],
        methods: vec![],
    };
    let mut refs = std::collections::HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(
        refs.contains("SomeType"),
        "impl の AssocConst.ty が refs に入るべき"
    );
}

#[test]
fn test_collect_type_refs_excludes_self() {
    // impl Foo { fn new(...) -> Self { ... } } で `Self` は ref に含まれない。
    // Self を含めると stub generator が `pub struct Self {}` を生成し
    // Rust の予約語衝突でコンパイル不可になるための回避（band-aid）。
    // 根本治療は IR レベルでの型名サニタイズ（I-374 参照）。
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![],
        for_trait: None,
        consts: vec![],
        methods: vec![Method {
            vis: Visibility::Public,
            name: "new".to_string(),
            has_self: false,
            has_mut_self: false,
            params: vec![],
            return_type: Some(RustType::Named {
                name: "Self".to_string(),
                type_args: vec![],
            }),
            body: Some(vec![]),
        }],
    };
    let mut refs = std::collections::HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(
        !refs.contains("Self"),
        "`Self` は ref 収集対象から除外される"
    );
}

#[test]
fn test_collect_type_refs_from_trait_supertraits() {
    // trait Foo: Bar + Baz<T> { } → Bar, Baz, T が refs に入る
    let item = Item::Trait {
        vis: Visibility::Public,
        name: "Foo".to_string(),
        type_params: vec![],
        supertraits: vec![
            TraitRef {
                name: "Bar".to_string(),
                type_args: vec![],
            },
            TraitRef {
                name: "Baz".to_string(),
                type_args: vec![RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![],
                }],
            },
        ],
        methods: vec![],
        associated_types: vec![],
    };
    let mut refs = std::collections::HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Bar"));
    assert!(refs.contains("Baz"));
    assert!(refs.contains("T"));
}

// =========================================================================
// T7: collect_type_refs_from_rust_type — RustType::QSelf 強化
// =========================================================================

#[test]
fn test_collect_type_refs_qself_extracts_qself_and_trait_name() {
    // <T as Promise>::Output → refs に T と Promise が入る（item 名 Output は除外）
    let ty = RustType::QSelf {
        qself: Box::new(named("T")),
        trait_ref: TraitRef {
            name: "Promise".to_string(),
            type_args: vec![],
        },
        item: "Output".to_string(),
    };
    let mut refs = HashSet::new();
    collect_type_refs_from_rust_type(&ty, &mut refs);
    assert!(refs.contains("T"));
    assert!(refs.contains("Promise"));
    assert!(!refs.contains("Output"));
}

#[test]
fn test_collect_type_refs_qself_walks_trait_type_args() {
    // <T as Container<Inner>>::Item → T, Container, Inner が入る
    let ty = RustType::QSelf {
        qself: Box::new(named("T")),
        trait_ref: TraitRef {
            name: "Container".to_string(),
            type_args: vec![named("Inner")],
        },
        item: "Item".to_string(),
    };
    let mut refs = HashSet::new();
    collect_type_refs_from_rust_type(&ty, &mut refs);
    assert!(refs.contains("T"));
    assert!(refs.contains("Container"));
    assert!(refs.contains("Inner"));
    assert!(!refs.contains("Item"));
}

#[test]
fn test_collect_type_refs_qself_walks_qself_inner() {
    // <Vec<Foo> as Promise>::Output → Vec, Foo, Promise が入る
    let ty = RustType::QSelf {
        qself: Box::new(RustType::Vec(Box::new(named("Foo")))),
        trait_ref: TraitRef {
            name: "Promise".to_string(),
            type_args: vec![],
        },
        item: "Output".to_string(),
    };
    let mut refs = HashSet::new();
    collect_type_refs_from_rust_type(&ty, &mut refs);
    assert!(refs.contains("Foo"));
    assert!(refs.contains("Promise"));
}

#[test]
fn test_collect_type_refs_dyn_trait_records_trait_name() {
    // dyn Greeter → refs に Greeter が入る
    let ty = RustType::DynTrait("Greeter".to_string());
    let mut refs = HashSet::new();
    collect_type_refs_from_rust_type(&ty, &mut refs);
    assert!(refs.contains("Greeter"));
}
