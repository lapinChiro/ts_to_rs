//! Assignment and update expression conversion.

use anyhow::{anyhow, Result};
use swc_common::Spanned;
use swc_ecma_ast as ast;

use crate::ir::{BinOp, ClosureBody, Expr, RustType, Stmt};
use crate::transformer::statements::nullish_assign::{pick_strategy, NullishAssignStrategy};
use crate::transformer::{Transformer, UnsupportedSyntaxError};

use super::member_access::extract_non_computed_field_name;
use super::member_dispatch::{
    dispatch_instance_member_update, dispatch_static_member_update, LogicalCompoundContext,
    MemberReceiverClassification,
};
use super::TS_OLD_BINDING;

/// Maps an arithmetic / bitwise compound `AssignOp` to the corresponding `BinOp`
/// for T8 setter dispatch. Returns `None` for plain `=`、A5 logical compound
/// (`??=`/`&&=`/`||=`) — those are routed through their own paths
/// (`dispatch_member_write` for plain `=`、`nullish_assign.rs` /
/// `compound_logical_assign.rs` for logical compound).
///
/// Op-axis orthogonality merge (Rule 1 (1-4) compliance): all 11 arithmetic /
/// bitwise compound ops share the same dispatch logic; this 1-to-1 mapping is
/// the single source of truth for the conversion (= matrix cells 20-29 +
/// 30-35 are op-axis orthogonality-equivalent under this mapping).
fn arithmetic_compound_op_to_binop(op: ast::AssignOp) -> Option<BinOp> {
    match op {
        ast::AssignOp::AddAssign => Some(BinOp::Add),
        ast::AssignOp::SubAssign => Some(BinOp::Sub),
        ast::AssignOp::MulAssign => Some(BinOp::Mul),
        ast::AssignOp::DivAssign => Some(BinOp::Div),
        ast::AssignOp::ModAssign => Some(BinOp::Mod),
        ast::AssignOp::BitAndAssign => Some(BinOp::BitAnd),
        ast::AssignOp::BitOrAssign => Some(BinOp::BitOr),
        ast::AssignOp::BitXorAssign => Some(BinOp::BitXor),
        ast::AssignOp::LShiftAssign => Some(BinOp::Shl),
        ast::AssignOp::RShiftAssign => Some(BinOp::Shr),
        ast::AssignOp::ZeroFillRShiftAssign => Some(BinOp::UShr),
        // Plain `=` (T6 plain Assign path) and A5 logical compound (??= / &&= /
        // ||=、T9 scope) are not in T8's scope: caller dispatches these through
        // their own paths.
        ast::AssignOp::Assign
        | ast::AssignOp::NullishAssign
        | ast::AssignOp::AndAssign
        | ast::AssignOp::OrAssign
        | ast::AssignOp::ExpAssign => None,
    }
}

impl<'a> Transformer<'a> {
    /// Converts an assignment expression (`target = value`) to `Expr::Assign`.
    pub(crate) fn convert_assign_expr(&mut self, assign: &ast::AssignExpr) -> Result<Expr> {
        // I-205 T6: plain `obj.x = v` (or `Class.x = v`) Member target は class member
        // dispatch (Write context) を経由する。setter dispatch (B3 / B4 / B8 setter) は
        // `Expr::MethodCall set_x` / `Expr::FnCall UserAssocFn set_x`、Tier 2 honest error
        // (B2 read-only / B6 method / B7 inherited) は `UnsupportedSyntaxError`、B1 field /
        // B9 unknown / static field は既存 FieldAccess Assign emission を `dispatch_member_write`
        // helper 内 fallback で統合実装。`AssignOp::Assign` (= `=`) で `MemberProp::Ident |
        // PrivateName` のみ gate (Computed `obj[i] = v` は既存 `convert_member_expr_for_write`
        // の `Expr::Index` 経路で handle、本 dispatch では unreachable)。Compound (+=, -=, etc.)
        // / nullish (??=) / logical (&&=, ||=) は subsequent T7-T9 で別途 setter dispatch 実装、
        // 本 T6 では plain `=` のみ gate。本 fix なしだと B3/B4/B8 instance/static setter で
        // existing `convert_member_expr_for_write` の FieldAccess Assign 経路に流れ、
        // `obj.x = v;` (struct field assign for non-existent field) Rust syntax で Tier 2
        // compile error を emit する状態 (= 既存 Tier 2 broken framework)。
        if assign.op == ast::AssignOp::Assign {
            if let ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(member)) = &assign.left
            {
                if matches!(
                    &member.prop,
                    ast::MemberProp::Ident(_) | ast::MemberProp::PrivateName(_)
                ) {
                    let value = self.convert_expr(&assign.right)?;
                    return self.dispatch_member_write(member, value);
                }
            }
        }

