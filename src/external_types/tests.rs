use super::*;

/// Helper: convert an ExternalType with a fresh SyntheticTypeRegistry.
fn conv(ty: &ExternalType) -> RustType {
    convert_external_type(ty, &mut SyntheticTypeRegistry::new())
}

// ── Primitive type parsing ─────────────────────────────────────

#[test]
fn test_parse_type_string_returns_rust_string() {
    let ty: ExternalType = serde_json::from_str(r#"{"kind":"string"}"#).unwrap();
    assert_eq!(conv(&ty), RustType::String);
}

#[test]
fn test_parse_type_number_returns_f64() {
    let ty: ExternalType = serde_json::from_str(r#"{"kind":"number"}"#).unwrap();
    assert_eq!(conv(&ty), RustType::F64);
}

#[test]
fn test_parse_type_boolean_returns_bool() {
    let ty: ExternalType = serde_json::from_str(r#"{"kind":"boolean"}"#).unwrap();
    assert_eq!(conv(&ty), RustType::Bool);
}

#[test]
fn test_parse_type_void_returns_unit() {
    let ty: ExternalType = serde_json::from_str(r#"{"kind":"void"}"#).unwrap();
    assert_eq!(conv(&ty), RustType::Unit);
}

#[test]
fn test_parse_type_any_returns_any() {
    let ty: ExternalType = serde_json::from_str(r#"{"kind":"any"}"#).unwrap();
    assert_eq!(conv(&ty), RustType::Any);
}

#[test]
fn test_parse_type_unknown_returns_any() {
    let ty: ExternalType = serde_json::from_str(r#"{"kind":"unknown"}"#).unwrap();
    assert_eq!(conv(&ty), RustType::Any);
}

#[test]
fn test_parse_type_never_returns_never() {
    let ty: ExternalType = serde_json::from_str(r#"{"kind":"never"}"#).unwrap();
    assert_eq!(conv(&ty), RustType::Never);
}

// ── Composite type parsing ─────────────────────────────────────

#[test]
fn test_parse_type_nullable_returns_option() {
    let ty: ExternalType =
        serde_json::from_str(r#"{"kind":"union","members":[{"kind":"string"},{"kind":"null"}]}"#)
            .unwrap();
    assert_eq!(conv(&ty), RustType::Option(Box::new(RustType::String)));
}

#[test]
fn test_parse_type_array_returns_vec() {
    let ty: ExternalType =
        serde_json::from_str(r#"{"kind":"array","element":{"kind":"number"}}"#).unwrap();
    assert_eq!(conv(&ty), RustType::Vec(Box::new(RustType::F64)));
}

#[test]
fn test_parse_type_tuple_returns_tuple() {
    let ty: ExternalType = serde_json::from_str(
        r#"{"kind":"tuple","elements":[{"kind":"string"},{"kind":"number"}]}"#,
    )
    .unwrap();
    assert_eq!(
        conv(&ty),
        RustType::Tuple(vec![RustType::String, RustType::F64])
    );
}

#[test]
fn test_parse_type_named_returns_named() {
    let ty: ExternalType = serde_json::from_str(r#"{"kind":"named","name":"Response"}"#).unwrap();
    assert_eq!(
        conv(&ty),
        RustType::Named {
            name: "Response".to_string(),
            type_args: vec![],
        }
    );
}

#[test]
fn test_parse_type_named_with_type_args() {
    let ty: ExternalType =
        serde_json::from_str(r#"{"kind":"named","name":"Promise","type_args":[{"kind":"any"}]}"#)
            .unwrap();
    assert_eq!(
        conv(&ty),
        RustType::Named {
            name: "Promise".to_string(),
            type_args: vec![RustType::Any],
        }
    );
}

#[test]
fn test_parse_type_function_returns_fn() {
    let ty: ExternalType = serde_json::from_str(
        r#"{"kind":"function","params":[{"kind":"string"}],"return_type":{"kind":"number"}}"#,
    )
    .unwrap();
    assert_eq!(
        conv(&ty),
        RustType::Fn {
            params: vec![RustType::String],
            return_type: Box::new(RustType::F64),
        }
    );
}

// ── Interface → TypeDef::Struct ────────────────────────────────

#[test]
fn test_parse_interface_returns_struct_typedef() {
    let json = r#"{
        "kind": "interface",
        "fields": [
            {"name": "status", "type": {"kind": "number"}, "readonly": true},
            {"name": "ok", "type": {"kind": "boolean"}, "readonly": true}
        ],
        "methods": {
            "clone": {
                "signatures": [
                    {"params": [], "return_type": {"kind": "named", "name": "Response"}}
                ]
            }
        }
    }"#;
    let def: ExternalTypeDef = serde_json::from_str(json).unwrap();
    let type_def = convert_external_typedef(&def, &mut SyntheticTypeRegistry::new()).unwrap();
    match type_def {
        TypeDef::Struct {
            fields, methods, ..
        } => {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].0, "status");
            assert_eq!(fields[0].1, RustType::F64);
            assert_eq!(fields[1].0, "ok");
            assert_eq!(fields[1].1, RustType::Bool);
            assert!(methods.contains_key("clone"));
        }
        _ => panic!("expected Struct, got {type_def:?}"),
    }
}

#[test]
fn test_parse_interface_with_inherited_methods() {
    // tsc resolves inheritance, so Body's methods appear directly on Response
    let json = r#"{
        "kind": "interface",
        "fields": [
            {"name": "status", "type": {"kind": "number"}}
        ],
        "methods": {
            "json": {
                "signatures": [
                    {"params": [], "return_type": {"kind": "named", "name": "Promise", "type_args": [{"kind": "any"}]}}
                ]
            },
            "text": {
                "signatures": [
                    {"params": [], "return_type": {"kind": "named", "name": "Promise", "type_args": [{"kind": "string"}]}}
                ]
            }
        }
    }"#;
    let def: ExternalTypeDef = serde_json::from_str(json).unwrap();
    let type_def = convert_external_typedef(&def, &mut SyntheticTypeRegistry::new()).unwrap();
    match type_def {
        TypeDef::Struct { methods, .. } => {
            assert!(methods.contains_key("json"), "should have inherited json()");
            assert!(methods.contains_key("text"), "should have inherited text()");
        }
        _ => panic!("expected Struct"),
    }
}

#[test]
fn test_parse_interface_optional_field_becomes_option() {
    let json = r#"{
        "kind": "interface",
        "fields": [
            {"name": "status", "type": {"kind": "number"}, "optional": true}
        ],
        "methods": {}
    }"#;
    let def: ExternalTypeDef = serde_json::from_str(json).unwrap();
    let type_def = convert_external_typedef(&def, &mut SyntheticTypeRegistry::new()).unwrap();
    match type_def {
        TypeDef::Struct { fields, .. } => {
            assert_eq!(fields[0].1, RustType::Option(Box::new(RustType::F64)));
        }
        _ => panic!("expected Struct"),
    }
}

// ── Function → TypeDef::Function ───────────────────────────────

#[test]
fn test_parse_function_returns_function_typedef() {
    let json = r#"{
        "kind": "function",
        "signatures": [
            {
                "params": [
                    {"name": "input", "type": {"kind": "string"}},
                    {"name": "init", "type": {"kind": "named", "name": "RequestInit"}, "optional": true}
                ],
                "return_type": {"kind": "named", "name": "Promise", "type_args": [{"kind": "named", "name": "Response"}]}
            }
        ]
    }"#;
    let def: ExternalTypeDef = serde_json::from_str(json).unwrap();
    let type_def = convert_external_typedef(&def, &mut SyntheticTypeRegistry::new()).unwrap();
    match type_def {
        TypeDef::Function {
            params,
            return_type,
            has_rest,
        } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].0, "input");
            assert_eq!(params[0].1, RustType::String);
            assert_eq!(params[1].0, "init");
            assert_eq!(
                params[1].1,
                RustType::Option(Box::new(RustType::Named {
                    name: "RequestInit".to_string(),
                    type_args: vec![],
                }))
            );
            assert!(return_type.is_some());
            assert!(!has_rest);
        }
        _ => panic!("expected Function"),
    }
}

// ── load_types_json ────────────────────────────────────────────

#[test]
fn test_load_types_json_registers_all_types() {
    let json = r#"{
        "version": 2,
        "types": {
            "Response": {
                "kind": "interface",
                "fields": [{"name": "status", "type": {"kind": "number"}}],
                "methods": {}
            },
            "Headers": {
                "kind": "interface",
                "fields": [],
                "methods": {
                    "get": {
                        "signatures": [
                            {"params": [{"name": "name", "type": {"kind": "string"}}], "return_type": {"kind": "union", "members": [{"kind": "string"}, {"kind": "null"}]}}
                        ]
                    }
                }
            }
        }
    }"#;
    let (registry, _synthetic) = load_types_json(json).unwrap();
    assert!(registry.get("Response").is_some());
    assert!(registry.get("Headers").is_some());
}

#[test]
fn test_load_types_json_invalid_json_returns_error() {
    let result = load_types_json("{not valid json}");
    assert!(result.is_err());
}

#[test]
fn test_load_types_json_version_mismatch_returns_error() {
    let json = r#"{"version": 99, "types": {}}"#;
    let result = load_types_json(json);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("version"),
        "error should mention version: {err}"
    );
}

