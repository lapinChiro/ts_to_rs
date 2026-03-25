use super::*;
use crate::ir::Stmt;
use crate::ir::{BinOp, Expr, Param, RustType, StructField, Visibility};
use crate::parser::parse_typescript;
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::TypeRegistry;
use crate::transformer::test_fixtures::TctxFixture;
use crate::transformer::Transformer;
#[test]
fn test_transform_module_empty() {
    let module = parse_typescript("").expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();
    assert!(items.is_empty());
}

#[test]
fn test_transform_module_import_single() {
    let source = r#"import { Foo } from "./bar";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Private,
            path: "crate::bar".to_string(),
            names: vec!["Foo".to_string()],
        }
    );
}

#[test]
fn test_transform_module_import_multiple() {
    let source = r#"import { A, B } from "./bar";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Private,
            path: "crate::bar".to_string(),
            names: vec!["A".to_string(), "B".to_string()],
        }
    );
}

#[test]
fn test_transform_module_import_nested_path() {
    let source = r#"import { Foo } from "./sub/bar";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Private,
            path: "crate::sub::bar".to_string(),
            names: vec!["Foo".to_string()],
        }
    );
}

#[test]
fn test_transform_module_import_hyphen_to_underscore() {
    let source = r#"import { Foo } from "./hono-base";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Private,
            path: "crate::hono_base".to_string(),
            names: vec!["Foo".to_string()],
        }
    );
}

#[test]
fn test_transform_module_import_nested_hyphen_path() {
    let source = r#"import { StatusCode } from "./utils/http-status";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Private,
            path: "crate::utils::http_status".to_string(),
            names: vec!["StatusCode".to_string()],
        }
    );
}

#[test]
fn test_transform_module_import_multiple_hyphens() {
    let source = r#"import { Foo } from "./my-long-name";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Private,
            path: "crate::my_long_name".to_string(),
            names: vec!["Foo".to_string()],
        }
    );
}

#[test]
fn test_transform_module_export_named_reexport_single() {
    let source = r#"export { Foo } from "./bar";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Public,
            path: "crate::bar".to_string(),
            names: vec!["Foo".to_string()],
        }
    );
}

#[test]
fn test_transform_module_export_named_reexport_multiple() {
    let source = r#"export { Foo, Bar } from "./baz";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Public,
            path: "crate::baz".to_string(),
            names: vec!["Foo".to_string(), "Bar".to_string()],
        }
    );
}

#[test]
fn test_transform_module_export_named_local_skipped() {
    let source = r#"
interface Foo { name: string; }
export { Foo };
"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    // Only the interface should be converted; the export { Foo } should be skipped
    assert_eq!(items.len(), 1);
    assert!(matches!(&items[0], Item::Struct { name, .. } if name == "Foo"));
}

#[test]
fn test_transform_module_import_external_skipped() {
    let source = r#"import { Foo } from "lodash";"#;
    let module = parse_typescript(source).expect("parse failed");
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
fn test_transform_module_skips_unsupported() {
    let source = r#"
            const x = 42;
            interface Foo { name: string; }
        "#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    // const x = 42 is skipped, only Foo is converted
    assert_eq!(items.len(), 1);
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

#[test]
fn test_transform_enum_numeric_auto_values() {
    let source = "enum Color { Red, Green, Blue }";
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Enum {
            vis,
            name,
            variants,
            ..
        } => {
            assert_eq!(*vis, Visibility::Private);
            assert_eq!(name, "Color");
            assert_eq!(variants.len(), 3);
            assert_eq!(variants[0].name, "Red");
            assert_eq!(variants[0].value, None);
            assert_eq!(variants[1].name, "Green");
            assert_eq!(variants[2].name, "Blue");
        }
        _ => panic!("expected Enum"),
    }
}

#[test]
fn test_transform_enum_numeric_explicit_values() {
    let source = "enum Status { Active = 1, Inactive = 0 }";
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Enum { variants, .. } => {
            assert_eq!(variants[0].name, "Active");
            assert_eq!(variants[0].value, Some(crate::ir::EnumValue::Number(1)));
            assert_eq!(variants[1].name, "Inactive");
            assert_eq!(variants[1].value, Some(crate::ir::EnumValue::Number(0)));
        }
        _ => panic!("expected Enum"),
    }
}

