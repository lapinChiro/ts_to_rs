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
            assert_eq!(params[0].name, "p");
            assert_eq!(
                params[0].ty,
                RustType::Named {
                    name: "Point".to_string(),
                    type_args: vec![],
                }
            );
            assert_eq!(params[1].name, "color");
            assert_eq!(params[1].ty, RustType::String);
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
            assert_eq!(params[0].name, "name");
            assert_eq!(params[0].ty, RustType::String);
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
            assert_eq!(params[0].name, "nums");
            assert_eq!(params[0].ty, RustType::Vec(Box::new(RustType::F64)));
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
            assert_eq!(params[0].name, "prefix");
            assert_eq!(params[0].ty, RustType::String);
            assert_eq!(params[1].name, "msgs");
            assert_eq!(params[1].ty, RustType::Vec(Box::new(RustType::String)));
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
            assert_eq!(params[0].name, "c");
            assert_eq!(params[0].ty, RustType::String);
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
            assert_eq!(params[0].name, "c");
            assert_eq!(params[0].ty, RustType::String);
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
            assert_eq!(params[0], ("a".to_string(), RustType::String).into());
            assert_eq!(params[1], ("b".to_string(), RustType::F64).into());
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
            assert_eq!(params[0].name, "c");
            assert_eq!(params[1].name, "key");
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
                fields.iter().any(|f| f.name == "name"),
                "should have 'name' field"
            );
        }
        other => panic!("expected Struct (mixed call sig + properties), got {other:?}"),
    }
}

// --- G3: fn type alias rest parameter ---

#[test]
fn test_fn_type_alias_rest_param_sets_has_rest() {
    let module = parse_typescript("type Fn = (...args: string[]) => void;").unwrap();
    let reg = build_registry(&module);
    match reg.get("Fn").unwrap() {
        TypeDef::Function {
            params, has_rest, ..
        } => {
            assert!(has_rest, "has_rest should be true");
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "args");
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

// --- G4: call signature type alias rest parameter ---

#[test]
fn test_call_signature_type_alias_rest_param_sets_has_rest() {
    let module = parse_typescript("type Handler = { (...args: string[]): void };").unwrap();
    let reg = build_registry(&module);
    match reg.get("Handler").unwrap() {
        TypeDef::Function {
            params, has_rest, ..
        } => {
            assert!(has_rest, "has_rest should be true");
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "args");
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

// --- G7: arrow default parameter → Option wrap ---

#[test]
fn test_arrow_default_param_option_wrap() {
    let module = parse_typescript(
        "const greet = (name: string, greeting: string = 'hello'): string => name;",
    )
    .unwrap();
    let reg = build_registry(&module);
    match reg.get("greet").unwrap() {
        TypeDef::Function { params, .. } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0], ("name".to_string(), RustType::String).into());
            assert_eq!(
                params[1],
                ParamDef {
                    name: "greeting".to_string(),
                    ty: RustType::Option(Box::new(RustType::String)),
                    optional: false,
                    has_default: true,
                }
            );
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

// --- collection phase unit tests (TsTypeInfo) ---

/// TypeScript の関数型エイリアスをパースして `TsTypeAliasDecl` を返す。
fn parse_type_alias(source: &str) -> swc_ecma_ast::TsTypeAliasDecl {
    let module = parse_typescript(source).unwrap();
    for item in &module.body {
        if let swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(
            swc_ecma_ast::Decl::TsTypeAlias(alias),
        )) = item
        {
            return *alias.clone();
        }
    }
    panic!("no TsTypeAliasDecl found in source: {source}");
}

#[test]
fn test_collect_fn_type_alias_returns_ts_type_info() {
    use crate::ts_type_info::TsTypeInfo;

    let alias = parse_type_alias("type Handler = (req: string, res: number) => boolean;");
    let ts_def = super::super::functions::try_collect_fn_type_alias(&alias)
        .expect("should detect function type alias");

    match ts_def {
        TypeDef::Function {
            params,
            return_type,
            has_rest,
            ..
        } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "req");
            assert_eq!(params[0].ty, TsTypeInfo::String);
            assert_eq!(params[1].name, "res");
            assert_eq!(params[1].ty, TsTypeInfo::Number);
            assert_eq!(return_type, Some(TsTypeInfo::Boolean));
            assert!(!has_rest);
        }
        _ => panic!("expected Function"),
    }
}

#[test]
fn test_collect_type_params_returns_ts_type_info_constraint() {
    use crate::ts_type_info::TsTypeInfo;

    let alias = parse_type_alias("type Fn<T extends string> = (x: T) => T;");
    let ts_def = super::super::functions::try_collect_fn_type_alias(&alias)
        .expect("should detect function type alias");

    if let TypeDef::Function { type_params, .. } = ts_def {
        assert_eq!(type_params.len(), 1);
        assert_eq!(type_params[0].name, "T");
        // constraint は TsTypeInfo（RustType ではない）
        assert_eq!(type_params[0].constraint, Some(TsTypeInfo::String));
    } else {
        panic!("expected Function");
    }
}

#[test]
fn test_collect_type_params_extracts_default() {
    use crate::ts_type_info::TsTypeInfo;

    let alias = parse_type_alias("type Fn<T, U = number> = (x: T) => U;");
    let ts_def = super::super::functions::try_collect_fn_type_alias(&alias)
        .expect("should detect function type alias");

    if let TypeDef::Function { type_params, .. } = ts_def {
        assert_eq!(type_params.len(), 2);
        // T: no constraint, no default
        assert_eq!(type_params[0].name, "T");
        assert_eq!(type_params[0].constraint, None);
        assert_eq!(type_params[0].default, None);
        // U: no constraint, default = number
        assert_eq!(type_params[1].name, "U");
        assert_eq!(type_params[1].constraint, None);
        assert_eq!(
            type_params[1].default,
            Some(TsTypeInfo::Number),
            "collect_type_params must extract default from SWC AST"
        );
    } else {
        panic!("expected Function");
    }
}
