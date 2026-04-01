use super::*;

// -- TsTypeLit in annotation position tests --

#[test]
fn test_convert_ts_type_type_lit_single_field_generates_struct() {
    let decl = parse_interface("interface T { x: { a: string }; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let mut extra = SyntheticTypeRegistry::new();
    let ty = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut extra,
        &TypeRegistry::new(),
    )
    .unwrap();

    match &ty {
        RustType::Named { name, .. } => {
            assert!(
                name.starts_with("_TypeLit"),
                "expected _TypeLitN, got: {name}"
            );
        }
        other => panic!("expected Named, got: {other:?}"),
    }
    assert_eq!(extra.all_items().len(), 1);
    match extra.all_items()[0] {
        Item::Struct { name, fields, .. } => {
            assert!(name.starts_with("_TypeLit"));
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].name, "a");
            assert_eq!(fields[0].ty, RustType::String);
        }
        other => panic!("expected Struct, got: {other:?}"),
    }
}

#[test]
fn test_convert_ts_type_type_lit_multiple_fields_generates_struct() {
    let decl = parse_interface("interface T { x: { a: string, b: number }; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let mut extra = SyntheticTypeRegistry::new();
    let ty = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut extra,
        &TypeRegistry::new(),
    )
    .unwrap();

    match &ty {
        RustType::Named { name, .. } => {
            assert!(name.starts_with("_TypeLit"));
        }
        other => panic!("expected Named, got: {other:?}"),
    }
    assert_eq!(extra.all_items().len(), 1);
    match extra.all_items()[0] {
        Item::Struct { fields, .. } => {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "a");
            assert_eq!(fields[0].ty, RustType::String);
            assert_eq!(fields[1].name, "b");
            assert_eq!(fields[1].ty, RustType::F64);
        }
        other => panic!("expected Struct, got: {other:?}"),
    }
}

#[test]
fn test_convert_ts_type_type_lit_optional_field_generates_option() {
    let decl = parse_interface("interface T { x: { a: string, b?: number }; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let mut extra = SyntheticTypeRegistry::new();
    let ty = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut extra,
        &TypeRegistry::new(),
    )
    .unwrap();

    assert!(matches!(ty, RustType::Named { .. }));
    assert_eq!(extra.all_items().len(), 1);
    match extra.all_items()[0] {
        Item::Struct { fields, .. } => {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "a");
            assert_eq!(fields[0].ty, RustType::String);
            assert_eq!(fields[1].name, "b");
            assert_eq!(fields[1].ty, RustType::Option(Box::new(RustType::F64)));
        }
        other => panic!("expected Struct, got: {other:?}"),
    }
}

// -- Utility type tests --

#[test]
fn test_utility_partial_registered_type_generates_all_option_fields() {
    let ts_type = parse_type_ann("Partial<Point>");
    let reg = reg_with_point();
    let mut extra_items = SyntheticTypeRegistry::new();
    let ty = convert_ts_type(&ts_type, &mut extra_items, &reg).unwrap();

    // Should return a Named type pointing to the synthesized struct
    assert_eq!(
        ty,
        RustType::Named {
            name: "PartialPoint".to_string(),
            type_args: vec![],
        }
    );

    // extra_items should contain the synthesized struct with all fields wrapped in Option
    assert!(
        extra_items.all_items().iter().any(|item| matches!(
            item,
            Item::Struct { name, fields, .. }
            if name == "PartialPoint"
                && fields.len() == 3
                && fields.iter().all(|f| matches!(&f.ty, RustType::Option(_)))
        )),
        "expected PartialPoint struct with all Option fields"
    );
}

#[test]
fn test_utility_partial_unregistered_type_falls_back() {
    let ts_type = parse_type_ann("Partial<Unknown>");
    let reg = TypeRegistry::new();
    let mut extra_items = SyntheticTypeRegistry::new();
    let ty = convert_ts_type(&ts_type, &mut extra_items, &reg).unwrap();

    // Should fall back to the inner type
    assert_eq!(
        ty,
        RustType::Named {
            name: "Unknown".to_string(),
            type_args: vec![],
        }
    );
    assert!(extra_items.all_items().is_empty());
}

#[test]
fn test_utility_required_strips_option_from_all_fields() {
    let ts_type = parse_type_ann("Required<OptPoint>");
    let mut reg = TypeRegistry::new();
    reg.register(
        "OptPoint".to_string(),
        TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::Option(Box::new(RustType::F64))).into(),
                ("y".to_string(), RustType::Option(Box::new(RustType::F64))).into(),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let mut extra_items = SyntheticTypeRegistry::new();
    let ty = convert_ts_type(&ts_type, &mut extra_items, &reg).unwrap();

    assert_eq!(
        ty,
        RustType::Named {
            name: "RequiredOptPoint".to_string(),
            type_args: vec![],
        }
    );

    assert!(
        extra_items.all_items().iter().any(|item| matches!(
            item,
            Item::Struct { name, fields, .. }
            if name == "RequiredOptPoint"
                && fields.len() == 2
                && fields.iter().all(|f| matches!(&f.ty, RustType::F64))
        )),
        "expected RequiredOptPoint struct with non-Option fields"
    );
}

