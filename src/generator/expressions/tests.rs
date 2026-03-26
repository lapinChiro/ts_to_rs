use super::*;
use crate::ir::{BinOp, ClosureBody, Expr, Param, RustType, Stmt, UnOp};

#[test]
fn test_generate_expr_number_whole() {
    assert_eq!(generate_expr(&Expr::NumberLit(42.0)), "42.0");
}

#[test]
fn test_generate_expr_number_fractional() {
    assert_eq!(generate_expr(&Expr::NumberLit(2.71)), "2.71");
}

#[test]
fn test_generate_expr_bool_true() {
    assert_eq!(generate_expr(&Expr::BoolLit(true)), "true");
}

#[test]
fn test_generate_expr_bool_false() {
    assert_eq!(generate_expr(&Expr::BoolLit(false)), "false");
}

#[test]
fn test_generate_expr_string_lit() {
    assert_eq!(
        generate_expr(&Expr::StringLit("hello".to_string())),
        "\"hello\""
    );
}

#[test]
fn test_generate_expr_ident() {
    assert_eq!(generate_expr(&Expr::Ident("foo".to_string())), "foo");
}

#[test]
fn test_generate_expr_tuple_literal() {
    let expr = Expr::Tuple {
        elements: vec![
            Expr::MethodCall {
                object: Box::new(Expr::StringLit("a".to_string())),
                method: "to_string".to_string(),
                args: vec![],
            },
            Expr::NumberLit(1.0),
        ],
    };
    assert_eq!(generate_expr(&expr), r#"("a".to_string(), 1.0)"#);
}

#[test]
fn test_generate_expr_binary_op() {
    let expr = Expr::BinaryOp {
        left: Box::new(Expr::Ident("a".to_string())),
        op: BinOp::Add,
        right: Box::new(Expr::Ident("b".to_string())),
    };
    assert_eq!(generate_expr(&expr), "a + b");
}

#[test]
fn test_generate_expr_bitwise_and_casts_to_i64() {
    let expr = Expr::BinaryOp {
        left: Box::new(Expr::Ident("a".to_string())),
        op: BinOp::BitAnd,
        right: Box::new(Expr::Ident("b".to_string())),
    };
    assert_eq!(generate_expr(&expr), "((a as i64) & (b as i64)) as f64");
}

#[test]
fn test_generate_expr_bitwise_or_casts_to_i64() {
    let expr = Expr::BinaryOp {
        left: Box::new(Expr::Ident("a".to_string())),
        op: BinOp::BitOr,
        right: Box::new(Expr::Ident("b".to_string())),
    };
    assert_eq!(generate_expr(&expr), "((a as i64) | (b as i64)) as f64");
}

#[test]
fn test_generate_expr_bitwise_xor_casts_to_i64() {
    let expr = Expr::BinaryOp {
        left: Box::new(Expr::Ident("a".to_string())),
        op: BinOp::BitXor,
        right: Box::new(Expr::Ident("b".to_string())),
    };
    assert_eq!(generate_expr(&expr), "((a as i64) ^ (b as i64)) as f64");
}

#[test]
fn test_generate_expr_bitwise_shl_casts_to_i64() {
    let expr = Expr::BinaryOp {
        left: Box::new(Expr::Ident("a".to_string())),
        op: BinOp::Shl,
        right: Box::new(Expr::Ident("b".to_string())),
    };
    assert_eq!(generate_expr(&expr), "((a as i64) << (b as i64)) as f64");
}

#[test]
fn test_generate_expr_bitwise_shr_casts_to_i64() {
    let expr = Expr::BinaryOp {
        left: Box::new(Expr::Ident("a".to_string())),
        op: BinOp::Shr,
        right: Box::new(Expr::Ident("b".to_string())),
    };
    assert_eq!(generate_expr(&expr), "((a as i64) >> (b as i64)) as f64");
}

#[test]
fn test_generate_expr_unsigned_shr_casts_to_u32() {
    let expr = Expr::BinaryOp {
        left: Box::new(Expr::Ident("a".to_string())),
        op: BinOp::UShr,
        right: Box::new(Expr::Ident("b".to_string())),
    };
    assert_eq!(
        generate_expr(&expr),
        "((a as i32 as u32) >> (b as u32)) as f64"
    );
}

#[test]
fn test_generate_expr_bitwise_nested_or_and() {
    let expr = Expr::BinaryOp {
        left: Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Ident("a".to_string())),
            op: BinOp::BitAnd,
            right: Box::new(Expr::Ident("b".to_string())),
        }),
        op: BinOp::BitOr,
        right: Box::new(Expr::Ident("c".to_string())),
    };
    assert_eq!(
        generate_expr(&expr),
        "((((a as i64) & (b as i64)) as f64 as i64) | (c as i64)) as f64"
    );
}

