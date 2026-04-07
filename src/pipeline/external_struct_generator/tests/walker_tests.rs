use super::*;

/// I-379: `Expr::BuiltinVariantValue(_)` (payload なしの builtin variant 値式参照、
/// 例: `None`) は walker から見て user type 参照を一切持たない構造化リーフであり、
/// `refs` に何も登録してはならない。旧 IR `Expr::Ident("None")` 時代は `is_external`
/// 事後フィルタに依存していた除外が、I-379 後は型レベルで保証される。
#[test]
fn test_walker_builtin_variant_value_does_not_register_any_refs() {
    for bv in [
        crate::ir::BuiltinVariant::Some,
        crate::ir::BuiltinVariant::None,
        crate::ir::BuiltinVariant::Ok,
        crate::ir::BuiltinVariant::Err,
    ] {
        let item = fn_with_body("f", vec![Stmt::Expr(Expr::BuiltinVariantValue(bv))]);
        let mut refs = HashSet::new();
        collect_type_refs_from_item(&item, &mut refs);
        assert!(
            refs.is_empty(),
            "BuiltinVariantValue({bv:?}) must not register any refs, got refs={refs:?}"
        );
    }
}

#[test]
fn test_walker_fn_call_type_ref_some_registers_user_type() {
    let item = fn_with_body(
        "f",
        vec![Stmt::Expr(Expr::FnCall {
            target: CallTarget::UserAssocFn {
                ty: crate::ir::UserTypeRef::new("Color"),
                method: "Red".to_string(),
            },
            args: vec![],
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(
        refs.contains("Color"),
        "type_ref: Some(\"Color\") must be registered, got refs={refs:?}"
    );
}

/// `CallTarget::Path { type_ref: None }` → walker must NOT register anything
/// even if a segment looks like a type name (Rust convention uppercase head).
/// This is the critical false-positive elimination.
#[test]
fn test_walker_fn_call_type_ref_none_skips_module_path_with_uppercase_segment() {
    // `HashMap::from(v)` — a std path with an uppercase head segment. The old
    // heuristic would have registered `HashMap`; the new structural walker
    // consults `type_ref` only and skips because it is `None`.
    let item = fn_with_body(
        "f",
        vec![Stmt::Expr(Expr::FnCall {
            target: CallTarget::ExternalPath(vec!["HashMap".to_string(), "from".to_string()]),
            args: vec![],
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(
        !refs.contains("HashMap"),
        "type_ref: None must skip registration, got refs={refs:?}"
    );
}

/// `CallTarget::Path { type_ref: None }` for a lowercase module-qualified call
/// (e.g. `scopeguard::guard(x)`) — should also skip. This is symmetric to the
/// previous test but with a lowercase head, to guarantee that the walker does
/// not secretly apply any case-based logic.
#[test]
fn test_walker_fn_call_type_ref_none_skips_lowercase_module_path() {
    let item = fn_with_body(
        "f",
        vec![Stmt::Expr(Expr::FnCall {
            target: CallTarget::ExternalPath(vec!["scopeguard".to_string(), "guard".to_string()]),
            args: vec![],
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(
        !refs.contains("scopeguard"),
        "lowercase module path must be skipped"
    );
    assert!(
        !refs.contains("guard"),
        "last segment must not be registered either"
    );
}

/// `CallTarget::Super` → walker must always skip, and must not confuse it with
/// a path whose first segment is `"super"`.
#[test]
fn test_walker_fn_call_super_is_skipped() {
    let item = fn_with_body(
        "f",
        vec![Stmt::Expr(Expr::FnCall {
            target: CallTarget::Super,
            args: vec![Expr::Ident("x".to_string())],
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.is_empty(), "Super must not register any refs");
}

/// **I-375 core correctness test**: a lowercase class name (`class myClass {}`)
/// must be captured by the walker because the Transformer records
/// `type_ref: Some("myClass")` structurally, independent of Rust naming
/// conventions. Before I-375, the walker's uppercase-head heuristic dropped
/// this reference silently.
#[test]
fn test_walker_lowercase_class_name_registered_via_type_ref() {
    // Construct a call that the Transformer would emit for `new myClass(1)`:
    //   CallTarget::UserAssocFn { ty: crate::ir::UserTypeRef::new("myClass"), method: "new".to_string() } — sets type_ref = Some("myClass")
    let item = fn_with_body(
        "f",
        vec![Stmt::Expr(Expr::FnCall {
            target: CallTarget::UserAssocFn {
                ty: crate::ir::UserTypeRef::new("myClass"),
                method: "new".to_string(),
            },
            args: vec![Expr::NumberLit(1.0)],
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(
        refs.contains("myClass"),
        "walker must register lowercase class names via type_ref, got refs={refs:?}"
    );
}

/// Symmetric to the lowercase-class case: an uppercase **free function** must
/// NOT be registered, even though its single segment starts with an uppercase
/// letter. The Transformer sets `type_ref: None` based on TypeRegistry lookup,
/// so the walker does not confuse conventions with semantics.
#[test]
fn test_walker_uppercase_free_function_not_registered_when_type_ref_is_none() {
    let item = fn_with_body(
        "f",
        vec![Stmt::Expr(Expr::FnCall {
            target: CallTarget::Free("Foo".to_string()), // simple() → type_ref: None
            args: vec![],
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(
        !refs.contains("Foo"),
        "uppercase free function must not be registered, got refs={refs:?}"
    );
}

/// The walker still recurses into `args` of a `Path` call so that nested user
/// type references (e.g. `foo(Wrapper { })`) are captured even when the
/// callee itself has no `type_ref`.
#[test]
fn test_walker_fn_call_recurses_into_args_even_when_target_has_no_type_ref() {
    let item = fn_with_body(
        "f",
        vec![Stmt::Expr(Expr::FnCall {
            target: CallTarget::Free("foo".to_string()),
            args: vec![Expr::StructInit {
                name: "Wrapper".to_string(),
                fields: vec![],
                base: None,
            }],
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(
        refs.contains("Wrapper"),
        "walker must recurse into args and register nested types"
    );
    assert!(
        !refs.contains("foo"),
        "call target `foo` must not be registered"
    );
}

// I-378 Phase 1: walker must recognise the structured value-position variants.

/// `Expr::EnumVariant { enum_ty: UserTypeRef("Color"), .. }` → walker must
/// register `Color` in refs. This is the structural replacement for the previous
/// `Expr::Ident("Color::Red")` form which the walker could not parse.
#[test]
fn test_walker_enum_variant_value_registers_parent_user_type() {
    let item = fn_with_body(
        "f",
        vec![Stmt::Expr(Expr::EnumVariant {
            enum_ty: crate::ir::UserTypeRef::new("Color"),
            variant: "Red".to_string(),
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(
        refs.contains("Color"),
        "Expr::EnumVariant must register the parent enum type, got refs={refs:?}"
    );
}

/// `Expr::PrimitiveAssocConst { f64::NAN }` → walker must NOT register `f64`
/// (primitive types are not user-defined). This is the structural replacement
/// for the broken-window `Expr::Ident("f64::NAN")` form.
#[test]
fn test_walker_primitive_assoc_const_does_not_register_primitive_type() {
    let item = fn_with_body(
        "f",
        vec![Stmt::Expr(Expr::PrimitiveAssocConst {
            ty: crate::ir::PrimitiveType::F64,
            name: "NAN".to_string(),
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(
        !refs.contains("f64"),
        "primitive types must not be registered, got refs={refs:?}"
    );
}

/// `Expr::StdConst(F64Pi)` → walker must NOT register anything (std module
/// paths are not user types).
#[test]
fn test_walker_std_const_does_not_register() {
    let item = fn_with_body(
        "f",
        vec![Stmt::Expr(Expr::StdConst(crate::ir::StdConst::F64Pi))],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(
        refs.is_empty()
            || (!refs.contains("std")
                && !refs.contains("f64")
                && !refs.contains("consts")
                && !refs.contains("PI")),
        "StdConst must not register any segment as a user type, got refs={refs:?}"
    );
}

// I-378 完全カバレッジ: 7 CallTarget variant 全てに対する walker integration test。
// PRD T7 で IrVisitor 化された TypeRefCollector が各 variant を構造的に正しく
// 処理することを保証する。前 review で hook firing test (visit_tests.rs) は
// 追加されたが、TypeRefCollector → 実 walker 経路の integration は未検証だった。

#[test]
fn test_walker_user_tuple_ctor_registers_user_type() {
    // `Wrapper(x)` for `interface Wrapper { (x: T): U }` (callable interface).
    let item = fn_with_body(
        "f",
        vec![Stmt::Expr(Expr::FnCall {
            target: CallTarget::UserTupleCtor(crate::ir::UserTypeRef::new("Wrapper")),
            args: vec![],
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(
        refs.contains("Wrapper"),
        "UserTupleCtor must register the inner UserTypeRef, got refs={refs:?}"
    );
}

#[test]
fn test_walker_user_enum_variant_ctor_registers_parent_enum_type() {
    // `Color::Red(x)` — payload-bearing enum variant constructor.
    let item = fn_with_body(
        "f",
        vec![Stmt::Expr(Expr::FnCall {
            target: CallTarget::UserEnumVariantCtor {
                enum_ty: crate::ir::UserTypeRef::new("Color"),
                variant: "Red".to_string(),
            },
            args: vec![],
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(
        refs.contains("Color"),
        "UserEnumVariantCtor must register the enum_ty, got refs={refs:?}"
    );
}

#[test]
fn test_walker_builtin_variant_does_not_register_anything() {
    // `Some(x)` / `None` / `Ok(x)` / `Err(x)` — Option/Result builtin constructors.
    // 型レベルで `UserTypeRef` を持たないため walker は何も登録しない。
    // これにより `RUST_BUILTIN_TYPES` から Some/None/Ok/Err のハードコード除外が
    // 構造的に不要になる (I-377 + I-378 で達成済み)。
    use crate::ir::BuiltinVariant;
    for v in [
        BuiltinVariant::Some,
        BuiltinVariant::None,
        BuiltinVariant::Ok,
        BuiltinVariant::Err,
    ] {
        let item = fn_with_body(
            "f",
            vec![Stmt::Expr(Expr::FnCall {
                target: CallTarget::BuiltinVariant(v),
                args: vec![],
            })],
        );
        let mut refs = HashSet::new();
        collect_type_refs_from_item(&item, &mut refs);
        assert!(
            refs.is_empty()
                || (!refs.contains("Some")
                    && !refs.contains("None")
                    && !refs.contains("Ok")
                    && !refs.contains("Err")),
            "BuiltinVariant::{v:?} must not register any builtin name, got refs={refs:?}"
        );
    }
}