#[test]
fn test_utility_pick_single_key_filters_fields() {
    let ts_type = parse_type_ann("Pick<Point, \"x\">");
    let reg = reg_with_point();
    let mut extra_items = SyntheticTypeRegistry::new();
    let ty = convert_ts_type(&ts_type, &mut extra_items, &reg).unwrap();

    assert_eq!(
        ty,
        RustType::Named {
            name: "PickPointX".to_string(),
            type_args: vec![],
        }
    );

    assert!(
        extra_items.all_items().iter().any(|item| matches!(
            item,
            Item::Struct { name, fields, .. }
            if name == "PickPointX"
                && fields.len() == 1
                && fields[0].name == "x"
        )),
        "expected PickPointX with field x"
    );
}

#[test]
fn test_utility_pick_union_keys_filters_fields() {
    let ts_type = parse_type_ann("Pick<Point, \"x\" | \"y\">");
    let reg = reg_with_point();
    let mut extra_items = SyntheticTypeRegistry::new();
    let ty = convert_ts_type(&ts_type, &mut extra_items, &reg).unwrap();

    assert_eq!(
        ty,
        RustType::Named {
            name: "PickPointXY".to_string(),
            type_args: vec![],
        }
    );

    assert!(
        extra_items.all_items().iter().any(|item| matches!(
            item,
            Item::Struct { name, fields, .. }
            if name == "PickPointXY" && fields.len() == 2
        )),
        "expected PickPointXY with 2 fields"
    );
}

#[test]
fn test_utility_omit_single_key_excludes_field() {
    let ts_type = parse_type_ann("Omit<Point, \"x\">");
    let reg = reg_with_point();
    let mut extra_items = SyntheticTypeRegistry::new();
    let ty = convert_ts_type(&ts_type, &mut extra_items, &reg).unwrap();

    assert_eq!(
        ty,
        RustType::Named {
            name: "OmitPointX".to_string(),
            type_args: vec![],
        }
    );

    assert!(
        extra_items.all_items().iter().any(|item| matches!(
            item,
            Item::Struct { name, fields, .. }
            if name == "OmitPointX"
                && fields.len() == 2
                && fields.iter().all(|f| f.name != "x")
        )),
        "expected OmitPointX without x"
    );
}

#[test]
fn test_utility_non_nullable_strips_option() {
    // NonNullable<string | null> → in our system, string | null becomes Option<String>
    // NonNullable should unwrap it to String
    let ts_type = parse_type_ann("NonNullable<string | null>");
    let mut extra_items = SyntheticTypeRegistry::new();
    let ty = convert_ts_type(&ts_type, &mut extra_items, &TypeRegistry::new()).unwrap();
    assert_eq!(ty, RustType::String);
}

#[test]
fn test_utility_partial_pick_nested() {
    // Partial<Pick<Point, "x" | "y">> should produce PartialPickPointXY
    let ts_type = parse_type_ann("Partial<Pick<Point, \"x\" | \"y\">>");
    let reg = reg_with_point();
    let mut extra_items = SyntheticTypeRegistry::new();
    let ty = convert_ts_type(&ts_type, &mut extra_items, &reg).unwrap();

    assert_eq!(
        ty,
        RustType::Named {
            name: "PartialPickPointXY".to_string(),
            type_args: vec![],
        }
    );

    // Should have both PickPointXY and PartialPickPointXY in extra_items
    assert!(
        extra_items
            .all_items()
            .iter()
            .any(|item| matches!(item, Item::Struct { name, .. } if name == "PickPointXY")),
        "expected PickPointXY in extra_items"
    );
    assert!(
        extra_items.all_items().iter().any(|item| matches!(
            item,
            Item::Struct { name, fields, .. }
            if name == "PartialPickPointXY"
                && fields.len() == 2
                && fields.iter().all(|f| matches!(&f.ty, RustType::Option(_)))
        )),
        "expected PartialPickPointXY with Option fields"
    );
}

// -- Index signature in annotation → HashMap --

#[test]
fn test_convert_ts_type_index_signature_in_annotation_to_hashmap() {
    let decl = parse_interface("interface T { x: { [key: string]: string }; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        ty,
        RustType::Named {
            name: "HashMap".to_string(),
            type_args: vec![RustType::String, RustType::String],
        }
    );
}
