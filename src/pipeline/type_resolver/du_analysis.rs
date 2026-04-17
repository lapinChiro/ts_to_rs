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
/// fields need to be bound in match arm patterns. This is the single source of
/// truth for DU field access scanning — both TypeResolver (for `DuFieldBinding`
/// registration) and Transformer (for match arm pattern binding construction)
/// must call this function.
///
/// Exhaustively walks all SWC AST `Expr` and `Stmt` variants catalogued in
/// `doc/grammar/ast-variants.md` so that field accesses nested inside any valid
/// TS construct (array/object literals, unary ops, awaits, type assertions,
/// optional chains, loops, nested switches, try/throw, etc.) are collected.
///
/// The walker is **scope-aware** for `obj_var` shadowing (I-148): when a
/// descendant construct introduces a new binding for the same name (e.g.
/// `for (const s of arr)` or `catch (s)` or `const s = ...`), `obj_var.field`
/// inside that scope refers to the inner binding, not the outer DU variable,
/// so those accesses are excluded. Without this guard, the match pattern would
/// falsely bind fields that the case body accesses on an unrelated variable,
/// silently producing wrong runtime values (Tier 1 silent semantic change).
pub(crate) fn collect_du_field_accesses_from_stmts(
    stmts: &[ast::Stmt],
    obj_var: &str,
    tag_field: &str,
) -> Vec<String> {
    let mut fields = Vec::new();
    walk_stmts(stmts, obj_var, tag_field, &mut fields, false);
    fields.sort();
    fields.dedup();
    fields
}

/// Walks a sequence of statements in declaration order, tracking `obj_var`
/// shadowing across sibling statements. Once a sibling declares `obj_var`, all
/// subsequent siblings (and their descendants) are treated as shadowed.
fn walk_stmts(
    stmts: &[ast::Stmt],
    obj_var: &str,
    tag_field: &str,
    fields: &mut Vec<String>,
    mut shadowed: bool,
) {
    for stmt in stmts {
        collect_du_field_accesses_from_stmt_inner(stmt, obj_var, tag_field, fields, shadowed);
        if !shadowed && stmt_declares_name(stmt, obj_var) {
            shadowed = true;
        }
    }
}

/// Returns true if the statement introduces a new binding for `name` in its
/// enclosing lexical scope (i.e. subsequent sibling statements see the new
/// binding, not the outer one).
///
/// Only `var` / `let` / `const` declarations fit this shape. `for` / `for…in` /
/// `for…of` / `try/catch` bindings are scoped to their own body, not to sibling
/// statements, so they are handled at the call site instead of here.
fn stmt_declares_name(stmt: &ast::Stmt, name: &str) -> bool {
    if let ast::Stmt::Decl(ast::Decl::Var(var_decl)) = stmt {
        return var_decl.decls.iter().any(|d| pat_binds_name(&d.name, name));
    }
    false
}

/// Recurses through a destructuring pattern checking whether any of its
/// bindings captures `name`. `Assign`/`Rest`/`Array`/`Object` patterns recurse;
/// `Ident` compares by symbol; `Expr`/`Invalid` bind nothing.
fn pat_binds_name(pat: &ast::Pat, name: &str) -> bool {
    match pat {
        ast::Pat::Ident(ident) => ident.id.sym.as_ref() == name,
        ast::Pat::Assign(assign) => pat_binds_name(&assign.left, name),
        ast::Pat::Rest(rest) => pat_binds_name(&rest.arg, name),
        ast::Pat::Array(arr) => arr
            .elems
            .iter()
            .flatten()
            .any(|elem| pat_binds_name(elem, name)),
        ast::Pat::Object(obj) => obj.props.iter().any(|p| match p {
            ast::ObjectPatProp::KeyValue(kv) => pat_binds_name(&kv.value, name),
            ast::ObjectPatProp::Assign(assign) => assign.key.id.sym.as_ref() == name,
            ast::ObjectPatProp::Rest(rest) => pat_binds_name(&rest.arg, name),
        }),
        ast::Pat::Expr(_) | ast::Pat::Invalid(_) => false,
    }
}

