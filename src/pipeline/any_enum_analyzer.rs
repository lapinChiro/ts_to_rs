//! Pipeline-level any-enum type analysis.
//!
//! Walks the AST to detect `any`-typed variables narrowed via `typeof`/`instanceof`,
//! generates synthetic enum types, and records overrides in [`FileTypeResolution`].
//! Runs after Type Collection and before Type Resolution in the pipeline.

use swc_common::Spanned;
use swc_ecma_ast as ast;

use super::any_narrowing;
use crate::ir::RustType;
use crate::pipeline::type_resolution::{AnyEnumOverride, FileTypeResolution};
use crate::pipeline::SyntheticTypeRegistry;

/// Analyzes a file's AST for any-typed variable narrowing and records overrides.
///
/// For each function/method/arrow in the file:
/// 1. Collects `any`-typed parameters and local variables
/// 2. Scans the body for `typeof`/`instanceof` constraints
/// 3. Registers synthetic enum types in `SyntheticTypeRegistry`
/// 4. Records position-scoped overrides in `FileTypeResolution`
pub fn analyze_any_enums(
    module: &ast::Module,
    resolution: &mut FileTypeResolution,
    synthetic: &mut SyntheticTypeRegistry,
) {
    for item in &module.body {
        match item {
            ast::ModuleItem::Stmt(stmt) => visit_stmt(stmt, resolution, synthetic),
            ast::ModuleItem::ModuleDecl(ast::ModuleDecl::ExportDecl(export)) => {
                visit_decl(&export.decl, resolution, synthetic);
            }
            ast::ModuleItem::ModuleDecl(ast::ModuleDecl::ExportDefaultExpr(default_expr)) => {
                visit_expr(&default_expr.expr, "", resolution, synthetic);
            }
            _ => {}
        }
    }
}

fn visit_stmt(
    stmt: &ast::Stmt,
    resolution: &mut FileTypeResolution,
    synthetic: &mut SyntheticTypeRegistry,
) {
    match stmt {
        ast::Stmt::Decl(decl) => visit_decl(decl, resolution, synthetic),
        ast::Stmt::Expr(expr_stmt) => visit_expr(&expr_stmt.expr, "", resolution, synthetic),
        _ => {}
    }
}

fn visit_decl(
    decl: &ast::Decl,
    resolution: &mut FileTypeResolution,
    synthetic: &mut SyntheticTypeRegistry,
) {
    match decl {
        ast::Decl::Fn(fn_decl) => {
            let name = fn_decl.ident.sym.to_string();
            analyze_fn_decl(&name, &fn_decl.function, resolution, synthetic);
        }
        ast::Decl::Var(var_decl) => {
            for decl in &var_decl.decls {
                if let Some(init) = &decl.init {
                    let name = match &decl.name {
                        ast::Pat::Ident(ident) => ident.id.sym.to_string(),
                        _ => String::new(),
                    };
                    visit_expr(init, &name, resolution, synthetic);
                }
            }
        }
        ast::Decl::Class(class_decl) => {
            visit_class(&class_decl.class, resolution, synthetic);
        }
        _ => {}
    }
}

fn visit_expr(
    expr: &ast::Expr,
    enclosing_name: &str,
    resolution: &mut FileTypeResolution,
    synthetic: &mut SyntheticTypeRegistry,
) {
    match expr {
        ast::Expr::Arrow(arrow) => {
            analyze_arrow(enclosing_name, arrow, resolution, synthetic);
        }
        ast::Expr::Fn(fn_expr) => {
            let name = fn_expr
                .ident
                .as_ref()
                .map(|id| id.sym.to_string())
                .unwrap_or_else(|| enclosing_name.to_string());
            analyze_fn_decl(&name, &fn_expr.function, resolution, synthetic);
        }
        _ => {}
    }
}

fn visit_class(
    class: &ast::Class,
    resolution: &mut FileTypeResolution,
    synthetic: &mut SyntheticTypeRegistry,
) {
    for member in &class.body {
        match member {
            ast::ClassMember::Method(method) => {
                let name = match &method.key {
                    ast::PropName::Ident(ident) => ident.sym.to_string(),
                    _ => String::new(),
                };
                analyze_fn_decl(&name, &method.function, resolution, synthetic);
            }
            ast::ClassMember::Constructor(ctor) => {
                if let Some(body) = &ctor.body {
                    let params: Vec<ast::Param> = ctor
                        .params
                        .iter()
                        .filter_map(|p| match p {
                            ast::ParamOrTsParamProp::Param(param) => Some(param.clone()),
                            _ => None,
                        })
                        .collect();
                    let scope_start = ctor.span.lo.0;
                    analyze_fn_body(
                        "constructor",
                        &params,
                        body,
                        scope_start,
                        resolution,
                        synthetic,
                    );
                }
            }
            _ => {}
        }
    }
}

