//! visibility 解決、parent 探索、共通ヘルパー。

use std::collections::HashMap;

use swc_ecma_ast as ast;

use crate::ir::visit::{walk_expr, IrVisitor};
use crate::ir::{
    AssocConst, Expr, Item, Method, RustType, Stmt, StructField, TraitRef, TypeParam, Visibility,
};

/// Creates an `Item::Struct` from a name, visibility, type parameters, and fields.
pub(super) fn make_struct(
    name: &str,
    vis: &Visibility,
    type_params: Vec<TypeParam>,
    fields: Vec<StructField>,
) -> Item {
    Item::Struct {
        vis: *vis,
        name: name.to_string(),
        type_params,
        fields,
        is_unit_struct: false,
    }
}

/// 型パラメータのリストから trait 参照を生成する。
///
/// 例: `type_params: [T, U]` → `TraitRef { name: "FooTrait", type_args: [TypeVar("T"), TypeVar("U")] }`
/// (I-387: 型パラメータ参照は構造化 `TypeVar` variant で表現)
pub(super) fn make_trait_ref(name: &str, type_params: &[TypeParam]) -> TraitRef {
    TraitRef {
        name: name.to_string(),
        type_args: type_params
            .iter()
            .map(|p| RustType::TypeVar {
                name: p.name.clone(),
            })
            .collect(),
    }
}

/// Strips visibility from methods for use in trait impl blocks.
pub(super) fn strip_method_visibility(methods: &[Method]) -> Vec<Method> {
    methods
        .iter()
        .map(|m| Method {
            vis: Visibility::Private,
            ..m.clone()
        })
        .collect()
}

/// Creates an `Item::Impl` block from type parameters, constants, constructor, and/or methods.
///
/// Returns `None` if constants, constructor, and methods are all empty.
pub(super) fn make_impl(
    struct_name: &str,
    type_params: Vec<TypeParam>,
    for_trait: Option<TraitRef>,
    consts: Vec<AssocConst>,
    ctor: Option<&Method>,
    methods: Vec<Method>,
) -> Option<Item> {
    let mut all_methods = Vec::new();
    if let Some(c) = ctor {
        all_methods.push(c.clone());
    }
    all_methods.extend(methods);

    if all_methods.is_empty() && consts.is_empty() {
        return None;
    }

    Some(Item::Impl {
        struct_name: struct_name.to_string(),
        type_params,
        for_trait,
        consts,
        methods: all_methods,
    })
}

/// Resolves the effective visibility of a class member based on its TypeScript accessibility modifier.
///
/// `protected` maps to `pub(crate)`, `private` maps to `Private`, and `public` (or unspecified)
/// inherits the class-level visibility.
pub(super) fn resolve_member_visibility(
    accessibility: Option<ast::Accessibility>,
    class_vis: &Visibility,
) -> Visibility {
    match accessibility {
        Some(ast::Accessibility::Protected) => Visibility::PubCrate,
        Some(ast::Accessibility::Private) => Visibility::Private,
        _ => *class_vis,
    }
}

