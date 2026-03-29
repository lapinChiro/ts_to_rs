use super::*;

// Uses `build_shape_registry()` from tests/mod.rs:
// Shape enum with tag_field="kind", variants: Circle(radius: f64), Square(width: f64, height: f64).

#[test]
fn test_du_switch_bindings_basic_records_field_access() {
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape): number {
    switch (s.kind) {
        case "circle":
            return s.radius;
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    let has_radius = res.du_field_bindings.iter().any(|b| b.var_name == "radius");
    assert!(
        has_radius,
        "DU switch should record 'radius' field binding, got: {:?}",
        res.du_field_bindings
    );
}

#[test]
fn test_du_switch_bindings_non_member_discriminant_skips() {
    let reg = build_shape_registry();
    // switch(x) where discriminant is a plain ident, not member expr
    let source = r#"
function f(x: string): void {
    switch (x) {
        case "circle":
            break;
    }
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.is_empty(),
        "non-member discriminant should produce no DU bindings, got: {:?}",
        res.du_field_bindings
    );
}

#[test]
fn test_du_switch_bindings_non_enum_type_skips() {
    // Object type not registered as enum → no bindings
    let reg = TypeRegistry::new();
    let source = r#"
function f(s: Unknown): void {
    switch (s.kind) {
        case "circle":
            return s.radius;
    }
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.is_empty(),
        "non-enum type should produce no DU bindings, got: {:?}",
        res.du_field_bindings
    );
}

#[test]
fn test_du_switch_bindings_tag_mismatch_skips() {
    // Shape enum has tag_field="kind", but switch uses s.type
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape): void {
    switch (s.type) {
        case "circle":
            return s.radius;
    }
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.is_empty(),
        "tag field mismatch should produce no DU bindings, got: {:?}",
        res.du_field_bindings
    );
}

#[test]
fn test_du_switch_bindings_fall_through_accumulates_variants() {
    let reg = build_shape_registry();
    // "circle" falls through to "square" body, both variants accumulated
    // "width" exists in Square, "radius" exists in Circle → both should be bound
    let source = r#"
function f(s: Shape): number {
    switch (s.kind) {
        case "circle":
        case "square":
            const r = s.radius;
            const w = s.width;
            return 0;
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    let has_radius = res.du_field_bindings.iter().any(|b| b.var_name == "radius");
    let has_width = res.du_field_bindings.iter().any(|b| b.var_name == "width");
    assert!(
        has_radius,
        "fall-through should accumulate Circle variant, binding 'radius', got: {:?}",
        res.du_field_bindings
    );
    assert!(
        has_width,
        "fall-through should accumulate Square variant, binding 'width', got: {:?}",
        res.du_field_bindings
    );
}

#[test]
fn test_du_switch_bindings_field_not_in_variant_skips() {
    let reg = build_shape_registry();
    // Circle variant has no "width" field → should not be in bindings
    let source = r#"
function f(s: Shape): number {
    switch (s.kind) {
        case "circle":
            return s.width;
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    let has_width = res.du_field_bindings.iter().any(|b| b.var_name == "width");
    assert!(
        !has_width,
        "field not in variant should not be recorded, got: {:?}",
        res.du_field_bindings
    );
}
