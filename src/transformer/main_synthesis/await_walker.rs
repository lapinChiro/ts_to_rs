//! Hand-rolled recursive walker for "module top-level `Expr::Await` detection
//! reachable WITHOUT crossing function / arrow / class body boundaries"
//! (= I-228 main fix per Spec stage 逆戻り 2026-05-07).
//!
//! Used by [`super::has_top_level_await`] (AST-only top-level await predicate)
//! and [`super::classify_init_kind`] (per-declarator await detection in
//! VarDecl init expressions). All 38 SWC `Expr` variants enumerated explicitly
//! per Rule 11 (d-1) self-applied compliance — no `_ =>` arms. Sub-walkers
//! ([`callee_contains_await_recursive`], [`opt_chain_base_contains_await_recursive`],
//! [`assign_target_contains_await_recursive`],
//! [`simple_assign_target_contains_await_recursive`]) carry the same compliance
//! standard for nested SWC AST shapes.
//!
//! Boundary variants (`Expr::Fn` / `Expr::Arrow` / `Expr::Class`) return
//! `false` — `await` inside a nested function body is in a separate async
//! context and does not contribute to top-level await detection.

use swc_ecma_ast::{self as ast, Expr};

/// Returns `true` if `expr` contains an [`Expr::Await`] sub-node reachable
/// WITHOUT crossing a function / arrow / class body boundary.
///
/// Recursive walker hand-rolled for I-228 main fix (= Spec stage 逆戻り
/// 2026-05-07): all 38 SWC `Expr` variants enumerated explicitly per Rule 11
/// (d-1) self-applied compliance, no `_ =>` arms. Boundary variants (Fn / Arrow
/// / Class) return `false` — `await` inside a nested function body is in a
/// separate async context and does not contribute to top-level await detection.
pub(super) fn expr_contains_await_recursive(expr: &Expr) -> bool {
    match expr {
        // === Target ===
        Expr::Await(_) => true,

        // === Boundary: function body is a separate async context ===
        Expr::Fn(_) | Expr::Arrow(_) => false,

        // === Class: function-body boundary, but super_class / decorators /
        // computed-key members ARE evaluated in the outer (enclosing) context.
        // Walk those sub-Exprs; do not walk method bodies. ===
        Expr::Class(class_expr) => class_contains_await_recursive(&class_expr.class),

        // === Leaves: no inner Expr ===
        Expr::This(_)
        | Expr::Lit(_)
        | Expr::Ident(_)
        | Expr::MetaProp(_)
        | Expr::PrivateName(_)
        | Expr::Invalid(_)
        | Expr::JSXEmpty(_)
        | Expr::JSXNamespacedName(_)
        | Expr::JSXMember(_) => false,

        // === Single-Expr containers ===
        Expr::Unary(u) => expr_contains_await_recursive(&u.arg),
        Expr::Update(u) => expr_contains_await_recursive(&u.arg),
        Expr::Paren(p) => expr_contains_await_recursive(&p.expr),
        Expr::TsTypeAssertion(t) => expr_contains_await_recursive(&t.expr),
        Expr::TsConstAssertion(t) => expr_contains_await_recursive(&t.expr),
        Expr::TsNonNull(t) => expr_contains_await_recursive(&t.expr),
        Expr::TsAs(t) => expr_contains_await_recursive(&t.expr),
        Expr::TsInstantiation(t) => expr_contains_await_recursive(&t.expr),
        Expr::TsSatisfies(t) => expr_contains_await_recursive(&t.expr),

        // === Yield: optional inner Expr ===
        Expr::Yield(y) => y.arg.as_deref().is_some_and(expr_contains_await_recursive),

        // === Multi-Expr containers ===
        Expr::Bin(b) => {
            expr_contains_await_recursive(&b.left) || expr_contains_await_recursive(&b.right)
        }
        Expr::Cond(c) => {
            expr_contains_await_recursive(&c.test)
                || expr_contains_await_recursive(&c.cons)
                || expr_contains_await_recursive(&c.alt)
        }
        Expr::Seq(s) => s.exprs.iter().any(|e| expr_contains_await_recursive(e)),
        Expr::Tpl(t) => t.exprs.iter().any(|e| expr_contains_await_recursive(e)),
        Expr::TaggedTpl(t) => {
            expr_contains_await_recursive(&t.tag)
                || t.tpl.exprs.iter().any(|e| expr_contains_await_recursive(e))
        }
        Expr::Member(m) => {
            expr_contains_await_recursive(&m.obj)
                || matches!(
                    &m.prop,
                    ast::MemberProp::Computed(c) if expr_contains_await_recursive(&c.expr)
                )
        }
        Expr::SuperProp(s) => matches!(
            &s.prop,
            ast::SuperProp::Computed(c) if expr_contains_await_recursive(&c.expr)
        ),
        Expr::Call(c) => {
            callee_contains_await_recursive(&c.callee)
                || c.args
                    .iter()
                    .any(|a| expr_contains_await_recursive(&a.expr))
        }
        Expr::New(n) => {
            expr_contains_await_recursive(&n.callee)
                || n.args
                    .as_ref()
                    .is_some_and(|args| args.iter().any(|a| expr_contains_await_recursive(&a.expr)))
        }
        Expr::OptChain(o) => opt_chain_base_contains_await_recursive(&o.base),
        Expr::Assign(a) => {
            assign_target_contains_await_recursive(&a.left)
                || expr_contains_await_recursive(&a.right)
        }
        Expr::Array(arr) => arr.elems.iter().any(|elem_opt| {
            elem_opt
                .as_ref()
                .is_some_and(|e| expr_contains_await_recursive(&e.expr))
        }),
        Expr::Object(obj) => obj.props.iter().any(|prop_or_spread| match prop_or_spread {
            ast::PropOrSpread::Spread(s) => expr_contains_await_recursive(&s.expr),
            ast::PropOrSpread::Prop(p) => match p.as_ref() {
                ast::Prop::Shorthand(_) => false,
                // Object KeyValue: walk both key (= computed expression at outer
                // context, e.g., `{ [await x]: 1 }`) and value (e.g., `{ k: await x }`).
                ast::Prop::KeyValue(kv) => {
                    prop_name_contains_await_recursive(&kv.key)
                        || expr_contains_await_recursive(&kv.value)
                }
                // Object AssignProp (`{ x = default }` shorthand-default form): key is
                // Ident (no inner expr), only value can contain await.
                ast::Prop::Assign(a) => expr_contains_await_recursive(&a.value),
                // Getter / Setter / Method: function body is a boundary (nested async
                // context), but the key (= computed PropName) IS evaluated in the outer
                // context. Walk the key only — `{ [await x](): void {} }` triggers
                // top-level await via the computed key, not via the body.
                ast::Prop::Method(m) => prop_name_contains_await_recursive(&m.key),
                ast::Prop::Getter(g) => prop_name_contains_await_recursive(&g.key),
                ast::Prop::Setter(s) => prop_name_contains_await_recursive(&s.key),
            },
        }),
        // JSX top-level usage is rare in TS module-level execution; conservative
        // false (= no recursion into JSX children). If reachable in real TS code,
        // expand to walk JSXElement.children / JSXFragment.children.
        Expr::JSXElement(_) | Expr::JSXFragment(_) => false,
    }
}

