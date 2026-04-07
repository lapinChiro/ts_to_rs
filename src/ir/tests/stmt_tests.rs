use super::super::*;

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
fn test_stmt_return() {
    let stmt = Stmt::Return(Some(Expr::NumberLit(1.0)));
    match stmt {
        Stmt::Return(Some(Expr::NumberLit(n))) => assert_eq!(n, 1.0),
        _ => panic!("expected Return"),
    }
}
