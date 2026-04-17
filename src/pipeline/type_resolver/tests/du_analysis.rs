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
fn test_tpl_inner_member_expr_is_resolved() {
    // I-021 root cause: `resolve_expr_inner::Tpl` must recurse into tpl.exprs
    // so that inner Member expressions (e.g., `event.x` inside `` `${event.x}` ``)
    // have their Ident `event` type registered in expr_types. Without this,
    // Transformer's `get_expr_type(&event)` returns None, DU detection fails,
    // and raw `event.x` is emitted instead of the destructured `x.clone()`.
    let reg = build_shape_registry();
    let source = r#"
function describe(s: Shape): string {
    switch (s.kind) {
        case "circle":
            return `radius=${s.radius}`;
    }
    return "";
}
"#;
    let res = resolve_with_reg(source, &reg);

    // Parse the module to find the `s` ident inside the template literal.
    let files = crate::pipeline::parse_files(vec![(
        std::path::PathBuf::from("test.ts"),
        source.to_string(),
    )])
    .unwrap();
    let file = &files.files[0];
    let fn_decl = match &file.module.body[0] {
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Fn(fd))) => fd,
        _ => panic!("expected fn decl"),
    };
    let switch_stmt = match &fn_decl.function.body.as_ref().unwrap().stmts[0] {
        swc_ecma_ast::Stmt::Switch(sw) => sw,
        _ => panic!("expected switch stmt"),
    };
    let first_case_return = match &switch_stmt.cases[0].cons[0] {
        swc_ecma_ast::Stmt::Return(ret) => ret,
        _ => panic!("expected return stmt"),
    };
    let tpl = match first_case_return.arg.as_deref() {
        Some(swc_ecma_ast::Expr::Tpl(t)) => t,
        _ => panic!("expected template literal"),
    };
    let member = match tpl.exprs[0].as_ref() {
        swc_ecma_ast::Expr::Member(m) => m,
        _ => panic!("expected member expr inside template"),
    };
    let s_ident = match member.obj.as_ref() {
        swc_ecma_ast::Expr::Ident(id) => id,
        _ => panic!("expected ident for member object"),
    };
    let s_span = Span::from_swc(s_ident.span);
    let ty = res.expr_type(s_span);
    assert!(
        matches!(ty, ResolvedType::Known(RustType::Named { name, .. }) if name == "Shape"),
        "s inside template literal should be resolved to Shape, got: {:?}",
        ty
    );

    // `s.radius` on the plain `Shape` enum type cannot be statically resolved
    // (the field only exists in the `Circle` variant), so `expr_types` may
    // legitimately return `Unknown` for the member span. The critical
    // invariant exercised above is that the inner `s` Ident is resolved,
    // which is what Transformer uses to detect the DU type in
    // `member_access.rs::convert_member_access`.
}

#[test]
fn test_du_field_binding_inside_tpl_lookup_succeeds() {
    // I-021: Verify the full binding lookup path — detect_du_switch_bindings + Tpl recursion.
    // After Tpl fix, `is_du_field_binding("radius", position)` should return true
    // for any position within the case body (including inside the template literal).
    use swc_common::Spanned;
    let reg = build_shape_registry();
    let source = r#"
function describe(s: Shape): string {
    switch (s.kind) {
        case "circle":
            return `radius=${s.radius}`;
    }
    return "";
}
"#;
    let res = resolve_with_reg(source, &reg);

    // Find the `s.radius` Member expression span inside the template literal.
    let files = crate::pipeline::parse_files(vec![(
        std::path::PathBuf::from("test.ts"),
        source.to_string(),
    )])
    .unwrap();
    let file = &files.files[0];
    let fn_decl = match &file.module.body[0] {
        swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Fn(fd))) => fd,
        _ => panic!("expected fn decl"),
    };
    let switch_stmt = match &fn_decl.function.body.as_ref().unwrap().stmts[0] {
        swc_ecma_ast::Stmt::Switch(sw) => sw,
        _ => panic!("expected switch stmt"),
    };
    let first_case_return = match &switch_stmt.cases[0].cons[0] {
        swc_ecma_ast::Stmt::Return(ret) => ret,
        _ => panic!("expected return stmt"),
    };
    let tpl = match first_case_return.arg.as_deref() {
        Some(swc_ecma_ast::Expr::Tpl(t)) => t,
        _ => panic!("expected template literal"),
    };
    let member = match tpl.exprs[0].as_ref() {
        swc_ecma_ast::Expr::Member(m) => m,
        _ => panic!("expected member expr inside template"),
    };

    let member_lo = member.span().lo.0;
    assert!(
        res.is_du_field_binding("radius", member_lo),
        "radius should be a DU field binding at position {} (inside Tpl expression), \
         bindings: {:?}",
        member_lo,
        res.du_field_bindings
    );
}