#[test]
fn test_load_types_json_union_registers_synthetic_enum() {
    let json = r#"{
        "version": 2,
        "types": {
            "Formatter": {
                "kind": "interface",
                "fields": [],
                "methods": {
                    "format": {
                        "signatures": [
                            {
                                "params": [{"name": "input", "type": {"kind": "union", "members": [{"kind": "string"}, {"kind": "number"}]}}],
                                "return_type": {"kind": "string"}
                            }
                        ]
                    }
                }
            }
        }
    }"#;
    let (_registry, synthetic) = load_types_json(json).unwrap();
    // The union {string | number} should be registered as a synthetic enum
    let has_enum = synthetic
        .all_items()
        .iter()
        .any(|item| matches!(item, crate::ir::Item::Enum { name, .. } if name == "F64OrString"));
    assert!(
        has_enum,
        "SyntheticTypeRegistry should contain F64OrString enum from union type"
    );
}

#[test]
fn test_merge_external_types_local_takes_precedence() {
    let external_json = r#"{
        "version": 2,
        "types": {
            "Foo": {
                "kind": "interface",
                "fields": [{"name": "external_field", "type": {"kind": "string"}}],
                "methods": {}
            }
        }
    }"#;
    let (external_reg, _) = load_types_json(external_json).unwrap();

    // Simulate local file defining Foo differently
    let local_source = "interface Foo { local_field: number; }";
    let module = crate::parser::parse_typescript(local_source).unwrap();
    let mut local_reg = crate::registry::build_registry(&module);
    // Merge external first, then local overwrites
    local_reg.merge(&external_reg);
    // Re-merge local on top (this is how transpile_with_registry works:
    // local reg merges shared, so local definitions win)
    let local_reg2 = crate::registry::build_registry(&module);
    local_reg.merge(&local_reg2);

    let foo = local_reg.get("Foo").unwrap();
    match foo {
        TypeDef::Struct { fields, .. } => {
            // Local definition should be present
            assert!(
                fields.iter().any(|(name, _)| name == "local_field"),
                "local definition should take precedence, got: {fields:?}"
            );
        }
        _ => panic!("expected Struct"),
    }
}

