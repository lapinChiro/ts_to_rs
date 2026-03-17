use super::*;
use crate::ir::Stmt;
use crate::ir::{BinOp, Expr, Param, RustType, StructField, Visibility};
use crate::parser::parse_typescript;
use crate::registry::TypeRegistry;

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
            } if struct_name == "Foo" && trait_name == "Greeter"
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
            } if struct_name == "Foo" && trait_name == "A"
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
            } if struct_name == "Foo" && trait_name == "B"
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
            } if t == "Greeter"
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
            } if struct_name == "Child" && trait_name == "ParentTrait"
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
            } if struct_name == "Child" && trait_name == "Greeter"
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

// --- TypeEnv tests ---

#[test]
fn test_type_env_insert_and_get_returns_registered_type() {
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::F64);

    assert_eq!(env.get("x"), Some(&RustType::F64));
}

#[test]
fn test_type_env_get_unregistered_returns_none() {
    let env = TypeEnv::new();

    assert_eq!(env.get("x"), None);
}

#[test]
fn test_type_env_insert_same_name_overwrites_shadowing() {
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::F64);
    env.insert("x".to_string(), RustType::String);

    assert_eq!(env.get("x"), Some(&RustType::String));
}

#[test]
fn test_type_env_get_parent_scope_variable_returns_type() {
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::F64);
    env.push_scope();

    // 子スコープから親の変数が見える
    assert_eq!(env.get("x"), Some(&RustType::F64));
}

#[test]
fn test_type_env_shadow_in_child_scope_hides_parent() {
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::F64);
    env.push_scope();
    env.insert("x".to_string(), RustType::String);

    // 子スコープではシャドウされた型
    assert_eq!(env.get("x"), Some(&RustType::String));

    // pop 後は親の型に戻る
    env.pop_scope();
    assert_eq!(env.get("x"), Some(&RustType::F64));
}

#[test]
fn test_type_env_pop_scope_removes_child_variables() {
    let mut env = TypeEnv::new();
    env.push_scope();
    env.insert("y".to_string(), RustType::Bool);
    env.pop_scope();

    // 子スコープの変数は pop 後に消える
    assert_eq!(env.get("y"), None);
}

#[test]
fn test_type_env_update_modifies_parent_scope() {
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::F64);
    env.push_scope();
    env.update("x".to_string(), RustType::String);

    // 子スコープから親の変数が更新される
    assert_eq!(env.get("x"), Some(&RustType::String));

    env.pop_scope();
    // pop 後も更新は維持される
    assert_eq!(env.get("x"), Some(&RustType::String));
}

#[test]
fn test_type_env_update_nonexistent_inserts_in_current_scope() {
    let mut env = TypeEnv::new();
    env.push_scope();
    env.update("z".to_string(), RustType::Bool);

    // 存在しない変数の update は現在のスコープに挿入
    assert_eq!(env.get("z"), Some(&RustType::Bool));

    env.pop_scope();
    // pop 後は消える（親スコープには入らない）
    assert_eq!(env.get("z"), None);
}

#[test]
fn test_type_env_clone_is_independent() {
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::F64);

    let mut cloned = env.clone();
    cloned.insert("x".to_string(), RustType::String);

    // 元の環境は変わらない
    assert_eq!(env.get("x"), Some(&RustType::F64));
    assert_eq!(cloned.get("x"), Some(&RustType::String));
}

#[test]
fn test_type_env_nested_scopes_three_levels() {
    let mut env = TypeEnv::new();
    env.insert("x".to_string(), RustType::F64);

    env.push_scope();
    env.insert("x".to_string(), RustType::String);

    env.push_scope();
    env.insert("x".to_string(), RustType::Bool);
    assert_eq!(env.get("x"), Some(&RustType::Bool));

    env.pop_scope();
    assert_eq!(env.get("x"), Some(&RustType::String));

    env.pop_scope();
    assert_eq!(env.get("x"), Some(&RustType::F64));
}
