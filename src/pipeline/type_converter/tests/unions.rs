use super::*;

// -- convert_ts_type: nullable unions --

#[test]
fn test_convert_ts_type_union_null() {
    let decl = parse_interface("interface T { x: string | null; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(ty, RustType::Option(Box::new(RustType::String)));
}

#[test]
fn test_convert_ts_type_union_undefined() {
    let decl = parse_interface("interface T { x: number | undefined; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(ty, RustType::Option(Box::new(RustType::F64)));
}

// -- convert_type_alias: string literal union --

#[test]
fn test_convert_type_alias_string_literal_union_produces_enum() {
    let decl = parse_type_alias(r#"type Direction = "up" | "down" | "left" | "right";"#);
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::Enum {
            vis,
            name,
            variants,
            ..
        } => {
            assert_eq!(vis, Visibility::Public);
            assert_eq!(name, "Direction");
            assert_eq!(variants.len(), 4);
            assert_eq!(variants[0].name, "Up");
            assert_eq!(
                variants[0].value,
                Some(crate::ir::EnumValue::Str("up".to_string()))
            );
            assert_eq!(variants[1].name, "Down");
            assert_eq!(variants[2].name, "Left");
            assert_eq!(variants[3].name, "Right");
        }
        _ => panic!("expected Item::Enum, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_string_literal_union_two_members() {
    let decl = parse_type_alias(r#"type Status = "active" | "inactive";"#);
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "Status");
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name, "Active");
            assert_eq!(variants[1].name, "Inactive");
        }
        _ => panic!("expected Item::Enum"),
    }
}

#[test]
fn test_convert_type_alias_string_literal_union_single_member() {
    let decl = parse_type_alias(r#"type Only = "only";"#);
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "Only");
            assert_eq!(variants.len(), 1);
            assert_eq!(variants[0].name, "Only");
        }
        _ => panic!("expected Item::Enum"),
    }
}

#[test]
fn test_convert_type_alias_string_literal_union_kebab_case() {
    let decl = parse_type_alias(r#"type X = "foo-bar" | "baz-qux";"#);
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::Enum { variants, .. } => {
            assert_eq!(variants[0].name, "FooBar");
            assert_eq!(variants[1].name, "BazQux");
        }
        _ => panic!("expected Item::Enum"),
    }
}

#[test]
fn test_convert_type_alias_numeric_literal_union_produces_enum() {
    let decl = parse_type_alias("type Code = 200 | 404 | 500;");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::Enum {
            vis,
            name,
            variants,
            ..
        } => {
            assert_eq!(vis, Visibility::Public);
            assert_eq!(name, "Code");
            assert_eq!(variants.len(), 3);
            assert_eq!(variants[0].name, "V200");
            assert_eq!(variants[0].value, Some(EnumValue::Number(200)));
            assert!(variants[0].data.is_none());
            assert_eq!(variants[1].name, "V404");
            assert_eq!(variants[1].value, Some(EnumValue::Number(404)));
            assert_eq!(variants[2].name, "V500");
            assert_eq!(variants[2].value, Some(EnumValue::Number(500)));
        }
        _ => panic!("expected Item::Enum, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_numeric_literal_union_two_members() {
    let decl = parse_type_alias("type Code = 200 | 404;");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "Code");
            assert_eq!(variants.len(), 2);
        }
        _ => panic!("expected Item::Enum"),
    }
}

// -- convert_type_alias: primitive union --

#[test]
fn test_convert_type_alias_primitive_union_two_types() {
    let decl = parse_type_alias("type Value = string | number;");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::Enum {
            vis,
            name,
            variants,
            ..
        } => {
            assert_eq!(vis, Visibility::Public);
            assert_eq!(name, "Value");
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name, "String");
            assert_eq!(variants[0].data, Some(RustType::String));
            assert!(variants[0].value.is_none());
            assert_eq!(variants[1].name, "F64");
            assert_eq!(variants[1].data, Some(RustType::F64));
        }
        _ => panic!("expected Item::Enum, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_primitive_union_three_types() {
    let decl = parse_type_alias("type Any = string | number | boolean;");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "Any");
            assert_eq!(variants.len(), 3);
            assert_eq!(variants[0].name, "String");
            assert_eq!(variants[1].name, "F64");
            assert_eq!(variants[2].name, "Bool");
        }
        _ => panic!("expected Item::Enum"),
    }
}

