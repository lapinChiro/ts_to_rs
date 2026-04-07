//! Switch statement conversion.
//!
//! Converts TypeScript `switch` statements into Rust `match` expressions.
//! Handles discriminated union switches, typeof switches, string enum switches,
//! clean matches (no fall-through), and fall-through patterns.

use anyhow::Result;
use swc_ecma_ast as ast;

use crate::ir::{BinOp, Expr, MatchArm, Pattern, RustType, Stmt};
use crate::transformer::Transformer;

/// Checks whether a case body is terminated (break, return, throw, or continue).
fn is_case_terminated(stmts: &[ast::Stmt]) -> bool {
    stmts.last().is_some_and(|s| {
        matches!(
            s,
            ast::Stmt::Break(_)
                | ast::Stmt::Return(_)
                | ast::Stmt::Throw(_)
                | ast::Stmt::Continue(_)
        )
    })
}

/// Returns true if the expression is a literal that can safely be used as a Rust match pattern.
///
/// Non-literal expressions (identifiers, function calls, etc.) would become variable bindings
/// in a Rust match, silently changing semantics. These must use match guards instead.
/// `NumberLit` (f64) is excluded because f64 does not implement `Eq`, making it invalid
/// as a match pattern. Numeric cases use match guards (`_ if x == 1.0`) instead.
fn is_literal_match_pattern(expr: &Expr) -> bool {
    match expr {
        Expr::IntLit(_) | Expr::StringLit(_) | Expr::BoolLit(_) => true,
        // Enum variant paths (e.g., Direction::Up) are valid match patterns
        Expr::Ident(name) => name.contains("::"),
        _ => false,
    }
}

/// Builds a combined guard expression from multiple non-literal patterns.
///
/// For a single pattern: `discriminant == pattern`
/// For multiple patterns: `discriminant == p1 || discriminant == p2 || ...`
fn build_combined_guard(discriminant: &Expr, patterns: Vec<Expr>) -> Expr {
    let mut parts = patterns.into_iter().map(|p| Expr::BinaryOp {
        left: Box::new(discriminant.clone()),
        op: BinOp::Eq,
        right: Box::new(p),
    });
    let first = parts.next().expect("at least one pattern");
    parts.fold(first, |acc, part| Expr::BinaryOp {
        left: Box::new(acc),
        op: BinOp::LogicalOr,
        right: Box::new(part),
    })
}

/// switch arm body 内で `obj_var.field` 形式のフィールドアクセスを収集する。
///
/// `tag_field`（discriminant フィールド）はスキップする。
fn collect_du_field_accesses(stmts: &[ast::Stmt], obj_var: &str, tag_field: &str) -> Vec<String> {
    let mut fields = Vec::new();
    for stmt in stmts {
        collect_du_field_accesses_from_stmt(stmt, obj_var, tag_field, &mut fields);
    }
    fields.sort();
    fields.dedup();
    fields
}

fn collect_du_field_accesses_from_stmt(
    stmt: &ast::Stmt,
    obj_var: &str,
    tag_field: &str,
    fields: &mut Vec<String>,
) {
    match stmt {
        ast::Stmt::Expr(expr_stmt) => {
            collect_du_field_accesses_from_expr(&expr_stmt.expr, obj_var, tag_field, fields);
        }
        ast::Stmt::Return(ret) => {
            if let Some(arg) = &ret.arg {
                collect_du_field_accesses_from_expr(arg, obj_var, tag_field, fields);
            }
        }
        ast::Stmt::Decl(ast::Decl::Var(var_decl)) => {
            for decl in &var_decl.decls {
                if let Some(init) = &decl.init {
                    collect_du_field_accesses_from_expr(init, obj_var, tag_field, fields);
                }
            }
        }
        ast::Stmt::If(if_stmt) => {
            collect_du_field_accesses_from_expr(&if_stmt.test, obj_var, tag_field, fields);
            collect_du_field_accesses_from_stmt(&if_stmt.cons, obj_var, tag_field, fields);
            if let Some(alt) = &if_stmt.alt {
                collect_du_field_accesses_from_stmt(alt, obj_var, tag_field, fields);
            }
        }
        ast::Stmt::Block(block) => {
            for s in &block.stmts {
                collect_du_field_accesses_from_stmt(s, obj_var, tag_field, fields);
            }
        }
        _ => {}
    }
}