        // I-205 T8: arithmetic / bitwise compound assign × Member × non-Computed
        // dispatch gate。`obj.x += v` (and the 10 sibling ops) routes through
        // `dispatch_member_compound` (T8) for class member dispatch (B4 instance
        // setter desugar / B8 static setter desugar / B2 read-only / B3 write-only-
        // read-fail / B6 method / B7 inherited Tier 2 honest error)、Fallback
        // (B1 field / B9 unknown / non-class receiver / static field) で既存
        // `Expr::Assign { FieldAccess, BinaryOp }` desugar emit (regression preserve)。
        // A5 logical compound (`??=`/`&&=`/`||=`) は subsequent T9 で setter dispatch
        // integration を `nullish_assign.rs` / `compound_logical_assign.rs` 経由で別途追加。
        if let Some(op) = arithmetic_compound_op_to_binop(assign.op) {
            if let ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(member)) = &assign.left
            {
                if matches!(
                    &member.prop,
                    ast::MemberProp::Ident(_) | ast::MemberProp::PrivateName(_)
                ) {
                    let rhs = self.convert_expr(&assign.right)?;
                    return self.dispatch_member_compound(member, op, rhs);
                }
            }
        }

        // I-205 T9: logical compound (`??=` / `&&=` / `||=`) × Member dispatch
        // gate (expression context)。Static / Instance with class member dispatch
        // (B2/B3/B4/B6/B7/B8) は `try_dispatch_member_logical_compound` 経由で
        // setter desugar emission、Fallback (B1 field / B9 unknown / non-class
        // receiver / static field / Computed) は `Ok(None)` 返却で既存 path に
        // 流す (cells 36 + 41-e regression preserve)。Statement-context (`obj.x ??= d;`
        // / `obj.x &&= v;` bare stmt) は別途 `try_convert_nullish_assign_stmt` /
        // `try_convert_compound_logical_assign_stmt` の Member arm で同 helper を
        // 呼ぶ、expression vs statement context は `LogicalCompoundContext` 引数で
        // 分岐 (Expression = Block + tail = `<getter>` / Statement = Block stmts
        // only、no tail)。
        if matches!(
            assign.op,
            ast::AssignOp::NullishAssign | ast::AssignOp::AndAssign | ast::AssignOp::OrAssign
        ) {
            if let ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(member)) = &assign.left
            {
                if let Some(block) = self.try_dispatch_member_logical_compound(
                    member,
                    assign.op,
                    &assign.right,
                    LogicalCompoundContext::Expression,
                )? {
                    return Ok(block);
                }
            }
        }

        let target = match &assign.left {
            ast::AssignTarget::Simple(simple) => match simple {
                ast::SimpleAssignTarget::Member(member) => {
                    self.convert_member_expr_for_write(member)?
                }
                ast::SimpleAssignTarget::Ident(ident) => Expr::Ident(ident.id.sym.to_string()),
                _ => return Err(anyhow!("unsupported assignment target")),
            },
            _ => return Err(anyhow!("unsupported assignment target pattern")),
        };

        // ??= (NullishAssign) — expression-context path (I-142).
        //
        // Statement-context `x ??= d;` is intercepted earlier in
        // `convert_stmt` by `try_convert_nullish_assign_stmt` and rewritten to
        // a shadow-let that preserves TS's post-`??=` narrowing. We only get
        // here when `x ??= d` appears inside a larger expression (call arg,
        // return value, ternary branch, condition, etc.), where the value of
        // the `??=` expression is observed.
        //
        // LHS-type dispatch goes through `pick_strategy` so that the Problem
        // Space matrix is encoded in exactly one place (see
        // `backlog/I-142-nullish-assign-shadow-let.md`):
        //
        // - `Option<T>` (`ShadowLet`): emit `*x.get_or_insert_with(|| d)`
        //   (Copy) or `x.get_or_insert_with(|| d).clone()` (!Copy).
        // - non-nullable `T` (`Identity`): `??=` is dead code at runtime →
        //   emit just `target` (Copy) or `target.clone()` (!Copy).
        // - `Any` (`BlockedByI050`): requires `serde_json::Value`-aware
        //   runtime null check + RHS coercion — surfaced as unsupported
        //   until the I-050 Any coercion umbrella PRD lands.
        //
        // `FieldAccess` LHS uses `get_or_insert_with` (same as Ident) or
        // `if is_none { assign Some(d) }` (stmt). `Index` LHS (HashMap) uses
        // `entry().or_insert_with()` — dispatched before `pick_strategy`
        // because HashMap ??= is key-existence, not Option null. (I-142-b/c)
        //
        // D-3: RHS conversion is strategy-local. `Identity` and `BlockedByI050`
        // never observe the RHS (dead at runtime / surfaced before emission),
        // so converting it up front would be dead work *and* could introduce
        // side-effect IR (expr-type recording, mutability marking) that TS
        // semantics don't perform — in `x ??= (y ??= z)` TS skips evaluation
        // of the inner `??=` entirely when `x` is non-null. Only `ShadowLet`
        // converts the RHS.
        if assign.op == ast::AssignOp::NullishAssign {
            // Resolve LHS type for strategy dispatch. Ident uses scoped var
            // type; Member uses TypeResolver's expr_types (populated by the
            // I-142-b/c TypeResolver extension above).
            let lhs_type = match &assign.left {
                ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(ident)) => self
                    .get_type_for_var(&ident.id.sym, ident.id.span)
                    .ok_or_else(|| {
                        UnsupportedSyntaxError::new(
                            "nullish-assign on unresolved type",
                            assign.span(),
                        )
                    })?,
                ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(member)) => self
                    .get_expr_type(&ast::Expr::Member(member.clone()))
                    .ok_or_else(|| {
                        UnsupportedSyntaxError::new(
                            "nullish-assign on unresolved member type",
                            assign.span(),
                        )
                    })?,
                _ => {
                    return Err(UnsupportedSyntaxError::new(
                        "unsupported nullish-assign target",
                        assign.span(),
                    )
                    .into())
                }
            };

            // I-142-c: Index on HashMap → entry().or_insert_with() bypasses
            // pick_strategy because HashMap ??= is key-existence, not Option null.
            if let Expr::Index {
                ref object,
                ref index,
            } = target
            {
                let right = self.convert_expr(&assign.right)?;
                let is_copy = lhs_type.is_copy_type();
                let closure = Expr::Closure {
                    params: vec![],
                    return_type: None,
                    body: ClosureBody::Expr(Box::new(right)),
                };
                // Clone the key for entry() because HashMap::entry takes
                // ownership, but the RHS closure may also reference the key
                // (e.g., `cache[key] ??= "prefix:" + key`).
                let key_for_entry = Expr::MethodCall {
                    object: Box::new(*index.clone()),
                    method: "clone".to_string(),
                    args: vec![],
                };
                let entry_call = Expr::MethodCall {
                    object: Box::new(Expr::MethodCall {
                        object: object.clone(),
                        method: "entry".to_string(),
                        args: vec![key_for_entry],
                    }),
                    method: "or_insert_with".to_string(),
                    args: vec![closure],
                };
                return Ok(if is_copy {
                    Expr::Deref(Box::new(entry_call))
                } else {
                    Expr::MethodCall {
                        object: Box::new(entry_call),
                        method: "clone".to_string(),
                        args: vec![],
                    }
                });
            }

            let strategy = pick_strategy(lhs_type);
            match strategy {
                NullishAssignStrategy::ShadowLet => {
                    let is_copy_inner = match lhs_type {
                        RustType::Option(inner) => inner.is_copy_type(),
                        _ => unreachable!("ShadowLet strategy is only picked for Option<T>"),
                    };
                    let right = self.convert_expr(&assign.right)?;
                    let closure = Expr::Closure {
                        params: vec![],
                        return_type: None,
                        body: ClosureBody::Expr(Box::new(right)),
                    };
                    let method_call = Expr::MethodCall {
                        object: Box::new(target),
                        method: "get_or_insert_with".to_string(),
                        args: vec![closure],
                    };

                    return Ok(if is_copy_inner {
                        Expr::Deref(Box::new(method_call))
                    } else {
                        Expr::MethodCall {
                            object: Box::new(method_call),
                            method: "clone".to_string(),
                            args: vec![],
                        }
                    });
                }
                NullishAssignStrategy::Identity => {
                    // `??=` on a non-nullable LHS is dead: the assign branch
                    // never fires and (D-3) the RHS is *intentionally* not
                    // converted so its side effects don't leak into IR. Emit
                    // the identity (with `.clone()` when `T: !Copy` so the
                    // expression yields an owned value rather than moving out
                    // of `ident`, matching the prior `.clone()` suffix of the
                    // ShadowLet path).
                    //
                    // INTERIM (I-048): the unconditional `.clone()` is
                    // conservative — an allocating copy is emitted even when
                    // the surrounding flow doesn't use `ident` again and a
                    // move would suffice. A precise move-vs-clone decision
                    // requires the ownership-inference umbrella (I-048); until
                    // it lands, we clone to keep the emission compile-safe.
                    let is_copy = lhs_type.is_copy_type();
                    return Ok(if is_copy {
                        target
                    } else {
                        Expr::MethodCall {
                            object: Box::new(target),
                            method: "clone".to_string(),
                            args: vec![],
                        }
                    });
                }
                NullishAssignStrategy::BlockedByI050 => {
                    return Err(UnsupportedSyntaxError::new(
                        "nullish-assign on Any LHS (I-050 Any coercion umbrella)",
                        assign.span(),
                    )
                    .into());
                }
            }
        }

        // I-161: `&&=` / `||=` expression-context desugar.
        //
        // Stmt-context `x &&= y;` is intercepted earlier in `convert_stmt` by
        // `try_convert_compound_logical_assign_stmt` and rewritten to a bare
        // `Stmt::If { cond, then_body: [assign], .. }`. We only get here when
        // the compound logical assign appears inside a larger expression, so
        // we emit a block-expression that performs the conditional assign
        // and yields the current LHS value as the tail.
        //
        // The per-type dispatch lives in `desugar_compound_logical_assign_expr`
        // (see `src/transformer/statements/compound_logical_assign.rs`) so
        // the Problem Space matrix is encoded in exactly one place.
        if matches!(
            assign.op,
            ast::AssignOp::AndAssign | ast::AssignOp::OrAssign
        ) {
            let lhs_type = match &assign.left {
                ast::AssignTarget::Simple(ast::SimpleAssignTarget::Ident(ident)) => self
                    .get_type_for_var(&ident.id.sym, ident.id.span)
                    .cloned()
                    .ok_or_else(|| {
                        UnsupportedSyntaxError::new(
                            "compound logical assign on unresolved ident type",
                            assign.span(),
                        )
                    })?,
                ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(member)) => self
                    .get_expr_type(&ast::Expr::Member(member.clone()))
                    .cloned()
                    .ok_or_else(|| {
                        UnsupportedSyntaxError::new(
                            "compound logical assign on unresolved member type",
                            assign.span(),
                        )
                    })?,
                _ => {
                    return Err(UnsupportedSyntaxError::new(
                        "unsupported compound logical assign target",
                        assign.span(),
                    )
                    .into());
                }
            };
            let right = self.convert_expr(&assign.right)?;
            return self.desugar_compound_logical_assign_expr(
                target,
                right,
                &lhs_type,
                assign.op,
                assign.span(),
            );
        }

        // Non-??= path: for compound assignment (=, +=, -=, *=, /=) and plain
        // `=`, the RHS is always observed. Convert it lazily here so the ??=
        // strategy arms above can skip it when dead.
        let right = self.convert_expr(&assign.right)?;

        // Plain `=` / arithmetic compound desugar (Ident / Computed target only):
        // Member target × plain `=` (T6) は line 33-44 の早期 return、Member target ×
        // arithmetic / bitwise compound (T8) は line 46-66 の早期 return で先 dispatch
        // 済 (`dispatch_member_write` / `dispatch_member_compound` 経由)、本 match arm
        // に到達するのは **Ident target / Computed target** の compound assign のみ。
        // 例: `i += 1;` (Ident) → `Expr::Assign { Ident, BinaryOp { Ident, Add, 1.0 } }`、
        //     `arr[i] += 1;` (Computed) → `Expr::Assign { Index, BinaryOp { Index, Add, 1.0 } }`。
        let value = match assign.op {
            ast::AssignOp::Assign => right,
            ast::AssignOp::AddAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::Add,
                right: Box::new(right),
            },
            ast::AssignOp::SubAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::Sub,
                right: Box::new(right),
            },
            ast::AssignOp::MulAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::Mul,
                right: Box::new(right),
            },
            ast::AssignOp::DivAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::Div,
                right: Box::new(right),
            },
            ast::AssignOp::ModAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::Mod,
                right: Box::new(right),
            },
            ast::AssignOp::BitAndAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::BitAnd,
                right: Box::new(right),
            },
            ast::AssignOp::BitOrAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::BitOr,
                right: Box::new(right),
            },
            ast::AssignOp::BitXorAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::BitXor,
                right: Box::new(right),
            },
            ast::AssignOp::LShiftAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::Shl,
                right: Box::new(right),
            },
            ast::AssignOp::RShiftAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::Shr,
                right: Box::new(right),
            },
            ast::AssignOp::ZeroFillRShiftAssign => Expr::BinaryOp {
                left: Box::new(target.clone()),
                op: BinOp::UShr,
                right: Box::new(right),
            },
            // I-205 T8 Iteration v12 third-review (Rule 11 (d-1) compliance):
            // explicit enumerate of remaining `ast::AssignOp` variants (= 3 ops)
            // で `_ =>` arm 排除、`UnsupportedSyntaxError::new` 経由で user-facing
            // line:col 含む transparent Tier 2 honest error を emit (T8 review F2/F3 fix)。
            //
            // - `AndAssign` / `OrAssign`: I-161 desugar path で line 247-281 で先に
            //   intercept、本 arm は **構造的 unreachable** (= `unreachable!()` macro で
            //   structural enforcement codify)。
            // - `ExpAssign` (`**=`): TS exponentiation compound assign。Member target は
            //   T8 dispatch gate (line 49-65) が `arithmetic_compound_op_to_binop` で
            //   `None` を return するため経由しない、Ident target の `**=` は本 arm で
            //   honest reject (= TS exponentiation conversion 別 architectural concern、
            //   PRD scope 外)。`UnsupportedSyntaxError` で user-facing localization。
            ast::AssignOp::AndAssign | ast::AssignOp::OrAssign => unreachable!(
                "convert_assign_expr compound desugar arm: AndAssign/OrAssign は I-161 \
                 desugar path で先に intercept される (line 247-281)、本 match arm は \
                 構造的 unreachable。op={:?}",
                assign.op
            ),
            ast::AssignOp::ExpAssign => {
                return Err(UnsupportedSyntaxError::new(
                    "exponentiation compound assign (**=) — TS exponentiation conversion \
                     out of I-205 PRD scope",
                    assign.span(),
                )
                .into())
            }
            // NullishAssign (`??=`) は line 142-251 で全 target shape (Ident / Member /
            // Computed) が intercept されるため、本 match arm は構造的 unreachable。
            // (Plain `=` は本 match の最初の arm `Assign => right` で legitimately handle
            // される: plain `=` × Ident target / Computed target が本 path に流れる、
            // T6 Member target gate は assignments.rs 早期 return で先に dispatch される。)
            ast::AssignOp::NullishAssign => unreachable!(
                "convert_assign_expr compound desugar arm: NullishAssign は line 142-251 で \
                 全 target shape が intercept される、本 match arm は構造的 unreachable。\
                 op={:?}",
                assign.op
            ),
        };
        Ok(Expr::Assign {
            target: Box::new(target),
            value: Box::new(value),
        })
    }
}

