use super::*;

#[test]
fn test_narrowing_typeof_string() {
    let res = resolve(
        r#"
        function foo(x: any) {
            if (typeof x === "string") {
                console.log(x);
            }
        }
        "#,
    );
    let has_string_narrowing = res
        .narrowing_events
        .iter()
        .any(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::String));
    assert!(
        has_string_narrowing,
        "typeof guard should create String narrowing event"
    );
}

#[test]
fn test_narrowing_instanceof() {
    let res = resolve(
        r#"
        function foo(x: any) {
            if (x instanceof Error) {
                console.log(x);
            }
        }
        "#,
    );
    let has_error_narrowing = res.narrowing_events.iter().any(|e| {
        e.var_name == "x"
            && matches!(&e.narrowed_type, RustType::Named { name, .. } if name == "Error")
    });
    assert!(
        has_error_narrowing,
        "instanceof guard should create Error narrowing event"
    );
}

#[test]
fn test_narrowing_null_check() {
    let res = resolve(
        r#"
        function foo(x: string | null) {
            if (x !== null) {
                console.log(x);
            }
        }
        "#,
    );
    let has_non_null_narrowing = res
        .narrowing_events
        .iter()
        .any(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::String));
    assert!(
        has_non_null_narrowing,
        "null check should narrow Option<String> to String"
    );
}

#[test]
fn test_narrowing_eq_null_generates_event_in_alternate() {
    // I-327: `x === null` should narrow x to T in the ELSE block
    let res = resolve(
        r#"
        function foo(x: string | null): string {
            if (x === null) {
                return "null";
            } else {
                return x.trim();
            }
        }
        "#,
    );
    // Should have a narrowing event for x → String in the else block
    let has_else_narrowing = res
        .narrowing_events
        .iter()
        .any(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::String));
    assert!(
        has_else_narrowing,
        "=== null should create String narrowing event in else block, events: {:?}",
        res.narrowing_events
    );
}

#[test]
fn test_narrowing_eq_null_without_alternate_generates_no_event() {
    // `x === null` without else block → no narrowing event
    let res = resolve(
        r#"
        function foo(x: string | null): void {
            if (x === null) {
                console.log("null");
            }
        }
        "#,
    );
    // Should NOT have any narrowing event (no else block to narrow into)
    let has_narrowing = res
        .narrowing_events
        .iter()
        .any(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::String));
    assert!(
        !has_narrowing,
        "=== null without else should not create narrowing event"
    );
}

#[test]
fn test_narrowing_typeof_neq_generates_event_in_alternate() {
    // `typeof x !== "string"` should narrow x to String in the ELSE block
    let res = resolve(
        r#"
        function foo(x: any) {
            if (typeof x !== "string") {
                console.log("not string");
            } else {
                console.log(x);
            }
        }
        "#,
    );
    let has_else_narrowing = res
        .narrowing_events
        .iter()
        .any(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::String));
    assert!(
        has_else_narrowing,
        "typeof !== should create String narrowing event in else block, events: {:?}",
        res.narrowing_events
    );
}

#[test]
fn test_narrowing_compound_eq_null_and_typeof_generates_both_events() {
    // Compound: x !== null && typeof y === "string"
    // x narrowing → consequent (is_neq=true), y narrowing → consequent (is_eq=true)
    let res = resolve(
        r#"
        function foo(x: string | null, y: any) {
            if (x !== null && typeof y === "string") {
                console.log(x);
                console.log(y);
            }
        }
        "#,
    );
    let has_x_narrowing = res
        .narrowing_events
        .iter()
        .any(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::String));
    let has_y_narrowing = res
        .narrowing_events
        .iter()
        .any(|e| e.var_name == "y" && matches!(e.narrowed_type, RustType::String));
    assert!(
        has_x_narrowing,
        "compound guard should narrow x in consequent, events: {:?}",
        res.narrowing_events
    );
    assert!(
        has_y_narrowing,
        "compound guard should narrow y in consequent, events: {:?}",
        res.narrowing_events
    );
}

