use std::collections::HashMap;

use super::super::*;

// --- I-100: RustType::substitute ---

#[test]
fn test_substitute_type_param_to_concrete() {
    // Named("T") に T→String → RustType::String
    let ty = RustType::TypeVar {
        name: "T".to_string(),
    };
    let bindings = HashMap::from([("T".to_string(), RustType::String)]);
    assert_eq!(ty.substitute(&bindings), RustType::String);
}

#[test]
fn test_substitute_vec_recursive() {
    // Vec<T> に T→F64 → Vec<F64>
    let ty = RustType::Vec(Box::new(RustType::TypeVar {
        name: "T".to_string(),
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
    let ty = RustType::Option(Box::new(RustType::Vec(Box::new(RustType::TypeVar {
        name: "T".to_string(),
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
fn test_substitute_qself_substitutes_qself_inner() {
    // <T as Promise>::Output に T→String → <String as Promise>::Output
    let ty = RustType::QSelf {
        qself: Box::new(RustType::TypeVar {
            name: "T".to_string(),
        }),
        trait_ref: TraitRef {
            name: "Promise".to_string(),
            type_args: vec![],
        },
        item: "Output".to_string(),
    };
    let bindings = HashMap::from([("T".to_string(), RustType::String)]);
    let expected = RustType::QSelf {
        qself: Box::new(RustType::String),
        trait_ref: TraitRef {
            name: "Promise".to_string(),
            type_args: vec![],
        },
        item: "Output".to_string(),
    };
    assert_eq!(ty.substitute(&bindings), expected);
}

#[test]
fn test_substitute_qself_substitutes_trait_args() {
    // <Self as Container<T>>::Item に T→F64 → <Self as Container<F64>>::Item
    let ty = RustType::QSelf {
        qself: Box::new(RustType::Named {
            name: "Self".to_string(),
            type_args: vec![],
        }),
        trait_ref: TraitRef {
            name: "Container".to_string(),
            type_args: vec![RustType::TypeVar {
                name: "T".to_string(),
            }],
        },
        item: "Item".to_string(),
    };
    let bindings = HashMap::from([("T".to_string(), RustType::F64)]);
    let result = ty.substitute(&bindings);
    if let RustType::QSelf { trait_ref, .. } = &result {
        assert_eq!(trait_ref.type_args, vec![RustType::F64]);
    } else {
        panic!("expected QSelf, got {result:?}");
    }
}

#[test]
fn test_uses_param_qself_detects_param_in_qself_inner() {
    // <T as Promise>::Output が T を使用していることを検出
    let ty = RustType::QSelf {
        qself: Box::new(RustType::TypeVar {
            name: "T".to_string(),
        }),
        trait_ref: TraitRef {
            name: "Promise".to_string(),
            type_args: vec![],
        },
        item: "Output".to_string(),
    };
    assert!(ty.uses_param("T"));
    assert!(!ty.uses_param("U"));
}

#[test]
fn test_uses_param_qself_detects_param_in_trait_args() {
    // <X as Container<T>>::Item が T を使用していることを検出
    let ty = RustType::QSelf {
        qself: Box::new(RustType::TypeVar {
            name: "X".to_string(),
        }),
        trait_ref: TraitRef {
            name: "Container".to_string(),
            type_args: vec![RustType::TypeVar {
                name: "T".to_string(),
            }],
        },
        item: "Item".to_string(),
    };
    assert!(ty.uses_param("T"));
}

#[test]
fn test_uses_param_qself_detects_param_as_trait_name() {
    // <X as T>::Item — trait 名そのものが型パラメータ（理論上の境界ケース）
    let ty = RustType::QSelf {
        qself: Box::new(RustType::TypeVar {
            name: "X".to_string(),
        }),
        trait_ref: TraitRef {
            name: "T".to_string(),
            type_args: vec![],
        },
        item: "Item".to_string(),
    };
    assert!(ty.uses_param("T"));
}

#[test]
fn test_substitute_named_type_args() {
    // Container<T> に T→String → Container<String>
    let ty = RustType::Named {
        name: "Container".to_string(),
        type_args: vec![RustType::TypeVar {
            name: "T".to_string(),
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
        ty: RustType::TypeVar {
            name: "T".to_string(),
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
        ty: Some(RustType::TypeVar {
            name: "T".to_string(),
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
            ty: Some(RustType::TypeVar {
                name: "T".to_string(),
            }),
        }],
        return_type: Some(RustType::Vec(Box::new(RustType::TypeVar {
            name: "T".to_string(),
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
            ty: Some(RustType::TypeVar {
                name: "T".to_string(),
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
        ty: Some(RustType::TypeVar {
            name: "T".to_string(),
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
        target: RustType::TypeVar {
            name: "T".to_string(),
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

// --- I-387: TypeVar / StdCollection / Primitive substitute ---

#[test]
fn test_substitute_replaces_type_var() {
    let ty = RustType::TypeVar {
        name: "T".to_string(),
    };
    let bindings = HashMap::from([("T".to_string(), RustType::String)]);
    assert_eq!(ty.substitute(&bindings), RustType::String);
}

#[test]
fn test_substitute_leaves_type_var_unbound() {
    let ty = RustType::TypeVar {
        name: "T".to_string(),
    };
    let bindings: HashMap<String, RustType> = HashMap::new();
    assert_eq!(
        ty.substitute(&bindings),
        RustType::TypeVar {
            name: "T".to_string(),
        }
    );
}

#[test]
fn test_substitute_recurses_into_std_collection_args() {
    let ty = RustType::StdCollection {
        kind: StdCollectionKind::Box,
        args: vec![RustType::TypeVar {
            name: "T".to_string(),
        }],
    };
    let bindings = HashMap::from([("T".to_string(), RustType::F64)]);
    assert_eq!(
        ty.substitute(&bindings),
        RustType::StdCollection {
            kind: StdCollectionKind::Box,
            args: vec![RustType::F64],
        }
    );
}

#[test]
fn test_substitute_leaves_primitive_unchanged() {
    let ty = RustType::Primitive(PrimitiveIntKind::Usize);
    let bindings = HashMap::from([("usize".to_string(), RustType::String)]);
    // `Primitive` は `Named` とは構造的に区別されるため、"usize" binding にも
    // 影響を受けない。
    assert_eq!(
        ty.substitute(&bindings),
        RustType::Primitive(PrimitiveIntKind::Usize)
    );
}

#[test]
fn test_substitute_recurses_into_nested_std_collection() {
    // HashMap<String, Box<T>>
    let ty = RustType::StdCollection {
        kind: StdCollectionKind::HashMap,
        args: vec![
            RustType::String,
            RustType::StdCollection {
                kind: StdCollectionKind::Box,
                args: vec![RustType::TypeVar {
                    name: "T".to_string(),
                }],
            },
        ],
    };
    let bindings = HashMap::from([("T".to_string(), RustType::F64)]);
    let expected = RustType::StdCollection {
        kind: StdCollectionKind::HashMap,
        args: vec![
            RustType::String,
            RustType::StdCollection {
                kind: StdCollectionKind::Box,
                args: vec![RustType::F64],
            },
        ],
    };
    assert_eq!(ty.substitute(&bindings), expected);
}
