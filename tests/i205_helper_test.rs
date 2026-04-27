//! I-205 helper functions test contracts (Spec stage、F-deep-deep-4 fix 2026-04-28)。
//!
//! `lookup_method_kind_with_parent_traversal` helper の test contracts を Spec stage で
//! author。Implementation Stage T13 (B7 inherited dispatch) で actual probe code を
//! fill in、`#[ignore]` 解除で green-ify。
//!
//! **Test contracts (Spec stage commitment)**:
//! 1. Single-level inheritance: parent class methods lookup
//! 2. Multi-level inheritance: depth N (Sub → Mid → Base) traversal
//! 3. Circular inheritance prevention: visited HashSet で stack overflow 回避
//! 4. B1 vs B7 disambiguation: 直接定義 (is_inherited=false) vs 継承 (is_inherited=true)
//!
//! **Lesson source (deep deep review F-deep-deep-4)**: 当初 `lookup_method_kind_with_parent_traversal`
//! を Design section に追加したが Spec stage test contract 不在 = "helper definition without
//! verification commitment" compromise。Spec stage で stub `#[test] #[ignore]` を author し
//! structural commitment 確立。

use ts_to_rs::transpile;

#[test]
#[ignore = "I-205 helper test stub: Implementation Stage T13 で fill in (single-level inheritance probe)"]
fn test_lookup_method_kind_single_level_inherited_getter() {
    // Implementation Stage T13:
    // - Define class Base with `get x()` + class Sub extends Base
    // - Call lookup_method_kind_with_parent_traversal(reg, "Sub", "x", &mut visited)
    // - Assert: returns Some((MethodKind::Getter, true /* is_inherited */))
    let _ = transpile;
    unimplemented!("Spec stage stub、Implementation Stage T13 で実装");
}

#[test]
#[ignore = "I-205 helper test stub: Implementation Stage T13 で fill in (multi-level inheritance probe)"]
fn test_lookup_method_kind_multi_level_inherited_getter() {
    // Implementation Stage T13:
    // - Define class Base { get x() {} } / class Mid extends Base / class Sub extends Mid
    // - Call lookup_method_kind_with_parent_traversal(reg, "Sub", "x", &mut visited)
    // - Assert: returns Some((MethodKind::Getter, true)) via depth-2 traversal
    let _ = transpile;
    unimplemented!("Spec stage stub、Implementation Stage T13 で実装");
}

#[test]
#[ignore = "I-205 helper test stub: Implementation Stage T13 で fill in (circular inheritance prevention probe)"]
fn test_lookup_method_kind_circular_inheritance_prevention() {
    // Implementation Stage T13:
    // - Synthetic test: TypeRegistry に class A extends B / class B extends A
    //   (parser-level では invalid だが TypeRegistry build 時 cycle 形成可能性)
    // - Call lookup_method_kind_with_parent_traversal(reg, "A", "x", &mut visited)
    // - Assert: visited HashSet で stack overflow 回避、None or first-level result return
    let _ = transpile;
    unimplemented!("Spec stage stub、Implementation Stage T13 で実装");
}

#[test]
#[ignore = "I-205 helper test stub: Implementation Stage T13 で fill in (B1 vs B7 disambiguation probe)"]
fn test_lookup_method_kind_direct_vs_inherited_disambiguation() {
    // Implementation Stage T13:
    // - Define class Foo { get x() {} } (B1 direct) / class Bar extends Foo (B7 inherited)
    // - Call lookup for both Foo.x and Bar.x
    // - Assert: Foo returns is_inherited=false、Bar returns is_inherited=true
    // - Both kind = Getter
    let _ = transpile;
    unimplemented!("Spec stage stub、Implementation Stage T13 で実装");
}