#[test]
fn test_narrowing_compound_no_alternate_narrowing_for_individual_guards() {
    // Compound with inverted null check: x === null && typeof y === "string"
    // In the else block, we know !(x===null && typeof y==="string") = (x!==null || typeof y!=="string")
    // This means we can NOT narrow x individually in the else block.
    // Alternate narrowing is only valid for simple (non-compound) conditions.
    let res = resolve(
        r#"
        function foo(x: string | null, y: any) {
            if (x === null && typeof y === "string") {
                console.log(y);
            } else {
                console.log(x);
            }
        }
        "#,
    );
    // x === null inside && → NO alternate narrowing (would be semantically incorrect)
    let x_events: Vec<_> = res
        .narrowing_events
        .iter()
        .filter(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::String))
        .collect();
    // y typeof === "string" inside && → consequent narrowing only
    let y_events: Vec<_> = res
        .narrowing_events
        .iter()
        .filter(|e| e.var_name == "y" && matches!(e.narrowed_type, RustType::String))
        .collect();
    assert_eq!(
        x_events.len(),
        0,
        "x should have NO narrowing events (compound && prevents alternate narrowing), events: {:?}",
        res.narrowing_events
    );
    assert_eq!(
        y_events.len(),
        1,
        "y should have 1 narrowing event (in consequent), events: {:?}",
        res.narrowing_events
    );
}
// ── narrowing: typeof "object"/"function" (I-215) ──

#[test]
fn test_narrowing_typeof_object_on_any_enum() {
    let (res, _synthetic) = resolve_with_any_analysis(
        r#"
        function foo(x: any) {
            if (typeof x === "object") {
                console.log(x);
            }
        }
        "#,
    );
    let has_object_narrowing = res
        .narrowing_events
        .iter()
        .any(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::Any));
    assert!(
        has_object_narrowing,
        "typeof 'object' on any should create narrowing event, events: {:?}",
        res.narrowing_events
    );
}

#[test]
fn test_narrowing_typeof_function_on_any_enum() {
    let (res, _synthetic) = resolve_with_any_analysis(
        r#"
        function foo(x: any) {
            if (typeof x === "function") {
                console.log(x);
            }
        }
        "#,
    );
    let has_function_narrowing = res
        .narrowing_events
        .iter()
        .any(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::Any));
    assert!(
        has_function_narrowing,
        "typeof 'function' on any should create narrowing event, events: {:?}",
        res.narrowing_events
    );
}

// ── complement narrowing (I-213) ──

#[test]
fn test_narrowing_complement_typeof_eq_in_alternate_2variant() {
    // typeof x === "string" on string|number → alternate should have F64 complement
    let res = resolve(
        r#"
        function foo(x: string | number): void {
            if (typeof x === "string") {
                console.log(x);
            } else {
                console.log(x);
            }
        }
        "#,
    );
    // Should have 2 events: String in consequent, F64 in alternate
    let has_f64_in_alt = res
        .narrowing_events
        .iter()
        .any(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::F64));
    assert!(
        has_f64_in_alt,
        "typeof === 'string' on string|number should create F64 complement in else, events: {:?}",
        res.narrowing_events
    );
}

#[test]
fn test_narrowing_complement_typeof_neq_in_consequent_2variant() {
    // typeof x !== "string" on string|number → consequent should have F64 complement
    let res = resolve(
        r#"
        function foo(x: string | number): void {
            if (typeof x !== "string") {
                console.log(x);
            } else {
                console.log(x);
            }
        }
        "#,
    );
    let has_f64_in_cons = res
        .narrowing_events
        .iter()
        .any(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::F64));
    assert!(
        has_f64_in_cons,
        "typeof !== 'string' on string|number should create F64 complement in then, events: {:?}",
        res.narrowing_events
    );
}

