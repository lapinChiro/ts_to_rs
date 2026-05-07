use super::*;

#[test]
fn test_transform_module_empty() {
    let module = parse_typescript("").expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();
    assert!(items.is_empty());
}

#[test]
fn test_transform_module_non_exported_is_private() {
    let source = "interface Foo { name: string; }";
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Struct { vis, .. } => assert_eq!(*vis, Visibility::Private),
        _ => panic!("expected Struct"),
    }
}

#[test]
fn test_transform_module_exported_is_public() {
    let source = "export interface Foo { name: string; }";
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Struct { vis, .. } => assert_eq!(*vis, Visibility::Public),
        _ => panic!("expected Struct"),
    }
}

#[test]
fn test_transform_module_single_interface() {
    let source = "interface Foo { name: string; age: number; }";
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Struct {
            vis: Visibility::Private,
            name: "Foo".to_string(),
            type_params: vec![],
            fields: vec![
                StructField {
                    vis: None,
                    name: "name".to_string(),
                    ty: RustType::String,
                },
                StructField {
                    vis: None,
                    name: "age".to_string(),
                    ty: RustType::F64,
                },
            ],
            is_unit_struct: false,
        }
    );
}

#[test]
fn test_transform_module_multiple_interfaces() {
    let source = r#"
            interface Foo { name: string; }
            interface Bar { count: number; }
        "#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 2);
}

#[test]
fn test_transform_module_type_alias_object() {
    let source = "type Point = { x: number; y: number; };";
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Struct { name, .. } => assert_eq!(name, "Point"),
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_transform_module_const_literal_and_interface() {
    let source = r#"
            const x = 42;
            interface Foo { name: string; }
        "#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    // const x = 42 → Item::Const (P1.5, type inferred as f64), Foo → Item::Struct
    assert_eq!(items.len(), 2);
    assert!(
        matches!(&items[0], Item::Const { name, ty, .. } if name == "x" && *ty == RustType::F64)
    );
    assert!(matches!(&items[1], Item::Struct { name, .. } if name == "Foo"));
}

#[test]
fn test_transform_module_skips_string_const() {
    let source = r#"
            const msg: string = "hello";
            interface Bar { id: number; }
        "#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    // const msg: string = "hello" → skipped (String const not const-safe), Bar → Item::Struct
    assert_eq!(items.len(), 1);
    assert!(matches!(&items[0], Item::Struct { name, .. } if name == "Bar"));
}

#[test]
fn test_transform_module_function_declaration() {
    let source = "function add(a: number, b: number): number { return a + b; }";
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Fn {
            vis: Visibility::Private,
            attributes: vec![],
            is_async: false,
            name: "add".to_string(),
            type_params: vec![],
            params: vec![
                Param {
                    name: "a".to_string(),
                    ty: Some(RustType::F64),
                },
                Param {
                    name: "b".to_string(),
                    ty: Some(RustType::F64),
                },
            ],
            return_type: Some(RustType::F64),
            body: vec![Stmt::TailExpr(Expr::BinaryOp {
                left: Box::new(Expr::Ident("a".to_string())),
                op: BinOp::Add,
                right: Box::new(Expr::Ident("b".to_string())),
            })],
        }
    );
}

#[test]
fn test_transform_module_mixed_items() {
    let source = r#"
            interface Foo { name: string; }
            function greet(name: string): string { return name; }
        "#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 2);
    match &items[0] {
        Item::Struct { name, .. } => assert_eq!(name, "Foo"),
        _ => panic!("expected Item::Struct"),
    }
    match &items[1] {
        Item::Fn { name, .. } => assert_eq!(name, "greet"),
        _ => panic!("expected Item::Fn"),
    }
}

// --- Top-level expression statements (I-180 → I-224 fn main synthesis) ---

