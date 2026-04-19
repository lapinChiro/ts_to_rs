//! Statement conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC statement nodes into the IR [`Stmt`] representation.

mod control_flow;
mod destructuring;
mod error_handling;
mod helpers;
mod loops;
pub(crate) mod mutability;
pub(crate) mod nullish_assign;
mod spread;
mod switch;

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{RustType, Stmt};
use crate::pipeline::type_converter::convert_type_for_position;
use crate::transformer::Transformer;
use crate::transformer::{extract_pat_ident_name, TypePosition, UnsupportedSyntaxError};
use mutability::mark_mutated_vars;
use nullish_assign::fuse_nullish_assign_shadow_lets;

/// I-154: The `__ts_` prefix namespace is reserved for ts_to_rs internal label
/// emission (`__ts_switch`, `__ts_try_block`, `__ts_do_while`, `__ts_do_while_loop`).
/// User labels starting with `__ts_` are rejected at all 3 label-introducing /
/// label-referencing sites (`Stmt::Labeled` declaration, labeled `Stmt::Break`,
/// labeled `Stmt::Continue`) to prevent silent collision with internal labels.
///
/// SWC parser accepts `break undefined_label;` (tsx catches it with "Undefined label"
/// syntax error, but SWC does not). Without lint on labeled break/continue, user
/// code writing `break __ts_switch;` (even unintentionally) would silently target
/// our internal labeled block.
///
/// Returns `Err(UnsupportedSyntaxError)` if `label_name` starts with `__ts_`.
pub(crate) fn check_ts_internal_label_namespace(label: &ast::Ident) -> Result<()> {
    if label.sym.as_ref().starts_with("__ts_") {
        return Err(UnsupportedSyntaxError::new(
            "label names starting with `__ts_` are reserved for ts_to_rs internal emission",
            label.span,
        )
        .into());
    }
    Ok(())
}