#[test]
fn test_narrowing_complement_3variant_sub_union() {
    // typeof x === "string" on string|number|boolean → alternate should have sub-union (F64|Bool)
    let (res, _synthetic) = resolve_with_synthetic(
        r#"
        function foo(x: string | number | boolean): void {
            if (typeof x === "string") {
                console.log(x);
            } else {
                console.log(x);
            }
        }
        "#,
    );
    // The complement should be a Named type (sub-union enum) containing F64 and Bool
    let complement_event = res
        .narrowing_events
        .iter()
        .find(|e| e.var_name == "x" && !matches!(e.narrowed_type, RustType::String));
    assert!(
        complement_event.is_some(),
        "3-variant union should have complement event, events: {:?}",
        res.narrowing_events
    );
    // The complement should be a Named enum (sub-union), not a primitive type
    let complement = complement_event.unwrap();
    let sub_union_name = match &complement.narrowed_type {
        RustType::Named { name, .. } => name.clone(),
        other => panic!(
            "complement of 3-variant union should be a Named sub-union, got: {:?}",
            other
        ),
    };
    // Verify the sub-union contains the correct variants (F64 and Bool)
    let sub_def = _synthetic
        .get(&sub_union_name)
        .expect("sub-union should exist in synthetic");
    let sub_variants = match &sub_def.item {
        crate::ir::Item::Enum { variants, .. } => {
            variants.iter().map(|v| v.name.as_str()).collect::<Vec<_>>()
        }
        _ => panic!("sub-union should be an Enum item"),
    };
    assert!(
        sub_variants.contains(&"F64"),
        "sub-union should contain F64, got: {:?}",
        sub_variants
    );
    assert!(
        sub_variants.contains(&"Bool"),
        "sub-union should contain Bool, got: {:?}",
        sub_variants
    );
    assert!(
        !sub_variants.contains(&"String"),
        "sub-union should NOT contain String, got: {:?}",
        sub_variants
    );
}

// ── early return narrowing (I-213) ──

#[test]
fn test_narrowing_early_return_typeof() {
    // if (typeof x === "string") { return x; } → x is F64 after
    let res = resolve(
        r#"
        function foo(x: string | number): number {
            if (typeof x === "string") {
                return 0;
            }
            return x;
        }
        "#,
    );
    // Should have complement narrowing for x → F64 after the if block
    let has_f64_after_if = res
        .narrowing_events
        .iter()
        .any(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::F64));
    assert!(
        has_f64_after_if,
        "early return with typeof should create complement narrowing, events: {:?}",
        res.narrowing_events
    );
}

#[test]
fn test_narrowing_early_return_null_check() {
    // if (x === null) { return; } → x is String after
    let res = resolve(
        r#"
        function foo(x: string | null): string {
            if (x === null) {
                return "";
            }
            return x;
        }
        "#,
    );
    // Should have narrowing for x → String after the if block
    let events_after_if: Vec<_> = res
        .narrowing_events
        .iter()
        .filter(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::String))
        .collect();
    assert!(
        !events_after_if.is_empty(),
        "early return with null check should narrow x to String after if, events: {:?}",
        res.narrowing_events
    );
}

#[test]
fn test_narrowing_early_return_throw() {
    // if (x === null) { throw new Error(); } → x is String after
    let res = resolve(
        r#"
        function foo(x: string | null): string {
            if (x === null) {
                throw new Error("null");
            }
            return x;
        }
        "#,
    );
    let events_after_if: Vec<_> = res
        .narrowing_events
        .iter()
        .filter(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::String))
        .collect();
    assert!(
        !events_after_if.is_empty(),
        "early return with throw should narrow x to String after if, events: {:?}",
        res.narrowing_events
    );
}

#[test]
fn test_narrowing_no_early_return_when_not_always_exit() {
    // if (typeof x === "string") { console.log(x); } → no complement after if
    let res = resolve(
        r#"
        function foo(x: string | number): void {
            if (typeof x === "string") {
                console.log(x);
            }
            console.log(x);
        }
        "#,
    );
    // Should NOT have F64 narrowing after the if (then-block doesn't always exit)
    let has_f64 = res
        .narrowing_events
        .iter()
        .any(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::F64));
    assert!(
        !has_f64,
        "non-exiting if should not create complement narrowing, events: {:?}",
        res.narrowing_events
    );
}

