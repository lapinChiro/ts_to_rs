//! I-154 / I-224 module-level `__ts_` namespace reservation lint.
//!
//! Rejects user-defined module-level identifiers whose name is matched by the
//! `__ts_*` reservation rule (= the synthesizer's renamed binary entry symbol
//! `__ts_main` and any future internal target). The lint is a precondition of
//! the I-224 fn main mechanism: if the user defines `function __ts_main() {}`
//! at module scope, the synthesizer cannot safely rename `function main()`
//! into `__ts_main` without producing a name collision in the converted Rust.
//!
//! # Caller contract
//!
//! [`scan_for_ts_namespace_collisions`] walks the [`Module`] body once and
//! returns one [`UnsupportedSyntaxError`] per offending identifier. The two
//! callers are:
//!
//! - [`crate::transformer::Transformer::transform_module`] (early-abort mode):
//!   propagates the **first** error as `Err(_)` and stops conversion.
//! - [`crate::transformer::Transformer::transform_module_collecting`]
//!   (collecting mode): extends its `unsupported` accumulator with all
//!   reported errors and continues converting the rest of the module for
//!   partial output.
//!
//! Both callers invoke this scan **before** any A-axis dispatch
//! (`is_executable_mode`, `detect_user_main`, `try_capture_module_item_into_main_stmts`)
//! so the namespace invariant supersedes structural dispatch.
//!
//! # I-224 INV-5 invariant
//!
//! Every reachable B4 cell (matrix # 9 / 19 / 20 / 29 / 39 / 40 / 49 / 59 / 69 /
//! 79 / 80) contains a top-level `function __ts_main()` (or analogous Decl
//! shape per axis A) and is rejected here with a Tier 2 honest error. The
//! identifier-level reservation strictly precedes A-axis structural dispatch
//! (= the rename target's reachability is independent of axis A / B / C).
//!
//! # Rule 11 (d-1) self-applied compliance
//!
//! Every match over [`ModuleItem`] / [`ModuleDecl`] / [`Stmt`] / [`Decl`] /
//! [`ast::DefaultDecl`] enumerates variants explicitly — no `_ =>` arms. New
//! SWC AST variants force a compile error here so every dispatch site is
//! updated together. Inner-binding `_` placeholders inside enumerated arms
//! (e.g., `Decl::Fn(_)`) are not `_ =>` arms and remain permitted.

use swc_ecma_ast::{self as ast, Decl, Module, ModuleDecl, ModuleItem, Stmt};

use crate::transformer::statements;
use crate::transformer::UnsupportedSyntaxError;

/// Scans the module's top-level for user-defined identifiers that collide
/// with the I-154 reserved `__ts_` prefix namespace, returning a Tier 2 honest
/// error per offending identifier.
///
/// Walks every [`ModuleItem`] and dispatches on AST shape (Decl variant +
/// `ExportDecl` wrapper + `ExportDefaultDecl` with named decl) to extract the
/// introduced identifier(s). Calls
/// [`statements::check_ts_internal_fn_name_namespace`] on each name and
/// accumulates rejection errors into a `Vec` for the caller to consume.
pub(crate) fn scan_for_ts_namespace_collisions(module: &Module) -> Vec<UnsupportedSyntaxError> {
    let mut errors = Vec::new();
    for item in &module.body {
        scan_module_item_for_collisions(item, &mut errors);
    }
    errors
}

