use super::*;

// -- convert_ts_type: array types --

#[test]
fn test_convert_ts_type_array_bracket() {
    let decl = parse_interface("interface T { x: string[]; }");
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
    assert_eq!(ty, RustType::Vec(Box::new(RustType::String)));
}

#[test]
fn test_convert_ts_type_array_generic() {
    let decl = parse_interface("interface T { x: Array<number>; }");
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
    assert_eq!(ty, RustType::Vec(Box::new(RustType::F64)));
}

// -- convert_ts_type: generic type arguments --

#[test]
fn test_convert_ts_type_named_with_type_args() {
    // `Container<string>` should become Named { name: "Container", type_args: [String] }
    let decl = parse_interface("interface T { x: Container<string>; }");
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
    assert_eq!(
        ty,
        RustType::Named {
            name: "Container".to_string(),
            type_args: vec![RustType::String],
        }
    );
}

#[test]
fn test_convert_ts_type_named_with_multiple_type_args() {
    let decl = parse_interface("interface T { x: Pair<string, number>; }");
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
    assert_eq!(
        ty,
        RustType::Named {
            name: "Pair".to_string(),
            type_args: vec![RustType::String, RustType::F64],
        }
    );
}

#[test]
fn test_convert_ts_type_named_without_type_args() {
    let decl = parse_interface("interface T { x: Point; }");
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
    assert_eq!(
        ty,
        RustType::Named {
            name: "Point".to_string(),
            type_args: vec![],
        }
    );
}

// -- convert_ts_type: function types --

#[test]
fn test_convert_ts_type_fn_type() {
    // `callback: (x: number) => string` → Fn { params: [F64], return_type: String }
    let decl = parse_interface("interface T { callback: (x: number) => string; }");
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
    assert_eq!(
        ty,
        RustType::Fn {
            params: vec![RustType::F64],
            return_type: Box::new(RustType::String),
        }
    );
}

#[test]
fn test_convert_ts_type_fn_type_no_params() {
    let decl = parse_interface("interface T { callback: () => boolean; }");
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
    assert_eq!(
        ty,
        RustType::Fn {
            params: vec![],
            return_type: Box::new(RustType::Bool),
        }
    );
}

#[test]
fn test_convert_ts_type_void_returns_unit() {
    let decl = parse_interface("interface T { callback: () => void; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    // The callback type is `() => void`, which is a TsFnType
    // whose return type is void. We check the return type is Unit.
    let ty = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    )
    .unwrap();
    assert_eq!(
        ty,
        RustType::Fn {
            params: vec![],
            return_type: Box::new(RustType::Unit),
        }
    );
}

// -- convert_ts_type: tuple types --

#[test]
fn test_convert_ts_type_tuple_two_elements() {
    let decl = parse_interface("interface T { x: [string, number]; }");
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
    assert_eq!(ty, RustType::Tuple(vec![RustType::String, RustType::F64]));
}

#[test]
fn test_convert_ts_type_tuple_single_element() {
    let decl = parse_interface("interface T { x: [boolean]; }");
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
    assert_eq!(ty, RustType::Tuple(vec![RustType::Bool]));
}

#[test]
fn test_convert_ts_type_tuple_empty() {
    let decl = parse_interface("interface T { x: []; }");
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
    assert_eq!(ty, RustType::Tuple(vec![]));
}

#[test]
fn test_convert_ts_type_tuple_nested() {
    let decl = parse_interface("interface T { x: [[string, number], boolean]; }");
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
    assert_eq!(
        ty,
        RustType::Tuple(vec![
            RustType::Tuple(vec![RustType::String, RustType::F64]),
            RustType::Bool,
        ])
    );
}

// -- indexed access types --

#[test]
fn test_convert_ts_type_indexed_access_string_key_returns_associated_type() {
    let decl = parse_interface("interface T { x: E['Bindings']; }");
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
    assert_eq!(
        ty,
        RustType::Named {
            name: "E::Bindings".to_string(),
            type_args: vec![],
        }
    );
}

#[test]
fn test_convert_ts_type_indexed_access_numeric_literal_key_returns_any() {
    let decl = parse_interface("interface T { x: E[0]; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let result = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    );
    // Numeric literal key [0] on unknown type → graceful fallback to Any
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), RustType::Any);
}

// -- Record → HashMap --

#[test]
fn test_convert_ts_type_record_string_number_returns_hashmap() {
    let decl = parse_interface("interface T { x: Record<string, number>; }");
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
    assert_eq!(
        ty,
        RustType::StdCollection {
            kind: crate::ir::StdCollectionKind::HashMap,
            args: vec![RustType::String, RustType::F64],
        }
    );
}