impl<'a> Transformer<'a> {
    /// Converts an update expression (`i++`, `i--`, `++i`, `--i`,
    /// `obj.x++`, `Class.x--`, etc.) to an `Expr::Block` that yields the
    /// expected old-value (postfix) or new-value (prefix) per ECMAScript spec.
    ///
    /// ## Dispatch by argument shape
    ///
    /// - **`Ident`** (`i++`): direct binding update
    ///   - Postfix `i++` → `{ let __ts_old = i; i = i + 1.0; __ts_old }`
    ///   - Prefix `++i` → `{ i = i + 1.0; i }`
    ///
    /// - **`Member` with `Ident`/`PrivateName` prop** (`obj.x++` / `Class.x++`):
    ///   class member dispatch via [`Self::classify_member_receiver`] (T6
    ///   shared classifier). Routes through [`dispatch_instance_member_update`]
    ///   / [`dispatch_static_member_update`] (T7) for class accessor receivers,
    ///   else falls back to `Expr::Assign { FieldAccess, BinaryOp }` for B1
    ///   field / B9 unknown / non-class receivers (cells 42, 45-a, 45-de).
    ///
    /// - **Other shapes** (`Computed obj[i]++`, complex receivers): existing
    ///   `unsupported update expression target` error path (matrix scope 外、
    ///   I-203 codebase-wide AST exhaustiveness compliance で別 PRD 取り扱い)。
    ///
    /// ## Postfix old-value preservation invariant (matrix cells 42-45)
    ///
    /// For both Member dispatch (B4 setter desugar) and Member fallback (B1
    /// field), postfix yields the **old** value while still performing the
    /// mutation; prefix yields the **new** value. The block expression is
    /// transparent in statement context (`obj.x++;` discards the tail expr).
    ///
    /// ## Variable namespace hygiene (I-154 + T7 extension)
    ///
    /// All emission bindings use the `__ts_` prefix per the [I-154 namespace
    /// reservation rule](crate::transformer::statements). T7 extends the
    /// `__ts_` namespace from labels (I-154 scope) to value bindings; the
    /// prior single-underscore `_old` (Ident form pre-T7) is renamed in this
    /// task as cohesive cleanup since T7 introduces new uses of the same
    /// emission pattern.
    pub(crate) fn convert_update_expr(&mut self, up: &ast::UpdateExpr) -> Result<Expr> {
        let op = match up.op {
            ast::UpdateOp::PlusPlus => BinOp::Add,
            ast::UpdateOp::MinusMinus => BinOp::Sub,
        };
        let is_postfix = !up.prefix;

        // SWC `ast::Expr` 全 38 variants を **exhaustive enumerate** (Rule 11 (d-1)
        // `_ => ` arm 全面禁止 compliant、`#[non_exhaustive]` ではないため Rust compiler
        // が新 variant 追加時 compile error で全 dispatch fix 強制 = structural enforcement)。
        // 本 dispatch arms:
        // - `Member` → class member dispatch (Static/Instance/Fallback) per T7 cells 42-45
        // - `Ident` → direct binding update (Ident form、I-154 + T7 extension to value
        //   bindings の `TS_OLD_BINDING` 経由)
        // - その他 36 variants → Tier 2 honest error per Rule 11 (d-2) Transformer phase
        //   mechanism (`UnsupportedSyntaxError` で user-facing line:col 含む transparent
        //   error reporting via `resolve_unsupported()` 経路)
        match up.arg.as_ref() {
            ast::Expr::Member(member) => {
                self.convert_update_expr_member_arm(member, op, is_postfix, up.span)
            }
            ast::Expr::Ident(ident) => {
                let name = ident.sym.to_string();
                let assign = Stmt::Expr(Expr::Assign {
                    target: Box::new(Expr::Ident(name.clone())),
                    value: Box::new(Expr::BinaryOp {
                        left: Box::new(Expr::Ident(name.clone())),
                        op,
                        right: Box::new(Expr::NumberLit(1.0)),
                    }),
                });
                if up.prefix {
                    // Prefix: ++i → { i = i + 1.0; i }
                    Ok(Expr::Block(vec![assign, Stmt::TailExpr(Expr::Ident(name))]))
                } else {
                    // Postfix: i++ → `{ let __ts_old = i; i = i + 1.0; __ts_old }`
                    // (binding name = `TS_OLD_BINDING`、I-154 namespace reservation
                    // extension to value bindings)
                    Ok(Expr::Block(vec![
                        Stmt::Let {
                            mutable: false,
                            name: TS_OLD_BINDING.to_string(),
                            ty: None,
                            init: Some(Expr::Ident(name)),
                        },
                        assign,
                        Stmt::TailExpr(Expr::Ident(TS_OLD_BINDING.to_string())),
                    ]))
                }
            }
            // Unsupported `ast::Expr` variants as UpdateExpr target (TS-invalid or
            // matrix scope-out)。Rule 11 (d-1) compliance per `or_pattern` で 36 variants
            // を集約 enumerate (Rule 11 (d-2) Transformer phase = `UnsupportedSyntaxError`
            // で line:col 含む user-facing error)。
            //
            // Categories (informational):
            // - Literal / wrapper shapes (TS-valid syntax で UpdateExpr target が type
            //   error or matrix scope-out): `This`, `Lit`, `Paren`, `TsAs`, `TsNonNull`,
            //   `TsTypeAssertion`, `TsConstAssertion`, `TsInstantiation`, `TsSatisfies`,
            //   `Seq`, `Arrow`, `Fn`, `Class`, `Tpl`, `TaggedTpl`, `Array`, `Object`,
            //   `Cond`
            // - Compound / call / chain shapes (TS で type error な UpdateExpr target):
            //   `Bin`, `Unary`, `Update`, `Assign`, `Call`, `New`, `MetaProp`, `Yield`,
            //   `Await`, `SuperProp`, `OptChain`, `PrivateName`
            // - Special / non-target shapes: `Invalid` (parser error), `JSXMember`,
            //   `JSXNamespacedName`, `JSXEmpty`, `JSXElement`, `JSXFragment`
            //   (JSX = TypeScript JSX/TSX context、UpdateExpr target にはならない)
            ast::Expr::This(_)
            | ast::Expr::Array(_)
            | ast::Expr::Object(_)
            | ast::Expr::Fn(_)
            | ast::Expr::Unary(_)
            | ast::Expr::Update(_)
            | ast::Expr::Bin(_)
            | ast::Expr::Assign(_)
            | ast::Expr::SuperProp(_)
            | ast::Expr::Cond(_)
            | ast::Expr::Call(_)
            | ast::Expr::New(_)
            | ast::Expr::Seq(_)
            | ast::Expr::Lit(_)
            | ast::Expr::Tpl(_)
            | ast::Expr::TaggedTpl(_)
            | ast::Expr::Arrow(_)
            | ast::Expr::Class(_)
            | ast::Expr::Yield(_)
            | ast::Expr::MetaProp(_)
            | ast::Expr::Await(_)
            | ast::Expr::Paren(_)
            | ast::Expr::JSXMember(_)
            | ast::Expr::JSXNamespacedName(_)
            | ast::Expr::JSXEmpty(_)
            | ast::Expr::JSXElement(_)
            | ast::Expr::JSXFragment(_)
            | ast::Expr::TsTypeAssertion(_)
            | ast::Expr::TsConstAssertion(_)
            | ast::Expr::TsNonNull(_)
            | ast::Expr::TsAs(_)
            | ast::Expr::TsInstantiation(_)
            | ast::Expr::TsSatisfies(_)
            | ast::Expr::PrivateName(_)
            | ast::Expr::OptChain(_)
            | ast::Expr::Invalid(_) => Err(UnsupportedSyntaxError::new(
                "unsupported update expression target",
                up.span,
            )
            .into()),
        }
    }

