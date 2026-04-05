use super::*;

#[test]
fn test_register_union_basic() {
    let mut reg = SyntheticTypeRegistry::new();
    let name = reg.register_union(&[RustType::String, RustType::F64]);
    assert!(!name.is_empty());
    assert!(reg.get(&name).is_some());
}

#[test]
fn test_register_union_idempotent() {
    let mut reg = SyntheticTypeRegistry::new();
    let name1 = reg.register_union(&[RustType::String, RustType::F64]);
    let name2 = reg.register_union(&[RustType::String, RustType::F64]);
    assert_eq!(name1, name2, "same union should return same name");
}

#[test]
fn test_register_union_order_independent() {
    let mut reg = SyntheticTypeRegistry::new();
    let name1 = reg.register_union(&[RustType::String, RustType::F64]);
    let name2 = reg.register_union(&[RustType::F64, RustType::String]);
    assert_eq!(
        name1, name2,
        "same members in different order should return same name"
    );
}

#[test]
fn test_register_union_different_types_get_different_names() {
    let mut reg = SyntheticTypeRegistry::new();
    let name1 = reg.register_union(&[RustType::String, RustType::F64]);
    let name2 = reg.register_union(&[RustType::String, RustType::Bool]);
    assert_ne!(name1, name2);
}

#[test]
fn test_register_union_name_format() {
    let mut reg = SyntheticTypeRegistry::new();
    let name = reg.register_union(&[RustType::String, RustType::F64]);
    // Names are sorted alphabetically: F64 comes before String
    assert_eq!(name, "F64OrString");
}

#[test]
fn test_register_inline_struct_basic() {
    let mut reg = SyntheticTypeRegistry::new();
    let name = reg.register_inline_struct(&[
        ("x".to_string(), RustType::F64),
        ("y".to_string(), RustType::String),
    ]);
    assert_eq!(name, "_TypeLit0");
    assert!(reg.get(&name).is_some());
}

#[test]
fn test_register_inline_struct_idempotent() {
    let mut reg = SyntheticTypeRegistry::new();
    let name1 = reg.register_inline_struct(&[("x".to_string(), RustType::F64)]);
    let name2 = reg.register_inline_struct(&[("x".to_string(), RustType::F64)]);
    assert_eq!(name1, name2);
}

#[test]
fn test_register_inline_struct_different_fields() {
    let mut reg = SyntheticTypeRegistry::new();
    let name1 = reg.register_inline_struct(&[("x".to_string(), RustType::F64)]);
    let name2 = reg.register_inline_struct(&[("y".to_string(), RustType::String)]);
    assert_ne!(name1, name2);
    assert_eq!(name1, "_TypeLit0");
    assert_eq!(name2, "_TypeLit1");
}

#[test]
fn test_register_any_enum() {
    let mut reg = SyntheticTypeRegistry::new();
    let name = reg.register_any_enum(
        "processData",
        "input",
        vec![EnumVariant {
            name: "String".to_string(),
            value: None,
            data: Some(RustType::String),
            fields: vec![],
        }],
    );
    assert_eq!(name, "ProcessDataInputType");
    assert!(reg.get(&name).is_some());
}

#[test]
fn test_all_items_returns_all_registered() {
    let mut reg = SyntheticTypeRegistry::new();
    reg.register_union(&[RustType::String, RustType::F64]);
    reg.register_inline_struct(&[("x".to_string(), RustType::Bool)]);
    reg.register_any_enum(
        "foo",
        "bar",
        vec![EnumVariant {
            name: "String".to_string(),
            value: None,
            data: Some(RustType::String),
            fields: vec![],
        }],
    );
    let items = reg.all_items();
    assert_eq!(items.len(), 3, "should have 3 synthetic types");
}

#[test]
fn test_get_nonexistent_returns_none() {
    let reg = SyntheticTypeRegistry::new();
    assert!(reg.get("NonExistent").is_none());
}

#[test]
fn test_to_pascal_case() {
    assert_eq!(to_pascal_case("process_data"), "ProcessData");
    assert_eq!(to_pascal_case("processData"), "ProcessData");
    assert_eq!(to_pascal_case("hono-base"), "HonoBase");
}

