use super::*;

// -- intersection type tests (type alias position) --

#[test]
fn test_convert_type_alias_intersection_two_type_lits_generates_struct() {
    let decl = parse_type_alias("type Combined = { name: string } & { age: number };");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::Struct { name, fields, .. } => {
            assert_eq!(name, "Combined");
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "name");
            assert_eq!(fields[0].ty, RustType::String);
            assert_eq!(fields[1].name, "age");
            assert_eq!(fields[1].ty, RustType::F64);
        }
        _ => panic!("expected Item::Struct, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_intersection_three_type_lits_generates_struct() {
    let decl = parse_type_alias("type C = { a: string } & { b: number } & { c: boolean };");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::Struct { name, fields, .. } => {
            assert_eq!(name, "C");
            assert_eq!(fields.len(), 3);
            assert_eq!(fields[0].name, "a");
            assert_eq!(fields[1].name, "b");
            assert_eq!(fields[2].name, "c");
        }
        _ => panic!("expected Item::Struct, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_intersection_optional_field_generates_option() {
    let decl = parse_type_alias("type C = { name: string } & { nick?: string };");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::Struct { fields, .. } => {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "name");
            assert_eq!(fields[0].ty, RustType::String);
            assert_eq!(fields[1].name, "nick");
            assert_eq!(fields[1].ty, RustType::Option(Box::new(RustType::String)));
        }
        _ => panic!("expected Item::Struct, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_intersection_duplicate_field_returns_error() {
    let decl = parse_type_alias("type C = { x: string } & { x: number };");
    let result = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    );
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("duplicate field"),
        "expected 'duplicate field' in error, got: {err_msg}"
    );
}

#[test]
fn test_convert_type_alias_intersection_type_ref_resolved_generates_merged_struct() {
    // TypeRegistry に Foo, Bar のフィールド情報がある場合、フィールド統合 struct を生成
    let mut reg = TypeRegistry::new();
    reg.register(
        "Foo".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![("a".to_string(), RustType::String).into()],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    reg.register(
        "Bar".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![("b".to_string(), RustType::F64).into()],
            std::collections::HashMap::new(),
            vec![],
        ),
    );
    let decl = parse_type_alias("type C = Foo & Bar;");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &reg,
    )
    .unwrap();
    match item {
        Item::Struct { name, fields, .. } => {
            assert_eq!(name, "C");
            assert_eq!(fields.len(), 2);
            let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
            assert!(names.contains(&"a"));
            assert!(names.contains(&"b"));
        }
        _ => panic!("expected Struct, got: {item:?}"),
    }
}

#[test]
fn test_convert_type_alias_intersection_type_ref_unresolved_generates_embedded_struct() {
    // TypeRegistry にない型参照の intersection → 型埋め込み struct
    let decl = parse_type_alias("type C = Foo & Bar;");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::Struct { name, fields, .. } => {
            assert_eq!(name, "C");
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "_0");
            assert_eq!(
                fields[0].ty,
                RustType::Named {
                    name: "Foo".to_string(),
                    type_args: vec![],
                }
            );
            assert_eq!(fields[1].name, "_1");
            assert_eq!(
                fields[1].ty,
                RustType::Named {
                    name: "Bar".to_string(),
                    type_args: vec![],
                }
            );
        }
        _ => panic!("expected Struct, got: {item:?}"),
    }
}

#[test]
fn test_convert_ts_type_intersection_annotation_returns_first_type() {
    // 型注記位置の intersection は合成 struct を生成する
    let decl = parse_interface("interface T { x: Foo & Bar; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let mut extra = SyntheticTypeRegistry::new();
    let result = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut extra,
        &TypeRegistry::new(),
    );
    assert!(
        result.is_ok(),
        "intersection in annotation should not error, got: {result:?}"
    );
    let ty = result.unwrap();
    // 合成 struct の Named が返される
    match &ty {
        RustType::Named { name, .. } => {
            assert!(
                name.starts_with("_Intersection"),
                "expected _IntersectionN, got: {name}"
            );
        }
        other => panic!("expected Named, got: {other:?}"),
    }
    // extra_items に struct が追加される
    assert_eq!(extra.all_items().len(), 1);
    match extra.all_items()[0] {
        Item::Struct { fields, .. } => {
            // Foo, Bar は TypeRegistry 未登録なので embedded フィールドになる
            assert_eq!(fields.len(), 2);
        }
        other => panic!("expected Struct, got: {other:?}"),
    }
}

// -- intersection in annotation position tests --

