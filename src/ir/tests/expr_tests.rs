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
        target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Err),
        args: vec![Expr::StringLit("something went wrong".to_string())],
    };
    match expr {
        Expr::FnCall { target, args } => {
            assert!(matches!(
                target,
                CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Err)
            ));
            assert_eq!(args.len(), 1);
        }
        _ => panic!("expected FnCall"),
    }
}

#[test]
fn test_expr_fn_call_ok() {
    let expr = Expr::FnCall {
        target: CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Ok),
        args: vec![Expr::NumberLit(42.0)],
    };
    match expr {
        Expr::FnCall { target, args } => {
            assert!(matches!(
                target,
                CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Ok)
            ));
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
        target: CallTarget::Free("f".to_string()),
        args: vec![],
    }))
    .is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_field_access_of_fn_call_returns_false() {
    assert!(!Expr::FieldAccess {
        object: Box::new(Expr::FnCall {
            target: CallTarget::Free("get_obj".to_string()),
            args: vec![],
        }),
        field: "x".to_string(),
    }
    .is_trivially_pure());
}

#[test]
fn test_is_trivially_pure_fn_call_returns_false() {
    assert!(!Expr::FnCall {
        target: CallTarget::Free("side_effect".to_string()),
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
            target: CallTarget::Free("get".to_string()),
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
        target: CallTarget::Free("compute".to_string()),
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
// I-378: `CallTarget` 7-variant structural tests
// ---------------------------------------------------------------------------

#[test]
fn test_call_target_free_variant_holds_identifier() {
    let target = CallTarget::Free("foo".to_string());
    assert!(matches!(&target, CallTarget::Free(name) if name == "foo"));
}

#[test]
fn test_call_target_builtin_variant_each_constructor() {
    use crate::ir::BuiltinVariant;
    for v in [
        BuiltinVariant::Some,
        BuiltinVariant::None,
        BuiltinVariant::Ok,
        BuiltinVariant::Err,
    ] {
        let target = CallTarget::BuiltinVariant(v);
        assert!(matches!(target, CallTarget::BuiltinVariant(_)));
    }
}

#[test]
fn test_call_target_external_path_holds_segments() {
    let target = CallTarget::ExternalPath(vec![
        "std".to_string(),
        "fs".to_string(),
        "write".to_string(),
    ]);
    if let CallTarget::ExternalPath(segs) = &target {
        assert_eq!(segs, &["std", "fs", "write"]);
    } else {
        panic!("expected ExternalPath");
    }
}

#[test]
fn test_call_target_user_assoc_fn_holds_user_type_ref() {
    use crate::ir::UserTypeRef;
    let target = CallTarget::UserAssocFn {
        ty: UserTypeRef::new("MyClass"),
        method: "new".to_string(),
    };
    if let CallTarget::UserAssocFn { ty, method } = &target {
        assert_eq!(ty.as_str(), "MyClass");
        assert_eq!(method, "new");
    } else {
        panic!("expected UserAssocFn");
    }
}

#[test]
fn test_call_target_user_tuple_ctor_holds_user_type_ref() {
    use crate::ir::UserTypeRef;
    let target = CallTarget::UserTupleCtor(UserTypeRef::new("Wrapper"));
    if let CallTarget::UserTupleCtor(ty) = &target {
        assert_eq!(ty.as_str(), "Wrapper");
    } else {
        panic!("expected UserTupleCtor");
    }
}

#[test]
fn test_call_target_user_enum_variant_ctor_holds_enum_type_and_variant() {
    use crate::ir::UserTypeRef;
    let target = CallTarget::UserEnumVariantCtor {
        enum_ty: UserTypeRef::new("Color"),
        variant: "Red".to_string(),
    };
    if let CallTarget::UserEnumVariantCtor { enum_ty, variant } = &target {
        assert_eq!(enum_ty.as_str(), "Color");
        assert_eq!(variant, "Red");
    } else {
        panic!("expected UserEnumVariantCtor");
    }
}

#[test]
fn test_call_target_super_unit_variant() {
    let target = CallTarget::Super;
    assert!(matches!(target, CallTarget::Super));
}