/// Returns `true` if the method body requires a `&mut self` receiver.
///
/// A method body requires `&mut self` if it contains, **at any depth** within its
/// statement tree, any operation that mutates state reached through `self`:
///
/// 1. **Self-rooted Assign target** (Direct / Index / Deref / nested field): the `target`
///    of an [`Expr::Assign`] is rooted at the `self` ident via any chain of
///    [`Expr::FieldAccess`] / [`Expr::Index`] / [`Expr::Deref`] (e.g., `self.x = v`,
///    `self.arr[i] = v`, `self.x.y = v`, `(*self).x = v`)。pre-T10 emission path 拡張、
///    Computed `this[i]` / nested struct write も symmetric に handle。
/// 2. **Setter dispatch on `self`**: `self.set_<x>(...)` (= [`Expr::MethodCall`] whose
///    `object` is the `self` ident and whose `method` starts with `"set_"`、I-205
///    T6/T7/T8/T10 setter dispatch family)。
///
/// ## Detection methodology and trade-offs
///
/// The walker is fully recursive (uses [`IrVisitor`] / [`walk_expr`]) so it correctly
/// detects mutation buried inside:
/// - [`Expr::Block`] (compound assign / update setter desugar
///   `{ let __ts_new = self.x() OP rhs; self.set_x(__ts_new); ... }`)
/// - control-flow constructs ([`Stmt::If`] / [`Stmt::While`] / [`Stmt::ForIn`] /
///   [`Stmt::WhileLet`] / [`Stmt::Loop`] / [`Stmt::IfLet`] / [`Stmt::LabeledBlock`])
/// - [`Expr::Closure`] bodies (mutation inside callbacks captured by `self`)
/// - [`Expr::Match`] arms / [`Expr::If`] expr branches / nested expressions
///
/// **Setter prefix heuristic (case 2)**: detection uses naming convention
/// (`method.starts_with("set_")`) rather than a class-registry lookup of
/// [`crate::ir::MethodKind::Setter`] entries. This is a deliberate trade-off:
///
/// - **Sound**: false positives (user-written non-setter methods named `set_*`) emit
///   `&mut self` which is **strictly more permissive than `&self`** (Rust accepts the
///   widened receiver). No silent semantic divergence.
/// - **No false negatives for T6/T7/T8/T10 dispatch**: those helpers always emit
///   `format!("set_{x}")` as the method name, so all framework-emitted setter calls
///   match the prefix.
/// - **Out-of-scope**: transitive mutation via non-setter method calls
///   (`self.helper()` where `helper(&mut self)` mutates) is **not detected** —
///   tracked in TODO `[I-223]` as a separate "method receiver inference completeness"
///   architectural concern.
///
/// ## I-205 T10 architectural rationale
///
/// pre-T10 では internal `this.x = v` は [`Expr::Assign { FieldAccess, value }`]
/// IR で emit されるため case (1) のみで `&mut self` 推論が成立していた。T6 (Write) /
/// T7 (Update) / T8 (Compound) / T10 (Internal `this.x` = E2) の setter dispatch 拡張により
/// IR は [`Expr::MethodCall { self, "set_x", ... }`] (Plain Write) または
/// [`Expr::Block { Let __ts_new = self.x() OP rhs; self.set_x(__ts_new); ... }`] (Compound /
/// Update) に変化。case (1) のみでは silent `&self` emit (= Rust E0596 compile error
/// "cannot borrow `*self` as mutable" 顕在化) となるため、case (2) を加えて structural
/// symmetry を回復、`this.x = v` 系 Write を含む全 method body で正しく `&mut self` を emit。
///
/// Recursive descent + self-rooted target chain detection are themselves structural
/// improvements over the pre-T10 `body_has_self_assignment` helper, which only inspected
/// top-level [`Stmt::Expr`] and only matched exactly `Expr::Assign { target: FieldAccess(self,
/// ..) }`, missing both nested control-flow assignments and Computed [`Expr::Index`] /
/// nested [`Expr::FieldAccess`] write targets. Now any assign-to-self-rooted-state or
/// setter-dispatch-on-self at any depth correctly triggers `&mut self`.
pub(super) fn body_requires_mut_self_borrow(body: &[Stmt]) -> bool {
    let mut visitor = MutSelfRequirementVisitor { found: false };
    for stmt in body {
        if visitor.found {
            break;
        }
        visitor.visit_stmt(stmt);
    }
    visitor.found
}

/// `IrVisitor` that records whether any visited expression requires `&mut self`.
///
/// Once `found = true` is set, subsequent visits short-circuit (no further descent) so the
/// walker stops as soon as the first mutating operation is identified.
struct MutSelfRequirementVisitor {
    found: bool,
}

impl IrVisitor for MutSelfRequirementVisitor {
    fn visit_expr(&mut self, expr: &Expr) {
        if self.found {
            return;
        }
        match expr {
            // Case (1): assignment to a self-rooted target (FieldAccess / Index / Deref
            // chain rooted at self) → &mut self required. Covers `self.x = v`,
            // `self.arr[i] = v`, `self.x.y = v`, `(*self).x = v` symmetrically (= L3-DD-1
            // Iteration v17 deep-deep review fix、pre-T10 helper の depth + Computed gap
            // を structural に解消)。
            Expr::Assign { target, .. } if target_roots_at_self(target) => {
                self.found = true;
            }
            // Case (2): setter MethodCall on self → &mut self required (T6/T7/T8/T10
            // setter dispatch family、prefix-based heuristic per doc comment trade-offs)。
            Expr::MethodCall { object, method, .. } if is_self_setter_call(object, method) => {
                self.found = true;
            }
            // All other expression shapes: descend into children to find mutation buried
            // inside Block / BinaryOp / FnCall args / closure bodies / etc.
            _ => walk_expr(self, expr),
        }
    }
}