// ── Built-in types ─────────────────────────────────────────────

#[test]
fn test_load_builtin_types_succeeds() {
    let (registry, _) = load_builtin_types().unwrap();
    // Should contain key Web API types
    assert!(
        registry.get("Response").is_some(),
        "builtin types should contain Response"
    );
    assert!(
        registry.get("Request").is_some(),
        "builtin types should contain Request"
    );
    assert!(
        registry.get("Headers").is_some(),
        "builtin types should contain Headers"
    );
    assert!(
        registry.get("URL").is_some(),
        "builtin types should contain URL"
    );
}

#[test]
fn test_builtin_response_has_status_field() {
    let (registry, _) = load_builtin_types().unwrap();
    let response = registry.get("Response").unwrap();
    match response {
        TypeDef::Struct { fields, .. } => {
            assert!(
                fields.iter().any(|(name, _)| name == "status"),
                "Response should have status field, got: {fields:?}"
            );
        }
        _ => panic!("Response should be a Struct"),
    }
}

#[test]
fn test_builtin_response_has_inherited_body_methods() {
    let (registry, _) = load_builtin_types().unwrap();
    let response = registry.get("Response").unwrap();
    match response {
        TypeDef::Struct { methods, .. } => {
            assert!(
                methods.contains_key("json"),
                "Response should have json() from Body, got methods: {:?}",
                methods.keys().collect::<Vec<_>>()
            );
            assert!(
                methods.contains_key("text"),
                "Response should have text() from Body, got methods: {:?}",
                methods.keys().collect::<Vec<_>>()
            );
        }
        _ => panic!("Response should be a Struct"),
    }
}

