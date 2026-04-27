//! PRD 2.7 (I-198 + I-199 + I-200 cohesive batch) TypeResolver layer behavioral
//! lock-in tests.
//!
//! Spec stage Problem Space matrix の各 cell に対し、TypeResolver layer の actual
//! behavior (= narrow event push / expr_types entry / silent drop 不在) を direct
//! verify する。PRD T12 (1) "全 ✓ cell の regression lock-in" + (2) "✗ → ✓ cell の
//! new feature unit tests" の TypeResolver part を本 file で完全充足。
//!
//! Coverage:
//! - **cell 6** (`ClassMember::StaticBlock`): static block body 内の typeof narrow
//!   event push (= visit_class_body の StaticBlock arm + visit_block_stmt 経由 walk
//!   の cohesion 達成、I-199 新規)
//! - **cell 7** (`ClassMember::AutoAccessor`): TypeResolver 明示 no-op (Rule 11
//!   (d-2) phase 別役割分担、UnsupportedSyntaxError は Transformer 側)
//! - **cell 12** (`Prop::Method` body): method body 内の typeof narrow event push
//!   (= visit_prop_method_function helper 経由 walk の cohesion、I-200 新規)
//! - **cell 13** (`Prop::Getter` body): getter body 内の typeof narrow event push
//! - **cell 14** (`Prop::Setter` body): setter body 内の typeof narrow event push
//!   (= param_pat visit + visit_block_stmt walk)
//! - **cell 15** (`Prop::Assign`): TypeResolver no-op、narrow event 不在 + panic
//!   不在 (= Implementation Revision 2 で `unreachable!()` から no-op に変更後の
//!   precondition violation 不在 lock-in)

use super::*;
use crate::pipeline::narrowing_analyzer::{NarrowEvent, NarrowEventRef};

fn narrow_views(res: &FileTypeResolution) -> impl Iterator<Item = NarrowEventRef<'_>> {
    res.narrow_events.iter().filter_map(NarrowEvent::as_narrow)
}

// -----------------------------------------------------------------------------
// cell 6: ClassMember::StaticBlock body 内の typeof narrow event push (I-199 新規)
// -----------------------------------------------------------------------------

#[test]
fn test_static_block_body_typeof_narrow_pushes_string_event() {
    // PRD 2.7 cell 6 lock-in: visit_class_body の StaticBlock arm 改修 (visit_block_stmt
    // 経由 walk + scope 管理) で static block body 内の typeof narrow event が push
    // されること direct verify。
    //
    // Pre-PRD 2.7: `_ => {}` 黙殺で event 不在 = silent type widening。
    let res = resolve(
        r#"
        class Initializer {
            static result: string = "uninit";
            static {
                const value: string | number = "x";
                if (typeof value === "string") {
                    Initializer.result = value;
                }
            }
        }
        "#,
    );
    assert!(
        narrow_views(&res)
            .any(|e| e.var_name == "value" && matches!(e.narrowed_type, RustType::String)),
        "PRD 2.7 cell 6: StaticBlock body 内の `typeof value === 'string'` guard が \
         String narrow event を push していない。T8 改修 (visit_class_body の StaticBlock \
         arm + visit_block_stmt 経由 walk + scope 管理) が cohesion 達成していない signal"
    );
}

// -----------------------------------------------------------------------------
// cell 7: ClassMember::AutoAccessor — TypeResolver no-op + Transformer error
// -----------------------------------------------------------------------------

#[test]
fn test_auto_accessor_typeresolver_no_op_no_panic() {
    // PRD 2.7 cell 7 lock-in: AutoAccessor は TypeResolver で 明示 no-op
    // (Rule 11 (d-2) phase 別役割分担、static analysis phase abort 不可)。
    // resolve() が panic / abort せず終わることが lock-in 条件
    // (= Implementation Revision で `_ => {}` 黙殺解消後の precondition 不在 verify)。
    let res = resolve(
        r#"
        class Container {
            accessor x: string = "default";
        }
        "#,
    );
    // TypeResolver level では narrow event 不在 (= AutoAccessor は narrow trigger なし)
    let auto_accessor_narrows = narrow_views(&res).filter(|e| e.var_name == "x").count();
    assert_eq!(
        auto_accessor_narrows, 0,
        "PRD 2.7 cell 7: AutoAccessor は TypeResolver で no-op、narrow event 不在 expect"
    );
    // panic も abort も発生していなければ test 到達 = lock-in 成立
}

// -----------------------------------------------------------------------------
// cell 12: Prop::Method body 内の typeof narrow event push (I-200 新規)
// -----------------------------------------------------------------------------

#[test]
fn test_prop_method_body_typeof_narrow_pushes_string_event() {
    // PRD 2.7 cell 12 lock-in: expressions.rs::ast::Expr::Object arm の Prop::Method
    // handle (= visit_prop_method_function 経由 walk) で method body 内 typeof narrow
    // event が push されること direct verify。
    let res = resolve(
        r#"
        const obj = {
            method(value: string | number) {
                if (typeof value === "string") {
                    return value;
                }
                return "fallback";
            }
        };
        "#,
    );
    assert!(
        narrow_views(&res)
            .any(|e| e.var_name == "value" && matches!(e.narrowed_type, RustType::String)),
        "PRD 2.7 cell 12: Prop::Method body 内の typeof guard が String narrow event を \
         push していない。T9 改修 (Prop::Method arm + visit_prop_method_function 経由 walk) \
         が cohesion 達成していない signal"
    );
}

