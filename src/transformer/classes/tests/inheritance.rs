use super::*;
use crate::ir::CallTarget;

use super::super::inheritance::rewrite_super_constructor;

// --- Expected type propagation ---

/// Step 7: Static property initializer should propagate type annotation.
/// `static config: Config = { name: "default" }` should produce StructInit, not error.
#[test]
fn test_convert_static_prop_propagates_type_annotation() {
    let mut reg = TypeRegistry::new();
    reg.register(
        "Config".to_string(),
        crate::registry::TypeDef::new_struct(
            vec![("name".to_string(), RustType::String).into()],
            std::collections::HashMap::new(),
            vec![],
        ),
    );

    let source = r#"class Foo { static config: Config = { name: "default" }; }"#;
    let f = TctxFixture::from_source_with_reg(source, reg);
    let tctx = f.tctx();

    let decl = match &f.module().body[0] {
        ModuleItem::Stmt(ast::Stmt::Decl(Decl::Class(decl))) => decl.clone(),
        _ => panic!("expected ClassDecl"),
    };
    let items = Transformer::for_module(&tctx, &mut SyntheticTypeRegistry::new())
        .transform_class_with_inheritance(
            &decl,
            Visibility::Private,
            &HashMap::new(),
            &HashMap::new(),
        )
        .unwrap();

    // Find the Impl item with static consts
    let impl_item = items
        .iter()
        .find(|item| matches!(item, Item::Impl { .. }))
        .expect("expected Item::Impl");

    match impl_item {
        Item::Impl { consts, .. } => {
            assert_eq!(consts.len(), 1);
            assert_eq!(consts[0].name, "config");
            match &consts[0].value {
                crate::ir::Expr::StructInit { name, fields, .. } => {
                    assert_eq!(name, "Config");
                    assert_eq!(fields[0].0, "name");
                    assert!(
                        matches!(&fields[0].1, crate::ir::Expr::MethodCall { method, .. } if method == "to_string"),
                        "expected .to_string() on string field, got {:?}",
                        fields[0].1
                    );
                }
                other => panic!("expected StructInit, got {other:?}"),
            }
        }
        _ => unreachable!(),
    }
}

// --- rewrite_super_constructor ---

#[test]
fn test_rewrite_super_constructor_arg_count_mismatch_returns_error() {
    // Parent has 2 fields but child's super() only passes 1 arg
    let parent_info = ClassInfo {
        name: "Parent".to_string(),
        type_params: vec![],
        parent: None,
        parent_type_args: vec![],
        fields: vec![
            StructField {
                vis: None,
                name: "a".to_string(),
                ty: RustType::F64,
            },
            StructField {
                vis: None,
                name: "b".to_string(),
                ty: RustType::String,
            },
        ],
        constructor: None,
        methods: vec![],
        vis: Visibility::Private,
        implements: vec![],
        is_abstract: false,
        static_consts: vec![],
    };

    let child_ctor = Method {
        vis: Visibility::Public,
        name: "new".to_string(),
        has_self: false,
        has_mut_self: false,
        params: vec![Param {
            name: "x".to_string(),
            ty: Some(RustType::F64),
        }],
        return_type: Some(RustType::Named {
            name: "Self".to_string(),
            type_args: vec![],
        }),
        body: Some(vec![Stmt::Expr(Expr::FnCall {
            target: CallTarget::Super,
            args: vec![Expr::Ident("x".to_string())], // only 1 arg, parent has 2 fields
        })]),
    };

    let result = rewrite_super_constructor(&child_ctor, &parent_info);
    assert!(
        result.is_err(),
        "expected error for arg count mismatch, got: {:?}",
        result
    );
}

#[test]
fn test_rewrite_super_constructor_merges_into_tail_struct_init() {
    let parent_info = ClassInfo {
        name: "Parent".to_string(),
        type_params: vec![],
        parent: None,
        parent_type_args: vec![],
        fields: vec![StructField {
            vis: None,
            name: "x".to_string(),
            ty: RustType::F64,
        }],
        constructor: None,
        methods: vec![],
        vis: Visibility::Public,
        implements: vec![],
        is_abstract: false,
        static_consts: vec![],
    };

    let child_ctor = Method {
        vis: Visibility::Public,
        name: "new".to_string(),
        has_self: false,
        has_mut_self: false,
        params: vec![Param {
            name: "x".to_string(),
            ty: Some(RustType::F64),
        }],
        return_type: Some(RustType::Named {
            name: "Self".to_string(),
            type_args: vec![],
        }),
        body: Some(vec![
            // super(x)
            Stmt::Expr(Expr::FnCall {
                target: CallTarget::Super,
                args: vec![Expr::Ident("x".to_string())],
            }),
            // Self { age: 10 } as tail expression
            Stmt::TailExpr(Expr::StructInit {
                name: "Self".to_string(),
                fields: vec![("age".to_string(), Expr::NumberLit(10.0))],
                base: None,
            }),
        ]),
    };

    let result = rewrite_super_constructor(&child_ctor, &parent_info).unwrap();
    let body = result.body.as_ref().unwrap();
    // super() call should be removed, and super fields merged into TailExpr(StructInit)
    assert_eq!(body.len(), 1, "expected 1 statement, got: {body:?}");
    match &body[0] {
        Stmt::TailExpr(Expr::StructInit { fields, .. }) => {
            // super field "x" should be first, then child field "age"
            assert_eq!(fields.len(), 2, "expected 2 fields, got: {fields:?}");
            assert_eq!(fields[0].0, "x");
            assert_eq!(fields[1].0, "age");
        }
        other => panic!("expected TailExpr(StructInit), got: {other:?}"),
    }
}

