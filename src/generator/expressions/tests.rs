use super::*;
use crate::ir::{BinOp, CallTarget, ClosureBody, Expr, Param, RustType, Stmt, UnOp};

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
        target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Err),
        args: vec![Expr::StringLit("error".to_string())],
    };
    assert_eq!(generate_expr(&expr), "Err(\"error\")");
}

#[test]
fn test_generate_expr_fn_call_ok() {
    let expr = Expr::FnCall {
        target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Ok),
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
            target: CallTarget::Free("foo".to_string()),
            args: vec![],
        }),
        method: "bar".to_string(),
        args: vec![],
    };
    assert_eq!(generate_expr(&expr), "foo().bar()");
}

// --- I-378: static method calls are now CallTarget::UserAssocFn (FnCall) ---
// MethodCall is strictly for instance method calls (`.method()` separator).

#[test]
fn test_generate_static_method_call_via_user_assoc_fn() {
    // I-378: `Foo::create(1)` is now `FnCall { UserAssocFn { ty: "Foo", method: "create" } }`,
    // not `MethodCall { Ident("Foo"), "create" }`. The generator's `is_type_ident`
    // uppercase heuristic was removed; classification is now structural at the
    // Transformer layer.
    let expr = Expr::FnCall {
        target: CallTarget::UserAssocFn {
            ty: crate::ir::UserTypeRef::new("Foo"),
            method: "create".to_string(),
        },
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
                patterns: vec![crate::ir::Pattern::Struct {
                    ctor: crate::ir::PatternCtor::UserEnumVariant {
                        enum_ty: crate::ir::UserTypeRef::new("Shape"),
                        variant: "Circle".to_string(),
                    },
                    fields: vec![("radius".to_string(), crate::ir::Pattern::binding("radius"))],
                    rest: true,
                }],
                guard: None,
                body: vec![Stmt::TailExpr(Expr::MethodCall {
                    object: Box::new(Expr::Ident("radius".to_string())),
                    method: "clone".to_string(),
                    args: vec![],
                })],
            },
            MatchArm {
                patterns: vec![crate::ir::Pattern::Wildcard],
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

// ---------------------------------------------------------------------------
// I-375: `Expr::FnCall` generator tests for each `CallTarget` variant
// ---------------------------------------------------------------------------

#[test]
fn test_generate_fn_call_single_segment_path() {
    let expr = Expr::FnCall {
        target: CallTarget::Free("foo".to_string()),
        args: vec![Expr::NumberLit(1.0), Expr::NumberLit(2.0)],
    };
    assert_eq!(generate_expr(&expr), "foo(1.0, 2.0)");
}

#[test]
fn test_generate_fn_call_two_segment_assoc_path() {
    // `Color::Red(x)` — synthetic enum variant constructor output.
    // The generator joins segments with `::` and emits the args verbatim;
    // any `.to_string()` wrapping is the Transformer's responsibility.
    let expr = Expr::FnCall {
        target: CallTarget::UserAssocFn {
            ty: crate::ir::UserTypeRef::new("Color"),
            method: "Red".to_string(),
        },
        args: vec![Expr::StringLit("red".to_string())],
    };
    assert_eq!(generate_expr(&expr), "Color::Red(\"red\")");
}

#[test]
fn test_generate_fn_call_multi_segment_path() {
    // `std::fs::write(path, data)` — a multi-segment std call
    let expr = Expr::FnCall {
        target: CallTarget::ExternalPath(vec![
            "std".to_string(),
            "fs".to_string(),
            "write".to_string(),
        ]),
        args: vec![
            Expr::Ident("path".to_string()),
            Expr::Ident("data".to_string()),
        ],
    };
    assert_eq!(generate_expr(&expr), "std::fs::write(path, data)");
}

#[test]
fn test_generate_fn_call_super() {
    let expr = Expr::FnCall {
        target: CallTarget::Super,
        args: vec![Expr::Ident("x".to_string())],
    };
    assert_eq!(generate_expr(&expr), "super(x)");
}

#[test]
fn test_generate_fn_call_super_no_args() {
    let expr = Expr::FnCall {
        target: CallTarget::Super,
        args: vec![],
    };
    assert_eq!(generate_expr(&expr), "super()");
}

#[test]
fn test_generate_fn_call_user_tuple_ctor_emits_bare_type_name() {
    // `Wrapper(x)` for callable interface tuple struct constructor.
    let expr = Expr::FnCall {
        target: CallTarget::UserTupleCtor(crate::ir::UserTypeRef::new("Wrapper")),
        args: vec![Expr::IntLit(42)],
    };
    assert_eq!(generate_expr(&expr), "Wrapper(42)");
}

#[test]
fn test_generate_fn_call_user_enum_variant_ctor_emits_enum_path() {
    // `Color::Red(x)` — payload enum variant constructor.
    let expr = Expr::FnCall {
        target: CallTarget::UserEnumVariantCtor {
            enum_ty: crate::ir::UserTypeRef::new("Color"),
            variant: "Red".to_string(),
        },
        args: vec![Expr::StringLit("red".to_string())],
    };
    assert_eq!(generate_expr(&expr), "Color::Red(\"red\")");
}

#[test]
fn test_generate_fn_call_builtin_variant_some_and_none() {
    // `Some(x)` / `None` — Option constructors.
    let some_expr = Expr::FnCall {
        target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Some),
        args: vec![Expr::IntLit(1)],
    };
    assert_eq!(generate_expr(&some_expr), "Some(1)");

    let none_expr = Expr::FnCall {
        target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::None),
        args: vec![],
    };
    assert_eq!(generate_expr(&none_expr), "None()");

    // I-379: `None` as a value reference (not a call) is structured as
    // `Expr::BuiltinVariantValue(BuiltinVariant::None)`.
    let none_value = Expr::BuiltinVariantValue(crate::ir::BuiltinVariant::None);
    assert_eq!(generate_expr(&none_value), "None");
    let some_value = Expr::BuiltinVariantValue(crate::ir::BuiltinVariant::Some);
    assert_eq!(generate_expr(&some_value), "Some");
    let ok_value = Expr::BuiltinVariantValue(crate::ir::BuiltinVariant::Ok);
    assert_eq!(generate_expr(&ok_value), "Ok");
    let err_value = Expr::BuiltinVariantValue(crate::ir::BuiltinVariant::Err);
    assert_eq!(generate_expr(&err_value), "Err");
}

