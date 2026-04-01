//! Discriminated union switch field binding detection.
//!
//! Detects `switch (obj.tag)` patterns where `obj` is a discriminated union type,
//! and records field bindings (`DuFieldBinding`) for each case body's field accesses.

use swc_common::Spanned;
use swc_ecma_ast as ast;

use super::*;
use crate::pipeline::type_resolution::DuFieldBinding;

impl<'a> TypeResolver<'a> {
    /// Detects discriminated union switch patterns and records field bindings.
    ///
    /// When `switch (s.kind)` where `s` has type `Shape` (a DU enum with
    /// `tag_field = "kind"`), each case body that accesses `s.radius` etc.
    /// gets those fields recorded as `DuFieldBinding` entries.
    pub(super) fn detect_du_switch_bindings(&mut self, switch_stmt: &ast::SwitchStmt) {
        // Check if discriminant is obj.field (member expression)
        let member = match switch_stmt.discriminant.as_ref() {
            ast::Expr::Member(m) => m,
            _ => return,
        };
        let field_name = match &member.prop {
            ast::MemberProp::Ident(ident) => ident.sym.to_string(),
            _ => return,
        };
        // Resolve the object variable name
        let obj_var_name = match member.obj.as_ref() {
            ast::Expr::Ident(ident) => ident.sym.to_string(),
            _ => return,
        };
        // Resolve the object's type
        let obj_type = self.lookup_var(&obj_var_name);
        let enum_name = match &obj_type {
            ResolvedType::Known(RustType::Named { name, .. }) => name.clone(),
            _ => return,
        };
        // Check if this is a DU enum with matching tag field
        let variant_fields = match self.registry.get(&enum_name) {
            Some(TypeDef::Enum {
                tag_field: Some(tag),
                variant_fields,
                string_values,
                ..
            }) if *tag == field_name => {
                // We need both string_values (to map case test → variant) and variant_fields
                (string_values.clone(), variant_fields.clone())
            }
            _ => return,
        };
        let (string_values, variant_fields) = variant_fields;

        // For each case, detect field accesses and record bindings
        let mut pending_variant_names: Vec<String> = Vec::new();
        for case in &switch_stmt.cases {
            // Map case test to variant name
            if let Some(test) = &case.test {
                let str_value = match test.as_ref() {
                    ast::Expr::Lit(ast::Lit::Str(s)) => s.value.to_string_lossy().into_owned(),
                    _ => continue,
                };
                if let Some(variant_name) = string_values.get(&str_value) {
                    pending_variant_names.push(variant_name.clone());
                }
            }

            // Empty body = fall-through, accumulate variants
            if case.cons.is_empty() {
                continue;
            }

            // Calculate scope range from case body statements
            let scope_range = case_body_span_range(&case.cons);
            let (scope_start, scope_end) = match scope_range {
                Some(range) => range,
                None => {
                    pending_variant_names.clear();
                    continue;
                }
            };

            // Collect field accesses on the DU variable
            let needed_fields =
                collect_du_field_accesses_from_stmts(&case.cons, &obj_var_name, &field_name);

            // Record bindings for fields that exist in the pending variants
            for field in &needed_fields {
                let field_exists_in_variant = pending_variant_names.iter().any(|vname| {
                    variant_fields
                        .get(vname)
                        .is_some_and(|fields| fields.iter().any(|f| f.name == *field))
                });
                if field_exists_in_variant {
                    self.result.du_field_bindings.push(DuFieldBinding {
                        var_name: field.clone(),
                        scope_start,
                        scope_end,
                    });
                }
            }

            pending_variant_names.clear();
        }
    }
}

/// Calculates the byte range of a switch case body (first stmt start to last stmt end).
pub(super) fn case_body_span_range(stmts: &[ast::Stmt]) -> Option<(u32, u32)> {
    let first = stmts.first()?;
    let last = stmts.last()?;
    Some((first.span().lo.0, last.span().hi.0))
}

/// Collects field names accessed on `obj_var` in the given statements (e.g., `s.radius` → "radius").
///
/// Excludes the tag field itself. Used by DU switch detection to determine which
/// fields need to be bound in match arm patterns.
pub(super) fn collect_du_field_accesses_from_stmts(
    stmts: &[ast::Stmt],
    obj_var: &str,
    tag_field: &str,
) -> Vec<String> {
    let mut fields = Vec::new();
    for stmt in stmts {
        collect_du_field_accesses_from_stmt_inner(stmt, obj_var, tag_field, &mut fields);
    }
    fields.sort();
    fields.dedup();
    fields
}

fn collect_du_field_accesses_from_stmt_inner(
    stmt: &ast::Stmt,
    obj_var: &str,
    tag_field: &str,
    fields: &mut Vec<String>,
) {
    match stmt {
        ast::Stmt::Expr(expr_stmt) => {
            collect_du_field_accesses_from_expr_inner(&expr_stmt.expr, obj_var, tag_field, fields);
        }
        ast::Stmt::Return(ret) => {
            if let Some(arg) = &ret.arg {
                collect_du_field_accesses_from_expr_inner(arg, obj_var, tag_field, fields);
            }
        }
        ast::Stmt::Decl(ast::Decl::Var(var_decl)) => {
            for decl in &var_decl.decls {
                if let Some(init) = &decl.init {
                    collect_du_field_accesses_from_expr_inner(init, obj_var, tag_field, fields);
                }
            }
        }
        ast::Stmt::If(if_stmt) => {
            collect_du_field_accesses_from_expr_inner(&if_stmt.test, obj_var, tag_field, fields);
            collect_du_field_accesses_from_stmt_inner(&if_stmt.cons, obj_var, tag_field, fields);
            if let Some(alt) = &if_stmt.alt {
                collect_du_field_accesses_from_stmt_inner(alt, obj_var, tag_field, fields);
            }
        }
        ast::Stmt::Block(block) => {
            for s in &block.stmts {
                collect_du_field_accesses_from_stmt_inner(s, obj_var, tag_field, fields);
            }
        }
        _ => {}
    }
}

