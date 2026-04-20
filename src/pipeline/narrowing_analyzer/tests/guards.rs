//! Unit tests for the guard-detection path of the narrowing analyzer.
//!
//! The `type_resolver/tests/narrowing` suite exercises the same code
//! end-to-end through the full [`TypeResolver`][] pipeline; these tests
//! instead drive [`detect_narrowing_guard`] /
//! [`detect_early_return_narrowing`] directly against a minimal mock
//! [`NarrowTypeContext`]. That keeps the trait contract covered in
//! isolation (missing-impl mocking, trait-boundary event pushing,
//! complement-registration side effect) without booting the resolver.
//!
//! [`TypeResolver`]: crate::pipeline::type_resolver::TypeResolver

use std::collections::HashMap;

use swc_common::Spanned;

use crate::ir::{EnumValue, EnumVariant, RustType};
use crate::parser::parse_typescript;
use crate::pipeline::narrowing_analyzer::{
    detect_early_return_narrowing, detect_narrowing_guard, NarrowEvent, NarrowTrigger,
    NarrowTypeContext, NullCheckKind, PrimaryTrigger,
};
use crate::pipeline::ResolvedType;

use swc_ecma_ast as ast;

/// Minimal in-memory [`NarrowTypeContext`] that supports the variable
/// lookups and synthetic-enum queries needed for guard detection
/// tests, and records every pushed [`NarrowEvent`].
struct MockCtx {
    /// Declared type of each variable by name.
    vars: HashMap<String, ResolvedType>,
    /// Synthetic enum variants by enum name.
    enums: HashMap<String, Vec<EnumVariant>>,
    /// Names produced by [`NarrowTypeContext::register_sub_union`]
    /// invocations, in call order. The enum is also *inserted* into
    /// `enums` so a later `synthetic_enum_variants` call on the
    /// registered name would succeed; the guard tests never need that
    /// round-trip, but the behavior matches the real
    /// [`SyntheticTypeRegistry::register_union`] contract.
    registered: Vec<String>,
    events: Vec<NarrowEvent>,
}

impl MockCtx {
    fn new() -> Self {
        Self {
            vars: HashMap::new(),
            enums: HashMap::new(),
            registered: Vec::new(),
            events: Vec::new(),
        }
    }

    fn with_var(mut self, name: &str, ty: RustType) -> Self {
        self.vars.insert(name.into(), ResolvedType::Known(ty));
        self
    }

    fn with_enum(mut self, name: &str, variants: Vec<EnumVariant>) -> Self {
        self.enums.insert(name.into(), variants);
        self
    }
}

impl NarrowTypeContext for MockCtx {
    fn lookup_var(&self, name: &str) -> ResolvedType {
        self.vars
            .get(name)
            .cloned()
            .unwrap_or(ResolvedType::Unknown)
    }

    fn synthetic_enum_variants(&self, enum_name: &str) -> Option<Vec<EnumVariant>> {
        self.enums.get(enum_name).cloned()
    }

    fn register_sub_union(&mut self, member_types: &[RustType]) -> String {
        let name = format!("_SubUnion{}", self.registered.len());
        // Match the real registry's behavior of producing a queryable
        // entry so follow-up complement lookups see the new enum.
        let variants = member_types
            .iter()
            .enumerate()
            .map(|(i, ty)| variant(&format!("V{i}"), ty.clone()))
            .collect();
        self.enums.insert(name.clone(), variants);
        self.registered.push(name.clone());
        name
    }

    fn push_narrow_event(&mut self, event: NarrowEvent) {
        self.events.push(event);
    }
}

fn variant(name: &str, data: RustType) -> EnumVariant {
    EnumVariant {
        name: name.into(),
        value: Some(EnumValue::Number(0)),
        data: Some(data),
        fields: Vec::new(),
    }
}

/// Parses a single top-level `if (...) { ... }` statement and returns
/// the test expression, consequent stmt, and optional alternate.
fn parse_if(source: &str) -> (Box<ast::Expr>, ast::Stmt, Option<ast::Stmt>) {
    let module = parse_typescript(source).expect("fixture must parse");
    for item in module.body {
        if let ast::ModuleItem::Stmt(ast::Stmt::If(if_stmt)) = item {
            return (if_stmt.test, *if_stmt.cons, if_stmt.alt.map(|boxed| *boxed));
        }
    }
    panic!("expected a top-level `if` statement; source:\n{source}");
}

