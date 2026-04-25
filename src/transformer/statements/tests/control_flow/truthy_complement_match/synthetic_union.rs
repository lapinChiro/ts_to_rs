//! Bang `!x` × `Option<synthetic-union>` per-variant emission tests.
//!
//! Separated from [`super::bang_layer_2`] because the per-variant arm
//! reconstruction logic in
//! [`Transformer::build_union_variant_truthy_arms`] requires rich
//! pattern / shadow-let / variant-ctor assertions that produce sizable
//! tests (~80-180 LOC each). Keeping these in a dedicated module
//! preserves cohesion (synthetic-union-specific emission shape) while
//! staying within the per-file LOC budget.

use super::*;

/// I-171 T5 SG-T5-2 lock-in: ElseBranch shape × `Option<synthetic-union>`
/// produces per-variant arms whose body inlines a `let <var_name> =
/// Enum::Variant(__ts_union_inner);` shadow before running the
/// user-written else_body. This ensures references to the outer var
/// name inside the else_body resolve to the narrow Named (synthetic
/// union) value — not the surrounding `Option<Named>` — for *every*
/// variant branch independently.
///
/// PRD Matrix C-5 enumerates only `Option<primitive>`; the synthetic-
/// union extension is implemented (via [`build_union_variant_truthy_arms`]
/// taking a `shape` parameter) but was not enumerated in PRD. This test
/// records the design choice and locks in the per-variant shadow emission.
#[test]
fn else_branch_form_synthetic_union_inlines_per_variant_shadow_let() {
    use crate::ir::{CallTarget, MatchArm, PatternCtor};

    // `x: string | Tag | null` → synthetic union enum + None tag.
    // The else branch references `x` (now narrowed to the synthetic union).
    let body = convert_named_fn_body(
        r#"
interface Tag {
    label: string;
}
function describe(x: string | Tag | null): string {
    if (!x) {
        return "no";
    } else {
        return typeof x === "string" ? "s:" + x : "tag:" + x.label;
    }
}
"#,
        "describe",
    );
    // Function body must be exactly one `Stmt::Match` (no Let wrap because
    // ElseBranch form does not consume the match value into an outer let).
    assert_eq!(body.len(), 1, "expected single Stmt::Match, got {body:?}");
    let arms = extract_match_stmt_arms(&body, 0);

    // Expect 3 arms: String variant (with `!is_empty` guard), Tag variant
    // (guard-less, JS always-truthy), and the wildcard `_ => then_body` arm.
    assert_eq!(
        arms.len(),
        3,
        "expected 3 arms (String guard + Tag guard-less + _ wildcard), got {arms:?}"
    );

    // Helper: verify a variant arm body starts with `let x = Enum::Variant(...);`
    // shadow rebinding to the outer var name (`x`).
    let assert_shadow_let_with_variant = |arm: &MatchArm, expected_variant: &str| {
        let Stmt::Let {
            mutable,
            name,
            ty,
            init: Some(init),
        } = &arm.body[0]
        else {
            panic!(
                "variant arm body[0] must be `Stmt::Let`, got {:?}",
                arm.body[0]
            );
        };
        assert!(!*mutable, "shadow let must be immutable");
        assert_eq!(
            name, "x",
            "shadow let must bind to outer var name (`x`), got `{name}`"
        );
        assert!(
            ty.is_none(),
            "shadow let must omit type annotation (inferred from variant ctor)"
        );
        let Expr::FnCall {
            target: CallTarget::UserEnumVariantCtor { variant, .. },
            args,
        } = init
        else {
            panic!("shadow let init must be `Enum::Variant(...)` ctor call, got {init:?}");
        };
        assert_eq!(
            variant, expected_variant,
            "shadow let must reconstruct {expected_variant} variant"
        );
        assert_eq!(args.len(), 1, "variant ctor takes one arg (the inner)");
        let Expr::Ident(arg_name) = &args[0] else {
            panic!("variant ctor arg must be Ident, got {:?}", args[0]);
        };
        assert_eq!(arg_name, "__ts_union_inner");
        assert!(
            arm.body.len() > 1,
            "variant arm must inline else_body after the shadow let"
        );
    };

    // Pattern-name extractor (order is registry-driven, not source-driven).
    fn variant_name(arm: &MatchArm) -> Option<&str> {
        let Pattern::TupleStruct {
            ctor: PatternCtor::Builtin(crate::ir::BuiltinVariant::Some),
            fields,
        } = &arm.patterns[0]
        else {
            return None;
        };
        let Some(Pattern::TupleStruct {
            ctor: PatternCtor::UserEnumVariant { variant, .. },
            ..
        }) = fields.first()
        else {
            return None;
        };
        Some(variant.as_str())
    }

    let string_arm = arms
        .iter()
        .find(|a| variant_name(a) == Some("String"))
        .expect("expected a `Some(Union::String(_))` arm");
    let tag_arm = arms
        .iter()
        .find(|a| variant_name(a) == Some("Tag"))
        .expect("expected a `Some(Union::Tag(_))` arm");

    assert!(
        string_arm.guard.is_some(),
        "String arm must have truthy guard, got {string_arm:?}"
    );
    assert!(
        tag_arm.guard.is_none(),
        "Tag arm must be guard-less (always-truthy), got {tag_arm:?}"
    );

    assert_shadow_let_with_variant(string_arm, "String");
    assert_shadow_let_with_variant(tag_arm, "Tag");

    // The wildcard arm runs the user's then_body (`return "no";`); no shadow,
    // no extra setup.
    let wildcard_arm = arms
        .iter()
        .find(|a| matches!(a.patterns[0], Pattern::Wildcard))
        .expect("expected a wildcard arm");
    assert!(wildcard_arm.guard.is_none());
    assert!(
        !matches!(wildcard_arm.body.first(), Some(Stmt::Let { name, .. }) if name == "x"),
        "wildcard arm must NOT introduce a shadow let — `x` must stay bound \
         to the outer Option<Named> in the falsy branch, got {:?}",
        wildcard_arm.body
    );
    let Stmt::Return(Some(ret_expr)) = &wildcard_arm.body[0] else {
        panic!(
            "wildcard arm must `return <some expr>`, got {:?}",
            wildcard_arm.body[0]
        );
    };
    let lowered = format!("{ret_expr:?}");
    assert!(
        lowered.contains("\"no\""),
        "wildcard return expression must surface the literal \"no\", got {ret_expr:?}"
    );
}

