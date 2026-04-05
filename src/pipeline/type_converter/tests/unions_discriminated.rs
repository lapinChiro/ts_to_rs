use super::*;

// -- discriminated unions --

#[test]
fn test_convert_type_alias_discriminated_union_two_variants_generates_serde_tagged_enum() {
    let decl = parse_type_alias(
        r#"type Event = { kind: "click", x: number } | { kind: "hover", y: number };"#,
    );
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        item,
        Item::Enum {
            vis: Visibility::Public,
            name: "Event".to_string(),
            type_params: vec![],
            serde_tag: Some("kind".to_string()),
            variants: vec![
                EnumVariant {
                    name: "Click".to_string(),
                    value: Some(EnumValue::Str("click".to_string())),
                    data: None,
                    fields: vec![StructField {
                        vis: Some(Visibility::Public),
                        name: "x".to_string(),
                        ty: RustType::F64,
                    }],
                },
                EnumVariant {
                    name: "Hover".to_string(),
                    value: Some(EnumValue::Str("hover".to_string())),
                    data: None,
                    fields: vec![StructField {
                        vis: Some(Visibility::Public),
                        name: "y".to_string(),
                        ty: RustType::F64,
                    }],
                },
            ],
        }
    );
}

#[test]
fn test_convert_type_alias_discriminated_union_three_variants_generates_serde_tagged_enum() {
    let decl = parse_type_alias(
        r#"type Shape = { tag: "circle", r: number } | { tag: "rect", w: number, h: number } | { tag: "line" };"#,
    );
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match &item {
        Item::Enum {
            serde_tag,
            variants,
            ..
        } => {
            assert_eq!(serde_tag, &Some("tag".to_string()));
            assert_eq!(variants.len(), 3);
            assert_eq!(variants[0].name, "Circle");
            assert_eq!(variants[0].fields.len(), 1); // r
            assert_eq!(variants[1].name, "Rect");
            assert_eq!(variants[1].fields.len(), 2); // w, h
            assert_eq!(variants[2].name, "Line");
            assert!(variants[2].fields.is_empty());
        }
        _ => panic!("expected Item::Enum, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_discriminated_union_no_extra_fields_generates_unit_variants() {
    let decl = parse_type_alias(r#"type Status = { kind: "active" } | { kind: "inactive" };"#);
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match &item {
        Item::Enum {
            serde_tag,
            variants,
            ..
        } => {
            assert_eq!(serde_tag, &Some("kind".to_string()));
            assert!(variants.iter().all(|v| v.fields.is_empty()));
        }
        _ => panic!("expected Item::Enum"),
    }
}

#[test]
fn test_convert_type_alias_discriminated_union_tag_field_type_generates_serde_tag() {
    let decl = parse_type_alias(
        r#"type Msg = { type: "text", body: string } | { type: "image", url: string };"#,
    );
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match &item {
        Item::Enum { serde_tag, .. } => {
            assert_eq!(serde_tag, &Some("type".to_string()));
        }
        _ => panic!("expected Item::Enum"),
    }
}

#[test]
fn test_convert_type_alias_union_without_common_discriminant_falls_through() {
    // No common string literal field → should fall through to existing union handling
    let decl = parse_type_alias(r#"type Mixed = { x: number } | { y: string };"#);
    let result = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    );
    // This should not produce a discriminated union — it may error or produce a different Item
    // The key assertion is that it does NOT produce an Enum with serde_tag
    if let Ok(Item::Enum { serde_tag, .. }) = result {
        assert_eq!(serde_tag, None);
    }
}

// --- find_discriminant_field (indirect via convert_type_alias) ---

#[test]
fn test_discriminated_union_duplicate_values_falls_through() {
    // Two variants with the same discriminant value → not a valid discriminated union
    let decl = parse_type_alias(
        r#"type Dup = { kind: "same", x: number } | { kind: "same", y: string };"#,
    );
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    // Should NOT produce a serde-tagged enum
    if let Item::Enum { serde_tag, .. } = &item {
        assert_eq!(
            serde_tag, &None,
            "duplicate discriminant values should not produce serde_tag"
        );
    }
}

#[test]
fn test_discriminated_union_no_common_string_literal_field() {
    // Members have no common field with string literal types
    let decl = parse_type_alias(r#"type NoDisc = { x: number } | { y: string };"#);
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    if let Item::Enum { serde_tag, .. } = &item {
        assert_eq!(
            serde_tag, &None,
            "no common string literal field should not produce serde_tag"
        );
    }
}

#[test]
fn test_discriminated_union_three_member_valid() {
    let decl = parse_type_alias(
        r#"type Tri = { t: "a", x: number } | { t: "b", y: string } | { t: "c", z: boolean };"#,
    );
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match &item {
        Item::Enum {
            serde_tag,
            variants,
            ..
        } => {
            assert_eq!(serde_tag, &Some("t".to_string()));
            assert_eq!(variants.len(), 3);
            assert_eq!(variants[0].name, "A");
            assert_eq!(variants[1].name, "B");
            assert_eq!(variants[2].name, "C");
        }
        other => panic!("expected discriminated Enum, got: {other:?}"),
    }
}