fn collect_du_field_accesses_from_expr(
    expr: &ast::Expr,
    obj_var: &str,
    tag_field: &str,
    fields: &mut Vec<String>,
) {
    match expr {
        ast::Expr::Member(member) => {
            // Check if this is obj_var.field
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
            // Also recurse into obj in case of nested access
            collect_du_field_accesses_from_expr(&member.obj, obj_var, tag_field, fields);
        }
        ast::Expr::Bin(bin) => {
            collect_du_field_accesses_from_expr(&bin.left, obj_var, tag_field, fields);
            collect_du_field_accesses_from_expr(&bin.right, obj_var, tag_field, fields);
        }
        ast::Expr::Unary(unary) => {
            collect_du_field_accesses_from_expr(&unary.arg, obj_var, tag_field, fields);
        }
        ast::Expr::Call(call) => {
            if let ast::Callee::Expr(callee) = &call.callee {
                collect_du_field_accesses_from_expr(callee, obj_var, tag_field, fields);
            }
            for arg in &call.args {
                collect_du_field_accesses_from_expr(&arg.expr, obj_var, tag_field, fields);
            }
        }
        ast::Expr::Paren(paren) => {
            collect_du_field_accesses_from_expr(&paren.expr, obj_var, tag_field, fields);
        }
        ast::Expr::Tpl(tpl) => {
            for expr in &tpl.exprs {
                collect_du_field_accesses_from_expr(expr, obj_var, tag_field, fields);
            }
        }
        ast::Expr::Cond(cond) => {
            collect_du_field_accesses_from_expr(&cond.test, obj_var, tag_field, fields);
            collect_du_field_accesses_from_expr(&cond.cons, obj_var, tag_field, fields);
            collect_du_field_accesses_from_expr(&cond.alt, obj_var, tag_field, fields);
        }
        _ => {}
    }
}

/// Converts a `switch` statement to a `match` expression or fall-through pattern.
///
/// - If all cases end with `break` (or are empty fall-throughs), generates a clean `Stmt::Match`.
/// - If any case has a non-empty body without `break` (fall-through with code), generates
///   a `LabeledBlock` + flag pattern.
impl<'a> Transformer<'a> {
    /// Converts a `switch` statement to IR match or if-else chain.
    pub(super) fn convert_switch_stmt(
        &mut self,
        switch: &ast::SwitchStmt,
        return_type: Option<&RustType>,
    ) -> Result<Vec<Stmt>> {
        if let Some(result) = self.try_convert_discriminated_union_switch(switch, return_type)? {
            return Ok(result);
        }

        if let Some(result) = self.try_convert_typeof_switch(switch, return_type)? {
            return Ok(result);
        }

        if let Some(result) = self.try_convert_string_enum_switch(switch, return_type)? {
            return Ok(result);
        }

        let mut discriminant = self.convert_expr(&switch.discriminant)?;

        let has_string_cases = switch
            .cases
            .iter()
            .any(|case| matches!(case.test.as_deref(), Some(ast::Expr::Lit(ast::Lit::Str(_)))));
        if has_string_cases {
            discriminant = Expr::MethodCall {
                object: Box::new(discriminant),
                method: "as_str".to_string(),
                args: vec![],
            };
        }

        let case_count = switch.cases.len();
        let has_code_fallthrough = switch.cases.iter().enumerate().any(|(i, case)| {
            let is_last = i == case_count - 1;
            let has_body = !case.cons.is_empty();
            let is_terminated = is_case_terminated(&case.cons);
            has_body && !is_terminated && !is_last
        });

        if has_code_fallthrough {
            self.convert_switch_fallthrough(switch, &discriminant, return_type)
        } else {
            self.convert_switch_clean_match(switch, discriminant, return_type)
        }
    }

