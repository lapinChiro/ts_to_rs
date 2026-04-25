//! Consolidated-match emission tests for `if (!x) ...` narrow lowering.
//!
//! Originally a single file (`truthy_complement_match.rs`); split into a
//! cohesive sub-folder once I-171 T5 + iterative-fix lock-in tests pushed
//! the file past the 1000-LOC budget.
//!
//! # Sub-module layout (cohesion-driven)
//!
//! - **This `mod.rs`**: shared assertion helpers + the I-144 T6-3 H-3
//!   mixed-union lock-in (the original sole inhabitant of the pre-T5
//!   file). The H-3 test is co-located with the helpers it pioneers
//!   because it predates and motivates the helper extraction.
//! - [`bang_layer_2`]: Bang `!x` Layer 2 lowering tests covering all
//!   three [`OptionTruthyShape`] variants for primitive / always-truthy
//!   inner types, peek-through wrappers, closure-reassign suppression,
//!   and the cross-cutting Some-wrap-coerce-via-narrow-event cohesion
//!   verification.
//! - [`synthetic_union`]: Bang `!x` × `Option<synthetic-union>` per-variant
//!   match emission (large fixtures with rich per-variant shadow / arm
//!   reconstruction assertions, separated to keep file sizes bounded).
//! - [`null_check_symmetric`]: `=== null` symmetric Let-wrap branch
//!   (deep-deep-deep-fix retroactive fill of
//!   [`Transformer::try_generate_narrowing_match`]) and its
//!   closure-reassign suppression — symmetric to the Bang Layer 2 path
//!   on a different code path.
//!
//! All tests use the full pipeline (TypeResolver + Transformer) and
//! inspect the emitted IR directly. The shared `convert_named_fn_body`
//! helper hides the pipeline boilerplate; the assertion helpers
//! (`extract_let_match_arms`, `extract_match_stmt_arms`,
//! `assert_arm_body_ends_with_tail_ident`) deduplicate the patterns
//! that recur across nearly every test.
//!
//! [`OptionTruthyShape`]: crate::transformer::statements::option_truthy_complement::OptionTruthyShape

use super::*;

mod bang_layer_2;
mod null_check_symmetric;
mod synthetic_union;

// -----------------------------------------------------------------------------
// Shared helpers
// -----------------------------------------------------------------------------

/// Parses a TS source containing function declarations, runs the
/// TypeResolver + Transformer pipeline, and returns the IR body of the
/// named function.
///
/// Bypasses the E2E Rust compile step — IR-level lock-in is sufficient
/// for the dispatch decisions inside the transformer (independent of
/// downstream coercion gaps that may block a full E2E run).
pub(super) fn convert_named_fn_body(source: &str, fn_name: &str) -> Vec<Stmt> {
    let module = parse_typescript(source).expect("parse failed");
    let source_reg = crate::registry::build_registry(&module);
    let mg = crate::pipeline::ModuleGraph::empty();
    let mut synthetic = SyntheticTypeRegistry::new();
    let parsed = crate::pipeline::ParsedFile {
        path: std::path::PathBuf::from("test.ts"),
        source: source.to_string(),
        module: module.clone(),
    };
    let mut resolver =
        crate::pipeline::type_resolver::TypeResolver::new(&source_reg, &mut synthetic);
    let res = resolver.resolve_file(&parsed);
    let tctx = TransformContext::new(&mg, &source_reg, &res, Path::new("test.ts"));
    let mut synthetic2 = synthetic;
    let items = Transformer::for_module(&tctx, &mut synthetic2)
        .transform_module(&module)
        .unwrap();
    items
        .iter()
        .find_map(|i| match i {
            crate::ir::Item::Fn { name, body, .. } if name == fn_name => Some(body.clone()),
            _ => None,
        })
        .expect("function not found")
}

/// Extracts the match arms from `Stmt::Let { init: Some(Expr::Match { arms, .. }), .. }`
/// at the given body index, asserting the let binds to `expected_name`.
///
/// Used by the EarlyReturn / EarlyReturnFromExitWithElse lock-in tests
/// where the lowering form is a Let-wrap match that rebinds the outer
/// var to the narrow value via outer-let.
pub(super) fn extract_let_match_arms<'a>(
    body: &'a [Stmt],
    idx: usize,
    expected_name: &str,
) -> &'a [crate::ir::MatchArm] {
    let Stmt::Let {
        name,
        init: Some(Expr::Match { arms, .. }),
        ..
    } = &body[idx]
    else {
        panic!(
            "expected `let {expected_name} = match ... {{ ... }};` at body[{idx}], got {:?}",
            body[idx]
        );
    };
    assert_eq!(
        name, expected_name,
        "outer let must rebind to expected var name"
    );
    arms
}