#[test]
fn test_generate_expr_arithmetic_with_bitwise_no_cast_on_arithmetic() {
    let expr = Expr::BinaryOp {
        left: Box::new(Expr::Ident("a".to_string())),
        op: BinOp::Add,
        right: Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Ident("b".to_string())),
            op: BinOp::BitAnd,
            right: Box::new(Expr::Ident("c".to_string())),
        }),
    };
    assert_eq!(
        generate_expr(&expr),
        "a + (((b as i64) & (c as i64)) as f64)"
    );
}

#[test]
fn test_generate_expr_field_access() {
    let expr = Expr::FieldAccess {
        object: Box::new(Expr::Ident("self".to_string())),
        field: "name".to_string(),
    };
    assert_eq!(generate_expr(&expr), "self.name");
}

#[test]
fn test_generate_expr_format_macro_no_args() {
    let expr = Expr::FormatMacro {
        template: "hello".to_string(),
        args: vec![],
    };
    assert_eq!(generate_expr(&expr), "format!(\"hello\")");
}

#[test]
fn test_generate_expr_format_macro_with_args() {
    let expr = Expr::FormatMacro {
        template: "Hello, {}!".to_string(),
        args: vec![Expr::Ident("name".to_string())],
    };
    assert_eq!(generate_expr(&expr), "format!(\"Hello, {}!\", name)");
}

#[test]
fn test_generate_expr_fn_call_err() {
    let expr = Expr::FnCall {
        name: "Err".to_string(),
        args: vec![Expr::StringLit("error".to_string())],
    };
    assert_eq!(generate_expr(&expr), "Err(\"error\")");
}

#[test]
fn test_generate_expr_fn_call_ok() {
    let expr = Expr::FnCall {
        name: "Ok".to_string(),
        args: vec![Expr::NumberLit(42.0)],
    };
    assert_eq!(generate_expr(&expr), "Ok(42.0)");
}

#[test]
fn test_generate_closure_expr_body() {
    let expr = Expr::Closure {
        params: vec![Param {
            name: "x".to_string(),
            ty: Some(RustType::F64),
        }],
        return_type: None,
        body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Ident("x".to_string())),
            op: BinOp::Add,
            right: Box::new(Expr::NumberLit(1.0)),
        })),
    };
    assert_eq!(generate_expr(&expr), "|x: f64| x + 1.0");
}

#[test]
fn test_generate_closure_block_body() {
    let expr = Expr::Closure {
        params: vec![Param {
            name: "x".to_string(),
            ty: Some(RustType::F64),
        }],
        return_type: Some(RustType::F64),
        body: ClosureBody::Block(vec![Stmt::TailExpr(Expr::BinaryOp {
            left: Box::new(Expr::Ident("x".to_string())),
            op: BinOp::Add,
            right: Box::new(Expr::NumberLit(1.0)),
        })]),
    };
    let expected = "|x: f64| -> f64 {\n    x + 1.0\n}";
    assert_eq!(generate_expr(&expr), expected);
}

#[test]
fn test_generate_closure_no_params() {
    let expr = Expr::Closure {
        params: vec![],
        return_type: None,
        body: ClosureBody::Expr(Box::new(Expr::NumberLit(42.0))),
    };
    assert_eq!(generate_expr(&expr), "|| 42.0");
}

#[test]
fn test_generate_closure_param_no_type_annotation() {
    let expr = Expr::Closure {
        params: vec![Param {
            name: "x".to_string(),
            ty: None,
        }],
        return_type: None,
        body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Ident("x".to_string())),
            op: BinOp::Add,
            right: Box::new(Expr::NumberLit(1.0)),
        })),
    };
    assert_eq!(generate_expr(&expr), "|x| x + 1.0");
}