/// Returns `true` if `expr` is the `self` identifier.
///
/// Building block for [`target_roots_at_self`] — captures the "self ident" leaf condition
/// to keep the recursion base thin and DRY.
fn is_self_ident(expr: &Expr) -> bool {
    matches!(expr, Expr::Ident(name) if name == "self")
}

/// Returns `true` if `expr` is rooted at `self` through any chain of [`Expr::FieldAccess`]
/// / [`Expr::Index`] / [`Expr::Deref`].
///
/// Used as the [`Expr::Assign`] `target` predicate by [`MutSelfRequirementVisitor`] to
/// detect assignments that mutate state reachable through `self` — e.g., `self.x = v`,
/// `self.arr[i] = v`, `self.x.y = v`, `(*self).x = v`. All such writes require `&mut self`
/// on the enclosing method receiver.
///
/// I-205 T10 Iteration v17 (deep-deep review L3-DD-1 fix): pre-Iteration-v17 helper
/// matched only the top-level shape `Expr::FieldAccess { object: self, .. }`, missing
/// Computed `this[i] = v` (= [`Expr::Index`] target) and nested `this.x.y = v` (= nested
/// [`Expr::FieldAccess`] target). Both of these are valid TS write patterns that require
/// `&mut self` in Rust; pre-v17 helper missed them, leading to silent `&self` emit and
/// E0596 compile errors. v17 introduces this recursive helper to restore structural
/// symmetry — any write rooted at `self` is correctly detected regardless of access path.
fn target_roots_at_self(expr: &Expr) -> bool {
    match expr {
        // Leaf: bare `self` ident (rare as Assign target — would mean `*self = ...`-like
        // shape, but completes the recursion base)
        Expr::Ident(_) => is_self_ident(expr),
        // Recursive cases: chain access through FieldAccess / Index / Deref
        Expr::FieldAccess { object, .. } => target_roots_at_self(object),
        Expr::Index { object, .. } => target_roots_at_self(object),
        Expr::Deref(inner) => target_roots_at_self(inner),
        // Any other shape (MethodCall return, FnCall, Block expression result, etc.)
        // does not root at `self` from the writer's perspective — even if the value
        // happens to be `self` at runtime, the static structure does not statically
        // identify `self` as the root.
        _ => false,
    }
}

/// Returns `true` if `(object, method)` represents a setter MethodCall on `self` —
/// i.e., `object == self` and `method` starts with `"set_"`.
///
/// I-205 T10: matches the IR emission shape of T6 / T7 / T8 / T10 setter dispatch helpers,
/// which produce `Expr::MethodCall { object: Expr::Ident("self"), method: format!("set_{x}"),
/// args: [...] }` for instance setter dispatch.
///
/// **Naming-convention heuristic trade-off** (`method.starts_with("set_")`): see the
/// `Detection methodology and trade-offs` section in [`body_requires_mut_self_borrow`] doc
/// comment for the rationale. Summary: false positives (user-written non-setter `set_*`
/// methods) are sound (`&mut self` is strictly more permissive); false negatives for
/// T6/T7/T8/T10 dispatch are impossible because those helpers always emit `set_<x>` names.
fn is_self_setter_call(object: &Expr, method: &str) -> bool {
    is_self_ident(object) && method.starts_with("set_")
}

/// Returns `true` if `expr` is a single-hop self field access — i.e.,
/// `Expr::FieldAccess { object: Expr::Ident("self"), field: _ }`.
///
/// I-205 T12 (Iteration v18): Used by [`insert_getter_body_clone_if_self_field_access`]
/// to detect the C1 limited pattern (`return self.field;`) for getter body `.clone()`
/// insertion.
///
/// **Single-hop only**: `self.field.nested` (= nested FieldAccess where the outer
/// object is itself a FieldAccess) returns `false`. The C1 pattern targets only
/// direct field reads of `self`; nested member access falls into the C2
/// comprehensive `.clone()` insertion category (cells 75/76/77/79/80, deferred
/// to a separate PRD).
///
/// Symmetric to [`is_self_ident`] / [`target_roots_at_self`] / [`is_self_setter_call`]
/// in the "self-rooted expression structural recognition" helper family. Unlike
/// `target_roots_at_self`'s recursive descent, this helper enforces single-hop
/// shape by construction (= no recursion).
fn is_self_single_hop_field_access(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::FieldAccess { object, .. } if is_self_ident(object)
    )
}

