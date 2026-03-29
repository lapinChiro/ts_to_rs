use super::*;

// --- TsParamProp (constructor parameter properties) ---

#[test]
fn test_param_prop_basic_public_generates_field_and_new() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl = parse_class_decl("class Foo { constructor(public x: number) {} }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Public,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

    // Struct should have field `x`
    match &items[0] {
        Item::Struct { fields, .. } => {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].name, "x");
            assert_eq!(fields[0].ty, RustType::F64);
            assert_eq!(fields[0].vis, Some(Visibility::Public));
        }
        _ => panic!("expected Item::Struct"),
    }

    // Impl should have `new(x: f64) -> Self`
    match &items[1] {
        Item::Impl { methods, .. } => {
            assert_eq!(methods.len(), 1);
            assert_eq!(methods[0].name, "new");
            assert_eq!(
                methods[0].params,
                vec![Param {
                    name: "x".to_string(),
                    ty: Some(RustType::F64),
                }]
            );
        }
        _ => panic!("expected Item::Impl"),
    }
}

#[test]
fn test_param_prop_private_generates_private_field() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl = parse_class_decl("class Foo { constructor(private x: number) {} }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Public,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

    match &items[0] {
        Item::Struct { fields, .. } => {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].name, "x");
            assert_eq!(fields[0].vis, Some(Visibility::Private));
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_param_prop_readonly_generates_field() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl = parse_class_decl("class Foo { constructor(public readonly x: string) {} }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Public,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

    match &items[0] {
        Item::Struct { fields, .. } => {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].name, "x");
            assert_eq!(fields[0].ty, RustType::String);
            assert_eq!(fields[0].vis, Some(Visibility::Public));
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_param_prop_with_default_value_generates_field_and_param() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl = parse_class_decl("class Foo { constructor(public x: number = 42) {} }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Public,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

    match &items[0] {
        Item::Struct { fields, .. } => {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].name, "x");
            assert_eq!(fields[0].ty, RustType::F64);
        }
        _ => panic!("expected Item::Struct"),
    }

    match &items[1] {
        Item::Impl { methods, .. } => {
            assert_eq!(methods[0].name, "new");
            assert_eq!(methods[0].params.len(), 1);
            assert_eq!(methods[0].params[0].name, "x");
        }
        _ => panic!("expected Item::Impl"),
    }
}

#[test]
fn test_param_prop_mixed_with_regular_param() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl = parse_class_decl(
        "class Foo { constructor(public x: number, y: string) { console.log(y); } }",
    );
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Public,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

    // Struct should only have field `x` (not `y`)
    match &items[0] {
        Item::Struct { fields, .. } => {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].name, "x");
        }
        _ => panic!("expected Item::Struct"),
    }

    // new() should have both params
    match &items[1] {
        Item::Impl { methods, .. } => {
            assert_eq!(methods[0].params.len(), 2);
            assert_eq!(methods[0].params[0].name, "x");
            assert_eq!(methods[0].params[1].name, "y");
        }
        _ => panic!("expected Item::Impl"),
    }
}

#[test]
fn test_param_prop_multiple_generates_multiple_fields() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl =
        parse_class_decl("class Foo { constructor(public x: number, private y: string) {} }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Public,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

    match &items[0] {
        Item::Struct { fields, .. } => {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "x");
            assert_eq!(fields[0].vis, Some(Visibility::Public));
            assert_eq!(fields[1].name, "y");
            assert_eq!(fields[1].vis, Some(Visibility::Private));
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_param_prop_with_existing_this_assignment() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl = parse_class_decl(
        "class Foo { z: boolean; constructor(public x: number) { this.z = true; } }",
    );
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Public,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

    // Struct should have both `z` (explicit) and `x` (param prop)
    match &items[0] {
        Item::Struct { fields, .. } => {
            assert_eq!(fields.len(), 2);
            let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
            assert!(names.contains(&"x"));
            assert!(names.contains(&"z"));
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_param_prop_with_body_logic_preserves_statements() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl = parse_class_decl("class Foo { constructor(public x: number) { console.log(x); } }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Public,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

    match &items[1] {
        Item::Impl { methods, .. } => {
            let body = methods[0].body.as_ref().unwrap();
            // Should have both the console.log and the Self init
            assert!(
                body.len() >= 2,
                "body should have logic + Self init, got {:?}",
                body
            );
        }
        _ => panic!("expected Item::Impl"),
    }
}

#[test]
fn test_convert_class_constructor_default_number_param() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // constructor(x: number = 0) should produce Option<f64> param + unwrap_or
    let decl =
        parse_class_decl("class Foo { x: number; constructor(x: number = 0) { this.x = x; } }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Private,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

    // Find the Impl item
    let impl_item = items.iter().find(|i| matches!(i, Item::Impl { .. }));
    assert!(impl_item.is_some(), "expected Impl item");

    match impl_item.unwrap() {
        Item::Impl { methods, .. } => {
            let new_method = methods.iter().find(|m| m.name == "new");
            assert!(new_method.is_some(), "expected 'new' method");
            let method = new_method.unwrap();
            // Parameter should be Option<f64>
            assert_eq!(method.params.len(), 1);
            assert_eq!(method.params[0].name, "x");
            assert_eq!(
                method.params[0].ty,
                Some(RustType::Option(Box::new(RustType::F64)))
            );
            // Body should contain unwrap_or expansion as first statement
            assert!(
                method.body.as_ref().unwrap().len() >= 2,
                "expected unwrap_or expansion + Self init, got {:?}",
                method.body.as_ref().unwrap()
            );
        }
        _ => panic!("expected Item::Impl"),
    }
}

#[test]
fn test_convert_class_constructor_default_empty_object_param() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // constructor(options: Options = {}) should produce Option<Options> + unwrap_or_default
    let decl = parse_class_decl("class Foo { constructor(options: Options = {}) {} }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Private,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

    let impl_item = items.iter().find(|i| matches!(i, Item::Impl { .. }));
    assert!(impl_item.is_some(), "expected Impl item");

    match impl_item.unwrap() {
        Item::Impl { methods, .. } => {
            let new_method = methods.iter().find(|m| m.name == "new");
            assert!(new_method.is_some(), "expected 'new' method");
            let method = new_method.unwrap();
            assert_eq!(method.params.len(), 1);
            assert_eq!(method.params[0].name, "options");
            assert_eq!(
                method.params[0].ty,
                Some(RustType::Option(Box::new(RustType::Named {
                    name: "Options".to_string(),
                    type_args: vec![],
                })))
            );
        }
        _ => panic!("expected Item::Impl"),
    }
}