#[test]
fn test_union_generates_enum_item() {
    let mut reg = SyntheticTypeRegistry::new();
    let name = reg.register_union(&[RustType::String, RustType::F64]);
    let def = reg.get(&name).unwrap();
    match &def.item {
        Item::Enum { variants, .. } => {
            assert_eq!(variants.len(), 2);
        }
        _ => panic!("expected Item::Enum"),
    }
}

#[test]
fn test_inline_struct_generates_struct_item() {
    let mut reg = SyntheticTypeRegistry::new();
    let name = reg.register_inline_struct(&[
        ("x".to_string(), RustType::F64),
        ("y".to_string(), RustType::String),
    ]);
    let def = reg.get(&name).unwrap();
    match &def.item {
        Item::Struct { fields, .. } => {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "x");
            assert_eq!(fields[1].name, "y");
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_generate_name_increments() {
    let mut reg = SyntheticTypeRegistry::new();
    assert_eq!(reg.generate_name("TypeLit"), "_TypeLit0");
    assert_eq!(reg.generate_name("TypeLit"), "_TypeLit1");
    assert_eq!(reg.generate_name("Intersection"), "_Intersection2");
}

#[test]
fn test_generate_name_independent_per_instance() {
    let mut reg1 = SyntheticTypeRegistry::new();
    let mut reg2 = SyntheticTypeRegistry::new();
    assert_eq!(reg1.generate_name("TypeLit"), "_TypeLit0");
    assert_eq!(reg2.generate_name("TypeLit"), "_TypeLit0");
}

#[test]
fn test_merge_combines_types() {
    let mut reg1 = SyntheticTypeRegistry::new();
    reg1.register_union(&[RustType::String, RustType::F64]);

    let mut reg2 = SyntheticTypeRegistry::new();
    reg2.register_inline_struct(&[("x".to_string(), RustType::Bool)]);

    reg1.merge(reg2);
    assert_eq!(reg1.all_items().len(), 2);
}

#[test]
fn test_merge_preserves_dedup() {
    let mut reg1 = SyntheticTypeRegistry::new();
    let name1 = reg1.register_union(&[RustType::String, RustType::F64]);

    let mut reg2 = SyntheticTypeRegistry::new();
    let name2 = reg2.register_union(&[RustType::String, RustType::F64]);

    assert_eq!(name1, name2); // Same name independently

    reg1.merge(reg2);
    // Should still be 1 item (dedup)
    let union_count = reg1
        .all_items()
        .iter()
        .filter(|item| matches!(item, Item::Enum { .. }))
        .count();
    assert_eq!(union_count, 1);
}

#[test]
fn test_merge_updates_counters() {
    let mut reg1 = SyntheticTypeRegistry::new();
    reg1.generate_name("TypeLit"); // counter = 1

    let mut reg2 = SyntheticTypeRegistry::new();
    reg2.generate_name("TypeLit"); // counter = 1
    reg2.generate_name("TypeLit"); // counter = 2
    reg2.generate_name("TypeLit"); // counter = 3

    reg1.merge(reg2);
    // After merge, counter should be max(1, 3) = 3
    assert_eq!(reg1.generate_name("TypeLit"), "_TypeLit3");
}

#[test]
fn test_variant_name_named_with_path_uses_last_segment() {
    let ty = RustType::Named {
        name: "serde_json::Value".to_string(),
        type_args: vec![],
    };
    assert_eq!(variant_name_for_type(&ty), "Value");
}

#[test]
fn test_variant_name_named_without_path_unchanged() {
    let ty = RustType::Named {
        name: "String".to_string(),
        type_args: vec![],
    };
    assert_eq!(variant_name_for_type(&ty), "String");
}

#[test]
fn test_variant_name_dyn_trait_with_path_uses_last_segment() {
    let ty = RustType::DynTrait("std::fmt::Display".to_string());
    assert_eq!(variant_name_for_type(&ty), "Display");
}

#[test]
fn test_variant_name_dyn_trait_without_path_unchanged() {
    let ty = RustType::DynTrait("Fn".to_string());
    assert_eq!(variant_name_for_type(&ty), "Fn");
}

#[test]
fn test_struct_signature_normalizes_field_names() {
    // raw name "my-field" and sanitized name "my_field" should produce the same struct
    let mut reg = SyntheticTypeRegistry::new();
    let name1 = reg.register_inline_struct(&[("my-field".to_string(), RustType::F64)]);
    let name2 = reg.register_inline_struct(&[("my_field".to_string(), RustType::F64)]);
    assert_eq!(
        name1, name2,
        "raw and sanitized field names should dedup to same struct"
    );
}

#[test]
fn test_register_intersection_struct_basic() {
    let mut reg = SyntheticTypeRegistry::new();
    let fields = vec![
        StructField {
            vis: Some(Visibility::Public),
            name: "x".to_string(),
            ty: RustType::F64,
        },
        StructField {
            vis: Some(Visibility::Public),
            name: "y".to_string(),
            ty: RustType::String,
        },
    ];
    let (name, is_new) = reg.register_intersection_struct(&fields);
    assert!(is_new);
    assert!(reg.get(&name).is_some());
}

#[test]
fn test_register_intersection_struct_dedup() {
    let mut reg = SyntheticTypeRegistry::new();
    let fields = vec![StructField {
        vis: Some(Visibility::Public),
        name: "x".to_string(),
        ty: RustType::F64,
    }];
    let (name1, is_new1) = reg.register_intersection_struct(&fields);
    let (name2, is_new2) = reg.register_intersection_struct(&fields);
    assert!(is_new1);
    assert!(!is_new2, "second registration should be a dedup hit");
    assert_eq!(name1, name2);
}

#[test]
fn test_intersection_struct_dedup_with_type_lit() {
    // Intersection struct and TypeLit with same fields should dedup
    let mut reg = SyntheticTypeRegistry::new();
    let type_lit_name = reg.register_inline_struct(&[("x".to_string(), RustType::F64)]);
    let (intersection_name, is_new) = reg.register_intersection_struct(&[StructField {
        vis: Some(Visibility::Public),
        name: "x".to_string(),
        ty: RustType::F64,
    }]);
    assert!(!is_new, "should dedup with existing TypeLit");
    assert_eq!(type_lit_name, intersection_name);
}

#[test]
fn test_register_intersection_enum_basic() {
    let mut reg = SyntheticTypeRegistry::new();
    let variants = vec![
        EnumVariant {
            name: "Variant0".to_string(),
            value: None,
            data: None,
            fields: vec![StructField {
                vis: Some(Visibility::Public),
                name: "x".to_string(),
                ty: RustType::F64,
            }],
        },
        EnumVariant {
            name: "Variant1".to_string(),
            value: None,
            data: None,
            fields: vec![StructField {
                vis: Some(Visibility::Public),
                name: "y".to_string(),
                ty: RustType::String,
            }],
        },
    ];
    let (name, is_new) = reg.register_intersection_enum(None, variants);
    assert!(is_new);
    assert!(reg.get(&name).is_some());
}

#[test]
fn test_register_intersection_enum_dedup() {
    let mut reg = SyntheticTypeRegistry::new();
    let make_variants = || {
        vec![
            EnumVariant {
                name: "A".to_string(),
                value: Some(crate::ir::EnumValue::Str("a".to_string())),
                data: None,
                fields: vec![StructField {
                    vis: Some(Visibility::Public),
                    name: "x".to_string(),
                    ty: RustType::F64,
                }],
            },
            EnumVariant {
                name: "B".to_string(),
                value: Some(crate::ir::EnumValue::Str("b".to_string())),
                data: None,
                fields: vec![StructField {
                    vis: Some(Visibility::Public),
                    name: "y".to_string(),
                    ty: RustType::String,
                }],
            },
        ]
    };
    let (name1, is_new1) = reg.register_intersection_enum(Some("type"), make_variants());
    let (name2, is_new2) = reg.register_intersection_enum(Some("type"), make_variants());
    assert!(is_new1);
    assert!(!is_new2, "second registration should be a dedup hit");
    assert_eq!(name1, name2);
}

#[test]
fn test_register_intersection_enum_different_tag() {
    let mut reg = SyntheticTypeRegistry::new();
    let variants = vec![EnumVariant {
        name: "A".to_string(),
        value: None,
        data: None,
        fields: vec![],
    }];
    let (name1, _) = reg.register_intersection_enum(Some("type"), variants.clone());
    let (name2, _) = reg.register_intersection_enum(Some("kind"), variants);
    assert_ne!(
        name1, name2,
        "different tags should produce different enums"
    );
}

#[test]
fn test_union_name_with_path_type_produces_valid_identifier() {
    let mut reg = SyntheticTypeRegistry::new();
    let name = reg.register_union(&[
        RustType::String,
        RustType::Named {
            name: "serde_json::Value".to_string(),
            type_args: vec![],
        },
    ]);
    assert_eq!(name, "StringOrValue");
}

#[test]
fn test_cross_origin_dedup_reverse_order() {
    // Intersection first, then TypeLit — should still dedup
    let mut reg = SyntheticTypeRegistry::new();
    let (intersection_name, is_new) = reg.register_intersection_struct(&[StructField {
        vis: Some(Visibility::Public),
        name: "x".to_string(),
        ty: RustType::F64,
    }]);
    assert!(is_new);
    let type_lit_name = reg.register_inline_struct(&[("x".to_string(), RustType::F64)]);
    assert_eq!(
        intersection_name, type_lit_name,
        "reverse order cross-origin dedup should work"
    );
}

#[test]
fn test_intersection_struct_field_order_independent() {
    let mut reg = SyntheticTypeRegistry::new();
    let (name1, _) = reg.register_intersection_struct(&[
        StructField {
            vis: Some(Visibility::Public),
            name: "a".to_string(),
            ty: RustType::F64,
        },
        StructField {
            vis: Some(Visibility::Public),
            name: "b".to_string(),
            ty: RustType::String,
        },
    ]);
    let (name2, is_new) = reg.register_intersection_struct(&[
        StructField {
            vis: Some(Visibility::Public),
            name: "b".to_string(),
            ty: RustType::String,
        },
        StructField {
            vis: Some(Visibility::Public),
            name: "a".to_string(),
            ty: RustType::F64,
        },
    ]);
    assert!(!is_new, "same fields in different order should dedup");
    assert_eq!(name1, name2);
}

#[test]
fn test_intersection_enum_different_variants() {
    let mut reg = SyntheticTypeRegistry::new();
    let (name1, _) = reg.register_intersection_enum(
        None,
        vec![EnumVariant {
            name: "A".to_string(),
            value: None,
            data: None,
            fields: vec![StructField {
                vis: Some(Visibility::Public),
                name: "x".to_string(),
                ty: RustType::F64,
            }],
        }],
    );
    let (name2, _) = reg.register_intersection_enum(
        None,
        vec![EnumVariant {
            name: "B".to_string(),
            value: None,
            data: None,
            fields: vec![StructField {
                vis: Some(Visibility::Public),
                name: "y".to_string(),
                ty: RustType::String,
            }],
        }],
    );
    assert_ne!(
        name1, name2,
        "different variants should produce different enums"
    );
}

#[test]
fn test_fork_dedup_state_includes_intersection_enum() {
    let mut reg = SyntheticTypeRegistry::new();
    let (name1, _) = reg.register_intersection_enum(
        Some("type"),
        vec![EnumVariant {
            name: "A".to_string(),
            value: None,
            data: None,
            fields: vec![],
        }],
    );
    let forked = reg.fork_dedup_state();
    // Fork should inherit the dedup state — same enum should resolve to same name
    // We can't call register_intersection_enum on the fork and check directly,
    // but we can verify the counter is inherited
    assert_eq!(
        forked.types.len(),
        0,
        "forked registry should have no types"
    );
    // Register same enum in fork — should reuse name from dedup state
    let mut forked = reg.fork_dedup_state();
    let (name2, is_new) = forked.register_intersection_enum(
        Some("type"),
        vec![EnumVariant {
            name: "A".to_string(),
            value: None,
            data: None,
            fields: vec![],
        }],
    );
    assert!(!is_new, "fork should inherit dedup state");
    assert_eq!(name1, name2);
}

#[test]
fn test_merge_includes_intersection_enum_dedup() {
    let mut reg1 = SyntheticTypeRegistry::new();
    let (name1, _) = reg1.register_intersection_enum(
        None,
        vec![EnumVariant {
            name: "X".to_string(),
            value: None,
            data: None,
            fields: vec![],
        }],
    );

    let mut reg2 = SyntheticTypeRegistry::new();
    let (name2, _) = reg2.register_intersection_enum(
        None,
        vec![EnumVariant {
            name: "X".to_string(),
            value: None,
            data: None,
            fields: vec![],
        }],
    );

    assert_eq!(name1, name2, "same enum independently should get same name");

    reg1.merge(reg2);
    // After merge, dedup should prevent duplicate
    let enum_count = reg1
        .all_items()
        .iter()
        .filter(|item| matches!(item, Item::Enum { .. }))
        .count();
    assert_eq!(
        enum_count, 1,
        "merged registry should have 1 enum (deduped)"
    );
}

// --- Phase 1: variant_name_for_type type_args tests ---

#[test]
fn test_variant_name_named_with_type_args() {
    let ty = RustType::Named {
        name: "Foo".to_string(),
        type_args: vec![RustType::String],
    };
    assert_eq!(variant_name_for_type(&ty), "FooString");
}

#[test]
fn test_variant_name_named_with_multiple_type_args() {
    let ty = RustType::Named {
        name: "Map".to_string(),
        type_args: vec![RustType::String, RustType::F64],
    };
    assert_eq!(variant_name_for_type(&ty), "MapStringF64");
}

#[test]
fn test_variant_name_named_different_type_args_differ() {
    let ty1 = RustType::Named {
        name: "Foo".to_string(),
        type_args: vec![RustType::String],
    };
    let ty2 = RustType::Named {
        name: "Foo".to_string(),
        type_args: vec![RustType::F64],
    };
    assert_ne!(
        variant_name_for_type(&ty1),
        variant_name_for_type(&ty2),
        "different type_args should produce different variant names"
    );
}

#[test]
fn test_variant_name_tuple_with_elements() {
    let ty = RustType::Tuple(vec![RustType::String, RustType::F64]);
    assert_eq!(variant_name_for_type(&ty), "TupleStringF64");
}

#[test]
fn test_variant_name_tuple_empty() {
    let ty = RustType::Tuple(vec![]);
    assert_eq!(variant_name_for_type(&ty), "Tuple");
}

#[test]
fn test_variant_name_tuple_different_elements_differ() {
    let ty1 = RustType::Tuple(vec![RustType::String, RustType::F64]);
    let ty2 = RustType::Tuple(vec![RustType::Bool]);
    assert_ne!(variant_name_for_type(&ty1), variant_name_for_type(&ty2));
}

#[test]
fn test_variant_name_result() {
    let ty = RustType::Result {
        ok: Box::new(RustType::String),
        err: Box::new(RustType::Any),
    };
    assert_eq!(variant_name_for_type(&ty), "ResultStringAny");
}

#[test]
fn test_variant_name_result_different_types_differ() {
    let ty1 = RustType::Result {
        ok: Box::new(RustType::String),
        err: Box::new(RustType::Any),
    };
    let ty2 = RustType::Result {
        ok: Box::new(RustType::F64),
        err: Box::new(RustType::Any),
    };
    assert_ne!(variant_name_for_type(&ty1), variant_name_for_type(&ty2));
}

#[test]
fn test_variant_name_fn_includes_return_type() {
    let ty = RustType::Fn {
        params: vec![RustType::String],
        return_type: Box::new(RustType::Bool),
    };
    assert_eq!(variant_name_for_type(&ty), "FnBool");
}

#[test]
fn test_variant_name_fn_different_return_types_differ() {
    let ty1 = RustType::Fn {
        params: vec![],
        return_type: Box::new(RustType::Bool),
    };
    let ty2 = RustType::Fn {
        params: vec![],
        return_type: Box::new(RustType::String),
    };
    assert_ne!(variant_name_for_type(&ty1), variant_name_for_type(&ty2));
}

#[test]
fn test_union_name_no_collision_with_type_args() {
    let mut reg = SyntheticTypeRegistry::new();
    let name1 = reg.register_union(&[
        RustType::Named {
            name: "Foo".to_string(),
            type_args: vec![RustType::String],
        },
        RustType::F64,
    ]);
    let name2 = reg.register_union(&[
        RustType::Named {
            name: "Foo".to_string(),
            type_args: vec![RustType::F64],
        },
        RustType::F64,
    ]);
    assert_ne!(
        name1, name2,
        "unions with different type_args should have different names"
    );
}

#[test]
fn test_register_union_deduplicates_variant_names() {
    // Two different types that produce the same variant name after path extraction:
    // "ns::Foo" and "Foo" both produce "Foo" via rsplit_once("::")
    let mut reg = SyntheticTypeRegistry::new();
    let name = reg.register_union(&[
        RustType::Named {
            name: "ns::Foo".to_string(),
            type_args: vec![],
        },
        RustType::Named {
            name: "Foo".to_string(),
            type_args: vec![],
        },
        RustType::F64,
    ]);
    let def = reg.get(&name).unwrap();
    match &def.item {
        Item::Enum { variants, .. } => {
            let names: Vec<&str> = variants.iter().map(|v| v.name.as_str()).collect();
            // Should not have duplicate "Foo" variants
            let unique_count = {
                let mut seen = vec![];
                for n in &names {
                    if !seen.contains(n) {
                        seen.push(n);
                    }
                }
                seen.len()
            };
            assert_eq!(
                names.len(),
                unique_count,
                "register_union should deduplicate variant names, got: {names:?}"
            );
        }
        _ => panic!("expected Item::Enum"),
    }
}

#[test]
fn test_register_union_fn_same_return_type_deduped() {
    // Two Fn types with same return type produce same variant name "FnBool"
    let mut reg = SyntheticTypeRegistry::new();
    let name = reg.register_union(&[
        RustType::Fn {
            params: vec![RustType::String],
            return_type: Box::new(RustType::Bool),
        },
        RustType::Fn {
            params: vec![RustType::F64],
            return_type: Box::new(RustType::Bool),
        },
    ]);
    let def = reg.get(&name).unwrap();
    match &def.item {
        Item::Enum { variants, .. } => {
            assert_eq!(
                variants.len(),
                1,
                "Fn types with same return type should be deduped to single variant"
            );
        }
        _ => panic!("expected Item::Enum"),
    }
}

// ── register_union: type param scope detection ──

#[test]
fn test_register_union_detects_type_params_from_scope() {
    let mut reg = SyntheticTypeRegistry::new();
    reg.push_type_param_scope(vec!["T".to_string()]);
    let name = reg.register_union(&[
        RustType::Named {
            name: "T".to_string(),
            type_args: vec![],
        },
        RustType::Vec(Box::new(RustType::Named {
            name: "T".to_string(),
            type_args: vec![],
        })),
    ]);
    let def = reg.get(&name).unwrap();
    match &def.item {
        Item::Enum { type_params, .. } => {
            assert_eq!(type_params.len(), 1, "should detect T from scope");
            assert_eq!(type_params[0].name, "T");
        }
        _ => panic!("expected Item::Enum for type param scope test"),
    }
}

#[test]
fn test_register_union_no_type_params_when_scope_empty() {
    let mut reg = SyntheticTypeRegistry::new();
    // No type param scope set
    let name = reg.register_union(&[
        RustType::Named {
            name: "T".to_string(),
            type_args: vec![],
        },
        RustType::F64,
    ]);
    let def = reg.get(&name).unwrap();
    match &def.item {
        Item::Enum { type_params, .. } => {
            assert!(
                type_params.is_empty(),
                "without scope, no type params should be detected"
            );
        }
        _ => panic!("expected Item::Enum for empty scope test"),
    }
}