/// I-205 T12 (Iteration v18): rewrites a Getter body's last statement to insert
/// `.clone()` when it returns a single-hop self field access (C1 limited pattern).
///
/// **Detection**: matches the last [`Stmt`] in `stmts` against
/// - `Stmt::Return(Some(Expr::FieldAccess { object: Ident("self"), .. }))` → rewrite
/// - `Stmt::TailExpr(Expr::FieldAccess { object: Ident("self"), .. })` → rewrite
///
/// **Rewrite**: replaces the inner expression with
/// `Expr::MethodCall { object: <FieldAccess>, method: "clone", args: vec![] }`.
///
/// **No-op cases** (Decision Table C reference, `backlog/I-205-...md`):
/// - Empty body (`stmts.is_empty()`): early return, no rewrite
/// - Last `Stmt` is not `Return(Some(_))` or `TailExpr(_)` (= `Stmt::Return(None)` /
///   `Let` / `Expr` / `If` / `Match` / `While` / etc.): no rewrite (cells 75/76/77/79/80
///   系列 = 別 PRD C2 scope)
/// - Inner `Expr` is not a single-hop self field access (= `self.field.nested` /
///   `obj.field` non-self / `self.field.clone()` already a MethodCall / computed expr /
///   etc.): no rewrite
///
/// **Caller responsibility** (gate location, `members.rs::build_method_inner`):
/// invoke this only when `kind == ast::MethodKind::Getter` AND `return_type` is
/// non-Copy (= `is_copy_type() = false`). The helper itself does not check those
/// conditions; gate is upstream so that Setter / Method / Copy-type Getter cases
/// never reach this helper.
///
/// **Rule 11 (d-1) compliance**: the `Stmt` enum match enumerates all 14 variants
/// explicitly without `_ =>` arm. Adding a new `Stmt` variant in the future will
/// produce a compile error here, forcing explicit treatment.
///
/// Takes `&mut [Stmt]` rather than `&mut Vec<Stmt>` because the helper does not
/// resize the body — only rewrites the last stmt in place via `last_mut()`. Slice
/// API suffices and is preferred per `clippy::ptr_arg`.
pub(super) fn insert_getter_body_clone_if_self_field_access(stmts: &mut [Stmt]) {
    let Some(last) = stmts.last_mut() else {
        return; // Empty body: no rewrite
    };

    let inner_slot: &mut Expr = match last {
        // Rewrite targets:
        Stmt::Return(Some(expr)) => expr,
        Stmt::TailExpr(expr) => expr,
        // Non-target variants — no rewrite (cells 75/76/77/79/80 系列 + others):
        Stmt::Return(None)
        | Stmt::Let { .. }
        | Stmt::If { .. }
        | Stmt::While { .. }
        | Stmt::WhileLet { .. }
        | Stmt::ForIn { .. }
        | Stmt::Loop { .. }
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Expr(_)
        | Stmt::IfLet { .. }
        | Stmt::Match { .. }
        | Stmt::LabeledBlock { .. } => return,
    };

    if !is_self_single_hop_field_access(inner_slot) {
        return; // Body shape mismatch — no rewrite
    }

    // Take ownership of the FieldAccess expr, wrap with .clone() MethodCall.
    // The placeholder `Expr::Ident(String::new())` is overwritten in the next
    // statement, so no observable trace remains.
    let original = std::mem::replace(inner_slot, Expr::Ident(String::new()));
    *inner_slot = Expr::MethodCall {
        object: Box::new(original),
        method: "clone".to_string(),
        args: vec![],
    };
}

/// Pre-scans all interface declarations to collect method names per interface.
///
/// Used by `implements` processing to determine which class methods belong to
/// which trait impl block.
pub(in crate::transformer) fn pre_scan_interface_methods(
    module: &ast::Module,
) -> HashMap<String, Vec<String>> {
    let mut map = HashMap::new();

    for module_item in &module.body {
        let decl = match module_item {
            ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::TsInterface(d))) => d,
            ast::ModuleItem::ModuleDecl(ast::ModuleDecl::ExportDecl(export)) => {
                if let ast::Decl::TsInterface(d) = &export.decl {
                    d
                } else {
                    continue;
                }
            }
            _ => continue,
        };

        let name = decl.id.sym.to_string();
        let method_names: Vec<String> = decl
            .body
            .body
            .iter()
            .filter_map(|member| {
                if let ast::TsTypeElement::TsMethodSignature(method) = member {
                    if let ast::Expr::Ident(ident) = method.key.as_ref() {
                        return Some(ident.sym.to_string());
                    }
                }
                None
            })
            .collect();

        if !method_names.is_empty() {
            map.insert(name, method_names);
        }
    }

    map
}

