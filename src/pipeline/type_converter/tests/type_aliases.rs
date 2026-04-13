use super::*;

// -- convert_type_alias: basic tests --

#[test]
fn test_convert_type_alias_object_literal() {
    let decl = parse_type_alias("type Point = { x: number; y: number; };");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();

    match item {
        Item::Struct { name, fields, .. } => {
            assert_eq!(name, "Point");
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "x");
            assert_eq!(fields[0].ty, RustType::F64);
            assert_eq!(fields[1].name, "y");
            assert_eq!(fields[1].ty, RustType::F64);
        }
        _ => panic!("expected Item::Struct"),
    }
}

#[test]
fn test_convert_type_alias_with_type_params() {
    let decl = parse_type_alias("type Pair<A, B> = { first: A; second: B; };");
    let item = convert_type_alias(
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

#[test]
fn test_convert_type_alias_keyword_type_returns_type_alias() {
    let decl = parse_type_alias("type Name = string;");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::TypeAlias { name, ty, .. } => {
            assert_eq!(name, "Name");
            assert_eq!(ty, RustType::String);
        }
        _ => panic!("expected Item::TypeAlias, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_object_keyword_returns_serde_json_value() {
    let decl = parse_type_alias("type X = object;");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::TypeAlias { name, ty, .. } => {
            assert_eq!(name, "X");
            assert_eq!(
                ty,
                RustType::Named {
                    name: "serde_json::Value".to_string(),
                    type_args: vec![],
                }
            );
        }
        _ => panic!("expected Item::TypeAlias, got {:?}", item),
    }
}

// -- convert_type_alias: function type body --

#[test]
fn test_convert_type_alias_function_type_single_param() {
    let decl = parse_type_alias("type Handler = (req: Request) => Response;");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();

    match item {
        Item::TypeAlias {
            vis,
            name,
            type_params,
            ty,
        } => {
            assert_eq!(vis, Visibility::Public);
            assert_eq!(name, "Handler");
            assert!(type_params.is_empty());
            assert_eq!(
                ty,
                RustType::Fn {
                    params: vec![RustType::Named {
                        name: "Request".to_string(),
                        type_args: vec![],
                    }],
                    return_type: Box::new(RustType::Named {
                        name: "Response".to_string(),
                        type_args: vec![],
                    }),
                }
            );
        }
        _ => panic!("expected Item::TypeAlias, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_function_type_no_params() {
    let decl = parse_type_alias("type Factory = () => Widget;");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();

    match item {
        Item::TypeAlias { ty, .. } => {
            assert_eq!(
                ty,
                RustType::Fn {
                    params: vec![],
                    return_type: Box::new(RustType::Named {
                        name: "Widget".to_string(),
                        type_args: vec![],
                    }),
                }
            );
        }
        _ => panic!("expected Item::TypeAlias"),
    }
}

#[test]
fn test_convert_type_alias_function_type_void_return() {
    let decl = parse_type_alias("type Callback = (x: number) => void;");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();

    match item {
        Item::TypeAlias { ty, .. } => {
            assert_eq!(
                ty,
                RustType::Fn {
                    params: vec![RustType::F64],
                    return_type: Box::new(RustType::Unit),
                }
            );
        }
        _ => panic!("expected Item::TypeAlias"),
    }
}

#[test]
fn test_convert_type_alias_function_type_multiple_params() {
    let decl = parse_type_alias("type ErrorHandler = (err: string, ctx: Context) => Response;");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();

    match item {
        Item::TypeAlias { ty, .. } => match ty {
            RustType::Fn { params, .. } => {
                assert_eq!(params.len(), 2);
                assert_eq!(params[0], RustType::String);
            }
            _ => panic!("expected RustType::Fn"),
        },
        _ => panic!("expected Item::TypeAlias"),
    }
}

#[test]
fn test_convert_type_alias_tuple_type() {
    let decl = parse_type_alias("type Pair = [string, number];");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();

    match item {
        Item::TypeAlias {
            vis,
            name,
            type_params,
            ty,
        } => {
            assert_eq!(vis, Visibility::Public);
            assert_eq!(name, "Pair");
            assert!(type_params.is_empty());
            assert_eq!(ty, RustType::Tuple(vec![RustType::String, RustType::F64]));
        }
        _ => panic!("expected Item::TypeAlias, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_function_type_with_generics() {
    let decl = parse_type_alias("type Mapper<T, U> = (item: T) => U;");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();

    match item {
        Item::TypeAlias { type_params, .. } => {
            assert_eq!(
                type_params,
                vec![
                    TypeParam {
                        name: "T".to_string(),
                        constraint: None,
                        default: None,
                    },
                    TypeParam {
                        name: "U".to_string(),
                        constraint: None,
                        default: None,
                    },
                ]
            );
        }
        _ => panic!("expected Item::TypeAlias"),
    }
}

// -- convert_type_alias: conditional types --

#[test]
fn test_convert_type_alias_conditional_filter_returns_type_alias_with_true_branch() {
    let decl = parse_type_alias("type Filter<T> = T extends string ? T : never;");
    let items = convert_type_alias_items(
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
            name: "Filter".to_string(),
            type_params: vec![TypeParam {
                name: "T".to_string(),
                constraint: None,
                default: None,
            }],
            // I-387: `T` is represented as TypeVar (not Named).
            ty: RustType::TypeVar {
                name: "T".to_string(),
            },
        }
    );
}

#[test]
fn test_convert_type_alias_conditional_simple_returns_type_alias_with_true_branch() {
    let decl = parse_type_alias("type ToNum<T> = T extends string ? number : boolean;");
    let items = convert_type_alias_items(
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
            name: "ToNum".to_string(),
            type_params: vec![],
            ty: RustType::F64,
        }
    );
}

#[test]
fn test_convert_type_alias_conditional_predicate_returns_bool() {
    let decl = parse_type_alias("type IsString<T> = T extends string ? true : false;");
    let items = convert_type_alias_items(
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
            name: "IsString".to_string(),
            type_params: vec![],
            ty: RustType::Bool,
        }
    );
}

#[test]
fn test_convert_type_alias_conditional_infer_returns_associated_type() {
    let decl = parse_type_alias("type Unwrap<T> = T extends Promise<infer U> ? U : never;");
    let mut synthetic = SyntheticTypeRegistry::new();
    let items = convert_type_alias_items(
        &decl,
        Visibility::Public,
        &mut synthetic,
        &TypeRegistry::new(),
    )
    .unwrap();
    // The type alias itself is in items; the synthetic Promise trait stub is in the registry
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0],
        Item::TypeAlias {
            vis: Visibility::Public,
            name: "Unwrap".to_string(),
            type_params: vec![TypeParam {
                name: "T".to_string(),
                constraint: None,
                default: None,
            }],
            ty: RustType::QSelf {
                qself: Box::new(RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![],
                }),
                trait_ref: crate::ir::TraitRef {
                    name: "Promise".to_string(),
                    type_args: vec![],
                },
                item: "Output".to_string(),
            },
        }
    );
    // Synthetic Promise trait stub should be in the registry
    let synth_items = synthetic.all_items();
    assert_eq!(synth_items.len(), 1);
    assert_eq!(
        *synth_items[0],
        Item::Trait {
            vis: Visibility::Public,
            name: "Promise".to_string(),
            type_params: vec![],
            supertraits: vec![],
            methods: vec![],
            associated_types: vec!["Output".to_string()],
        }
    );
}

