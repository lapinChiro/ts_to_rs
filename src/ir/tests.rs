use super::*;

#[test]
fn test_rust_type_primitives() {
    let _t: RustType = RustType::String;
    let _t: RustType = RustType::F64;
    let _t: RustType = RustType::Bool;
}

#[test]
fn test_rust_type_option() {
    let inner = RustType::String;
    let _t: RustType = RustType::Option(Box::new(inner));
}

#[test]
fn test_rust_type_vec() {
    let inner = RustType::F64;
    let _t: RustType = RustType::Vec(Box::new(inner));
}

#[test]
fn test_visibility() {
    let _pub = Visibility::Public;
    let _priv = Visibility::Private;
}

#[test]
fn test_item_struct() {
    let item = Item::Struct {
        vis: Visibility::Public,
        name: "Point".to_string(),
        type_params: vec![],
        fields: vec![
            StructField {
                vis: None,
                name: "x".to_string(),
                ty: RustType::F64,
            },
            StructField {
                vis: None,
                name: "y".to_string(),
                ty: RustType::Option(Box::new(RustType::F64)),
            },
        ],
    };
    match item {
        Item::Struct { name, fields, .. } => {
            assert_eq!(name, "Point");
            assert_eq!(fields.len(), 2);
        }
        _ => panic!("expected Struct"),
    }
}

#[test]
fn test_item_enum_no_values() {
    let item = Item::Enum {
        vis: Visibility::Public,
        name: "Color".to_string(),
        serde_tag: None,
        variants: vec![
            EnumVariant {
                name: "Red".to_string(),
                value: None,
                data: None,
                fields: vec![],
            },
            EnumVariant {
                name: "Green".to_string(),
                value: None,
                data: None,
                fields: vec![],
            },
        ],
    };
    match item {
        Item::Enum { name, variants, .. } => {
            assert_eq!(name, "Color");
            assert_eq!(variants.len(), 2);
            assert!(variants[0].value.is_none());
        }
        _ => panic!("expected Enum"),
    }
}

#[test]
fn test_item_enum_numeric_values() {
    let item = Item::Enum {
        vis: Visibility::Public,
        name: "Status".to_string(),
        serde_tag: None,
        variants: vec![
            EnumVariant {
                name: "Active".to_string(),
                value: Some(EnumValue::Number(1)),
                data: None,
                fields: vec![],
            },
            EnumVariant {
                name: "Inactive".to_string(),
                value: Some(EnumValue::Number(0)),
                data: None,
                fields: vec![],
            },
        ],
    };
    match &item {
        Item::Enum { variants, .. } => {
            assert_eq!(variants[0].value, Some(EnumValue::Number(1)));
            assert_eq!(variants[1].value, Some(EnumValue::Number(0)));
        }
        _ => panic!("expected Enum"),
    }
}

#[test]
fn test_item_enum_string_values() {
    let item = Item::Enum {
        vis: Visibility::Public,
        name: "Direction".to_string(),
        serde_tag: None,
        variants: vec![
            EnumVariant {
                name: "Up".to_string(),
                value: Some(EnumValue::Str("UP".to_string())),
                data: None,
                fields: vec![],
            },
            EnumVariant {
                name: "Down".to_string(),
                value: Some(EnumValue::Str("DOWN".to_string())),
                data: None,
                fields: vec![],
            },
        ],
    };
    match &item {
        Item::Enum { variants, .. } => {
            assert_eq!(variants[0].value, Some(EnumValue::Str("UP".to_string())));
        }
        _ => panic!("expected Enum"),
    }
}

#[test]
fn test_item_fn() {
    let item = Item::Fn {
        vis: Visibility::Public,
        attributes: vec![],
        is_async: false,
        name: "add".to_string(),
        type_params: vec![],
        params: vec![
            Param {
                name: "a".to_string(),
                ty: Some(RustType::F64),
            },
            Param {
                name: "b".to_string(),
                ty: Some(RustType::F64),
            },
        ],
        return_type: Some(RustType::F64),
        body: vec![],
    };
    match item {
        Item::Fn { name, params, .. } => {
            assert_eq!(name, "add");
            assert_eq!(params.len(), 2);
        }
        _ => panic!("expected Fn"),
    }
}

