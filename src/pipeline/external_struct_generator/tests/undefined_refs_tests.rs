use super::*;
use std::collections::HashSet;

// I-383 T5: Item::Enum の type_params が type_param 除外フィルタに含まれること
#[test]
fn test_collect_all_undefined_refs_excludes_enum_type_params() {
    use crate::ir::TypeParam;
    let items = [Item::Enum {
        vis: Visibility::Public,
        name: "MOrVecM".to_string(),
        type_params: vec![TypeParam {
            name: "M".to_string(),
            constraint: None,
        }],
        serde_tag: None,
        variants: vec![EnumVariant {
            name: "M".to_string(),
            data: Some(RustType::Named {
                name: "M".to_string(),
                type_args: vec![],
            }),
            fields: vec![],
            value: None,
        }],
    }];
    let refs = collect_all_undefined_references(&items.iter().collect::<Vec<_>>());
    assert!(
        !refs.contains("M"),
        "type parameter `M` of Item::Enum should be excluded from undefined refs, got {refs:?}"
    );
}

#[test]
fn test_collect_all_undefined_refs_includes_non_external() {
    // registry に登録されていない型も検出する（is_external フィルタなし）
    let items = [Item::Enum {
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
    let refs = collect_all_undefined_references(&items.iter().collect::<Vec<_>>());
    assert!(refs.contains("UnknownType"));
}

#[test]
fn test_collect_all_undefined_refs_excludes_defined() {
    let items = [
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
    let refs = collect_all_undefined_references(&items.iter().collect::<Vec<_>>());
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
        &HashSet::new(),
        &registry,
        &SyntheticTypeRegistry::new(),
    );
    let has_stub = items.iter().any(|item| {
        matches!(item, Item::Struct { name, fields, .. } if name == "MissingType" && fields.is_empty())
    });
    assert!(has_stub, "should generate stub for MissingType");
}

#[test]
fn test_generate_stub_structs_excludes_defined_elsewhere_names() {
    // I-376 C3: Phase 5c stub pass が user 定義型 (別モジュールで定義済み) を stub 化
    // しないことを検証。`defined_elsewhere_names` に含まれる型名は `items` 内で未定義
    // でも stub struct を生成してはならない。
    let registry = TypeRegistry::new();
    let mut items = vec![Item::Enum {
        vis: Visibility::Public,
        name: "SyntheticEnum".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![
            EnumVariant {
                name: "User".to_string(),
                data: Some(RustType::Named {
                    name: "UserType".to_string(),
                    type_args: vec![],
                }),
                fields: vec![],
                value: None,
            },
            EnumVariant {
                name: "Missing".to_string(),
                data: Some(RustType::Named {
                    name: "GenuinelyMissing".to_string(),
                    type_args: vec![],
                }),
                fields: vec![],
                value: None,
            },
        ],
    }];
    let defined_elsewhere: HashSet<String> = std::iter::once("UserType".to_string()).collect();
    generate_stub_structs(
        &mut items,
        &defined_elsewhere,
        &registry,
        &SyntheticTypeRegistry::new(),
    );

    // GenuinelyMissing は未定義 → stub 生成される
    assert!(
        items
            .iter()
            .any(|item| matches!(item, Item::Struct { name, .. } if name == "GenuinelyMissing")),
        "GenuinelyMissing should be stubbed",
    );
    // UserType は defined_elsewhere に含まれる → stub 生成禁止
    assert!(
        !items
            .iter()
            .any(|item| matches!(item, Item::Struct { name, .. } if name == "UserType")),
        "UserType must not be stubbed because it is defined_elsewhere",
    );
}

#[test]
fn test_generate_stub_structs_fixpoint_resolves_transitive_refs() {
    // Stub 生成の fixpoint 動作検証: `generate_external_struct` でフル生成される型の
    // フィールドが新たな未定義型を参照する場合、次の iteration で stub 化される。
    let mut registry = TypeRegistry::new();
    register_external_struct(
        &mut registry,
        "Root",
        vec![("child", named("Child"))],
        vec![],
    );
    let mut items = vec![Item::Struct {
        vis: Visibility::Public,
        name: "Holder".to_string(),
        type_params: vec![],
        fields: vec![StructField {
            vis: Some(Visibility::Public),
            name: "r".to_string(),
            ty: named("Root"),
        }],
    }];
    generate_stub_structs(
        &mut items,
        &HashSet::new(),
        &registry,
        &SyntheticTypeRegistry::new(),
    );
    // Iteration 1: Root は registry 経由でフル生成 → child: Child フィールド追加
    // Iteration 2: Child は未定義 → 空 stub 生成
    assert!(
        items
            .iter()
            .any(|i| matches!(i, Item::Struct { name, .. } if name == "Root")),
        "Root should be generated via registry"
    );
    assert!(
        items
            .iter()
            .any(|i| matches!(i, Item::Struct { name, .. } if name == "Child")),
        "transitive Child stub should be generated in a subsequent iteration"
    );
}

#[test]
fn test_undefined_refs_imported_types_excluded() {
    // use foo::Imported; struct S { f: Imported } → Imported は filtered out
    let items = [
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
    let refs = collect_all_undefined_references(&items.iter().collect::<Vec<_>>());
    assert!(!refs.contains("Imported"));
}

#[test]
fn test_undefined_refs_type_params_excluded() {
    // struct S<T> { f: T } → T は型パラメータなので除外
    let items = [Item::Struct {
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
    let refs = collect_all_undefined_references(&items.iter().collect::<Vec<_>>());
    assert!(!refs.contains("T"));
}

#[test]
fn test_undefined_refs_other_defined_item_excluded() {
    // pool 内に Foo の定義が存在する場合、Bar 内の Foo 参照は undefined と見なさない。
    // I-376: 従来の `defined_only` 非対称引数を撤廃したため、定義と参照を同じ `items`
    // プールに混在させても正しく機能することを確認する。
    let foo = Item::Struct {
        vis: Visibility::Public,
        name: "Foo".to_string(),
        type_params: vec![],
        fields: vec![],
    };
    let bar = Item::Struct {
        vis: Visibility::Public,
        name: "Bar".to_string(),
        type_params: vec![],
        fields: vec![StructField {
            vis: Some(Visibility::Public),
            name: "f".to_string(),
            ty: named("Foo"),
        }],
    };
    let pool: Vec<&Item> = vec![&bar, &foo];
    let refs = collect_all_undefined_references(&pool);
    assert!(!refs.contains("Foo"));
}

#[test]
fn test_undefined_refs_path_qualified_excluded() {
    // serde_json::Value が refs にあっても "::" を含むので除外
    let items = [Item::Struct {
        vis: Visibility::Public,
        name: "S".to_string(),
        type_params: vec![],
        fields: vec![StructField {
            vis: Some(Visibility::Public),
            name: "f".to_string(),
            ty: named("foo::Bar"),
        }],
    }];
    let refs = collect_all_undefined_references(&items.iter().collect::<Vec<_>>());
    assert!(!refs.contains("foo::Bar"));
}

#[test]
fn test_undefined_refs_collect_undefined_applies_external_filter() {
    // is_external のみ追加で適用される（registry に登録されている外部型のみ通す）
    let mut registry = TypeRegistry::new();
    register_external_struct(&mut registry, "External", vec![], vec![]);
    let items = [Item::Struct {
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
    let refs = collect_undefined_type_references(&items.iter().collect::<Vec<_>>(), &registry);
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
    let items = [fn_with_body(
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
    let refs = collect_all_undefined_references(&items.iter().collect::<Vec<_>>());
    assert!(!refs.contains("Some"));
    assert!(!refs.contains("None"));
    assert!(!refs.contains("Ok"));
    assert!(!refs.contains("Err"));
}
