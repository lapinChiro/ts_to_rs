use super::*;

#[test]
fn test_convert_class_properties_only() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl = parse_class_decl("class Foo { x: number; y: string; }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Private,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Struct {
            vis: Visibility::Private,
            name: "Foo".to_string(),
            type_params: vec![],
            fields: vec![
                StructField {
                    vis: Some(Visibility::Private),
                    name: "x".to_string(),
                    ty: RustType::F64,
                },
                StructField {
                    vis: Some(Visibility::Private),
                    name: "y".to_string(),
                    ty: RustType::String,
                },
            ],
        }
    );
}

#[test]
fn test_convert_class_constructor() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl = parse_class_decl("class Foo { x: number; constructor(x: number) { this.x = x; } }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Private,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

    assert_eq!(items.len(), 2);
    match &items[1] {
        Item::Impl {
            struct_name,
            methods,
            ..
        } => {
            assert_eq!(struct_name, "Foo");
            assert_eq!(methods.len(), 1);
            assert_eq!(methods[0].name, "new");
            assert!(!methods[0].has_self);
            assert_eq!(
                methods[0].return_type,
                Some(RustType::Named {
                    name: "Self".to_string(),
                    type_args: vec![]
                })
            );
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
fn test_convert_class_method_with_self() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl =
        parse_class_decl("class Foo { name: string; greet(): string { return this.name; } }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Private,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

    assert_eq!(items.len(), 2);
    match &items[1] {
        Item::Impl { methods, .. } => {
            assert_eq!(methods.len(), 1);
            assert_eq!(methods[0].name, "greet");
            assert!(methods[0].has_self);
            assert_eq!(methods[0].return_type, Some(RustType::String));
        }
        _ => panic!("expected Item::Impl"),
    }
}

#[test]
fn test_convert_class_export_visibility() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl = parse_class_decl("class Foo { x: number; greet(): string { return this.x; } }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Public,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

    match &items[0] {
        Item::Struct { vis, .. } => assert_eq!(*vis, Visibility::Public),
        _ => panic!("expected Struct"),
    }
    match &items[1] {
        Item::Impl { methods, .. } => {
            assert_eq!(methods[0].vis, Visibility::Public);
        }
        _ => panic!("expected Impl"),
    }
}

#[test]
fn test_convert_class_static_method_has_no_self() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl = parse_class_decl("class Foo { x: number; static bar(): number { return 1; } }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Private,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

    assert_eq!(items.len(), 2);
    match &items[1] {
        Item::Impl { methods, .. } => {
            assert_eq!(methods.len(), 1);
            assert_eq!(methods[0].name, "bar");
            assert!(
                !methods[0].has_self,
                "static method should not have self, got has_self=true"
            );
            assert!(
                !methods[0].has_mut_self,
                "static method should not have mut self"
            );
            assert_eq!(methods[0].return_type, Some(RustType::F64));
        }
        _ => panic!("expected Item::Impl"),
    }
}

#[test]
fn test_extract_class_info_implements_single() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl =
        parse_class_decl("class Foo implements Greeter { greet(): string { return 'hi'; } }");
    let info = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .extract_class_info(&decl, Visibility::Private)
        .unwrap();

    let impl_names: Vec<&str> = info.implements.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(impl_names, vec!["Greeter"]);
}

#[test]
fn test_extract_class_info_implements_multiple() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl = parse_class_decl("class Foo implements A, B { foo(): void {} bar(): void {} }");
    let info = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .extract_class_info(&decl, Visibility::Private)
        .unwrap();

    let impl_names: Vec<&str> = info.implements.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(impl_names, vec!["A", "B"]);
}

#[test]
fn test_extract_class_info_no_implements() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl = parse_class_decl("class Foo { x: number; }");
    let info = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .extract_class_info(&decl, Visibility::Private)
        .unwrap();

    assert!(info.implements.is_empty());
}

#[test]
fn test_convert_class_this_to_self() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl = parse_class_decl(
        "class Foo { name: string; constructor(name: string) { this.name = name; } }",
    );
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Private,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

    match &items[1] {
        Item::Impl { methods, .. } => {
            // Constructor body should contain `self.name = name`
            // which would be an Expr statement with assignment
            assert!(methods[0].body.as_ref().is_some_and(|b| !b.is_empty()));
        }
        _ => panic!("expected Impl"),
    }
}