/// I-171 T5 deep-fix Matrix C-5d sub-case + synthetic union: the same
/// "then-exit + else-non-exit" Let-wrap form applies to
/// `Option<synthetic-union>`. Each per-variant arm body must:
/// (1) inline `let <var_name> = Enum::Variant(__ts_union_inner);` shadow
///     so user else_body sees the narrow synthetic-union value;
/// (2) inline the user's else_body;
/// (3) tail-emit `<var_name>` so the outer `let x = match x { ... };`
///     rebinds `x: Union` post-if (synthetic union narrow materialised).
#[test]
fn then_exit_with_non_exit_else_synthetic_union_threads_narrow_through_outer_let() {
    use crate::ir::CallTarget;

    let body = convert_named_fn_body(
        r#"
interface Tag {
    label: string;
}
function describe(x: string | Tag | null): string {
    if (!x) {
        return "no";
    } else {
        // non-exit else (no return) — control falls through to post-if.
    }
    // Post-if must see `x` narrowed to the synthetic union so this
    // typeof check + member access compiles.
    return typeof x === "string" ? "s:" + x : "tag:" + x.label;
}
"#,
        "describe",
    );
    let arms = extract_let_match_arms(&body, 0, "x");

    // Per-variant arm bodies must end with `TailExpr(Ident("x"))` so the
    // outer let receives the narrowed (synthetic union) value. First stmt
    // must be the shadow `let x = Enum::Variant(__ts_union_inner);`.
    for arm in arms.iter() {
        if matches!(arm.patterns[0], Pattern::Wildcard) {
            continue;
        }
        assert_arm_body_ends_with_tail_ident(arm, "x");

        let Stmt::Let {
            name,
            init:
                Some(Expr::FnCall {
                    target: CallTarget::UserEnumVariantCtor { .. },
                    ..
                }),
            ..
        } = &arm.body[0]
        else {
            panic!(
                "variant arm body must start with `let x = Enum::Variant(...);` \
                 shadow, got {:?}",
                arm.body[0]
            );
        };
        assert_eq!(name, "x", "shadow let must bind to outer var name");
    }
}