#[test]
fn test_rewrite_super_constructor_merges_into_return_struct_init() {
    let parent_info = ClassInfo {
        name: "Parent".to_string(),
        type_params: vec![],
        parent: None,
        parent_type_args: vec![],
        fields: vec![StructField {
            vis: None,
            name: "name".to_string(),
            ty: RustType::String,
        }],
        constructor: None,
        methods: vec![],
        vis: Visibility::Public,
        implements: vec![],
        is_abstract: false,
        static_consts: vec![],
    };

    let child_ctor = Method {
        vis: Visibility::Public,
        name: "new".to_string(),
        has_self: false,
        has_mut_self: false,
        params: vec![Param {
            name: "name".to_string(),
            ty: Some(RustType::String),
        }],
        return_type: Some(RustType::Named {
            name: "Self".to_string(),
            type_args: vec![],
        }),
        body: Some(vec![
            Stmt::Expr(Expr::FnCall {
                target: CallTarget::Super,
                args: vec![Expr::Ident("name".to_string())],
            }),
            Stmt::Return(Some(Expr::StructInit {
                name: "Self".to_string(),
                fields: vec![("age".to_string(), Expr::NumberLit(25.0))],
                base: None,
            })),
        ]),
    };

    let result = rewrite_super_constructor(&child_ctor, &parent_info).unwrap();
    let body = result.body.as_ref().unwrap();
    assert_eq!(body.len(), 1);
    match &body[0] {
        Stmt::Return(Some(Expr::StructInit { fields, .. })) => {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].0, "name");
            assert_eq!(fields[1].0, "age");
        }
        other => panic!("expected Return(StructInit), got: {other:?}"),
    }
}

#[test]
fn test_rewrite_super_constructor_no_struct_init_creates_new() {
    let parent_info = ClassInfo {
        name: "Parent".to_string(),
        type_params: vec![],
        parent: None,
        parent_type_args: vec![],
        fields: vec![StructField {
            vis: None,
            name: "x".to_string(),
            ty: RustType::F64,
        }],
        constructor: None,
        methods: vec![],
        vis: Visibility::Public,
        implements: vec![],
        is_abstract: false,
        static_consts: vec![],
    };

    // Body has super() call but no StructInit — should create a new one
    let child_ctor = Method {
        vis: Visibility::Public,
        name: "new".to_string(),
        has_self: false,
        has_mut_self: false,
        params: vec![Param {
            name: "x".to_string(),
            ty: Some(RustType::F64),
        }],
        return_type: Some(RustType::Named {
            name: "Self".to_string(),
            type_args: vec![],
        }),
        body: Some(vec![
            Stmt::Expr(Expr::FnCall {
                target: CallTarget::Super,
                args: vec![Expr::Ident("x".to_string())],
            }),
            Stmt::Expr(Expr::FnCall {
                target: CallTarget::Free("println".to_string()),
                args: vec![],
            }),
        ]),
    };

    let result = rewrite_super_constructor(&child_ctor, &parent_info).unwrap();
    let body = result.body.as_ref().unwrap();
    // super() removed, println kept, and new StructInit appended
    assert_eq!(body.len(), 2, "expected 2 statements, got: {body:?}");
    match &body[0] {
        Stmt::Expr(Expr::FnCall { target, .. }) => {
            assert!(matches!(target, CallTarget::Free(ref __n) if __n == "println"))
        }
        other => panic!("expected println call, got: {other:?}"),
    }
    match &body[1] {
        Stmt::TailExpr(Expr::StructInit { name, fields, .. }) => {
            assert_eq!(name, "Self");
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].0, "x");
        }
        other => panic!("expected TailExpr(StructInit {{ Self }}), got: {other:?}"),
    }
}

#[test]
fn test_rewrite_super_constructor_no_super_call_preserves_body() {
    let parent_info = ClassInfo {
        name: "Parent".to_string(),
        type_params: vec![],
        parent: None,
        parent_type_args: vec![],
        fields: vec![StructField {
            vis: None,
            name: "x".to_string(),
            ty: RustType::F64,
        }],
        constructor: None,
        methods: vec![],
        vis: Visibility::Public,
        implements: vec![],
        is_abstract: false,
        static_consts: vec![],
    };

    // Body has no super() call — should be preserved as-is
    let child_ctor = Method {
        vis: Visibility::Public,
        name: "new".to_string(),
        has_self: false,
        has_mut_self: false,
        params: vec![],
        return_type: Some(RustType::Named {
            name: "Self".to_string(),
            type_args: vec![],
        }),
        body: Some(vec![Stmt::TailExpr(Expr::StructInit {
            name: "Self".to_string(),
            fields: vec![("x".to_string(), Expr::NumberLit(0.0))],
            base: None,
        })]),
    };

    let result = rewrite_super_constructor(&child_ctor, &parent_info).unwrap();
    let body = result.body.as_ref().unwrap();
    // No super() → no super fields extracted → existing StructInit preserved as-is
    assert_eq!(body.len(), 1);
    match &body[0] {
        Stmt::TailExpr(Expr::StructInit { fields, .. }) => {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].0, "x");
        }
        other => panic!("expected TailExpr(StructInit), got: {other:?}"),
    }
}