#[test]
fn test_transform_module_top_level_expr_stmt_synthesizes_fn_main() {
    // I-224 T4-1: Top-level expression like `console.log("init")` is captured
    // into the synthesized `fn main()` body (replaces the legacy
    // `pub fn init()` mechanism). The fn main is `Visibility::Private` (no
    // `pub`) per I-224 INV-5 — the binary entry convention does not require /
    // permit `pub fn main`.
    let source = r#"
        interface Foo { name: string; }
        console.log("init");
    "#;
    let module = parse_typescript(source).expect("parse failed");
    let (items, unsupported) = transform_module_collecting(&module, &TypeRegistry::new()).unwrap();
    // Foo should be converted
    assert!(items
        .iter()
        .any(|i| matches!(i, Item::Struct { name, .. } if name == "Foo")));
    // console.log should be in synthesized fn main() body
    let main_fn = items
        .iter()
        .find(|i| matches!(i, Item::Fn { name, .. } if name == "main"));
    assert!(
        main_fn.is_some(),
        "expected synthesized fn main from top-level expression, got items: {items:?}"
    );
    if let Some(Item::Fn {
        vis,
        attributes,
        is_async,
        body,
        ..
    }) = main_fn
    {
        assert!(
            matches!(vis, Visibility::Private),
            "synthesized fn main must be Private, got: {vis:?}"
        );
        assert!(
            attributes.is_empty(),
            "sync fn main has no attributes (no `#[tokio::main]`), got: {attributes:?}"
        );
        assert!(!is_async, "sync fn main must not be async");
        assert_eq!(
            body.len(),
            1,
            "expected 1 captured stmt, got {}",
            body.len()
        );
    }
    assert!(
        unsupported.is_empty(),
        "expected no unsupported errors, got: {unsupported:?}"
    );
    // INV-4 lock-in (= I-224 T4-1 production wiring contract): no legacy
    // `pub fn init()` Item is emitted any longer.
    assert!(
        !items
            .iter()
            .any(|i| matches!(i, Item::Fn { name, .. } if name == "init")),
        "legacy `pub fn init` mechanism must be retired (T4-1), got items: {items:?}"
    );
}

#[test]
fn test_transform_module_multiple_top_level_exprs_merge_into_single_fn_main() {
    // I-224 T4-1: multiple top-level Stmt::Expr statements merge into a single
    // synthesized fn main body, source order preserved (= INV-1).
    let source = r#"
        console.log("first");
        console.log("second");
    "#;
    let module = parse_typescript(source).expect("parse failed");
    let (items, _) = transform_module_collecting(&module, &TypeRegistry::new()).unwrap();
    let main_fns: Vec<_> = items
        .iter()
        .filter(|i| matches!(i, Item::Fn { name, .. } if name == "main"))
        .collect();
    assert_eq!(
        main_fns.len(),
        1,
        "expected exactly 1 synthesized fn main, got {}",
        main_fns.len()
    );
    if let Some(Item::Fn { body, .. }) = main_fns.first() {
        assert_eq!(
            body.len(),
            2,
            "expected 2 captured stmts in fn main body, got {}",
            body.len()
        );
    }
}

#[test]
fn test_transform_module_no_top_level_exprs_no_fn_main_synthesis() {
    // I-224 T4-1: library mode (declarations only, no executable triggers) →
    // no synthesized fn main (= LibraryNone dispatch arm). The user can use
    // the converted Rust as a library, with `cargo build --lib` style
    // consumption; no binary entry is emitted.
    let source = "interface Foo { name: string; }";
    let module = parse_typescript(source).expect("parse failed");
    let (items, _) = transform_module_collecting(&module, &TypeRegistry::new()).unwrap();
    let has_synthesized_main = items
        .iter()
        .any(|i| matches!(i, Item::Fn { name, .. } if name == "main"));
    assert!(
        !has_synthesized_main,
        "library-mode source must not produce synthesized fn main, got items: {items:?}"
    );
    let has_legacy_init = items
        .iter()
        .any(|i| matches!(i, Item::Fn { name, .. } if name == "init"));
    assert!(
        !has_legacy_init,
        "legacy `pub fn init` mechanism must be retired (T4-1), got items: {items:?}"
    );
}

// --- T4-1 capture-failure dispatch contract (try_capture Err branch) ---
//
// `Transformer::try_capture_module_item_into_main_stmts` returns `Err(_)` when
// an item is in capture scope (Stmt::Expr / Decl::Var FnMainBodyCapture /
// ExportDecl(Decl::Var) FnMainBodyCapture) but the inner `convert_expr` call
// fails (e.g., the captured init contains an unsupported expression form).
// The caller-side contract is split:
// - **Abort mode** (`transform_module`): propagate the first error verbatim so
//   the transpile result fails fast.
// - **Collecting mode** (`transform_module_collecting`): downcast the error to
//   `UnsupportedSyntaxError` and accumulate; the captured item is **not**
//   emitted via `transform_module_item` (= avoids duplicate / partial emission
//   when the captured init partially converted). Conversion of subsequent
//   items continues to maximise partial output.