/// Returns true if the `ForHead` (loop binding site for `for`/`for…in`/
/// `for…of`) introduces a new binding for `name`.
fn for_head_binds_name(head: &ast::ForHead, name: &str) -> bool {
    match head {
        ast::ForHead::VarDecl(var_decl) => {
            var_decl.decls.iter().any(|d| pat_binds_name(&d.name, name))
        }
        ast::ForHead::UsingDecl(using) => using.decls.iter().any(|d| pat_binds_name(&d.name, name)),
        ast::ForHead::Pat(pat) => pat_binds_name(pat, name),
    }
}

fn collect_du_field_accesses_from_stmt_inner(
    stmt: &ast::Stmt,
    obj_var: &str,
    tag_field: &str,
    fields: &mut Vec<String>,
    shadowed: bool,
) {
    // If `obj_var` is already shadowed at this point in the enclosing scope,
    // any `obj_var.field` inside this statement (or its descendants) refers to
    // the inner binding, not the outer DU variable. Skip collection wholesale:
    // since JS has lexical scope and we only un-shadow by leaving a scope (not
    // entering one), no descendant can re-expose the outer binding.
    if shadowed {
        return;
    }
    match stmt {
        ast::Stmt::Expr(expr_stmt) => {
            collect_du_field_accesses_from_expr_inner(&expr_stmt.expr, obj_var, tag_field, fields);
        }
        ast::Stmt::Return(ret) => {
            if let Some(arg) = &ret.arg {
                collect_du_field_accesses_from_expr_inner(arg, obj_var, tag_field, fields);
            }
        }
        ast::Stmt::Throw(throw_stmt) => {
            collect_du_field_accesses_from_expr_inner(&throw_stmt.arg, obj_var, tag_field, fields);
        }
        ast::Stmt::Decl(ast::Decl::Var(var_decl)) => {
            for decl in &var_decl.decls {
                if let Some(init) = &decl.init {
                    collect_du_field_accesses_from_expr_inner(init, obj_var, tag_field, fields);
                }
            }
        }
        ast::Stmt::Decl(_) => {
            // Fn / Class / TsInterface / TsTypeAlias / TsEnum — body access to
            // `obj_var` from the enclosing case scope would require closure
            // capture semantics that are outside this walker's scope
            // (I-048 所有権推論 PRD で別途扱う).
        }
        ast::Stmt::If(if_stmt) => {
            collect_du_field_accesses_from_expr_inner(&if_stmt.test, obj_var, tag_field, fields);
            collect_du_field_accesses_from_stmt_inner(
                &if_stmt.cons,
                obj_var,
                tag_field,
                fields,
                false,
            );
            if let Some(alt) = &if_stmt.alt {
                collect_du_field_accesses_from_stmt_inner(alt, obj_var, tag_field, fields, false);
            }
        }
        ast::Stmt::Block(block) => {
            // A block introduces its own scope: sibling decls shadowing
            // `obj_var` only affect subsequent siblings of this block, not
            // this block's own VarDecl-at-top siblings. `walk_stmts` threads
            // the shadowing flag through the block's own stmt list.
            walk_stmts(&block.stmts, obj_var, tag_field, fields, false);
        }
        ast::Stmt::While(while_stmt) => {
            collect_du_field_accesses_from_expr_inner(&while_stmt.test, obj_var, tag_field, fields);
            collect_du_field_accesses_from_stmt_inner(
                &while_stmt.body,
                obj_var,
                tag_field,
                fields,
                false,
            );
        }
        ast::Stmt::DoWhile(do_while) => {
            collect_du_field_accesses_from_expr_inner(&do_while.test, obj_var, tag_field, fields);
            collect_du_field_accesses_from_stmt_inner(
                &do_while.body,
                obj_var,
                tag_field,
                fields,
                false,
            );
        }
        ast::Stmt::For(for_stmt) => {
            // `for (let obj_var = ...; test; update) { body }` — the init
            // binding scopes to the test/update/body only. Compute whether the
            // init shadows `obj_var` and propagate into those scopes.
            let shadowed_in_header = if let Some(init) = &for_stmt.init {
                match init {
                    ast::VarDeclOrExpr::VarDecl(var_decl) => {
                        for decl in &var_decl.decls {
                            if let Some(init_expr) = &decl.init {
                                collect_du_field_accesses_from_expr_inner(
                                    init_expr, obj_var, tag_field, fields,
                                );
                            }
                        }
                        var_decl
                            .decls
                            .iter()
                            .any(|d| pat_binds_name(&d.name, obj_var))
                    }
                    ast::VarDeclOrExpr::Expr(expr) => {
                        collect_du_field_accesses_from_expr_inner(expr, obj_var, tag_field, fields);
                        false
                    }
                }
            } else {
                false
            };
            if !shadowed_in_header {
                if let Some(test) = &for_stmt.test {
                    collect_du_field_accesses_from_expr_inner(test, obj_var, tag_field, fields);
                }
                if let Some(update) = &for_stmt.update {
                    collect_du_field_accesses_from_expr_inner(update, obj_var, tag_field, fields);
                }
            }
            collect_du_field_accesses_from_stmt_inner(
                &for_stmt.body,
                obj_var,
                tag_field,
                fields,
                shadowed_in_header,
            );
        }
        ast::Stmt::ForIn(for_in) => {
            // `for (const obj_var in rhs) { body }` — left binding scopes the
            // body, but the rhs is evaluated BEFORE the binding is introduced,
            // so the rhs still references the outer `obj_var`.
            collect_du_field_accesses_from_expr_inner(&for_in.right, obj_var, tag_field, fields);
            let body_shadowed = for_head_binds_name(&for_in.left, obj_var);
            collect_du_field_accesses_from_stmt_inner(
                &for_in.body,
                obj_var,
                tag_field,
                fields,
                body_shadowed,
            );
        }
        ast::Stmt::ForOf(for_of) => {
            collect_du_field_accesses_from_expr_inner(&for_of.right, obj_var, tag_field, fields);
            let body_shadowed = for_head_binds_name(&for_of.left, obj_var);
            collect_du_field_accesses_from_stmt_inner(
                &for_of.body,
                obj_var,
                tag_field,
                fields,
                body_shadowed,
            );
        }
        ast::Stmt::Switch(switch_stmt) => {
            collect_du_field_accesses_from_expr_inner(
                &switch_stmt.discriminant,
                obj_var,
                tag_field,
                fields,
            );
            for case in &switch_stmt.cases {
                if let Some(test) = &case.test {
                    collect_du_field_accesses_from_expr_inner(test, obj_var, tag_field, fields);
                }
                // Each case body is a fresh lexical scope.
                walk_stmts(&case.cons, obj_var, tag_field, fields, false);
            }
        }
        ast::Stmt::Try(try_stmt) => {
            walk_stmts(&try_stmt.block.stmts, obj_var, tag_field, fields, false);
            if let Some(handler) = &try_stmt.handler {
                // `catch (obj_var) { body }` shadows the outer DU variable
                // for the handler body.
                let handler_shadowed = handler
                    .param
                    .as_ref()
                    .is_some_and(|p| pat_binds_name(p, obj_var));
                walk_stmts(
                    &handler.body.stmts,
                    obj_var,
                    tag_field,
                    fields,
                    handler_shadowed,
                );
            }
            if let Some(finalizer) = &try_stmt.finalizer {
                walk_stmts(&finalizer.stmts, obj_var, tag_field, fields, false);
            }
        }
        ast::Stmt::Labeled(labeled) => {
            collect_du_field_accesses_from_stmt_inner(
                &labeled.body,
                obj_var,
                tag_field,
                fields,
                false,
            );
        }
        // Break / Continue / Empty / Debugger / With — no embedded expression
        // carrying `obj_var.field`. `With` is additionally banned under TS
        // strict mode (`doc/grammar/ast-variants.md` Tier 2).
        ast::Stmt::Break(_)
        | ast::Stmt::Continue(_)
        | ast::Stmt::Empty(_)
        | ast::Stmt::Debugger(_)
        | ast::Stmt::With(_) => {}
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
            // Computed property keys (`obj[expr]`) may embed `obj_var.field`.
            if let ast::MemberProp::Computed(c) = &member.prop {
                collect_du_field_accesses_from_expr_inner(&c.expr, obj_var, tag_field, fields);
            }
        }
        ast::Expr::SuperProp(super_prop) => {
            if let ast::SuperProp::Computed(c) = &super_prop.prop {
                collect_du_field_accesses_from_expr_inner(&c.expr, obj_var, tag_field, fields);
            }
        }
        ast::Expr::Call(call) => {
            if let ast::Callee::Expr(callee) = &call.callee {
                collect_du_field_accesses_from_expr_inner(callee, obj_var, tag_field, fields);
            }
            for arg in &call.args {
                collect_du_field_accesses_from_expr_inner(&arg.expr, obj_var, tag_field, fields);
            }
        }
        ast::Expr::New(new_expr) => {
            collect_du_field_accesses_from_expr_inner(&new_expr.callee, obj_var, tag_field, fields);
            if let Some(args) = &new_expr.args {
                for arg in args {
                    collect_du_field_accesses_from_expr_inner(
                        &arg.expr, obj_var, tag_field, fields,
                    );
                }
            }
        }
        ast::Expr::Bin(bin) => {
            collect_du_field_accesses_from_expr_inner(&bin.left, obj_var, tag_field, fields);
            collect_du_field_accesses_from_expr_inner(&bin.right, obj_var, tag_field, fields);
        }
        ast::Expr::Unary(unary) => {
            collect_du_field_accesses_from_expr_inner(&unary.arg, obj_var, tag_field, fields);
        }
        ast::Expr::Update(update) => {
            collect_du_field_accesses_from_expr_inner(&update.arg, obj_var, tag_field, fields);
        }
        ast::Expr::Tpl(tpl) => {
            for expr in &tpl.exprs {
                collect_du_field_accesses_from_expr_inner(expr, obj_var, tag_field, fields);
            }
        }
        ast::Expr::TaggedTpl(tagged) => {
            collect_du_field_accesses_from_expr_inner(&tagged.tag, obj_var, tag_field, fields);
            for expr in &tagged.tpl.exprs {
                collect_du_field_accesses_from_expr_inner(expr, obj_var, tag_field, fields);
            }
        }
        ast::Expr::Paren(paren) => {
            collect_du_field_accesses_from_expr_inner(&paren.expr, obj_var, tag_field, fields);
        }
        ast::Expr::Assign(assign) => {
            // Both sides of an assignment may reference the DU variable.
            // The LHS is an AssignTarget; only the Simple::Member variant can
            // textually contain `obj_var.<f>` (the Simple::Ident target is a
            // plain name, and destructuring patterns walk their inner exprs).
            match &assign.left {
                ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(member)) => {
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
                    collect_du_field_accesses_from_expr_inner(
                        &member.obj,
                        obj_var,
                        tag_field,
                        fields,
                    );
                    if let ast::MemberProp::Computed(c) = &member.prop {
                        collect_du_field_accesses_from_expr_inner(
                            &c.expr, obj_var, tag_field, fields,
                        );
                    }
                }
                ast::AssignTarget::Simple(ast::SimpleAssignTarget::SuperProp(sp)) => {
                    if let ast::SuperProp::Computed(c) = &sp.prop {
                        collect_du_field_accesses_from_expr_inner(
                            &c.expr, obj_var, tag_field, fields,
                        );
                    }
                }
                ast::AssignTarget::Simple(ast::SimpleAssignTarget::Paren(paren)) => {
                    collect_du_field_accesses_from_expr_inner(
                        &paren.expr,
                        obj_var,
                        tag_field,
                        fields,
                    );
                }
                ast::AssignTarget::Simple(ast::SimpleAssignTarget::OptChain(opt)) => {
                    // OptChainExpr itself doesn't dereference; walk its base
                    // (Member or Call) via the shared OptChain arm handler.
                    walk_opt_chain_base(&opt.base, obj_var, tag_field, fields);
                }
                ast::AssignTarget::Simple(ast::SimpleAssignTarget::TsAs(ts_as)) => {
                    collect_du_field_accesses_from_expr_inner(
                        &ts_as.expr,
                        obj_var,
                        tag_field,
                        fields,
                    );
                }
                ast::AssignTarget::Simple(ast::SimpleAssignTarget::TsTypeAssertion(ta)) => {
                    collect_du_field_accesses_from_expr_inner(&ta.expr, obj_var, tag_field, fields);
                }
                ast::AssignTarget::Simple(ast::SimpleAssignTarget::TsNonNull(tnn)) => {
                    collect_du_field_accesses_from_expr_inner(
                        &tnn.expr, obj_var, tag_field, fields,
                    );
                }
                ast::AssignTarget::Simple(ast::SimpleAssignTarget::TsSatisfies(tsat)) => {
                    collect_du_field_accesses_from_expr_inner(
                        &tsat.expr, obj_var, tag_field, fields,
                    );
                }
                ast::AssignTarget::Simple(ast::SimpleAssignTarget::TsInstantiation(ti)) => {
                    collect_du_field_accesses_from_expr_inner(&ti.expr, obj_var, tag_field, fields);
                }
                ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(_))
                | ast::AssignTarget::Simple(ast::SimpleAssignTarget::Invalid(_)) => {}
                ast::AssignTarget::Pat(pat) => {
                    walk_assign_target_pat_for_du_field(pat, obj_var, tag_field, fields);
                }
            }
            collect_du_field_accesses_from_expr_inner(&assign.right, obj_var, tag_field, fields);
        }
        ast::Expr::Cond(cond) => {
            collect_du_field_accesses_from_expr_inner(&cond.test, obj_var, tag_field, fields);
            collect_du_field_accesses_from_expr_inner(&cond.cons, obj_var, tag_field, fields);
            collect_du_field_accesses_from_expr_inner(&cond.alt, obj_var, tag_field, fields);
        }
        ast::Expr::Array(arr) => {
            for elem in arr.elems.iter().flatten() {
                collect_du_field_accesses_from_expr_inner(&elem.expr, obj_var, tag_field, fields);
            }
        }
        ast::Expr::Object(obj) => {
            for prop in &obj.props {
                match prop {
                    ast::PropOrSpread::Prop(prop) => match prop.as_ref() {
                        ast::Prop::KeyValue(kv) => {
                            if let ast::PropName::Computed(c) = &kv.key {
                                collect_du_field_accesses_from_expr_inner(
                                    &c.expr, obj_var, tag_field, fields,
                                );
                            }
                            collect_du_field_accesses_from_expr_inner(
                                &kv.value, obj_var, tag_field, fields,
                            );
                        }
                        ast::Prop::Assign(assign) => {
                            collect_du_field_accesses_from_expr_inner(
                                &assign.value,
                                obj_var,
                                tag_field,
                                fields,
                            );
                        }
                        ast::Prop::Method(method) => {
                            if let ast::PropName::Computed(c) = &method.key {
                                collect_du_field_accesses_from_expr_inner(
                                    &c.expr, obj_var, tag_field, fields,
                                );
                            }
                        }
                        ast::Prop::Getter(_) | ast::Prop::Setter(_) | ast::Prop::Shorthand(_) => {
                            // Shorthand may reference obj_var as `{ obj_var }` but
                            // that's a ref to the variable itself, not `obj_var.field`.
                        }
                    },
                    ast::PropOrSpread::Spread(spread) => {
                        collect_du_field_accesses_from_expr_inner(
                            &spread.expr,
                            obj_var,
                            tag_field,
                            fields,
                        );
                    }
                }
            }
        }
        ast::Expr::OptChain(opt) => walk_opt_chain_base(&opt.base, obj_var, tag_field, fields),
        ast::Expr::Await(await_expr) => {
            collect_du_field_accesses_from_expr_inner(&await_expr.arg, obj_var, tag_field, fields);
        }
        ast::Expr::Yield(yield_expr) => {
            if let Some(arg) = &yield_expr.arg {
                collect_du_field_accesses_from_expr_inner(arg, obj_var, tag_field, fields);
            }
        }
        ast::Expr::Seq(seq) => {
            for e in &seq.exprs {
                collect_du_field_accesses_from_expr_inner(e, obj_var, tag_field, fields);
            }
        }
        ast::Expr::TsAs(ts_as) => {
            collect_du_field_accesses_from_expr_inner(&ts_as.expr, obj_var, tag_field, fields);
        }
        ast::Expr::TsTypeAssertion(ta) => {
            collect_du_field_accesses_from_expr_inner(&ta.expr, obj_var, tag_field, fields);
        }
        ast::Expr::TsNonNull(tnn) => {
            collect_du_field_accesses_from_expr_inner(&tnn.expr, obj_var, tag_field, fields);
        }
        ast::Expr::TsConstAssertion(tc) => {
            collect_du_field_accesses_from_expr_inner(&tc.expr, obj_var, tag_field, fields);
        }
        ast::Expr::TsSatisfies(ts) => {
            collect_du_field_accesses_from_expr_inner(&ts.expr, obj_var, tag_field, fields);
        }
        ast::Expr::TsInstantiation(ti) => {
            collect_du_field_accesses_from_expr_inner(&ti.expr, obj_var, tag_field, fields);
        }
        // Arrow / Fn bodies may reference `obj_var` via closure capture, but the
        // capture semantics (move vs borrow) interact with DU ownership in ways
        // that are out of this walker's scope. I-048 所有権推論 PRD が扱う。
        // Not collecting here means DU field access inside a closure within a
        // case arm is emitted via inline match, which remains semantically
        // correct albeit verbose.
        ast::Expr::Arrow(_) | ast::Expr::Fn(_) => {}
        // Terminals / reference-only / NA constructs.
        ast::Expr::Ident(_)
        | ast::Expr::Lit(_)
        | ast::Expr::This(_)
        | ast::Expr::Class(_)
        | ast::Expr::MetaProp(_)
        | ast::Expr::PrivateName(_)
        | ast::Expr::Invalid(_)
        | ast::Expr::JSXMember(_)
        | ast::Expr::JSXNamespacedName(_)
        | ast::Expr::JSXEmpty(_)
        | ast::Expr::JSXElement(_)
        | ast::Expr::JSXFragment(_) => {}
    }
}