#[test]
fn test_generate_fn_call_user_assoc_fn_emits_qualified_path() {
    // I-378: `CallTarget::UserAssocFn` は `UserTypeRef` を保持し、generator は
    // 単純に `{ty}::{method}(args)` を emit する。I-375 の `Path { type_ref }` は
    // metadata 形式だったが、I-378 で構造的に区別されるようになった。
    let target = Expr::FnCall {
        target: CallTarget::UserAssocFn {
            ty: crate::ir::UserTypeRef::new("myClass"),
            method: "new".to_string(),
        },
        args: vec![],
    };
    assert_eq!(generate_expr(&target), "myClass::new()");
}

// I-378 Phase 1: rendering tests for the 3 new structured Expr variants.

#[test]
fn renders_enum_variant_as_qualified_path() {
    let expr = Expr::EnumVariant {
        enum_ty: crate::ir::UserTypeRef::new("Color"),
        variant: "Red".to_string(),
    };
    assert_eq!(generate_expr(&expr), "Color::Red");
}

#[test]
fn renders_primitive_assoc_const_as_qualified_path() {
    let nan = Expr::PrimitiveAssocConst {
        ty: crate::ir::PrimitiveType::F64,
        name: "NAN".to_string(),
    };
    assert_eq!(generate_expr(&nan), "f64::NAN");

    let i32_max = Expr::PrimitiveAssocConst {
        ty: crate::ir::PrimitiveType::I32,
        name: "MAX".to_string(),
    };
    assert_eq!(generate_expr(&i32_max), "i32::MAX");
}

#[test]
fn renders_std_const_via_rust_path() {
    assert_eq!(
        generate_expr(&Expr::StdConst(crate::ir::StdConst::F64Pi)),
        "std::f64::consts::PI"
    );
    assert_eq!(
        generate_expr(&Expr::StdConst(crate::ir::StdConst::F64Ln2)),
        "std::f64::consts::LN_2"
    );
    assert_eq!(
        generate_expr(&Expr::StdConst(crate::ir::StdConst::F64Sqrt2)),
        "std::f64::consts::SQRT_2"
    );
}