/// Parses `source`, finds the first top-level `if`, and invokes
/// [`detect_narrowing_guard`] against `ctx`. Returns a reference to
/// the mutated context for post-condition assertions.
fn run_guard(source: &str, mut ctx: MockCtx) -> MockCtx {
    let (test, cons, alt) = parse_if(source);
    detect_narrowing_guard(&test, &cons, alt.as_ref(), &mut ctx);
    ctx
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[test]
fn typeof_primitive_emits_positive_only_when_no_alt() {
    let ctx = run_guard(
        r#"if (typeof x === "string") { foo(); }"#,
        MockCtx::new().with_var("x", RustType::Any),
    );
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().expect("Narrow event");
    assert_eq!(ev.var_name, "x");
    assert!(matches!(ev.narrowed_type, RustType::String));
    assert!(matches!(
        ev.trigger,
        NarrowTrigger::Primary(PrimaryTrigger::TypeofGuard(s)) if s == "string"
    ));
}

#[test]
fn typeof_primitive_complement_uses_sub_union_for_three_plus_variants() {
    // Variable has union enum `StringOrF64OrBool` with 3 variants.
    // typeof x === "string" → positive: String in cons. complement in
    // alt: sub-union of {F64, Bool}.
    let ctx = MockCtx::new()
        .with_var(
            "x",
            RustType::Named {
                name: "StringOrF64OrBool".into(),
                type_args: vec![],
            },
        )
        .with_enum(
            "StringOrF64OrBool",
            vec![
                variant("String", RustType::String),
                variant("F64", RustType::F64),
                variant("Bool", RustType::Bool),
            ],
        );
    let ctx = run_guard(r#"if (typeof x === "string") { a(); } else { b(); }"#, ctx);
    assert_eq!(
        ctx.registered.len(),
        1,
        "complement must register a sub-union for 3-variant union"
    );
    let registered = ctx.registered[0].clone();
    // Events: positive (String) + complement (sub-union)
    assert_eq!(ctx.events.len(), 2);
    let positive = ctx.events[0].as_narrow().unwrap();
    assert!(matches!(positive.narrowed_type, RustType::String));
    let complement = ctx.events[1].as_narrow().unwrap();
    assert!(
        matches!(complement.narrowed_type, RustType::Named { name, .. } if *name == registered)
    );
}

#[test]
fn typeof_complement_with_two_variant_union_uses_bare_type() {
    // 2-variant union → complement is the bare remaining type
    let ctx = MockCtx::new()
        .with_var(
            "x",
            RustType::Named {
                name: "StringOrF64".into(),
                type_args: vec![],
            },
        )
        .with_enum(
            "StringOrF64",
            vec![
                variant("String", RustType::String),
                variant("F64", RustType::F64),
            ],
        );
    let ctx = run_guard(r#"if (typeof x === "string") { a(); } else { b(); }"#, ctx);
    assert!(
        ctx.registered.is_empty(),
        "2-variant union must NOT register a sub-union"
    );
    assert_eq!(ctx.events.len(), 2);
    let complement = ctx.events[1].as_narrow().unwrap();
    assert!(matches!(complement.narrowed_type, RustType::F64));
}

#[test]
fn null_check_narrows_option_in_positive_scope_only() {
    let ctx = run_guard(
        r#"if (x !== null) { a(); } else { b(); }"#,
        MockCtx::new().with_var("x", RustType::Option(Box::new(RustType::String))),
    );
    // Positive branch only: no complement for null check (intentional —
    // the `else` branch retains `Option<T>` which matches Rust semantics).
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    assert_eq!(ev.var_name, "x");
    assert!(matches!(ev.narrowed_type, RustType::String));
    assert!(matches!(
        ev.trigger,
        NarrowTrigger::Primary(PrimaryTrigger::NullCheck(NullCheckKind::NotEqEqNull))
    ));
}

#[test]
fn null_check_on_non_option_is_a_no_op() {
    let ctx = run_guard(
        r#"if (x !== null) { a(); }"#,
        MockCtx::new().with_var("x", RustType::String),
    );
    assert!(
        ctx.events.is_empty(),
        "non-Option LHS must not generate a null-check narrow"
    );
}

#[test]
fn truthy_on_option_narrows_to_inner_type() {
    let ctx = run_guard(
        r#"if (x) { a(); }"#,
        MockCtx::new().with_var("x", RustType::Option(Box::new(RustType::F64))),
    );
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    assert_eq!(ev.var_name, "x");
    assert!(matches!(ev.narrowed_type, RustType::F64));
    assert!(matches!(
        ev.trigger,
        NarrowTrigger::Primary(PrimaryTrigger::Truthy)
    ));
}

#[test]
fn truthy_on_non_option_is_a_no_op() {
    // An un-narrowable LHS should quietly skip — the guard detector must
    // not push spurious events or panic.
    let ctx = run_guard(
        r#"if (x) { a(); }"#,
        MockCtx::new().with_var("x", RustType::String),
    );
    assert!(ctx.events.is_empty());
}

#[test]
fn instanceof_emits_positive_and_complement() {
    let ctx = MockCtx::new()
        .with_var(
            "x",
            RustType::Named {
                name: "ErrorOrStr".into(),
                type_args: vec![],
            },
        )
        .with_enum(
            "ErrorOrStr",
            vec![
                variant(
                    "Error",
                    RustType::Named {
                        name: "Error".into(),
                        type_args: vec![],
                    },
                ),
                variant("String", RustType::String),
            ],
        );
    let ctx = run_guard(r#"if (x instanceof Error) { a(); } else { b(); }"#, ctx);
    assert_eq!(ctx.events.len(), 2);
    let pos = ctx.events[0].as_narrow().unwrap();
    assert!(matches!(&pos.narrowed_type, RustType::Named { name, .. } if name == "Error"));
    assert!(matches!(
        pos.trigger,
        NarrowTrigger::Primary(PrimaryTrigger::InstanceofGuard(n)) if n == "Error"
    ));
    let complement = ctx.events[1].as_narrow().unwrap();
    assert!(matches!(&complement.narrowed_type, RustType::String));
}

#[test]
fn logical_and_recurses_on_both_legs_without_complement() {
    // `typeof x === "string" && typeof y === "number"` emits two
    // positive narrows, no complement (De Morgan: !(A && B) ≠ !A).
    let ctx = run_guard(
        r#"if (typeof x === "string" && typeof y === "number") { a(); } else { b(); }"#,
        MockCtx::new()
            .with_var("x", RustType::Any)
            .with_var("y", RustType::Any),
    );
    // Two positive narrows (one for x, one for y), zero complements.
    assert_eq!(ctx.events.len(), 2);
    let (lhs, rhs) = (
        ctx.events[0].as_narrow().unwrap(),
        ctx.events[1].as_narrow().unwrap(),
    );
    assert_eq!(lhs.var_name, "x");
    assert_eq!(rhs.var_name, "y");
    assert!(matches!(lhs.narrowed_type, RustType::String));
    assert!(matches!(rhs.narrowed_type, RustType::F64));
    // Both narrows target the positive (cons) span.
    assert_eq!(lhs.scope_start, rhs.scope_start);
    assert_eq!(lhs.scope_end, rhs.scope_end);
}

#[test]
fn unresolved_variable_never_narrows() {
    // `lookup_var` returns `Unknown` — detector must skip silently.
    let ctx = run_guard(
        r#"if (x) { a(); }"#,
        MockCtx::new(), // no vars registered
    );
    assert!(ctx.events.is_empty());
}

#[test]
fn typeof_neq_flips_positive_to_alt_and_complement_to_cons() {
    // `typeof x !== "string"`: positive (non-String) goes to alt,
    // complement (String) goes to cons. Inverts the `===` dispatch.
    let ctx = MockCtx::new()
        .with_var(
            "x",
            RustType::Named {
                name: "StringOrF64".into(),
                type_args: vec![],
            },
        )
        .with_enum(
            "StringOrF64",
            vec![
                variant("String", RustType::String),
                variant("F64", RustType::F64),
            ],
        );
    let ctx = run_guard(r#"if (typeof x !== "string") { a(); } else { b(); }"#, ctx);
    assert_eq!(ctx.events.len(), 2);
    // Event 0: positive (String) recorded against ALT span.
    let positive = ctx.events[0].as_narrow().unwrap();
    assert!(matches!(positive.narrowed_type, RustType::String));
    // Event 1: complement (F64 — the remaining variant) recorded against CONS span.
    let complement = ctx.events[1].as_narrow().unwrap();
    assert!(matches!(complement.narrowed_type, RustType::F64));
    // Spans must differ: positive → alt, complement → cons.
    assert_ne!(positive.scope_start, complement.scope_start);
}

#[test]
fn null_check_eq_null_narrows_alt_branch() {
    // `x === null`: the narrow (non-null T) belongs in the ALT (else)
    // branch, because the cons branch fires when x IS null.
    let ctx = run_guard(
        r#"if (x === null) { a(); } else { b(); }"#,
        MockCtx::new().with_var("x", RustType::Option(Box::new(RustType::F64))),
    );
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    assert!(matches!(ev.narrowed_type, RustType::F64));
    assert!(matches!(
        ev.trigger,
        NarrowTrigger::Primary(PrimaryTrigger::NullCheck(NullCheckKind::EqEqEqNull))
    ));
}

#[test]
fn null_check_classifies_loose_vs_strict_and_undefined() {
    // Decision table over {`==`, `!=`, `===`, `!==`} × {`null`, `undefined`}.
    // Strict variants distinguish `null` from `undefined`; loose ones merge
    // them (JS coercion semantics).
    let cases: &[(&str, NullCheckKind)] = &[
        (
            r#"if (x == null) { a(); } else { b(); }"#,
            NullCheckKind::EqNull,
        ),
        (r#"if (x != null) { a(); }"#, NullCheckKind::NotEqNull),
        (
            r#"if (x === null) { a(); } else { b(); }"#,
            NullCheckKind::EqEqEqNull,
        ),
        (r#"if (x !== null) { a(); }"#, NullCheckKind::NotEqEqNull),
        (
            r#"if (x === undefined) { a(); } else { b(); }"#,
            NullCheckKind::EqEqEqUndefined,
        ),
        (
            r#"if (x !== undefined) { a(); }"#,
            NullCheckKind::NotEqEqUndefined,
        ),
    ];
    for (src, expected_kind) in cases {
        let ctx = run_guard(
            src,
            MockCtx::new().with_var("x", RustType::Option(Box::new(RustType::String))),
        );
        assert_eq!(
            ctx.events.len(),
            1,
            "case `{src}` must emit exactly one Narrow event"
        );
        let ev = ctx.events[0].as_narrow().unwrap();
        assert!(
            matches!(
                &ev.trigger,
                NarrowTrigger::Primary(PrimaryTrigger::NullCheck(actual)) if actual == expected_kind
            ),
            "case `{src}` trigger mismatch; got {:?}",
            ev.trigger
        );
    }
}

#[test]
fn typeof_object_resolves_via_synthetic_enum_variant() {
    // `typeof x === "object"` requires looking up the variable's union
    // enum and finding a variant whose data type is `Named`. This
    // exercises the `resolve_typeof_narrowed_type_from_var` path that
    // primitive typeofs bypass.
    let ctx = MockCtx::new()
        .with_var(
            "x",
            RustType::Named {
                name: "UserOrStr".into(),
                type_args: vec![],
            },
        )
        .with_enum(
            "UserOrStr",
            vec![
                variant(
                    "User",
                    RustType::Named {
                        name: "User".into(),
                        type_args: vec![],
                    },
                ),
                variant("String", RustType::String),
            ],
        );
    let ctx = run_guard(r#"if (typeof x === "object") { a(); }"#, ctx);
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    assert!(matches!(&ev.narrowed_type, RustType::Named { name, .. } if name == "User"));
    assert!(matches!(
        &ev.trigger,
        NarrowTrigger::Primary(PrimaryTrigger::TypeofGuard(s)) if s == "object"
    ));
}

// -----------------------------------------------------------------------------
// Early-return complement path
// -----------------------------------------------------------------------------

#[test]
fn early_return_null_check_narrows_fallthrough_scope() {
    // `if (x === null) return;` followed by code — the fall-through
    // should narrow `x` to the Option's inner type.
    let source = r#"
        function foo(x: string | null) {
            if (x === null) { return; }
            console.log(x);
        }
    "#;
    // Find the if-stmt inside the function body.
    let module = parse_typescript(source).expect("parse");
    let ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl))) = &module.body[0] else {
        panic!("expected fn decl")
    };
    let body = fn_decl.function.body.as_ref().expect("fn has body");
    let ast::Stmt::If(if_stmt) = &body.stmts[0] else {
        panic!("expected if stmt")
    };
    let if_end = if_stmt.cons.span().hi.0;
    let block_end = body.span().hi.0;
    let mut ctx = MockCtx::new().with_var("x", RustType::Option(Box::new(RustType::String)));
    detect_early_return_narrowing(&if_stmt.test, if_end, block_end, &mut ctx);
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    assert!(matches!(ev.narrowed_type, RustType::String));
    assert!(ev.trigger.is_early_return_complement());
    assert_eq!(ev.scope_start, if_end);
    assert_eq!(ev.scope_end, block_end);
}

#[test]
fn early_return_bang_truthy_narrows_fallthrough_scope() {
    let source = r#"
        function foo(x: string | null) {
            if (!x) { return; }
            console.log(x);
        }
    "#;
    let module = parse_typescript(source).expect("parse");
    let ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl))) = &module.body[0] else {
        panic!("expected fn decl")
    };
    let body = fn_decl.function.body.as_ref().expect("fn has body");
    let ast::Stmt::If(if_stmt) = &body.stmts[0] else {
        panic!("expected if stmt")
    };
    let if_end = if_stmt.cons.span().hi.0;
    let block_end = body.span().hi.0;
    let mut ctx = MockCtx::new().with_var("x", RustType::Option(Box::new(RustType::String)));
    detect_early_return_narrowing(&if_stmt.test, if_end, block_end, &mut ctx);
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    assert!(matches!(ev.narrowed_type, RustType::String));
    assert!(matches!(
        ev.trigger,
        NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::Truthy)
    ));
}

#[test]
fn early_return_typeof_narrows_fallthrough_to_complement() {
    // `if (typeof x === "string") return;` — fall-through has x as the
    // complement type (F64 here) via `EarlyReturnComplement`.
    let source = r#"
        function foo(x: string | number) {
            if (typeof x === "string") { return; }
            console.log(x);
        }
    "#;
    let module = parse_typescript(source).expect("parse");
    let ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl))) = &module.body[0] else {
        panic!("expected fn decl")
    };
    let body = fn_decl.function.body.as_ref().expect("fn has body");
    let ast::Stmt::If(if_stmt) = &body.stmts[0] else {
        panic!("expected if stmt")
    };
    let if_end = if_stmt.cons.span().hi.0;
    let block_end = body.span().hi.0;
    let mut ctx = MockCtx::new()
        .with_var(
            "x",
            RustType::Named {
                name: "StringOrF64".into(),
                type_args: vec![],
            },
        )
        .with_enum(
            "StringOrF64",
            vec![
                variant("String", RustType::String),
                variant("F64", RustType::F64),
            ],
        );
    detect_early_return_narrowing(&if_stmt.test, if_end, block_end, &mut ctx);
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    assert!(matches!(ev.narrowed_type, RustType::F64));
    assert!(matches!(
        &ev.trigger,
        NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::TypeofGuard(s)) if s == "string"
    ));
}

