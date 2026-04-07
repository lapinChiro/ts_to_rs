use super::*;

#[test]
fn test_walker_fn_call_type_ref_some_registers_user_type() {
    let item = fn_with_body(
        "f",
        vec![Stmt::Expr(Expr::FnCall {
            target: CallTarget::assoc("Color", "Red"),
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
            target: CallTarget::Path {
                segments: vec!["HashMap".to_string(), "from".to_string()],
                type_ref: None,
            },
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
            target: CallTarget::path(&["scopeguard", "guard"]),
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
    //   CallTarget::assoc("myClass", "new") — sets type_ref = Some("myClass")
    let item = fn_with_body(
        "f",
        vec![Stmt::Expr(Expr::FnCall {
            target: CallTarget::assoc("myClass", "new"),
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
            target: CallTarget::simple("Foo"), // simple() → type_ref: None
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
            target: CallTarget::simple("foo"),
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
