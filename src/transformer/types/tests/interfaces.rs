use super::*;

// -- convert_interface: basic tests --

#[test]
fn test_convert_interface_basic() {
    let decl = parse_interface("interface Foo { name: string; age: number; }");
    let item = convert_interface(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();

    match item {
        Item::Struct {
            vis,
            name,
            type_params,
            fields,
        } => {
            assert_eq!(vis, Visibility::Public);
            assert_eq!(name, "Foo");
            assert!(type_params.is_empty());
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "name");
            assert_eq!(fields[0].ty, RustType::String);
            assert_eq!(fields[1].name, "age");
            assert_eq!(fields[1].ty, RustType::F64);
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_convert_interface_optional_field() {
    let decl = parse_interface("interface Bar { label?: string; }");
    let item = convert_interface(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();

    match item {
        Item::Struct { fields, .. } => {
            assert_eq!(fields[0].name, "label");
            assert_eq!(fields[0].ty, RustType::Option(Box::new(RustType::String)));
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_convert_interface_optional_union_null_no_double_wrap() {
    // `name?: string | null` should be `Option<String>`, not `Option<Option<String>>`
    let decl = parse_interface("interface Baz { name?: string | null; }");
    let item = convert_interface(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();

    match item {
        Item::Struct { fields, .. } => {
            assert_eq!(fields[0].ty, RustType::Option(Box::new(RustType::String)));
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_convert_interface_vec_field() {
    let decl = parse_interface("interface Qux { items: number[]; }");
    let item = convert_interface(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();

    match item {
        Item::Struct { fields, .. } => {
            assert_eq!(fields[0].ty, RustType::Vec(Box::new(RustType::F64)));
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_convert_interface_with_type_params() {
    let decl = parse_interface("interface Container<T> { value: T; }");
    let item = convert_interface(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();

    match item {
        Item::Struct { type_params, .. } => {
            assert_eq!(
                type_params,
                vec![TypeParam {
                    name: "T".to_string(),
                    constraint: None
                }]
            );
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_convert_interface_with_multiple_type_params() {
    let decl = parse_interface("interface Pair<A, B> { first: A; second: B; }");
    let item = convert_interface(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();

    match item {
        Item::Struct { type_params, .. } => {
            assert_eq!(
                type_params,
                vec![
                    TypeParam {
                        name: "A".to_string(),
                        constraint: None
                    },
                    TypeParam {
                        name: "B".to_string(),
                        constraint: None
                    },
                ]
            );
        }
        _ => panic!("expected Item::Struct"),
    }
}

// -- convert_interface with method signatures --

#[test]
fn test_convert_interface_method_only_generates_trait() {
    let decl = parse_interface("interface Greeter { greet(name: string): string; }");
    let item = convert_interface(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();

    match item {
        Item::Trait {
            vis, name, methods, ..
        } => {
            assert_eq!(vis, Visibility::Public);
            assert_eq!(name, "Greeter");
            assert_eq!(methods.len(), 1);
            assert_eq!(methods[0].name, "greet");
            assert!(methods[0].has_self);
            assert_eq!(methods[0].params.len(), 1);
            assert_eq!(methods[0].params[0].name, "name");
            assert_eq!(methods[0].params[0].ty, Some(RustType::String));
            assert_eq!(methods[0].return_type, Some(RustType::String));
        }
        _ => panic!("expected Item::Trait, got {:?}", item),
    }
}

#[test]
fn test_convert_interface_method_no_args_void_return() {
    let decl = parse_interface("interface Runner { run(): void; }");
    let item = convert_interface(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();

    match item {
        Item::Trait { methods, .. } => {
            assert_eq!(methods[0].name, "run");
            assert!(methods[0].has_self);
            assert!(methods[0].params.is_empty());
            assert_eq!(methods[0].return_type, None);
        }
        _ => panic!("expected Item::Trait"),
    }
}

#[test]
fn test_convert_interface_method_multiple_params() {
    let decl = parse_interface("interface Math { add(a: number, b: number): number; }");
    let item = convert_interface(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();

    match item {
        Item::Trait { methods, .. } => {
            assert_eq!(methods[0].params.len(), 2);
            assert_eq!(methods[0].params[0].name, "a");
            assert_eq!(methods[0].params[1].name, "b");
            assert_eq!(methods[0].return_type, Some(RustType::F64));
        }
        _ => panic!("expected Item::Trait"),
    }
}

#[test]
fn test_convert_interface_properties_only_still_struct() {
    let decl = parse_interface("interface Point { x: number; y: number; }");
    let item = convert_interface(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();

    assert!(matches!(item, Item::Struct { .. }));
}

#[test]
fn test_convert_interface_method_with_type_params() {
    let decl = parse_interface("interface Repo<T> { find(id: string): T; save(item: T): void; }");
    let item = convert_interface(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();

    match item {
        Item::Trait { name, methods, .. } => {
            assert_eq!(name, "Repo");
            assert_eq!(methods.len(), 2);
            assert_eq!(methods[0].name, "find");
            assert_eq!(methods[1].name, "save");
        }
        _ => panic!("expected Item::Trait"),
    }
}

// -- call signatures --

#[test]
fn test_convert_interface_call_signature_single_generates_fn_type_alias() {
    let decl = parse_interface("interface Callback { (x: number): string }");
    let items = convert_interface_items(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::TypeAlias {
            vis: Visibility::Public,
            name: "Callback".to_string(),
            type_params: vec![],
            ty: RustType::Fn {
                params: vec![RustType::F64],
                return_type: Box::new(RustType::String),
            },
        }
    );
}

#[test]
fn test_convert_interface_call_signature_overload_uses_longest() {
    let decl = parse_interface(
        "interface Overloaded { (x: number): string; (x: number, y: string): boolean }",
    );
    let items = convert_interface_items(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::TypeAlias { ty, .. } => match ty {
            RustType::Fn { params, .. } => {
                assert_eq!(params.len(), 2);
            }
            _ => panic!("expected RustType::Fn"),
        },
        _ => panic!("expected Item::TypeAlias"),
    }
}

#[test]
fn test_convert_interface_call_signature_no_params_generates_fn_type() {
    let decl = parse_interface("interface Factory { (): void }");
    let items = convert_interface_items(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::TypeAlias {
            vis: Visibility::Public,
            name: "Factory".to_string(),
            type_params: vec![],
            ty: RustType::Fn {
                params: vec![],
                return_type: Box::new(RustType::Unit),
            },
        }
    );
}

// --- call signature rest parameters ---

#[test]
fn test_convert_interface_call_signature_rest_param_generates_fn_type() {
    let decl = parse_interface("interface Handler { (...args: number[]): void }");
    let items = convert_interface_items(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::TypeAlias {
            vis: Visibility::Public,
            name: "Handler".to_string(),
            type_params: vec![],
            ty: RustType::Fn {
                params: vec![RustType::Vec(Box::new(RustType::F64))],
                return_type: Box::new(RustType::Unit),
            },
        }
    );
}

#[test]
fn test_convert_interface_call_signature_mixed_and_rest_generates_fn_type() {
    let decl =
        parse_interface("interface Formatter { (template: string, ...values: number[]): string }");
    let items = convert_interface_items(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::TypeAlias {
            ty: RustType::Fn {
                params,
                return_type,
            },
            ..
        } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0], RustType::String);
            assert_eq!(params[1], RustType::Vec(Box::new(RustType::F64)));
            assert_eq!(return_type.as_ref(), &RustType::String);
        }
        _ => panic!("expected TypeAlias with Fn type"),
    }
}

// -- mixed properties and methods --

#[test]
fn test_convert_interface_mixed_props_and_methods_generates_struct_and_trait() {
    let decl = parse_interface("interface Ctx { name: string; greet(msg: string): void }");
    let items = convert_interface_items(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(items.len(), 3);
    // First: struct with properties (named {Name}Data)
    match &items[0] {
        Item::Struct { name, fields, .. } => {
            assert_eq!(name, "CtxData");
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].name, "name");
        }
        _ => panic!("expected Item::Struct, got {:?}", items[0]),
    }
    // Second: trait with methods (named {Name} — the interface name)
    match &items[1] {
        Item::Trait { name, methods, .. } => {
            assert_eq!(name, "Ctx");
            assert_eq!(methods.len(), 1);
            assert_eq!(methods[0].name, "greet");
        }
        _ => panic!("expected Item::Trait, got {:?}", items[1]),
    }
    // Third: impl trait for struct
    match &items[2] {
        Item::Impl {
            struct_name,
            for_trait,
            ..
        } => {
            assert_eq!(struct_name, "CtxData");
            assert_eq!(for_trait.as_ref().map(|t| t.name.as_str()), Some("Ctx"));
        }
        _ => panic!("expected Item::Impl, got {:?}", items[2]),
    }
}