#[test]
fn test_generate_closure_expr_body_with_return_type_has_braces() {
    let expr = Expr::Closure {
        params: vec![Param {
            name: "x".to_string(),
            ty: Some(RustType::F64),
        }],
        return_type: Some(RustType::F64),
        body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Ident("x".to_string())),
            op: BinOp::Mul,
            right: Box::new(Expr::NumberLit(2.0)),
        })),
    };
    assert_eq!(generate_expr(&expr), "|x: f64| -> f64 { x * 2.0 }");
}

#[test]
fn test_generate_closure_expr_body_without_return_type_no_braces() {
    let expr = Expr::Closure {
        params: vec![Param {
            name: "x".to_string(),
            ty: Some(RustType::F64),
        }],
        return_type: None,
        body: ClosureBody::Expr(Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Ident("x".to_string())),
            op: BinOp::Mul,
            right: Box::new(Expr::NumberLit(2.0)),
        })),
    };
    assert_eq!(generate_expr(&expr), "|x: f64| x * 2.0");
}

#[test]
fn test_generate_expr_vec_numbers() {
    let expr = Expr::Vec {
        elements: vec![
            Expr::NumberLit(1.0),
            Expr::NumberLit(2.0),
            Expr::NumberLit(3.0),
        ],
    };
    assert_eq!(generate_expr(&expr), "vec![1.0, 2.0, 3.0]");
}

#[test]
fn test_generate_expr_vec_empty() {
    let expr = Expr::Vec { elements: vec![] };
    assert_eq!(generate_expr(&expr), "vec![]");
}

#[test]
fn test_generate_expr_vec_single() {
    let expr = Expr::Vec {
        elements: vec![Expr::StringLit("hello".to_string())],
    };
    assert_eq!(generate_expr(&expr), "vec![\"hello\"]");
}

// -- If expression tests --

#[test]
fn test_generate_expr_if_basic() {
    let expr = Expr::If {
        condition: Box::new(Expr::Ident("flag".to_string())),
        then_expr: Box::new(Expr::Ident("x".to_string())),
        else_expr: Box::new(Expr::Ident("y".to_string())),
    };
    assert_eq!(generate_expr(&expr), "if flag { x } else { y }");
}

#[test]
fn test_generate_expr_if_with_literals() {
    let expr = Expr::If {
        condition: Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Ident("a".to_string())),
            op: BinOp::Gt,
            right: Box::new(Expr::NumberLit(0.0)),
        }),
        then_expr: Box::new(Expr::NumberLit(1.0)),
        else_expr: Box::new(Expr::NumberLit(2.0)),
    };
    assert_eq!(generate_expr(&expr), "if a > 0.0 { 1.0 } else { 2.0 }");
}

#[test]
fn test_generate_expr_if_nested() {
    let expr = Expr::If {
        condition: Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Ident("x".to_string())),
            op: BinOp::Gt,
            right: Box::new(Expr::NumberLit(0.0)),
        }),
        then_expr: Box::new(Expr::StringLit("positive".to_string())),
        else_expr: Box::new(Expr::If {
            condition: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Ident("x".to_string())),
                op: BinOp::Lt,
                right: Box::new(Expr::NumberLit(0.0)),
            }),
            then_expr: Box::new(Expr::StringLit("negative".to_string())),
            else_expr: Box::new(Expr::StringLit("zero".to_string())),
        }),
    };
    assert_eq!(
        generate_expr(&expr),
        "if x > 0.0 { \"positive\" } else { if x < 0.0 { \"negative\" } else { \"zero\" } }"
    );
}

// -- MacroCall tests --

#[test]
fn test_generate_expr_macro_call_no_args() {
    let expr = Expr::MacroCall {
        name: "println".to_string(),
        args: vec![],
        use_debug: vec![],
    };
    assert_eq!(generate_expr(&expr), "println!()");
}

