//! Bare registry operations: `get` / `all_items` / `generate_name` / `merge` /
//! `register_any_enum` + Item-shape verification (Enum / Struct).

use super::super::*;

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

// ── generate_name ──

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

// ── merge ──

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

// ── Item shape verification ──

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