/// Converts an SWC [`ast::Stmt`] into an IR [`Stmt`].
///
/// The `return_type` parameter is the enclosing function's return type, propagated to
/// return statements so that expected-type-based coercions (e.g., `StringLit` → `.to_string()`)
/// are applied automatically.
///
/// # Supported conversions
///
/// - Variable declarations (`const` → `let`, `let` → `let mut`)
/// - Return statements
/// - If/else statements
/// - Expression statements
///
/// # Errors
///
/// Returns an error for unsupported statement types.
impl<'a> Transformer<'a> {
    /// Converts an SWC [`ast::Stmt`] into IR [`Stmt`]s.
    pub(crate) fn convert_stmt(
        &mut self,
        stmt: &ast::Stmt,
        return_type: Option<&RustType>,
    ) -> Result<Vec<Stmt>> {
        match stmt {
            ast::Stmt::Return(ret) => {
                if let Some(stmts) = self.try_expand_spread_return(ret)? {
                    return Ok(stmts);
                }
                // I-050: pass return_type as expected_override only when it is
                // Any (serde_json::Value), to trigger concrete→Value coercion.
                // For other return types, rely on TypeResolver's expected_type.
                let any_override = return_type.filter(|t| matches!(t, RustType::Any));
                let expr = ret
                    .arg
                    .as_ref()
                    .map(|e| self.convert_expr_with_expected(e, any_override))
                    .transpose()?;
                Ok(vec![Stmt::Return(expr)])
            }
            ast::Stmt::Decl(ast::Decl::Var(var_decl)) => {
                if let Some(stmts) = self.try_expand_spread_var_decl(var_decl)? {
                    return Ok(stmts);
                }
                if let Some(expanded) = self.try_convert_object_destructuring(var_decl)? {
                    Ok(expanded)
                } else if let Some(expanded) = self.try_convert_array_destructuring(var_decl)? {
                    Ok(expanded)
                } else {
                    self.convert_var_decl(var_decl)
                }
            }
            ast::Stmt::If(if_stmt) => self.convert_if_stmt(if_stmt, return_type),
            ast::Stmt::Expr(expr_stmt) => {
                if let Some(stmts) = self.try_expand_spread_expr_stmt(expr_stmt)? {
                    return Ok(stmts);
                }
                // I-142: intercept `x ??= d;` (Ident LHS) to preserve TS
                // narrowing via shadow-let. Other `??=` shapes fall through
                // to `convert_expr`, which handles expression-context paths
                // or reports unsupported.
                if let Some(stmts) = self.try_convert_nullish_assign_stmt(&expr_stmt.expr)? {
                    return Ok(stmts);
                }
                let expr = self.convert_expr(&expr_stmt.expr)?;
                Ok(vec![Stmt::Expr(expr)])
            }
            ast::Stmt::Throw(throw_stmt) => Ok(vec![self.convert_throw_stmt(throw_stmt)?]),
            ast::Stmt::While(while_stmt) => self.convert_while_stmt(while_stmt, return_type),
            ast::Stmt::ForOf(for_of) => Ok(vec![self.convert_for_of_stmt(for_of, return_type)?]),
            ast::Stmt::For(for_stmt) => match self.convert_for_stmt(for_stmt, return_type) {
                Ok(s) => Ok(vec![s]),
                Err(_) => self.convert_for_stmt_as_loop(for_stmt, return_type),
            },
            ast::Stmt::Break(break_stmt) => {
                // I-154: reject `break __ts_X;` references (see `check_ts_internal_label_namespace`).
                if let Some(label) = &break_stmt.label {
                    check_ts_internal_label_namespace(label)?;
                }
                let label = break_stmt.label.as_ref().map(|l| l.sym.to_string());
                Ok(vec![Stmt::Break { label, value: None }])
            }
            ast::Stmt::Continue(cont_stmt) => {
                // I-154: reject `continue __ts_X;` references.
                if let Some(label) = &cont_stmt.label {
                    check_ts_internal_label_namespace(label)?;
                }
                let label = cont_stmt.label.as_ref().map(|l| l.sym.to_string());
                Ok(vec![Stmt::Continue { label }])
            }
            ast::Stmt::Labeled(labeled_stmt) => {
                // I-154: reject `__ts_X: <body>` declarations
                // (also checked inside convert_labeled_stmt, but early-fail here).
                check_ts_internal_label_namespace(&labeled_stmt.label)?;
                Ok(vec![self.convert_labeled_stmt(labeled_stmt, return_type)?])
            }
            ast::Stmt::DoWhile(do_while) => Ok(vec![self.convert_do_while_stmt(
                do_while,
                return_type,
                None,
            )?]),
            ast::Stmt::Try(try_stmt) => self.convert_try_stmt(try_stmt, return_type),
            ast::Stmt::Decl(ast::Decl::Fn(fn_decl)) => {
                Ok(vec![self.convert_nested_fn_decl(fn_decl)?])
            }
            ast::Stmt::Switch(switch_stmt) => self.convert_switch_stmt(switch_stmt, return_type),
            ast::Stmt::ForIn(for_in) => Ok(vec![self.convert_for_in_stmt(for_in, return_type)?]),
            // I-153 T0: Bare `{ ... }` block stmt (e.g., `case 1: { const x = 1; return x; }`).
            // TS allows block stmts at any stmt position. Flatten into the parent's
            // stmt sequence; enclosing Rust scope (match arm / fn body / if body) already
            // provides a `{ }` Rust block. Valid TS with block-scoped `const`/`let` keeps
            // equivalent semantics because parent scope contains the vars only until the
            // enclosing Rust block ends. Block-scope violations (`{ const x = 1; } x;`)
            // are ill-formed TS (tsc errors) and out of scope per project purpose.
            ast::Stmt::Block(block) => self.convert_stmt_list(&block.stmts, return_type),
            ast::Stmt::Decl(ast::Decl::TsInterface(_) | ast::Decl::TsTypeAlias(_)) => Ok(vec![]),
            ast::Stmt::Empty(_) => Ok(vec![]),
            _ => Err(anyhow!("unsupported statement: {:?}", stmt)),
        }
    }

