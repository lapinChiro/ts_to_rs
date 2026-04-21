//! I-144 T6-3 H-3 lock-in: consolidated `match` emission for `!x` early-return on
//! `Option<Union>` where the union mixes primitive and Named (object) variants.
//! The Named variant must emit a guard-less arm (JS always-truthy semantics for
//! object references).
//!
//! This integration test runs the full pipeline (TypeResolver + Transformer)
//! and inspects the emitted IR directly, bypassing the E2E Rust compile step
//! because the separate call-arg Union coercion gap (non-literal → Union
//! variant wrap) blocks a full E2E run for this cell. The IR-level lock-in
//! is sufficient because the H-3 fix lives entirely inside
//! `build_union_variant_truthy_arms` — the consolidated match is materialized
//! well before any call site and is independent of call-arg coercion.

use super::*;

#[test]
fn test_try_generate_option_truthy_complement_match_h3_mixed_union_emits_guard_only_for_primitives()
{
    use crate::ir::{CallTarget, MatchArm, PatternCtor};

    let source = r#"
interface Tag {
    label: string;
}
function describe(x: string | Tag | null): string {
    if (!x) return "none";
    if (typeof x === "string") return "s:" + x;
    return "tag:" + x.label;
}
"#;
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

    // Extract the function `describe` and convert it.
    let mut synthetic2 = synthetic;
    let items = Transformer::for_module(&tctx, &mut synthetic2)
        .transform_module(&module)
        .unwrap();
    let describe = items
        .iter()
        .find_map(|i| {
            if let crate::ir::Item::Fn {
                name,
                body,
                return_type,
                ..
            } = i
            {
                if name == "describe" {
                    return Some((body.clone(), return_type.clone()));
                }
            }
            None
        })
        .expect("describe function not found");

    // First body stmt must be the consolidated match `let x = match x { ... }`
    // produced by `try_generate_option_truthy_complement_match`.
    let Stmt::Let {
        init: Some(Expr::Match { expr: _, ref arms }),
        ..
    } = describe.0[0]
    else {
        panic!(
            "expected first stmt to be `let x = match x {{ ... }}`, got {:?}",
            describe.0[0]
        );
    };

    // Expect 3 arms: String variant (with `!is_empty` guard), Tag variant
    // (guard-less, JS always-truthy), and the `_ => return ...` exit arm.
    assert_eq!(
        arms.len(),
        3,
        "expected 3 arms (String guard / Tag guard-less / exit), got {arms:?}"
    );

    let is_primitive_variant_arm = |arm: &MatchArm, expected_variant: &str| -> bool {
        // Pattern: Some(Enum::Variant(__ts_union_inner))
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
        variant == expected_variant && arm.guard.is_some()
    };

    let is_guard_less_named_arm = |arm: &MatchArm, expected_variant: &str| -> bool {
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
        variant == expected_variant && arm.guard.is_none()
    };

    let is_exit_arm = |arm: &MatchArm| -> bool {
        matches!(arm.patterns[0], Pattern::Wildcard) && arm.guard.is_none()
    };

    assert!(
        is_primitive_variant_arm(&arms[0], "String"),
        "arm 0 must be Some(Union::String(_)) WITH guard, got {:?}",
        arms[0]
    );
    assert!(
        is_guard_less_named_arm(&arms[1], "Tag"),
        "arm 1 must be Some(Union::Tag(_)) WITHOUT guard (JS always-truthy), got {:?}",
        arms[1]
    );
    assert!(
        is_exit_arm(&arms[2]),
        "arm 2 must be `_ => <exit>`, got {:?}",
        arms[2]
    );

    // Verify the Tag arm body re-emits `Tag` variant constructor (guard-less
    // branch must still re-wrap to preserve the union type in the outer `x`).
    let Stmt::TailExpr(Expr::FnCall {
        target: CallTarget::UserEnumVariantCtor { ref variant, .. },
        ..
    }) = arms[1].body[0]
    else {
        panic!(
            "Tag arm body must be TailExpr(Union::Tag(...)), got {:?}",
            arms[1].body
        );
    };
    assert_eq!(variant, "Tag", "Tag arm body must reconstruct Tag variant");
}
