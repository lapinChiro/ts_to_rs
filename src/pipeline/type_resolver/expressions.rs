//! Expression type resolution for TypeResolver.
//!
//! Resolves types for all expression forms: literals, binary ops, member access,
//! function calls, arrow functions, object/array literals, optional chaining, etc.

use swc_common::Spanned;
use swc_ecma_ast as ast;

use super::*;
use crate::pipeline::type_converter::convert_ts_type;
use crate::pipeline::type_resolution::Span;
use crate::registry::select_overload;
use crate::transformer::type_position::{wrap_trait_for_position, TypePosition};

impl<'a> TypeResolver<'a> {
    /// 式の型を解決し、Known な結果を `expr_types` に記録する。
    ///
    /// 全ての部分式の型が `expr_types` に蓄積されるため、Transformer は
    /// `get_expr_type(tctx, expr)` だけで任意の式の型を取得できる。
    pub(super) fn resolve_expr(&mut self, expr: &ast::Expr) -> ResolvedType {
        let ty = self.resolve_expr_inner(expr);
        if matches!(ty, ResolvedType::Known(_)) {
            let span = Span::from_swc(expr.span());
            self.result
                .expr_types
                .entry(span)
                .or_insert_with(|| ty.clone());
        }
        ty
    }

    fn resolve_expr_inner(&mut self, expr: &ast::Expr) -> ResolvedType {
        match expr {
            ast::Expr::Ident(ident) => self.lookup_var(ident.sym.as_ref()),
            ast::Expr::Lit(ast::Lit::Str(_)) => ResolvedType::Known(RustType::String),
            ast::Expr::Lit(ast::Lit::Num(_)) => ResolvedType::Known(RustType::F64),
            ast::Expr::Lit(ast::Lit::Bool(_)) => ResolvedType::Known(RustType::Bool),
            ast::Expr::Lit(ast::Lit::Null(_)) => {
                ResolvedType::Known(RustType::Option(Box::new(RustType::Any)))
            }
            ast::Expr::Tpl(tpl) => {
                // Template literal: recursively resolve each interpolated expression
                // so `expr_types` contains entries for inner sub-expressions.
                // Without this, downstream lookups (e.g. `is_du_field_binding`
                // checking `get_expr_type(&event)` inside `` `${event.x}` ``)
                // return Unknown and fall through to raw member access emission.
                for expr in &tpl.exprs {
                    self.resolve_expr(expr);
                }
                ResolvedType::Known(RustType::String)
            }
            ast::Expr::TaggedTpl(tagged) => {
                // Tagged template: recurse into tag and interpolated exprs for
                // consistency with Tpl. The tag's return type is not analyzed
                // here (converter treats TaggedTpl as unsupported — I-110), so
                // the overall type is Unknown.
                self.resolve_expr(&tagged.tag);
                for expr in &tagged.tpl.exprs {
                    self.resolve_expr(expr);
                }
                ResolvedType::Unknown
            }
            ast::Expr::Bin(bin) => self.resolve_bin_expr(bin),
            ast::Expr::Member(member) => self.resolve_member_expr(member),
            ast::Expr::Call(call) => self.resolve_call_expr(call),
            ast::Expr::New(new_expr) => self.resolve_new_expr(new_expr),
            ast::Expr::Paren(paren) => self.resolve_expr(&paren.expr),
            ast::Expr::TsAs(ts_as) => {
                // Propagate `as T` type to the inner expression as expected type.
                // This allows `{...x} as SomeType` to resolve the object literal
                // with SomeType as expected, enabling struct name resolution.
                let as_type = convert_ts_type(&ts_as.type_ann, self.synthetic, self.registry).ok();
                if let Some(ref ty) = as_type {
                    let expr_span = Span::from_swc(ts_as.expr.span());
                    self.result.expected_types.insert(expr_span, ty.clone());
                    self.propagate_expected(&ts_as.expr, ty);
                }
                // Resolve inner expression to register its type and trigger nested
                // call resolution (e.g., `foo(bar(x) as T)` needs bar's args typed).
                self.resolve_expr(&ts_as.expr);
                as_type
                    .map(|ty| {
                        let wrapped =
                            wrap_trait_for_position(ty, TypePosition::Value, self.registry);
                        ResolvedType::Known(wrapped)
                    })
                    .unwrap_or(ResolvedType::Unknown)
            }
            ast::Expr::Array(arr) => self.resolve_array_expr(arr),
            ast::Expr::Arrow(arrow) => self.resolve_arrow_expr(arrow),
            ast::Expr::Fn(fn_expr) => self.resolve_fn_expr(fn_expr),
            ast::Expr::Assign(assign) => {
                // Propagate LHS type as expected on RHS (only for plain `=`, not `+=`/`-=` etc.)
                if assign.op == ast::AssignOp::Assign {
                    if let Some(simple) = assign.left.as_simple() {
                        let lhs_type = match simple {
                            ast::SimpleAssignTarget::Ident(ident) => {
                                self.mark_var_mutable(ident.id.sym.as_ref());
                                // I-142: record LHS type at the ident's span so
                                // downstream ??= handling in the Transformer can
                                // read it. Plain `=` also benefits from this for
                                // consistency (assign-target idents previously
                                // had no expr_types entry even for plain `=`).
                                self.record_assign_target_ident_type(ident)
                            }
                            ast::SimpleAssignTarget::Member(member) => {
                                // Mark the object variable as mutable
                                if let ast::Expr::Ident(ident) = member.obj.as_ref() {
                                    self.mark_var_mutable(ident.sym.as_ref());
                                }
                                let obj_type = self.resolve_expr(&member.obj);
                                if let ResolvedType::Known(ref ty) = obj_type {
                                    self.resolve_member_type(ty, &member.prop)
                                } else {
                                    ResolvedType::Unknown
                                }
                            }
                            _ => ResolvedType::Unknown,
                        };
                        if let ResolvedType::Known(ref ty) = lhs_type {
                            let rhs_span = Span::from_swc(assign.right.span());
                            self.result.expected_types.insert(rhs_span, ty.clone());
                            self.propagate_expected(&assign.right, ty);
                        }
                    }
                } else {
                    // Compound assignments (+=, -=, ??=, etc.) mark the target
                    // mutable. `??=` (I-142) additionally needs the LHS type
                    // recorded at the ident's span and inner-T expected-type
                    // propagation onto the RHS — other compound ops (`+=`,
                    // `-=`, …) do not read the LHS type from expr_types, so
                    // we leave their historical no-op behavior untouched to
                    // avoid rippling expected-type side effects through
                    // unrelated code paths.
                    match assign.left.as_simple() {
                        Some(ast::SimpleAssignTarget::Ident(ident)) => {
                            self.mark_var_mutable(ident.id.sym.as_ref());
                            if assign.op == ast::AssignOp::NullishAssign {
                                let lhs_type = self.record_assign_target_ident_type(ident);
                                if let ResolvedType::Known(RustType::Option(inner)) = &lhs_type {
                                    let rhs_span = Span::from_swc(assign.right.span());
                                    self.result
                                        .expected_types
                                        .insert(rhs_span, (**inner).clone());
                                    self.propagate_expected(&assign.right, inner);
                                }
                            }
                        }
                        Some(ast::SimpleAssignTarget::Member(member)) => {
                            if let ast::Expr::Ident(ident) = member.obj.as_ref() {
                                self.mark_var_mutable(ident.sym.as_ref());
                            }
                            // I-142-b/c: resolve field/index type for ??= so
                            // the Transformer can read it via get_expr_type and
                            // dispatch on pick_strategy.
                            if assign.op == ast::AssignOp::NullishAssign {
                                let obj_type = self.resolve_expr(&member.obj);
                                if let ResolvedType::Known(ref ty) = obj_type {
                                    // For named fields: use resolve_member_type.
                                    // For computed index (HashMap): extract value type.
                                    let field_type = match &member.prop {
                                        ast::MemberProp::Computed(_) => {
                                            // HashMap<K, V> → value type is V
                                            match ty {
                                                RustType::StdCollection {
                                                    kind: crate::ir::StdCollectionKind::HashMap,
                                                    args,
                                                } if args.len() == 2 => {
                                                    ResolvedType::Known(args[1].clone())
                                                }
                                                _ => self.resolve_member_type(ty, &member.prop),
                                            }
                                        }
                                        _ => self.resolve_member_type(ty, &member.prop),
                                    };
                                    if let ResolvedType::Known(ref ft) = field_type {
                                        let member_span = Span::from_swc(member.span());
                                        self.result
                                            .expr_types
                                            .insert(member_span, ResolvedType::Known(ft.clone()));
                                        if let RustType::Option(inner) = ft {
                                            let rhs_span = Span::from_swc(assign.right.span());
                                            self.result
                                                .expected_types
                                                .insert(rhs_span, (**inner).clone());
                                            self.propagate_expected(&assign.right, inner);
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                self.resolve_expr(&assign.right)
            }
            ast::Expr::Cond(cond) => {
                // Ternary: resolve test and both branches.
                // Test must be resolved so sub-expression types (e.g., variable Idents
                // in `x !== null`) are available in expr_types for NarrowingGuard lookup.
                self.resolve_expr(&cond.test);
                // If either branch is null/undefined or already Option<T>,
                // the result is Option<T>.
                let cons = self.resolve_expr(&cond.cons);
                let alt = self.resolve_expr(&cond.alt);

                let cons_is_null =
                    crate::pipeline::narrowing_patterns::is_null_or_undefined(&cond.cons);
                let alt_is_null =
                    crate::pipeline::narrowing_patterns::is_null_or_undefined(&cond.alt);
                let cons_is_option = matches!(&cons, ResolvedType::Known(RustType::Option(_)));
                let alt_is_option = matches!(&alt, ResolvedType::Known(RustType::Option(_)));

                let produces_option =
                    cons_is_null || alt_is_null || cons_is_option || alt_is_option;

                if produces_option {
                    // Pick the non-null branch's type as the value type. `wrap_optional`
                    // is idempotent so an already-Option branch stays single-wrapped.
                    let value_type = if cons_is_null { &alt } else { &cons };
                    match value_type {
                        ResolvedType::Known(ty) => ResolvedType::Known(ty.clone().wrap_optional()),
                        ResolvedType::Unknown => ResolvedType::Known(RustType::Any.wrap_optional()),
                    }
                } else {
                    match (&cons, &alt) {
                        // Both known and same type → return that type
                        (ResolvedType::Known(c), ResolvedType::Known(a)) if c == a => cons,
                        // Both known but different types → generate union
                        (ResolvedType::Known(c), ResolvedType::Known(a)) => {
                            let union_types = vec![c.clone(), a.clone()];
                            let enum_name = self.synthetic.register_union(&union_types);
                            ResolvedType::Known(RustType::Named {
                                name: enum_name,
                                type_args: vec![],
                            })
                        }
                        // One unknown → prefer the known one
                        (ResolvedType::Known(_), ResolvedType::Unknown) => cons,
                        (ResolvedType::Unknown, ResolvedType::Known(_)) => alt,
                        // Both unknown
                        _ => ResolvedType::Unknown,
                    }
                }
            }
            ast::Expr::Unary(unary) => {
                // Resolve operand to register its expr_type (used by Transformer
                // for typeof/unary plus operand type decisions)
                self.resolve_expr(&unary.arg);
                match unary.op {
                    ast::UnaryOp::TypeOf => ResolvedType::Known(RustType::String),
                    ast::UnaryOp::Bang => ResolvedType::Known(RustType::Bool),
                    ast::UnaryOp::Minus | ast::UnaryOp::Plus => ResolvedType::Known(RustType::F64),
                    _ => ResolvedType::Unknown,
                }
            }
            ast::Expr::Await(await_expr) => self.resolve_expr(&await_expr.arg),
            ast::Expr::TsNonNull(ts_non_null) => {
                // x! (non-null assertion) — unwrap Option, return inner type
                let inner = self.resolve_expr(&ts_non_null.expr);
                match inner {
                    ResolvedType::Known(RustType::Option(inner_ty)) => {
                        ResolvedType::Known(*inner_ty)
                    }
                    other => other, // Not Option — return as-is
                }
            }
            ast::Expr::TsTypeAssertion(assertion) => {
                // <T>x — same as TsAs: propagate T as expected type to inner expression
                let as_type =
                    convert_ts_type(&assertion.type_ann, self.synthetic, self.registry).ok();
                if let Some(ref ty) = as_type {
                    let expr_span = Span::from_swc(assertion.expr.span());
                    self.result.expected_types.insert(expr_span, ty.clone());
                    self.propagate_expected(&assertion.expr, ty);
                }
                self.resolve_expr(&assertion.expr);
                as_type
                    .map(|ty| {
                        let wrapped =
                            wrap_trait_for_position(ty, TypePosition::Value, self.registry);
                        ResolvedType::Known(wrapped)
                    })
                    .unwrap_or(ResolvedType::Unknown)
            }
            ast::Expr::TsConstAssertion(const_assertion) => {
                // x as const — return inner expression's type
                self.resolve_expr(&const_assertion.expr)
            }
            ast::Expr::Object(obj) => {
                // Walk property values to resolve their types and collect field info.
                // For spread sources, resolve their types and extract fields from
                // TypeRegistry to build a complete field list.
                let mut explicit_fields: Vec<(String, RustType)> = Vec::new();
                let mut spread_types: Vec<RustType> = Vec::new();
                // Track the total number of non-spread properties to detect partial
                // resolution (some fields resolved, some didn't). We must not generate
                // an anonymous struct with missing fields — that would silently drop them.
                let mut total_explicit_props = 0u32;

                for prop in &obj.props {
                    match prop {
                        ast::PropOrSpread::Prop(prop) => match prop.as_ref() {
                            ast::Prop::KeyValue(kv) => {
                                total_explicit_props += 1;
                                let span = Span::from_swc(kv.value.span());
                                let ty = self.resolve_expr(&kv.value);
                                self.result.expr_types.insert(span, ty.clone());

                                let key = extract_prop_name(&kv.key);
                                if let (Some(key), ResolvedType::Known(rust_ty)) = (key, ty) {
                                    explicit_fields.push((key, rust_ty));
                                }
                            }
                            ast::Prop::Shorthand(ident) => {
                                total_explicit_props += 1;
                                let span = Span::from_swc(ident.span);
                                let name = ident.sym.to_string();
                                let ty = self.lookup_var(&name);
                                self.result.expr_types.insert(span, ty.clone());

                                if let ResolvedType::Known(rust_ty) = ty {
                                    explicit_fields.push((name, rust_ty));
                                }
                            }
                            _ => {
                                total_explicit_props += 1;
                            }
                        },
                        ast::PropOrSpread::Spread(spread) => {
                            let span = Span::from_swc(spread.expr.span());
                            let ty = self.resolve_expr(&spread.expr);
                            self.result.expr_types.insert(span, ty.clone());
                            if let ResolvedType::Known(rust_ty) = ty {
                                spread_types.push(rust_ty);
                            }
                        }
                    }
                }

                let obj_span = Span::from_swc(obj.span);

                // Store resolved spread fields for Transformer's spread expansion.
                // Must be done before the early return for pre-set expected types,
                // because the Transformer needs field names/types to convert `...spread`
                // into individual `field: spread.field` accesses regardless of how the
                // expected type was determined.
                if !spread_types.is_empty() {
                    if let Some(fields) = self.merge_object_fields(&spread_types, &explicit_fields)
                    {
                        self.result.spread_fields.insert(obj_span, fields);
                    }
                }

                if self.result.expected_types.contains_key(&obj_span) {
                    // Expected type already set (from annotation, return type, etc.)
                    // — skip anonymous struct generation
                    return ResolvedType::Unknown;
                }

                // Abort if any explicit field's type couldn't be resolved.
                // Generating an anonymous struct with missing fields would cause confusing
                // Rust compile errors (unknown field) rather than the clear "requires type
                // annotation" error from the Transformer.
                if explicit_fields.len() != total_explicit_props as usize {
                    return ResolvedType::Unknown;
                }

                // Build merged field list: spread source fields + explicit fields.
                // When spreads exist, use the pre-stored spread_fields (computed above).
                // When no spreads, merge from explicit fields only.
                let merged = if !spread_types.is_empty() {
                    match self.result.spread_fields.get(&obj_span).cloned() {
                        Some(fields) if !fields.is_empty() => fields,
                        _ => return ResolvedType::Unknown,
                    }
                } else {
                    match self.merge_object_fields(&[], &explicit_fields) {
                        Some(fields) if !fields.is_empty() => fields,
                        _ => return ResolvedType::Unknown,
                    }
                };

                // Determine the expected type:
                // - If all spread sources are the same Named type (including type_args)
                //   and no extra explicit fields, use that type directly.
                // - Otherwise, generate an anonymous struct from the merged fields.
                let expected_ty = if explicit_fields.is_empty() && !spread_types.is_empty() {
                    if let Some(common_type) = common_named_type(&spread_types) {
                        common_type
                    } else {
                        let name = self.synthetic.register_inline_struct(&merged);
                        RustType::Named {
                            name,
                            type_args: vec![],
                        }
                    }
                } else {
                    let name = self.synthetic.register_inline_struct(&merged);
                    RustType::Named {
                        name,
                        type_args: vec![],
                    }
                };
                self.result
                    .expected_types
                    .insert(obj_span, expected_ty.clone());
                ResolvedType::Known(expected_ty)
            }
            ast::Expr::OptChain(opt) => {
                // Optional chaining: x?.y or x?.method(args)
                // Result is always Option<T> where T is the resolved member/call type.
                let inner_result = match &*opt.base {
                    ast::OptChainBase::Member(member) => {
                        let obj_type = self.resolve_expr(&member.obj);
                        // Unwrap Option<T> → T for field lookup
                        let unwrapped = match &obj_type {
                            ResolvedType::Known(RustType::Option(inner)) => {
                                ResolvedType::Known(inner.as_ref().clone())
                            }
                            other => other.clone(),
                        };
                        // Resolve field type using the same logic as resolve_member_expr
                        match &unwrapped {
                            ResolvedType::Known(ty) => self.resolve_member_type(ty, &member.prop),
                            _ => ResolvedType::Unknown,
                        }
                    }
                    ast::OptChainBase::Call(opt_call) => {
                        // x?.method(args) — the callee may itself be an OptChain
                        // wrapping a Member expression. Unwrap to extract the
                        // actual Member for method param lookup.
                        let effective_member = match opt_call.callee.as_ref() {
                            ast::Expr::OptChain(inner_opt) => {
                                if let ast::OptChainBase::Member(m) = &*inner_opt.base {
                                    Some(m)
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        };
                        let call_return_type = if let Some(member) = effective_member {
                            let obj_type = self.resolve_expr(&member.obj);
                            if let ResolvedType::Known(ref rust_ty) = obj_type {
                                let inner_ty = match rust_ty {
                                    RustType::Option(inner) => inner.as_ref(),
                                    other => other,
                                };
                                let method_name = match &member.prop {
                                    ast::MemberProp::Ident(ident) => Some(ident.sym.to_string()),
                                    _ => None,
                                };
                                let n_args = opt_call.args.len();
                                // Set expected types for args
                                if let Some((param_types, has_rest)) =
                                    method_name.as_deref().and_then(|name| {
                                        self.lookup_method_params(inner_ty, name, n_args, &[])
                                    })
                                {
                                    self.propagate_call_arg_expected_types(
                                        &opt_call.args,
                                        &param_types,
                                        has_rest,
                                    );
                                }
                                // Collect resolved arg types for overload resolution
                                let arg_types = self.collect_resolved_arg_types(&opt_call.args);
                                // Resolve return type
                                method_name
                                    .as_deref()
                                    .map(|name| {
                                        self.resolve_method_return_type(
                                            inner_ty, name, n_args, &arg_types,
                                        )
                                    })
                                    .unwrap_or(ResolvedType::Unknown)
                            } else {
                                ResolvedType::Unknown
                            }
                        } else {
                            self.set_call_arg_expected_types(&opt_call.callee, &opt_call.args);
                            ResolvedType::Unknown
                        };
                        // Walk callee for side effects
                        let callee_span = Span::from_swc(opt_call.callee.span());
                        let callee_ty = self.resolve_expr(&opt_call.callee);
                        self.result.expr_types.insert(callee_span, callee_ty);
                        // Resolve all argument expressions
                        for arg in &opt_call.args {
                            self.resolve_expr(&arg.expr);
                        }
                        call_return_type
                    }
                };
                // Wrap in Option<T>. OptChain always produces an optional result.
                // `wrap_optional` is idempotent: an already-Option inner stays single-wrapped.
                match inner_result {
                    ResolvedType::Known(ty) => ResolvedType::Known(ty.wrap_optional()),
                    ResolvedType::Unknown => ResolvedType::Known(RustType::Any.wrap_optional()),
                }
            }
            ast::Expr::Update(_) => {
                // i++ / i-- → f64
                ResolvedType::Known(RustType::F64)
            }
            ast::Expr::This(_) => {
                // `this` — resolve from scope (registered by visit_class_decl)
                self.lookup_var("this")
            }
            ast::Expr::Seq(seq) => {
                // Comma expression: evaluate all, return last
                let mut last = ResolvedType::Unknown;
                for expr in &seq.exprs {
                    let span = Span::from_swc(expr.span());
                    let ty = self.resolve_expr(expr);
                    self.result.expr_types.insert(span, ty.clone());
                    last = ty;
                }
                last
            }
            ast::Expr::Class(class_expr) => {
                // Class expression: `const C = class Foo { ... }` or `const C = class { ... }`
                let class_name = class_expr
                    .ident
                    .as_ref()
                    .map(|id| id.sym.to_string())
                    .unwrap_or_default();
                let class_span = class_expr
                    .ident
                    .as_ref()
                    .map(|id| Span::from_swc(id.span))
                    .unwrap_or_else(|| Span::from_swc(class_expr.class.span));
                self.visit_class_body(&class_expr.class, &class_name, class_span);
                ResolvedType::Unknown
            }
            _ => ResolvedType::Unknown,
        }
    }

    fn resolve_bin_expr(&mut self, bin: &ast::BinExpr) -> ResolvedType {
        use ast::BinaryOp::*;
        match bin.op {
            Lt | LtEq | Gt | GtEq | EqEq | NotEq | EqEqEq | NotEqEq | In | InstanceOf => {
                // Resolve operands to register their expr_types (used by Transformer
                // for typeof comparison, enum string comparison, in operator, etc.)
                self.resolve_expr(&bin.left);
                self.resolve_expr(&bin.right);
                ResolvedType::Known(RustType::Bool)
            }
            Add => {
                let left_ty = self.resolve_expr(&bin.left);
                let right_ty = self.resolve_expr(&bin.right);
                if matches!(left_ty, ResolvedType::Known(RustType::String))
                    || matches!(right_ty, ResolvedType::Known(RustType::String))
                {
                    ResolvedType::Known(RustType::String)
                } else {
                    ResolvedType::Known(RustType::F64)
                }
            }
            Sub | Mul | Div | Mod | Exp | BitAnd | BitOr | BitXor | LShift | RShift
            | ZeroFillRShift => {
                self.resolve_expr(&bin.left);
                self.resolve_expr(&bin.right);
                ResolvedType::Known(RustType::F64)
            }
            LogicalAnd => {
                // Both sides must be resolved to register all sub-expression types
                // (e.g., `typeof x === "string" && typeof y === "number"` needs both
                // x and y registered in expr_types for narrowing guard resolution)
                let left = self.resolve_expr(&bin.left);
                let right = self.resolve_expr(&bin.right);
                if !matches!(right, ResolvedType::Unknown) {
                    right
                } else {
                    left
                }
            }
            LogicalOr => {
                let left = self.resolve_expr(&bin.left);
                // `x || {}` — propagate left operand's type to fallback object literal
                if let ResolvedType::Known(ref left_ty) = left {
                    self.propagate_fallback_expected(&bin.right, left_ty);
                }
                let right = self.resolve_expr(&bin.right);
                if !matches!(right, ResolvedType::Unknown) {
                    right
                } else {
                    left
                }
            }
            NullishCoalescing => {
                let left = self.resolve_expr(&bin.left);
                let lhs_span = Span::from_swc(bin.left.span());
                let rhs_span = Span::from_swc(bin.right.span());

                // The `??` operator asserts runtime nullability of LHS regardless
                // of its static TS type (e.g., `arr[i]` has TS type `T` yet may be
                // undefined at runtime; `Option<T>` already-nullable). Unified
                // propagation: always set LHS span expected to `Option<inner>` so
                // `convert_member_expr` emits Option-preserving IR (`.get().cloned()`
                // or `.get().cloned().flatten()` for `Vec<Option<T>>`), and set RHS
                // span expected to `inner` (the final NC result type).
                //
                // `inner` is the unwrapped element type: `T` for non-Option LHS,
                // and the Option's inner type for Option LHS. Both paths produce
                // identical LHS-expected (`Option<inner>`), only differing in how
                // `inner` is computed. I-022.
                if let ResolvedType::Known(left_ty) = &left {
                    let inner = match left_ty {
                        RustType::Option(inner) => inner.as_ref().clone(),
                        other => other.clone(),
                    };
                    let lhs_expected = RustType::Option(Box::new(inner.clone()));
                    self.result
                        .expected_types
                        .insert(lhs_span, lhs_expected.clone());
                    self.propagate_expected(&bin.left, &lhs_expected);

                    // Preserve chain-case Option RHS: if a parent `propagate_expected`
                    // (the `Bin(NullishCoalescing)` arm) has already set RHS span to
                    // `Option<T>`, keep it so inner operands in an `a ?? b ?? c` chain
                    // produce Option-preserving IR. Otherwise (terminate case) use
                    // `inner` — the unwrapped final NC result type.
                    let existing_rhs = self.result.expected_types.get(&rhs_span).cloned();
                    let rhs_expected = match existing_rhs {
                        Some(RustType::Option(_)) => existing_rhs.unwrap(),
                        _ => inner.clone(),
                    };
                    self.result
                        .expected_types
                        .insert(rhs_span, rhs_expected.clone());
                    self.propagate_expected(&bin.right, &rhs_expected);
                }
                // LHS type unknown: leave expected types unset (existing
                // untyped-expression behavior).
                let right = self.resolve_expr(&bin.right);
                if !matches!(right, ResolvedType::Unknown) {
                    right
                } else {
                    left
                }
            }
        }
    }

    /// `x || {}` パターンで、右辺がオブジェクトリテラルの場合に左辺の解決済み型を
    /// expected type として右辺に伝播する。
    ///
    /// `??` (NullishCoalescing) は I-022 以降、この helper を使わず上記 arm で直接
    /// LHS/RHS の expected type を propagate する (runtime nullability を LHS に
    /// 反映するため汎用 propagation が必要)。本 helper は LogicalOr 専用。
    fn propagate_fallback_expected(&mut self, rhs: &ast::Expr, left_ty: &RustType) {
        if matches!(rhs, ast::Expr::Object(_)) {
            let resolved = self.resolve_type_params_in_type(left_ty);
            let rhs_span = Span::from_swc(rhs.span());
            self.result
                .expected_types
                .insert(rhs_span, resolved.clone());
            self.propagate_expected(rhs, &resolved);
        }
    }

    fn resolve_member_expr(&mut self, member: &ast::MemberExpr) -> ResolvedType {
        let obj_type = self.resolve_expr(&member.obj);
        match &obj_type {
            ResolvedType::Known(ty) => self.resolve_member_type(ty, &member.prop),
            ResolvedType::Unknown => ResolvedType::Unknown,
        }
    }

    /// Resolves the type of a member access given the object's type and property.
    pub(super) fn resolve_member_type(
        &self,
        obj_rust_type: &RustType,
        prop: &ast::MemberProp,
    ) -> ResolvedType {
        // Array/tuple/HashMap indexing
        if let ast::MemberProp::Computed(computed) = prop {
            match obj_rust_type {
                RustType::Vec(elem_ty) => return ResolvedType::Known(elem_ty.as_ref().clone()),
                RustType::Tuple(elems) => {
                    if let ast::Expr::Lit(ast::Lit::Num(num)) = &*computed.expr {
                        let idx = num.value as usize;
                        if idx < elems.len() {
                            return ResolvedType::Known(elems[idx].clone());
                        }
                    }
                    return ResolvedType::Unknown;
                }
                // HashMap<K, V>[key] → V
                RustType::Named { name, type_args }
                    if name == "HashMap" && type_args.len() == 2 =>
                {
                    return ResolvedType::Known(type_args[1].clone());
                }
                // I-387: StdCollection 版 HashMap
                RustType::StdCollection {
                    kind: crate::ir::StdCollectionKind::HashMap,
                    args,
                } if args.len() == 2 => {
                    return ResolvedType::Known(args[1].clone());
                }
                _ => {}
            }
        }

        // Named field access (Ident and PrivateName)
        let field_name = match prop {
            ast::MemberProp::Ident(ident) => ident.sym.to_string(),
            ast::MemberProp::PrivateName(private) => private.name.to_string(),
            _ => return ResolvedType::Unknown,
        };

        // Special case: .length on String/Vec (hardcoded for performance — avoids registry lookup)
        if field_name == "length" && matches!(obj_rust_type, RustType::String | RustType::Vec(_)) {
            return ResolvedType::Known(RustType::F64);
        }

        // 1. TypeRegistry (handles Vec→Array, String, Named, DynTrait, etc.)
        if let Some(ty) = self.registry.lookup_field_type(obj_rust_type, &field_name) {
            return ResolvedType::Known(ty);
        }

        // 2. Struct fields fallback (SyntheticTypeRegistry + type parameter constraints)
        if let RustType::Named { name, type_args } = obj_rust_type {
            if let Some(fields) = self.resolve_struct_fields_by_name(name, type_args) {
                if let Some((_, ty)) = fields.iter().find(|(n, _)| n == &field_name) {
                    return ResolvedType::Known(ty.clone());
                }
            }
        }
        // I-387: TypeVar の member access は constraint lookup で解決。
        if let RustType::TypeVar { name } = obj_rust_type {
            if let Some(fields) = self.resolve_struct_fields_by_name(name, &[]) {
                if let Some((_, ty)) = fields.iter().find(|(n, _)| n == &field_name) {
                    return ResolvedType::Known(ty.clone());
                }
            }
        }

        ResolvedType::Unknown
    }

    fn resolve_new_expr(&mut self, new_expr: &ast::NewExpr) -> ResolvedType {
        let class_name = match new_expr.callee.as_ref() {
            ast::Expr::Ident(ident) => ident.sym.to_string(),
            _ => return ResolvedType::Unknown,
        };

        // Convert explicit type arguments: `new Foo<string, number>(...)` → [String, F64]
        let explicit_type_args: Vec<RustType> = new_expr
            .type_args
            .as_ref()
            .map(|ta| convert_explicit_type_args(ta, self.synthetic, self.registry))
            .unwrap_or_default();

        if let Some(type_def) = self.registry.get(&class_name) {
            // Build type param bindings from explicit type args
            let type_param_bindings = match &type_def {
                TypeDef::Struct { type_params, .. } if !explicit_type_args.is_empty() => {
                    build_type_arg_bindings(type_params, &explicit_type_args)
                }
                _ => HashMap::new(),
            };

            // Temporarily add type arg bindings to constraints for param resolution
            let prev_constraints = if !type_param_bindings.is_empty() {
                let mut merged = self.type_param_constraints.clone();
                merged.extend(type_param_bindings);
                Some(std::mem::replace(&mut self.type_param_constraints, merged))
            } else {
                None
            };

            if let Some(args) = &new_expr.args {
                // Resolve parameter types: constructor signature first, then field fallback
                let param_types: Option<(Vec<RustType>, bool)> = match &type_def {
                    TypeDef::Struct {
                        constructor: Some(sigs),
                        ..
                    } if !sigs.is_empty() => {
                        let (_, sig) = select_overload(sigs, args.len(), &[]);
                        Some((
                            sig.params.iter().map(|p| p.ty.clone()).collect(),
                            sig.has_rest,
                        ))
                    }
                    TypeDef::Struct { fields, .. } => {
                        // Fallback: no constructor defined, use field types
                        Some((fields.iter().map(|f| f.ty.clone()).collect(), false))
                    }
                    _ => None,
                };
                if let Some((param_types, has_rest)) = param_types {
                    self.propagate_call_arg_expected_types(args, &param_types, has_rest);
                }
            }

            // Restore constraints
            if let Some(prev) = prev_constraints {
                self.type_param_constraints = prev;
            }

            // Resolve argument expressions (needed for type arg inference)
            if let Some(args) = &new_expr.args {
                for arg in args {
                    self.resolve_expr(&arg.expr);
                }
            }

            // Infer type args from resolved argument types when no explicit type args
            let inferred_type_args = if explicit_type_args.is_empty() {
                self.infer_new_expr_type_args(type_def, new_expr)
            } else {
                explicit_type_args
            };

            ResolvedType::Known(RustType::Named {
                name: class_name,
                type_args: inferred_type_args,
            })
        } else {
            ResolvedType::Unknown
        }
    }

    /// Infers type arguments for a `new` expression from resolved argument types.
    fn infer_new_expr_type_args(
        &self,
        type_def: &TypeDef,
        new_expr: &ast::NewExpr,
    ) -> Vec<RustType> {
        let TypeDef::Struct {
            type_params,
            constructor,
            ..
        } = type_def
        else {
            return vec![];
        };
        if type_params.is_empty() {
            return vec![];
        }

        let Some(args) = &new_expr.args else {
            return vec![];
        };

        // Get constructor param types
        let param_types: Vec<RustType> = match constructor {
            Some(sigs) if !sigs.is_empty() => {
                let (_, sig) = select_overload(sigs, args.len(), &[]);
                sig.params.iter().map(|p| p.ty.clone()).collect()
            }
            _ => return vec![],
        };

        let arg_types = self.collect_resolved_arg_types(args);
        let bindings =
            super::call_resolution::infer_type_args(type_params, &param_types, &arg_types);

        if bindings.is_empty() {
            return vec![];
        }

        // Build type args in the order of type_params.
        // Only return type args if ALL type params were inferred.
        // Partial inference (some params unresolved) returns empty to avoid
        // introducing Any as a type argument (type-fallback-safety concern).
        let inferred: Vec<RustType> = type_params
            .iter()
            .filter_map(|tp| bindings.get(&tp.name).cloned())
            .collect();

        if inferred.len() == type_params.len() {
            inferred
        } else {
            vec![]
        }
    }
}