#[test]
fn test_du_switch_bindings_collects_from_array_literal() {
    // I-021 walker coverage: `Expr::Array` elements must be walked so that
    // `return [s.radius]` binds `radius` in the match pattern.
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape): number[] {
    switch (s.kind) {
        case "circle":
            return [s.radius];
    }
    return [];
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "Array literal element should contribute to DU field bindings, got: {:?}",
        res.du_field_bindings
    );
}

#[test]
fn test_du_switch_bindings_collects_from_object_literal() {
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape): { r: number } {
    switch (s.kind) {
        case "circle":
            return { r: s.radius };
    }
    return { r: 0 };
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "Object literal value should contribute to DU field bindings, got: {:?}",
        res.du_field_bindings
    );
}

#[test]
fn test_du_switch_bindings_collects_from_unary() {
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape): number {
    switch (s.kind) {
        case "circle":
            return -s.radius;
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "Unary expression arg should contribute to DU field bindings"
    );
}

#[test]
fn test_du_switch_bindings_collects_from_await() {
    let reg = build_shape_registry();
    let source = r#"
async function f(s: Shape): Promise<number> {
    switch (s.kind) {
        case "circle":
            return await s.radius;
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "Await expression arg should contribute to DU field bindings"
    );
}

#[test]
fn test_du_switch_bindings_collects_from_ts_as() {
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape): number {
    switch (s.kind) {
        case "circle":
            return s.radius as number;
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "TsAs expression inner should contribute to DU field bindings"
    );
}

#[test]
fn test_du_switch_bindings_collects_from_ts_non_null() {
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape): number {
    switch (s.kind) {
        case "circle":
            return s.radius!;
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "TsNonNull expression inner should contribute to DU field bindings"
    );
}

#[test]
fn test_du_switch_bindings_collects_from_opt_chain() {
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape): number | undefined {
    switch (s.kind) {
        case "circle":
            return s.radius?.valueOf();
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "OptChain base should contribute to DU field bindings"
    );
}

#[test]
fn test_du_switch_bindings_collects_from_new_expr() {
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape): object {
    switch (s.kind) {
        case "circle":
            return new Number(s.radius);
    }
    return new Object();
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "New expression args should contribute to DU field bindings"
    );
}

#[test]
fn test_du_switch_bindings_collects_from_seq() {
    let reg = build_shape_registry();
    // SeqExpr: `(a, b, c)` — collects the last but should walk all.
    let source = r#"
function f(s: Shape): number {
    switch (s.kind) {
        case "circle":
            return (0, s.radius);
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "Seq expression should contribute to DU field bindings"
    );
}

#[test]
fn test_du_switch_bindings_collects_from_throw() {
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape): never {
    switch (s.kind) {
        case "circle":
            throw new Error(String(s.radius));
    }
    throw new Error("none");
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "Throw statement arg should contribute to DU field bindings"
    );
}

#[test]
fn test_du_switch_bindings_collects_from_while_loop() {
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape): number {
    switch (s.kind) {
        case "circle":
            while (s.radius > 0) {
                return s.radius;
            }
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "While loop test/body should contribute to DU field bindings"
    );
}

#[test]
fn test_du_switch_bindings_collects_from_for_loop() {
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape): number {
    switch (s.kind) {
        case "circle":
            for (let i = s.radius; i > 0; i--) {
                return i;
            }
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "For-loop init should contribute to DU field bindings"
    );
}

#[test]
fn test_du_switch_bindings_collects_from_try_catch() {
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape): number {
    switch (s.kind) {
        case "circle":
            try {
                return s.radius;
            } catch (e) {
                return 0;
            }
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "Try block should contribute to DU field bindings"
    );
}

