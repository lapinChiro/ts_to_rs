//! User `main` symbol B-axis classification ([`UserMainKind`]) + substitution
//! gate dispatch state ([`UserMainSubstitution`]) — the two cohesive halves of
//! I-224's user main handling concern.
//!
//! [`UserMainKind`] is the **input** dimension (= what shape did the user
//! declare for their `main`?) detected by [`detect_user_main`].
//! [`UserMainSubstitution`] is the **output** dispatch state derived from the
//! `(is_executable_mode, UserMainKind)` pair via
//! [`UserMainSubstitution::from_dispatch`] — consumed by `convert_call_expr` /
//! `convert_fn_decl` / `convert_var_decl_module_level` to decide rename + call
//! site rewriting + the `.await` wrap for async user main.
//!
//! Co-locating both halves of the dispatch (= types + detection + derivation)
//! in a single module keeps related knowledge together (cohesion), eliminates
//! duplicated dispatch table match blocks at the call sites (DRY via
//! [`UserMainSubstitution::from_dispatch`]), and keeps the parent
//! `main_synthesis/mod.rs` file under the 1000-line file-line check threshold
//! while preserving Rule 11 (d-1) self-applied compliance — every `Decl` /
//! `ModuleItem` / `DefaultDecl` variant is enumerated explicitly.
//!
//! **Ambient declaration handling (`is_ambient_decl`)**: TypeScript `declare`-
//! marked declarations (`declare function main()`, `declare const main: T`,
//! `declare class main`, etc.) introduce no Rust runtime construct, so they
//! are treated as B0 (no user main) regardless of the identifier name —
//! the namespace lint (`scan_for_ts_namespace_collisions`) is the orthogonal
//! reservation invariant for `__ts_main` ambient cases.

use swc_ecma_ast::{self as ast, Decl, Expr, Module, ModuleDecl, ModuleItem, Stmt};

/// User-defined `main` symbol classification (Axis B of the PRD problem space).
///
/// Public to support `tests/i224_helper_test.rs::test_dispatch_arm_one_to_one_mapping_per_in_scope_cell`,
/// which composes [`super::is_executable_mode`] / [`detect_user_main`] /
/// [`super::has_top_level_await`] with [`super::classify_dispatch_arm`] to lock
/// in the Rule 9 (a) 1-to-1 mapping invariant.
///
/// Detected by [`detect_user_main`] from a single pass over the module body.
/// Determines the `user_main_kind` dimension of the dispatch tree (Design section #2).
///
/// **Collision precedence**: when multiple module items introduce names matching either
/// `main` or `__ts_main`, [`UserMainKind::Collision`] takes precedence over all other
/// kinds (= INV-5 priority arm at the dispatch level — independent of, and complementary
/// to, the namespace-lint rejection performed by `scan_for_ts_namespace_collisions`
/// in `transformer/mod.rs`).
#[doc(hidden)]
// I-224 internal classification, exposed publicly only for external
// integration tests; not part of the documented public API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserMainKind {
    /// B0: no user-defined `main` symbol.
    None,
    /// B1: synchronous `function main` / sync arrow init / sync fn-expr init.
    FnSync,
    /// B2: async `function main` / async arrow init / async fn-expr init.
    FnAsync,
    /// B3: non-callable `main` symbol — `class main` / `interface main` /
    /// `type main = ...` / `enum main` / `namespace main` / `const main = <non-callable>`.
    NonFn,
    /// B4: user-defined `__ts_main` identifier — collides with the synthesized rename
    /// target [`crate::transformer::expressions::TS_MAIN_RENAME`].
    Collision,
}