#[test]
fn test_stmt_let() {
    let stmt = Stmt::Let {
        mutable: false,
        name: "x".to_string(),
        ty: None,
        init: Some(Expr::NumberLit(42.0)),
    };
    match stmt {
        Stmt::Let { name, mutable, .. } => {
            assert_eq!(name, "x");
            assert!(!mutable);
        }
        _ => panic!("expected Let"),
    }
}

#[test]
fn test_stmt_let_mut() {
    let stmt = Stmt::Let {
        mutable: true,
        name: "count".to_string(),
        ty: Some(RustType::F64),
        init: Some(Expr::NumberLit(0.0)),
    };
    match stmt {
        Stmt::Let { mutable, .. } => assert!(mutable),
        _ => panic!("expected Let"),
    }
}

#[test]
fn test_stmt_if_else() {
    let stmt = Stmt::If {
        condition: Expr::BoolLit(true),
        then_body: vec![],
        else_body: Some(vec![]),
    };
    match stmt {
        Stmt::If { else_body, .. } => assert!(else_body.is_some()),
        _ => panic!("expected If"),
    }
}

#[test]
fn test_stmt_if_no_else() {
    let stmt = Stmt::If {
        condition: Expr::BoolLit(false),
        then_body: vec![],
        else_body: None,
    };
    match stmt {
        Stmt::If { else_body, .. } => assert!(else_body.is_none()),
        _ => panic!("expected If"),
    }
}

#[test]
fn test_expr_literals() {
    let _n = Expr::NumberLit(2.71);
    let _b = Expr::BoolLit(true);
    let _s = Expr::StringLit("hello".to_string());
}

#[test]
fn test_expr_ident() {
    let e = Expr::Ident("foo".to_string());
    match e {
        Expr::Ident(name) => assert_eq!(name, "foo"),
        _ => panic!("expected Ident"),
    }
}

#[test]
fn test_expr_format_macro() {
    let e = Expr::FormatMacro {
        template: "Hello, {}!".to_string(),
        args: vec![Expr::Ident("name".to_string())],
    };
    match e {
        Expr::FormatMacro { template, args } => {
            assert_eq!(template, "Hello, {}!");
            assert_eq!(args.len(), 1);
        }
        _ => panic!("expected FormatMacro"),
    }
}

#[test]
fn test_rust_type_result() {
    let ty = RustType::Result {
        ok: Box::new(RustType::String),
        err: Box::new(RustType::String),
    };
    match ty {
        RustType::Result { ok, err } => {
            assert_eq!(*ok, RustType::String);
            assert_eq!(*err, RustType::String);
        }
        _ => panic!("expected Result"),
    }
}

#[test]
fn test_stmt_while() {
    let stmt = Stmt::While {
        label: None,
        condition: Expr::BoolLit(true),
        body: vec![Stmt::Expr(Expr::Ident("x".to_string()))],
    };
    match stmt {
        Stmt::While {
            condition, body, ..
        } => {
            assert_eq!(condition, Expr::BoolLit(true));
            assert_eq!(body.len(), 1);
        }
        _ => panic!("expected While"),
    }
}

#[test]
fn test_stmt_for_in() {
    let stmt = Stmt::ForIn {
        label: None,
        var: "i".to_string(),
        iterable: Expr::Range {
            start: Some(Box::new(Expr::NumberLit(0.0))),
            end: Some(Box::new(Expr::NumberLit(10.0))),
        },
        body: vec![],
    };
    match stmt {
        Stmt::ForIn {
            var,
            iterable,
            body,
            ..
        } => {
            assert_eq!(var, "i");
            assert!(matches!(iterable, Expr::Range { .. }));
            assert!(body.is_empty());
        }
        _ => panic!("expected ForIn"),
    }
}