#[test]
fn test_builtin_headers_has_get_method() {
    let (registry, _) = load_builtin_types().unwrap();
    let headers = registry.get("Headers").unwrap();
    match headers {
        TypeDef::Struct { methods, .. } => {
            assert!(
                methods.contains_key("get"),
                "Headers should have get() method"
            );
            assert!(
                methods.contains_key("set"),
                "Headers should have set() method"
            );
            assert!(
                methods.contains_key("append"),
                "Headers should have append() method"
            );
        }
        _ => panic!("Headers should be a Struct"),
    }
}

// ── ECMAScript built-in types ─────────────────────────────────

#[test]
fn test_load_builtin_types_contains_ecmascript_string() {
    let (registry, _) = load_builtin_types().unwrap();
    let string_type = registry
        .get("String")
        .expect("builtin types should contain String");
    match string_type {
        TypeDef::Struct { methods, .. } => {
            assert!(methods.contains_key("trim"), "String should have trim()");
            assert!(methods.contains_key("split"), "String should have split()");
            assert!(
                methods.contains_key("toLowerCase"),
                "String should have toLowerCase()"
            );
        }
        _ => panic!("String should be a Struct"),
    }
}

#[test]
fn test_load_builtin_types_contains_ecmascript_array() {
    let (registry, _) = load_builtin_types().unwrap();
    let array_type = registry
        .get("Array")
        .expect("builtin types should contain Array");
    match array_type {
        TypeDef::Struct { methods, .. } => {
            assert!(methods.contains_key("map"), "Array should have map()");
            assert!(methods.contains_key("filter"), "Array should have filter()");
            assert!(methods.contains_key("find"), "Array should have find()");
        }
        _ => panic!("Array should be a Struct"),
    }
}

#[test]
fn test_load_builtin_types_contains_ecmascript_date() {
    let (registry, _) = load_builtin_types().unwrap();
    assert!(
        registry.get("Date").is_some(),
        "builtin types should contain Date"
    );
}

#[test]
fn test_load_builtin_types_contains_ecmascript_error() {
    let (registry, _) = load_builtin_types().unwrap();
    assert!(
        registry.get("Error").is_some(),
        "builtin types should contain Error"
    );
}

#[test]
fn test_load_builtin_types_contains_ecmascript_map() {
    let (registry, _) = load_builtin_types().unwrap();
    let map_type = registry
        .get("Map")
        .expect("builtin types should contain Map");
    match map_type {
        TypeDef::Struct { methods, .. } => {
            assert!(methods.contains_key("get"), "Map should have get()");
            assert!(methods.contains_key("set"), "Map should have set()");
            assert!(methods.contains_key("has"), "Map should have has()");
        }
        _ => panic!("Map should be a Struct"),
    }
}

#[test]
fn test_load_builtin_types_contains_ecmascript_set() {
    let (registry, _) = load_builtin_types().unwrap();
    let set_type = registry
        .get("Set")
        .expect("builtin types should contain Set");
    match set_type {
        TypeDef::Struct { methods, .. } => {
            assert!(methods.contains_key("add"), "Set should have add()");
            assert!(methods.contains_key("has"), "Set should have has()");
            assert!(methods.contains_key("delete"), "Set should have delete()");
        }
        _ => panic!("Set should be a Struct"),
    }
}

#[test]
fn test_load_builtin_types_web_api_still_present_after_ecmascript_addition() {
    let (registry, _) = load_builtin_types().unwrap();
    // Web API types should still be loaded
    assert!(
        registry.get("Response").is_some(),
        "Response should still be present"
    );
    assert!(
        registry.get("Request").is_some(),
        "Request should still be present"
    );
    assert!(
        registry.get("Headers").is_some(),
        "Headers should still be present"
    );
}

