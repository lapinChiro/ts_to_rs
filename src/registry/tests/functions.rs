use super::*;

// --- build_registry: function declarations ---

#[test]
fn test_build_registry_function() {
    let module =
        parse_typescript("function draw(p: Point, color: string): boolean { return true; }")
            .unwrap();
    let reg = build_registry(&module);
    match reg.get("draw").unwrap() {
        TypeDef::Function {
            params,
            return_type,
            ..
        } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].0, "p");
            assert_eq!(
                params[0].1,
                RustType::Named {
                    name: "Point".to_string(),
                    type_args: vec![],
                }
            );
            assert_eq!(params[1].0, "color");
            assert_eq!(params[1].1, RustType::String);
            assert_eq!(*return_type, Some(RustType::Bool));
        }
        _ => panic!("expected Function"),
    }
}

#[test]
fn test_build_registry_arrow_function() {
    let module = parse_typescript("const greet = (name: string): string => name;").unwrap();
    let reg = build_registry(&module);
    match reg.get("greet").unwrap() {
        TypeDef::Function {
            params,
            return_type,
            ..
        } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].0, "name");
            assert_eq!(params[0].1, RustType::String);
            assert_eq!(*return_type, Some(RustType::String));
        }
        _ => panic!("expected Function"),
    }
}

#[test]
fn test_build_registry_fn_rest_param_sets_has_rest_true() {
    let module = parse_typescript("function sum(...nums: number[]): number { return 0; }").unwrap();
    let reg = build_registry(&module);
    match reg.get("sum").unwrap() {
        TypeDef::Function {
            params, has_rest, ..
        } => {
            assert!(has_rest, "has_rest should be true for rest param");
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].0, "nums");
            assert_eq!(params[0].1, RustType::Vec(Box::new(RustType::F64)));
        }
        _ => panic!("expected Function"),
    }
}

#[test]
fn test_build_registry_fn_mixed_and_rest_param() {
    let module =
        parse_typescript("function log(prefix: string, ...msgs: string[]): void {}").unwrap();
    let reg = build_registry(&module);
    match reg.get("log").unwrap() {
        TypeDef::Function {
            params, has_rest, ..
        } => {
            assert!(has_rest);
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].0, "prefix");
            assert_eq!(params[0].1, RustType::String);
            assert_eq!(params[1].0, "msgs");
            assert_eq!(params[1].1, RustType::Vec(Box::new(RustType::String)));
        }
        _ => panic!("expected Function"),
    }
}

#[test]
fn test_build_registry_fn_no_rest_param_has_rest_false() {
    let module = parse_typescript("function greet(name: string): void {}").unwrap();
    let reg = build_registry(&module);
    match reg.get("greet").unwrap() {
        TypeDef::Function { has_rest, .. } => {
            assert!(!has_rest, "has_rest should be false without rest param");
        }
        _ => panic!("expected Function"),
    }
}

// --- Function type alias registration ---

#[test]
fn test_build_registry_fn_type_alias_with_params() {
    let module = parse_typescript("type Handler = (c: string) => number;").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Handler").expect("Handler should be registered");
    match def {
        TypeDef::Function {
            params,
            return_type,
            ..
        } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].0, "c");
            assert_eq!(params[0].1, RustType::String);
            assert_eq!(*return_type, Some(RustType::F64));
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

#[test]
fn test_build_registry_fn_type_alias_no_params() {
    let module = parse_typescript("type Factory = () => string;").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Factory").expect("Factory should be registered");
    match def {
        TypeDef::Function {
            params,
            return_type,
            ..
        } => {
            assert!(params.is_empty(), "expected no params, got {:?}", params);
            assert_eq!(*return_type, Some(RustType::String));
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

// --- Call signature type alias ---

#[test]
fn test_build_registry_call_signature_type_alias_registers_as_function() {
    let module = parse_typescript("type Handler = { (c: string): number };").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Handler").expect("Handler should be registered");
    match def {
        TypeDef::Function {
            params,
            return_type,
            ..
        } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].0, "c");
            assert_eq!(params[0].1, RustType::String);
            assert_eq!(*return_type, Some(RustType::F64));
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

#[test]
fn test_build_registry_call_signature_type_alias_multiple_params() {
    let module = parse_typescript("type Callback = { (a: string, b: number): boolean };").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Callback").expect("Callback should be registered");
    match def {
        TypeDef::Function {
            params,
            return_type,
            ..
        } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0], ("a".to_string(), RustType::String));
            assert_eq!(params[1], ("b".to_string(), RustType::F64));
            assert_eq!(*return_type, Some(RustType::Bool));
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

#[test]
fn test_build_registry_call_signature_type_alias_no_params() {
    let module = parse_typescript("type Factory = { (): string };").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Factory").expect("Factory should be registered");
    match def {
        TypeDef::Function {
            params,
            return_type,
            ..
        } => {
            assert!(params.is_empty());
            assert_eq!(*return_type, Some(RustType::String));
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

#[test]
fn test_build_registry_call_signature_overload_picks_longest() {
    let module = parse_typescript(
        "type GetCookie = { (c: string): string; (c: string, key: string): number };",
    )
    .unwrap();
    let reg = build_registry(&module);
    let def = reg
        .get("GetCookie")
        .expect("GetCookie should be registered");
    match def {
        TypeDef::Function {
            params,
            return_type,
            ..
        } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].0, "c");
            assert_eq!(params[1].0, "key");
            assert_eq!(*return_type, Some(RustType::F64));
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

#[test]
fn test_build_registry_call_signature_with_properties_stays_struct() {
    let module = parse_typescript("type Mixed = { name: string; (x: number): void };").unwrap();
    let reg = build_registry(&module);
    let def = reg.get("Mixed").expect("Mixed should be registered");
    match def {
        TypeDef::Struct { fields, .. } => {
            assert!(
                fields.iter().any(|(n, _)| n == "name"),
                "should have 'name' field"
            );
        }
        other => panic!("expected Struct (mixed call sig + properties), got {other:?}"),
    }
}