// -- convert_type_alias: mixed union --

#[test]
fn test_convert_type_alias_mixed_union_string_and_number_literal() {
    let decl = parse_type_alias(r#"type Mixed = "ok" | 404;"#);
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "Mixed");
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name, "Ok");
            assert_eq!(variants[0].value, Some(EnumValue::Str("ok".to_string())));
            assert!(variants[0].data.is_none());
            assert_eq!(variants[1].name, "V404");
            assert_eq!(variants[1].value, Some(EnumValue::Number(404)));
        }
        _ => panic!("expected Item::Enum, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_nullable_union_with_multiple_types() {
    // `type Opt = string | number | null` → enum (nullable wrapping is future work)
    let decl = parse_type_alias("type Opt = string | number | null;");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "Opt");
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name, "String");
            assert_eq!(variants[1].name, "F64");
        }
        _ => panic!("expected Item::Enum, got {:?}", item),
    }
}

// -- convert_type_alias: union with type references --

#[test]
fn test_convert_type_alias_union_type_refs_generates_data_enum() {
    let decl = parse_type_alias("type R = Success | Failure;");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::Enum {
            vis,
            name,
            variants,
            ..
        } => {
            assert_eq!(vis, Visibility::Public);
            assert_eq!(name, "R");
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name, "Success");
            assert_eq!(
                variants[0].data,
                Some(RustType::Named {
                    name: "Success".to_string(),
                    type_args: vec![],
                })
            );
            assert!(variants[0].value.is_none());
            assert_eq!(variants[1].name, "Failure");
            assert_eq!(
                variants[1].data,
                Some(RustType::Named {
                    name: "Failure".to_string(),
                    type_args: vec![],
                })
            );
        }
        _ => panic!("expected Item::Enum, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_union_type_ref_and_keyword_generates_data_enum() {
    let decl = parse_type_alias("type V = string | MyType;");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "V");
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name, "String");
            assert_eq!(variants[0].data, Some(RustType::String));
            assert_eq!(variants[1].name, "MyType");
            assert_eq!(
                variants[1].data,
                Some(RustType::Named {
                    name: "MyType".to_string(),
                    type_args: vec![],
                })
            );
        }
        _ => panic!("expected Item::Enum, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_union_generic_type_ref_generates_data_enum() {
    let decl = parse_type_alias("type R = Response | Promise<Response>;");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "R");
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name, "Response");
            assert_eq!(
                variants[0].data,
                Some(RustType::Named {
                    name: "Response".to_string(),
                    type_args: vec![],
                })
            );
            assert_eq!(variants[1].name, "Promise");
            assert_eq!(
                variants[1].data,
                Some(RustType::Named {
                    name: "Promise".to_string(),
                    type_args: vec![RustType::Named {
                        name: "Response".to_string(),
                        type_args: vec![],
                    }],
                })
            );
        }
        _ => panic!("expected Item::Enum, got {:?}", item),
    }
}

// -- nullable union type alias tests --

#[test]
fn test_convert_type_alias_nullable_single_keyword_generates_option_alias() {
    let decl = parse_type_alias("type MaybeString = string | null;");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::TypeAlias { name, ty, .. } => {
            assert_eq!(name, "MaybeString");
            assert_eq!(ty, RustType::Option(Box::new(RustType::String)));
        }
        _ => panic!("expected Item::TypeAlias, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_nullable_single_type_ref_generates_option_alias() {
    let decl = parse_type_alias("type MaybeUser = MyType | null;");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::TypeAlias { name, ty, .. } => {
            assert_eq!(name, "MaybeUser");
            assert_eq!(
                ty,
                RustType::Option(Box::new(RustType::Named {
                    name: "MyType".to_string(),
                    type_args: vec![],
                }))
            );
        }
        _ => panic!("expected Item::TypeAlias, got {:?}", item),
    }
}

