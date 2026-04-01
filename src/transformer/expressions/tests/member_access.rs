use super::*;

#[test]
fn test_convert_expr_member_this_field() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("this.name;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::FieldAccess {
            object: Box::new(Expr::Ident("self".to_string())),
            field: "name".to_string(),
        }
    );
}

#[test]
fn test_convert_expr_member_non_this() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("obj.field;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::FieldAccess {
            object: Box::new(Expr::Ident("obj".to_string())),
            field: "field".to_string(),
        }
    );
}

#[test]
fn test_convert_expr_member_enum_access_from_registry() {
    // enum Color { Red, Green, Blue }
    // Color.Red  →  Color::Red
    let mut reg = TypeRegistry::new();
    use crate::registry::TypeDef;
    reg.register(
        "Color".to_string(),
        TypeDef::Enum {
            type_params: vec![],
            variants: vec!["Red".to_string(), "Green".to_string(), "Blue".to_string()],
            string_values: std::collections::HashMap::new(),
            tag_field: None,
            variant_fields: std::collections::HashMap::new(),
        },
    );

    let f = TctxFixture::with_reg(reg);
    let tctx = f.tctx();
    let swc_expr = parse_expr("Color.Red;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(result, Expr::Ident("Color::Red".to_string()));
}

#[test]
fn test_convert_expr_member_non_enum_unchanged() {
    // obj.field should remain FieldAccess when obj is not an enum
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("obj.field;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::FieldAccess {
            object: Box::new(Expr::Ident("obj".to_string())),
            field: "field".to_string(),
        }
    );
}

#[test]
fn test_convert_member_expr_array_index_literal_generates_safe_get() {
    // arr[0] → arr.get(0).cloned() (safe bounds-checked access, I-319)
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("arr[0];");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        build_safe_index_expr(
            Expr::Ident("arr".to_string()),
            convert_index_to_usize(Expr::NumberLit(0.0)),
        )
    );
}

#[test]
fn test_convert_member_expr_array_index_variable_generates_safe_get() {
    // arr[i] → arr.get(i as usize).cloned() (safe bounds-checked access, I-319)
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("arr[i];");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        build_safe_index_expr(
            Expr::Ident("arr".to_string()),
            convert_index_to_usize(Expr::Ident("i".to_string())),
        )
    );
}

#[test]
fn test_convert_member_expr_tuple_literal_index_generates_field_access() {
    let f = TctxFixture::from_source("function f(pair: [string, number]) { pair[0]; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::FieldAccess {
            object: Box::new(Expr::Ident("pair".to_string())),
            field: "0".to_string(),
        }
    );
}

#[test]
fn test_convert_member_expr_tuple_second_index_generates_field_access() {
    let f = TctxFixture::from_source("function f(pair: [string, number]) { pair[1]; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::FieldAccess {
            object: Box::new(Expr::Ident("pair".to_string())),
            field: "1".to_string(),
        }
    );
}

#[test]
fn test_convert_member_expr_non_tuple_index_generates_safe_get() {
    // Non-tuple type still gets safe indexing
    let f = TctxFixture::new();
    let tctx = f.tctx();

    let swc_expr = parse_expr("arr[0];");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        build_safe_index_expr(
            Expr::Ident("arr".to_string()),
            convert_index_to_usize(Expr::NumberLit(0.0)),
        )
    );
}

#[test]
fn test_convert_expr_private_field_access_generates_field_access() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    // this.#field → self._field
    let expr = parse_expr("this.#routes");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&expr)
        .unwrap();
    match &result {
        Expr::FieldAccess { object, field } => {
            assert!(matches!(object.as_ref(), Expr::Ident(name) if name == "self"));
            assert_eq!(field, "_routes");
        }
        other => panic!("expected FieldAccess, got: {other:?}"),
    }
}

#[test]
fn test_convert_member_expr_for_write_keeps_direct_index() {
    // Assignment target: arr[0] = value → arr[0] (direct index, not .get())
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("arr[0];");
    let member = match &swc_expr {
        ast::Expr::Member(m) => m,
        _ => panic!("expected member expression"),
    };
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_member_expr_for_write(member)
        .unwrap();
    assert_eq!(
        result,
        Expr::Index {
            object: Box::new(Expr::Ident("arr".to_string())),
            index: Box::new(Expr::NumberLit(0.0)),
        }
    );
}

#[test]
fn test_convert_member_expr_vec_literal_index_generates_safe_get() {
    // Vec<T> typed: arr[0] → arr.get(0).cloned()
    let f = TctxFixture::from_source("function f(arr: number[]) { arr[0]; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        build_safe_index_expr(
            Expr::Ident("arr".to_string()),
            convert_index_to_usize(Expr::NumberLit(0.0)),
        )
    );
}

#[test]
fn test_convert_member_expr_vec_variable_index_generates_safe_get() {
    // Vec<T> typed: arr[i] → arr.get(i as usize).cloned()
    let f = TctxFixture::from_source("function f(arr: string[], i: number) { arr[i]; }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        build_safe_index_expr(
            Expr::Ident("arr".to_string()),
            convert_index_to_usize(Expr::Ident("i".to_string())),
        )
    );
}

#[test]
fn test_convert_member_expr_length_generates_len_cast() {
    // arr.length → arr.len() as f64
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("arr.length;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::Cast {
            expr: Box::new(Expr::MethodCall {
                object: Box::new(Expr::Ident("arr".to_string())),
                method: "len".to_string(),
                args: vec![],
            }),
            target: RustType::F64,
        }
    );
}

#[test]
fn test_convert_member_expr_for_write_variable_index_keeps_direct_index() {
    // Assignment target with variable: arr[i] = value → arr[i] (direct index)
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("arr[i];");
    let member = match &swc_expr {
        ast::Expr::Member(m) => m,
        _ => panic!("expected member expression"),
    };
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_member_expr_for_write(member)
        .unwrap();
    assert_eq!(
        result,
        Expr::Index {
            object: Box::new(Expr::Ident("arr".to_string())),
            index: Box::new(Expr::Ident("i".to_string())),
        }
    );
}

#[test]
fn test_convert_member_expr_slice_generates_direct_range_index() {
    // arr.slice(1, 3) → arr[1..3].to_vec()
    // The Range index must use direct Expr::Index, not .get().cloned()
    let f = TctxFixture::from_source("function f(arr: number[]) { arr.slice(1, 3); }");
    let tctx = f.tctx();
    let swc_expr = extract_fn_body_expr_stmt(f.module(), 0, 0);
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    // Should be MethodCall { object: Index { object: arr, index: Range }, method: "to_vec" }
    // NOT MethodCall { object: MethodCall { ..get.. }, method: "cloned" }
    match &result {
        Expr::MethodCall { object, method, .. } if method == "to_vec" => {
            assert!(
                matches!(object.as_ref(), Expr::Index { index, .. }
                    if matches!(index.as_ref(), Expr::Range { .. })),
                "slice should produce Index with Range, got: {object:?}"
            );
        }
        _ => panic!("expected MethodCall(to_vec) with Index(Range), got: {result:?}"),
    }
}

#[test]
fn test_convert_member_expr_process_env_var() {
    // process.env.HOME → std::env::var("HOME").unwrap()
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("process.env.HOME;");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
        .unwrap();
    assert_eq!(
        result,
        Expr::MethodCall {
            object: Box::new(Expr::FnCall {
                name: "std::env::var".to_string(),
                args: vec![Expr::StringLit("HOME".to_string())],
            }),
            method: "unwrap".to_string(),
            args: vec![],
        }
    );
}
