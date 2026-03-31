use super::*;
use crate::ir::Item;

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
    let impl_item = items.iter().find(
        |i| matches!(i, Item::Impl { methods, .. } if methods.iter().any(|m| m.name == "helper")),
    );
    assert!(
        impl_item.is_some(),
        "expected 'helper' method in impl block, items: {items:?}"
    );
    if let Some(Item::Impl { methods, .. }) = impl_item {
        let helper = methods.iter().find(|m| m.name == "helper").unwrap();
        assert_eq!(
            helper.vis,
            Visibility::Private,
            "private method should have Private visibility"
        );
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
        assert!(
            count_field.is_some(),
            "expected 'count' field (# prefix removed)"
        );
        if let Some(f) = count_field {
            assert_eq!(
                f.vis,
                Some(Visibility::Private),
                "private prop should have Private visibility"
            );
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

// --- transform_class_with_inheritance integration tests ---

#[test]
fn test_transform_class_abstract_generates_trait() {
    let source = r#"
abstract class Animal {
    abstract speak(): string;
    move(): void { console.log("moving"); }
}
"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    // Abstract class should produce a Trait, not a Struct
    let has_trait = items
        .iter()
        .any(|i| matches!(i, Item::Trait { name, .. } if name == "Animal"));
    assert!(
        has_trait,
        "abstract class should generate trait, got: {items:?}"
    );

    let has_struct = items
        .iter()
        .any(|i| matches!(i, Item::Struct { name, .. } if name == "Animal"));
    assert!(
        !has_struct,
        "abstract class should NOT generate struct, got: {items:?}"
    );

    // Trait should contain both abstract and concrete methods
    if let Some(Item::Trait { methods, .. }) = items
        .iter()
        .find(|i| matches!(i, Item::Trait { name, .. } if name == "Animal"))
    {
        assert!(
            methods.iter().any(|m| m.name == "speak"),
            "trait should contain abstract method 'speak'"
        );
        // "move" is a Rust keyword, so the transformer may escape it as "r#move"
        assert!(
            methods
                .iter()
                .any(|m| m.name == "r#move" || m.name == "move"),
            "trait should contain concrete method 'move', got: {:?}",
            methods.iter().map(|m| &m.name).collect::<Vec<_>>()
        );
    }
}

#[test]
fn test_transform_class_parent_generates_trait_and_struct() {
    let source = r#"
class Parent {
    name: string;
    getName(): string { return this.name; }
}
class Child extends Parent {
    age: number;
}
"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    // Parent class (extended by Child) should produce Struct + Trait + Impl
    let has_parent_struct = items
        .iter()
        .any(|i| matches!(i, Item::Struct { name, .. } if name == "Parent"));
    assert!(
        has_parent_struct,
        "parent class should generate struct, got: {items:?}"
    );

    let has_parent_trait = items
        .iter()
        .any(|i| matches!(i, Item::Trait { name, .. } if name == "ParentTrait"));
    assert!(
        has_parent_trait,
        "parent class should generate ParentTrait, got: {items:?}"
    );

    let has_trait_impl = items.iter().any(|i| {
        matches!(
            i,
            Item::Impl {
                struct_name,
                for_trait: Some(trait_ref),
                ..
            } if struct_name == "Parent" && trait_ref.name == "ParentTrait"
        )
    });
    assert!(
        has_trait_impl,
        "parent should have impl ParentTrait for Parent, got: {items:?}"
    );
}

#[test]
fn test_transform_class_child_of_abstract_generates_impl_trait() {
    let source = r#"
abstract class Animal {
    abstract speak(): string;
}
class Dog extends Animal {
    name: string;
    speak(): string { return this.name; }
}
"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    // Dog should have Struct + impl Animal for Dog
    let has_dog_struct = items
        .iter()
        .any(|i| matches!(i, Item::Struct { name, .. } if name == "Dog"));
    assert!(
        has_dog_struct,
        "child of abstract should generate struct, got: {items:?}"
    );

    let has_trait_impl = items.iter().any(|i| {
        matches!(
            i,
            Item::Impl {
                struct_name,
                for_trait: Some(trait_ref),
                ..
            } if struct_name == "Dog" && trait_ref.name == "Animal"
        )
    });
    assert!(
        has_trait_impl,
        "child of abstract should have impl Animal for Dog, got: {items:?}"
    );
}

#[test]
fn test_transform_class_child_with_implements_generates_all() {
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

    // Child struct
    let has_child_struct = items
        .iter()
        .any(|i| matches!(i, Item::Struct { name, .. } if name == "Child"));
    assert!(has_child_struct, "should have Child struct, got: {items:?}");

    // impl ParentTrait for Child
    let has_parent_trait_impl = items.iter().any(|i| {
        matches!(
            i,
            Item::Impl {
                struct_name,
                for_trait: Some(trait_ref),
                ..
            } if struct_name == "Child" && trait_ref.name == "ParentTrait"
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
                for_trait: Some(trait_ref),
                ..
            } if struct_name == "Child" && trait_ref.name == "Greeter"
        )
    });
    assert!(
        has_greeter_impl,
        "should have impl Greeter for Child, got: {items:?}"
    );

    // impl Child (own methods)
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
    assert!(own_impl.is_some(), "should have impl Child, got: {items:?}");
    if let Some(Item::Impl { methods, .. }) = own_impl {
        assert!(
            methods.iter().any(|m| m.name == "helper"),
            "own impl should contain helper"
        );
        assert!(
            !methods.iter().any(|m| m.name == "greet"),
            "own impl should NOT contain greet (belongs to impl Greeter)"
        );
    }
}

/// I-335 integration: pre_scan_classes → build_mut_method_names → mark_mut_params_from_body
/// Verifies the full pipeline generates `let mut counter = counter;` rebinding
/// when a function parameter receives a `&mut self` method call.
#[test]
fn test_transform_module_mut_self_method_generates_param_rebinding() {
    let source = r#"
class Counter {
    _count: number;
    increment(): void {
        this._count += 1;
    }
    get_count(): number {
        return this._count;
    }
}

function useCounter(counter: Counter): number {
    counter.increment();
    return counter.get_count();
}
"#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    // Find the useCounter function (name is not snake_cased for standalone functions)
    let use_counter_fn = items
        .iter()
        .find(|item| matches!(item, Item::Fn { name, .. } if name == "useCounter"));
    assert!(
        use_counter_fn.is_some(),
        "should have use_counter function, got: {items:#?}"
    );

    let body = match use_counter_fn.unwrap() {
        Item::Fn { body, .. } => body,
        _ => unreachable!(),
    };

    // First statement should be the `let mut counter = counter;` rebinding
    let has_rebinding = body.iter().any(|stmt| {
        matches!(
            stmt,
            Stmt::Let {
                mutable: true,
                name,
                init: Some(Expr::Ident(init_name)),
                ..
            } if name == "counter" && init_name == "counter"
        )
    });
    assert!(
        has_rebinding,
        "useCounter body should contain 'let mut counter = counter;' rebinding, got: {body:#?}"
    );
}