fn collect_du_field_accesses_from_expr_inner(
    expr: &ast::Expr,
    obj_var: &str,
    tag_field: &str,
    fields: &mut Vec<String>,
) {
    match expr {
        ast::Expr::Member(member) => {
            if let ast::Expr::Ident(ident) = member.obj.as_ref() {
                if ident.sym.as_ref() == obj_var {
                    if let ast::MemberProp::Ident(prop) = &member.prop {
                        let field_name = prop.sym.to_string();
                        if field_name != tag_field {
                            fields.push(field_name);
                        }
                    }
                }
            }
            collect_du_field_accesses_from_expr_inner(&member.obj, obj_var, tag_field, fields);
        }
        ast::Expr::Call(call) => {
            if let ast::Callee::Expr(callee) = &call.callee {
                collect_du_field_accesses_from_expr_inner(callee, obj_var, tag_field, fields);
            }
            for arg in &call.args {
                collect_du_field_accesses_from_expr_inner(&arg.expr, obj_var, tag_field, fields);
            }
        }
        ast::Expr::Bin(bin) => {
            collect_du_field_accesses_from_expr_inner(&bin.left, obj_var, tag_field, fields);
            collect_du_field_accesses_from_expr_inner(&bin.right, obj_var, tag_field, fields);
        }
        ast::Expr::Tpl(tpl) => {
            for expr in &tpl.exprs {
                collect_du_field_accesses_from_expr_inner(expr, obj_var, tag_field, fields);
            }
        }
        ast::Expr::Paren(paren) => {
            collect_du_field_accesses_from_expr_inner(&paren.expr, obj_var, tag_field, fields);
        }
        ast::Expr::Assign(assign) => {
            collect_du_field_accesses_from_expr_inner(&assign.right, obj_var, tag_field, fields);
        }
        ast::Expr::Cond(cond) => {
            collect_du_field_accesses_from_expr_inner(&cond.test, obj_var, tag_field, fields);
            collect_du_field_accesses_from_expr_inner(&cond.cons, obj_var, tag_field, fields);
            collect_du_field_accesses_from_expr_inner(&cond.alt, obj_var, tag_field, fields);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_typescript;

    /// Extracts statements from the body of the first function in a TS source.
    fn parse_function_body_stmts(source: &str) -> Vec<ast::Stmt> {
        let module = parse_typescript(source).expect("parse should succeed");
        for item in &module.body {
            if let ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Fn(fn_decl))) = item {
                if let Some(body) = &fn_decl.function.body {
                    return body.stmts.clone();
                }
            }
        }
        panic!("no function found in source");
    }

    /// Extracts statements from raw TS expression statements (wrapped in a function for parsing).
    fn parse_stmts(body_source: &str) -> Vec<ast::Stmt> {
        let source = format!("function __wrapper__() {{ {body_source} }}");
        parse_function_body_stmts(&source)
    }

    #[test]
    fn test_collect_du_field_accesses_member_access_collects_field() {
        let stmts = parse_stmts("s.radius;");
        let fields = collect_du_field_accesses_from_stmts(&stmts, "s", "kind");
        assert_eq!(fields, vec!["radius".to_string()]);
    }

    #[test]
    fn test_collect_du_field_accesses_tag_field_excluded() {
        // CRITICAL: tag field must be excluded to prevent silent semantic changes
        let stmts = parse_stmts("s.kind;");
        let fields = collect_du_field_accesses_from_stmts(&stmts, "s", "kind");
        assert!(
            fields.is_empty(),
            "tag field 'kind' should be excluded, got: {fields:?}"
        );
    }

    #[test]
    fn test_collect_du_field_accesses_deduplicates() {
        let stmts = parse_stmts("s.radius; s.radius; s.radius;");
        let fields = collect_du_field_accesses_from_stmts(&stmts, "s", "kind");
        assert_eq!(
            fields,
            vec!["radius".to_string()],
            "duplicate accesses should be deduplicated"
        );
    }

    #[test]
    fn test_collect_du_field_accesses_nested_in_call_args() {
        let stmts = parse_stmts("console.log(s.radius);");
        let fields = collect_du_field_accesses_from_stmts(&stmts, "s", "kind");
        assert_eq!(
            fields,
            vec!["radius".to_string()],
            "field access in call args should be collected"
        );
    }

    #[test]
    fn test_collect_du_field_accesses_in_template_literal() {
        let stmts = parse_stmts("`${s.name}`;");
        let fields = collect_du_field_accesses_from_stmts(&stmts, "s", "kind");
        assert_eq!(
            fields,
            vec!["name".to_string()],
            "field access in template literal should be collected"
        );
    }

    #[test]
    fn test_collect_du_field_accesses_in_conditional_expr() {
        let stmts = parse_stmts("true ? s.a : s.b;");
        let fields = collect_du_field_accesses_from_stmts(&stmts, "s", "kind");
        assert_eq!(
            fields,
            vec!["a".to_string(), "b".to_string()],
            "both branches of conditional should be collected"
        );
    }
}
