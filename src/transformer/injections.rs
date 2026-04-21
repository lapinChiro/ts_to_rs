//! Post-IR-generation helper injections.
//!
//! After the module-level transformer has produced the IR `Vec<Item>`, two
//! scanning passes inject runtime support code when their respective IR
//! nodes are present:
//!
//! - [`inject_js_typeof_if_needed`] — appends a `js_typeof` free function
//!   when any item contains [`Expr::RuntimeTypeof`]. Preserves TS `typeof`
//!   semantics for dynamically-typed values by mapping `serde_json::Value`
//!   variants to JavaScript typeof strings at runtime.
//! - [`inject_regex_import_if_needed`] — prepends a `use regex::Regex;`
//!   import when any item contains [`Expr::Regex`].
//!
//! Each injection uses a dedicated `IrVisitor` ([`RuntimeTypeofDetector`],
//! [`RegexDetector`]) that short-circuits on first detection. Visitor-based
//! detection replaces the pre-I-377 hand-rolled recursive scans which
//! silently missed `Closure` / `StructInit` / `Match` arms via `_ => false`
//! — see the I-377 commit for the latent-bug empirical trace.
//!
//! Both injections are module-scope-private: they are consumed only by
//! [`super::Transformer`]'s `transform_module` / `transform_module_collecting`
//! entry points.

use crate::ir::visit::IrVisitor;
use crate::ir::{Expr, Item, Visibility};

/// Injects a `js_typeof` helper function if any item contains
/// [`Expr::RuntimeTypeof`].
///
/// The helper maps `serde_json::Value` variants to JavaScript typeof strings
/// at runtime, preserving TypeScript's `typeof` semantics for dynamically-
/// typed values.
pub(super) fn inject_js_typeof_if_needed(items: &mut Vec<Item>) {
    if !items_contain_runtime_typeof(items) {
        return;
    }
    items.push(Item::RawCode(
        r#"fn js_typeof(val: &serde_json::Value) -> &'static str {
    match val {
        serde_json::Value::String(_) => "string",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Null => "undefined",
        _ => "object",
    }
}"#
        .to_string(),
    ));
}

/// Injects `use regex::Regex;` at the top of the item list if any item
/// contains [`Expr::Regex`].
pub(super) fn inject_regex_import_if_needed(items: &mut Vec<Item>) {
    if items_contain_regex(items) {
        items.insert(
            0,
            Item::Use {
                vis: Visibility::Private,
                path: "regex".to_string(),
                names: vec!["Regex".to_string()],
            },
        );
    }
}

/// [`Expr::RuntimeTypeof`] が任意の項目内に存在するかを構造的に検出する visitor。
///
/// I-377 以前は `expr_contains_runtime_typeof` / `stmts_contain_runtime_typeof` /
/// `items_contain_runtime_typeof` の 3 つの手書き再帰関数で実装されており、
/// `Expr::Closure` / `Expr::StructInit` / `Expr::Match` などの variant を
/// `_ => false` で黙殺していた（latent バグ: closure 内の RuntimeTypeof が
/// 未検出になり `js_typeof` helper が注入されない）。`IrVisitor` 化により全
/// variant が `walk_*` で走査されるため、この抜けが構造的に解消される。
#[derive(Default)]
struct RuntimeTypeofDetector {
    found: bool,
}

impl IrVisitor for RuntimeTypeofDetector {
    fn visit_expr(&mut self, expr: &Expr) {
        if self.found {
            return;
        }
        if matches!(expr, Expr::RuntimeTypeof { .. }) {
            self.found = true;
            return;
        }
        crate::ir::visit::walk_expr(self, expr);
    }
}

fn items_contain_runtime_typeof(items: &[Item]) -> bool {
    let mut detector = RuntimeTypeofDetector::default();
    for item in items {
        detector.visit_item(item);
        if detector.found {
            return true;
        }
    }
    false
}

/// [`Expr::Regex`] が任意の項目内に存在するかを構造的に検出する visitor。
///
/// [`RuntimeTypeofDetector`] と同じ理由で `IrVisitor` 化されている：以前の手書き
/// `expr_contains_regex` / `stmts_contain_regex` は `Expr::Closure` や
/// `Expr::IfLet` 内の Regex を検出できなかった（`_ => false` で黙殺）。
#[derive(Default)]
struct RegexDetector {
    found: bool,
}