// ── early return: typeof !== (H5) ──

#[test]
fn test_narrowing_early_return_typeof_neq() {
    // if (typeof x !== "string") { return; } → x is String after
    let res = resolve(
        r#"
        function foo(x: string | number): string {
            if (typeof x !== "string") {
                return "";
            }
            return x;
        }
        "#,
    );
    let has_string_after_if = res
        .narrowing_events
        .iter()
        .any(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::String));
    assert!(
        has_string_after_if,
        "early return with typeof !== should narrow x to String, events: {:?}",
        res.narrowing_events
    );
}

// ── early return: negated truthy (C1) ──

#[test]
fn test_narrowing_early_return_negated_truthy() {
    // if (!x) { return; } → x is String after
    let res = resolve(
        r#"
        function foo(x: string | null): string {
            if (!x) {
                return "";
            }
            return x;
        }
        "#,
    );
    let has_string_after_if = res
        .narrowing_events
        .iter()
        .any(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::String));
    assert!(
        has_string_after_if,
        "early return with !x should narrow x to String, events: {:?}",
        res.narrowing_events
    );
}

// ── early return: instanceof (L-3.2) ──

#[test]
fn test_narrowing_early_return_instanceof() {
    // if (x instanceof Dog) { return; } → x is complement after
    // For any-narrowing enum with [Dog, Other], complement is None (Other excluded),
    // so no narrowing event is created. This tests that the code doesn't panic.
    let (res, _) = resolve_with_any_analysis(
        r#"
        class Dog { name: string; }
        function foo(x: any): void {
            if (x instanceof Dog) {
                return;
            }
            console.log(x);
        }
        "#,
    );
    // With any-narrowing [Dog(Dog), Other(Any)], complement of Dog excludes Other → None.
    // No complement narrowing event is expected.
    let has_instanceof_event = res.narrowing_events.iter().any(|e| {
        e.var_name == "x"
            && matches!(&e.narrowed_type, RustType::Named { name, .. } if name == "Dog")
    });
    assert!(
        has_instanceof_event,
        "instanceof should create positive narrowing event, events: {:?}",
        res.narrowing_events
    );
}

// ── early return scope boundary verification (H3) ──

#[test]
fn test_narrowing_early_return_scope_boundary() {
    // Verify that early return narrowing has correct scope_start/scope_end
    let res = resolve(
        r#"
        function foo(x: string | number): number {
            if (typeof x === "string") {
                return 0;
            }
            return x;
        }
        "#,
    );
    let complement_event = res
        .narrowing_events
        .iter()
        .find(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::F64));
    assert!(complement_event.is_some(), "should have F64 complement");
    let event = complement_event.unwrap();
    // scope_start should be AFTER the if-block ends (not inside it)
    // scope_end should be at the enclosing block end
    assert!(
        event.scope_start < event.scope_end,
        "scope_start ({}) should be less than scope_end ({})",
        event.scope_start,
        event.scope_end
    );
    // The positive String event should have a different scope
    let positive_event = res
        .narrowing_events
        .iter()
        .find(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::String));
    assert!(positive_event.is_some(), "should have String positive");
    let pos = positive_event.unwrap();
    // Positive event's scope (inside if block) should NOT overlap with complement's scope
    assert!(
        pos.scope_end <= event.scope_start,
        "positive scope end ({}) should be <= complement scope start ({})",
        pos.scope_end,
        event.scope_start
    );
}

// ── narrowing: typeof "number", "boolean", truthy (gap tests) ──

#[test]
fn test_narrowing_typeof_number() {
    let res = resolve(
        r#"
        function foo(x: any) {
            if (typeof x === "number") {
                console.log(x);
            }
        }
        "#,
    );
    let has_f64_narrowing = res
        .narrowing_events
        .iter()
        .any(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::F64));
    assert!(
        has_f64_narrowing,
        "typeof 'number' guard should create F64 narrowing event"
    );
}