#[test]
fn test_generate_expr_macro_call_single_string_literal() {
    let expr = Expr::MacroCall {
        name: "println".to_string(),
        args: vec![Expr::StringLit("hello".to_string())],
        use_debug: vec![false],
    };
    assert_eq!(generate_expr(&expr), "println!(\"hello\")");
}

#[test]
fn test_generate_expr_macro_call_single_ident() {
    let expr = Expr::MacroCall {
        name: "println".to_string(),
        args: vec![Expr::Ident("x".to_string())],
        use_debug: vec![false],
    };
    assert_eq!(generate_expr(&expr), "println!(\"{}\", x)");
}

#[test]
fn test_generate_expr_macro_call_multiple_args() {
    let expr = Expr::MacroCall {
        name: "println".to_string(),
        args: vec![
            Expr::StringLit("value:".to_string()),
            Expr::Ident("x".to_string()),
        ],
        use_debug: vec![false, false],
    };
    assert_eq!(generate_expr(&expr), "println!(\"{} {}\", \"value:\", x)");
}

#[test]
fn test_generate_expr_macro_call_eprintln() {
    let expr = Expr::MacroCall {
        name: "eprintln".to_string(),
        args: vec![Expr::Ident("err".to_string())],
        use_debug: vec![false],
    };
    assert_eq!(generate_expr(&expr), "eprintln!(\"{}\", err)");
}

#[test]
fn test_generate_expr_macro_call_use_debug_single() {
    let expr = Expr::MacroCall {
        name: "println".to_string(),
        args: vec![Expr::Ident("arr".to_string())],
        use_debug: vec![true],
    };
    assert_eq!(generate_expr(&expr), "println!(\"{:?}\", arr)");
}

#[test]
fn test_generate_expr_macro_call_use_debug_mixed() {
    let expr = Expr::MacroCall {
        name: "println".to_string(),
        args: vec![
            Expr::StringLit("items:".to_string()),
            Expr::Ident("arr".to_string()),
        ],
        use_debug: vec![false, true],
    };
    assert_eq!(
        generate_expr(&expr),
        "println!(\"{} {:?}\", \"items:\", arr)"
    );
}

#[test]
fn test_generate_method_call_binary_op_receiver_needs_parens() {
    let expr = Expr::MethodCall {
        object: Box::new(Expr::BinaryOp {
            left: Box::new(Expr::Ident("a".to_string())),
            op: BinOp::Add,
            right: Box::new(Expr::Ident("b".to_string())),
        }),
        method: "sqrt".to_string(),
        args: vec![],
    };
    assert_eq!(generate_expr(&expr), "(a + b).sqrt()");
}

#[test]
fn test_generate_method_call_unary_op_receiver_needs_parens() {
    let expr = Expr::MethodCall {
        object: Box::new(Expr::UnaryOp {
            op: UnOp::Neg,
            operand: Box::new(Expr::Ident("x".to_string())),
        }),
        method: "abs".to_string(),
        args: vec![],
    };
    assert_eq!(generate_expr(&expr), "(-x).abs()");
}

#[test]
fn test_generate_method_call_cast_receiver_needs_parens() {
    let expr = Expr::MethodCall {
        object: Box::new(Expr::Cast {
            expr: Box::new(Expr::Ident("x".to_string())),
            target: RustType::F64,
        }),
        method: "abs".to_string(),
        args: vec![],
    };
    assert_eq!(generate_expr(&expr), "(x as f64).abs()");
}

#[test]
fn test_generate_method_call_ident_receiver_no_parens() {
    let expr = Expr::MethodCall {
        object: Box::new(Expr::Ident("x".to_string())),
        method: "abs".to_string(),
        args: vec![],
    };
    assert_eq!(generate_expr(&expr), "x.abs()");
}

#[test]
fn test_generate_method_call_chain_no_parens() {
    let expr = Expr::MethodCall {
        object: Box::new(Expr::MethodCall {
            object: Box::new(Expr::Ident("x".to_string())),
            method: "foo".to_string(),
            args: vec![],
        }),
        method: "bar".to_string(),
        args: vec![],
    };
    assert_eq!(generate_expr(&expr), "x.foo().bar()");
}