#[test]
fn test_convert_type_alias_nullable_undefined_generates_option_alias() {
    let decl = parse_type_alias("type MaybeNum = number | undefined;");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::TypeAlias { name, ty, .. } => {
            assert_eq!(name, "MaybeNum");
            assert_eq!(ty, RustType::Option(Box::new(RustType::F64)));
        }
        _ => panic!("expected Item::TypeAlias, got {:?}", item),
    }
}

// -- non-nullable union, union never/void --

#[test]
fn test_convert_ts_type_non_nullable_union_generates_enum_in_extra_items() {
    let decl = parse_interface("interface T { x: string | number; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let mut extra = SyntheticTypeRegistry::new();
    let ty = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut extra,
        &TypeRegistry::new(),
    )
    .unwrap();
    // The return type should be a Named reference to the generated enum
    assert!(
        matches!(ty, RustType::Named { .. }),
        "expected Named type referencing generated enum, got: {ty:?}"
    );
    // An enum Item should be added to extra_items
    assert_eq!(
        extra.all_items().len(),
        1,
        "expected 1 extra item (enum), got: {extra:?}"
    );
    match extra.all_items()[0] {
        Item::Enum { variants, .. } => {
            assert_eq!(variants.len(), 2);
        }
        _ => panic!(
            "expected Enum in extra_items, got: {:?}",
            extra.all_items()[0]
        ),
    }
}

#[test]
fn test_convert_ts_type_union_never_simplified_to_single_type() {
    // string | never → string (never should be removed)
    let decl = parse_interface("interface X { x: string | never; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(ty, RustType::String);
}

#[test]
fn test_convert_ts_type_union_void_treated_as_nullable() {
    // string | void → Option<String> (void treated like null/undefined)
    let decl = parse_interface("interface X { x: string | void; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(ty, RustType::Option(Box::new(RustType::String)));
}

#[test]
fn test_convert_type_alias_union_void_generates_option() {
    // type X = string | void → type X = Option<String> (void filtered in type alias)
    let decl = parse_type_alias("type X = string | void;");
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
            assert_eq!(ty, RustType::Option(Box::new(RustType::String)));
        }
        other => panic!("expected TypeAlias, got {:?}", other),
    }
}

#[test]
fn test_convert_type_alias_intersection_union_complex_generates_enum() {
    let decl = parse_type_alias("type X = { a: string } & { b: number } | { c: boolean };");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match item {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "X");
            assert_eq!(variants.len(), 2);
            // First variant: intersection { a: string } & { b: number } → merged fields
            assert_eq!(variants[0].fields.len(), 2);
            let field_names: Vec<&str> =
                variants[0].fields.iter().map(|f| f.name.as_str()).collect();
            assert!(field_names.contains(&"a"));
            assert!(field_names.contains(&"b"));
            // Second variant: { c: boolean }
            assert_eq!(variants[1].fields.len(), 1);
            assert_eq!(variants[1].fields[0].name, "c");
        }
        _ => panic!("expected Item::Enum, got {:?}", item),
    }
}

// --- nullable + multi-type union tests ---

#[test]
fn test_convert_ts_type_nullable_multi_type_generates_option_enum() {
    let decl = parse_interface("interface T { x: string | number | null; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let mut extra = SyntheticTypeRegistry::new();
    let ty = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut extra,
        &TypeRegistry::new(),
    )
    .unwrap();

    match &ty {
        RustType::Option(inner) => match inner.as_ref() {
            RustType::Named { name, .. } => {
                assert_eq!(name, "F64OrString");
            }
            other => panic!("expected Named inside Option, got: {other:?}"),
        },
        other => panic!("expected Option, got: {other:?}"),
    }

    assert_eq!(
        extra.all_items().len(),
        1,
        "expected 1 extra item, got: {extra:?}"
    );
    match extra.all_items()[0] {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "F64OrString");
            assert_eq!(variants.len(), 2);
        }
        other => panic!("expected Enum, got: {other:?}"),
    }
}

