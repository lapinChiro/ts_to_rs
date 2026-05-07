//! User `main` symbol detection (B-axis classification of the I-224 PRD problem
//! space) — `detect_user_main` + helpers.
//!
//! Walks the module body to identify the user-defined `main` symbol (or the
//! reserved `__ts_main` collision marker) and classifies its shape into
//! [`super::UserMainKind`]. Extracted from `mod.rs` to keep the file under the
//! 1000-line file-line check threshold while preserving Rule 11 (d-1)
//! self-applied compliance — every `Decl` / `ModuleItem` / `DefaultDecl`
//! variant is enumerated explicitly.
//!
//! **Ambient declaration handling (`is_ambient_decl`)**: TypeScript `declare`-
//! marked declarations (`declare function main()`, `declare const main: T`,
//! `declare class main`, etc.) introduce no Rust runtime construct, so they
//! are treated as B0 (no user main) regardless of the identifier name —
//! the namespace lint (`scan_for_ts_namespace_collisions`) is the orthogonal
//! reservation invariant for `__ts_main` ambient cases.

use swc_ecma_ast::{self as ast, Decl, Expr, Module, ModuleDecl, ModuleItem, Stmt};

use super::UserMainKind;

/// Classifies a single module-level identifier `name` introduced by a declaration of
/// shape `shape` into a [`UserMainKind`], or `None` if `name` is unrelated.
///
/// **Shape parameter**: encodes the structural kind of the declaration's value
/// (`Fn { is_async }` for callable shapes, `NonFn` for non-callable). This lets the
/// classifier produce `FnSync` vs `FnAsync` vs `NonFn` without inspecting the AST itself.
fn classify_main_identifier(name: &str, shape: DeclShape) -> Option<UserMainKind> {
    if name == "__ts_main" {
        // B4: collides with the synthesized rename target.
        Some(UserMainKind::Collision)
    } else if name == "main" {
        match shape {
            DeclShape::Fn { is_async: false } => Some(UserMainKind::FnSync),
            DeclShape::Fn { is_async: true } => Some(UserMainKind::FnAsync),
            DeclShape::NonFn => Some(UserMainKind::NonFn),
        }
    } else {
        // Other identifiers (including non-`__ts_main` `__ts_*` namespace violations,
        // which are rejected separately by `scan_for_ts_namespace_collisions`) are
        // not B-axis triggers.
        None
    }
}

/// Structural shape of a declaration's value, passed to [`classify_main_identifier`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclShape {
    /// Callable shape: `function`, arrow init, fn-expr init.
    Fn { is_async: bool },
    /// Non-callable shape: class / interface / type alias / enum / namespace /
    /// non-callable Var init.
    NonFn,
}

/// Returns `true` if `decl` is an ambient (`declare`-marked) declaration that
/// emits no Rust runtime construct.
///
/// The PRD's B-axis (Axis B) classifies user-defined `main` symbols by their
/// **runtime shape** — `function main()` with a body becomes B1 / B2 (callable),
/// `class main { ... }` with a runtime constructor becomes B3 (non-callable
/// preserved into Rust). Ambient counterparts (`declare function main(): void;`,
/// `declare class main {}`, `declare const main: T;`, `declare enum main`,
/// `declare namespace main`) are TS-namespace-only — they assert the existence
/// of `main` at runtime without providing a body. From the Rust transpiler's POV
/// they introduce no rename target (no body to rename) and no preserved Rust
/// item (nothing to emit), so they are **not B-axis triggers** and are
/// classified as B0 (No user main).
///
/// **Interface / type alias** are inherently type-only regardless of the
/// `declare` keyword (TS spec); they remain B3 NonFn (= name preserved in TS
/// namespace, no Rust-side collision because Rust type / value namespaces are
/// disjoint). **Using declarations** are runtime resource bindings, never
/// `declare`-marked in practice.
///
/// **Collision precedence note**: ambient `__ts_main` (`declare function
/// __ts_main(): void;`) is independently rejected by the namespace lint
/// (`scan_for_ts_namespace_collisions` in `transformer/mod.rs`) regardless of
/// this filter — the namespace reservation invariant operates at the
/// identifier level, not the AST-shape level. Treating ambient `__ts_main` as
/// B0 here is therefore consistent with namespace-lint semantics: the lint
/// fires upstream, and the dispatch tree never sees the source.
fn is_ambient_decl(decl: &Decl) -> bool {
    match decl {
        Decl::Fn(fn_decl) => fn_decl.declare,
        Decl::Var(var_decl) => var_decl.declare,
        Decl::Class(class_decl) => class_decl.declare,
        Decl::TsEnum(enum_decl) => enum_decl.declare,
        Decl::TsModule(module_decl) => module_decl.declare,
        // Always type-only — declare keyword is redundant.
        Decl::TsInterface(_) | Decl::TsTypeAlias(_) => false,
        // Runtime resource bindings.
        Decl::Using(_) => false,
    }
}

