use super::*;

#[test]
fn test_collect_all_undefined_refs_includes_non_external() {
    // registry に登録されていない型も検出する（is_external フィルタなし）
    let items = vec![Item::Enum {
        vis: Visibility::Public,
        name: "MyEnum".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![EnumVariant {
            name: "A".to_string(),
            data: Some(RustType::Named {
                name: "UnknownType".to_string(),
                type_args: vec![],
            }),
            fields: vec![],
            value: None,
        }],
    }];
    let refs = collect_all_undefined_references(&items, &[], &[]);
    assert!(refs.contains("UnknownType"));
}

#[test]
fn test_collect_all_undefined_refs_excludes_defined() {
    let items = vec![
        Item::Struct {
            vis: Visibility::Public,
            name: "Foo".to_string(),
            type_params: vec![],
            fields: vec![],
        },
        Item::Enum {
            vis: Visibility::Public,
            name: "MyEnum".to_string(),
            type_params: vec![],
            serde_tag: None,
            variants: vec![EnumVariant {
                name: "A".to_string(),
                data: Some(RustType::Named {
                    name: "Foo".to_string(),
                    type_args: vec![],
                }),
                fields: vec![],
                value: None,
            }],
        },
    ];
    let refs = collect_all_undefined_references(&items, &[], &[]);
    assert!(!refs.contains("Foo"), "defined types should be excluded");
}

#[test]
fn test_generate_stub_structs_creates_empty_stubs() {
    let registry = TypeRegistry::new();
    let mut items = vec![Item::Enum {
        vis: Visibility::Public,
        name: "MyEnum".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![EnumVariant {
            name: "A".to_string(),
            data: Some(RustType::Named {
                name: "MissingType".to_string(),
                type_args: vec![],
            }),
            fields: vec![],
            value: None,
        }],
    }];
    generate_stub_structs(
        &mut items,
        &[],
        &[],
        &registry,
        &SyntheticTypeRegistry::new(),
    );
    let has_stub = items.iter().any(|item| {
        matches!(item, Item::Struct { name, fields, .. } if name == "MissingType" && fields.is_empty())
    });
    assert!(has_stub, "should generate stub for MissingType");
}

#[test]
fn test_undefined_refs_imported_types_excluded() {
    // use foo::Imported; struct S { f: Imported } → Imported は filtered out
    let items = vec![
        Item::Use {
            vis: Visibility::Private,
            path: "foo".to_string(),
            names: vec!["Imported".to_string()],
        },
        Item::Struct {
            vis: Visibility::Public,
            name: "S".to_string(),
            type_params: vec![],
            fields: vec![StructField {
                vis: Some(Visibility::Public),
                name: "f".to_string(),
                ty: named("Imported"),
            }],
        },
    ];
    let refs = collect_all_undefined_references(&items, &[], &[]);
    assert!(!refs.contains("Imported"));
}

#[test]
fn test_undefined_refs_type_params_excluded() {
    // struct S<T> { f: T } → T は型パラメータなので除外
    let items = vec![Item::Struct {
        vis: Visibility::Public,
        name: "S".to_string(),
        type_params: vec![TypeParam {
            name: "T".to_string(),
            constraint: None,
        }],
        fields: vec![StructField {
            vis: Some(Visibility::Public),
            name: "f".to_string(),
            ty: named("T"),
        }],
    }];
    let refs = collect_all_undefined_references(&items, &[], &[]);
    assert!(!refs.contains("T"));
}

#[test]
fn test_undefined_refs_defined_only_excluded() {
    // defined_only に Foo がある場合、items 内の Foo 参照は undefined と見なさない
    let items = vec![Item::Struct {
        vis: Visibility::Public,
        name: "Bar".to_string(),
        type_params: vec![],
        fields: vec![StructField {
            vis: Some(Visibility::Public),
            name: "f".to_string(),
            ty: named("Foo"),
        }],
    }];
    let defined_only = vec![Item::Struct {
        vis: Visibility::Public,
        name: "Foo".to_string(),
        type_params: vec![],
        fields: vec![],
    }];
    let refs = collect_all_undefined_references(&items, &[], &defined_only);
    assert!(!refs.contains("Foo"));
}

#[test]
fn test_undefined_refs_path_qualified_excluded() {
    // serde_json::Value が refs にあっても "::" を含むので除外
    let items = vec![Item::Struct {
        vis: Visibility::Public,
        name: "S".to_string(),
        type_params: vec![],
        fields: vec![StructField {
            vis: Some(Visibility::Public),
            name: "f".to_string(),
            ty: named("foo::Bar"),
        }],
    }];
    let refs = collect_all_undefined_references(&items, &[], &[]);
    assert!(!refs.contains("foo::Bar"));
}

#[test]
fn test_undefined_refs_collect_undefined_applies_external_filter() {
    // is_external のみ追加で適用される（registry に登録されている外部型のみ通す）
    let mut registry = TypeRegistry::new();
    register_external_struct(&mut registry, "External", vec![], vec![]);
    let items = vec![Item::Struct {
        vis: Visibility::Public,
        name: "S".to_string(),
        type_params: vec![],
        fields: vec![
            StructField {
                vis: Some(Visibility::Public),
                name: "a".to_string(),
                ty: named("External"),
            },
            StructField {
                vis: Some(Visibility::Public),
                name: "b".to_string(),
                ty: named("NotExternal"),
            },
        ],
    }];
    let refs = collect_undefined_type_references(&items, &[], &[], &registry);
    assert!(refs.contains("External"));
    assert!(!refs.contains("NotExternal"));
}

#[test]
fn test_undefined_refs_some_none_ok_err_not_registered_structurally() {
    // I-375: `Some` / `None` / `Ok` / `Err` are builtin `Option` / `Result`
    // variant constructors. The Transformer constructs them as
    // `CallTarget::simple(...)` with `type_ref: None`, so the reference walker
    // does not register them in the reference graph at all. The previous
    // implementation had to hard-code these names into `RUST_BUILTIN_TYPES`
    // to filter them out after the uppercase-head heuristic matched; the new
    // structural classification removes that band-aid entirely.
    //
    // This test asserts both layers of the new behavior:
    //  1. The walker does NOT put `Some` / `None` / `Ok` / `Err` into refs.
    //  2. `collect_all_undefined_references` (which runs the walker then
    //     filters) is consistent with layer 1.
    let items = vec![fn_with_body(
        "f",
        vec![
            Stmt::Expr(Expr::FnCall {
                target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Some),
                args: vec![],
            }),
            Stmt::Expr(Expr::FnCall {
                target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::None),
                args: vec![],
            }),
            Stmt::Expr(Expr::FnCall {
                target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Ok),
                args: vec![],
            }),
            Stmt::Expr(Expr::FnCall {
                target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Err),
                args: vec![],
            }),
        ],
    )];

    // Layer 1: walker sees `type_ref: None`, nothing is registered.
    let mut walker_refs = HashSet::new();
    collect_type_refs_from_item(&items[0], &mut walker_refs);
    assert!(!walker_refs.contains("Some"));
    assert!(!walker_refs.contains("None"));
    assert!(!walker_refs.contains("Ok"));
    assert!(!walker_refs.contains("Err"));

    // Layer 2: UndefinedRefScope propagates the same result.
    let refs = collect_all_undefined_references(&items, &[], &[]);
    assert!(!refs.contains("Some"));
    assert!(!refs.contains("None"));
    assert!(!refs.contains("Ok"));
    assert!(!refs.contains("Err"));
}