#[test]
fn test_load_types_into_merges_into_existing_registry() {
    let json_a = r#"{
        "version": 2,
        "types": {
            "Foo": {
                "kind": "interface",
                "fields": [{"name": "x", "type": {"kind": "string"}}],
                "methods": {}
            }
        }
    }"#;
    let json_b = r#"{
        "version": 2,
        "types": {
            "Bar": {
                "kind": "interface",
                "fields": [{"name": "y", "type": {"kind": "number"}}],
                "methods": {}
            }
        }
    }"#;
    let mut registry = TypeRegistry::new();
    let mut synthetic = SyntheticTypeRegistry::new();
    load_types_into(&mut registry, &mut synthetic, json_a).unwrap();
    load_types_into(&mut registry, &mut synthetic, json_b).unwrap();
    assert!(registry.get("Foo").is_some(), "Foo should be present");
    assert!(registry.get("Bar").is_some(), "Bar should be present");
}

// ── Multiple signatures (overloads) ───────────────────────────

#[test]
fn test_parse_interface_multiple_signatures_all_stored() {
    let json = r#"{
        "kind": "interface",
        "fields": [],
        "methods": {
            "from": {
                "signatures": [
                    {"params": [{"name": "iterable", "type": {"kind": "any"}}], "return_type": {"kind": "named", "name": "Array"}},
                    {"params": [{"name": "iterable", "type": {"kind": "any"}}, {"name": "mapfn", "type": {"kind": "any"}}], "return_type": {"kind": "named", "name": "Array"}}
                ]
            }
        }
    }"#;
    let def: ExternalTypeDef = serde_json::from_str(json).unwrap();
    let mut synthetic = crate::pipeline::SyntheticTypeRegistry::new();
    let type_def = convert_external_typedef(&def, &mut synthetic).unwrap();
    match type_def {
        TypeDef::Struct { methods, .. } => {
            let sigs = methods.get("from").expect("from method should exist");
            assert_eq!(sigs.len(), 2, "should store all overload signatures");
            assert_eq!(sigs[0].params.len(), 1);
            assert_eq!(sigs[1].params.len(), 2);
        }
        _ => panic!("expected Struct"),
    }
}

// ── Union → synthetic enum ────────────────────────────────────

#[test]
fn test_convert_union_multi_member_returns_named_synthetic_enum() {
    let mut synthetic = crate::pipeline::SyntheticTypeRegistry::new();
    let members = vec![ExternalType::String, ExternalType::Number];
    let result = convert_union_type(&members, &mut synthetic);
    // register_union sorts variants alphabetically, so F64 comes before String
    let expected_name = "F64OrString";
    match &result {
        RustType::Named { name, .. } => {
            assert_eq!(name, expected_name);
        }
        _ => panic!("expected Named (synthetic enum), got: {result:?}"),
    }
    // Verify the enum is registered in synthetic
    assert!(
        synthetic.all_items().iter().any(|item| {
            if let crate::ir::Item::Enum { name, .. } = item {
                name == expected_name
            } else {
                false
            }
        }),
        "{expected_name} should be registered in SyntheticTypeRegistry"
    );
}

#[test]
fn test_convert_union_nullable_preserves_option_pattern() {
    let mut synthetic = crate::pipeline::SyntheticTypeRegistry::new();
    let members = vec![ExternalType::String, ExternalType::Null];
    let result = convert_union_type(&members, &mut synthetic);
    assert_eq!(result, RustType::Option(Box::new(RustType::String)));
}

#[test]
fn test_convert_union_nullable_multi_member_returns_option_named() {
    let mut synthetic = crate::pipeline::SyntheticTypeRegistry::new();
    let members = vec![
        ExternalType::String,
        ExternalType::Number,
        ExternalType::Null,
    ];
    let result = convert_union_type(&members, &mut synthetic);
    match &result {
        RustType::Option(inner) => match inner.as_ref() {
            RustType::Named { name, .. } => {
                assert_eq!(name, "F64OrString");
            }
            _ => panic!("expected Named inside Option, got: {inner:?}"),
        },
        _ => panic!("expected Option, got: {result:?}"),
    }
}

