use super::super::*;

#[test]
fn test_item_struct() {
    let item = Item::Struct {
        vis: Visibility::Public,
        name: "Point".to_string(),
        type_params: vec![],
        fields: vec![
            StructField {
                vis: None,
                name: "x".to_string(),
                ty: RustType::F64,
            },
            StructField {
                vis: None,
                name: "y".to_string(),
                ty: RustType::Option(Box::new(RustType::F64)),
            },
        ],
    };
    match item {
        Item::Struct { name, fields, .. } => {
            assert_eq!(name, "Point");
            assert_eq!(fields.len(), 2);
        }
        _ => panic!("expected Struct"),
    }
}

#[test]
fn test_item_enum_no_values() {
    let item = Item::Enum {
        vis: Visibility::Public,
        name: "Color".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![
            EnumVariant {
                name: "Red".to_string(),
                value: None,
                data: None,
                fields: vec![],
            },
            EnumVariant {
                name: "Green".to_string(),
                value: None,
                data: None,
                fields: vec![],
            },
        ],
    };
    match item {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "Color");
            assert_eq!(variants.len(), 2);
            assert!(variants[0].value.is_none());
        }
        _ => panic!("expected Enum"),
    }
}

#[test]
fn test_item_enum_numeric_values() {
    let item = Item::Enum {
        vis: Visibility::Public,
        name: "Status".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![
            EnumVariant {
                name: "Active".to_string(),
                value: Some(EnumValue::Number(1)),
                data: None,
                fields: vec![],
            },
            EnumVariant {
                name: "Inactive".to_string(),
                value: Some(EnumValue::Number(0)),
                data: None,
                fields: vec![],
            },
        ],
    };
    match &item {
        Item::Enum { variants, .. } => {
            assert_eq!(variants[0].value, Some(EnumValue::Number(1)));
            assert_eq!(variants[1].value, Some(EnumValue::Number(0)));
        }
        _ => panic!("expected Enum"),
    }
}

#[test]
fn test_item_enum_string_values() {
    let item = Item::Enum {
        vis: Visibility::Public,
        name: "Direction".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![
            EnumVariant {
                name: "Up".to_string(),
                value: Some(EnumValue::Str("UP".to_string())),
                data: None,
                fields: vec![],
            },
            EnumVariant {
                name: "Down".to_string(),
                value: Some(EnumValue::Str("DOWN".to_string())),
                data: None,
                fields: vec![],
            },
        ],
    };
    match &item {
        Item::Enum { variants, .. } => {
            assert_eq!(variants[0].value, Some(EnumValue::Str("UP".to_string())));
        }
        _ => panic!("expected Enum"),
    }
}

#[test]
fn test_item_fn() {
    let item = Item::Fn {
        vis: Visibility::Public,
        attributes: vec![],
        is_async: false,
        name: "add".to_string(),
        type_params: vec![],
        params: vec![
            Param {
                name: "a".to_string(),
                ty: Some(RustType::F64),
            },
            Param {
                name: "b".to_string(),
                ty: Some(RustType::F64),
            },
        ],
        return_type: Some(RustType::F64),
        body: vec![],
    };
    match item {
        Item::Fn { name, params, .. } => {
            assert_eq!(name, "add");
            assert_eq!(params.len(), 2);
        }
        _ => panic!("expected Fn"),
    }
}

// =========================================================================
// Item::canonical_name() — A-1-1 で追加
// =========================================================================

fn empty_struct(name: &str) -> Item {
    Item::Struct {
        vis: Visibility::Public,
        name: name.to_string(),
        type_params: vec![],
        fields: vec![],
    }
}

#[test]
fn test_canonical_name_struct() {
    assert_eq!(empty_struct("Foo").canonical_name(), Some("Foo"));
}

#[test]
fn test_canonical_name_enum() {
    let item = Item::Enum {
        vis: Visibility::Public,
        name: "MyEnum".to_string(),
        type_params: vec![],
        serde_tag: None,
        variants: vec![],
    };
    assert_eq!(item.canonical_name(), Some("MyEnum"));
}

#[test]
fn test_canonical_name_trait() {
    let item = Item::Trait {
        vis: Visibility::Public,
        name: "MyTrait".to_string(),
        type_params: vec![],
        supertraits: vec![],
        methods: vec![],
        associated_types: vec![],
    };
    assert_eq!(item.canonical_name(), Some("MyTrait"));
}

#[test]
fn test_canonical_name_type_alias() {
    let item = Item::TypeAlias {
        vis: Visibility::Public,
        name: "MyAlias".to_string(),
        type_params: vec![],
        ty: RustType::String,
    };
    assert_eq!(item.canonical_name(), Some("MyAlias"));
}

#[test]
fn test_canonical_name_fn() {
    let item = Item::Fn {
        vis: Visibility::Public,
        attributes: vec![],
        is_async: false,
        name: "my_fn".to_string(),
        type_params: vec![],
        params: vec![],
        return_type: None,
        body: vec![],
    };
    assert_eq!(item.canonical_name(), Some("my_fn"));
}

#[test]
fn test_canonical_name_impl_returns_struct_name() {
    // impl は struct_name を識別名とする（trait impl の場合も同じ struct_name）
    let item = Item::Impl {
        struct_name: "Foo".to_string(),
        type_params: vec![],
        for_trait: None,
        consts: vec![],
        methods: vec![],
    };
    assert_eq!(item.canonical_name(), Some("Foo"));
}

#[test]
fn test_canonical_name_use_returns_none() {
    let item = Item::Use {
        vis: Visibility::Private,
        path: "crate::foo".to_string(),
        names: vec!["Bar".to_string()],
    };
    assert_eq!(item.canonical_name(), None);
}

#[test]
fn test_canonical_name_comment_returns_none() {
    let item = Item::Comment("a comment".to_string());
    assert_eq!(item.canonical_name(), None);
}

#[test]
fn test_canonical_name_raw_code_returns_none() {
    let item = Item::RawCode("fn raw() {}".to_string());
    assert_eq!(item.canonical_name(), None);
}