#[test]
fn test_transform_enum_string_values() {
    let source = r#"enum Direction { Up = "UP", Down = "DOWN" }"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Enum { variants, .. } => {
            assert_eq!(variants[0].name, "Up");
            assert_eq!(
                variants[0].value,
                Some(crate::ir::EnumValue::Str("UP".to_string()))
            );
            assert_eq!(variants[1].name, "Down");
            assert_eq!(
                variants[1].value,
                Some(crate::ir::EnumValue::Str("DOWN".to_string()))
            );
        }
        _ => panic!("expected Enum"),
    }
}

#[test]
fn test_transform_enum_export_is_public() {
    let source = "export enum Color { Red, Green }";
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Enum { vis, .. } => assert_eq!(*vis, Visibility::Public),
        _ => panic!("expected Enum"),
    }
}

#[test]
fn test_transform_enum_empty() {
    let source = "enum Empty { }";
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Enum { variants, .. } => assert!(variants.is_empty()),
        _ => panic!("expected Enum"),
    }
}

#[test]
fn test_transform_enum_single_member() {
    let source = "enum Single { Only = -1 }";
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Enum { variants, .. } => {
            assert_eq!(variants.len(), 1);
            assert_eq!(variants[0].name, "Only");
            assert_eq!(variants[0].value, Some(crate::ir::EnumValue::Number(-1)));
        }
        _ => panic!("expected Enum"),
    }
}

#[test]
fn test_transform_module_class_implements_single_interface() {
    let source = r#"
interface Greeter { greet(): string; }
class Foo implements Greeter { greet(): string { return "hi"; } }
"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    // Expect: Trait(Greeter) + Struct(Foo) + Impl(Foo for Greeter)
    let has_trait = items
        .iter()
        .any(|i| matches!(i, Item::Trait { name, .. } if name == "Greeter"));
    assert!(has_trait, "should have Greeter trait, got: {items:?}");

    let has_struct = items
        .iter()
        .any(|i| matches!(i, Item::Struct { name, .. } if name == "Foo"));
    assert!(has_struct, "should have Foo struct, got: {items:?}");

    let has_trait_impl = items.iter().any(|i| {
        matches!(
            i,
            Item::Impl {
                struct_name,
                for_trait: Some(trait_name),
                ..
            } if struct_name == "Foo" && trait_name.name == "Greeter"
        )
    });
    assert!(
        has_trait_impl,
        "should have impl Greeter for Foo, got: {items:?}"
    );
}

#[test]
fn test_transform_module_class_implements_multiple_interfaces() {
    let source = r#"
interface A { foo(): void; }
interface B { bar(): void; }
class Foo implements A, B {
    foo(): void {}
    bar(): void {}
}
"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    let has_impl_a = items.iter().any(|i| {
        matches!(
            i,
            Item::Impl {
                struct_name,
                for_trait: Some(trait_name),
                ..
            } if struct_name == "Foo" && trait_name.name == "A"
        )
    });
    assert!(has_impl_a, "should have impl A for Foo, got: {items:?}");

    let has_impl_b = items.iter().any(|i| {
        matches!(
            i,
            Item::Impl {
                struct_name,
                for_trait: Some(trait_name),
                ..
            } if struct_name == "Foo" && trait_name.name == "B"
        )
    });
    assert!(has_impl_b, "should have impl B for Foo, got: {items:?}");
}