    /// Member target arm of [`Self::convert_update_expr`] (T7 cells 42-45).
    ///
    /// Splits on [`MemberReceiverClassification`]:
    /// - `Static` → [`dispatch_static_member_update`] (B8 setter desugar /
    ///   defensive Tier 2 for B2/B3/B6/B7 static)
    /// - `Instance` → [`dispatch_instance_member_update`] (B4 setter desugar
    ///   with numeric type check / Tier 2 for B2/B3/B6/B7)
    /// - `Fallback` → direct `Expr::Assign { FieldAccess, BinaryOp }` block
    ///   with postfix/prefix old-/new-value preservation (cells 42, 45-a,
    ///   45-de = B1 field + B9 unknown regression Tier 2 → Tier 1 transition)
    ///
    /// `MemberProp` shape handling (本 arm 入口の field name extraction):
    /// - `MemberProp::Ident` / `MemberProp::PrivateName`:
    ///   `extract_non_computed_field_name` が `Some(field_name)` を返し、
    ///   `classify_member_receiver` 経由 dispatch に進む (`dispatch_member_write`
    ///   と symmetric な class member dispatch)。
    /// - `MemberProp::Computed` (`obj[i]++`):
    ///   `extract_non_computed_field_name` が `None` を返し、本 arm 入口で
    ///   `unsupported update expression target` anyhow Err を直接 return
    ///   (= outer `convert_update_expr` の non-Ident reject path と **同一 wording**、
    ///   Computed update は matrix scope 外 / 別 architectural concern として
    ///   I-203 codebase-wide AST exhaustiveness で取り扱う)。本 reject は
    ///   `MemberReceiverClassification::Fallback` ではなく **early return**、
    ///   classify_member_receiver は呼ばれない。
    fn convert_update_expr_member_arm(
        &mut self,
        member: &ast::MemberExpr,
        op: BinOp,
        is_postfix: bool,
        up_span: swc_common::Span,
    ) -> Result<Expr> {
        // `MemberProp` shape gate:
        // - `Ident` / `PrivateName` → `Some(field)` で classify_member_receiver dispatch
        // - `Computed` → `None` で early return (matrix scope 外、`convert_update_expr`
        //   の non-Ident reject wording と統一)。Transformer phase mechanism per Rule 11
        //   (d-2) で `UnsupportedSyntaxError` 必須 (`up_span` で whole UpdateExpr context
        //   の line:col を user-facing error に含む、Computed update target の precise
        //   localization)。
        let Some(field) = extract_non_computed_field_name(&member.prop) else {
            return Err(UnsupportedSyntaxError::new(
                "unsupported update expression target",
                up_span,
            )
            .into());
        };

        match self.classify_member_receiver(&member.obj, &field) {
            MemberReceiverClassification::Static {
                class_name,
                sigs,
                is_inherited,
            } => dispatch_static_member_update(
                &class_name,
                &field,
                &sigs,
                is_inherited,
                op,
                is_postfix,
                &member.obj,
            ),
            MemberReceiverClassification::Instance { sigs, is_inherited } => {
                let object = self.convert_expr(&member.obj)?;
                dispatch_instance_member_update(
                    &object,
                    &field,
                    &sigs,
                    is_inherited,
                    op,
                    is_postfix,
                    &member.obj,
                )
            }
            MemberReceiverClassification::Fallback => {
                // B1 field / B9 unknown / non-class receiver / static field:
                // direct `obj.x = obj.x OP 1.0` block with postfix/prefix
                // old-/new-value preservation (cells 42, 45-a, 45-de regression
                // Tier 2 → Tier 1 transition for B1/B9 from current-broken state
                // where `convert_update_expr` rejected all Member targets).
                let object = self.convert_expr(&member.obj)?;
                Ok(build_fallback_field_update_block(
                    &object, &field, op, is_postfix,
                ))
            }
        }
    }
}

