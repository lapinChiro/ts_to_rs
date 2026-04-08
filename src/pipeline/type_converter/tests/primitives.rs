use super::*;

// -- convert_ts_type: basic primitive types --

#[test]
fn test_convert_ts_type_string() {
    let decl = parse_interface("interface T { x: string; }");
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
fn test_convert_ts_type_number() {
    let decl = parse_interface("interface T { x: number; }");
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
    assert_eq!(ty, RustType::F64);
}

#[test]
fn test_convert_ts_type_boolean() {
    let decl = parse_interface("interface T { x: boolean; }");
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
    assert_eq!(ty, RustType::Bool);
}

// -- convert_ts_type: keyword types (any, unknown, never) --

#[test]
fn test_convert_ts_type_any() {
    let decl = parse_interface("interface T { x: any; }");
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
    assert_eq!(ty, RustType::Any);
}

#[test]
fn test_convert_ts_type_unknown() {
    let decl = parse_interface("interface T { x: unknown; }");
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
    assert_eq!(ty, RustType::Any);
}

#[test]
fn test_convert_ts_type_never() {
    let decl = parse_interface("interface T { x: never; }");
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
    assert_eq!(ty, RustType::Never);
}

// -- object keyword --

#[test]
fn test_convert_ts_type_object_keyword_returns_serde_json_value() {
    let decl = parse_interface("interface T { x: object; }");
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
            name: "serde_json::Value".to_string(),
            type_args: vec![],
        }
    );
}

// -- TsLitType in annotation position --

#[test]
fn test_convert_ts_type_lit_string_returns_string() {
    let decl = parse_interface("interface T { x: \"hello\"; }");
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
fn test_convert_ts_type_lit_bool_true_returns_bool() {
    let decl = parse_interface("interface T { x: true; }");
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
    assert_eq!(ty, RustType::Bool);
}

#[test]
fn test_convert_ts_type_lit_bool_false_returns_bool() {
    let decl = parse_interface("interface T { x: false; }");
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
    assert_eq!(ty, RustType::Bool);
}

#[test]
fn test_convert_ts_type_lit_number_returns_f64() {
    let decl = parse_interface("interface T { x: 42; }");
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
    assert_eq!(ty, RustType::F64);
}

// -- TsConditionalType in annotation position --

#[test]
fn test_convert_ts_type_conditional_true_branch_returns_type() {
    let decl = parse_interface("interface T { x: string extends object ? boolean : number; }");
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
    assert_eq!(ty, RustType::Bool);
}

#[test]
fn test_convert_ts_type_conditional_bool_predicate_returns_bool() {
    let decl = parse_interface("interface T { x: string extends object ? true : false; }");
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
    assert_eq!(ty, RustType::Bool);
}

// -- TsTypePredicate → Bool --

#[test]
fn test_convert_ts_type_predicate_returns_bool() {
    // `(x: any) => x is string` is a function type whose return is TsTypePredicate
    let decl = parse_type_alias("type Guard = (x: any) => x is string;");
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
                    params: vec![RustType::Any],
                    return_type: Box::new(RustType::Bool),
                }
            );
        }
        _ => panic!("expected Item::TypeAlias, got {:?}", item),
    }
}

// -- TsUndefinedKeyword → Unit --

#[test]
fn test_convert_ts_type_undefined_keyword_returns_unit() {
    let decl = parse_interface("interface T { x: undefined; }");
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
    assert_eq!(ty, RustType::Unit);
}

// -- TsNullKeyword → Unit --

#[test]
fn test_convert_ts_type_null_keyword_returns_unit() {
    let decl = parse_interface("interface T { x: null; }");
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
    assert_eq!(ty, RustType::Unit);
}

// -- TsBigIntKeyword → i128 --

#[test]
fn test_convert_ts_type_bigint_keyword_returns_i128() {
    let decl = parse_interface("interface T { x: bigint; }");
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
    assert_eq!(ty, RustType::Primitive(crate::ir::PrimitiveIntKind::I128));
}