    /// `switch (typeof x)` を enum match に変換する。
    fn try_convert_typeof_switch(
        &mut self,
        switch: &ast::SwitchStmt,
        return_type: Option<&RustType>,
    ) -> Result<Option<Vec<Stmt>>> {
        let typeof_ident = match switch.discriminant.as_ref() {
            ast::Expr::Unary(unary) if unary.op == ast::UnaryOp::TypeOf => {
                if let ast::Expr::Ident(ident) = unary.arg.as_ref() {
                    Some(ident)
                } else {
                    None
                }
            }
            _ => None,
        };
        let typeof_ident = match typeof_ident {
            Some(ident) => ident,
            None => return Ok(None),
        };
        let var_name = typeof_ident.sym.to_string();

        let var_type = match self.get_expr_type(&ast::Expr::Ident(typeof_ident.clone())) {
            Some(ty) => ty.clone(),
            None => return Ok(None),
        };
        let enum_name = match &var_type {
            RustType::Named { name, type_args } if type_args.is_empty() => name.clone(),
            _ => return Ok(None),
        };
        if !matches!(
            self.reg().get(&enum_name),
            Some(crate::registry::TypeDef::Enum { .. })
        ) {
            return Ok(None);
        }

        // Bail out if any case has code fall-through (non-empty body without terminator
        // followed by another case). typeof switch with fall-through is unusual and the
        // regular switch conversion handles it correctly.
        let case_count = switch.cases.len();
        let has_code_fallthrough = switch.cases.iter().enumerate().any(|(i, case)| {
            let is_last = i == case_count - 1;
            let has_body = !case.cons.is_empty();
            let is_terminated = is_case_terminated(&case.cons);
            has_body && !is_terminated && !is_last
        });
        if has_code_fallthrough {
            return Ok(None);
        }

        // Build match arms from cases.
        // Pending entries accumulate during empty-body fall-through (case "string": case "number": ...)
        // and are flushed when a non-empty body is encountered — each pending entry generates
        // a separate arm with the same body, because Rust `|` patterns cannot bind different types.
        let mut arms: Vec<MatchArm> = Vec::new();
        let mut pending: Vec<Pattern> = Vec::new();

        for case in &switch.cases {
            if let Some(test) = &case.test {
                // Extract typeof string from the case: case "string", case "number", etc.
                let typeof_str = match test.as_ref() {
                    ast::Expr::Lit(ast::Lit::Str(s)) => s.value.to_string_lossy().into_owned(),
                    _ => return Ok(None), // Non-string case — bail out
                };

                // Resolve to enum variant
                let variant = self.resolve_typeof_to_enum_variant(&var_type, &typeof_str);
                // Structured `Enum::Variant(var_name)` tuple struct pattern.
                // I-377 pre-refactor encoded this as
                // `MatchPattern::Literal(Expr::Ident("Enum::Variant(var_name)"))`, which was
                // a pipeline-integrity violation (display-formatted string in IR).
                let pattern = match variant {
                    Some((ename, vname)) => Pattern::TupleStruct {
                        path: vec![ename, vname],
                        fields: vec![Pattern::binding(var_name.as_str())],
                    },
                    None => {
                        // typeof string doesn't match any variant — skip this conversion
                        return Ok(None);
                    }
                };

                // Accumulate pattern (will be flushed when we hit a non-empty body)
                pending.push(pattern);

                if case.cons.is_empty() {
                    continue;
                }

                // Non-empty body: flush all pending patterns as separate arms with this body.
                for pat in pending.drain(..) {
                    let body: Vec<Stmt> = case
                        .cons
                        .iter()
                        .filter(|s| !matches!(s, ast::Stmt::Break(_) | ast::Stmt::Continue(_)))
                        .map(|s| self.convert_stmt(s, return_type))
                        .collect::<Result<Vec<_>>>()?
                        .into_iter()
                        .flatten()
                        .collect();

                    arms.push(MatchArm {
                        patterns: vec![pat],
                        guard: None,
                        body,
                    });
                }
            } else {
                // default case — also flush any pending patterns
                for pat in pending.drain(..) {
                    let body: Vec<Stmt> = case
                        .cons
                        .iter()
                        .filter(|s| !matches!(s, ast::Stmt::Break(_) | ast::Stmt::Continue(_)))
                        .map(|s| self.convert_stmt(s, return_type))
                        .collect::<Result<Vec<_>>>()?
                        .into_iter()
                        .flatten()
                        .collect();
                    arms.push(MatchArm {
                        patterns: vec![pat],
                        guard: None,
                        body,
                    });
                }
                let body: Vec<Stmt> = case
                    .cons
                    .iter()
                    .filter(|s| !matches!(s, ast::Stmt::Break(_) | ast::Stmt::Continue(_)))
                    .map(|s| self.convert_stmt(s, return_type))
                    .collect::<Result<Vec<_>>>()?
                    .into_iter()
                    .flatten()
                    .collect();

                arms.push(MatchArm {
                    patterns: vec![Pattern::Wildcard],
                    guard: None,
                    body,
                });
            }
        }

        // Flush any remaining pending patterns (trailing empty-body cases that fell off
        // the end of the switch). Generate arms with empty body to preserve match exhaustiveness.
        for pat in pending.drain(..) {
            arms.push(MatchArm {
                patterns: vec![pat],
                guard: None,
                body: vec![],
            });
        }

        // Move wildcard (default) arm to the end — in Rust match, `_` before other arms
        // makes subsequent arms unreachable, but in TS switch, default only matches when
        // no case matched, regardless of source position.
        move_wildcard_arm_to_end(&mut arms);

        Ok(Some(vec![Stmt::Match {
            expr: Expr::Ident(var_name),
            arms,
        }]))
    }