/// Analyzes a function declaration for any-enum overrides.
fn analyze_fn_decl(
    name: &str,
    function: &ast::Function,
    resolution: &mut FileTypeResolution,
    synthetic: &mut SyntheticTypeRegistry,
) {
    if let Some(body) = &function.body {
        // Use function.span (includes params) so overrides cover param declarations
        let fn_span_start = function.span.lo.0;
        analyze_fn_body(
            name,
            &function.params,
            body,
            fn_span_start,
            resolution,
            synthetic,
        );
    }
}

/// Analyzes a function body for any-typed params and local variables.
///
/// `scope_start` should be the function's span start (not the body's), so that
/// overrides cover parameter declarations which appear before the body.
fn analyze_fn_body(
    fn_name: &str,
    params: &[ast::Param],
    body: &ast::BlockStmt,
    scope_start: u32,
    resolution: &mut FileTypeResolution,
    synthetic: &mut SyntheticTypeRegistry,
) {
    let scope_end = body.span.hi.0;

    // 1. Any-typed parameters: collect constraints from body
    let any_param_names: Vec<String> = params
        .iter()
        .filter_map(|p| {
            if let ast::Pat::Ident(ident) = &p.pat {
                let is_any_or_unknown = ident.type_ann.as_ref().is_some_and(|ann| {
                    matches!(
                        ann.type_ann.as_ref(),
                        ast::TsType::TsKeywordType(kw)
                            if matches!(kw.kind,
                                ast::TsKeywordTypeKind::TsAnyKeyword
                                | ast::TsKeywordTypeKind::TsUnknownKeyword
                            )
                    )
                });
                if is_any_or_unknown {
                    Some(ident.id.sym.to_string())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    if !any_param_names.is_empty() {
        let constraints = any_narrowing::collect_any_constraints(body, &any_param_names);
        register_overrides(
            fn_name,
            &constraints,
            scope_start,
            scope_end,
            resolution,
            synthetic,
        );
    }

    // 2. Any-typed local variables: collect constraints from body
    let local_any_names = any_narrowing::collect_any_local_var_names(body);
    if !local_any_names.is_empty() {
        let constraints = any_narrowing::collect_any_constraints(body, &local_any_names);
        register_overrides(
            fn_name,
            &constraints,
            scope_start,
            scope_end,
            resolution,
            synthetic,
        );
    }

    // 3. Recurse into nested functions/arrows in the body
    for stmt in &body.stmts {
        visit_nested_stmt(stmt, resolution, synthetic);
    }
}

/// Analyzes an arrow function for any-typed params.
fn analyze_arrow(
    name: &str,
    arrow: &ast::ArrowExpr,
    resolution: &mut FileTypeResolution,
    synthetic: &mut SyntheticTypeRegistry,
) {
    let any_param_names: Vec<String> = arrow
        .params
        .iter()
        .filter_map(|p| {
            if let ast::Pat::Ident(ident) = p {
                let has_any_type = ident.type_ann.as_ref().is_none_or(|ann| {
                    matches!(
                        ann.type_ann.as_ref(),
                        ast::TsType::TsKeywordType(kw)
                            if matches!(kw.kind,
                                ast::TsKeywordTypeKind::TsAnyKeyword
                                | ast::TsKeywordTypeKind::TsUnknownKeyword
                            )
                    )
                });
                if has_any_type {
                    Some(ident.id.sym.to_string())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    // Use arrow.span (includes params) so overrides cover param declarations
    let scope_start = arrow.span.lo.0;
    let scope_end = match &*arrow.body {
        ast::BlockStmtOrExpr::BlockStmt(block) => block.span.hi.0,
        ast::BlockStmtOrExpr::Expr(expr) => expr.span().hi.0,
    };

    if !any_param_names.is_empty() {
        let constraints = match &*arrow.body {
            ast::BlockStmtOrExpr::BlockStmt(body) => {
                any_narrowing::collect_any_constraints(body, &any_param_names)
            }
            ast::BlockStmtOrExpr::Expr(expr) => {
                any_narrowing::collect_any_constraints_from_expr(expr, &any_param_names)
            }
        };
        register_overrides(
            name,
            &constraints,
            scope_start,
            scope_end,
            resolution,
            synthetic,
        );
    }

    // Recurse into nested functions/arrows
    match &*arrow.body {
        ast::BlockStmtOrExpr::BlockStmt(block) => {
            for stmt in &block.stmts {
                visit_nested_stmt(stmt, resolution, synthetic);
            }
        }
        ast::BlockStmtOrExpr::Expr(expr) => {
            visit_expr(expr, "", resolution, synthetic);
        }
    }
}

/// Registers overrides from constraint analysis results.
fn register_overrides(
    fn_name: &str,
    constraints: &std::collections::HashMap<String, any_narrowing::AnyTypeConstraints>,
    scope_start: u32,
    scope_end: u32,
    resolution: &mut FileTypeResolution,
    synthetic: &mut SyntheticTypeRegistry,
) {
    for (var_name, constraint) in constraints {
        if constraint.is_empty() {
            continue;
        }
        let variants = any_narrowing::build_any_enum_variants(constraint);
        let enum_name = synthetic.register_any_enum(fn_name, var_name, variants);
        let enum_type = RustType::Named {
            name: enum_name,
            type_args: vec![],
        };
        resolution.any_enum_overrides.push(AnyEnumOverride {
            var_name: var_name.clone(),
            scope_start,
            scope_end,
            enum_type,
        });
    }
}

/// Visits nested statements to find functions/arrows (but not the top-level body).
fn visit_nested_stmt(
    stmt: &ast::Stmt,
    resolution: &mut FileTypeResolution,
    synthetic: &mut SyntheticTypeRegistry,
) {
    match stmt {
        ast::Stmt::Decl(decl) => visit_decl(decl, resolution, synthetic),
        ast::Stmt::Expr(expr_stmt) => visit_expr(&expr_stmt.expr, "", resolution, synthetic),
        ast::Stmt::If(if_stmt) => {
            visit_nested_stmt(&if_stmt.cons, resolution, synthetic);
            if let Some(alt) = &if_stmt.alt {
                visit_nested_stmt(alt, resolution, synthetic);
            }
        }
        ast::Stmt::Block(block) => {
            for s in &block.stmts {
                visit_nested_stmt(s, resolution, synthetic);
            }
        }
        ast::Stmt::For(for_stmt) => {
            if let Some(body) = for_stmt.body.as_block() {
                for s in &body.stmts {
                    visit_nested_stmt(s, resolution, synthetic);
                }
            }
        }
        ast::Stmt::ForOf(for_of) => {
            if let Some(body) = for_of.body.as_block() {
                for s in &body.stmts {
                    visit_nested_stmt(s, resolution, synthetic);
                }
            }
        }
        ast::Stmt::While(while_stmt) => {
            if let Some(body) = while_stmt.body.as_block() {
                for s in &body.stmts {
                    visit_nested_stmt(s, resolution, synthetic);
                }
            }
        }
        ast::Stmt::Return(ret) => {
            if let Some(arg) = &ret.arg {
                visit_expr(arg, "", resolution, synthetic);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::{parse_files, SyntheticTypeRegistry};
    use std::path::PathBuf;

    fn analyze(source: &str) -> (FileTypeResolution, SyntheticTypeRegistry) {
        let files = parse_files(vec![(PathBuf::from("test.ts"), source.to_string())]).unwrap();
        let file = &files.files[0];
        let mut resolution = FileTypeResolution::empty();
        let mut synthetic = SyntheticTypeRegistry::new();
        analyze_any_enums(&file.module, &mut resolution, &mut synthetic);
        (resolution, synthetic)
    }

    #[test]
    fn test_fn_param_any_override_detected() {
        let source = r#"
function process(data: any): string {
    if (typeof data === "string") {
        return data;
    }
    if (typeof data === "number") {
        return data.toString();
    }
    return "";
}
"#;
        let (resolution, synthetic) = analyze(source);

        assert!(
            !resolution.any_enum_overrides.is_empty(),
            "should detect any enum override for 'data' param"
        );
        let override_entry = &resolution.any_enum_overrides[0];
        assert_eq!(override_entry.var_name, "data");
        assert!(
            matches!(&override_entry.enum_type, RustType::Named { name, .. } if name == "ProcessDataType"),
            "enum type should be 'ProcessDataType', got: {:?}",
            override_entry.enum_type
        );

        // Should be registered in synthetic registry
        assert!(
            !synthetic.all_items().is_empty(),
            "should register enum in synthetic registry"
        );
    }

    #[test]
    fn test_fn_local_any_var_override_detected() {
        let source = r#"
function process(): string {
    const x: any = getInput();
    if (typeof x === "string") {
        return x;
    }
    return "";
}
"#;
        let (resolution, _) = analyze(source);

        let x_override = resolution
            .any_enum_overrides
            .iter()
            .find(|o| o.var_name == "x");
        assert!(
            x_override.is_some(),
            "should detect any enum override for local var 'x'"
        );
    }

    #[test]
    fn test_arrow_param_any_override_detected() {
        let source = r#"
const process = (data: any): string => {
    if (typeof data === "string") {
        return data;
    }
    return "";
};
"#;
        let (resolution, _) = analyze(source);

        assert!(
            !resolution.any_enum_overrides.is_empty(),
            "should detect any enum override for arrow param 'data'"
        );
        assert_eq!(resolution.any_enum_overrides[0].var_name, "data");
    }

    #[test]
    fn test_any_enum_override_scoped_to_function_body() {
        let source = r#"
function foo(x: any): string {
    if (typeof x === "string") { return x; }
    return "";
}
function bar(y: number): number { return y; }
"#;
        let (resolution, _) = analyze(source);

        let x_override = resolution
            .any_enum_overrides
            .iter()
            .find(|o| o.var_name == "x")
            .expect("should have override for x");

        // Override should be scoped to foo's body, not bar's
        assert!(x_override.scope_start > 0);
        assert!(x_override.scope_end > x_override.scope_start);

        // Should NOT find override for y (not any-typed)
        assert!(
            resolution
                .any_enum_overrides
                .iter()
                .find(|o| o.var_name == "y")
                .is_none(),
            "should not have override for non-any param"
        );
    }

    #[test]
    fn test_any_enum_override_lookup() {
        let source = r#"
function process(data: any): string {
    if (typeof data === "string") {
        return data;
    }
    return "";
}
"#;
        let (resolution, _) = analyze(source);
        let o = &resolution.any_enum_overrides[0];

        // Inside scope
        assert!(
            resolution
                .any_enum_override("data", o.scope_start + 1)
                .is_some(),
            "should find override inside scope"
        );

        // Outside scope
        assert!(
            resolution.any_enum_override("data", 0).is_none(),
            "should not find override outside scope"
        );

        // Wrong name
        assert!(
            resolution
                .any_enum_override("other", o.scope_start + 1)
                .is_none(),
            "should not find override for wrong name"
        );
    }

    #[test]
    fn test_arrow_expr_body_any_override_detected() {
        let source = r#"
const check = (data: any): boolean => typeof data === "string";
"#;
        let (resolution, synthetic) = analyze(source);

        assert!(
            !resolution.any_enum_overrides.is_empty(),
            "should detect any enum override in expression-body arrow"
        );
        assert_eq!(resolution.any_enum_overrides[0].var_name, "data");
        assert!(
            !synthetic.all_items().is_empty(),
            "should register enum in synthetic registry"
        );
    }

    #[test]
    fn test_class_method_any_param_detected() {
        let source = r#"
class Processor {
    handle(data: any): string {
        if (typeof data === "string") {
            return data;
        }
        return "";
    }
}
"#;
        let (resolution, _) = analyze(source);

        let data_override = resolution
            .any_enum_overrides
            .iter()
            .find(|o| o.var_name == "data");
        assert!(
            data_override.is_some(),
            "should detect any enum override for class method param"
        );
    }

    #[test]
    fn test_class_constructor_any_param_detected() {
        let source = r#"
class Processor {
    value: string;
    constructor(data: any) {
        if (typeof data === "string") {
            this.value = data;
        } else {
            this.value = "";
        }
    }
}
"#;
        let (resolution, _) = analyze(source);

        let data_override = resolution
            .any_enum_overrides
            .iter()
            .find(|o| o.var_name == "data");
        assert!(
            data_override.is_some(),
            "should detect any enum override for constructor param"
        );
    }

    #[test]
    fn test_no_any_params_produces_empty_result() {
        let source = r#"
function process(data: string): string {
    return data;
}
"#;
        let (resolution, synthetic) = analyze(source);

        assert!(
            resolution.any_enum_overrides.is_empty(),
            "no any params should produce no overrides"
        );
        assert!(
            synthetic.all_items().is_empty(),
            "no any params should produce no synthetic items"
        );
    }

    #[test]
    fn test_nested_arrow_in_function_detected() {
        let source = r#"
function outer() {
    const inner = (x: any): string => {
        if (typeof x === "number") {
            return x.toString();
        }
        return "";
    };
}
"#;
        let (resolution, _) = analyze(source);

        let x_override = resolution
            .any_enum_overrides
            .iter()
            .find(|o| o.var_name == "x");
        assert!(
            x_override.is_some(),
            "should detect any enum override for nested arrow function param"
        );
    }
}