#[test]
fn early_return_instanceof_narrows_fallthrough_to_complement() {
    // `if (x instanceof Error) return;` — fall-through has x as the
    // complement (String here).
    let source = r#"
        function foo(x: Error | string) {
            if (x instanceof Error) { return; }
            console.log(x);
        }
    "#;
    let module = parse_typescript(source).expect("parse");
    let ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl))) = &module.body[0] else {
        panic!("expected fn decl")
    };
    let body = fn_decl.function.body.as_ref().expect("fn has body");
    let ast::Stmt::If(if_stmt) = &body.stmts[0] else {
        panic!("expected if stmt")
    };
    let if_end = if_stmt.cons.span().hi.0;
    let block_end = body.span().hi.0;
    let mut ctx = MockCtx::new()
        .with_var(
            "x",
            RustType::Named {
                name: "ErrorOrStr".into(),
                type_args: vec![],
            },
        )
        .with_enum(
            "ErrorOrStr",
            vec![
                variant(
                    "Error",
                    RustType::Named {
                        name: "Error".into(),
                        type_args: vec![],
                    },
                ),
                variant("String", RustType::String),
            ],
        );
    detect_early_return_narrowing(&if_stmt.test, if_end, block_end, &mut ctx);
    assert_eq!(ctx.events.len(), 1);
    let ev = ctx.events[0].as_narrow().unwrap();
    assert!(matches!(ev.narrowed_type, RustType::String));
    assert!(matches!(
        &ev.trigger,
        NarrowTrigger::EarlyReturnComplement(PrimaryTrigger::InstanceofGuard(n)) if n == "Error"
    ));
}

#[test]
fn early_return_skips_empty_fallthrough_scope() {
    // if_end >= block_end → detector must be a no-op, no events.
    let source = r#"
        function foo(x: string | null) {
            if (x === null) { return; }
        }
    "#;
    let module = parse_typescript(source).expect("parse");
    let ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl))) = &module.body[0] else {
        panic!("expected fn decl")
    };
    let body = fn_decl.function.body.as_ref().expect("fn has body");
    let ast::Stmt::If(if_stmt) = &body.stmts[0] else {
        panic!("expected if stmt")
    };
    let if_end = if_stmt.cons.span().hi.0;
    // Simulate a zero-width fall-through by placing block_end AT if_end.
    let block_end = if_end;
    let mut ctx = MockCtx::new().with_var("x", RustType::Option(Box::new(RustType::String)));
    detect_early_return_narrowing(&if_stmt.test, if_end, block_end, &mut ctx);
    assert!(
        ctx.events.is_empty(),
        "empty fall-through range must produce no events"
    );
}
