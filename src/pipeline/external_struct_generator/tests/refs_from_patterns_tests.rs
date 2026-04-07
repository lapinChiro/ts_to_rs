use super::*;
use crate::ir::{BuiltinVariant, PatternCtor, UserTypeRef};

fn user_enum(enum_name: &str, variant: &str) -> PatternCtor {
    PatternCtor::UserEnumVariant {
        enum_ty: UserTypeRef::new(enum_name),
        variant: variant.to_string(),
    }
}

fn user_struct(name: &str) -> PatternCtor {
    PatternCtor::UserStruct(UserTypeRef::new(name))
}

// Pattern walking — UserEnumVariant / UserStruct ctor → user type ref が refs に登録される
// =========================================================================

#[test]
fn test_collect_type_refs_match_arm_enum_variant_pattern() {
    // match x { Color::Red => ... } → Color が refs
    let item = fn_with_body(
        "f",
        vec![Stmt::Match {
            expr: Expr::Ident("x".to_string()),
            arms: vec![MatchArm {
                patterns: vec![Pattern::UnitStruct {
                    ctor: user_enum("Color", "Red"),
                }],
                guard: None,
                body: vec![],
            }],
        }],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Color"));
}

#[test]
fn test_collect_type_refs_match_arm_lowercase_enum_captured() {
    // I-380: lowercase 始まりの enum 名 (`myenum::bar`) も `UserEnumVariant`
    // として構造的に refs に登録される。
    let item = fn_with_body(
        "f",
        vec![Stmt::Match {
            expr: Expr::Ident("x".to_string()),
            arms: vec![MatchArm {
                patterns: vec![Pattern::UnitStruct {
                    ctor: user_enum("myenum", "bar"),
                }],
                guard: None,
                body: vec![],
            }],
        }],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("myenum"));
}

#[test]
fn test_collect_type_refs_match_arm_literal_walks_expr() {
    // match x { 1 => Wrapper { } } の本体 StructInit を拾う
    let item = fn_with_body(
        "f",
        vec![Stmt::Match {
            expr: Expr::Ident("x".to_string()),
            arms: vec![MatchArm {
                patterns: vec![Pattern::Literal(Expr::IntLit(1))],
                guard: None,
                body: vec![Stmt::TailExpr(Expr::StructInit {
                    name: "Wrapper".to_string(),
                    fields: vec![],
                    base: None,
                })],
            }],
        }],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Wrapper"));
}

#[test]
fn test_collect_type_refs_match_arm_guard_walked() {
    // match x { _ if Wrapper { }.is_valid() => ... } の guard 内 StructInit を拾う
    let item = fn_with_body(
        "f",
        vec![Stmt::Match {
            expr: Expr::Ident("x".to_string()),
            arms: vec![MatchArm {
                patterns: vec![Pattern::Wildcard],
                guard: Some(Expr::StructInit {
                    name: "Wrapper".to_string(),
                    fields: vec![],
                    base: None,
                }),
                body: vec![],
            }],
        }],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Wrapper"));
}

// =========================================================================
// 構造化 Pattern walking — Stmt::IfLet / Stmt::WhileLet / Expr::Matches
// =========================================================================

#[test]
fn test_collect_type_refs_stmt_iflet_pattern() {
    let item = fn_with_body(
        "f",
        vec![Stmt::IfLet {
            pattern: Pattern::UnitStruct {
                ctor: user_enum("Color", "Red"),
            },
            expr: Expr::Ident("x".to_string()),
            then_body: vec![],
            else_body: None,
        }],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Color"));
}

#[test]
fn test_collect_type_refs_stmt_whilelet_pattern() {
    let item = fn_with_body(
        "f",
        vec![Stmt::WhileLet {
            label: None,
            pattern: Pattern::TupleStruct {
                ctor: user_enum("Color", "Red"),
                fields: vec![Pattern::binding("x")],
            },
            expr: Expr::Ident("it".to_string()),
            body: vec![],
        }],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Color"));
}

#[test]
fn test_collect_type_refs_expr_matches_pattern() {
    let item = fn_with_body(
        "f",
        vec![Stmt::TailExpr(Expr::Matches {
            expr: Box::new(Expr::Ident("x".to_string())),
            pattern: Box::new(Pattern::TupleStruct {
                ctor: user_enum("Color", "Red"),
                fields: vec![Pattern::Wildcard],
            }),
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Color"));
}

#[test]
fn test_collect_type_refs_pattern_struct_form() {
    // if let Foo { x, .. } = ... → Foo が refs (UserStruct ctor)
    let item = fn_with_body(
        "f",
        vec![Stmt::IfLet {
            pattern: Pattern::Struct {
                ctor: user_struct("Foo"),
                fields: vec![("x".to_string(), Pattern::binding("x"))],
                rest: true,
            },
            expr: Expr::Ident("y".to_string()),
            then_body: vec![],
            else_body: None,
        }],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Foo"));
}

#[test]
fn test_collect_type_refs_pattern_wildcard_no_extraction() {
    let item = fn_with_body(
        "f",
        vec![Stmt::IfLet {
            pattern: Pattern::Wildcard,
            expr: Expr::Ident("x".to_string()),
            then_body: vec![],
            else_body: None,
        }],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.is_empty());
}

#[test]
fn test_collect_type_refs_pattern_builtin_variants_excluded_structurally() {
    // I-380: `Some`/`None`/`Ok`/`Err` は `PatternCtor::Builtin` で構造的に
    // 区別され、`visit_user_type_ref` フックを発火しないため refs に登録されない。
    // (PATTERN_LANG_BUILTINS 文字列除外リストは構造的に不要になった)
    for (pat, name) in [
        (
            Pattern::TupleStruct {
                ctor: PatternCtor::Builtin(BuiltinVariant::Some),
                fields: vec![Pattern::binding("x")],
            },
            "Some",
        ),
        (Pattern::none(), "None"),
        (
            Pattern::TupleStruct {
                ctor: PatternCtor::Builtin(BuiltinVariant::Ok),
                fields: vec![Pattern::binding("v")],
            },
            "Ok",
        ),
        (
            Pattern::TupleStruct {
                ctor: PatternCtor::Builtin(BuiltinVariant::Err),
                fields: vec![Pattern::binding("e")],
            },
            "Err",
        ),
    ] {
        let item = fn_with_body(
            "f",
            vec![Stmt::IfLet {
                pattern: pat,
                expr: Expr::Ident("x".to_string()),
                then_body: vec![],
                else_body: None,
            }],
        );
        let mut refs = HashSet::new();
        collect_type_refs_from_item(&item, &mut refs);
        assert!(!refs.contains(name), "{name} should be excluded from refs");
    }
}