    /// Converts a variable declaration to IR `Stmt::Let` statements.
    fn convert_var_decl(&mut self, var_decl: &ast::VarDecl) -> Result<Vec<Stmt>> {
        let mut stmts = Vec::new();
        for declarator in &var_decl.decls {
            let name = extract_pat_ident_name(&declarator.name)?;

            let ty = match &declarator.name {
                ast::Pat::Ident(ident) => {
                    let converted = if let Some(ann) = ident.type_ann.as_ref() {
                        Some(convert_type_for_position(
                            &ann.type_ann,
                            TypePosition::Value,
                            self.synthetic,
                            self.reg(),
                        )?)
                    } else {
                        None
                    };
                    match converted {
                        Some(RustType::Any) => {
                            // Check FileTypeResolution for any-enum override
                            // (computed by pipeline's any_enum_analyzer)
                            let pos = ident.id.span.lo.0;
                            self.tctx
                                .type_resolution
                                .any_enum_override(&name, pos)
                                .cloned()
                                .or(converted)
                        }
                        other => other,
                    }
                }
                _ => None,
            };

            // Start all variables as immutable.
            // `mark_mutated_vars` will upgrade to `let mut` when actual
            // mutations (reassignment, field assignment, mutating method call) are detected.
            let mutable = false;

            // I-050: when the declared type is Any (serde_json::Value), pass it
            // as expected_override to trigger concrete→Value coercion.
            let any_init_override = ty.as_ref().filter(|t| matches!(t, RustType::Any));
            let init = declarator
                .init
                .as_ref()
                .map(|e| self.convert_expr_with_expected(e, any_init_override))
                .transpose()?;

            stmts.push(Stmt::Let {
                mutable,
                name,
                ty,
                init,
            });
        }
        Ok(stmts)
    }

    /// Converts a list of SWC statements into IR statements.
    pub(crate) fn convert_stmt_list(
        &mut self,
        stmts: &[ast::Stmt],
        return_type: Option<&RustType>,
    ) -> Result<Vec<Stmt>> {
        let mut result = Vec::new();
        for (i, stmt) in stmts.iter().enumerate() {
            // I-142 D-1: surface `<ident> ??= d; ... <ident> = v;`
            // narrowing-reset patterns as UnsupportedSyntaxError *before*
            // emitting the shadow-let that would produce a silent compile
            // error later. `remaining` covers the same Rust lexical scope the
            // shadow-let would span; nested blocks are scanned recursively
            // (closures excluded) by the scanner. See D-1 in
            // `backlog/I-142-nullish-assign-shadow-let.md`.
            self.pre_check_narrowing_reset(stmt, &stmts[i + 1..])?;
            let converted = self.convert_stmt(stmt, return_type)?;
            result.extend(converted);
        }
        // I-142: collapse `let x = init; let x = x.unwrap_or[_else](...);`
        // pairs emitted by `try_convert_nullish_assign_stmt` into a single
        // fused `let x = init.unwrap_or[_else](...);`. Must run before
        // `mark_mutated_vars` so the (cosmetic) fused form is the final
        // shape the mutation-inference pass inspects.
        fuse_nullish_assign_shadow_lets(&mut result);
        mark_mutated_vars(&mut result, &self.mut_method_names);
        Ok(result)
    }

    /// Converts a block statement or single statement into IR statements.
    pub(crate) fn convert_block_or_stmt(
        &mut self,
        stmt: &ast::Stmt,
        return_type: Option<&RustType>,
    ) -> Result<Vec<Stmt>> {
        match stmt {
            ast::Stmt::Block(block) => self.convert_stmt_list(&block.stmts, return_type),
            other => self.convert_stmt(other, return_type),
        }
    }
}

#[cfg(test)]
mod tests;
