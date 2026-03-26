use super::*;

#[test]
fn test_transform_enum_numeric_auto_values() {
    let source = "enum Color { Red, Green, Blue }";
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Enum {
            vis,
            name,
            variants,
            ..
        } => {
            assert_eq!(*vis, Visibility::Private);
            assert_eq!(name, "Color");
            assert_eq!(variants.len(), 3);
            assert_eq!(variants[0].name, "Red");
            assert_eq!(variants[0].value, None);
            assert_eq!(variants[1].name, "Green");
            assert_eq!(variants[2].name, "Blue");
        }
        _ => panic!("expected Enum"),
    }
}

#[test]
fn test_transform_enum_numeric_explicit_values() {
    let source = "enum Status { Active = 1, Inactive = 0 }";
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Enum { variants, .. } => {
            assert_eq!(variants[0].name, "Active");
            assert_eq!(variants[0].value, Some(crate::ir::EnumValue::Number(1)));
            assert_eq!(variants[1].name, "Inactive");
            assert_eq!(variants[1].value, Some(crate::ir::EnumValue::Number(0)));
        }
        _ => panic!("expected Enum"),
    }
}

#[test]
fn test_transform_enum_string_values() {
    let source = r#"enum Direction { Up = "UP", Down = "DOWN" }"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Enum { variants, .. } => {
            assert_eq!(variants[0].name, "Up");
            assert_eq!(
                variants[0].value,
                Some(crate::ir::EnumValue::Str("UP".to_string()))
            );
            assert_eq!(variants[1].name, "Down");
            assert_eq!(
                variants[1].value,
                Some(crate::ir::EnumValue::Str("DOWN".to_string()))
            );
        }
        _ => panic!("expected Enum"),
    }
}

#[test]
fn test_transform_enum_export_is_public() {
    let source = "export enum Color { Red, Green }";
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Enum { vis, .. } => assert_eq!(*vis, Visibility::Public),
        _ => panic!("expected Enum"),
    }
}

#[test]
fn test_transform_enum_empty() {
    let source = "enum Empty { }";
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Enum { variants, .. } => assert!(variants.is_empty()),
        _ => panic!("expected Enum"),
    }
}

#[test]
fn test_transform_enum_single_member() {
    let source = "enum Single { Only = -1 }";
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Enum { variants, .. } => {
            assert_eq!(variants.len(), 1);
            assert_eq!(variants[0].name, "Only");
            assert_eq!(variants[0].value, Some(crate::ir::EnumValue::Number(-1)));
        }
        _ => panic!("expected Enum"),
    }
}

#[test]
fn test_transform_enum_computed_member_bitshift() {
    let source = "enum Flags { Read = 1 << 0, Write = 1 << 1 }";
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Enum { variants, .. } => {
            assert_eq!(variants[0].name, "Read");
            assert_eq!(
                variants[0].value,
                Some(crate::ir::EnumValue::Expr("1 << 0".to_string()))
            );
            assert_eq!(variants[1].name, "Write");
            assert_eq!(
                variants[1].value,
                Some(crate::ir::EnumValue::Expr("1 << 1".to_string()))
            );
        }
        _ => panic!("expected Enum"),
    }
}