/// `Class` recursive walker for class-shape outer-context await detection.
///
/// Walks the class's **outer-context** sub-Exprs (= evaluated when the class is
/// defined, not when methods are called):
/// - `super_class`: `class C extends f(await x) {}` — call expression at outer scope.
/// - `body[i]` member computed keys: `class C { [await x](): void {} }` — key
///   computed at outer scope.
///
/// Does NOT walk method bodies (= function-body boundary, separate async
/// context), property value initializers (= instance-construction or
/// class-definition sync context, TS rejects `await` either way), or class /
/// member decorators (= ts_to_rs's `parse_typescript` uses `TsSyntax::default()`
/// which does **not** enable decorator syntax, so decorator-related AST nodes
/// are unreachable from any source the parser accepts; walking them would be
/// dead code). Per Rule 11 (d-1): all `ClassMember` variants enumerated.
///
/// Used by both [`expr_contains_await_recursive`] (for `Expr::Class`) and
/// [`super::has_top_level_await`] / [`super::is_executable_mode`] (for top-level
/// `Decl::Class`). Sharing the walker eliminates the silent-divergence risk
/// between `Expr::Class` and `Decl::Class` paths.
pub(super) fn class_contains_await_recursive(class: &ast::Class) -> bool {
    if class
        .super_class
        .as_deref()
        .is_some_and(expr_contains_await_recursive)
    {
        return true;
    }
    class.body.iter().any(class_member_contains_await_recursive)
}

/// `ClassMember` recursive walker — checks **outer-context** sub-Exprs only.
/// See [`class_contains_await_recursive`] for the scope rationale (decorators /
/// property initializers / method bodies are intentionally skipped). Per
/// Rule 11 (d-1): all `ClassMember` variants enumerated.
fn class_member_contains_await_recursive(member: &ast::ClassMember) -> bool {
    match member {
        // Computed property keys (PropName::Computed) at the outer (class-
        // definition) context.
        ast::ClassMember::Method(m) => prop_name_contains_await_recursive(&m.key),
        ast::ClassMember::ClassProp(p) => prop_name_contains_await_recursive(&p.key),
        // PrivateName key has no nested Expr (= just identifier).
        ast::ClassMember::PrivateMethod(_) | ast::ClassMember::PrivateProp(_) => false,
        // Constructor (function-body boundary) / StaticBlock (sync class-init
        // context, TS rejects await) / TsIndexSignature (type-only) / Empty
        // (no inner expr) / AutoAccessor (Stage-3 proposal, conservative skip).
        ast::ClassMember::Constructor(_)
        | ast::ClassMember::StaticBlock(_)
        | ast::ClassMember::TsIndexSignature(_)
        | ast::ClassMember::Empty(_)
        | ast::ClassMember::AutoAccessor(_) => false,
    }
}

