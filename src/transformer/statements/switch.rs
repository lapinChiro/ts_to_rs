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
///
/// Peeks through `ast::Stmt::Block` wrappers (I-153 T0): `case 1: { return 1; }`
/// is terminated by the inner return, even though the surface-level last stmt is
/// a block. Without this, the switch emission would take the fallthrough path
/// unnecessarily.
fn is_case_terminated(stmts: &[ast::Stmt]) -> bool {
    stmts.last().is_some_and(|s| match s {
        ast::Stmt::Break(_)
        | ast::Stmt::Return(_)
        | ast::Stmt::Throw(_)
        | ast::Stmt::Continue(_) => true,
        ast::Stmt::Block(block) => is_case_terminated(&block.stmts),
        _ => false,
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
        // I-378: enum unit variants are structured `Expr::EnumVariant`. They
        // render as `Enum::Variant` paths which are valid Rust path patterns.
        // (Previously relied on `Expr::Ident("Enum::Variant")` substring detection,
        // a broken-window pattern eliminated by I-378.)
        Expr::EnumVariant { .. } => true,
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

// DU field access collection is delegated to
// `crate::pipeline::type_resolver::du_analysis::collect_du_field_accesses_from_stmts`,
// which is the single source of truth for DU switch scanning. See that function
// for the full AST coverage spec (all `Expr` and `Stmt` variants exhaustively
// matched per `doc/grammar/ast-variants.md`).
use crate::pipeline::type_resolver::du_analysis::collect_du_field_accesses_from_stmts;

/// I-153: Recursively rewrites nested bare `Stmt::Break { label: None, value: None }`
/// in switch case body into labeled breaks targeting the switch's labeled block.
///
/// # Descent policy (exhaustive over 14 IR `Stmt` variants)
///
/// - **Descent** (same-switch scope): `Stmt::If.{then_body, else_body}`,
///   `Stmt::IfLet.{then_body, else_body}`, `Stmt::Match.arms[*].body`.
/// - **Skip** (inner emission 所掌): `Stmt::LabeledBlock { body, .. }` — inner
///   emissions (nested switch / try / do-while) already routed their own breaks
///   to their own labels. Outer walker must not re-target them.
/// - **Non-descent** (loop boundary): `Stmt::While / WhileLet / ForIn / Loop` —
///   inner `break` correctly targets inner loop in both TS and Rust.
/// - **Leaf**: `Stmt::Let / Continue / Return / Expr / TailExpr` (Rust AST 上
///   `Stmt::Break` は Expr の中に埋め込まれない、empirical 確認済) +
///   labeled `Stmt::Break { label: Some(_), .. }` or `Stmt::Break { value: Some(_), .. }`
///   は pass-through。
///
/// Returns `true` if any rewrite occurred (used by switch emission paths to decide
/// whether the whole `Stmt::Match` needs to be wrapped in a `Stmt::LabeledBlock`).
pub(super) fn rewrite_nested_bare_break_in_stmts(stmts: &mut [Stmt], switch_label: &str) -> bool {
    let mut rewritten = false;
    for stmt in stmts.iter_mut() {
        rewritten |= rewrite_nested_bare_break_in_stmt(stmt, switch_label);
    }
    rewritten
}

fn rewrite_nested_bare_break_in_stmt(stmt: &mut Stmt, switch_label: &str) -> bool {
    match stmt {
        // ============ DESCENT (non-loop, non-fn 境界) ============
        Stmt::If {
            then_body,
            else_body,
            ..
        } => {
            let mut r = rewrite_nested_bare_break_in_stmts(then_body, switch_label);
            if let Some(eb) = else_body {
                r |= rewrite_nested_bare_break_in_stmts(eb, switch_label);
            }
            r
        }
        Stmt::IfLet {
            then_body,
            else_body,
            ..
        } => {
            let mut r = rewrite_nested_bare_break_in_stmts(then_body, switch_label);
            if let Some(eb) = else_body {
                r |= rewrite_nested_bare_break_in_stmts(eb, switch_label);
            }
            r
        }
        Stmt::Match { arms, .. } => {
            // Must visit every arm (no short-circuit), hence the explicit loop
            // rather than `.any(...)` which stops at the first `true`.
            let mut any_rewrite = false;
            for arm in arms.iter_mut() {
                any_rewrite |= rewrite_nested_bare_break_in_stmts(&mut arm.body, switch_label);
            }
            any_rewrite
        }

        // ============ SKIP (inner emission 所掌尊重) ============
        // Nested `Stmt::LabeledBlock` (e.g., `__ts_switch` from inner switch,
        // `__ts_try_block` from try, `__ts_do_while` from do-while) already has
        // its own bare break rewriting applied. User labels (future I-158) also
        // must be respected — user's `break L` inside such a block targets the
        // user's label, not our switch.
        Stmt::LabeledBlock { .. } => false,

        // ============ LEAF: bare break → labeled break ============
        Stmt::Break {
            label: None,
            value: None,
        } => {
            *stmt = Stmt::Break {
                label: Some(switch_label.to_string()),
                value: None,
            };
            true
        }

        // ============ NON-DESCENT (loop 境界、inner break は inner loop を正しく target) ============
        Stmt::While { .. } | Stmt::WhileLet { .. } | Stmt::ForIn { .. } | Stmt::Loop { .. } => {
            false
        }

        // ============ LEAF (break を含まない、または既 labeled break) ============
        // - `Stmt::Let`: init Expr に Stmt::Break は埋め込まれない (empirical)
        // - `Stmt::Break { label: Some(_), .. }` / `{ value: Some(_), .. }`: labeled break / break-with-value は pass-through
        // - `Stmt::Continue`: continue は TS/Rust で同 semantics (enclosing loop)
        // - `Stmt::Return / Expr / TailExpr`: Expr に Stmt::Break 非埋め込み (empirical)
        Stmt::Let { .. }
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Return(_)
        | Stmt::Expr(_)
        | Stmt::TailExpr(_) => false,
    }
}

/// I-153: The internal label used by ts_to_rs for switch-escape emission.
///
/// Reserved under the `__ts_` namespace (see `check_ts_internal_label_namespace` in
/// the transformer module). User labels starting with `__ts_` are rejected at
/// `convert_labeled_stmt` entry, so collision with user code is impossible.
pub(crate) const TS_INTERNAL_SWITCH_LABEL: &str = "__ts_switch";

/// I-153: Conditionally wraps a `Stmt::Match` produced by a clean-match switch path
/// in a `Stmt::LabeledBlock` when any arm body contains a nested bare break that
/// was rewritten to `break '__ts_switch`.
///
/// The wrap is conditional to avoid emitting an unused label (Rust warns on
/// `unused_labels`). When no rewrite is needed, the original `Stmt::Match` is
/// returned unchanged.
fn wrap_match_with_switch_label_if_needed(arms: Vec<MatchArm>, match_expr: Expr) -> Vec<Stmt> {
    let mut arms = arms;
    let mut rewritten = false;
    for arm in arms.iter_mut() {
        rewritten |= rewrite_nested_bare_break_in_stmts(&mut arm.body, TS_INTERNAL_SWITCH_LABEL);
    }
    let match_stmt = Stmt::Match {
        expr: match_expr,
        arms,
    };
    if rewritten {
        vec![Stmt::LabeledBlock {
            label: TS_INTERNAL_SWITCH_LABEL.to_string(),
            body: vec![match_stmt],
        }]
    } else {
        vec![match_stmt]
    }
}

/// Converts a `switch` statement to a `match` expression or fall-through pattern.
///
/// - If all cases end with `break` (or are empty fall-throughs), generates a clean `Stmt::Match`.
/// - If any case has a non-empty body without `break` (fall-through with code), generates
///   a `LabeledBlock` + flag pattern.
impl<'a> Transformer<'a> {
    /// Converts the `cons` of a `switch` case into IR statements, filtering
    /// out terminators per the caller's policy.
    ///
    /// `drop_continue = true` filters both `break` and `continue` (the normal
    /// clean-match / fall-through cases where `continue` escapes the switch).
    /// `drop_continue = false` keeps `continue` (used in the labeled-block
    /// fall-through emission path where `continue` carries meaning).
    ///
    /// `??=` emission within case bodies is driven by the CFG-analyzer
    /// hints populated by `TypeResolver::collect_emission_hints` on the
    /// enclosing function body, so this method does not participate in that
    /// dispatch directly (I-144 T6-1 retired the per-case pre-check).
    fn convert_switch_case_body(
        &mut self,
        cons: &[ast::Stmt],
        return_type: Option<&RustType>,
        drop_continue: bool,
    ) -> Result<Vec<Stmt>> {
        let mut result = Vec::new();
        for stmt in cons {
            if matches!(stmt, ast::Stmt::Break(_)) {
                continue;
            }
            if drop_continue && matches!(stmt, ast::Stmt::Continue(_)) {
                continue;
            }
            result.extend(self.convert_stmt(stmt, return_type)?);
        }
        Ok(result)
    }

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
                        ctor: crate::ir::PatternCtor::UserEnumVariant {
                            enum_ty: crate::ir::UserTypeRef::new(ename),
                            variant: vname,
                        },
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
                    let body = self.convert_switch_case_body(&case.cons, return_type, true)?;

                    arms.push(MatchArm {
                        patterns: vec![pat],
                        guard: None,
                        body,
                    });
                }
            } else {
                // default case — also flush any pending patterns
                for pat in pending.drain(..) {
                    let body = self.convert_switch_case_body(&case.cons, return_type, true)?;
                    arms.push(MatchArm {
                        patterns: vec![pat],
                        guard: None,
                        body,
                    });
                }
                let body = self.convert_switch_case_body(&case.cons, return_type, true)?;

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

        // I-153: conditionally wrap in `'__ts_switch:` labeled block if any arm body
        // has nested bare breaks that need to target the switch (not an outer loop).
        Ok(Some(wrap_match_with_switch_label_if_needed(
            arms,
            Expr::Ident(var_name),
        )))
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
                    // Emit `Pattern::UnitStruct` for variants without fields —
                    // the `{ .. }` struct-pattern shape is valid but unidiomatic
                    // for unit variants (e.g., `Status::Active` not `Status::Active { .. }`).
                    // For data variants, emit `Pattern::Struct { rest: true }` so
                    // needed_fields can be filled in below.
                    let is_unit_variant = variant_fields
                        .get(variant_name)
                        .map(|fields| fields.is_empty())
                        .unwrap_or(false);
                    let ctor = crate::ir::PatternCtor::UserEnumVariant {
                        enum_ty: crate::ir::UserTypeRef::new(enum_name.clone()),
                        variant: variant_name.clone(),
                    };
                    let pattern = if is_unit_variant {
                        Pattern::UnitStruct { ctor }
                    } else {
                        Pattern::Struct {
                            ctor,
                            fields: vec![],
                            rest: true,
                        }
                    };
                    pending_patterns.push(pattern);
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
                collect_du_field_accesses_from_stmts(&case.cons, var_name, &field_name)
            } else {
                Vec::new()
            };

            // Update bindings on pending patterns with needed field names
            if !needed_fields.is_empty() {
                for pattern in &mut pending_patterns {
                    if let Pattern::Struct { ctor, fields, .. } = pattern {
                        // Structured variant name access: extract the variant from
                        // the structured ctor (UserEnumVariant constructed above).
                        let vname = match ctor {
                            crate::ir::PatternCtor::UserEnumVariant { variant, .. } => {
                                variant.clone()
                            }
                            _ => String::new(),
                        };
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
            let body = self.convert_switch_case_body(&case.cons, return_type, true)?;

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

        // I-153: conditional labeled-block wrap (same rationale as typeof switch).
        Ok(Some(wrap_match_with_switch_label_if_needed(
            arms, match_expr,
        )))
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
                        ctor: crate::ir::PatternCtor::UserEnumVariant {
                            enum_ty: crate::ir::UserTypeRef::new(enum_name.clone()),
                            variant: variant_name.clone(),
                        },
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

            let body = self.convert_switch_case_body(&case.cons, return_type, true)?;

            arms.push(MatchArm {
                patterns: std::mem::take(&mut pending_patterns),
                guard: None,
                body,
            });
        }

        move_wildcard_arm_to_end(&mut arms);

        // I-153: conditional labeled-block wrap (string-enum switch path).
        Ok(Some(wrap_match_with_switch_label_if_needed(
            arms,
            discriminant,
        )))
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
            let body = self.convert_switch_case_body(&case.cons, return_type, true)?;

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

        // I-153: conditional labeled-block wrap (clean-match path).
        Ok(wrap_match_with_switch_label_if_needed(arms, discriminant))
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

            // Labeled-block fallthrough emission preserves `continue` (it
            // carries control flow to the next case body); only `break` is
            // dropped.
            let mut body = self.convert_switch_case_body(&case.cons, return_type, false)?;

            // I-153: rewrite nested bare breaks in case body to target `'__ts_switch`.
            // The arm-end break is appended below (already labeled), so the walker only
            // affects genuinely nested breaks (inside if / block / try-catch / IfLet / Match arms).
            rewrite_nested_bare_break_in_stmts(&mut body, TS_INTERNAL_SWITCH_LABEL);

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
                        label: Some(TS_INTERNAL_SWITCH_LABEL.to_string()),
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
            label: TS_INTERNAL_SWITCH_LABEL.to_string(),
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
