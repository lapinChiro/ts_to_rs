use super::super::*;

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
        target: CallTarget::simple("Err"),
        args: vec![Expr::StringLit("something went wrong".to_string())],
    };
    match expr {
        Expr::FnCall { target, args } => {
            assert_eq!(target.as_simple(), Some("Err"));
            assert_eq!(args.len(), 1);
        }
        _ => panic!("expected FnCall"),
    }
}

#[test]
fn test_expr_fn_call_ok() {
    let expr = Expr::FnCall {
        target: CallTarget::simple("Ok"),
        args: vec![Expr::NumberLit(42.0)],
    };
    match expr {
        Expr::FnCall { target, args } => {
            assert_eq!(target.as_simple(), Some("Ok"));
            assert_eq!(args.len(), 1);
        }
        _ => panic!("expected FnCall"),
    }
}

// -- BinOp tests --

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

// -- Expr::is_trivially_pure tests --

#[test]
fn test_is_trivially_pure_number_lit_returns_true() {
    assert!(Expr::NumberLit(42.0).is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_int_lit_returns_true() {
    assert!(Expr::IntLit(42).is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_string_lit_returns_true() {
    assert!(Expr::StringLit("hello".to_string()).is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_bool_lit_returns_true() {
    assert!(Expr::BoolLit(true).is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_ident_returns_true() {
    assert!(Expr::Ident("x".to_string()).is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_unit_returns_true() {
    assert!(Expr::Unit.is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_ref_of_pure_returns_true() {
    assert!(Expr::Ref(Box::new(Expr::Ident("x".to_string()))).is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_deref_of_pure_returns_true() {
    assert!(Expr::Deref(Box::new(Expr::Ident("x".to_string()))).is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_await_returns_false() {
    assert!(!Expr::Await(Box::new(Expr::Ident("x".to_string()))).is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_field_access_of_pure_returns_true() {
    assert!(Expr::FieldAccess {
        object: Box::new(Expr::Ident("p".to_string())),
        field: "x".to_string(),
    }
    .is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_nested_field_access_of_pure_returns_true() {
    assert!(Expr::FieldAccess {
        object: Box::new(Expr::FieldAccess {
            object: Box::new(Expr::Ident("a".to_string())),
            field: "b".to_string(),
        }),
        field: "c".to_string(),
    }
    .is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_ref_of_fn_call_returns_false() {
    assert!(!Expr::Ref(Box::new(Expr::FnCall {
        target: CallTarget::simple("f"),
        args: vec![],
    }))
    .is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_field_access_of_fn_call_returns_false() {
    assert!(!Expr::FieldAccess {
        object: Box::new(Expr::FnCall {
            target: CallTarget::simple("get_obj"),
            args: vec![],
        }),
        field: "x".to_string(),
    }
    .is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_fn_call_returns_false() {
    assert!(!Expr::FnCall {
        target: CallTarget::simple("side_effect"),
        args: vec![],
    }
    .is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_method_call_push_returns_false() {
    assert!(!Expr::MethodCall {
        object: Box::new(Expr::Ident("x".to_string())),
        method: "push".to_string(),
        args: vec![],
    }
    .is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_method_call_to_string_of_pure_returns_true() {
    assert!(Expr::MethodCall {
        object: Box::new(Expr::StringLit("hello".to_string())),
        method: "to_string".to_string(),
        args: vec![],
    }
    .is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_method_call_clone_of_pure_returns_true() {
    assert!(Expr::MethodCall {
        object: Box::new(Expr::Ident("x".to_string())),
        method: "clone".to_string(),
        args: vec![],
    }
    .is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_method_call_to_string_of_fn_call_returns_false() {
    assert!(!Expr::MethodCall {
        object: Box::new(Expr::FnCall {
            target: CallTarget::simple("get"),
            args: vec![],
        }),
        method: "to_string".to_string(),
        args: vec![],
    }
    .is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_assign_returns_false() {
    assert!(!Expr::Assign {
        target: Box::new(Expr::Ident("x".to_string())),
        value: Box::new(Expr::NumberLit(1.0)),
    }
    .is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_macro_call_returns_false() {
    assert!(!Expr::MacroCall {
        name: "println".to_string(),
        args: vec![],
        use_debug: vec![],
    }
    .is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_block_returns_false() {
    assert!(!Expr::Block(vec![]).is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_binary_op_returns_false() {
    // Conservative: binary ops could theoretically involve operator overloading
    assert!(!Expr::BinaryOp {
        left: Box::new(Expr::NumberLit(1.0)),
        op: BinOp::Add,
        right: Box::new(Expr::NumberLit(2.0)),
    }
    .is_trivially_pure());
}

// -- Expr::is_copy_literal tests --

#[test]
fn test_is_copy_literal_number_lit_returns_true() {
    assert!(Expr::NumberLit(0.0).is_copy_literal());
}

#[test]
fn test_is_copy_literal_int_lit_returns_true() {
    assert!(Expr::IntLit(42).is_copy_literal());
}

#[test]
fn test_is_copy_literal_bool_lit_returns_true() {
    assert!(Expr::BoolLit(false).is_copy_literal());
}

#[test]
fn test_is_copy_literal_unit_returns_true() {
    assert!(Expr::Unit.is_copy_literal());
}

#[test]
fn test_is_copy_literal_string_lit_returns_false() {
    // StringLit generates String allocation (.to_string()), not Copy
    assert!(!Expr::StringLit("hello".to_string()).is_copy_literal());
}

#[test]
fn test_is_copy_literal_ident_returns_false() {
    // Ident may be non-Copy type, cannot determine at IR level
    assert!(!Expr::Ident("x".to_string()).is_copy_literal());
}

#[test]
fn test_is_copy_literal_fn_call_returns_false() {
    assert!(!Expr::FnCall {
        target: CallTarget::simple("compute"),
        args: vec![],
    }
    .is_copy_literal());
}

#[test]
fn test_is_copy_literal_method_call_returns_false() {
    assert!(!Expr::MethodCall {
        object: Box::new(Expr::Ident("obj".to_string())),
        method: "get_value".to_string(),
        args: vec![],
    }
    .is_copy_literal());
}

#[test]
fn test_is_copy_literal_struct_init_returns_false() {
    assert!(!Expr::StructInit {
        name: "Config".to_string(),
        fields: vec![],
        base: None,
    }
    .is_copy_literal());
}

#[test]
fn test_is_copy_literal_field_access_returns_false() {
    assert!(!Expr::FieldAccess {
        object: Box::new(Expr::Ident("self".to_string())),
        field: "x".to_string(),
    }
    .is_copy_literal());
}

// ---------------------------------------------------------------------------
// I-375: `CallTarget` enum — helper constructors and pattern-match utilities
// ---------------------------------------------------------------------------

#[test]
fn test_call_target_simple_constructs_single_segment_path_without_type_ref() {
    let target = CallTarget::simple("foo");
    match target {
        CallTarget::Path { segments, type_ref } => {
            assert_eq!(segments, vec!["foo".to_string()]);
            assert_eq!(type_ref, None);
        }
        _ => panic!("expected CallTarget::Path"),
    }
}

#[test]
fn test_call_target_assoc_constructs_two_segment_path_with_type_ref() {
    let target = CallTarget::assoc("MyClass", "new");
    match target {
        CallTarget::Path { segments, type_ref } => {
            assert_eq!(segments, vec!["MyClass".to_string(), "new".to_string()]);
            assert_eq!(type_ref, Some("MyClass".to_string()));
        }
        _ => panic!("expected CallTarget::Path"),
    }
}

#[test]
fn test_call_target_path_constructs_multi_segment_without_type_ref() {
    let target = CallTarget::path(&["std", "fs", "write"]);
    match target {
        CallTarget::Path { segments, type_ref } => {
            assert_eq!(
                segments,
                vec!["std".to_string(), "fs".to_string(), "write".to_string()]
            );
            assert_eq!(type_ref, None);
        }
        _ => panic!("expected CallTarget::Path"),
    }
}

#[test]
fn test_call_target_super_variant_constructs() {
    // `Super` is a unit variant; simply verify it round-trips through match.
    let target = CallTarget::Super;
    assert!(matches!(target, CallTarget::Super));
}

#[test]
fn test_call_target_as_simple_returns_single_segment_name() {
    let target = CallTarget::simple("Err");
    assert_eq!(target.as_simple(), Some("Err"));
}

#[test]
fn test_call_target_as_simple_returns_none_for_multi_segment_path() {
    // Multi-segment paths must NOT look like a single identifier.
    let target = CallTarget::assoc("Color", "Red");
    assert_eq!(target.as_simple(), None);
    let target = CallTarget::path(&["std", "mem", "take"]);
    assert_eq!(target.as_simple(), None);
}

#[test]
fn test_call_target_as_simple_returns_none_for_super() {
    let target = CallTarget::Super;
    assert_eq!(target.as_simple(), None);
}

#[test]
fn test_call_target_is_path_matches_exact_segments() {
    let target = CallTarget::path(&["scopeguard", "guard"]);
    assert!(target.is_path(&["scopeguard", "guard"]));
}

#[test]
fn test_call_target_is_path_rejects_length_mismatch() {
    let target = CallTarget::path(&["std", "mem", "take"]);
    assert!(!target.is_path(&["std", "mem"]));
    assert!(!target.is_path(&["std", "mem", "take", "extra"]));
}

#[test]
fn test_call_target_is_path_rejects_segment_mismatch() {
    let target = CallTarget::path(&["std", "fs", "read"]);
    assert!(!target.is_path(&["std", "fs", "write"]));
}

#[test]
fn test_call_target_is_path_rejects_super_variant() {
    // `Super` must never compare equal to any identifier path.
    assert!(!CallTarget::Super.is_path(&["super"]));
    assert!(!CallTarget::Super.is_path(&[]));
}

#[test]
fn test_call_target_is_path_matches_single_segment() {
    let target = CallTarget::simple("foo");
    assert!(target.is_path(&["foo"]));
    assert!(!target.is_path(&["bar"]));
}

#[test]
fn test_call_target_assoc_type_ref_preserves_first_segment() {
    // The `type_ref` must literally match the first segment so the reference
    // walker registers the same identifier that generator emits.
    let target = CallTarget::assoc("lowerCaseClass", "new");
    match target {
        CallTarget::Path { segments, type_ref } => {
            assert_eq!(segments[0], "lowerCaseClass");
            assert_eq!(type_ref.as_deref(), Some("lowerCaseClass"));
        }
        _ => panic!("expected Path"),
    }
}