/// I-224 user `main` rename + call substitution mode.
///
/// Encodes the dispatch state of the substitution gate as three mutually
/// exclusive cases so that downstream emission paths
/// (= `convert_call_expr` / `convert_fn_decl` / `convert_var_decl_module_level`)
/// can decide rename + call site rewriting **and** the `.await` wrap for async
/// user main without re-deriving the dispatch from
/// `(is_executable_mode, user_main_kind)` at every site.
///
/// **Source of truth**: derived from `(is_executable_mode, UserMainKind)` by
/// [`UserMainSubstitution::from_dispatch`]. The dispatch table:
///
/// | `is_executable_mode` | [`UserMainKind`] | result |
/// |---|---|---|
/// | `false` | (any) | [`UserMainSubstitution::None`] |
/// | `true`  | `None` / `NonFn` / `Collision` | [`UserMainSubstitution::None`] |
/// | `true`  | `FnSync` | [`UserMainSubstitution::SyncRename`] |
/// | `true`  | `FnAsync` | [`UserMainSubstitution::AsyncRename`] |
///
/// Library-mode + B1 / B2 (cells 3 / 5 / 23 / 25) yields `None` because the
/// user's `main` is the binary entry point directly per the
/// `LibraryFnSyncDirect` / `LibraryFnAsyncDirect` arms — no rename or
/// substitution required.
///
/// **Iteration v11 2026-05-08 Tier 1 silent-loss fix**:
/// [`UserMainSubstitution::AsyncRename`] adds the `.await` wrap to the
/// substituted call so `__ts_main().await` runs to completion inside the
/// synthesized `#[tokio::main] async fn main()` body. Without the wrap the
/// returned `Future` would be silently dropped (= the renamed async user
/// main's body never runs and its observable side effects, including
/// `console.log` output, are silently lost = Tier 1 silent semantic change
/// for cells 11 / 23 / 75 + their C1 counterparts after T8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UserMainSubstitution {
    /// No substitution gate fires. `main()` call sites and `function main`
    /// declarations emit verbatim. Used in library mode (any B variant) and
    /// for B0 / B3 / Collision in executable mode.
    None,
    /// Sync rename + sync substituted call (`main()` → `__ts_main()`). Used
    /// for executable mode + B1 (sync user main) cells.
    SyncRename,
    /// Async rename + substituted call wrapped in `.await` (`main()` →
    /// `__ts_main().await`). Used for executable mode + B2 (async user main)
    /// cells, ensuring the renamed async user main runs to completion inside
    /// the synthesized `#[tokio::main] async fn main()` body.
    AsyncRename,
}

impl UserMainSubstitution {
    /// Builds the substitution mode from `(is_executable_mode, UserMainKind)`.
    ///
    /// Encodes the I-224 dispatch table that `Transformer::transform_module` /
    /// `Transformer::transform_module_collecting` previously inlined as
    /// duplicated `match` blocks — this constructor is the single source of
    /// truth (DRY).
    pub(crate) fn from_dispatch(is_executable_mode: bool, user_main_kind: UserMainKind) -> Self {
        if !is_executable_mode {
            return Self::None;
        }
        match user_main_kind {
            UserMainKind::FnSync => Self::SyncRename,
            UserMainKind::FnAsync => Self::AsyncRename,
            UserMainKind::None | UserMainKind::NonFn | UserMainKind::Collision => Self::None,
        }
    }

    /// Returns `true` if the substitution gate fires (= rename + call rewrite).
    /// Both `SyncRename` and `AsyncRename` enable the gate; only `AsyncRename`
    /// additionally wraps the substituted call in `.await`.
    #[inline]
    pub(crate) fn is_active(self) -> bool {
        !matches!(self, Self::None)
    }

    /// Returns `true` only for the async user main substitute (= the case where
    /// the substituted `__ts_main()` call must be wrapped in `.await`).
    #[inline]
    pub(crate) fn is_async(self) -> bool {
        matches!(self, Self::AsyncRename)
    }
}

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

#[cfg(test)]
mod tests {
    //! Unit tests for [`UserMainSubstitution::from_dispatch`] +
    //! [`UserMainSubstitution::is_active`] / [`UserMainSubstitution::is_async`]
    //! predicates. End-to-end behavior is covered by INV-2 / INV-3 / INV-5
    //! invariants tests + e2e cells; these direct unit tests lock in the
    //! Cartesian-product dispatch table at the type level (= decision-table
    //! coverage per `.claude/rules/testing.md`).
    use super::{UserMainKind, UserMainSubstitution};