#[test]
fn test_transform_module_class_implements_with_own_methods() {
    let source = r#"
interface Greeter { greet(): string; }
class Foo implements Greeter {
    greet(): string { return "hi"; }
    helper(): void {}
}
"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    // greet should be in impl Greeter for Foo
    let trait_impl = items.iter().find(|i| {
        matches!(
            i,
            Item::Impl {
                for_trait: Some(t),
                ..
            } if t.name == "Greeter"
        )
    });
    assert!(trait_impl.is_some(), "should have impl Greeter for Foo");
    if let Some(Item::Impl { methods, .. }) = trait_impl {
        assert!(
            methods.iter().any(|m| m.name == "greet"),
            "trait impl should contain greet"
        );
        assert!(
            !methods.iter().any(|m| m.name == "helper"),
            "trait impl should NOT contain helper"
        );
    }

    // helper should be in impl Foo
    let own_impl = items.iter().find(|i| {
        matches!(
            i,
            Item::Impl {
                for_trait: None,
                struct_name,
                ..
            } if struct_name == "Foo"
        )
    });
    assert!(own_impl.is_some(), "should have impl Foo");
    if let Some(Item::Impl { methods, .. }) = own_impl {
        assert!(
            methods.iter().any(|m| m.name == "helper"),
            "own impl should contain helper"
        );
        assert!(
            !methods.iter().any(|m| m.name == "greet"),
            "own impl should NOT contain greet"
        );
    }
}

#[test]
fn test_transform_module_class_extends_and_implements() {
    let source = r#"
interface Greeter { greet(): string; }
class Parent {
    name: string;
    getName(): string { return this.name; }
}
class Child extends Parent implements Greeter {
    age: number;
    greet(): string { return this.name; }
    helper(): void {}
}
"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    // Child struct should exist with parent + child fields
    let child_struct = items
        .iter()
        .find(|i| matches!(i, Item::Struct { name, .. } if name == "Child"));
    assert!(
        child_struct.is_some(),
        "should have Child struct, got: {items:?}"
    );
    if let Some(Item::Struct { fields, .. }) = child_struct {
        let field_names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
        assert!(
            field_names.contains(&"name"),
            "should have parent field 'name'"
        );
        assert!(
            field_names.contains(&"age"),
            "should have child field 'age'"
        );
    }

    // impl ParentTrait for Child
    let has_parent_trait_impl = items.iter().any(|i| {
        matches!(
            i,
            Item::Impl {
                struct_name,
                for_trait: Some(trait_name),
                ..
            } if struct_name == "Child" && trait_name.name == "ParentTrait"
        )
    });
    assert!(
        has_parent_trait_impl,
        "should have impl ParentTrait for Child, got: {items:?}"
    );

    // impl Greeter for Child
    let has_greeter_impl = items.iter().any(|i| {
        matches!(
            i,
            Item::Impl {
                struct_name,
                for_trait: Some(trait_name),
                ..
            } if struct_name == "Child" && trait_name.name == "Greeter"
        )
    });
    assert!(
        has_greeter_impl,
        "should have impl Greeter for Child, got: {items:?}"
    );

    // impl Child (own methods not in any trait)
    let own_impl = items.iter().find(|i| {
        matches!(
            i,
            Item::Impl {
                struct_name,
                for_trait: None,
                ..
            } if struct_name == "Child"
        )
    });
    assert!(own_impl.is_some(), "should have impl Child");
    if let Some(Item::Impl { methods, .. }) = own_impl {
        assert!(
            methods.iter().any(|m| m.name == "helper"),
            "own impl should contain helper"
        );
        assert!(
            !methods.iter().any(|m| m.name == "greet"),
            "own impl should NOT contain greet (it belongs to impl Greeter)"
        );
    }
}

#[test]
fn test_transform_enum_computed_member_bitshift() {
    let source = "enum Flags { Read = 1 << 0, Write = 1 << 1 }";
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Enum { variants, .. } => {
            assert_eq!(variants[0].name, "Read");
            assert_eq!(
                variants[0].value,
                Some(crate::ir::EnumValue::Expr("1 << 0".to_string()))
            );
            assert_eq!(variants[1].name, "Write");
            assert_eq!(
                variants[1].value,
                Some(crate::ir::EnumValue::Expr("1 << 1".to_string()))
            );
        }
        _ => panic!("expected Enum"),
    }
}

// ---- export * ----

#[test]
fn test_transform_module_export_all_relative() {
    let source = r#"export * from "./utils";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Public,
            path: "crate::utils".to_string(),
            names: vec!["*".to_string()],
        }
    );
}

#[test]
fn test_transform_module_export_all_external_skipped() {
    let source = r#"export * from "some-package";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();
    assert!(items.is_empty());
}