/// `PropName` recursive walker for object literal computed property keys
/// (= `{ [await x]: 1 }` / `{ [await x](): void {} }` / etc.). Per Rule 11
/// (d-1): all `PropName` variants enumerated.
fn prop_name_contains_await_recursive(name: &ast::PropName) -> bool {
    match name {
        // Static names: no inner expression to walk.
        ast::PropName::Ident(_)
        | ast::PropName::Str(_)
        | ast::PropName::Num(_)
        | ast::PropName::BigInt(_) => false,
        // Computed: `[<expr>]` evaluates the expression at the outer (enclosing)
        // context, NOT inside the property's value (= for Method/Getter/Setter the
        // function body is a boundary but the key isn't).
        ast::PropName::Computed(c) => expr_contains_await_recursive(&c.expr),
    }
}

/// `Callee` recursive walker for `expr_contains_await_recursive`. Per Rule 11
/// (d-1): all `Callee` variants enumerated.
fn callee_contains_await_recursive(callee: &ast::Callee) -> bool {
    match callee {
        ast::Callee::Super(_) | ast::Callee::Import(_) => false,
        ast::Callee::Expr(e) => expr_contains_await_recursive(e),
    }
}

/// `OptChainBase` recursive walker. Per Rule 11 (d-1): all variants enumerated.
fn opt_chain_base_contains_await_recursive(base: &ast::OptChainBase) -> bool {
    match base {
        ast::OptChainBase::Member(m) => {
            expr_contains_await_recursive(&m.obj)
                || matches!(
                    &m.prop,
                    ast::MemberProp::Computed(c) if expr_contains_await_recursive(&c.expr)
                )
        }
        ast::OptChainBase::Call(c) => {
            expr_contains_await_recursive(&c.callee)
                || c.args
                    .iter()
                    .any(|a| expr_contains_await_recursive(&a.expr))
        }
    }
}

/// `AssignTarget` recursive walker. Per Rule 11 (d-1): both Simple / Pat
/// variants enumerated; Pat is destructuring without inner Expr (= no recursion).
fn assign_target_contains_await_recursive(target: &ast::AssignTarget) -> bool {
    match target {
        ast::AssignTarget::Simple(s) => simple_assign_target_contains_await_recursive(s),
        // Destructuring pattern (`[a, b] = ...` / `{a, b} = ...`); inner default
        // values can contain Expr but for top-level await detection at module level
        // this is a corner case; conservative false.
        ast::AssignTarget::Pat(_) => false,
    }
}

/// `SimpleAssignTarget` recursive walker. Per Rule 11 (d-1): all variants
/// enumerated.
fn simple_assign_target_contains_await_recursive(s: &ast::SimpleAssignTarget) -> bool {
    match s {
        ast::SimpleAssignTarget::Ident(_) => false,
        ast::SimpleAssignTarget::Member(m) => {
            expr_contains_await_recursive(&m.obj)
                || matches!(
                    &m.prop,
                    ast::MemberProp::Computed(c) if expr_contains_await_recursive(&c.expr)
                )
        }
        ast::SimpleAssignTarget::SuperProp(s) => matches!(
            &s.prop,
            ast::SuperProp::Computed(c) if expr_contains_await_recursive(&c.expr)
        ),
        ast::SimpleAssignTarget::Paren(p) => expr_contains_await_recursive(&p.expr),
        ast::SimpleAssignTarget::OptChain(o) => opt_chain_base_contains_await_recursive(&o.base),
        ast::SimpleAssignTarget::TsAs(t) => expr_contains_await_recursive(&t.expr),
        ast::SimpleAssignTarget::TsSatisfies(t) => expr_contains_await_recursive(&t.expr),
        ast::SimpleAssignTarget::TsNonNull(t) => expr_contains_await_recursive(&t.expr),
        ast::SimpleAssignTarget::TsTypeAssertion(t) => expr_contains_await_recursive(&t.expr),
        ast::SimpleAssignTarget::TsInstantiation(t) => expr_contains_await_recursive(&t.expr),
        ast::SimpleAssignTarget::Invalid(_) => false,
    }
}