#[test]
fn test_du_switch_bindings_collects_from_update_expr() {
    let reg = build_shape_registry();
    // `s.radius++` — walker must recurse into Update.arg.
    let source = r#"
function f(s: Shape): number {
    switch (s.kind) {
        case "circle":
            s.radius++;
            return s.radius;
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "Update expression arg should contribute to DU field bindings"
    );
}

#[test]
fn test_du_switch_bindings_collects_from_tagged_template() {
    let reg = build_shape_registry();
    let source = r#"
function tag(strs: TemplateStringsArray, ...vals: number[]): string {
    return strs.join("-");
}
function f(s: Shape): string {
    switch (s.kind) {
        case "circle":
            return tag`r=${s.radius}`;
    }
    return "";
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "TaggedTpl interpolation should contribute to DU field bindings"
    );
}

#[test]
fn test_du_switch_bindings_collects_from_for_in_loop() {
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape): number {
    switch (s.kind) {
        case "circle":
            for (const key in { r: s.radius }) {
                return 1;
            }
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "ForIn loop right-hand side should contribute to DU field bindings"
    );
}

#[test]
fn test_du_switch_bindings_collects_from_for_of_loop() {
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape): number {
    switch (s.kind) {
        case "circle":
            for (const n of [s.radius]) {
                return n;
            }
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "ForOf loop right-hand side should contribute to DU field bindings"
    );
}

#[test]
fn test_du_switch_bindings_collects_from_do_while_loop() {
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape): number {
    switch (s.kind) {
        case "circle":
            do {
                return s.radius;
            } while (s.radius > 0);
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "DoWhile loop body + test should contribute to DU field bindings"
    );
}

#[test]
fn test_du_switch_bindings_collects_from_nested_switch() {
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape, n: number): number {
    switch (s.kind) {
        case "circle":
            switch (n) {
                case 1: return s.radius;
                case 2: return 0;
            }
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "Nested switch arm body should contribute to DU field bindings"
    );
}

#[test]
fn test_du_switch_bindings_collects_from_labeled_stmt() {
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape): number {
    switch (s.kind) {
        case "circle":
            block: {
                if (true) return s.radius;
            }
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "Labeled stmt body should contribute to DU field bindings"
    );
}

#[test]
fn test_du_switch_bindings_collects_from_assign_lhs() {
    let reg = build_shape_registry();
    // `s.radius = ...` — walker must recurse into AssignTarget::Member.
    let source = r#"
function f(s: Shape): number {
    switch (s.kind) {
        case "circle":
            s.radius = 0;
            return 1;
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "Assignment target (member LHS) should contribute to DU field bindings"
    );
}

#[test]
fn test_du_switch_bindings_skips_shadowed_for_of() {
    // I-148: `for (const s of arr)` inside a case body shadows the outer DU
    // variable `s`. The inner `s.radius` refers to the loop-local binding,
    // NOT the outer Shape. The walker must NOT collect `radius` here —
    // otherwise the outer match arm would bind `radius` and silent rename
    // the inner `s.radius` to the outer binding (Tier 1 silent semantic change).
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape, arr: Shape[]): void {
    switch (s.kind) {
        case "circle":
            for (const s of arr) {
                if (s.kind === "circle") console.log(s.radius);
            }
            return;
    }
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        !res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "radius must NOT be bound — inner `s.radius` refers to the loop variable, \
         not the outer Shape. got bindings: {:?}",
        res.du_field_bindings
    );
}

#[test]
fn test_du_switch_bindings_skips_shadowed_catch_param() {
    // I-148: `catch (s)` binds a new `s` for the handler body. Inner
    // `s.radius` (even though meaningless for Error param) must not be
    // collected as an outer-`s` field binding.
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape): number {
    switch (s.kind) {
        case "circle":
            try {
                return 0;
            } catch (s) {
                // `s` here is the caught error, not the outer Shape.
                // `(s as any).radius` is gibberish but the walker must not
                // treat it as a DU field access.
                return (s as any).radius;
            }
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        !res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "radius must NOT be bound when `s` is shadowed by catch param, \
         got: {:?}",
        res.du_field_bindings
    );
}

#[test]
fn test_du_switch_bindings_skips_shadowed_var_decl_in_block() {
    // I-148: `const s = ...` inside the case body shadows the outer DU
    // variable for all *subsequent* sibling statements in the same block.
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape): number {
    switch (s.kind) {
        case "circle": {
            const s = { radius: 99 };
            return s.radius;  // inner s, NOT outer Shape
        }
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        !res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "radius must NOT be bound after shadowing const decl, got: {:?}",
        res.du_field_bindings
    );
}

#[test]
fn test_du_switch_bindings_collects_before_shadowing_var_decl() {
    // I-148: `s.radius` BEFORE the shadowing decl is still the outer Shape.
    // Only stmts AFTER the decl are shadowed; the walker must still collect
    // radius from the pre-decl access.
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape): number {
    switch (s.kind) {
        case "circle": {
            const r = s.radius;  // outer s — MUST collect "radius"
            const s = { radius: 99 };  // shadow from here
            return s.radius;  // inner s — MUST skip
        }
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "radius (accessed before shadowing decl) must be bound, got: {:?}",
        res.du_field_bindings
    );
}

#[test]
fn test_du_switch_bindings_skips_shadowed_for_in() {
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape): number {
    switch (s.kind) {
        case "circle":
            for (const s in { a: 1 }) {
                return (s as any).radius;
            }
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        !res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "radius must NOT be bound when shadowed by for-in, got: {:?}",
        res.du_field_bindings
    );
}

#[test]
fn test_du_switch_bindings_skips_shadowed_c_style_for_init() {
    let reg = build_shape_registry();
    let source = r#"
function f(s: Shape): number {
    switch (s.kind) {
        case "circle":
            for (let s = 0; s < 3; s++) {
                return s;  // inner `s` (a number) — no field access
            }
    }
    return 0;
}
"#;
    let res = resolve_with_reg(source, &reg);
    assert!(
        !res.du_field_bindings.iter().any(|b| b.var_name == "radius"),
        "C-style for init must shadow the outer s inside test/update/body, got: {:?}",
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