    /// discriminated union の tag フィールドに対する switch を enum match に変換する。
    fn try_convert_discriminated_union_switch(
        &mut self,
        switch: &ast::SwitchStmt,
        return_type: Option<&RustType>,
    ) -> Result<Option<Vec<Stmt>>> {
        use crate::registry::TypeDef;
        // Check if discriminant is a member expression (e.g., s.kind)
        let member = match switch.discriminant.as_ref() {
            ast::Expr::Member(m) => m,
            _ => return Ok(None),
        };

        let field_name = match &member.prop {
            ast::MemberProp::Ident(ident) => ident.sym.to_string(),
            _ => return Ok(None),
        };

        // Resolve the object's type
        let obj_type = self.get_expr_type(&member.obj);
        let enum_name = match obj_type {
            Some(RustType::Named { name, .. }) => name.clone(),
            _ => return Ok(None),
        };

        // Check if this is a discriminated union and the field is the tag
        let (string_values, variant_fields) = match self.reg().get(&enum_name) {
            Some(TypeDef::Enum {
                tag_field: Some(tag),
                string_values,
                variant_fields,
                ..
            }) if *tag == field_name => (string_values, variant_fields),
            _ => return Ok(None),
        };

        // Extract the object variable name for field access rewriting (e.g., "s" from "s.kind")
        let obj_var_name = match member.obj.as_ref() {
            ast::Expr::Ident(ident) => Some(ident.sym.to_string()),
            _ => None,
        };

        // Convert the match: match on &object (not object.tag)
        // Cat A: receiver object
        let object = self.convert_expr(&member.obj)?;
        let match_expr = Expr::Ref(Box::new(object));

        let mut arms: Vec<MatchArm> = Vec::new();
        let mut pending_patterns: Vec<Pattern> = Vec::new();

        for case in &switch.cases {
            if let Some(test) = &case.test {
                // Extract string literal from case
                let str_value = match test.as_ref() {
                    ast::Expr::Lit(ast::Lit::Str(s)) => s.value.to_string_lossy().into_owned(),
                    _ => return Ok(None), // Non-string case → fallback to normal switch
                };

                if let Some(variant_name) = string_values.get(&str_value) {
                    // Struct-variant pattern with no initial bindings. Bindings are
                    // filled in below based on field accesses scanned in the body.
                    pending_patterns.push(Pattern::Struct {
                        path: vec![enum_name.clone(), variant_name.clone()],
                        fields: vec![],
                        rest: true,
                    });
                } else {
                    return Ok(None); // Unknown variant → fallback
                }
            }

            // Empty body = fall-through, accumulate patterns
            if case.cons.is_empty() {
                continue;
            }

            // Scan body for field accesses on the DU variable (e.g., s.radius)
            // and collect field names to bind in the match pattern
            let needed_fields = if let Some(ref var_name) = obj_var_name {
                collect_du_field_accesses(&case.cons, var_name, &field_name)
            } else {
                Vec::new()
            };

            // Update bindings on pending patterns with needed field names
            if !needed_fields.is_empty() {
                for pattern in &mut pending_patterns {
                    if let Pattern::Struct { path, fields, .. } = pattern {
                        // Structured variant name access: the last segment is the
                        // variant name (e.g., `Shape::Circle` → `Circle`).
                        let vname = path.last().cloned().unwrap_or_default();
                        if let Some(type_fields) = variant_fields.get(&vname) {
                            *fields = needed_fields
                                .iter()
                                .filter(|f| type_fields.iter().any(|fd| &fd.name == *f))
                                .map(|f| (f.clone(), Pattern::binding(f.as_str())))
                                .collect();
                        }
                    }
                }
            }

            // TypeResolver's DU field binding detection (is_du_field_binding)
            // handles field variable resolution, so no explicit scope needed.
            let body = case
                .cons
                .iter()
                .filter(|s| !matches!(s, ast::Stmt::Break(_) | ast::Stmt::Continue(_)))
                .map(|s| self.convert_stmt(s, return_type))
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect();

            if case.test.is_none() {
                pending_patterns.push(Pattern::Wildcard);
            }

            arms.push(MatchArm {
                patterns: std::mem::take(&mut pending_patterns),
                guard: None,
                body,
            });
        }

        move_wildcard_arm_to_end(&mut arms);

        Ok(Some(vec![Stmt::Match {
            expr: match_expr,
            arms,
        }]))
    }

