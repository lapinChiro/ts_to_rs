use super::*;

// ── TypeRegistry::lookup_method_sigs / lookup_field_type / resolve_type_def ──

#[test]
fn test_lookup_method_sigs_named_type() {
    let mut reg = TypeRegistry::new();
    let mut methods = HashMap::new();
    methods.insert(
        "greet".to_string(),
        vec![MethodSignature {
            params: vec![("msg".to_string(), RustType::String).into()],
            return_type: None,
            has_rest: false,
        }],
    );
    reg.register(
        "Greeter".to_string(),
        TypeDef::new_interface(vec![], vec![], methods, vec![]),
    );

    let result = reg.lookup_method_sigs(
        &RustType::Named {
            name: "Greeter".to_string(),
            type_args: vec![],
        },
        "greet",
    );
    assert!(result.is_some(), "should find method on Named type");
    assert_eq!(result.unwrap().len(), 1);
}

#[test]
fn test_lookup_method_sigs_string_type() {
    let mut reg = TypeRegistry::new();
    let mut methods = HashMap::new();
    methods.insert(
        "charAt".to_string(),
        vec![MethodSignature {
            params: vec![("pos".to_string(), RustType::F64).into()],
            return_type: Some(RustType::String),
            has_rest: false,
        }],
    );
    reg.register(
        "String".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods,
            constructor: None,
            call_signatures: vec![],
            extends: vec![],
            is_interface: true,
        },
    );

    let result = reg.lookup_method_sigs(&RustType::String, "charAt");
    assert!(
        result.is_some(),
        "should find method on RustType::String via String interface"
    );
}

#[test]
fn test_lookup_method_sigs_vec_type() {
    let mut reg = TypeRegistry::new();
    let mut methods = HashMap::new();
    methods.insert(
        "push".to_string(),
        vec![MethodSignature {
            params: vec![(
                "items".to_string(),
                RustType::Vec(Box::new(RustType::Named {
                    name: "T".to_string(),
                    type_args: vec![],
                })),
            )
                .into()],
            return_type: Some(RustType::F64),
            has_rest: true,
        }],
    );
    reg.register(
        "Array".to_string(),
        TypeDef::Struct {
            type_params: vec![crate::ir::TypeParam {
                name: "T".to_string(),
                constraint: None,
            }],
            fields: vec![],
            methods,
            constructor: None,
            call_signatures: vec![],
            extends: vec![],
            is_interface: true,
        },
    );

    let result = reg.lookup_method_sigs(&RustType::Vec(Box::new(RustType::F64)), "push");
    assert!(
        result.is_some(),
        "should find method on Vec via Array→instantiate"
    );
}

#[test]
fn test_lookup_method_sigs_unknown_type_returns_none() {
    let reg = TypeRegistry::new();
    assert!(reg.lookup_method_sigs(&RustType::F64, "foo").is_none());
    assert!(reg.lookup_method_sigs(&RustType::Bool, "foo").is_none());
}

#[test]
fn test_lookup_field_type_named_struct() {
    let mut reg = TypeRegistry::new();
    reg.register(
        "Point".to_string(),
        TypeDef::new_struct(
            vec![
                ("x".to_string(), RustType::F64).into(),
                ("y".to_string(), RustType::F64).into(),
            ],
            Default::default(),
            vec![],
        ),
    );

    let result = reg.lookup_field_type(
        &RustType::Named {
            name: "Point".to_string(),
            type_args: vec![],
        },
        "x",
    );
    assert_eq!(result, Some(RustType::F64));

    // Non-existent field
    assert!(reg
        .lookup_field_type(
            &RustType::Named {
                name: "Point".to_string(),
                type_args: vec![],
            },
            "z",
        )
        .is_none());
}

#[test]
fn test_lookup_field_type_unknown_type_returns_none() {
    let reg = TypeRegistry::new();
    assert!(reg.lookup_field_type(&RustType::F64, "length").is_none());
}
