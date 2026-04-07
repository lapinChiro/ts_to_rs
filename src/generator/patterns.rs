//! `Pattern` IR から Rust pattern 文字列へのレンダリング。
//!
//! `Stmt::IfLet` / `Stmt::WhileLet` / `Expr::IfLet` / `Expr::Matches` および
//! `MatchArm::patterns` から呼び出される。I-377 以前は pattern を `String` として
//! IR に保持していたが、pipeline-integrity ルール（IR に display-formatted 文字列
//! 禁止）に従い構造化 `Pattern` へ移行。文字列化責務はこのモジュールに集約する。

use crate::generator::expressions::generate_expr;
use crate::ir::{Pattern, PatternCtor};

/// `PatternCtor` を Rust ソースの path 文字列にレンダリングする。
fn render_pattern_ctor(ctor: &PatternCtor) -> String {
    match ctor {
        PatternCtor::Builtin(b) => b.as_rust_str().to_string(),
        PatternCtor::UserEnumVariant { enum_ty, variant } => {
            format!("{}::{}", enum_ty.as_str(), variant)
        }
        PatternCtor::UserStruct(ty) => ty.as_str().to_string(),
    }
}

/// `Pattern` IR を Rust ソースの pattern 文字列にレンダリングする。
pub(crate) fn render_pattern(pat: &Pattern) -> String {
    match pat {
        Pattern::Wildcard => "_".to_string(),
        Pattern::Literal(expr) => generate_expr(expr),
        Pattern::Binding {
            name,
            is_mut,
            subpat,
        } => {
            let prefix = if *is_mut { "mut " } else { "" };
            match subpat {
                Some(sub) => format!("{prefix}{name} @ {}", render_pattern(sub)),
                None => format!("{prefix}{name}"),
            }
        }
        Pattern::TupleStruct { ctor, fields } => {
            let path_str = render_pattern_ctor(ctor);
            let field_strs: Vec<String> = fields.iter().map(render_pattern).collect();
            format!("{path_str}({})", field_strs.join(", "))
        }
        Pattern::Struct { ctor, fields, rest } => {
            let path_str = render_pattern_ctor(ctor);
            // Named field short-hand: if the bound pattern is just `Pattern::Binding { name: n, .. }`
            // and matches the field name, emit `n` instead of `n: n`.
            let mut parts: Vec<String> = fields
                .iter()
                .map(|(field_name, field_pat)| match field_pat {
                    Pattern::Binding {
                        name,
                        is_mut: false,
                        subpat: None,
                    } if name == field_name => field_name.clone(),
                    _ => format!("{field_name}: {}", render_pattern(field_pat)),
                })
                .collect();
            if *rest {
                parts.push("..".to_string());
            }
            if parts.is_empty() {
                format!("{path_str} {{}}")
            } else {
                format!("{path_str} {{ {} }}", parts.join(", "))
            }
        }
        Pattern::UnitStruct { ctor } => render_pattern_ctor(ctor),
        Pattern::Or(pats) => pats
            .iter()
            .map(render_pattern)
            .collect::<Vec<_>>()
            .join(" | "),
        Pattern::Range {
            start,
            end,
            inclusive,
        } => {
            let s = start.as_deref().map(generate_expr).unwrap_or_default();
            let e = end.as_deref().map(generate_expr).unwrap_or_default();
            let op = if *inclusive { "..=" } else { ".." };
            format!("{s}{op}{e}")
        }
        Pattern::Ref { mutable, inner } => {
            let prefix = if *mutable { "&mut " } else { "&" };
            format!("{prefix}{}", render_pattern(inner))
        }
        Pattern::Tuple(pats) => {
            let strs: Vec<String> = pats.iter().map(render_pattern).collect();
            format!("({})", strs.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BuiltinVariant, Expr, UserTypeRef};

    fn user_enum_ctor(enum_name: &str, variant: &str) -> PatternCtor {
        PatternCtor::UserEnumVariant {
            enum_ty: UserTypeRef::new(enum_name),
            variant: variant.to_string(),
        }
    }
    fn user_struct_ctor(name: &str) -> PatternCtor {
        PatternCtor::UserStruct(UserTypeRef::new(name))
    }

    #[test]
    fn render_wildcard() {
        assert_eq!(render_pattern(&Pattern::Wildcard), "_");
    }

    #[test]
    fn render_literal_int() {
        assert_eq!(render_pattern(&Pattern::Literal(Expr::IntLit(42))), "42");
    }

    #[test]
    fn render_binding_plain() {
        assert_eq!(render_pattern(&Pattern::binding("x")), "x");
    }

    #[test]
    fn render_binding_mut() {
        assert_eq!(
            render_pattern(&Pattern::Binding {
                name: "x".to_string(),
                is_mut: true,
                subpat: None,
            }),
            "mut x"
        );
    }

    #[test]
    fn render_binding_with_subpat() {
        assert_eq!(
            render_pattern(&Pattern::Binding {
                name: "x".to_string(),
                is_mut: false,
                subpat: Some(Box::new(Pattern::Wildcard)),
            }),
            "x @ _"
        );
    }

    #[test]
    fn render_tuple_struct_single_segment() {
        assert_eq!(render_pattern(&Pattern::some_binding("v")), "Some(v)");
    }

    #[test]
    fn render_tuple_struct_multi_segment() {
        let pat = Pattern::TupleStruct {
            ctor: user_enum_ctor("Color", "Red"),
            fields: vec![Pattern::binding("r")],
        };
        assert_eq!(render_pattern(&pat), "Color::Red(r)");
    }

    #[test]
    fn render_tuple_struct_zero_fields() {
        let pat = Pattern::TupleStruct {
            ctor: user_enum_ctor("E", "Empty"),
            fields: vec![],
        };
        assert_eq!(render_pattern(&pat), "E::Empty()");
    }

    #[test]
    fn render_struct_pattern_with_shorthand() {
        let pat = Pattern::Struct {
            ctor: user_enum_ctor("Shape", "Circle"),
            fields: vec![("radius".to_string(), Pattern::binding("radius"))],
            rest: true,
        };
        assert_eq!(render_pattern(&pat), "Shape::Circle { radius, .. }");
    }

    #[test]
    fn render_struct_pattern_rename() {
        let pat = Pattern::Struct {
            ctor: user_struct_ctor("Foo"),
            fields: vec![("x".to_string(), Pattern::binding("y"))],
            rest: false,
        };
        assert_eq!(render_pattern(&pat), "Foo { x: y }");
    }

    #[test]
    fn render_struct_empty_with_rest() {
        let pat = Pattern::Struct {
            ctor: user_struct_ctor("Foo"),
            fields: vec![],
            rest: true,
        };
        assert_eq!(render_pattern(&pat), "Foo { .. }");
    }

    #[test]
    fn render_unit_struct_single() {
        assert_eq!(render_pattern(&Pattern::none()), "None");
    }

    #[test]
    fn render_unit_struct_multi() {
        let pat = Pattern::UnitStruct {
            ctor: user_enum_ctor("Color", "Green"),
        };
        assert_eq!(render_pattern(&pat), "Color::Green");
    }

    #[test]
    fn render_pattern_ctor_builtin_variants() {
        for (b, expected) in [
            (BuiltinVariant::Some, "Some"),
            (BuiltinVariant::None, "None"),
            (BuiltinVariant::Ok, "Ok"),
            (BuiltinVariant::Err, "Err"),
        ] {
            assert_eq!(render_pattern_ctor(&PatternCtor::Builtin(b)), expected);
        }
    }

    #[test]
    fn render_pattern_ctor_user_struct_emits_bare_name() {
        assert_eq!(render_pattern_ctor(&user_struct_ctor("Foo")), "Foo");
    }

    #[test]
    fn render_or_pattern() {
        let pat = Pattern::Or(vec![
            Pattern::Literal(Expr::IntLit(1)),
            Pattern::Literal(Expr::IntLit(2)),
            Pattern::Wildcard,
        ]);
        assert_eq!(render_pattern(&pat), "1 | 2 | _");
    }

    #[test]
    fn render_range_inclusive() {
        let pat = Pattern::Range {
            start: Some(Box::new(Expr::IntLit(1))),
            end: Some(Box::new(Expr::IntLit(5))),
            inclusive: true,
        };
        assert_eq!(render_pattern(&pat), "1..=5");
    }

    #[test]
    fn render_ref_pattern() {
        let pat = Pattern::Ref {
            mutable: false,
            inner: Box::new(Pattern::binding("x")),
        };
        assert_eq!(render_pattern(&pat), "&x");
    }

    #[test]
    fn render_ref_mut_pattern() {
        let pat = Pattern::Ref {
            mutable: true,
            inner: Box::new(Pattern::binding("x")),
        };
        assert_eq!(render_pattern(&pat), "&mut x");
    }

    #[test]
    fn render_tuple_pattern() {
        let pat = Pattern::Tuple(vec![
            Pattern::binding("a"),
            Pattern::binding("b"),
            Pattern::Wildcard,
        ]);
        assert_eq!(render_pattern(&pat), "(a, b, _)");
    }

    #[test]
    fn render_nested_some_of_color_red() {
        // Some(Color::Red(x))
        let pat = Pattern::TupleStruct {
            ctor: PatternCtor::Builtin(BuiltinVariant::Some),
            fields: vec![Pattern::TupleStruct {
                ctor: user_enum_ctor("Color", "Red"),
                fields: vec![Pattern::binding("x")],
            }],
        };
        assert_eq!(render_pattern(&pat), "Some(Color::Red(x))");
    }

    // --- boundary cases ---

    #[test]
    fn render_struct_empty_no_rest() {
        // `Foo {}` — both `fields` and `rest` empty.
        let pat = Pattern::Struct {
            ctor: user_struct_ctor("Foo"),
            fields: vec![],
            rest: false,
        };
        assert_eq!(render_pattern(&pat), "Foo {}");
    }

    #[test]
    fn render_struct_no_shorthand_when_field_renames_binding() {
        // `Foo { name: alias }` — binding name differs from field name, so no shorthand.
        let pat = Pattern::Struct {
            ctor: user_struct_ctor("Foo"),
            fields: vec![("name".to_string(), Pattern::binding("alias"))],
            rest: false,
        };
        assert_eq!(render_pattern(&pat), "Foo { name: alias }");
    }

    #[test]
    fn render_struct_no_shorthand_when_binding_is_mut() {
        // `mut x` is not eligible for shorthand even if name matches.
        let pat = Pattern::Struct {
            ctor: user_struct_ctor("Foo"),
            fields: vec![(
                "x".to_string(),
                Pattern::Binding {
                    name: "x".to_string(),
                    is_mut: true,
                    subpat: None,
                },
            )],
            rest: false,
        };
        assert_eq!(render_pattern(&pat), "Foo { x: mut x }");
    }

    #[test]
    fn render_struct_no_shorthand_when_subpat_present() {
        // `x @ _` is not eligible for shorthand.
        let pat = Pattern::Struct {
            ctor: user_struct_ctor("Foo"),
            fields: vec![(
                "x".to_string(),
                Pattern::Binding {
                    name: "x".to_string(),
                    is_mut: false,
                    subpat: Some(Box::new(Pattern::Wildcard)),
                },
            )],
            rest: false,
        };
        assert_eq!(render_pattern(&pat), "Foo { x: x @ _ }");
    }

    #[test]
    fn render_range_open_start() {
        let pat = Pattern::Range {
            start: None,
            end: Some(Box::new(Expr::IntLit(10))),
            inclusive: false,
        };
        assert_eq!(render_pattern(&pat), "..10");
    }

    #[test]
    fn render_range_open_end() {
        let pat = Pattern::Range {
            start: Some(Box::new(Expr::IntLit(0))),
            end: None,
            inclusive: false,
        };
        assert_eq!(render_pattern(&pat), "0..");
    }

    #[test]
    fn render_range_exclusive() {
        let pat = Pattern::Range {
            start: Some(Box::new(Expr::IntLit(1))),
            end: Some(Box::new(Expr::IntLit(5))),
            inclusive: false,
        };
        assert_eq!(render_pattern(&pat), "1..5");
    }

    #[test]
    fn render_or_single_element() {
        // Single element Or — no separator.
        let pat = Pattern::Or(vec![Pattern::Wildcard]);
        assert_eq!(render_pattern(&pat), "_");
    }

    #[test]
    fn render_tuple_empty() {
        // `()` — unit tuple pattern.
        let pat = Pattern::Tuple(vec![]);
        assert_eq!(render_pattern(&pat), "()");
    }

    #[test]
    fn render_unit_struct_user_struct() {
        // `UserStruct` constructor: bare name.
        let pat = Pattern::UnitStruct {
            ctor: user_struct_ctor("Empty"),
        };
        assert_eq!(render_pattern(&pat), "Empty");
    }
}