/// Builds the direct field update block for `obj.x++` / `obj.x--` Fallback
/// (B1 field、B9 unknown、non-class receiver、static field) — used by T7
/// `convert_update_expr_member_arm` when no class member dispatch fires.
///
/// Postfix `obj.x++`:
/// ```text
/// { let __ts_old = obj.x; obj.x = __ts_old <op> 1.0; __ts_old }
/// ```
///
/// Prefix `++obj.x`:
/// ```text
/// { obj.x = obj.x <op> 1.0; obj.x }
/// ```
///
/// Symmetric with the Ident-target form in [`Transformer::convert_update_expr`]
/// — postfix uses `__ts_old` let binding for old-value preservation, prefix
/// re-reads `obj.x` for the tail expr.
fn build_fallback_field_update_block(
    object: &Expr,
    field: &str,
    op: BinOp,
    is_postfix: bool,
) -> Expr {
    let field_access = Expr::FieldAccess {
        object: Box::new(object.clone()),
        field: field.to_string(),
    };
    let assign = Stmt::Expr(Expr::Assign {
        target: Box::new(field_access.clone()),
        value: Box::new(Expr::BinaryOp {
            left: Box::new(if is_postfix {
                Expr::Ident(TS_OLD_BINDING.to_string())
            } else {
                field_access.clone()
            }),
            op,
            right: Box::new(Expr::NumberLit(1.0)),
        }),
    });
    if is_postfix {
        Expr::Block(vec![
            Stmt::Let {
                mutable: false,
                name: TS_OLD_BINDING.to_string(),
                ty: None,
                init: Some(field_access),
            },
            assign,
            Stmt::TailExpr(Expr::Ident(TS_OLD_BINDING.to_string())),
        ])
    } else {
        Expr::Block(vec![assign, Stmt::TailExpr(field_access)])
    }
}
