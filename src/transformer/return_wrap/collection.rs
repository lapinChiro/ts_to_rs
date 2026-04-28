//! Return leaf type collection: SWC AST walking + type resolution.
//!
//! Walks SWC arrow / function bodies in depth-first order, locating return statements
//! and tail expressions, and resolving their types via the canonical
//! [`FileTypeResolution::resolve_expr_type`](crate::pipeline::type_resolution::FileTypeResolution::resolve_expr_type)
//! primitive. The resulting `Vec<ReturnLeafType>` is consumed positionally by the
//! IR-side `wrap_body_returns` walker, so the order of leaves must match the IR walk.

use swc_common::Spanned;
use swc_ecma_ast as ast;

use crate::ir::RustType;
use crate::pipeline::type_resolution::FileTypeResolution;

/// Pre-collected type and span for a return leaf expression.
///
/// Collected from SWC AST before IR conversion. Consumed positionally
/// by `wrap_body_returns` / `wrap_expr_tail` during IR post-processing.
#[derive(Debug, Clone)]
pub(crate) struct ReturnLeafType {
    /// Resolved type from TypeResolver (`None` if unknown).
    pub ty: Option<RustType>,
    /// Source byte span `(lo, hi)` for error reporting.
    pub span: (u32, u32),
}

/// Collects return leaf expression types from a SWC arrow body.
///
/// Walks the arrow body in depth-first order, finding all return/tail leaf
/// expressions and resolving their types from `FileTypeResolution::expr_types`.
/// Ternary branches (`CondExpr`) are recursively expanded to leaf level.
///
/// The resulting `Vec` is consumed positionally by `wrap_body_returns`.
/// The positional invariant (SWC and IR return leaves in same depth-first order)
/// holds because the Transformer preserves statement structure and return ordering.
pub(crate) fn collect_return_leaf_types(
    arrow: &ast::ArrowExpr,
    type_resolution: &FileTypeResolution,
) -> Vec<ReturnLeafType> {
    let mut out = Vec::new();
    match &*arrow.body {
        ast::BlockStmtOrExpr::Expr(expr) => {
            collect_expr_leaf_types(expr, type_resolution, &mut out);
        }
        ast::BlockStmtOrExpr::BlockStmt(block) => {
            collect_stmts_return_leaf_types(&block.stmts, type_resolution, &mut out);
        }
    }
    out
}

/// Collects return leaf types from a sequence of SWC statements.
pub(crate) fn collect_stmts_return_leaf_types(
    stmts: &[ast::Stmt],
    type_resolution: &FileTypeResolution,
    out: &mut Vec<ReturnLeafType>,
) {
    for stmt in stmts {
        collect_stmt_return_leaf_types(stmt, type_resolution, out);
    }
}

/// Collects return leaf types from a single SWC statement.
///
/// Recurses into all block-containing statement structures (if/else, for, while,
/// try/catch, switch, labeled blocks) to find all nested return statements.
/// Must mirror the IR-side walk in `wrap_body_returns` to maintain the
/// positional invariant.
fn collect_stmt_return_leaf_types(
    stmt: &ast::Stmt,
    type_resolution: &FileTypeResolution,
    out: &mut Vec<ReturnLeafType>,
) {
    match stmt {
        ast::Stmt::Return(ret) => {
            if let Some(arg) = &ret.arg {
                collect_expr_leaf_types(arg, type_resolution, out);
            }
        }
        ast::Stmt::If(if_stmt) => {
            collect_stmt_return_leaf_types(&if_stmt.cons, type_resolution, out);
            if let Some(alt) = &if_stmt.alt {
                collect_stmt_return_leaf_types(alt, type_resolution, out);
            }
        }
        ast::Stmt::Block(block) => {
            collect_stmts_return_leaf_types(&block.stmts, type_resolution, out);
        }
        ast::Stmt::Switch(switch) => {
            for case in &switch.cases {
                collect_stmts_return_leaf_types(&case.cons, type_resolution, out);
            }
        }
        ast::Stmt::For(for_stmt) => {
            collect_stmt_return_leaf_types(&for_stmt.body, type_resolution, out);
        }
        ast::Stmt::ForIn(for_in) => {
            collect_stmt_return_leaf_types(&for_in.body, type_resolution, out);
        }
        ast::Stmt::ForOf(for_of) => {
            collect_stmt_return_leaf_types(&for_of.body, type_resolution, out);
        }
        ast::Stmt::While(while_stmt) => {
            collect_stmt_return_leaf_types(&while_stmt.body, type_resolution, out);
        }
        ast::Stmt::DoWhile(do_while) => {
            collect_stmt_return_leaf_types(&do_while.body, type_resolution, out);
        }
        ast::Stmt::Try(try_stmt) => {
            collect_stmts_return_leaf_types(&try_stmt.block.stmts, type_resolution, out);
            if let Some(catch) = &try_stmt.handler {
                collect_stmts_return_leaf_types(&catch.body.stmts, type_resolution, out);
            }
            // finally は collect しない。IR 側では finally body が
            // scopeguard::guard クロージャ内に封入されるため、
            // wrap_body_returns が walk せず位置不一致になる。
            // finally 内の return は JS でも非推奨パターン。
        }
        ast::Stmt::Labeled(labeled) => {
            collect_stmt_return_leaf_types(&labeled.body, type_resolution, out);
        }
        _ => {}
    }
}

