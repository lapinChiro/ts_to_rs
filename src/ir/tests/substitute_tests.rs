use std::collections::HashMap;

use super::super::*;

// --- I-100: RustType::substitute ---

#[test]
fn test_substitute_type_param_to_concrete() {
    // Named("T") に T→String → RustType::String
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
    let bindings = HashMap::from([("T".to_string(), RustType::String)]);
    assert_eq!(RustType::Bool.substitute(&bindings), RustType::Bool);
}

#[test]
fn test_substitute_named_type_args() {
    // Container<T> に T→String → Container<String>
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

// --- StructField::substitute ---

#[test]
fn test_struct_field_substitute_named_to_concrete() {
    let field = StructField {
        vis: Some(Visibility::Public),
        name: "value".to_string(),
        ty: RustType::Named {
            name: "T".to_string(),
            type_args: vec![],
        },
    };
    let bindings = HashMap::from([("T".to_string(), RustType::F64)]);
    let result = field.substitute(&bindings);
    assert_eq!(result.name, "value");
    assert_eq!(result.ty, RustType::F64);
    assert_eq!(result.vis, Some(Visibility::Public));
}

// --- Param::substitute ---

#[test]
fn test_param_substitute_named_to_concrete() {
    let param = Param {
        name: "x".to_string(),
        ty: Some(RustType::Named {
            name: "T".to_string(),
            type_args: vec![],
        }),
    };
    let bindings = HashMap::from([("T".to_string(), RustType::F64)]);
    let result = param.substitute(&bindings);
    assert_eq!(result.name, "x");
    assert_eq!(result.ty, Some(RustType::F64));
}

#[test]
fn test_param_substitute_none_ty_unchanged() {
    let param = Param {
        name: "x".to_string(),
        ty: None,
    };
    let bindings = HashMap::from([("T".to_string(), RustType::F64)]);
    let result = param.substitute(&bindings);
    assert_eq!(result.ty, None);
}

// --- Method::substitute ---

#[test]
fn test_method_substitute_params_and_return_type() {
    let method = Method {
        vis: Visibility::Public,
        name: "process".to_string(),
        has_self: true,
        has_mut_self: false,
        params: vec![Param {
            name: "input".to_string(),
            ty: Some(RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            }),
        }],
        return_type: Some(RustType::Vec(Box::new(RustType::Named {
            name: "T".to_string(),
            type_args: vec![],
        }))),
        body: None,
    };
    let bindings = HashMap::from([("T".to_string(), RustType::String)]);
    let result = method.substitute(&bindings);
    assert_eq!(result.params[0].ty, Some(RustType::String));
    assert_eq!(
        result.return_type,
        Some(RustType::Vec(Box::new(RustType::String)))
    );
}

#[test]
fn test_method_substitute_body_stmts() {
    let method = Method {
        vis: Visibility::Public,
        name: "make".to_string(),
        has_self: false,
        has_mut_self: false,
        params: vec![],
        return_type: None,
        body: Some(vec![Stmt::Let {
            mutable: false,
            name: "x".to_string(),
            ty: Some(RustType::Named {
                name: "T".to_string(),
                type_args: vec![],
            }),
            init: None,
        }]),
    };
    let bindings = HashMap::from([("T".to_string(), RustType::F64)]);
    let result = method.substitute(&bindings);
    match &result.body.as_ref().unwrap()[0] {
        Stmt::Let { ty, .. } => {
            assert_eq!(*ty, Some(RustType::F64));
        }
        _ => panic!("expected Let"),
    }
}

// --- Stmt::substitute ---

#[test]
fn test_stmt_let_substitute_ty() {
    let stmt = Stmt::Let {
        mutable: false,
        name: "x".to_string(),
        ty: Some(RustType::Named {
            name: "T".to_string(),
            type_args: vec![],
        }),
        init: None,
    };
    let bindings = HashMap::from([("T".to_string(), RustType::Bool)]);
    let result = stmt.substitute(&bindings);
    match result {
        Stmt::Let { ty, .. } => {
            assert_eq!(ty, Some(RustType::Bool));
        }
        _ => panic!("expected Let"),
    }
}

// --- Expr::substitute ---

#[test]
fn test_expr_cast_substitute_target() {
    let expr = Expr::Cast {
        expr: Box::new(Expr::Ident("x".to_string())),
        target: RustType::Named {
            name: "T".to_string(),
            type_args: vec![],
        },
    };
    let bindings = HashMap::from([("T".to_string(), RustType::F64)]);
    let result = expr.substitute(&bindings);
    match result {
        Expr::Cast { target, .. } => {
            assert_eq!(target, RustType::F64);
        }
        _ => panic!("expected Cast"),
    }
}
