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

// --- Top-level expression statements (I-180) ---

#[test]
fn test_transform_module_top_level_expr_stmt_generates_init_fn() {
    // Top-level expression like `console.log("init")` → pub fn init() { ... }
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
    // console.log should be in init() function
    let init_fn = items
        .iter()
        .find(|i| matches!(i, Item::Fn { name, .. } if name == "init"));
    assert!(
        init_fn.is_some(),
        "expected init() function from top-level expression, got items: {items:?}"
    );
    assert!(
        unsupported.is_empty(),
        "expected no unsupported errors, got: {unsupported:?}"
    );
}

#[test]
fn test_transform_module_multiple_top_level_exprs_merge_into_single_init() {
    let source = r#"
        console.log("first");
        console.log("second");
    "#;
    let module = parse_typescript(source).expect("parse failed");
    let (items, _) = transform_module_collecting(&module, &TypeRegistry::new()).unwrap();
    let init_fns: Vec<_> = items
        .iter()
        .filter(|i| matches!(i, Item::Fn { name, .. } if name == "init"))
        .collect();
    assert_eq!(
        init_fns.len(),
        1,
        "expected exactly 1 init() function, got {}",
        init_fns.len()
    );
}

#[test]
fn test_transform_module_no_top_level_exprs_no_init_fn() {
    let source = "interface Foo { name: string; }";
    let module = parse_typescript(source).expect("parse failed");
    let (items, _) = transform_module_collecting(&module, &TypeRegistry::new()).unwrap();
    let has_init = items
        .iter()
        .any(|i| matches!(i, Item::Fn { name, .. } if name == "init"));
    assert!(
        !has_init,
        "no init() should be generated when no top-level expressions exist"
    );
}
