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
fn test_convert_ts_type_indexed_access_non_string_key_returns_error() {
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
    assert!(result.is_err());
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
        RustType::Named {
            name: "HashMap".to_string(),
            type_args: vec![RustType::String, RustType::F64],
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
            params: vec![("x".to_string(), RustType::F64)],
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
