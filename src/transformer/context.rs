//! Immutable context for the Transformer.
//!
//! `TransformContext` bundles all read-only data that the Transformer needs,
//! replacing the individual `&TypeRegistry`, `current_file_dir`, etc. parameters.
//! `SyntheticTypeRegistry` is excluded because it requires `&mut` access;
//! it will be integrated in P8 when the unified pipeline makes it immutable.

use std::path::Path;

use crate::pipeline::module_graph::ModuleGraph;
use crate::pipeline::type_resolution::FileTypeResolution;
use crate::registry::TypeRegistry;

/// Immutable context for the Transformer.
///
/// Contains all read-only references the Transformer needs for conversion.
/// `SyntheticTypeRegistry` is passed separately as `&mut` (merged in P8).
pub struct TransformContext<'a> {
    /// Module dependency graph for import resolution.
    pub module_graph: &'a ModuleGraph,
    /// Type definitions collected from the source files.
    pub type_registry: &'a TypeRegistry,
    /// Pre-computed type resolution results from `TypeResolver`.
    pub type_resolution: &'a FileTypeResolution,
    /// Path of the current file being transformed.
    pub file_path: &'a Path,
}

impl<'a> TransformContext<'a> {
    /// Creates a new `TransformContext`.
    pub fn new(
        module_graph: &'a ModuleGraph,
        type_registry: &'a TypeRegistry,
        type_resolution: &'a FileTypeResolution,
        file_path: &'a Path,
    ) -> Self {
        Self {
            module_graph,
            type_registry,
            type_resolution,
            file_path,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::ir::{Expr, Item, MatchArm, MatchPattern, Stmt, Visibility};
    use crate::parser::parse_typescript;
    use crate::pipeline::type_resolution::FileTypeResolution;
    use crate::pipeline::ModuleGraph;
    use crate::pipeline::SyntheticTypeRegistry;
    use crate::registry::TypeRegistry;
    use crate::transformer::test_fixtures::TctxFixture;

    // ===== TransformContext 基本動作 =====

    #[test]
    fn test_transform_context_new_holds_all_fields() {
        let reg = TypeRegistry::new();
        let resolution = FileTypeResolution::empty();
        let mg = ModuleGraph::empty();
        let path = Path::new("src/foo.ts");
        let ctx = TransformContext::new(&mg, &reg, &resolution, path);
        assert_eq!(ctx.file_path, Path::new("src/foo.ts"));
    }

    #[test]
    fn test_transform_with_context_basic_fn_produces_same_ir() {
        let f = TctxFixture::new();
        let (items, output) = f.transform(r#"function hello(): string { return "world"; }"#);
        assert!(!items.is_empty(), "should produce at least one item");
        assert!(
            output.contains("fn hello"),
            "should contain function: {output}"
        );
    }

    #[test]
    fn test_transform_with_context_empty_resolution_backward_compat() {
        let source = r#"export function add(a: number, b: number): number { return a + b; }"#;
        let module = parse_typescript(source).unwrap();
        let reg = TypeRegistry::new();

        // Old API
        let mg = ModuleGraph::empty();
        let resolution = FileTypeResolution::empty();
        let old_tctx = TransformContext::new(&mg, &reg, &resolution, Path::new("test.ts"));
        let mut synthetic_old = SyntheticTypeRegistry::new();
        let old_items = crate::transformer::transform_module_with_path(
            &module,
            &old_tctx,
            None,
            &mut synthetic_old,
        )
        .unwrap();

        // New API with empty resolution
        let f = TctxFixture::new();
        let mut synthetic_new = SyntheticTypeRegistry::new();
        let new_items = crate::transformer::transform_module_with_context(
            &module,
            &f.tctx(),
            &mut synthetic_new,
        )
        .unwrap();

        let old_output = crate::generator::generate(&old_items);
        let new_output = crate::generator::generate(&new_items);
        assert_eq!(old_output, new_output);
    }

    // ===== expr_types lookup =====

    #[test]
    fn test_expr_type_lookup_known_type_from_resolution() {
        let source = "function greet(name: string): string { return name; }";
        let f = TctxFixture::from_source(source);
        assert!(
            !f.tctx().type_resolution.expr_types.is_empty(),
            "TypeResolver should resolve expression types"
        );

        let (_, output) = f.transform(source);
        assert!(output.contains("fn greet"), "should generate function");
    }

    #[test]
    fn test_expr_type_unknown_fallback_to_heuristics() {
        // TypeResolver sets expected type for return → should produce .to_string()
        let source = r#"function greet(): string { return "hello"; }"#;
        let f = TctxFixture::from_source(source);
        let (_, output) = f.transform(source);
        assert!(
            output.contains("to_string()"),
            "TypeResolver should set expected type causing .to_string(): {output}"
        );
    }

    // ===== expected_types lookup =====

    #[test]
    fn test_expected_type_lookup_from_resolution() {
        let source = r#"function greet(): string { return "hello"; }"#;
        let f = TctxFixture::from_source(source);
        assert!(
            !f.tctx().type_resolution.expected_types.is_empty(),
            "TypeResolver should set expected types for return statements"
        );

        let (_, output) = f.transform(source);
        assert!(
            output.contains("to_string()"),
            "expected type String should cause .to_string(): {output}"
        );
    }

    // ===== narrowing lookup =====

    #[test]
    fn test_narrowing_from_resolution_events_overrides_type_env() {
        let source = r#"
function check(x: string | number): string {
    if (typeof x === "string") {
        return x;
    }
    return "";
}"#;
        let f = TctxFixture::from_source(source);
        assert!(
            !f.tctx().type_resolution.narrowing_events.is_empty(),
            "TypeResolver should detect typeof narrowing"
        );

        let (_, output) = f.transform(source);
        assert!(
            output.contains("fn check"),
            "should generate the function: {output}"
        );
    }

    // ===== Generator semantic judgment removal =====

    #[test]
    fn test_regex_use_in_ir_from_transformer() {
        // Transformer should emit Item::Use for regex::Regex
        let f = TctxFixture::new();
        let (items, _) =
            f.transform(r#"function test_re(): boolean { return /hello/.test("world"); }"#);

        let has_regex_use = items.iter().any(|item| {
            matches!(item, Item::Use { path, names, .. }
                if path == "regex" && names.contains(&"Regex".to_string()))
        });
        assert!(
            has_regex_use,
            "Transformer should emit Item::Use for regex::Regex. Items: {items:?}"
        );
    }

    #[test]
    fn test_generator_no_regex_scan_transparent() {
        // Generator should NOT scan for Regex::new() and add use statements.
        let items = vec![Item::Fn {
            vis: Visibility::Private,
            attributes: vec![],
            name: "make_regex".to_string(),
            is_async: false,
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::Expr(Expr::FnCall {
                name: "Regex::new".to_string(),
                args: vec![Expr::StringLit("hello".to_string())],
            })],
        }];

        let output = crate::generator::generate(&items);
        assert!(
            !output.contains("use regex::Regex;"),
            "Generator should not inject regex import. Output:\n{output}"
        );
    }

    #[test]
    fn test_match_as_str_in_ir_from_transformer() {
        // Transformer should emit .as_str() on match discriminant for string patterns
        let source = r#"
function describe(s: string): string {
    switch (s) {
        case "a": return "alpha";
        case "b": return "beta";
        default: return "unknown";
    }
}"#;
        let f = TctxFixture::new();
        let (items, _) = f.transform(source);

        let fn_item = items
            .iter()
            .find(|i| matches!(i, Item::Fn { name, .. } if name == "describe"));
        assert!(fn_item.is_some(), "should have describe function");

        if let Some(Item::Fn { body, .. }) = fn_item {
            let match_stmt = body.iter().find(|s| matches!(s, Stmt::Match { .. }));
            assert!(match_stmt.is_some(), "should have a match statement");

            if let Some(Stmt::Match { expr, .. }) = match_stmt {
                let has_as_str = matches!(
                    expr,
                    Expr::MethodCall { method, .. } if method == "as_str"
                );
                assert!(
                    has_as_str,
                    "Transformer should apply .as_str() to match discriminant. Got: {expr:?}"
                );
            }
        }
    }

    #[test]
    fn test_generator_no_match_as_str_injection() {
        // Generator should NOT add .as_str() to match discriminants.
        let items = vec![Item::Fn {
            vis: Visibility::Private,
            attributes: vec![],
            name: "test_fn".to_string(),
            is_async: false,
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::Match {
                expr: Expr::Ident("s".to_string()),
                arms: vec![
                    MatchArm {
                        patterns: vec![MatchPattern::Literal(Expr::StringLit("a".to_string()))],
                        guard: None,
                        body: vec![Stmt::Return(Some(Expr::StringLit("alpha".to_string())))],
                    },
                    MatchArm {
                        patterns: vec![MatchPattern::Wildcard],
                        guard: None,
                        body: vec![Stmt::Return(Some(Expr::StringLit("other".to_string())))],
                    },
                ],
            }],
        }];

        let output = crate::generator::generate(&items);
        assert!(
            !output.contains(".as_str()"),
            "Generator should not inject .as_str(). Output:\n{output}"
        );
    }
}