#[test]
fn test_try_capture_err_branch_abort_mode_propagates_first_error() {
    // `class { method() {} }` is `Expr::Class` — a class **expression**
    // (anonymous class) which `convert_expr` currently does not support
    // (= falls through to the `_ => Err("unsupported expression")` arm).
    // Wrapping it in a top-level `Stmt::Expr` (`console.log(...)`) makes the
    // module enter executable mode (= `is_executable_mode = true`), routing
    // the Stmt::Expr through `try_capture_module_item_into_main_stmts`.
    // Inside the capture, `convert_expr` recurses into the call's argument,
    // hits the Class sub-expression, and returns `Err(_)`. In abort mode the
    // error must propagate as the function's `Err` return value.
    //
    // **Fixture choice rationale (T5-1 review fix 2026-05-08)**: previously
    // this test used `{a: 1} satisfies Foo` (= `TsSatisfies(Object)`), but
    // T5-1 added a `TsSatisfies` passthrough to `convert_expr` (passes
    // through to inner Object literal) so the satisfies form no longer
    // surfaces as the outer error. Using `Class` keeps the test future-
    // proof: Class expressions are a fundamental Rust-incompatible shape
    // (anonymous classes cannot be expressed as let-bindings without
    // synthetic name generation; left as Tier 2 honest reject for follow-up
    // PRD scope) and will continue to hit the `_ => Err` arm.
    let source = r#"
        console.log(class { method() {} });
    "#;
    let module = parse_typescript(source).expect("parse failed");
    let result = transform_module(&module, &TypeRegistry::new());
    let err = match result {
        Ok(items) => panic!(
            "expected Err from try_capture failure (abort mode), got Ok with items: {items:?}"
        ),
        Err(e) => format!("{e:#}"),
    };
    assert!(
        err.contains("Class") || err.to_lowercase().contains("class"),
        "expected error message to mention Class / class (= the unsupported \
         capture sub-expression), got: `{err}`"
    );
}

#[test]
fn test_try_capture_err_branch_collecting_mode_accumulates_and_skips_emission() {
    // Same source as the abort-mode test. Collecting mode must:
    //   (a) accumulate the convert_expr error into `unsupported`,
    //   (b) NOT emit the failed Stmt::Expr (= no `pub fn init` ghost from a
    //       legacy code path, no double-emission via transform_module_item).
    //   (c) continue conversion of unrelated items (the surrounding decl
    //       `function helper(): void {}` must still emit as `Item::Fn`).
    let source = r#"
        function helper(): void {}
        console.log(class { method() {} });
    "#;
    let module = parse_typescript(source).expect("parse failed");
    let (items, unsupported) = transform_module_collecting(&module, &TypeRegistry::new()).unwrap();

    // (a) Accumulator received the Class failure.
    assert!(
        unsupported
            .iter()
            .any(|u| u.kind.contains("Class") || u.kind.to_lowercase().contains("class")),
        "expected `Class` / `class` in accumulated unsupported list, got: {unsupported:?}"
    );

    // (b) No legacy `init` Item was synthesized despite the captured Stmt::Expr
    //     failing to convert (= regression guard against re-introducing the
    //     pre-T4-1 `init_stmts` partial-emit pattern).
    assert!(
        !items
            .iter()
            .any(|i| matches!(i, Item::Fn { name, .. } if name == "init")),
        "legacy `pub fn init` mechanism must stay retired even on capture failure, \
         got items: {items:?}"
    );

    // (c) The unrelated declaration (interface Foo) is still emitted as a
    //     struct item (= partial-output contract preserved).
    assert!(
        items
            .iter()
            .any(|i| matches!(i, Item::Fn { name, .. } if name == "helper")),
        "expected `helper` fn to still be emitted (partial-output contract), \
         got items: {items:?}"
    );
}
