//! Cross-subsystem integration tests.
//!
//! Verifies behavior at boundaries between dedup origins, fork / merge round-trip,
//! and parent-type inheritance into forks (I-177-E invariant). These tests stress
//! interaction effects between otherwise-isolated registry operations.

use super::super::*;
use super::helpers::pub_field;

#[test]
fn test_cross_origin_dedup_reverse_order() {
    // Intersection first, then TypeLit — should still dedup
    let mut reg = SyntheticTypeRegistry::new();
    let (intersection_name, is_new) =
        reg.register_intersection_struct(&[pub_field("x", RustType::F64)]);
    assert!(is_new);
    let type_lit_name = reg.register_inline_struct(&[("x".to_string(), RustType::F64)]);
    assert_eq!(
        intersection_name, type_lit_name,
        "reverse order cross-origin dedup should work"
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
    // I-177-E: fork inherits both dedup state AND types so that subsequent
    // queries (e.g., `synthetic_enum_variants` for narrow guard) can see
    // builtin / parent-registered types.
    assert_eq!(
        forked.types.len(),
        1,
        "forked registry inherits parent's types (I-177-E)"
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
fn test_fork_dedup_state_inherits_types_for_query() {
    // I-177-E core invariant: parent で register された types は fork で
    // get(name) が Some を返す。これにより builtin / parent-inherited
    // synthetic types を fork から query 可能になる (narrow guard の
    // synthetic_enum_variants が成功する prerequisite)。
    let mut reg = SyntheticTypeRegistry::new();
    let name = reg.register_union(&[RustType::F64, RustType::String]);
    assert!(reg.get(&name).is_some(), "parent has the registered type");

    let forked = reg.fork_dedup_state();
    // Pre-fix RED expectation: forked.get(name) was None (types empty).
    // Post-fix GREEN: types cloned, query succeeds.
    assert!(
        forked.get(&name).is_some(),
        "fork inherits parent's types so queries on parent-registered names succeed"
    );
    // Variants should be identical to parent's
    let parent_variants = match &reg.get(&name).unwrap().item {
        Item::Enum { variants, .. } => variants.clone(),
        _ => panic!("expected Enum item"),
    };
    let fork_variants = match &forked.get(&name).unwrap().item {
        Item::Enum { variants, .. } => variants.clone(),
        _ => panic!("expected Enum item"),
    };
    assert_eq!(
        parent_variants, fork_variants,
        "fork's clone of parent's type has identical variants"
    );
}

#[test]
fn test_fork_dedup_state_round_trip_preserves_parent_types() {
    // I-177-E invariant INV-CE-3: parent → fork → register more in fork →
    // merge fork into parent → parent の元 types が unchanged で残る
    // (overwrite-with-same-value も含む round-trip 安全性)。
    let mut reg = SyntheticTypeRegistry::new();
    let parent_name = reg.register_union(&[RustType::F64, RustType::String]);
    let parent_variants_before = match &reg.get(&parent_name).unwrap().item {
        Item::Enum { variants, .. } => variants.clone(),
        _ => panic!(),
    };

    let mut forked = reg.fork_dedup_state();
    // Register a NEW type in fork (different signature)
    let new_name = forked.register_union(&[RustType::Bool, RustType::F64]);
    assert_ne!(parent_name, new_name);

    // Merge fork back into parent
    reg.merge(forked);

    // Parent's original type unchanged after round-trip
    let parent_variants_after = match &reg.get(&parent_name).unwrap().item {
        Item::Enum { variants, .. } => variants.clone(),
        _ => panic!(),
    };
    assert_eq!(
        parent_variants_before, parent_variants_after,
        "round-trip preserves parent's original type content"
    );
    // Parent now also has the new type added in fork
    assert!(
        reg.get(&new_name).is_some(),
        "merge brings fork's new types into parent"
    );
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