#[test]
fn test_convert_ts_type_record_number_value_uses_resolved_key_type() {
    let decl = parse_interface("interface T { x: Record<number, string>; }");
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
    assert_eq!(
        ty,
        RustType::StdCollection {
            kind: crate::ir::StdCollectionKind::HashMap,
            args: vec![RustType::F64, RustType::String],
        }
    );
}

// -- Readonly → inner type --

#[test]
fn test_convert_ts_type_readonly_returns_inner_type() {
    let decl = parse_interface("interface T { x: Readonly<Point>; }");
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
    assert_eq!(
        ty,
        RustType::Named {
            name: "Point".to_string(),
            type_args: vec![],
        }
    );
}

// --- readonly type operator ---

#[test]
fn test_convert_ts_type_readonly_array_returns_vec_string() {
    // readonly string[] → Vec<String>
    let decl = parse_interface("interface T { x: readonly string[]; }");
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
    assert_eq!(ty, RustType::Vec(Box::new(RustType::String)));
}

#[test]
fn test_convert_ts_type_readonly_number_array_returns_vec_f64() {
    // readonly number[] → Vec<f64>
    let decl = parse_interface("interface T { x: readonly number[]; }");
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
    assert_eq!(ty, RustType::Vec(Box::new(RustType::F64)));
}

// --- typeof type query ---

#[test]
fn test_convert_ts_type_typeof_known_fn_returns_fn_type() {
    // typeof knownFn where knownFn is registered → returns its fn type
    use crate::registry::TypeDef;
    let mut reg = TypeRegistry::new();
    reg.register(
        "myFunc".to_string(),
        TypeDef::Function {
            type_params: vec![],
            params: vec![("x".to_string(), RustType::F64).into()],
            return_type: Some(RustType::String),
            has_rest: false,
        },
    );
    let decl = parse_interface("interface T { f: typeof myFunc; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut SyntheticTypeRegistry::new(),
        &reg,
    )
    .unwrap();
    assert_eq!(
        ty,
        RustType::Fn {
            params: vec![RustType::F64],
            return_type: Box::new(RustType::String),
        }
    );
}

#[test]
fn test_convert_ts_type_typeof_unknown_returns_error() {
    // typeof unknownFn where unknownFn is NOT registered → error
    let decl = parse_interface("interface T { f: typeof unknownFn; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let result = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    );
    assert!(result.is_err(), "typeof unknown should return error");
}

// --- T2: typeof on Struct (class with constructor) ---

#[test]
fn test_convert_ts_type_typeof_struct_with_constructor_returns_fn_type() {
    // typeof MyClass where MyClass is a Struct with constructor → Fn type
    use crate::registry::{MethodSignature, TypeDef};
    let mut reg = TypeRegistry::new();
    reg.register(
        "MyClass".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![("url".to_string(), RustType::String).into()],
            methods: std::collections::HashMap::new(),
            constructor: Some(vec![MethodSignature {
                params: vec![
                    ("url".to_string(), RustType::String).into(),
                    (
                        "options".to_string(),
                        RustType::Option(Box::new(RustType::String)),
                    )
                        .into(),
                ],
                return_type: None,
                has_rest: false,
                type_params: vec![],
            }]),
            call_signatures: vec![],
            extends: vec![],
            is_interface: false,
        },
    );
    let decl = parse_interface("interface T { f: typeof MyClass; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut SyntheticTypeRegistry::new(),
        &reg,
    )
    .unwrap();
    // typeof ClassName → constructor function type: fn(params) -> ClassName
    assert_eq!(
        ty,
        RustType::Fn {
            params: vec![
                RustType::String,
                RustType::Option(Box::new(RustType::String))
            ],
            return_type: Box::new(RustType::Named {
                name: "MyClass".to_string(),
                type_args: vec![],
            }),
        }
    );
}

#[test]
fn test_convert_ts_type_typeof_struct_without_constructor_returns_named() {
    // typeof MyStruct where MyStruct has no constructor → Named type
    use crate::registry::TypeDef;
    let mut reg = TypeRegistry::new();
    reg.register(
        "MyStruct".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![("x".to_string(), RustType::F64).into()],
            methods: std::collections::HashMap::new(),
            constructor: None,
            call_signatures: vec![],
            extends: vec![],
            is_interface: true,
        },
    );
    let decl = parse_interface("interface T { f: typeof MyStruct; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut SyntheticTypeRegistry::new(),
        &reg,
    )
    .unwrap();
    assert_eq!(
        ty,
        RustType::Named {
            name: "MyStruct".to_string(),
            type_args: vec![],
        }
    );
}