#[test]
fn test_convert_ts_type_nullable_null_undefined_dedup_returns_option() {
    let decl = parse_interface("interface T { x: string | null | undefined; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let mut extra = SyntheticTypeRegistry::new();
    let ty = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut extra,
        &TypeRegistry::new(),
    )
    .unwrap();

    assert_eq!(ty, RustType::Option(Box::new(RustType::String)));
    assert!(
        extra.all_items().is_empty(),
        "no extra items expected for single type"
    );
}

#[test]
fn test_convert_ts_type_nullable_three_types_generates_option_enum() {
    let decl = parse_interface("interface T { x: boolean | string | number | null; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let mut extra = SyntheticTypeRegistry::new();
    let ty = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut extra,
        &TypeRegistry::new(),
    )
    .unwrap();

    match &ty {
        RustType::Option(inner) => match inner.as_ref() {
            RustType::Named { name, .. } => {
                assert_eq!(name, "BoolOrF64OrString");
            }
            other => panic!("expected Named inside Option, got: {other:?}"),
        },
        other => panic!("expected Option, got: {other:?}"),
    }

    assert_eq!(extra.all_items().len(), 1);
    match extra.all_items()[0] {
        Item::Enum { variants, .. } => {
            assert_eq!(variants.len(), 3);
        }
        other => panic!("expected Enum, got: {other:?}"),
    }
}

// --- prelude shadowing (I-317) ---

#[test]
fn test_union_named_result_gets_ts_prefix() {
    let decl = parse_type_alias("type Result = Success | Failure;");
    let reg = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();
    let item = crate::pipeline::type_converter::convert_type_alias(
        &decl,
        Visibility::Public,
        &mut synthetic,
        &reg,
    )
    .unwrap();
    match &item {
        Item::Enum { name, .. } => {
            assert_eq!(
                name, "TsResult",
                "type Result should be renamed to TsResult"
            );
        }
        other => panic!("expected Enum, got: {other:?}"),
    }
}

#[test]
fn test_union_named_custom_not_prefixed() {
    let decl = parse_type_alias("type Status = Active | Inactive;");
    let reg = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();
    let item = crate::pipeline::type_converter::convert_type_alias(
        &decl,
        Visibility::Public,
        &mut synthetic,
        &reg,
    )
    .unwrap();
    match &item {
        Item::Enum { name, .. } => {
            assert_eq!(name, "Status", "non-prelude type should not be prefixed");
        }
        other => panic!("expected Enum, got: {other:?}"),
    }
}

// --- convert_unsupported_union_member (indirect via convert_type_alias) ---

#[test]
fn test_union_with_function_type_produces_fn_variant() {
    // `type U = string | ((x: number) => string)` → enum with String + Fn variants
    let decl = parse_type_alias("type U = string | ((x: number) => string);");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match &item {
        Item::Enum { variants, .. } => {
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name, "String");
            assert_eq!(variants[1].name, "Fn");
            match &variants[1].data {
                Some(RustType::Fn {
                    params,
                    return_type,
                }) => {
                    assert_eq!(params.len(), 1);
                    assert_eq!(params[0], RustType::F64);
                    assert_eq!(return_type.as_ref(), &RustType::String);
                }
                other => panic!("expected Fn variant data, got: {other:?}"),
            }
        }
        other => panic!("expected Enum, got: {other:?}"),
    }
}

