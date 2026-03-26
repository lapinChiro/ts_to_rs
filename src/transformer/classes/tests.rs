use std::collections::HashMap;

use super::*;
use crate::ir::{Item, Param, RustType, StructField, Visibility};
use crate::parser::parse_typescript;
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::TypeRegistry;
use crate::transformer::test_fixtures::TctxFixture;
use crate::transformer::Transformer;
use swc_ecma_ast::{Decl, ModuleItem};

use super::inheritance::rewrite_super_constructor;

/// Helper: parse TS source and extract the first ClassDecl.
fn parse_class_decl(source: &str) -> ast::ClassDecl {
    let module = parse_typescript(source).expect("parse failed");
    match &module.body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Class(decl))) => decl.clone(),
        _ => panic!("expected ClassDecl"),
    }
}

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
fn test_rewrite_super_constructor_arg_count_mismatch_returns_error() {
    use crate::ir::{Expr, Method, Stmt};

    // Parent has 2 fields but child's super() only passes 1 arg
    let parent_info = ClassInfo {
        name: "Parent".to_string(),
        type_params: vec![],
        parent: None,
        parent_type_args: vec![],
        fields: vec![
            StructField {
                vis: None,
                name: "a".to_string(),
                ty: RustType::F64,
            },
            StructField {
                vis: None,
                name: "b".to_string(),
                ty: RustType::String,
            },
        ],
        constructor: None,
        methods: vec![],
        vis: Visibility::Private,
        implements: vec![],
        is_abstract: false,
        static_consts: vec![],
    };

    let child_ctor = Method {
        vis: Visibility::Public,
        name: "new".to_string(),
        has_self: false,
        has_mut_self: false,
        params: vec![Param {
            name: "x".to_string(),
            ty: Some(RustType::F64),
        }],
        return_type: Some(RustType::Named {
            name: "Self".to_string(),
            type_args: vec![],
        }),
        body: Some(vec![Stmt::Expr(Expr::FnCall {
            name: "super".to_string(),
            args: vec![Expr::Ident("x".to_string())], // only 1 arg, parent has 2 fields
        })]),
    };

    let result = rewrite_super_constructor(&child_ctor, &parent_info);
    assert!(
        result.is_err(),
        "expected error for arg count mismatch, got: {:?}",
        result
    );
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

// --- Expected type propagation ---

/// Step 7: Static property initializer should propagate type annotation.
/// `static config: Config = { name: "default" }` should produce StructInit, not error.
#[test]
fn test_convert_static_prop_propagates_type_annotation() {
    let mut reg = TypeRegistry::new();
    reg.register(
        "Config".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![("name".to_string(), RustType::String)],
            std::collections::HashMap::new(),
            vec![],
        ),
    );

    let source = r#"class Foo { static config: Config = { name: "default" }; }"#;
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();

    let decl = match &f.module().body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Class(decl))) => decl.clone(),
        _ => panic!("expected ClassDecl"),
    };
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Private,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

    // Find the Impl item with static consts
    let impl_item = items
        .iter()
        .find(|item| matches!(item, Item::Impl { .. }))
        .expect("expected Item::Impl");

    match impl_item {
        Item::Impl { consts, .. } => {
            assert_eq!(consts.len(), 1);
            assert_eq!(consts[0].name, "config");
            match &consts[0].value {
                crate::ir::Expr::StructInit { name, fields, .. } => {
                    assert_eq!(name, "Config");
                    assert_eq!(fields[0].0, "name");
                    assert!(
                        matches!(&fields[0].1, crate::ir::Expr::MethodCall { method, .. } if method == "to_string"),
                        "expected .to_string() on string field, got {:?}",
                        fields[0].1
                    );
                }
                other => panic!("expected StructInit, got {other:?}"),
            }
        }
        _ => unreachable!(),
    }
}