#[test]
fn test_convert_ts_type_typeof_enum_returns_named() {
    // typeof MyEnum → Named type
    use crate::registry::TypeDef;
    let mut reg = TypeRegistry::new();
    reg.register(
        "Direction".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Up".to_string(), "Down".to_string()],
            string_values: std::collections::HashMap::new(),
            tag_field: None,
            variant_fields: std::collections::HashMap::new(),
        },
    );
    let decl = parse_interface("interface T { d: typeof Direction; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut SyntheticTypeRegistry::new(),
        &reg,
    )
    .unwrap();
    assert_eq!(
        ty,
        RustType::Named {
            name: "Direction".to_string(),
            type_args: vec![],
        }
    );
}

#[test]
fn test_convert_ts_type_typeof_struct_empty_constructors_returns_named() {
    // typeof MyStruct where constructor is Some(vec![]) → Named type (not Fn)
    use crate::registry::TypeDef;
    let mut reg = TypeRegistry::new();
    reg.register(
        "EmptyCtor".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![],
            methods: std::collections::HashMap::new(),
            constructor: Some(vec![]),
            call_signatures: vec![],
            extends: vec![],
            is_interface: false,
        },
    );
    let decl = parse_interface("interface T { f: typeof EmptyCtor; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut SyntheticTypeRegistry::new(),
        &reg,
    )
    .unwrap();
    assert_eq!(
        ty,
        RustType::Named {
            name: "EmptyCtor".to_string(),
            type_args: vec![],
        }
    );
}

#[test]
fn test_convert_ts_type_indexed_access_string_key_with_registry_resolves_field_type() {
    // Config['host'] where Config is registered with field 'host: String' → String
    use crate::registry::TypeDef;
    let mut reg = TypeRegistry::new();
    reg.register(
        "Config".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![
                ("host".to_string(), RustType::String).into(),
                ("port".to_string(), RustType::F64).into(),
            ],
            methods: std::collections::HashMap::new(),
            constructor: None,
            call_signatures: vec![],
            extends: vec![],
            is_interface: true,
        },
    );
    let decl = parse_interface("interface T { h: Config['host']; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut SyntheticTypeRegistry::new(),
        &reg,
    )
    .unwrap();
    // Registry-based resolution: Config['host'] → String (actual field type)
    assert_eq!(ty, RustType::String);
}

// --- T3: indexed access non-string keys ---

#[test]
fn test_convert_ts_type_indexed_access_number_keyword_key_fallback_to_any() {
    // T[number] on unknown type → graceful fallback to Any
    let decl = parse_interface("interface T { x: E[number]; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let result = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), RustType::Any);
}

#[test]
fn test_convert_ts_type_indexed_access_type_param_key_returns_any() {
    // E[K] where both E and K are unknown → graceful fallback to Any
    let decl = parse_interface("interface T { x: E[K]; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let result = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), RustType::Any);
}

#[test]
fn test_convert_ts_type_indexed_access_typeof_base_with_registered_struct() {
    // (typeof X)['host'] where X is registered as Struct with field 'host: String' → String
    use crate::registry::TypeDef;
    let mut reg = TypeRegistry::new();
    reg.register(
        "Config".to_string(),
        TypeDef::Struct {
            type_params: vec![],
            fields: vec![("host".to_string(), RustType::String).into()],
            methods: std::collections::HashMap::new(),
            constructor: None,
            call_signatures: vec![],
            extends: vec![],
            is_interface: true,
        },
    );
    let decl = parse_interface("interface T { x: (typeof Config)['host']; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let ty = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut SyntheticTypeRegistry::new(),
        &reg,
    )
    .unwrap();
    // Registry-based resolution: typeof Config → Config, Config['host'] → String
    assert_eq!(ty, RustType::String);
}

#[test]
fn test_convert_ts_type_indexed_access_typeof_base_unregistered_returns_any() {
    // (typeof unknown)['key'] where unknown is NOT registered → graceful fallback to Any
    let decl = parse_interface("interface T { x: (typeof unknown)['key']; }");
    let prop = match &decl.body.body[0] {
        TsTypeElement::TsPropertySignature(p) => p,
        _ => panic!("expected property signature"),
    };
    let result = convert_ts_type(
        &prop.type_ann.as_ref().unwrap().type_ann,
        &mut SyntheticTypeRegistry::new(),
        &TypeRegistry::new(),
    );
    // typeof on unregistered name fails → obj_name fallback → Any
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), RustType::Any);
}

#[test]
fn test_convert_ts_type_indexed_access_parenthesized_base() {
    // (SomeType)['key'] → SomeType::key (strip parens)
    let decl = parse_interface("interface T { x: (SomeType)['key']; }");
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
    assert_eq!(
        ty,
        RustType::Named {
            name: "SomeType::key".to_string(),
            type_args: vec![],
        }
    );
}
