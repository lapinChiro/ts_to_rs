use super::*;

// ---- variable type annotation propagation to arrow return type ----

#[test]
fn test_transform_var_type_arrow_propagates_return_type() {
    let source = r#"
        interface Point { x: number; y: number; }
        export const make: (n: number) => Point = (n: number) => {
            return { x: n, y: 0 };
        };
    "#;
    let f = TctxFixture::from_source(source);
    let (items, _) = f.transform(source);

    let fn_item = items
        .iter()
        .find(|i| matches!(i, Item::Fn { name, .. } if name == "make"));
    assert!(fn_item.is_some(), "expected fn make, got: {items:?}");
    match fn_item.unwrap() {
        Item::Fn { return_type, .. } => {
            assert_eq!(
                *return_type,
                Some(RustType::Named {
                    name: "Point".to_string(),
                    type_args: vec![],
                })
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn test_transform_var_type_alias_arrow_propagates_return_type() {
    let source = r#"
        interface Info { name: string; }
        type GetInfo = (key: string) => Info;
        export const getInfo: GetInfo = (key: string) => {
            return { name: key };
        };
    "#;
    let f = TctxFixture::from_source(source);
    let (items, _) = f.transform(source);

    let fn_item = items
        .iter()
        .find(|i| matches!(i, Item::Fn { name, .. } if name == "getInfo"));
    assert!(fn_item.is_some(), "expected fn getInfo");
    match fn_item.unwrap() {
        Item::Fn { return_type, .. } => {
            assert_eq!(
                *return_type,
                Some(RustType::Named {
                    name: "Info".to_string(),
                    type_args: vec![],
                })
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn test_transform_var_arrow_explicit_return_type_takes_priority() {
    let source = r#"
        const f: (x: number) => string = (x: number): number => {
            return x;
        };
    "#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    match &items[0] {
        Item::Fn { return_type, .. } => {
            assert_eq!(*return_type, Some(RustType::F64));
        }
        _ => panic!("expected Item::Fn"),
    }
}

// ---- param type inference from variable annotation ----

#[test]
fn test_transform_var_arrow_infers_param_types_from_variable_annotation() {
    // const f: (x: number, y: string) => void = (x, y) => { ... }
    // → fn f(x: f64, y: String) { ... }
    let source = r#"
        const f: (x: number, y: string) => void = (x, y) => {
            console.log(x);
        };
    "#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    match &items[0] {
        Item::Fn { params, .. } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "x");
            assert_eq!(params[0].ty, Some(RustType::F64));
            assert_eq!(params[1].name, "y");
            assert_eq!(params[1].ty, Some(RustType::String));
        }
        _ => panic!("expected Item::Fn"),
    }
}

#[test]
fn test_transform_var_arrow_infers_param_types_from_named_type_alias() {
    // type Handler = (c: Context) => ConnInfo
    // const getInfo: Handler = (c) => { ... }
    // → fn getInfo(c: Context) -> ConnInfo { ... }
    let source = r#"
        type Handler = (c: string) => number;
        const getInfo: Handler = (c) => {
            return 0;
        };
    "#;
    let module = parse_typescript(source).expect("parse failed");
    let reg = crate::registry::build_registry(&module);
    let items = transform_module(&module, &reg).unwrap();

    let fn_item = items
        .iter()
        .find(|i| matches!(i, Item::Fn { name, .. } if name == "getInfo"));
    assert!(fn_item.is_some(), "expected fn getInfo");
    match fn_item.unwrap() {
        Item::Fn { params, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "c");
            assert_eq!(params[0].ty, Some(RustType::String));
        }
        _ => unreachable!(),
    }
}

#[test]
fn test_transform_var_arrow_explicit_param_type_not_overridden() {
    // Explicit param annotation should NOT be overridden by variable type
    let source = r#"
        const f: (x: number) => void = (x: string) => {
            console.log(x);
        };
    "#;
    let module = parse_typescript(source).expect("parse failed");
    let items = transform_module(&module, &TypeRegistry::new()).unwrap();

    match &items[0] {
        Item::Fn { params, .. } => {
            assert_eq!(params[0].ty, Some(RustType::String)); // explicit wins
        }
        _ => panic!("expected Item::Fn"),
    }
}

// ---- extract_fn_return_type tests ----

#[test]
fn test_extract_fn_return_type_from_fn_type() {
    let ty = RustType::Fn {
        params: vec![],
        return_type: Box::new(RustType::String),
    };
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let t = Transformer::for_module(&tctx, &mut synthetic);
    let result = t.extract_fn_return_type(&ty);
    assert_eq!(result, Some(RustType::String));
}

#[test]
fn test_extract_fn_return_type_from_named_type_in_registry() {
    let mut reg = TypeRegistry::new();
    reg.register(
        "GetInfo".to_string(),
        crate::registry::TypeDef::Function {
            params: vec![],
            return_type: Some(RustType::Named {
                name: "Info".to_string(),
                type_args: vec![],
            }),
            has_rest: false,
        },
    );
    let ty = RustType::Named {
        name: "GetInfo".to_string(),
        type_args: vec![],
    };
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let t = Transformer::for_module(&tctx, &mut synthetic);
    let result = t.extract_fn_return_type(&ty);
    assert_eq!(
        result,
        Some(RustType::Named {
            name: "Info".to_string(),
            type_args: vec![],
        })
    );
}

#[test]
fn test_extract_fn_return_type_unknown_returns_none() {
    let ty = RustType::String;
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let t = Transformer::for_module(&tctx, &mut synthetic);
    let result = t.extract_fn_return_type(&ty);
    assert_eq!(result, None);
}

// ---- extract_fn_param_types tests ----

#[test]
fn test_extract_fn_param_types_from_fn_type() {
    let ty = RustType::Fn {
        params: vec![RustType::F64, RustType::String],
        return_type: Box::new(RustType::Unit),
    };
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let t = Transformer::for_module(&tctx, &mut synthetic);
    let result = t.extract_fn_param_types(&ty);
    assert_eq!(result, Some(vec![RustType::F64, RustType::String]));
}

#[test]
fn test_extract_fn_param_types_from_named_type_in_registry() {
    let mut reg = TypeRegistry::new();
    reg.register(
        "GetConnInfo".to_string(),
        crate::registry::TypeDef::Function {
            params: vec![(
                "c".to_string(),
                RustType::Named {
                    name: "Context".to_string(),
                    type_args: vec![],
                },
            )],
            return_type: Some(RustType::Named {
                name: "ConnInfo".to_string(),
                type_args: vec![],
            }),
            has_rest: false,
        },
    );
    let ty = RustType::Named {
        name: "GetConnInfo".to_string(),
        type_args: vec![],
    };
    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let t = Transformer::for_module(&tctx, &mut synthetic);
    let result = t.extract_fn_param_types(&ty);
    assert_eq!(
        result,
        Some(vec![RustType::Named {
            name: "Context".to_string(),
            type_args: vec![]
        }])
    );
}

#[test]
fn test_extract_fn_param_types_unknown_returns_none() {
    let ty = RustType::String;
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let mut synthetic = SyntheticTypeRegistry::new();
    let t = Transformer::for_module(&tctx, &mut synthetic);
    let result = t.extract_fn_param_types(&ty);
    assert_eq!(result, None);
}