/// Classifies a [`Decl`] for B-axis main detection.
///
/// **Rule 11 (d-1) compliance**: every `Decl` variant is enumerated.
fn detect_user_main_from_decl(decl: &Decl) -> Option<UserMainKind> {
    if is_ambient_decl(decl) {
        // Ambient declarations introduce no Rust runtime construct (see
        // [`is_ambient_decl`]'s docstring); they are not B-axis triggers.
        return None;
    }
    match decl {
        Decl::Fn(fn_decl) => {
            let shape = DeclShape::Fn {
                is_async: fn_decl.function.is_async,
            };
            classify_main_identifier(fn_decl.ident.sym.as_str(), shape)
        }
        Decl::Var(var_decl) => {
            for declarator in &var_decl.decls {
                let ast::Pat::Ident(binding) = &declarator.name else {
                    // Destructuring (`const { main } = ...` / `const [main] = ...`) is not a
                    // documented B-axis vector; the matrix's B variants only cover plain
                    // BindingIdent. Skip.
                    continue;
                };
                let name = binding.id.sym.as_str();
                let shape = match declarator.init.as_deref() {
                    Some(Expr::Arrow(arrow)) => DeclShape::Fn {
                        is_async: arrow.is_async,
                    },
                    Some(Expr::Fn(fn_expr)) => DeclShape::Fn {
                        is_async: fn_expr.function.is_async,
                    },
                    // Any other init (Lit / Call / Ident / etc.) is non-callable from
                    // the B-axis perspective: `const main = 42;` is B3 (NonFn).
                    Some(_) | None => DeclShape::NonFn,
                };
                if let Some(kind) = classify_main_identifier(name, shape) {
                    return Some(kind);
                }
            }
            None
        }
        Decl::Class(class_decl) => {
            classify_main_identifier(class_decl.ident.sym.as_str(), DeclShape::NonFn)
        }
        Decl::TsInterface(interface) => {
            classify_main_identifier(interface.id.sym.as_str(), DeclShape::NonFn)
        }
        Decl::TsTypeAlias(alias) => {
            classify_main_identifier(alias.id.sym.as_str(), DeclShape::NonFn)
        }
        Decl::TsEnum(enum_decl) => {
            classify_main_identifier(enum_decl.id.sym.as_str(), DeclShape::NonFn)
        }
        Decl::TsModule(module_decl) => match &module_decl.id {
            ast::TsModuleName::Ident(ident) => {
                classify_main_identifier(ident.sym.as_str(), DeclShape::NonFn)
            }
            // Ambient string-named modules (`declare module "fs" { ... }`) do not
            // introduce a B-axis identifier in the user's namespace.
            ast::TsModuleName::Str(_) => None,
        },
        Decl::Using(using_decl) => {
            for declarator in &using_decl.decls {
                let ast::Pat::Ident(binding) = &declarator.name else {
                    continue;
                };
                if let Some(kind) =
                    classify_main_identifier(binding.id.sym.as_str(), DeclShape::NonFn)
                {
                    return Some(kind);
                }
            }
            None
        }
    }
}

/// Classifies an [`ast::DefaultDecl`] (the inner of `export default function main() {}`
/// etc.) for B-axis main detection.
fn detect_user_main_from_default_decl(decl: &ast::DefaultDecl) -> Option<UserMainKind> {
    match decl {
        ast::DefaultDecl::Fn(fn_expr) => fn_expr.ident.as_ref().and_then(|ident| {
            let shape = DeclShape::Fn {
                is_async: fn_expr.function.is_async,
            };
            classify_main_identifier(ident.sym.as_str(), shape)
        }),
        ast::DefaultDecl::Class(class_expr) => class_expr
            .ident
            .as_ref()
            .and_then(|ident| classify_main_identifier(ident.sym.as_str(), DeclShape::NonFn)),
        ast::DefaultDecl::TsInterfaceDecl(interface) => {
            classify_main_identifier(interface.id.sym.as_str(), DeclShape::NonFn)
        }
    }
}

/// Walks `module.body` and returns the [`UserMainKind`] for the user's `main` /
/// `__ts_main` symbol (or `None` if neither is present).
///
/// **Collision precedence (B4 priority)**: when the body contains both a
/// `__ts_main` collision marker AND a non-collision `main` declaration,
/// [`UserMainKind::Collision`] wins — matching the PRD dispatch tree's
/// `(_, Collision, _)` arm having priority over A/C-axis dispatch.
///
/// **Rule 11 (d-1) compliance**: every `ModuleItem` / `ModuleDecl` variant that can
/// introduce a module-level identifier is enumerated.
#[doc(hidden)] // I-224 internal predicate, exposed for external integration tests.
pub fn detect_user_main(module: &Module) -> UserMainKind {
    let mut detected = UserMainKind::None;
    for item in &module.body {
        let candidate: Option<UserMainKind> = match item {
            ModuleItem::Stmt(Stmt::Decl(decl)) => detect_user_main_from_decl(decl),
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => {
                detect_user_main_from_decl(&export.decl)
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(default)) => {
                detect_user_main_from_default_decl(&default.decl)
            }
            // The remaining ModuleDecl variants (Import / ExportNamed / ExportAll /
            // ExportDefaultExpr / TsImportEquals / TsExportAssignment /
            // TsNamespaceExport) carry references to already-declared names rather
            // than introducing fresh ones — no B-axis detection here.
            ModuleItem::ModuleDecl(
                ModuleDecl::Import(_)
                | ModuleDecl::ExportNamed(_)
                | ModuleDecl::ExportAll(_)
                | ModuleDecl::ExportDefaultExpr(_)
                | ModuleDecl::TsImportEquals(_)
                | ModuleDecl::TsExportAssignment(_)
                | ModuleDecl::TsNamespaceExport(_),
            ) => None,
            // Non-Decl Stmt variants do not introduce module-level identifiers.
            ModuleItem::Stmt(
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
                | Stmt::Expr(_),
            ) => None,
        };
        if let Some(kind) = candidate {
            // Collision wins immediately (= INV-5 priority); other kinds fill the
            // first non-None slot and are not overwritten by later non-collision
            // detections (first-decl precedence within the non-collision class).
            if kind == UserMainKind::Collision {
                return UserMainKind::Collision;
            }
            if detected == UserMainKind::None {
                detected = kind;
            }
        }
    }
    detected
}