#[test]
fn test_convert_ts_type_intersection_type_lits_generates_merged_struct() {
    let decl = parse_interface("interface T { x: { a: string } & { b: number }; }");
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
                name.starts_with("_Intersection"),
                "expected _IntersectionN, got: {name}"
            );
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
fn test_convert_ts_type_intersection_type_ref_and_type_lit_generates_struct() {
    let decl = parse_interface("interface T { x: Foo & { c: number }; }");
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
            assert!(name.starts_with("_Intersection"));
        }
        other => panic!("expected Named, got: {other:?}"),
    }
    assert_eq!(extra.all_items().len(), 1);
    match extra.all_items()[0] {
        Item::Struct { fields, .. } => {
            // Foo は TypeRegistry 未登録なので embedded フィールド _0: Foo
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "_0");
            assert_eq!(fields[1].name, "c");
            assert_eq!(fields[1].ty, RustType::F64);
        }
        other => panic!("expected Struct, got: {other:?}"),
    }
}

#[test]
fn test_convert_ts_type_intersection_duplicate_field_returns_error() {
    let decl = parse_interface("interface T { x: { a: string } & { a: number }; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let result = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    );
    assert!(result.is_err(), "duplicate field should error");
    assert!(
        result.unwrap_err().to_string().contains("duplicate field"),
        "error should mention duplicate field"
    );
}

// -- intersection method signature impl generation (I-248) --

#[test]
fn test_intersection_with_method_generates_impl_in_synthetic() {
    let decl = parse_type_alias("type X = { a: string } & { foo(): void };");
    let mut synthetic = SyntheticTypeRegistry::new();
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut synthetic,
        &TypeRegistry::new(),
    )
    .unwrap();

    // Primary item should be a struct with the property field
    match &item {
        Item::Struct { name, fields, .. } => {
            assert_eq!(name, "X");
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].name, "a");
        }
        other => panic!("expected Item::Struct, got {other:?}"),
    }

    // Synthetic should contain an Item::Impl for X with the method
    let impl_items: Vec<_> = synthetic
        .all_items()
        .into_iter()
        .filter(|i| matches!(i, Item::Impl { .. }))
        .collect();
    assert_eq!(impl_items.len(), 1, "expected 1 impl block in synthetic");
    match impl_items[0] {
        Item::Impl {
            struct_name,
            methods,
            for_trait,
            ..
        } => {
            assert_eq!(struct_name, "X");
            assert!(for_trait.is_none());
            assert_eq!(methods.len(), 1);
            assert_eq!(methods[0].name, "foo");
            assert!(methods[0].has_self);
            assert!(methods[0].return_type.is_none()); // void → None
        }
        other => panic!("expected Item::Impl, got {other:?}"),
    }
}

#[test]
fn test_intersection_properties_only_no_impl_generated() {
    let decl = parse_type_alias("type X = { a: string } & { b: number };");
    let mut synthetic = SyntheticTypeRegistry::new();
    let _item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut synthetic,
        &TypeRegistry::new(),
    )
    .unwrap();

    // No impl blocks should be generated
    let impl_items: Vec<_> = synthetic
        .all_items()
        .into_iter()
        .filter(|i| matches!(i, Item::Impl { .. }))
        .collect();
    assert!(
        impl_items.is_empty(),
        "no impl should be generated for properties-only intersection"
    );
}

#[test]
fn test_intersection_in_annotation_with_method_generates_impl() {
    let decl =
        parse_interface("interface T { x: { a: string } & { greet(name: string): string }; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let mut synthetic = SyntheticTypeRegistry::new();
    let ty = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut synthetic,
        &TypeRegistry::new(),
    )
    .unwrap();

    // Type should be a named reference to the synthetic struct
    let struct_name = match &ty {
        RustType::Named { name, .. } => name.clone(),
        other => panic!("expected Named, got: {other:?}"),
    };

    // Synthetic should contain both a struct and an impl
    let all = synthetic.all_items();
    let structs: Vec<_> = all
        .iter()
        .filter(|i| matches!(i, Item::Struct { .. }))
        .collect();
    let impls: Vec<_> = all
        .iter()
        .filter(|i| matches!(i, Item::Impl { .. }))
        .collect();

    assert_eq!(structs.len(), 1);
    assert_eq!(impls.len(), 1);

    match impls[0] {
        Item::Impl {
            struct_name: impl_for,
            methods,
            ..
        } => {
            assert_eq!(impl_for, &struct_name);
            assert_eq!(methods.len(), 1);
            assert_eq!(methods[0].name, "greet");
            assert_eq!(methods[0].params.len(), 1);
            assert_eq!(methods[0].params[0].name, "name");
            assert_eq!(methods[0].return_type, Some(RustType::String));
        }
        other => panic!("expected Item::Impl, got {other:?}"),
    }
}