#[test]
fn test_expr_range() {
    let expr = Expr::Range {
        start: Some(Box::new(Expr::NumberLit(0.0))),
        end: Some(Box::new(Expr::NumberLit(5.0))),
    };
    match expr {
        Expr::Range { start, end } => {
            assert_eq!(*start.unwrap(), Expr::NumberLit(0.0));
            assert_eq!(*end.unwrap(), Expr::NumberLit(5.0));
        }
        _ => panic!("expected Range"),
    }
}

#[test]
fn test_expr_fn_call_err() {
    let expr = Expr::FnCall {
        name: "Err".to_string(),
        args: vec![Expr::StringLit("something went wrong".to_string())],
    };
    match expr {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "Err");
            assert_eq!(args.len(), 1);
        }
        _ => panic!("expected FnCall"),
    }
}

#[test]
fn test_expr_fn_call_ok() {
    let expr = Expr::FnCall {
        name: "Ok".to_string(),
        args: vec![Expr::NumberLit(42.0)],
    };
    match expr {
        Expr::FnCall { name, args } => {
            assert_eq!(name, "Ok");
            assert_eq!(args.len(), 1);
        }
        _ => panic!("expected FnCall"),
    }
}

#[test]
fn test_expr_return() {
    let stmt = Stmt::Return(Some(Expr::NumberLit(1.0)));
    match stmt {
        Stmt::Return(Some(Expr::NumberLit(n))) => assert_eq!(n, 1.0),
        _ => panic!("expected Return"),
    }
}

#[test]
fn test_binop_bitwise_as_str() {
    assert_eq!(BinOp::BitAnd.as_str(), "&");
    assert_eq!(BinOp::BitOr.as_str(), "|");
    assert_eq!(BinOp::BitXor.as_str(), "^");
    assert_eq!(BinOp::Shl.as_str(), "<<");
    assert_eq!(BinOp::Shr.as_str(), ">>");
}

#[test]
fn test_binop_is_bitwise_returns_true_for_bitwise_ops() {
    assert!(BinOp::BitAnd.is_bitwise());
    assert!(BinOp::BitOr.is_bitwise());
    assert!(BinOp::BitXor.is_bitwise());
    assert!(BinOp::Shl.is_bitwise());
    assert!(BinOp::Shr.is_bitwise());
}

#[test]
fn test_binop_is_bitwise_returns_false_for_non_bitwise_ops() {
    assert!(!BinOp::Add.is_bitwise());
    assert!(!BinOp::Sub.is_bitwise());
    assert!(!BinOp::Mul.is_bitwise());
    assert!(!BinOp::Div.is_bitwise());
    assert!(!BinOp::Mod.is_bitwise());
    assert!(!BinOp::Eq.is_bitwise());
    assert!(!BinOp::NotEq.is_bitwise());
    assert!(!BinOp::Lt.is_bitwise());
    assert!(!BinOp::LtEq.is_bitwise());
    assert!(!BinOp::Gt.is_bitwise());
    assert!(!BinOp::GtEq.is_bitwise());
    assert!(!BinOp::LogicalAnd.is_bitwise());
    assert!(!BinOp::LogicalOr.is_bitwise());
}

#[test]
fn test_binop_bitwise_precedence_order() {
    // Rust precedence: Shl/Shr > BitAnd > BitXor > BitOr
    assert!(BinOp::Shl.precedence() > BinOp::BitAnd.precedence());
    assert!(BinOp::Shr.precedence() > BinOp::BitAnd.precedence());
    assert!(BinOp::BitAnd.precedence() > BinOp::BitXor.precedence());
    assert!(BinOp::BitXor.precedence() > BinOp::BitOr.precedence());
}

// --- I-100: RustType::substitute ---

#[test]
fn test_substitute_type_param_to_concrete() {
    // Named("T") に T→String → RustType::String
    use std::collections::HashMap;
    let ty = RustType::Named {
        name: "T".to_string(),
        type_args: vec![],
    };
    let bindings = HashMap::from([("T".to_string(), RustType::String)]);
    assert_eq!(ty.substitute(&bindings), RustType::String);
}