/// Extracts the match arms from `Stmt::Match { arms, .. }` at the given
/// body index. Used by the ElseBranch lock-in tests where the lowering
/// form is a bare `Stmt::Match` without an outer let wrap.
pub(super) fn extract_match_stmt_arms(body: &[Stmt], idx: usize) -> &[crate::ir::MatchArm] {
    let Stmt::Match { arms, .. } = &body[idx] else {
        panic!("expected `Stmt::Match` at body[{idx}], got {:?}", body[idx]);
    };
    arms
}

/// Asserts that the arm body ends with `Stmt::TailExpr(Expr::Ident(expected_name))`.
///
/// Used by the EarlyReturnFromExitWithElse lock-in tests where the
/// `Some(x)` arm runs the user else_body then tail-emits the narrowed
/// value to feed the outer let.
pub(super) fn assert_arm_body_ends_with_tail_ident(arm: &crate::ir::MatchArm, expected_name: &str) {
    let last = arm.body.last().expect("arm body must not be empty");
    let Stmt::TailExpr(Expr::Ident(tail_name)) = last else {
        panic!("arm body must end with `TailExpr(Ident({expected_name}))`, got {last:?}");
    };
    assert_eq!(tail_name, expected_name);
}

// -----------------------------------------------------------------------------
// I-144 T6-3 H-3 mixed-union lock-in
// -----------------------------------------------------------------------------

/// I-144 T6-3 H-3 lock-in: consolidated `match` emission for `!x`
/// early-return on `Option<Union>` where the union mixes primitive and
/// Named (object) variants.
///
/// The Named variant must emit a guard-less arm (JS always-truthy
/// semantics for object references); the primitive variant must carry a
/// truthy guard (`!is_empty()` for `String`).
///
/// The shared `build_union_variant_truthy_arms` helper inside
/// [`Transformer::try_generate_option_truthy_complement_match`] is what
/// produces the per-variant arms — this test locks in the per-variant
/// guard / no-guard split independently of any call-arg coercion gap
/// that would otherwise block a full E2E run.
#[test]
fn h3_mixed_union_emits_guard_only_for_primitives() {
    use crate::ir::{CallTarget, MatchArm, PatternCtor};

    let body = convert_named_fn_body(
        r#"
interface Tag { label: string; }
function describe(x: string | Tag | null): string {
    if (!x) return "none";
    if (typeof x === "string") return "s:" + x;
    return "tag:" + x.label;
}
"#,
        "describe",
    );

    let arms = extract_let_match_arms(&body, 0, "x");

    // Expect 3 arms: String variant (with `!is_empty` guard), Tag variant
    // (guard-less, JS always-truthy), and the `_ => return ...` exit arm.
    assert_eq!(
        arms.len(),
        3,
        "expected 3 arms (String guard / Tag guard-less / exit), got {arms:?}"
    );

    let variant_pattern = |arm: &MatchArm, expected_variant: &str| -> bool {
        let Pattern::TupleStruct {
            ctor: PatternCtor::Builtin(crate::ir::BuiltinVariant::Some),
            fields,
        } = &arm.patterns[0]
        else {
            return false;
        };
        let Some(Pattern::TupleStruct {
            ctor: PatternCtor::UserEnumVariant { variant, .. },
            ..
        }) = fields.first()
        else {
            return false;
        };
        variant == expected_variant
    };

    assert!(
        variant_pattern(&arms[0], "String") && arms[0].guard.is_some(),
        "arm 0 must be Some(Union::String(_)) WITH `!is_empty()` guard, got {:?}",
        arms[0]
    );
    assert!(
        variant_pattern(&arms[1], "Tag") && arms[1].guard.is_none(),
        "arm 1 must be Some(Union::Tag(_)) WITHOUT guard (JS always-truthy), got {:?}",
        arms[1]
    );
    assert!(
        matches!(arms[2].patterns[0], Pattern::Wildcard) && arms[2].guard.is_none(),
        "arm 2 must be `_ => <exit>`, got {:?}",
        arms[2]
    );

    // The Tag arm body must reconstruct the variant via
    // `Union::Tag(__ts_union_inner)` so the outer `let x = match x { ... }`
    // rebinds `x` to the (narrowed) synthetic-union type, NOT the bare
    // inner.
    let Stmt::TailExpr(Expr::FnCall {
        target: CallTarget::UserEnumVariantCtor { variant, .. },
        ..
    }) = &arms[1].body[0]
    else {
        panic!(
            "Tag arm body must be TailExpr(Union::Tag(...)), got {:?}",
            arms[1].body
        );
    };
    assert_eq!(variant, "Tag", "Tag arm body must reconstruct Tag variant");
}