#[test]
fn test_narrowing_typeof_boolean() {
    let res = resolve(
        r#"
        function foo(x: any) {
            if (typeof x === "boolean") {
                console.log(x);
            }
        }
        "#,
    );
    let has_bool_narrowing = res
        .narrowing_events
        .iter()
        .any(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::Bool));
    assert!(
        has_bool_narrowing,
        "typeof 'boolean' guard should create Bool narrowing event"
    );
}

#[test]
fn test_narrowing_truthy_check() {
    let res = resolve(
        r#"
        function foo(x: string | null) {
            if (x) {
                console.log(x);
            }
        }
        "#,
    );
    let has_truthy_narrowing = res
        .narrowing_events
        .iter()
        .any(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::String));
    assert!(
        has_truthy_narrowing,
        "truthy check should narrow Option<String> to String"
    );
}

// ── block_always_exits tests ──

mod block_always_exits_tests {
    use crate::parser::parse_typescript;
    use crate::pipeline::type_resolver::narrowing::block_always_exits;
    use swc_ecma_ast::{ModuleItem, Stmt};

    /// Parses a function body and returns the first statement.
    fn parse_first_stmt(fn_body: &str) -> Stmt {
        let source = format!("function f() {{ {fn_body} }}");
        let module = parse_typescript(&source).expect("parse failed");
        match &module.body[0] {
            ModuleItem::Stmt(Stmt::Decl(swc_ecma_ast::Decl::Fn(fn_decl))) => {
                fn_decl.function.body.as_ref().unwrap().stmts[0].clone()
            }
            _ => panic!("expected function declaration"),
        }
    }

    #[test]
    fn test_block_always_exits_return_true() {
        let stmt = parse_first_stmt("return 1;");
        assert!(block_always_exits(&stmt));
    }

    #[test]
    fn test_block_always_exits_throw_true() {
        let stmt = parse_first_stmt("throw new Error();");
        assert!(block_always_exits(&stmt));
    }

    #[test]
    fn test_block_always_exits_break_true() {
        let stmt = parse_first_stmt("while (true) { break; }");
        if let Stmt::While(w) = &stmt {
            if let Some(block) = w.body.as_block() {
                assert!(
                    block_always_exits(&block.stmts[0]),
                    "break should always exit"
                );
            }
        }
    }

    #[test]
    fn test_block_always_exits_continue_true() {
        let stmt = parse_first_stmt("while (true) { continue; }");
        if let Stmt::While(w) = &stmt {
            if let Some(block) = w.body.as_block() {
                assert!(
                    block_always_exits(&block.stmts[0]),
                    "continue should always exit"
                );
            }
        }
    }

    #[test]
    fn test_block_always_exits_empty_block_false() {
        let stmt = parse_first_stmt("{}");
        assert!(!block_always_exits(&stmt));
    }

    #[test]
    fn test_block_always_exits_block_ending_with_return_true() {
        let stmt = parse_first_stmt("{ const x = 1; return x; }");
        assert!(block_always_exits(&stmt));
    }

    #[test]
    fn test_block_always_exits_block_ending_with_expr_false() {
        let stmt = parse_first_stmt("{ const x = 1; x; }");
        assert!(!block_always_exits(&stmt));
    }

    #[test]
    fn test_block_always_exits_if_both_branches_exit_true() {
        let stmt = parse_first_stmt("if (true) { return 1; } else { return 2; }");
        assert!(block_always_exits(&stmt));
    }

    #[test]
    fn test_block_always_exits_if_only_then_exits_false() {
        let stmt = parse_first_stmt("if (true) { return 1; } else { console.log(1); }");
        assert!(!block_always_exits(&stmt));
    }

    #[test]
    fn test_block_always_exits_if_no_else_false() {
        let stmt = parse_first_stmt("if (true) { return 1; }");
        assert!(!block_always_exits(&stmt));
    }

    #[test]
    fn test_block_always_exits_expr_stmt_false() {
        let stmt = parse_first_stmt("console.log(1);");
        assert!(!block_always_exits(&stmt));
    }
}