/// Identifies which classes are parents (are extended by another class).
pub(super) fn find_parent_class_names(
    class_map: &HashMap<String, super::ClassInfo>,
) -> std::collections::HashSet<String> {
    class_map
        .values()
        .filter_map(|info| info.parent.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    //! Unit tests for [`body_requires_mut_self_borrow`] (I-205 T10 structural fix for
    //! `&mut self` inference covering pre-T10 `self.x = value` field assignments,
    //! T6/T7/T8/T10 setter dispatch `self.set_<x>(...)` MethodCalls, and Iteration v17
    //! deep-deep review extension for self-rooted Index/Deref/nested FieldAccess Assign
    //! targets)。
    //!
    //! ## Coverage matrix (testing.md compliance、`test_<target>_<condition>_<expected>`
    //! naming pattern)
    //!
    //! All test names follow the `test_body_requires_mut_self_borrow_<condition>_<expected>`
    //! convention per `.claude/rules/testing.md` (= L1-DD-1 Iteration v17 fix from
    //! deep-deep review、helper-level lock-in 全 rename)。
    //!
    //! ### Case 1 (Self-rooted Assign target) coverage
    //!
    //! - Direct `self.x = v` (FieldAccess) → expected `true`
    //! - `self.arr[i] = v` (Index) → expected `true` (v17 NEW)
    //! - `self.x.y = v` (nested FieldAccess) → expected `true` (v17 NEW)
    //! - `(*self).x = v` (Deref) → expected `true` (v17 NEW、rare but structurally covered)
    //! - Non-self-rooted Assign (e.g., `obj.x = v` / `local = v`) → expected `false`
    //!
    //! ### Case 2 (Self setter dispatch) coverage
    //!
    //! - Direct `self.set_x(v)` → expected `true`
    //! - `set_*` prefix non-self (`obj.set_x(v)`) → expected `false` (false-positive guard)
    //! - Non-setter on self (`self.compute()`) → expected `false` (false-positive guard)
    //!
    //! ### Recursive descent coverage
    //!
    //! - Block containing setter call → expected `true`
    //! - If-stmt then_body with setter → expected `true`
    //! - While loop body with setter → expected `true`
    //! - Closure body with setter → expected `true` (v17 NEW、IrVisitor walk_expr Closure
    //!   arm の symmetric verify)
    //! - Stmt::Let init with setter → expected `true`
    //!
    //! ### Read-only / boundary cases
    //!
    //! - Empty body → expected `false`
    //! - Read-only body (return self.x) → expected `false`

    use super::*;
    use crate::ir::{ClosureBody, Expr};

    fn self_ident() -> Expr {
        Expr::Ident("self".to_string())
    }

    fn self_field(field: &str) -> Expr {
        Expr::FieldAccess {
            object: Box::new(self_ident()),
            field: field.to_string(),
        }
    }

    fn self_method_call(method: &str, args: Vec<Expr>) -> Expr {
        Expr::MethodCall {
            object: Box::new(self_ident()),
            method: method.to_string(),
            args,
        }
    }

    // =========================================================================
    // Boundary: empty body
    // =========================================================================

    #[test]
    fn test_body_requires_mut_self_borrow_empty_body_returns_false() {
        assert!(!body_requires_mut_self_borrow(&[]));
    }

    // =========================================================================
    // Case 1: Self-rooted Assign target
    // =========================================================================

    #[test]
    fn test_body_requires_mut_self_borrow_top_level_self_field_assign_returns_true() {
        // Pre-T10 case (1) baseline: `self.x = 5` direct field assignment must trigger
        // &mut self requirement (regression lock-in for Iteration v17 helper rename).
        let body = vec![Stmt::Expr(Expr::Assign {
            target: Box::new(self_field("x")),
            value: Box::new(Expr::NumberLit(5.0)),
        })];
        assert!(body_requires_mut_self_borrow(&body));
    }

    #[test]
    fn test_body_requires_mut_self_borrow_self_indexed_assign_returns_true() {
        // L3-DD-1 fix (Iteration v17、deep-deep review): `self.arr[i] = v` is
        // `Expr::Assign { target: Index { object: FieldAccess(self, arr), index: i },
        // value: v }`. Pre-v17 helper missed this (only matched top-level FieldAccess
        // shape) → silent &self emit → Rust E0596。target_roots_at_self recursive helper
        // により Index chain rooted at self を検出して structural fix。
        let body = vec![Stmt::Expr(Expr::Assign {
            target: Box::new(Expr::Index {
                object: Box::new(self_field("arr")),
                index: Box::new(Expr::NumberLit(0.0)),
            }),
            value: Box::new(Expr::NumberLit(42.0)),
        })];
        assert!(body_requires_mut_self_borrow(&body));
    }

    #[test]
    fn test_body_requires_mut_self_borrow_self_nested_field_assign_returns_true() {
        // L3-DD-1 fix (Iteration v17): `self.outer.inner = v` は nested FieldAccess
        // chain。target_roots_at_self の recursive descent through FieldAccess を verify。
        let body = vec![Stmt::Expr(Expr::Assign {
            target: Box::new(Expr::FieldAccess {
                object: Box::new(self_field("outer")),
                field: "inner".to_string(),
            }),
            value: Box::new(Expr::NumberLit(1.0)),
        })];
        assert!(body_requires_mut_self_borrow(&body));
    }

    #[test]
    fn test_body_requires_mut_self_borrow_self_deref_field_assign_returns_true() {
        // L3-DD-1 fix (Iteration v17): `(*self).x = v` (rare、Rust 上 explicit deref
        // pattern)。target_roots_at_self の Expr::Deref arm を verify。Iterates with
        // FieldAccess on top of Deref (= `(*self).x` IR shape)。
        let body = vec![Stmt::Expr(Expr::Assign {
            target: Box::new(Expr::FieldAccess {
                object: Box::new(Expr::Deref(Box::new(self_ident()))),
                field: "x".to_string(),
            }),
            value: Box::new(Expr::NumberLit(7.0)),
        })];
        assert!(body_requires_mut_self_borrow(&body));
    }

    #[test]
    fn test_body_requires_mut_self_borrow_non_self_assign_returns_false() {
        // `obj.x = v` (= 別 ident への Assign) — must NOT trigger &mut self
        // (= false-positive guard for target_roots_at_self の base case)。
        let body = vec![Stmt::Expr(Expr::Assign {
            target: Box::new(Expr::FieldAccess {
                object: Box::new(Expr::Ident("obj".to_string())),
                field: "x".to_string(),
            }),
            value: Box::new(Expr::NumberLit(5.0)),
        })];
        assert!(!body_requires_mut_self_borrow(&body));
    }

    // =========================================================================
    // Case 2: Self setter dispatch
    // =========================================================================

    #[test]
    fn test_body_requires_mut_self_borrow_top_level_self_setter_call_returns_true() {
        // T10 case (2): `self.set_x(5.0)` — must require &mut self
        // (Rust setter signature `&mut self`、caller body must propagate)。
        let body = vec![Stmt::Expr(self_method_call(
            "set_x",
            vec![Expr::NumberLit(5.0)],
        ))];
        assert!(body_requires_mut_self_borrow(&body));
    }

    #[test]
    fn test_body_requires_mut_self_borrow_setter_call_on_non_self_returns_false() {
        // `obj.set_x(5)` where `obj != self` — false-positive guard for `is_self_setter_call`
        // の object check。
        let other_setter_call = Expr::MethodCall {
            object: Box::new(Expr::Ident("obj".to_string())),
            method: "set_x".to_string(),
            args: vec![Expr::NumberLit(5.0)],
        };
        let body = vec![Stmt::Expr(other_setter_call)];
        assert!(!body_requires_mut_self_borrow(&body));
    }

    #[test]
    fn test_body_requires_mut_self_borrow_non_setter_method_call_on_self_returns_false() {
        // `self.compute()` (no `set_` prefix) — false-positive guard for
        // `is_self_setter_call` の method prefix check。
        let body = vec![Stmt::Expr(self_method_call("compute", vec![]))];
        assert!(!body_requires_mut_self_borrow(&body));
    }

    // =========================================================================
    // Recursive descent coverage
    // =========================================================================

    #[test]
    fn test_body_requires_mut_self_borrow_block_with_setter_call_returns_true() {
        // T8 compound assign emit shape: `{ let __ts_new = self.x() + 1; self.set_x(__ts_new); __ts_new }`
        // Recursive walker must descend into Block.stmts to find the setter call.
        let block_expr = Expr::Block(vec![
            Stmt::Let {
                mutable: false,
                name: "__ts_new".to_string(),
                ty: None,
                init: Some(Expr::BinaryOp {
                    left: Box::new(self_method_call("x", vec![])),
                    op: crate::ir::BinOp::Add,
                    right: Box::new(Expr::NumberLit(1.0)),
                }),
            },
            Stmt::Expr(self_method_call(
                "set_x",
                vec![Expr::Ident("__ts_new".to_string())],
            )),
            Stmt::TailExpr(Expr::Ident("__ts_new".to_string())),
        ]);
        let body = vec![Stmt::Expr(block_expr)];
        assert!(body_requires_mut_self_borrow(&body));
    }

    #[test]
    fn test_body_requires_mut_self_borrow_if_stmt_with_setter_in_then_returns_true() {
        // `if cond { self.set_x(5) }` — pre-T10 helper (top-level Expr::Assign-only) missed
        // even `if cond { self.x = 5 }` due to lack of recursion. Recursive walker fixes
        // this for both case 1 and case 2.
        let body = vec![Stmt::If {
            condition: Expr::BoolLit(true),
            then_body: vec![Stmt::Expr(self_method_call(
                "set_x",
                vec![Expr::NumberLit(5.0)],
            ))],
            else_body: None,
        }];
        assert!(body_requires_mut_self_borrow(&body));
    }

    #[test]
    fn test_body_requires_mut_self_borrow_while_loop_with_setter_returns_true() {
        // `while cond { self.set_x(0) }` — recursive descent into Stmt::While.body
        let body = vec![Stmt::While {
            label: None,
            condition: Expr::BoolLit(true),
            body: vec![Stmt::Expr(self_method_call(
                "set_x",
                vec![Expr::NumberLit(0.0)],
            ))],
        }];
        assert!(body_requires_mut_self_borrow(&body));
    }

    #[test]
    fn test_body_requires_mut_self_borrow_closure_body_with_setter_returns_true() {
        // L3 cross-axis verification (Iteration v17、deep-deep review): closure body
        // (= `[x].forEach(i => this.set_x(i))` 等の callback) で setter dispatch が発火
        // した場合、closure 自体は `&mut self` capture を必要とするため outer method も
        // `&mut self` 必須。`walk_expr` の `Expr::Closure` arm を経由する recursive descent
        // を verify (= IrVisitor walker の closure 経路 lock-in、subsequent T で closure
        // 機能拡張時に regression detection)。
        let closure_expr = Expr::Closure {
            params: vec![],
            return_type: None,
            body: ClosureBody::Expr(Box::new(self_method_call(
                "set_x",
                vec![Expr::NumberLit(1.0)],
            ))),
        };
        let body = vec![Stmt::Expr(closure_expr)];
        assert!(body_requires_mut_self_borrow(&body));
    }

    #[test]
    fn test_body_requires_mut_self_borrow_let_init_with_setter_call_returns_true() {
        // `let v = self.set_and_get(0);` — setter-like call buried in let-init expression。
        // Note: `set_and_get` starts with `set_` so it triggers the heuristic. False
        // positive scenario: a hypothetical `set_and_compute` that doesn't actually mutate
        // would also trigger, but this is sound (= &mut self is strictly more permissive
        // than &self).
        let body = vec![Stmt::Let {
            mutable: false,
            name: "v".to_string(),
            ty: None,
            init: Some(self_method_call("set_and_get", vec![])),
        }];
        assert!(body_requires_mut_self_borrow(&body));
    }

    // =========================================================================
    // Read-only / no mutation
    // =========================================================================

    #[test]
    fn test_body_requires_mut_self_borrow_read_only_body_returns_false() {
        // `return self.x;` — read-only body without any mutation
        let body = vec![Stmt::Return(Some(self_field("x")))];
        assert!(!body_requires_mut_self_borrow(&body));
    }
}