impl IrVisitor for RegexDetector {
    fn visit_expr(&mut self, expr: &Expr) {
        if self.found {
            return;
        }
        if matches!(expr, Expr::Regex { .. }) {
            self.found = true;
            return;
        }
        crate::ir::visit::walk_expr(self, expr);
    }
}

fn items_contain_regex(items: &[Item]) -> bool {
    let mut detector = RegexDetector::default();
    for item in items {
        detector.visit_item(item);
        if detector.found {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BinOp, CallTarget, ClosureBody, Param, RustType, Stmt};

    fn fn_item(body: Vec<Stmt>) -> Item {
        Item::Fn {
            vis: Visibility::Private,
            attributes: vec![],
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body,
        }
    }

    /// `RuntimeTypeof` nested inside a closure body must be detected.
    ///
    /// 以前の手書き `expr_contains_runtime_typeof` は `Expr::Closure` arm を
    /// `_ => false` で黙殺しており、この入力では false を返していた（latent bug）。
    /// `RuntimeTypeofDetector: IrVisitor` 化により `walk_expr` 経由で
    /// Closure 内部が走査され、正しく true を返す。
    #[test]
    fn runtime_typeof_detected_inside_closure_body() {
        let closure_expr = Expr::Closure {
            params: vec![Param {
                name: "x".to_string(),
                ty: Some(RustType::Any),
            }],
            return_type: None,
            body: ClosureBody::Expr(Box::new(Expr::RuntimeTypeof {
                operand: Box::new(Expr::Ident("x".to_string())),
            })),
        };
        let item = fn_item(vec![Stmt::TailExpr(closure_expr)]);
        assert!(
            items_contain_runtime_typeof(&[item]),
            "RuntimeTypeof inside a closure body must be detected"
        );
    }

    /// `RuntimeTypeof` nested inside a `Match` arm body must be detected.
    #[test]
    fn runtime_typeof_detected_inside_match_arm() {
        let item = fn_item(vec![Stmt::Match {
            expr: Expr::Ident("x".to_string()),
            arms: vec![crate::ir::MatchArm {
                patterns: vec![crate::ir::Pattern::Wildcard],
                guard: None,
                body: vec![Stmt::TailExpr(Expr::RuntimeTypeof {
                    operand: Box::new(Expr::Ident("x".to_string())),
                })],
            }],
        }]);
        assert!(
            items_contain_runtime_typeof(&[item]),
            "RuntimeTypeof inside a match arm must be detected"
        );
    }

    /// Items without `RuntimeTypeof` must return false.
    #[test]
    fn runtime_typeof_absent_returns_false() {
        let item = fn_item(vec![Stmt::TailExpr(Expr::BinaryOp {
            left: Box::new(Expr::Ident("x".to_string())),
            op: BinOp::Add,
            right: Box::new(Expr::IntLit(1)),
        })]);
        assert!(!items_contain_runtime_typeof(&[item]));
    }

    /// `Regex` nested inside a closure body must be detected.
    ///
    /// 以前の手書き `expr_contains_regex` は Closure arm を `_ => false` で
    /// 黙殺していた（latent bug）。
    #[test]
    fn regex_detected_inside_closure_body() {
        let closure_expr = Expr::Closure {
            params: vec![],
            return_type: None,
            body: ClosureBody::Expr(Box::new(Expr::Regex {
                pattern: "abc".to_string(),
                global: false,
                sticky: false,
            })),
        };
        let item = fn_item(vec![Stmt::TailExpr(closure_expr)]);
        assert!(
            items_contain_regex(&[item]),
            "Regex inside a closure body must be detected"
        );
    }

    /// Items without `Regex` must return false.
    #[test]
    fn regex_absent_returns_false() {
        let item = fn_item(vec![Stmt::Expr(Expr::FnCall {
            target: CallTarget::Free("f".to_string()),
            args: vec![],
        })]);
        assert!(!items_contain_regex(&[item]));
    }

    /// FnCall args are walked.
    #[test]
    fn runtime_typeof_detected_inside_fncall_args() {
        let item = fn_item(vec![Stmt::Expr(Expr::FnCall {
            target: CallTarget::Free("wrap".to_string()),
            args: vec![Expr::RuntimeTypeof {
                operand: Box::new(Expr::Ident("x".to_string())),
            }],
        })]);
        assert!(items_contain_runtime_typeof(&[item]));
    }
}