#[test]
fn test_convert_type_alias_conditional_nested_generates_type_alias() {
    // Nested conditional types are now handled recursively via convert_conditional_type
    // Outer: T extends string ? (inner) : never → true branch = inner conditional
    // Inner: T extends "a" ? number : boolean → true branch = number
    let decl = parse_type_alias(
        "type Foo<T> = T extends string ? T extends \"a\" ? number : boolean : never;",
    );
    let items = convert_type_alias_items(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(items.len(), 1);
    // Recursive true-branch fallback: outer true → inner true → number → f64
    assert_eq!(
        items[0],
        Item::TypeAlias {
            vis: Visibility::Public,
            name: "Foo".to_string(),
            type_params: vec![],
            ty: RustType::F64,
        }
    );
}

// -- Index signature in type literal → HashMap --

#[test]
fn test_convert_type_alias_index_signature_to_hashmap() {
    let decl = parse_type_alias("type Foo = { [key: string]: number };");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();

    match item {
        Item::TypeAlias { name, ty, .. } => {
            assert_eq!(name, "Foo");
            assert_eq!(
                ty,
                RustType::StdCollection {
                    kind: crate::ir::StdCollectionKind::HashMap,
                    args: vec![RustType::String, RustType::F64],
                }
            );
        }
        _ => panic!("expected Item::TypeAlias, got {:?}", item),
    }
}

// --- Type alias fallback to convert_ts_type ---

#[test]
fn test_convert_type_alias_type_ref_generates_type_alias() {
    // type Params = Record<string, string> → type Params = HashMap<String, String>
    let decl = parse_type_alias("type Params = Record<string, string>;");
    let items = convert_type_alias_items(
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
            name: "Params".to_string(),
            type_params: vec![],
            ty: RustType::StdCollection {
                kind: crate::ir::StdCollectionKind::HashMap,
                args: vec![RustType::String, RustType::String],
            },
        }
    );
}

#[test]
fn test_convert_type_alias_array_generates_type_alias() {
    // type Names = string[] → type Names = Vec<String>
    let decl = parse_type_alias("type Names = string[];");
    let items = convert_type_alias_items(
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
            name: "Names".to_string(),
            type_params: vec![],
            ty: RustType::Vec(Box::new(RustType::String)),
        }
    );
}

#[test]
fn test_convert_type_alias_keyof_typeof_generates_string_union() {
    // type K = keyof typeof obj → string union of fields
    // We need "obj" registered in the registry with fields
    let mut reg = TypeRegistry::new();
    reg.register(
        "AlgorithmTypes".to_string(),
        TypeDef::new_struct(
            vec![
                ("HS256".to_string(), RustType::String).into(),
                ("RS256".to_string(), RustType::String).into(),
            ],
            std::collections::HashMap::new(),
            vec![],
        ),
    );

    let decl = parse_type_alias("type Algo = keyof typeof AlgorithmTypes;");
    let items = convert_type_alias_items(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &reg,
    )
    .unwrap();
    assert_eq!(items.len(), 1);
    // Should produce a string literal union enum
    match &items[0] {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "Algo");
            let variant_names: Vec<&str> = variants.iter().map(|v| v.name.as_str()).collect();
            assert!(
                variant_names.contains(&"HS256"),
                "expected HS256 variant, got {variant_names:?}"
            );
            assert!(
                variant_names.contains(&"RS256"),
                "expected RS256 variant, got {variant_names:?}"
            );
        }
        other => panic!("expected Enum, got {other:?}"),
    }
}

#[test]
fn test_convert_type_alias_type_literal_method_generates_trait() {
    // type X = { foo(): string } → trait X { fn foo(&self) -> String }
    let decl = parse_type_alias("type X = { foo(): string };");
    let items = convert_type_alias_items(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    // Should produce a trait (not error)
    assert!(
        items
            .iter()
            .any(|i| matches!(i, Item::Trait { name, .. } if name == "X")),
        "expected trait X, got {items:?}"
    );
}
