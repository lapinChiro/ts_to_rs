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
fn test_convert_member_expr_array_index_literal_generates_index() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("arr[0];");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
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
fn test_convert_member_expr_array_index_variable_generates_index() {
    let f = TctxFixture::new();
    let tctx = f.tctx();
    let swc_expr = parse_expr("arr[i];");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
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
fn test_convert_member_expr_non_tuple_index_unchanged() {
    let f = TctxFixture::new();
    let tctx = f.tctx();

    let swc_expr = parse_expr("arr[0];");
    let result = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .convert_expr(&swc_expr)
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