#[test]
fn test_union_with_tuple_type_produces_tuple_variant() {
    // `type U = string | [number, boolean]` → enum with String + Tuple variants
    let decl = parse_type_alias("type U = string | [number, boolean];");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match &item {
        Item::Enum { variants, .. } => {
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name, "String");
            assert_eq!(variants[1].name, "Tuple");
            assert_eq!(
                variants[1].data,
                Some(RustType::Tuple(vec![RustType::F64, RustType::Bool]))
            );
        }
        other => panic!("expected Enum, got: {other:?}"),
    }
}

#[test]
fn test_union_with_object_literal_produces_other_variant() {
    // Object literal in union falls to unsupported path when not all members are object types
    // `type U = string | { x: number }` → string variant + struct-like handling
    let decl = parse_type_alias("type U = string | { x: number };");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match &item {
        Item::Enum { variants, .. } => {
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name, "String");
            // The second variant gets the object type through the general union handler
        }
        other => panic!("expected Enum, got: {other:?}"),
    }
}

// --- convert_fn_type_to_rust (indirect: param without type annotation gets skipped) ---

#[test]
fn test_union_fn_type_param_without_annotation_skipped() {
    // In TS, a fn type param without annotation → skipped in convert_fn_type_to_rust
    // `(x, y: string) => void` → only y is collected
    let decl = parse_type_alias("type U = string | ((x, y: string) => void);");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match &item {
        Item::Enum { variants, .. } => {
            let fn_variant = variants.iter().find(|v| v.name == "Fn");
            assert!(fn_variant.is_some(), "expected Fn variant");
            match &fn_variant.unwrap().data {
                Some(RustType::Fn {
                    params,
                    return_type,
                }) => {
                    // x has no type annotation → skipped; only y: string is collected
                    assert_eq!(
                        params.len(),
                        1,
                        "param without annotation should be skipped"
                    );
                    assert_eq!(params[0], RustType::String);
                    assert_eq!(return_type.as_ref(), &RustType::Unit);
                }
                other => panic!("expected Fn data, got: {other:?}"),
            }
        }
        other => panic!("expected Enum, got: {other:?}"),
    }
}

/// I-040 T5: `convert_fn_type_to_rust` (utilities.rs) が embedded fn type の
/// optional パラメータ (`y?: number`) を `Option<f64>` で `RustType::Fn { params }`
/// に格納することを保証する。union member 経由で間接的に到達するパス。
#[test]
fn test_convert_fn_type_optional_param_wraps() {
    let decl = parse_type_alias("type U = string | ((x: number, y?: number) => void);");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match &item {
        Item::Enum { variants, .. } => {
            let fn_variant = variants
                .iter()
                .find(|v| v.name == "Fn")
                .expect("expected Fn variant");
            match &fn_variant.data {
                Some(RustType::Fn {
                    params,
                    return_type,
                }) => {
                    assert_eq!(params.len(), 2);
                    assert_eq!(params[0], RustType::F64);
                    assert_eq!(params[1], RustType::Option(Box::new(RustType::F64)));
                    assert_eq!(return_type.as_ref(), &RustType::Unit);
                }
                other => panic!("expected Fn data, got: {other:?}"),
            }
        }
        other => panic!("expected Enum, got: {other:?}"),
    }
}

#[test]
fn test_union_fn_type_all_params_annotated() {
    let decl = parse_type_alias("type U = string | ((a: number, b: boolean) => string);");
    let item = convert_type_alias(
        &decl,
        Visibility::Public,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    match &item {
        Item::Enum { variants, .. } => {
            let fn_variant = variants.iter().find(|v| v.name == "Fn");
            assert!(fn_variant.is_some(), "expected Fn variant");
            match &fn_variant.unwrap().data {
                Some(RustType::Fn {
                    params,
                    return_type,
                }) => {
                    assert_eq!(params.len(), 2, "all annotated params should be collected");
                    assert_eq!(params[0], RustType::F64);
                    assert_eq!(params[1], RustType::Bool);
                    assert_eq!(return_type.as_ref(), &RustType::String);
                }
                other => panic!("expected Fn data, got: {other:?}"),
            }
        }
        other => panic!("expected Enum, got: {other:?}"),
    }
}