// ---- variable type annotation propagation to arrow return type ----

#[test]
fn test_transform_var_type_arrow_propagates_return_type() {
    let source = r#"
        interface Point { x: number; y: number; }
        export const make: (n: number) => Point = (n: number) => {
            return { x: n, y: 0 };
        };
    "#;
    let f = TctxFixture::from_source(source);
    let (items, _) = f.transform(source);

    let fn_item = items
        .iter()
        .find(|i| matches!(i, Item::Fn { name, .. } if name == "make"));
    assert!(fn_item.is_some(), "expected fn make, got: {items:?}");
    match fn_item.unwrap() {
        Item::Fn { return_type, .. } => {
            assert_eq!(
                *return_type,
                Some(RustType::Named {
                    name: "Point".to_string(),
                    type_args: vec![],
                })
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn test_transform_var_type_alias_arrow_propagates_return_type() {
    let source = r#"
        interface Info { name: string; }
        type GetInfo = (key: string) => Info;
        export const getInfo: GetInfo = (key: string) => {
            return { name: key };
        };
    "#;
    let f = TctxFixture::from_source(source);
    let (items, _) = f.transform(source);

    let fn_item = items
        .iter()
        .find(|i| matches!(i, Item::Fn { name, .. } if name == "getInfo"));
    assert!(fn_item.is_some(), "expected fn getInfo");
    match fn_item.unwrap() {
        Item::Fn { return_type, .. } => {
            assert_eq!(
                *return_type,
                Some(RustType::Named {
                    name: "Info".to_string(),
                    type_args: vec![],
                })
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn test_transform_var_arrow_explicit_return_type_takes_priority() {
    let source = r#"
        const f: (x: number) => string = (x: number): number => {
            return x;
        };
    "#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    match &items[0] {
        Item::Fn { return_type, .. } => {
            assert_eq!(*return_type, Some(RustType::F64));
        }
        _ => panic!("expected Item::Fn"),
    }
}

// ---- param type inference from variable annotation ----

#[test]
fn test_transform_var_arrow_infers_param_types_from_variable_annotation() {
    // const f: (x: number, y: string) => void = (x, y) => { ... }
    // → fn f(x: f64, y: String) { ... }
    let source = r#"
        const f: (x: number, y: string) => void = (x, y) => {
            console.log(x);
        };
    "#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    match &items[0] {
        Item::Fn { params, .. } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "x");
            assert_eq!(params[0].ty, Some(RustType::F64));
            assert_eq!(params[1].name, "y");
            assert_eq!(params[1].ty, Some(RustType::String));
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_transform_var_arrow_infers_param_types_from_named_type_alias() {
    // type Handler = (c: Context) => ConnInfo
    // const getInfo: Handler = (c) => { ... }
    // → fn getInfo(c: Context) -> ConnInfo { ... }
    let source = r#"
        type Handler = (c: string) => number;
        const getInfo: Handler = (c) => {
            return 0;
        };
    "#;
    let module = parse_typescript(source).expect("parse failed");
    let reg = crate::registry::build_registry(&module);
    let items = transform_module(&module, &reg).unwrap();

    let fn_item = items
        .iter()
        .find(|i| matches!(i, Item::Fn { name, .. } if name == "getInfo"));
    assert!(fn_item.is_some(), "expected fn getInfo");
    match fn_item.unwrap() {
        Item::Fn { params, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "c");
            assert_eq!(params[0].ty, Some(RustType::String));
        }
        _ => unreachable!(),
    }
}

#[test]
fn test_transform_var_arrow_explicit_param_type_not_overridden() {
    // Explicit param annotation should NOT be overridden by variable type
    let source = r#"
        const f: (x: number) => void = (x: string) => {
            console.log(x);
        };
    "#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    match &items[0] {
        Item::Fn { params, .. } => {
            assert_eq!(params[0].ty, Some(RustType::String)); // explicit wins
        }
        _ => panic!("expected Item::Fn"),
    }
}

// ---- extract_fn_return_type tests ----

#[test]
fn test_extract_fn_return_type_from_fn_type() {
    let ty = RustType::Fn {
        params: vec![],
        return_type: Box::new(RustType::String),
    };
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let t = Transformer::for_module(&tctx, &mut synthetic);
    let result = t.extract_fn_return_type(&ty);
    assert_eq!(result, Some(RustType::String));
}

#[test]
fn test_extract_fn_return_type_from_named_type_in_registry() {
    let mut reg = TypeRegistry::new();
    reg.register(
        "GetInfo".to_string(),
        crate::registry::TypeDef::Function {
            params: vec![],
            return_type: Some(RustType::Named {
                name: "Info".to_string(),
                type_args: vec![],
            }),
            has_rest: false,
        },
    );
    let ty = RustType::Named {
        name: "GetInfo".to_string(),
        type_args: vec![],
    };
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let t = Transformer::for_module(&tctx, &mut synthetic);
    let result = t.extract_fn_return_type(&ty);
    assert_eq!(
        result,
        Some(RustType::Named {
            name: "Info".to_string(),
            type_args: vec![],
        })
    );
}

#[test]
fn test_extract_fn_return_type_unknown_returns_none() {
    let ty = RustType::String;
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let t = Transformer::for_module(&tctx, &mut synthetic);
    let result = t.extract_fn_return_type(&ty);
    assert_eq!(result, None);
}

// ---- extract_fn_param_types tests ----

#[test]
fn test_extract_fn_param_types_from_fn_type() {
    let ty = RustType::Fn {
        params: vec![RustType::F64, RustType::String],
        return_type: Box::new(RustType::Unit),
    };
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let t = Transformer::for_module(&tctx, &mut synthetic);
    let result = t.extract_fn_param_types(&ty);
    assert_eq!(result, Some(vec![RustType::F64, RustType::String]));
}

#[test]
fn test_extract_fn_param_types_from_named_type_in_registry() {
    let mut reg = TypeRegistry::new();
    reg.register(
        "GetConnInfo".to_string(),
        crate::registry::TypeDef::Function {
            params: vec![(
                "c".to_string(),
                RustType::Named {
                    name: "Context".to_string(),
                    type_args: vec![],
                },
            )],
            return_type: Some(RustType::Named {
                name: "ConnInfo".to_string(),
                type_args: vec![],
            }),
            has_rest: false,
        },
    );
    let ty = RustType::Named {
        name: "GetConnInfo".to_string(),
        type_args: vec![],
    };
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let t = Transformer::for_module(&tctx, &mut synthetic);
    let result = t.extract_fn_param_types(&ty);
    assert_eq!(
        result,
        Some(vec![RustType::Named {
            name: "Context".to_string(),
            type_args: vec![]
        }])
    );
}

#[test]
fn test_extract_fn_param_types_unknown_returns_none() {
    let ty = RustType::String;
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let t = Transformer::for_module(&tctx, &mut synthetic);
    let result = t.extract_fn_param_types(&ty);
    assert_eq!(result, None);
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
    let init_fn = items.iter().find(|i| matches!(i, Item::Fn { name, .. } if name == "init"));
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
    assert!(!has_init, "no init() should be generated when no top-level expressions exist");
}

// --- private class members ---

#[test]
fn test_private_method_generates_non_pub_method() {
    let source = r#"
        class Foo {
            #helper(): string { return "help"; }
            public greet(): string { return this.#helper(); }
        }
    "#;
    let module = parse_typescript(source).expect("parse failed");
    let (items, unsupported) = transform_module_collecting(&module, &TypeRegistry::new()).unwrap();
    assert!(
        unsupported.is_empty(),
        "private method should not be unsupported: {unsupported:?}"
    );
    // Find the impl block
    let impl_item = items
        .iter()
        .find(|i| matches!(i, Item::Impl { methods, .. } if methods.iter().any(|m| m.name == "helper")));
    assert!(
        impl_item.is_some(),
        "expected 'helper' method in impl block, items: {items:?}"
    );
    if let Some(Item::Impl { methods, .. }) = impl_item {
        let helper = methods.iter().find(|m| m.name == "helper").unwrap();
        assert_eq!(helper.vis, Visibility::Private, "private method should have Private visibility");
    }
}

#[test]
fn test_private_prop_generates_non_pub_field() {
    let source = r#"
        class Counter {
            #count: number;
            public value: string;
        }
    "#;
    let module = parse_typescript(source).expect("parse failed");
    let (items, unsupported) = transform_module_collecting(&module, &TypeRegistry::new()).unwrap();
    assert!(
        unsupported.is_empty(),
        "private prop should not be unsupported: {unsupported:?}"
    );
    let struct_item = items
        .iter()
        .find(|i| matches!(i, Item::Struct { name, .. } if name == "Counter"));
    assert!(struct_item.is_some(), "expected Counter struct");
    if let Some(Item::Struct { fields, .. }) = struct_item {
        let count_field = fields.iter().find(|f| f.name == "count");
        assert!(count_field.is_some(), "expected 'count' field (# prefix removed)");
        if let Some(f) = count_field {
            assert_eq!(f.vis, Some(Visibility::Private), "private prop should have Private visibility");
        }
    }
}

#[test]
fn test_static_block_generates_init_static_method() {
    let source = r#"
        class Cache {
            static {
                console.log("initializing");
            }
        }
    "#;
    let module = parse_typescript(source).expect("parse failed");
    let (items, unsupported) = transform_module_collecting(&module, &TypeRegistry::new()).unwrap();
    assert!(
        unsupported.is_empty(),
        "static block should not be unsupported: {unsupported:?}"
    );
    let impl_item = items
        .iter()
        .find(|i| matches!(i, Item::Impl { methods, .. } if methods.iter().any(|m| m.name == "_init_static")));
    assert!(
        impl_item.is_some(),
        "expected '_init_static' method, items: {items:?}"
    );
}

// --- declare module error propagation ---

#[test]
fn test_declare_module_inner_error_reported_in_resilient_mode() {
    // An unsupported declaration inside `declare module` should be reported, not silently dropped
    let source = r#"
        declare module 'test' {
            interface Valid { x: string; }
            using y = something;
        }
    "#;
    let module = parse_typescript(source).expect("parse failed");
    let (items, unsupported) = transform_module_collecting(&module, &TypeRegistry::new()).unwrap();
    // Valid interface should still be converted
    assert!(
        items
            .iter()
            .any(|i| matches!(i, Item::Struct { name, .. } if name == "Valid")),
        "valid interface should be converted: {items:?}"
    );
    // `using` declaration should be reported as unsupported
    assert!(
        !unsupported.is_empty(),
        "unsupported declaration inside declare module should be reported"
    );
}

#[test]
fn test_declare_module_inner_error_propagates_in_strict_mode() {
    // In strict mode (non-resilient), unsupported inside declare module should error
    let source = r#"
        declare module 'test' {
            using y = something;
        }
    "#;
    let module = parse_typescript(source).expect("parse failed");
    let result = transform_module(&module, &TypeRegistry::new());
    assert!(result.is_err(), "unsupported inside declare module should error in strict mode");
}

// --- D1: import resolution with ModuleGraph ---

#[test]
fn test_transform_import_module_graph_fallback_when_empty_graph() {
    // ModuleGraph::empty() (single-file mode) → falls back to convert_relative_path_to_crate_path
    let source = r#"import { Foo } from "./bar";"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Private,
            path: "crate::bar".to_string(),
            names: vec!["Foo".to_string()],
        }
    );
}

#[test]
fn test_transform_import_module_graph_resolves_import_path() {
    // When ModuleGraph can resolve the import, it should use the resolved path
    // instead of convert_relative_path_to_crate_path
    use crate::pipeline::ModuleGraphBuilder;

    let root = std::path::Path::new("");
    let file_a = std::path::PathBuf::from("adapter/server.ts");
    let file_b = std::path::PathBuf::from("types.ts");
    let source_a = r#"import { Config } from "../types";"#;
    let source_b = r#"export interface Config { port: number; }"#;

    let known_files: std::collections::HashSet<std::path::PathBuf> = [
        std::path::PathBuf::from("adapter/server.ts"),
        std::path::PathBuf::from("types.ts"),
    ]
    .into_iter()
    .collect();

    let parsed = crate::pipeline::parse_files(vec![
        (file_a.clone(), source_a.to_string()),
        (file_b.clone(), source_b.to_string()),
    ])
    .unwrap();

    let resolver =
        crate::pipeline::module_resolver::NodeModuleResolver::new(root.to_path_buf(), known_files);
    let module_graph = ModuleGraphBuilder::new(&parsed, &resolver, root).build();

    let reg = TypeRegistry::new();
    let res = crate::pipeline::type_resolution::FileTypeResolution::empty();
    let tctx =
        crate::transformer::context::TransformContext::new(&module_graph, &reg, &res, &file_a);

    let mut synthetic = crate::pipeline::SyntheticTypeRegistry::new();
    let items = crate::transformer::transform_module_with_context(
        &parsed.files[0].module,
        &tctx,
        &mut synthetic,
    )
    .unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Private,
            path: "crate::types".to_string(),
            names: vec!["Config".to_string()],
        }
    );
}

#[test]
fn test_transform_import_module_graph_resolves_reexport_chain() {
    // When B re-exports from C, importing from B should resolve to C's module path
    use crate::pipeline::ModuleGraphBuilder;

    let root = std::path::Path::new("");
    let file_a = std::path::PathBuf::from("app.ts");
    let file_b = std::path::PathBuf::from("index.ts");
    let file_c = std::path::PathBuf::from("types.ts");
    let source_a = r#"import { Config } from "./index";"#;
    let source_b = r#"export { Config } from "./types";"#;
    let source_c = r#"export interface Config { port: number; }"#;

    let known_files: std::collections::HashSet<std::path::PathBuf> = [
        std::path::PathBuf::from("app.ts"),
        std::path::PathBuf::from("index.ts"),
        std::path::PathBuf::from("types.ts"),
    ]
    .into_iter()
    .collect();

    let parsed = crate::pipeline::parse_files(vec![
        (file_a.clone(), source_a.to_string()),
        (file_b.clone(), source_b.to_string()),
        (file_c.clone(), source_c.to_string()),
    ])
    .unwrap();

    let resolver =
        crate::pipeline::module_resolver::NodeModuleResolver::new(root.to_path_buf(), known_files);
    let module_graph = ModuleGraphBuilder::new(&parsed, &resolver, root).build();

    let reg = TypeRegistry::new();
    let res = crate::pipeline::type_resolution::FileTypeResolution::empty();
    let tctx =
        crate::transformer::context::TransformContext::new(&module_graph, &reg, &res, &file_a);

    let mut synthetic = crate::pipeline::SyntheticTypeRegistry::new();
    let items = crate::transformer::transform_module_with_context(
        &parsed.files[0].module,
        &tctx,
        &mut synthetic,
    )
    .unwrap();

    assert_eq!(items.len(), 1);
    // Config should resolve to crate::types (where it's originally defined),
    // NOT crate (where index.ts re-exports it from)
    assert_eq!(
        items[0],
        Item::Use {
            vis: Visibility::Private,
            path: "crate::types".to_string(),
            names: vec!["Config".to_string()],
        }
    );
}

#[test]
fn test_unsupported_syntax_error_new_creates_correct_instance() {
    use swc_common::{BytePos, Span};
    let span = Span::new(BytePos(42), BytePos(50));
    let err = super::UnsupportedSyntaxError::new("TestKind", span);
    assert_eq!(err.kind, "TestKind");
    assert_eq!(err.byte_pos, 42);
}

#[test]
fn test_unsupported_syntax_error_new_converts_to_anyhow() {
    use swc_common::{BytePos, Span};
    let span = Span::new(BytePos(10), BytePos(20));
    let err = super::UnsupportedSyntaxError::new("SomeError", span);
    let anyhow_err: anyhow::Error = err.into();
    let downcasted = anyhow_err
        .downcast::<super::UnsupportedSyntaxError>()
        .unwrap();
    assert_eq!(downcasted.kind, "SomeError");
    assert_eq!(downcasted.byte_pos, 10);
}