/// Walks an `OptChainBase` collecting DU field accesses. Shared by both the
/// expression-level `OptChain` arm and the `SimpleAssignTarget::OptChain` arm
/// (assignment target of the form `obj_var.field?.x = rhs`).
fn walk_opt_chain_base(
    base: &ast::OptChainBase,
    obj_var: &str,
    tag_field: &str,
    fields: &mut Vec<String>,
) {
    match base {
        ast::OptChainBase::Member(member) => {
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
            if let ast::MemberProp::Computed(c) = &member.prop {
                collect_du_field_accesses_from_expr_inner(&c.expr, obj_var, tag_field, fields);
            }
        }
        ast::OptChainBase::Call(call) => {
            collect_du_field_accesses_from_expr_inner(&call.callee, obj_var, tag_field, fields);
            for arg in &call.args {
                collect_du_field_accesses_from_expr_inner(&arg.expr, obj_var, tag_field, fields);
            }
        }
    }
}

/// Recurses into destructuring assignment targets (array / object patterns)
/// looking for `obj_var.field` accesses nested inside default-value expressions.
///
/// This is called only when the LHS of an assignment is a `AssignTargetPat`
/// (array / object destructuring). Plain ident destructuring binds LHS names
/// (not relevant to DU field access). But default-value exprs like
/// `[{ a = obj.field } = {}]` may contain field accesses.
fn walk_assign_target_pat_for_du_field(
    pat: &ast::AssignTargetPat,
    obj_var: &str,
    tag_field: &str,
    fields: &mut Vec<String>,
) {
    match pat {
        ast::AssignTargetPat::Array(arr) => {
            for elem in arr.elems.iter().flatten() {
                walk_pat_for_du_field(elem, obj_var, tag_field, fields);
            }
        }
        ast::AssignTargetPat::Object(obj) => {
            for prop in &obj.props {
                match prop {
                    ast::ObjectPatProp::KeyValue(kv) => {
                        if let ast::PropName::Computed(c) = &kv.key {
                            collect_du_field_accesses_from_expr_inner(
                                &c.expr, obj_var, tag_field, fields,
                            );
                        }
                        walk_pat_for_du_field(&kv.value, obj_var, tag_field, fields);
                    }
                    ast::ObjectPatProp::Assign(assign) => {
                        if let Some(default) = &assign.value {
                            collect_du_field_accesses_from_expr_inner(
                                default, obj_var, tag_field, fields,
                            );
                        }
                    }
                    ast::ObjectPatProp::Rest(rest) => {
                        walk_pat_for_du_field(&rest.arg, obj_var, tag_field, fields);
                    }
                }
            }
        }
        ast::AssignTargetPat::Invalid(_) => {}
    }
}