#[test]
fn test_substitute_vec_recursive() {
    // Vec<T> に T→F64 → Vec<F64>
    use std::collections::HashMap;
    let ty = RustType::Vec(Box::new(RustType::Named {
        name: "T".to_string(),
        type_args: vec![],
    }));
    let bindings = HashMap::from([("T".to_string(), RustType::F64)]);
    assert_eq!(
        ty.substitute(&bindings),
        RustType::Vec(Box::new(RustType::F64))
    );
}

#[test]
fn test_substitute_option_recursive() {
    // Option<Vec<T>> に T→String → Option<Vec<String>>
    use std::collections::HashMap;
    let ty = RustType::Option(Box::new(RustType::Vec(Box::new(RustType::Named {
        name: "T".to_string(),
        type_args: vec![],
    }))));
    let bindings = HashMap::from([("T".to_string(), RustType::String)]);
    assert_eq!(
        ty.substitute(&bindings),
        RustType::Option(Box::new(RustType::Vec(Box::new(RustType::String))))
    );
}

#[test]
fn test_substitute_unrelated_type_unchanged() {
    // RustType::Bool に T→String → Bool（変化なし）
    use std::collections::HashMap;
    let bindings = HashMap::from([("T".to_string(), RustType::String)]);
    assert_eq!(RustType::Bool.substitute(&bindings), RustType::Bool);
}

#[test]
fn test_substitute_named_type_args() {
    // Container<T> に T→String → Container<String>
    use std::collections::HashMap;
    let ty = RustType::Named {
        name: "Container".to_string(),
        type_args: vec![RustType::Named {
            name: "T".to_string(),
            type_args: vec![],
        }],
    };
    let bindings = HashMap::from([("T".to_string(), RustType::String)]);
    assert_eq!(
        ty.substitute(&bindings),
        RustType::Named {
            name: "Container".to_string(),
            type_args: vec![RustType::String],
        }
    );
}

// -- sanitize_field_name tests --

#[test]
fn test_sanitize_field_name_hyphen_replaced() {
    assert_eq!(sanitize_field_name("Content-Type"), "Content_Type");
}

#[test]
fn test_sanitize_field_name_brackets_removed() {
    assert_eq!(sanitize_field_name("foo[0]"), "foo0");
}

#[test]
fn test_sanitize_field_name_underscore_only_becomes_field() {
    assert_eq!(sanitize_field_name("_"), "_field");
}

#[test]
fn test_sanitize_field_name_digit_prefix_escaped() {
    assert_eq!(sanitize_field_name("0abc"), "_0abc");
}

#[test]
fn test_sanitize_field_name_empty_becomes_empty_sentinel() {
    assert_eq!(sanitize_field_name(""), "_empty");
}

#[test]
fn test_sanitize_field_name_normal_passthrough() {
    assert_eq!(sanitize_field_name("name"), "name");
}

#[test]
fn test_sanitize_field_name_keyword_not_escaped() {
    // キーワードエスケープは generator (escape_ident) の責務。
    // sanitize_field_name は文字レベルのサニタイズのみ。
    assert_eq!(sanitize_field_name("type"), "type");
}

// -- camel_to_snake tests --

#[test]
fn test_camel_to_snake_simple() {
    assert_eq!(camel_to_snake("byteLength"), "byte_length");
}

#[test]
fn test_camel_to_snake_acronym() {
    assert_eq!(camel_to_snake("toISOString"), "to_iso_string");
}

#[test]
fn test_camel_to_snake_all_upper_acronym() {
    assert_eq!(camel_to_snake("XMLHTTPRequest"), "xmlhttp_request");
}

#[test]
fn test_camel_to_snake_already_snake() {
    assert_eq!(camel_to_snake("already_snake"), "already_snake");
}

#[test]
fn test_camel_to_snake_single_word() {
    assert_eq!(camel_to_snake("name"), "name");
}