/// Inner per-item dispatcher. Per Rule 11 (d-1) `_ =>` is forbidden, so every
/// [`ModuleItem`] / [`ModuleDecl`] / [`Stmt`] variant is enumerated
/// explicitly; variants that do not introduce module-level user-defined
/// identifiers are matched with an empty body documenting the reason.
fn scan_module_item_for_collisions(item: &ModuleItem, errors: &mut Vec<UnsupportedSyntaxError>) {
    match item {
        ModuleItem::ModuleDecl(decl) => match decl {
            ModuleDecl::ExportDecl(export) => scan_decl_for_collisions(&export.decl, errors),
            ModuleDecl::ExportDefaultDecl(default) => {
                scan_default_decl_for_collisions(&default.decl, errors);
            }
            // Empty: imports introduce local bindings only (scoped per-file),
            // re-exports carry already-declared names (covered at the decl
            // site), and `ExportDefaultExpr` / `TsExportAssignment` /
            // `TsNamespaceExport` do not introduce fresh identifiers.
            ModuleDecl::Import(_)
            | ModuleDecl::ExportNamed(_)
            | ModuleDecl::ExportDefaultExpr(_)
            | ModuleDecl::ExportAll(_)
            | ModuleDecl::TsImportEquals(_)
            | ModuleDecl::TsExportAssignment(_)
            | ModuleDecl::TsNamespaceExport(_) => {}
        },
        ModuleItem::Stmt(stmt) => match stmt {
            Stmt::Decl(decl) => scan_decl_for_collisions(decl, errors),
            // Empty: non-Decl statements do not introduce module-level
            // identifiers. Module-level `let __ts_main = ...` lives under
            // `Stmt::Decl(Decl::Var)` and is handled in that branch.
            Stmt::Block(_)
            | Stmt::Empty(_)
            | Stmt::Debugger(_)
            | Stmt::With(_)
            | Stmt::Return(_)
            | Stmt::Labeled(_)
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::If(_)
            | Stmt::Switch(_)
            | Stmt::Throw(_)
            | Stmt::Try(_)
            | Stmt::While(_)
            | Stmt::DoWhile(_)
            | Stmt::For(_)
            | Stmt::ForIn(_)
            | Stmt::ForOf(_)
            | Stmt::Expr(_) => {}
        },
    }
}

/// Per-[`Decl`] variant dispatcher: extracts the introduced identifier(s) and
/// calls the namespace validator on each. Per Rule 11 (d-1) every variant is
/// enumerated explicitly.
fn scan_decl_for_collisions(decl: &Decl, errors: &mut Vec<UnsupportedSyntaxError>) {
    match decl {
        Decl::Fn(fn_decl) => check_ident_for_collision(&fn_decl.ident, errors),
        Decl::Class(class_decl) => check_ident_for_collision(&class_decl.ident, errors),
        Decl::Var(var_decl) => {
            for declarator in &var_decl.decls {
                // BindingIdent form (`const X = ...`, `let X = ...`,
                // `var X = ...`) is the matrix B4-axis shape. Destructuring
                // patterns (`const { X } = ...`, `const [X] = ...`) are not
                // currently a documented collision vector; extend here with a
                // recursive `Pat` walker if one ever surfaces.
                if let ast::Pat::Ident(binding) = &declarator.name {
                    check_ident_for_collision(&binding.id, errors);
                }
            }
        }
        Decl::Using(using_decl) => {
            for declarator in &using_decl.decls {
                if let ast::Pat::Ident(binding) = &declarator.name {
                    check_ident_for_collision(&binding.id, errors);
                }
            }
        }
        Decl::TsInterface(interface) => check_ident_for_collision(&interface.id, errors),
        Decl::TsTypeAlias(alias) => check_ident_for_collision(&alias.id, errors),
        Decl::TsEnum(enum_decl) => check_ident_for_collision(&enum_decl.id, errors),
        Decl::TsModule(module_decl) => match &module_decl.id {
            ast::TsModuleName::Ident(ident) => check_ident_for_collision(ident, errors),
            // Ambient string-named modules (`declare module "fs" { ... }`) do
            // not introduce a collidable Rust identifier — skip.
            ast::TsModuleName::Str(_) => {}
        },
    }
}

/// Default-decl dispatcher: only [`ast::DefaultDecl::Class`] / `Fn` /
/// `TsInterfaceDecl` carry an identifier — anonymous defaults
/// (`export default function() {}`) cannot collide.
fn scan_default_decl_for_collisions(
    decl: &ast::DefaultDecl,
    errors: &mut Vec<UnsupportedSyntaxError>,
) {
    match decl {
        ast::DefaultDecl::Class(class_expr) => {
            if let Some(ident) = &class_expr.ident {
                check_ident_for_collision(ident, errors);
            }
        }
        ast::DefaultDecl::Fn(fn_expr) => {
            if let Some(ident) = &fn_expr.ident {
                check_ident_for_collision(ident, errors);
            }
        }
        ast::DefaultDecl::TsInterfaceDecl(interface) => {
            check_ident_for_collision(&interface.id, errors);
        }
    }
}

/// Helper: validate one identifier against the `__ts_` namespace and append
/// any rejection error to the accumulator. The validator returns the concrete
/// `UnsupportedSyntaxError` directly (no anyhow wrapping), so the only thing
/// to do here is forward the `Err` into the accumulator.
fn check_ident_for_collision(ident: &ast::Ident, errors: &mut Vec<UnsupportedSyntaxError>) {
    if let Err(unsup) = statements::check_ts_internal_fn_name_namespace(&ident.sym, ident.span) {
        errors.push(unsup);
    }
}