#[test]
fn test_generate_method_call_fn_call_receiver_no_parens() {
    let expr = Expr::MethodCall {
        object: Box::new(Expr::FnCall {
            name: "foo".to_string(),
            args: vec![],
        }),
        method: "bar".to_string(),
        args: vec![],
    };
    assert_eq!(generate_expr(&expr), "foo().bar()");
}

// --- static method :: separator ---

#[test]
fn test_generate_static_method_call_uses_double_colon() {
    let expr = Expr::MethodCall {
        object: Box::new(Expr::Ident("Foo".to_string())),
        method: "create".to_string(),
        args: vec![Expr::IntLit(1)],
    };
    assert_eq!(generate_expr(&expr), "Foo::create(1)");
}

#[test]
fn test_generate_instance_method_call_uses_dot() {
    let expr = Expr::MethodCall {
        object: Box::new(Expr::Ident("foo".to_string())),
        method: "create".to_string(),
        args: vec![],
    };
    assert_eq!(generate_expr(&expr), "foo.create()");
}

#[test]
fn test_generate_expr_vec_nested() {
    let expr = Expr::Vec {
        elements: vec![
            Expr::Vec {
                elements: vec![Expr::NumberLit(1.0)],
            },
            Expr::Vec {
                elements: vec![Expr::NumberLit(2.0)],
            },
        ],
    };
    assert_eq!(generate_expr(&expr), "vec![vec![1.0], vec![2.0]]");
}

// --- Rust reserved word escape tests ---

#[test]
fn test_escape_ident_method_call_reserved_word_adds_r_hash() {
    let expr = Expr::MethodCall {
        object: Box::new(Expr::Ident("obj".to_string())),
        method: "match".to_string(),
        args: vec![Expr::Ident("x".to_string())],
    };
    assert_eq!(generate_expr(&expr), "obj.r#match(x)");
}

#[test]
fn test_escape_ident_let_reserved_word_adds_r_hash() {
    let stmt = Stmt::Let {
        mutable: false,
        name: "type".to_string(),
        ty: None,
        init: Some(Expr::NumberLit(1.0)),
    };
    let result = generate_stmt(&stmt, 0);
    assert!(result.contains("r#type"), "expected r#type in: {result}");
}

#[test]
fn test_escape_ident_field_access_reserved_word_adds_r_hash() {
    let expr = Expr::FieldAccess {
        object: Box::new(Expr::Ident("obj".to_string())),
        field: "match".to_string(),
    };
    assert_eq!(generate_expr(&expr), "obj.r#match");
}

#[test]
fn test_escape_ident_non_reserved_word_unchanged() {
    let expr = Expr::MethodCall {
        object: Box::new(Expr::Ident("obj".to_string())),
        method: "foo".to_string(),
        args: vec![Expr::Ident("x".to_string())],
    };
    assert_eq!(generate_expr(&expr), "obj.foo(x)");
}

#[test]
fn test_generate_expr_deref_renders_star() {
    let expr = Expr::Deref(Box::new(Expr::Ident("x".to_string())));
    assert_eq!(generate_expr(&expr), "*x");
}

#[test]
fn test_generate_expr_ref_renders_ampersand() {
    let expr = Expr::Ref(Box::new(Expr::Ident("sep".to_string())));
    assert_eq!(generate_expr(&expr), "&sep");
}

#[test]
fn test_generate_expr_ref_number_renders_ampersand_literal() {
    let expr = Expr::Ref(Box::new(Expr::NumberLit(0.0)));
    assert_eq!(generate_expr(&expr), "&0.0");
}

#[test]
fn test_generate_expr_unit_renders_parens() {
    assert_eq!(generate_expr(&Expr::Unit), "()");
}

#[test]
fn test_generate_expr_int_lit_positive_renders_number() {
    assert_eq!(generate_expr(&Expr::IntLit(42)), "42");
}

#[test]
fn test_generate_expr_int_lit_negative_renders_negative() {
    assert_eq!(generate_expr(&Expr::IntLit(-1)), "-1");
}

#[test]
fn test_generate_expr_int_lit_zero_renders_zero() {
    assert_eq!(generate_expr(&Expr::IntLit(0)), "0");
}