    /// Converts a switch on a string enum (non-tagged) into a match with enum variant patterns.
    ///
    /// Detects `switch (dir) { case "up": ... }` where `dir` is typed as a string enum like
    /// `Direction`, and resolves `"up"` → `Direction::Up` using the enum's `string_values` map.
    fn try_convert_string_enum_switch(
        &mut self,
        switch: &ast::SwitchStmt,
        return_type: Option<&RustType>,
    ) -> Result<Option<Vec<Stmt>>> {
        use crate::registry::TypeDef;

        // Resolve the discriminant's type
        let disc_type = self.get_expr_type(&switch.discriminant);
        let enum_name = match disc_type {
            Some(RustType::Named { name, .. }) => name.clone(),
            _ => return Ok(None),
        };

        // Check if this is a string enum (non-tagged, with string_values)
        let string_values = match self.reg().get(&enum_name) {
            Some(TypeDef::Enum {
                tag_field: None,
                string_values,
                ..
            }) if !string_values.is_empty() => string_values,
            _ => return Ok(None),
        };

        // Convert discriminant
        let discriminant = self.convert_expr(&switch.discriminant)?;

        let mut arms: Vec<MatchArm> = Vec::new();
        let mut pending_patterns: Vec<Pattern> = Vec::new();

        for case in &switch.cases {
            if let Some(test) = &case.test {
                // Extract string literal from case
                let str_value = match test.as_ref() {
                    ast::Expr::Lit(ast::Lit::Str(s)) => s.value.to_string_lossy().into_owned(),
                    _ => return Ok(None), // Non-string case → fallback to normal switch
                };

                if let Some(variant_name) = string_values.get(&str_value) {
                    pending_patterns.push(Pattern::UnitStruct {
                        path: vec![enum_name.clone(), variant_name.clone()],
                    });
                } else {
                    return Ok(None); // Unknown variant → fallback
                }
            }

            // Empty body = fall-through, accumulate patterns
            if case.cons.is_empty() {
                continue;
            }

            // Default case
            if case.test.is_none() {
                pending_patterns.push(Pattern::Wildcard);
            }

            let body = case
                .cons
                .iter()
                .filter(|s| !matches!(s, ast::Stmt::Break(_) | ast::Stmt::Continue(_)))
                .map(|s| self.convert_stmt(s, return_type))
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect();

            arms.push(MatchArm {
                patterns: std::mem::take(&mut pending_patterns),
                guard: None,
                body,
            });
        }

        move_wildcard_arm_to_end(&mut arms);

        Ok(Some(vec![Stmt::Match {
            expr: discriminant,
            arms,
        }]))
    }