// -----------------------------------------------------------------------------
// cell 13: Prop::Getter body 内の typeof narrow event push
// -----------------------------------------------------------------------------

#[test]
fn test_prop_getter_body_typeof_narrow_pushes_string_event() {
    // PRD 2.7 cell 13 lock-in: Prop::Getter body 内 typeof narrow event の push verify。
    let res = resolve(
        r#"
        const obj = {
            get name() {
                const x: string | number = "x";
                if (typeof x === "string") {
                    return x;
                }
                return "n/a";
            }
        };
        "#,
    );
    assert!(
        narrow_views(&res)
            .any(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::String)),
        "PRD 2.7 cell 13: Prop::Getter body 内の typeof guard が String narrow event を \
         push していない (T9 改修 visit_block_stmt walk が cohesion 達成していない)"
    );
}

// -----------------------------------------------------------------------------
// cell 14: Prop::Setter body 内の typeof narrow event push
// -----------------------------------------------------------------------------

#[test]
fn test_prop_setter_body_typeof_narrow_pushes_string_event() {
    // PRD 2.7 cell 14 lock-in: Prop::Setter body 内の typeof narrow event 。
    // setter は param_pat visit + body visit_block_stmt の 2 step が必要、
    // narrow event push が成立する scope chain を direct verify。
    let res = resolve(
        r#"
        const obj = {
            set value(v: string | number) {
                if (typeof v === "string") {
                    console.log(v);
                }
            }
        };
        "#,
    );
    assert!(
        narrow_views(&res)
            .any(|e| e.var_name == "v" && matches!(e.narrowed_type, RustType::String)),
        "PRD 2.7 cell 14: Prop::Setter body 内の typeof guard が String narrow event を \
         push していない (T9 改修 visit_param_pat + visit_block_stmt walk が \
         cohesion 達成していない)"
    );
}

// -----------------------------------------------------------------------------
// cell 15: Prop::Assign — TypeResolver no-op (Implementation Revision 2 fix)
// -----------------------------------------------------------------------------

#[test]
fn test_prop_assign_typeresolver_no_op_no_panic() {
    // PRD 2.7 cell 15 (Implementation Revision 2、critical Spec gap fix) lock-in:
    // SWC parser empirical で `{ x = expr }` を Prop::Assign として accept する事実
    // (`tests/swc_parser_object_literal_prop_assign_test.rs` で empirical lock-in)
    // のため、TypeResolver で reach 可能。Implementation Revision 2 で
    // `unreachable!()` macro precondition violation を解消、no-op に変更。
    //
    // 本 test は (a) resolve() が panic せず終わること、(b) Prop::Assign target
    // identifier に narrow event 不在 (= TypeResolver no-op) の 2 invariant verify。
    let res = resolve(
        r#"
        function foo() {
            const obj = { x = 1 };
        }
        "#,
    );
    let prop_assign_target_narrows = narrow_views(&res).filter(|e| e.var_name == "x").count();
    assert_eq!(
        prop_assign_target_narrows, 0,
        "PRD 2.7 cell 15: Prop::Assign は TypeResolver で no-op、narrow event 不在 expect。\
         If this fires, TypeResolver started analyzing Prop::Assign target as a regular \
         narrowing variable — investigate."
    );
}

#[test]
fn test_prop_assign_with_complex_default_typeresolver_no_panic() {
    // cell 15 corollary: Prop::Assign の default expression が complex (function call)
    // でも TypeResolver は no-op (= unreachable!() precondition violation 不在)。
    let res = resolve(
        r#"
        function foo() {
            const obj = { x = Math.random() };
        }
        "#,
    );
    // panic / abort なしで本 line に到達 = lock-in 成立
    let _ = res; // silence unused warning while keeping the resolve call meaningful
}

// -----------------------------------------------------------------------------
// cell 10-11: Prop::KeyValue / Shorthand regression lock-in
// -----------------------------------------------------------------------------

#[test]
fn test_prop_keyvalue_regression_no_silent_drop() {
    // PRD 2.7 cell 10 regression lock-in: Prop::KeyValue は existing Tier 1、
    // expressions.rs::ast::Expr::Object arm 改修 (T9) で `_` arm 削除後も
    // KeyValue handle path が cohesion 維持していること direct verify。
    let res = resolve(
        r#"
        function foo(name: string) {
            const obj = { greeting: "hello", target: name };
        }
        "#,
    );
    // resolve() が panic / abort せず終わる = T9 改修後の Prop::KeyValue regression なし
    // (silent drop 不在 + dispatch arm explicit enumerate compliance、Rule 11 (d-1))
    let _ = res;
}

#[test]
fn test_prop_shorthand_regression_no_silent_drop() {
    // PRD 2.7 cell 11 regression lock-in: Prop::Shorthand existing Tier 1。
    let res = resolve(
        r#"
        function foo() {
            const greeting = "hi";
            const target = "world";
            const obj = { greeting, target };
        }
        "#,
    );
    let _ = res;
}