fn walk_pat_for_du_field(pat: &ast::Pat, obj_var: &str, tag_field: &str, fields: &mut Vec<String>) {
    match pat {
        ast::Pat::Array(arr) => {
            for elem in arr.elems.iter().flatten() {
                walk_pat_for_du_field(elem, obj_var, tag_field, fields);
            }
        }
        ast::Pat::Object(obj) => {
            for prop in &obj.props {
                match prop {
                    ast::ObjectPatProp::KeyValue(kv) => {
                        if let ast::PropName::Computed(c) = &kv.key {
                            collect_du_field_accesses_from_expr_inner(
                                &c.expr, obj_var, tag_field, fields,
                            );
                        }
                        walk_pat_for_du_field(&kv.value, obj_var, tag_field, fields);
                    }
                    ast::ObjectPatProp::Assign(assign) => {
                        if let Some(default) = &assign.value {
                            collect_du_field_accesses_from_expr_inner(
                                default, obj_var, tag_field, fields,
                            );
                        }
                    }
                    ast::ObjectPatProp::Rest(rest) => {
                        walk_pat_for_du_field(&rest.arg, obj_var, tag_field, fields);
                    }
                }
            }
        }
        ast::Pat::Assign(assign) => {
            walk_pat_for_du_field(&assign.left, obj_var, tag_field, fields);
            collect_du_field_accesses_from_expr_inner(&assign.right, obj_var, tag_field, fields);
        }
        ast::Pat::Rest(rest) => {
            walk_pat_for_du_field(&rest.arg, obj_var, tag_field, fields);
        }
        ast::Pat::Expr(expr) => {
            collect_du_field_accesses_from_expr_inner(expr, obj_var, tag_field, fields);
        }
        ast::Pat::Ident(_) | ast::Pat::Invalid(_) => {}
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
