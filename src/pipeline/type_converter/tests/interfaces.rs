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
            ..
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
                    constraint: None,
                    default: None,
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
                        constraint: None,
                        default: None,
                    },
                    TypeParam {
                        name: "B".to_string(),
                        constraint: None,
                        default: None,
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
            assert_eq!(methods[0].return_type, Some(RustType::Unit));
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

/// I-040 T2: interface method の optional パラメータ (`y?: number`) が
/// `Option<f64>` にラップされた `Method.params` で IR に格納されることを保証する。
#[test]
fn test_convert_method_signature_optional_param_wraps_in_option() {
    let decl = parse_interface("interface Foo { bar(x: number, y?: number): number; }");
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
            assert_eq!(methods[0].params[0].name, "x");
            assert_eq!(methods[0].params[0].ty, Some(RustType::F64));
            assert_eq!(methods[0].params[1].name, "y");
            assert_eq!(
                methods[0].params[1].ty,
                Some(RustType::Option(Box::new(RustType::F64)))
            );
        }
        _ => panic!("expected Item::Trait, got {:?}", item),
    }
}

/// I-040 T2 negative: required interface method param は Option ラップされない。
#[test]
fn test_convert_method_signature_required_param_not_wrapped() {
    let decl = parse_interface("interface Foo { bar(x: number, y: number): number; }");
    let item = convert_interface(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();

    match item {
        Item::Trait { methods, .. } => {
            assert_eq!(methods[0].params[0].ty, Some(RustType::F64));
            assert_eq!(methods[0].params[1].ty, Some(RustType::F64));
        }
        _ => panic!("expected Item::Trait"),
    }
}

/// I-040 T3: callable interface (`(y?: number): void`) の optional パラメータが
/// `Option<f64>` にラップされた `call_N` メソッドの params で IR に格納される。
#[test]
fn test_convert_callable_interface_optional_param_wraps() {
    let decl = parse_interface("interface Handler { (x: number, y?: number): void; }");
    let item = convert_interface(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();

    match item {
        Item::Trait { methods, .. } => {
            assert_eq!(methods.len(), 1);
            assert_eq!(methods[0].name, "call_0");
            assert_eq!(methods[0].params.len(), 2);
            assert_eq!(methods[0].params[0].ty, Some(RustType::F64));
            assert_eq!(
                methods[0].params[1].ty,
                Some(RustType::Option(Box::new(RustType::F64)))
            );
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
fn test_convert_interface_call_signature_single_generates_trait() {
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
        Item::Trait {
            vis: Visibility::Public,
            name: "Callback".to_string(),
            type_params: vec![],
            supertraits: vec![],
            methods: vec![Method {
                vis: Visibility::Public,
                name: "call_0".to_string(),
                is_async: false,
                has_self: true,
                has_mut_self: false,
                params: vec![Param {
                    name: "x".to_string(),
                    ty: Some(RustType::F64),
                }],
                return_type: Some(RustType::String),
                body: None,
            }],
            associated_types: vec![],
        }
    );
}

#[test]
fn test_convert_interface_call_signature_overload_generates_trait_methods() {
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
        Item::Trait { methods, .. } => {
            assert_eq!(methods.len(), 2);
            assert_eq!(methods[0].name, "call_0");
            assert_eq!(methods[0].params.len(), 1);
            assert_eq!(methods[1].name, "call_1");
            assert_eq!(methods[1].params.len(), 2);
        }
        _ => panic!("expected Item::Trait"),
    }
}

#[test]
fn test_convert_interface_call_signature_no_params_generates_trait() {
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
        Item::Trait {
            vis: Visibility::Public,
            name: "Factory".to_string(),
            type_params: vec![],
            supertraits: vec![],
            methods: vec![Method {
                vis: Visibility::Public,
                name: "call_0".to_string(),
                is_async: false,
                has_self: true,
                has_mut_self: false,
                params: vec![],
                return_type: None,
                body: None,
            }],
            associated_types: vec![],
        }
    );
}

// --- call signature rest parameters ---

#[test]
fn test_convert_interface_call_signature_rest_param_generates_trait() {
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
        Item::Trait {
            vis: Visibility::Public,
            name: "Handler".to_string(),
            type_params: vec![],
            supertraits: vec![],
            methods: vec![Method {
                vis: Visibility::Public,
                name: "call_0".to_string(),
                is_async: false,
                has_self: true,
                has_mut_self: false,
                params: vec![Param {
                    name: "args".to_string(),
                    ty: Some(RustType::Vec(Box::new(RustType::F64))),
                }],
                return_type: None,
                body: None,
            }],
            associated_types: vec![],
        }
    );
}

#[test]
fn test_convert_interface_call_signature_mixed_and_rest_generates_trait() {
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
        Item::Trait { methods, .. } => {
            assert_eq!(methods.len(), 1);
            let m = &methods[0];
            assert_eq!(m.name, "call_0");
            assert_eq!(m.params.len(), 2);
            assert_eq!(m.params[0].name, "template");
            assert_eq!(m.params[0].ty, Some(RustType::String));
            assert_eq!(m.params[1].name, "values");
            assert_eq!(m.params[1].ty, Some(RustType::Vec(Box::new(RustType::F64))));
            assert_eq!(m.return_type, Some(RustType::String));
        }
        _ => panic!("expected Item::Trait"),
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

/// P12.1: Call-signature-level type param with default is propagated to trait.
///
/// This test covers the partition missed by interface-level tests: type params
/// defined ON the call signature (not the interface) with defaults.
/// Without this test, `interfaces.rs:249` `default: None` hardcoding went undetected.
#[test]
fn test_convert_interface_call_sig_type_param_default_propagated() {
    let decl = parse_interface("interface WithDefault { <T = string>(x: T): T }");
    let items = convert_interface_items(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Trait {
            type_params,
            methods,
            ..
        } => {
            // Call-sig type param T with default=string must be merged into trait
            assert_eq!(type_params.len(), 1);
            assert_eq!(type_params[0].name, "T");
            assert_eq!(
                type_params[0].default,
                Some(RustType::String),
                "call-sig type param default must be propagated, not None"
            );
            // Method should use T
            assert_eq!(methods.len(), 1);
            assert_eq!(methods[0].name, "call_0");
        }
        _ => panic!("expected Item::Trait"),
    }
}