    /// Decision-table coverage for [`UserMainSubstitution::from_dispatch`]:
    /// 2 (`is_executable_mode` bool) × 5 (`UserMainKind` variants) = 10 cells.
    /// Each cell is enumerated explicitly so a future change to the dispatch
    /// table requires updating this test in lock-step (= bug-affirming
    /// regression detection).
    #[test]
    fn from_dispatch_covers_full_decision_table() {
        // Library mode (is_executable_mode = false): all UserMainKind variants
        // map to None — the user's `main` (if any) is the binary entry directly
        // per the LibraryFnSync/AsyncDirect arms; no substitute fires.
        assert_eq!(
            UserMainSubstitution::from_dispatch(false, UserMainKind::None),
            UserMainSubstitution::None,
        );
        assert_eq!(
            UserMainSubstitution::from_dispatch(false, UserMainKind::FnSync),
            UserMainSubstitution::None,
        );
        assert_eq!(
            UserMainSubstitution::from_dispatch(false, UserMainKind::FnAsync),
            UserMainSubstitution::None,
        );
        assert_eq!(
            UserMainSubstitution::from_dispatch(false, UserMainKind::NonFn),
            UserMainSubstitution::None,
        );
        assert_eq!(
            UserMainSubstitution::from_dispatch(false, UserMainKind::Collision),
            UserMainSubstitution::None,
        );
        // Executable mode (is_executable_mode = true):
        // - FnSync → SyncRename (B1 dispatch arm)
        // - FnAsync → AsyncRename (B2 dispatch arm; .await wrap fires for substituted call)
        // - None / NonFn / Collision → None (no substitute):
        //   - None: no user main to rename (B0).
        //   - NonFn: B3 — user `main` is non-callable (interface / class / etc.); no
        //     `main()` call sites exist in user code that would need rewriting.
        //   - Collision: B4 — user already named their function `__ts_main`; namespace
        //     lint reports this upstream and synthesis is suppressed via the Collision
        //     dispatch arm; no substitute needed for any source-level `main()` calls
        //     (cell-19 fixture exercises this path with both `__ts_main()` and a top
        //     `console.log` but no `main()` call).
        assert_eq!(
            UserMainSubstitution::from_dispatch(true, UserMainKind::None),
            UserMainSubstitution::None,
        );
        assert_eq!(
            UserMainSubstitution::from_dispatch(true, UserMainKind::FnSync),
            UserMainSubstitution::SyncRename,
        );
        assert_eq!(
            UserMainSubstitution::from_dispatch(true, UserMainKind::FnAsync),
            UserMainSubstitution::AsyncRename,
        );
        assert_eq!(
            UserMainSubstitution::from_dispatch(true, UserMainKind::NonFn),
            UserMainSubstitution::None,
        );
        assert_eq!(
            UserMainSubstitution::from_dispatch(true, UserMainKind::Collision),
            UserMainSubstitution::None,
        );
    }

    /// `is_active()` returns `true` iff the substitute gate fires.
    /// Both `SyncRename` and `AsyncRename` enable the gate; only `None`
    /// disables it.
    #[test]
    fn is_active_returns_true_for_rename_variants_only() {
        assert!(!UserMainSubstitution::None.is_active());
        assert!(UserMainSubstitution::SyncRename.is_active());
        assert!(UserMainSubstitution::AsyncRename.is_active());
    }

    /// `is_async()` returns `true` only for `AsyncRename` (= the variant that
    /// adds the `.await` wrap to substituted calls). `None` and `SyncRename`
    /// both return `false` — the wrap must only fire for B2 (async user main)
    /// in executable mode, never for B0 / B1 / B3 / B4.
    #[test]
    fn is_async_returns_true_only_for_async_rename() {
        assert!(!UserMainSubstitution::None.is_async());
        assert!(!UserMainSubstitution::SyncRename.is_async());
        assert!(UserMainSubstitution::AsyncRename.is_async());
    }

    /// **Cross-predicate consistency**: `is_async()` implies `is_active()`
    /// (= the wrap can only fire when the substitute itself fires). Locks in
    /// the structural invariant that the `.await` wrap is **only** a property
    /// of an active substitute, never an orphan async-flag without rename.
    #[test]
    fn is_async_implies_is_active() {
        for variant in [
            UserMainSubstitution::None,
            UserMainSubstitution::SyncRename,
            UserMainSubstitution::AsyncRename,
        ] {
            if variant.is_async() {
                assert!(
                    variant.is_active(),
                    "is_async() => is_active() invariant violated for {variant:?}",
                );
            }
        }
    }
}