/// Collects leaf types from a SWC expression in return position.
///
/// Recursively collects from ternary branches and parenthesized expressions.
/// For leaf expressions, resolves the type via the canonical
/// [`FileTypeResolution::resolve_expr_type`] primitive (I-177-B), so the
/// `narrowed_type` 優先 → `expr_type` fallback precedence shared with
/// [`Transformer::get_expr_type`](crate::transformer::Transformer::get_expr_type)
/// is preserved (single source of truth, canonical primitive 経由による DRY 保証)。
///
/// Note: SeqExpr (comma operator) は IR にサポートされておらず、
/// Transformer で変換エラーになるため collect しない。
fn collect_expr_leaf_types(
    expr: &ast::Expr,
    type_resolution: &FileTypeResolution,
    out: &mut Vec<ReturnLeafType>,
) {
    match expr {
        // Ternary: recurse into both branches
        ast::Expr::Cond(cond) => {
            collect_expr_leaf_types(&cond.cons, type_resolution, out);
            collect_expr_leaf_types(&cond.alt, type_resolution, out);
        }
        // Parenthesized: unwrap
        ast::Expr::Paren(paren) => {
            collect_expr_leaf_types(&paren.expr, type_resolution, out);
        }
        // Leaf expression: resolve type via canonical primitive
        leaf => {
            let swc_span = leaf.span();
            let ty = type_resolution.resolve_expr_type(leaf).cloned();
            out.push(ReturnLeafType {
                ty,
                span: (swc_span.lo.0, swc_span.hi.0),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::{parse_files, SyntheticTypeRegistry};
    use crate::registry::build_registry;

    /// Helper: parse source, build registry with callable interface, resolve types,
    /// extract the arrow from the const declaration, and collect return leaf types.
    fn collect_leaves_for_callable(interface_def: &str, const_decl: &str) -> Vec<ReturnLeafType> {
        let (leaves, _events) = collect_leaves_with_events_for_callable(interface_def, const_decl);
        leaves
    }

    /// Helper: same as `collect_leaves_for_callable`, but also returns the narrow
    /// events for debugging / cross-axis verification.
    fn collect_leaves_with_events_for_callable(
        interface_def: &str,
        const_decl: &str,
    ) -> (
        Vec<ReturnLeafType>,
        Vec<crate::pipeline::narrowing_analyzer::NarrowEvent>,
    ) {
        let source = format!("{interface_def}\n{const_decl}");
        let files = parse_files(vec![(std::path::PathBuf::from("test.ts"), source)]).unwrap();
        let file = &files.files[0];
        let reg = build_registry(&file.module);
        let mut synthetic = SyntheticTypeRegistry::new();

        let mut resolver = crate::pipeline::type_resolver::TypeResolver::new(&reg, &mut synthetic);
        let resolution = resolver.resolve_file(file);

        // Extract the arrow from the second module item (first is interface)
        let var_decl = match &file.module.body[1] {
            swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Var(
                vd,
            ))) => vd,
            _ => panic!("expected var decl"),
        };
        let arrow = match var_decl.decls[0].init.as_deref() {
            Some(ast::Expr::Arrow(a)) => a,
            _ => panic!("expected arrow expr"),
        };

        let leaves = collect_return_leaf_types(arrow, &resolution);
        (leaves, resolution.narrow_events.clone())
    }

    #[test]
    fn collect_leaves_expression_body_single_ident() {
        let leaves = collect_leaves_for_callable(
            "interface F { (c: string): string; }",
            "const f: F = (c: string): string => c;",
        );
        assert_eq!(leaves.len(), 1, "single expression body → 1 leaf");
        assert_eq!(leaves[0].ty, Some(RustType::String));
    }

    #[test]
    fn collect_leaves_block_body_multiple_returns() {
        let leaves = collect_leaves_for_callable(
            "interface G { (c: string, key: string): number; }",
            r#"const g: G = (c: string, key: string): number => {
                if (key) { return 42; }
                return 0;
            };"#,
        );
        assert_eq!(leaves.len(), 2, "two return statements → 2 leaves");
        assert_eq!(leaves[0].ty, Some(RustType::F64));
        assert_eq!(leaves[1].ty, Some(RustType::F64));
    }

    #[test]
    fn collect_leaves_ternary_expression_body() {
        let leaves = collect_leaves_for_callable(
            "interface H { (c: string): string; }",
            r#"const h: H = (c: string): string => c ? c : "fallback";"#,
        );
        assert_eq!(leaves.len(), 2, "ternary → 2 leaves (then + else)");
        assert_eq!(leaves[0].ty, Some(RustType::String));
        assert_eq!(leaves[1].ty, Some(RustType::String));
    }

    #[test]
    fn collect_leaves_for_loop_nested_return() {
        let leaves = collect_leaves_for_callable(
            "interface I { (c: string): string; }",
            r#"const i: I = (c: string): string => {
                for (let x = 0; x < 10; x++) {
                    if (x > 5) { return c; }
                }
                return "default";
            };"#,
        );
        assert_eq!(
            leaves.len(),
            2,
            "for with nested return + final return → 2 leaves"
        );
        // c is a param (String), "default" is a string literal (String)
        assert_eq!(leaves[0].ty, Some(RustType::String));
        assert_eq!(leaves[1].ty, Some(RustType::String));
    }

    #[test]
    fn collect_leaves_try_catch_returns() {
        let leaves = collect_leaves_for_callable(
            "interface J { (c: string): string; }",
            r#"const j: J = (c: string): string => {
                try {
                    return c;
                } catch (e) {
                    return "error";
                }
            };"#,
        );
        assert_eq!(leaves.len(), 2, "try + catch each with return → 2 leaves");
        assert_eq!(leaves[0].ty, Some(RustType::String));
        assert_eq!(leaves[1].ty, Some(RustType::String));
    }

    /// Helper: parse source containing a top-level `function h(...) {...}` declaration
    /// and return narrow events + leaves for its body.
    fn collect_leaves_with_events_for_fn_decl(
        src: &str,
    ) -> (
        Vec<ReturnLeafType>,
        Vec<crate::pipeline::narrowing_analyzer::NarrowEvent>,
    ) {
        let files =
            parse_files(vec![(std::path::PathBuf::from("test.ts"), src.to_string())]).unwrap();
        let file = &files.files[0];
        let reg = build_registry(&file.module);
        let mut synthetic = SyntheticTypeRegistry::new();
        let mut resolver = crate::pipeline::type_resolver::TypeResolver::new(&reg, &mut synthetic);
        let resolution = resolver.resolve_file(file);

        // Walk fn decl body via collect_stmts_return_leaf_types
        let fn_decl = match &file.module.body[0] {
            swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Fn(d))) => {
                d
            }
            _ => panic!("expected fn decl"),
        };
        let body = fn_decl.function.body.as_ref().expect("fn body");
        let mut leaves = Vec::new();
        collect_stmts_return_leaf_types(&body.stmts, &resolution, &mut leaves);
        (leaves, resolution.narrow_events.clone())
    }

    #[test]
    fn test_collect_leaves_typeof_narrow_post_if_return_fn_decl() {
        // Empirical lock-in for declaration form (visit_fn_decl path which already
        // sets current_block_end correctly — pre-existing GREEN baseline).
        let (leaves, _events) = collect_leaves_with_events_for_fn_decl(
            r#"function h(x: string | number): string | number {
    if (typeof x === "string") return 0;
    else { console.log("ne"); }
    return x;
}
console.log(h(42));
console.log(h("a"));"#,
        );
        assert_eq!(leaves.len(), 2);
        assert_eq!(leaves[0].ty, Some(RustType::F64), "leaf 0 = `0` literal");
        assert_eq!(
            leaves[1].ty,
            Some(RustType::F64),
            "leaf 1 = post-if `x` narrowed to F64"
        );
    }

    /// Helper: parse source containing a top-level `const h = function (...) {...}`
    /// fn-expression assignment and return narrow events + leaves for its body.
    fn collect_leaves_with_events_for_fn_expr(
        src: &str,
    ) -> (
        Vec<ReturnLeafType>,
        Vec<crate::pipeline::narrowing_analyzer::NarrowEvent>,
    ) {
        let files =
            parse_files(vec![(std::path::PathBuf::from("test.ts"), src.to_string())]).unwrap();
        let file = &files.files[0];
        let reg = build_registry(&file.module);
        let mut synthetic = SyntheticTypeRegistry::new();
        let mut resolver = crate::pipeline::type_resolver::TypeResolver::new(&reg, &mut synthetic);
        let resolution = resolver.resolve_file(file);

        // Extract the FnExpr from `const h = function (...) {...};`
        let var_decl = match &file.module.body[0] {
            swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Var(
                vd,
            ))) => vd,
            _ => panic!("expected var decl"),
        };
        let fn_expr = match var_decl.decls[0].init.as_deref() {
            Some(ast::Expr::Fn(fe)) => fe,
            _ => panic!("expected fn expr init"),
        };
        let body = fn_expr.function.body.as_ref().expect("fn body");
        let mut leaves = Vec::new();
        collect_stmts_return_leaf_types(&body.stmts, &resolution, &mut leaves);
        (leaves, resolution.narrow_events.clone())
    }

    #[test]
    fn test_typeof_narrow_post_if_pushes_early_return_complement_in_class_method() {
        // I-177-F (extended): class method body は `visit_method_function` 経由で walk
        // される。pre-fix では `for stmt in &body.stmts` で直接 iterate して
        // `current_block_end` を set しないため、method body 内の if-stmt with
        // typeof guard + then-exit + else-non-exit が EarlyReturnComplement narrow
        // event を post-if scope に push しない (silent type widening risk)。
        // post-fix: `visit_block_stmt(body)` 経由で current_block_end を set し、
        // detect_early_return_narrowing が正しく fire する。
        use crate::pipeline::narrowing_analyzer::NarrowEvent;
        let src = r#"class Processor {
    process(x: string | number): number {
        if (typeof x === "string") return 0;
        else { console.log("ne"); }
        return x.valueOf();
    }
}"#;
        let files =
            parse_files(vec![(std::path::PathBuf::from("test.ts"), src.to_string())]).unwrap();
        let file = &files.files[0];
        let reg = build_registry(&file.module);
        let mut synthetic = SyntheticTypeRegistry::new();
        let mut resolver = crate::pipeline::type_resolver::TypeResolver::new(&reg, &mut synthetic);
        let resolution = resolver.resolve_file(file);

        // 期待: 3 narrow events (Primary then-branch + Primary else-branch +
        // EarlyReturnComplement post-if)
        let early_return_complement_count = resolution
            .narrow_events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    NarrowEvent::Narrow {
                        var_name,
                        trigger: crate::pipeline::narrowing_analyzer::NarrowTrigger::EarlyReturnComplement(_),
                        ..
                    } if var_name == "x"
                )
            })
            .count();
        assert_eq!(
            early_return_complement_count, 1,
            "class method body should push EarlyReturnComplement narrow event \
             at post-if scope (visit_block_stmt 経由で current_block_end が set される)"
        );
    }

    #[test]
    fn test_typeof_narrow_post_if_pushes_early_return_complement_in_class_constructor() {
        // I-177-F (extended): constructor body symmetric — visit_class_decl 内の
        // ast::ClassMember::Constructor arm で body を walk するが、pre-fix では
        // `visit_block_stmt` を skip して直接 iterate。post-fix で symmetric に修正。
        use crate::pipeline::narrowing_analyzer::NarrowEvent;
        let src = r#"class Container {
    field: number;
    constructor(x: string | number) {
        if (typeof x === "string") {
            this.field = 0;
            return;
        }
        else { console.log("ne"); }
        this.field = x.valueOf();
    }
}"#;
        let files =
            parse_files(vec![(std::path::PathBuf::from("test.ts"), src.to_string())]).unwrap();
        let file = &files.files[0];
        let reg = build_registry(&file.module);
        let mut synthetic = SyntheticTypeRegistry::new();
        let mut resolver = crate::pipeline::type_resolver::TypeResolver::new(&reg, &mut synthetic);
        let resolution = resolver.resolve_file(file);

        let early_return_complement_count = resolution
            .narrow_events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    NarrowEvent::Narrow {
                        var_name,
                        trigger: crate::pipeline::narrowing_analyzer::NarrowTrigger::EarlyReturnComplement(_),
                        ..
                    } if var_name == "x"
                )
            })
            .count();
        assert_eq!(
            early_return_complement_count, 1,
            "constructor body should push EarlyReturnComplement narrow event \
             at post-if scope"
        );
    }

    #[test]
    fn test_collect_leaves_typeof_narrow_post_if_return_fn_expr() {
        // I-177-F symmetric: function expression form (resolve_fn_expr path).
        // Same defect class as arrow form — pre-fix `current_block_end` not set,
        // post-fix visit_block_stmt 経由で正しく set される.
        let (leaves, _events) = collect_leaves_with_events_for_fn_expr(
            r#"const h = function (x: string | number): string | number {
    if (typeof x === "string") return 0;
    else { console.log("ne"); }
    return x;
};"#,
        );
        assert_eq!(leaves.len(), 2);
        assert_eq!(leaves[0].ty, Some(RustType::F64));
        assert_eq!(
            leaves[1].ty,
            Some(RustType::F64),
            "leaf 1 = post-if `x` narrowed to F64 in fn-expression body"
        );
    }

    #[test]
    fn test_collect_leaves_typeof_narrow_post_if_return_arrow_form() {
        // I-177-B Matrix cell #9 + I-177-F (callable-interface arrow form):
        // typeof narrow の then-branch が exit して post-if で `return x` が実行される
        // 場合、`x` は narrowed type (`F64`) を持つ。
        //
        // Architectural dependency chain (2026-04-26):
        //  1. I-177-E: synthetic fork inherits types — enables compute_complement_type
        //     to find variants for builtin-pre-registered union types.
        //  2. I-177-B: collect_expr_leaf_types canonical helper — correct query order
        //     for Ident leaf types.
        //  3. I-177-F: `resolve_arrow_expr` body walks via `visit_block_stmt` so that
        //     `current_block_end` is set, allowing `detect_early_return_narrowing` to
        //     push EarlyReturnComplement narrow events in arrow body post-if scope.
        let leaves = collect_leaves_for_callable(
            "interface H { (x: string | number): string | number; }",
            r#"const h: H = (x: string | number): string | number => {
                if (typeof x === "string") return 0;
                else { console.log("ne"); }
                return x;
            };"#,
        );
        // 2 leaves: `0` (return 0, F64 from NumLit) + `x` (return x, F64 from narrow).
        assert_eq!(leaves.len(), 2, "if/else + post-if return → 2 leaves");
        assert_eq!(
            leaves[0].ty,
            Some(RustType::F64),
            "leaf 0 = `0` literal in then branch, NumLit type = F64"
        );
        assert_eq!(
            leaves[1].ty,
            Some(RustType::F64),
            "leaf 1 = `x` Ident in post-if return, narrowed to F64 \
             (else branch was string-excluded by typeof narrow)"
        );
    }
}