#[test]
fn test_extract_class_info_abstract_flag_is_true() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl = parse_class_decl("abstract class Shape { abstract area(): number; }");
    let info = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .extract_class_info(&decl, Visibility::Private)
        .unwrap();
    assert!(info.is_abstract);
}

#[test]
fn test_convert_abstract_class_abstract_only_generates_trait() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl = parse_class_decl("abstract class Shape { abstract area(): number; }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Public,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();
    // Should produce a single Trait item, not Struct + Impl
    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Trait {
            vis, name, methods, ..
        } => {
            assert_eq!(*vis, Visibility::Public);
            assert_eq!(name, "Shape");
            assert_eq!(methods.len(), 1);
            assert_eq!(methods[0].name, "area");
            assert!(
                methods[0].body.is_none(),
                "abstract method should have no body"
            );
            assert_eq!(methods[0].return_type, Some(RustType::F64));
        }
        _ => panic!("expected Item::Trait, got {:?}", items[0]),
    }
}

#[test]
fn test_convert_abstract_class_mixed_generates_trait_with_defaults() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl = parse_class_decl(
        "abstract class Shape { abstract area(): number; describe(): string { return \"shape\"; } }",
    );
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Public,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();
    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Trait { methods, .. } => {
            assert_eq!(methods.len(), 2);
            // abstract method: no body
            assert_eq!(methods[0].name, "area");
            assert!(methods[0].body.is_none());
            // concrete method: has body (default impl)
            assert_eq!(methods[1].name, "describe");
            assert!(methods[1].body.as_ref().is_some_and(|b| !b.is_empty()));
        }
        _ => panic!("expected Item::Trait, got {:?}", items[0]),
    }
}

#[test]
fn test_convert_abstract_class_concrete_only_generates_trait_with_defaults() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl = parse_class_decl("abstract class Foo { bar(): number { return 1; } }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Public,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();
    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Trait { methods, .. } => {
            assert_eq!(methods.len(), 1);
            assert_eq!(methods[0].name, "bar");
            assert!(methods[0].body.as_ref().is_some_and(|b| !b.is_empty()));
        }
        _ => panic!("expected Item::Trait, got {:?}", items[0]),
    }
}

#[test]
fn test_convert_class_static_prop_generates_assoc_const() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl =
        parse_class_decl("class Config { static readonly MAX_SIZE: number = 100; value: number; }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Public,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();
    // Should have: Struct (only value field) + Impl (with const MAX_SIZE)
    match &items[0] {
        Item::Struct { fields, .. } => {
            assert_eq!(
                fields.len(),
                1,
                "static prop should not be in struct fields"
            );
            assert_eq!(fields[0].name, "value");
        }
        _ => panic!("expected Item::Struct, got {:?}", items[0]),
    }
    match &items[1] {
        Item::Impl { consts, .. } => {
            assert_eq!(consts.len(), 1);
            assert_eq!(consts[0].name, "MAX_SIZE");
            assert_eq!(consts[0].ty, RustType::F64);
            assert_eq!(consts[0].value, crate::ir::Expr::NumberLit(100.0));
        }
        _ => panic!("expected Item::Impl, got {:?}", items[1]),
    }
}

#[test]
fn test_convert_class_static_string_prop_generates_assoc_const() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl = parse_class_decl("class Foo { static NAME: string = \"hello\"; x: number; }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Public,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();
    match &items[1] {
        Item::Impl { consts, .. } => {
            assert_eq!(consts.len(), 1);
            assert_eq!(consts[0].name, "NAME");
            assert_eq!(consts[0].ty, RustType::String);
        }
        _ => panic!("expected Item::Impl, got {:?}", items[1]),
    }
}

#[test]
fn test_convert_class_protected_method_generates_pub_crate() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl = parse_class_decl("class Foo { protected greet(): string { return 'hi'; } }");
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
            assert_eq!(methods.len(), 1);
            assert_eq!(methods[0].name, "greet");
            assert_eq!(methods[0].vis, Visibility::PubCrate);
        }
        _ => panic!("expected Item::Impl"),
    }
}

#[test]
fn test_convert_class_protected_property_generates_pub_crate_field() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let decl = parse_class_decl("class Foo { protected x: number; }");
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Public,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();
    // Verify via generator output since StructField doesn't have vis yet
    let output = crate::generator::generate(&items);
    assert!(output.contains("pub(crate) x: f64"));
}
