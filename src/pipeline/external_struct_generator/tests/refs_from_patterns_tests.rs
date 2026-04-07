use super::*;

// T8c: MatchArm pattern walking — EnumVariant.path uppercase extraction
// =========================================================================

#[test]
fn test_collect_type_refs_match_arm_enum_variant_pattern() {
    // match x { Color::Red { .. } => ... } → Color が refs
    let item = fn_with_body(
        "f",
        vec![Stmt::Match {
            expr: Expr::Ident("x".to_string()),
            arms: vec![MatchArm {
                patterns: vec![Pattern::UnitStruct {
                    path: vec!["Color".to_string(), "Red".to_string()],
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
fn test_collect_type_refs_match_arm_lowercase_path_captured() {
    // I-377 以降、lowercase class 名も構造的に捕捉される（uppercase-head
    // ヒューリスティック廃止）。
    // match x { myenum::bar => ... } → `myenum` が refs に登録されることを確認。
    let item = fn_with_body(
        "f",
        vec![Stmt::Match {
            expr: Expr::Ident("x".to_string()),
            arms: vec![MatchArm {
                patterns: vec![Pattern::UnitStruct {
                    path: vec!["myenum".to_string(), "bar".to_string()],
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
    // match x { 1 => Wrapper { } => ... } の本体に StructInit が含まれる場合、Wrapper を拾う
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
// T8d: 構造化 Pattern walking — Stmt::IfLet / Stmt::WhileLet / Expr::Matches
// =========================================================================
//
// I-377 以降、pattern は `String` ではなく構造化 `Pattern` enum。walker は
// `path: Vec<String>` の先頭セグメントを直接取り出すため、lowercase 先頭の
// 型名も正しく捕捉される（uppercase-head ヒューリスティック廃止）。`Some` /
// `None` / `Ok` / `Err` は言語組み込みの variant として `PATTERN_LANG_BUILTINS`
// で明示除外される。

#[test]
fn test_collect_type_refs_stmt_iflet_pattern() {
    // if let Color::Red = x { ... } → Color が refs
    let item = fn_with_body(
        "f",
        vec![Stmt::IfLet {
            pattern: Pattern::UnitStruct {
                path: vec!["Color".to_string(), "Red".to_string()],
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
    // while let Color::Red(x) = it { ... } → Color が refs
    let item = fn_with_body(
        "f",
        vec![Stmt::WhileLet {
            label: None,
            pattern: Pattern::TupleStruct {
                path: vec!["Color".to_string(), "Red".to_string()],
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
    // matches!(x, Color::Red(_)) → Color が refs
    let item = fn_with_body(
        "f",
        vec![Stmt::TailExpr(Expr::Matches {
            expr: Box::new(Expr::Ident("x".to_string())),
            pattern: Box::new(Pattern::TupleStruct {
                path: vec!["Color".to_string(), "Red".to_string()],
                fields: vec![Pattern::Wildcard],
            }),
        })],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("Color"));
}

#[test]
fn test_collect_type_refs_pattern_lowercase_captured() {
    // I-377: uppercase-head ヒューリスティック廃止により、lowercase class
    // 名も構造的に捕捉される（false negative 解消）。
    let item = fn_with_body(
        "f",
        vec![Stmt::IfLet {
            pattern: Pattern::UnitStruct {
                path: vec!["myenum".to_string(), "bar".to_string()],
            },
            expr: Expr::Ident("x".to_string()),
            then_body: vec![],
            else_body: None,
        }],
    );
    let mut refs = HashSet::new();
    collect_type_refs_from_item(&item, &mut refs);
    assert!(refs.contains("myenum"));
}

#[test]
fn test_collect_type_refs_pattern_struct_form() {
    // if let Foo { x, .. } = ... → Foo が refs
    let item = fn_with_body(
        "f",
        vec![Stmt::IfLet {
            pattern: Pattern::Struct {
                path: vec!["Foo".to_string()],
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
    // if let _ = x { ... } → wildcard、何も抽出しない
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
fn test_collect_type_refs_pattern_some_none_ok_err_excluded() {
    // I-377: `Some`/`None`/`Ok`/`Err` は Option/Result の variant コンストラクタで
    // あり外部型 stub 生成の対象外（`PATTERN_LANG_BUILTINS` で除外）。
    for (pat, name) in [
        (
            Pattern::TupleStruct {
                path: vec!["Some".to_string()],
                fields: vec![Pattern::binding("x")],
            },
            "Some",
        ),
        (Pattern::none(), "None"),
        (
            Pattern::TupleStruct {
                path: vec!["Ok".to_string()],
                fields: vec![Pattern::binding("v")],
            },
            "Ok",
        ),
        (
            Pattern::TupleStruct {
                path: vec!["Err".to_string()],
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