#[test]
fn test_escape_ident_self_not_escaped() {
    let expr = Expr::FieldAccess {
        object: Box::new(Expr::Ident("self".to_string())),
        field: "x".to_string(),
    };
    assert_eq!(generate_expr(&expr), "self.x");
}

#[test]
fn test_generate_struct_init_with_base_renders_update_syntax() {
    let expr = Expr::StructInit {
        name: "Foo".to_string(),
        fields: vec![("key".to_string(), Expr::NumberLit(1.0))],
        base: Some(Box::new(Expr::Ident("other".to_string()))),
    };
    assert_eq!(generate_expr(&expr), "Foo { key: 1.0, ..other }");
}

#[test]
fn test_generate_struct_init_base_only_renders_update_syntax() {
    let expr = Expr::StructInit {
        name: "Foo".to_string(),
        fields: vec![],
        base: Some(Box::new(Expr::Ident("other".to_string()))),
    };
    assert_eq!(generate_expr(&expr), "Foo { ..other }");
}

#[test]
fn test_generate_expr_block_renders_block_expression() {
    let expr = Expr::Block(vec![
        Stmt::Let {
            mutable: true,
            name: "_v".to_string(),
            ty: None,
            init: Some(Expr::Vec {
                elements: vec![Expr::NumberLit(1.0)],
            }),
        },
        Stmt::TailExpr(Expr::Ident("_v".to_string())),
    ]);
    let expected = "{\n    let mut _v = vec![1.0];\n    _v\n}";
    assert_eq!(generate_expr(&expr), expected);
}

#[test]
fn test_generate_expr_match_with_enum_variant_bindings() {
    use crate::ir::MatchArm;
    let expr = Expr::Match {
        expr: Box::new(Expr::Ref(Box::new(Expr::Ident("s".to_string())))),
        arms: vec![
            MatchArm {
                patterns: vec![crate::ir::MatchPattern::EnumVariant {
                    path: "Shape::Circle".to_string(),
                    bindings: vec!["radius".to_string()],
                }],
                guard: None,
                body: vec![Stmt::TailExpr(Expr::MethodCall {
                    object: Box::new(Expr::Ident("radius".to_string())),
                    method: "clone".to_string(),
                    args: vec![],
                })],
            },
            MatchArm {
                patterns: vec![crate::ir::MatchPattern::Wildcard],
                guard: None,
                body: vec![Stmt::TailExpr(Expr::MacroCall {
                    name: "panic".to_string(),
                    args: vec![Expr::StringLit("unexpected variant".to_string())],
                    use_debug: vec![false],
                })],
            },
        ],
    };
    let expected = "match &s {\n    Shape::Circle { radius, .. } => {\n        radius.clone()\n    }\n    _ => {\n        panic!(\"unexpected variant\")\n    }\n}";
    assert_eq!(generate_expr(&expr), expected);
}

#[test]
fn test_generate_expr_runtime_typeof_produces_helper_call() {
    let expr = Expr::RuntimeTypeof {
        operand: Box::new(Expr::Ident("x".to_string())),
    };
    assert_eq!(generate_expr(&expr), "js_typeof(&x)");
}

#[test]
fn test_escape_rust_string_backslash() {
    assert_eq!(escape_rust_string(r"a\b"), r"a\\b");
}

#[test]
fn test_escape_rust_string_double_quote() {
    assert_eq!(escape_rust_string(r#"say "hello""#), r#"say \"hello\""#);
}

#[test]
fn test_escape_rust_string_newline_tab() {
    assert_eq!(escape_rust_string("a\nb"), r"a\nb");
    assert_eq!(escape_rust_string("a\tb"), r"a\tb");
}

#[test]
fn test_escape_rust_string_plain_text_unchanged() {
    assert_eq!(escape_rust_string("hello world"), "hello world");
}

#[test]
fn test_escape_rust_string_null_and_control_chars() {
    assert_eq!(escape_rust_string("\0"), r"\0");
    assert_eq!(escape_rust_string("\r"), r"\r");
}

#[test]
fn test_generate_string_lit_with_special_chars() {
    let expr = Expr::StringLit(r#"a"b\c"#.to_string());
    assert_eq!(generate_expr(&expr), r#""a\"b\\c""#);
}