    /// Converts a switch with no code fall-through into a clean `Stmt::Match`.
    fn convert_switch_clean_match(
        &mut self,
        switch: &ast::SwitchStmt,
        discriminant: Expr,
        return_type: Option<&RustType>,
    ) -> Result<Vec<Stmt>> {
        let mut arms: Vec<MatchArm> = Vec::new();
        let mut pending_patterns: Vec<Pattern> = Vec::new();
        let mut pending_exprs: Vec<Expr> = Vec::new();

        // Build expected type from discriminant type for case value conversion.
        // Only propagate for enum types (Named types registered in registry), not primitives.

        for case in &switch.cases {
            if let Some(test) = &case.test {
                let pattern = self.convert_expr(test)?;
                pending_exprs.push(pattern.clone());
                pending_patterns.push(Pattern::Literal(pattern));
            }

            // Empty body = fall-through to next case, accumulate patterns
            if case.cons.is_empty() {
                continue;
            }

            // Non-empty body: create an arm with all accumulated patterns
            let body = case
                .cons
                .iter()
                .filter(|s| !matches!(s, ast::Stmt::Break(_) | ast::Stmt::Continue(_)))
                .map(|s| self.convert_stmt(s, return_type))
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect();

            if case.test.is_none() {
                pending_patterns.push(Pattern::Wildcard);
            }

            // Check if any pending pattern is non-literal
            let has_non_literal = pending_exprs.iter().any(|e| !is_literal_match_pattern(e));

            let (patterns, guard) = if has_non_literal {
                // Convert to wildcard + guard to avoid variable binding in match
                let guard = build_combined_guard(&discriminant, std::mem::take(&mut pending_exprs));
                std::mem::take(&mut pending_patterns);
                (vec![Pattern::Wildcard], Some(guard))
            } else {
                pending_exprs.clear();
                (std::mem::take(&mut pending_patterns), None)
            };

            arms.push(MatchArm {
                patterns,
                guard,
                body,
            });
        }

        // Move wildcard (default) arm to the end (same rationale as try_convert_typeof_switch)
        move_wildcard_arm_to_end(&mut arms);

        Ok(vec![Stmt::Match {
            expr: discriminant,
            arms,
        }])
    }

    /// Converts a switch with code fall-through into a labeled block + flag pattern.
    fn convert_switch_fallthrough(
        &mut self,
        switch: &ast::SwitchStmt,
        discriminant: &Expr,
        return_type: Option<&RustType>,
    ) -> Result<Vec<Stmt>> {
        let mut block_body = Vec::new();

        // let mut _fall = false;
        block_body.push(Stmt::Let {
            mutable: true,
            name: "_fall".to_string(),
            ty: None,
            init: Some(Expr::BoolLit(false)),
        });

        for case in &switch.cases {
            let ends_with_break = case
                .cons
                .last()
                .is_some_and(|s| matches!(s, ast::Stmt::Break(_)));

            let body: Vec<Stmt> = case
                .cons
                .iter()
                .filter(|s| !matches!(s, ast::Stmt::Break(_)))
                .map(|s| self.convert_stmt(s, return_type))
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect();

            if let Some(test) = &case.test {
                // case val: ...
                // Only propagate enum types to case values, not primitives
                let test_expr = self.convert_expr(test)?;
                let condition = Expr::BinaryOp {
                    left: Box::new(Expr::BinaryOp {
                        left: Box::new(discriminant.clone()),
                        op: BinOp::Eq,
                        right: Box::new(test_expr),
                    }),
                    op: BinOp::LogicalOr,
                    right: Box::new(Expr::Ident("_fall".to_string())),
                };

                let mut then_body = body;
                if ends_with_break {
                    then_body.push(Stmt::Break {
                        label: Some("switch".to_string()),
                        value: None,
                    });
                } else {
                    // No break → set fall-through flag
                    then_body.push(Stmt::Expr(Expr::Assign {
                        target: Box::new(Expr::Ident("_fall".to_string())),
                        value: Box::new(Expr::BoolLit(true)),
                    }));
                }

                block_body.push(Stmt::If {
                    condition,
                    then_body,
                    else_body: None,
                });
            } else {
                // default: ... (always executes if reached)
                block_body.extend(body);
            }
        }

        Ok(vec![Stmt::LabeledBlock {
            label: "switch".to_string(),
            body: block_body,
        }])
    }
}

/// Moves the default arm (wildcard without guard) to the end of the arms list.
///
/// In TypeScript, `default` matches when no `case` matched, regardless of its source position.
/// In Rust `match`, `_` matches everything and makes subsequent arms unreachable.
/// This post-processing ensures the default arm is always last.
///
/// Only targets arms with `Wildcard` pattern and NO guard — non-literal case arms
/// use `Wildcard + guard` to avoid variable binding, and those must remain in place.
fn move_wildcard_arm_to_end(arms: &mut Vec<MatchArm>) {
    if let Some(idx) = arms.iter().position(|arm| {
        arm.guard.is_none() && arm.patterns.iter().any(|p| matches!(p, Pattern::Wildcard))
    }) {
        if idx < arms.len() - 1 {
            let default_arm = arms.remove(idx);
            arms.push(default_arm);
        }
    }
}