// ── type_params tests ────────────────────────────────────────────

#[test]
fn test_external_type_params_deserialized() {
    let json = r#"{
        "version": 2,
        "types": {
            "Container": {
                "kind": "interface",
                "type_params": [{"name": "T"}],
                "fields": [{"name": "value", "type": {"kind": "named", "name": "T"}}],
                "methods": {}
            }
        }
    }"#;
    let (registry, _) = load_types_json(json).unwrap();
    let typedef = registry.get("Container").unwrap();
    assert_eq!(typedef.type_params().len(), 1);
    assert_eq!(typedef.type_params()[0].name, "T");
    assert!(typedef.type_params()[0].constraint.is_none());
}

#[test]
fn test_external_type_params_with_constraint() {
    let json = r#"{
        "version": 2,
        "types": {
            "Bounded": {
                "kind": "interface",
                "type_params": [{"name": "T", "constraint": {"kind": "named", "name": "Foo"}}],
                "fields": [],
                "methods": {}
            }
        }
    }"#;
    let (registry, _) = load_types_json(json).unwrap();
    let typedef = registry.get("Bounded").unwrap();
    assert_eq!(typedef.type_params().len(), 1);
    assert_eq!(typedef.type_params()[0].name, "T");
    assert!(matches!(
        &typedef.type_params()[0].constraint,
        Some(RustType::Named { name, .. }) if name == "Foo"
    ));
}

#[test]
fn test_external_type_params_absent_defaults_empty() {
    let json = r#"{
        "version": 2,
        "types": {
            "Simple": {
                "kind": "interface",
                "fields": [{"name": "x", "type": {"kind": "number"}}],
                "methods": {}
            }
        }
    }"#;
    let (registry, _) = load_types_json(json).unwrap();
    let typedef = registry.get("Simple").unwrap();
    assert!(typedef.type_params().is_empty());
}

#[test]
fn test_load_interface_with_constructors() {
    let json = r#"{
        "version": 2,
        "types": {
            "MyClass": {
                "kind": "interface",
                "fields": [
                    {"name": "running", "type": {"kind": "boolean"}}
                ],
                "methods": {},
                "constructors": [
                    {
                        "params": [
                            {"name": "name", "type": {"kind": "string"}},
                            {"name": "port", "type": {"kind": "number"}}
                        ],
                        "return_type": {"kind": "named", "name": "MyClass"}
                    }
                ]
            }
        }
    }"#;
    let (registry, _) = load_types_json(json).unwrap();
    let typedef = registry.get("MyClass").unwrap();
    match typedef {
        TypeDef::Struct { constructor, .. } => {
            let sigs = constructor
                .as_ref()
                .expect("should have constructor signatures");
            assert_eq!(sigs.len(), 1);
            assert_eq!(sigs[0].params.len(), 2);
            assert_eq!(sigs[0].params[0].0, "name");
            assert_eq!(sigs[0].params[0].1, RustType::String);
            assert_eq!(sigs[0].params[1].0, "port");
            assert_eq!(sigs[0].params[1].1, RustType::F64);
        }
        _ => panic!("expected Struct"),
    }
}

#[test]
fn test_builtin_response_has_constructor() {
    // Verify that the builtin Response type has constructor signatures loaded
    let (registry, _synthetic) = load_builtin_types().unwrap();
    let typedef = registry
        .get("Response")
        .expect("Response should be registered");
    match typedef {
        TypeDef::Struct { constructor, .. } => {
            let sigs = constructor
                .as_ref()
                .expect("Response should have constructor signatures");
            assert!(!sigs.is_empty(), "should have at least one constructor");
            // Response constructor: (body?, init?: ResponseInit)
            let sig = &sigs[0];
            assert_eq!(sig.params.len(), 2, "Response constructor has 2 params");
            assert_eq!(sig.params[1].0, "init");
            // init param type should be Named("ResponseInit")
            match &sig.params[1].1 {
                RustType::Named { name, .. } => assert_eq!(name, "ResponseInit"),
                other => panic!("expected Named(ResponseInit), got {other:?}"),
            }
        }
        _ => panic!("expected Struct"),
    }
}
