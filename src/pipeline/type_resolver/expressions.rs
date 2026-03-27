//! Expression type resolution for TypeResolver.
//!
//! Resolves types for all expression forms: literals, binary ops, member access,
//! function calls, arrow functions, object/array literals, optional chaining, etc.

use swc_common::Spanned;
use swc_ecma_ast as ast;

use super::*;
use crate::pipeline::type_converter::convert_ts_type;
use crate::pipeline::type_resolution::Span;
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
            ast::Expr::Tpl(_) => ResolvedType::Known(RustType::String),
            ast::Expr::Bin(bin) => self.resolve_bin_expr(bin),
            ast::Expr::Member(member) => self.resolve_member_expr(member),
            ast::Expr::Call(call) => self.resolve_call_expr(call),
            ast::Expr::New(new_expr) => self.resolve_new_expr(new_expr),
            ast::Expr::Paren(paren) => self.resolve_expr(&paren.expr),
            ast::Expr::TsAs(ts_as) => {
                // Resolve inner expression to register its type and trigger nested
                // call resolution (e.g., `foo(bar(x) as T)` needs bar's args typed).
                self.resolve_expr(&ts_as.expr);
                convert_ts_type(&ts_as.type_ann, self.synthetic, self.registry)
                    .map(ResolvedType::Known)
                    .unwrap_or(ResolvedType::Unknown)
            }
            ast::Expr::Array(arr) => self.resolve_array_expr(arr),
            ast::Expr::Arrow(arrow) => self.resolve_arrow_expr(arrow),
            ast::Expr::Fn(fn_expr) => self.resolve_fn_expr(fn_expr),
            ast::Expr::Assign(assign) => {
                // Mark left side as mutable
                if let Some(ast::SimpleAssignTarget::Ident(ident)) = assign.left.as_simple() {
                    self.mark_var_mutable(ident.id.sym.as_ref());
                    // Set LHS type as expected on RHS and propagate
                    let lhs_type = self.lookup_var(ident.id.sym.as_ref());
                    if let ResolvedType::Known(ref ty) = lhs_type {
                        let rhs_span = Span::from_swc(assign.right.span());
                        self.result.expected_types.insert(rhs_span, ty.clone());
                        self.propagate_expected(&assign.right, ty);
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

                let cons_is_null = is_null_or_undefined(&cond.cons);
                let alt_is_null = is_null_or_undefined(&cond.alt);
                let cons_is_option = matches!(&cons, ResolvedType::Known(RustType::Option(_)));
                let alt_is_option = matches!(&alt, ResolvedType::Known(RustType::Option(_)));

                let produces_option =
                    cons_is_null || alt_is_null || cons_is_option || alt_is_option;

                if produces_option {
                    // Pick the non-null branch's type as the value type
                    let value_type = if cons_is_null { &alt } else { &cons };
                    match value_type {
                        ResolvedType::Known(RustType::Option(_)) => value_type.clone(),
                        ResolvedType::Known(ty) => {
                            ResolvedType::Known(RustType::Option(Box::new(ty.clone())))
                        }
                        ResolvedType::Unknown => {
                            // Value type unknown but result is optional
                            ResolvedType::Known(RustType::Option(Box::new(RustType::Any)))
                        }
                    }
                } else {
                    // Neither branch is null/Option: prefer non-Unknown
                    if !matches!(cons, ResolvedType::Unknown) {
                        cons
                    } else {
                        alt
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
                // <T>x — same as TsAs
                self.resolve_expr(&assertion.expr);
                convert_ts_type(&assertion.type_ann, self.synthetic, self.registry)
                    .map(ResolvedType::Known)
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
                // Returns None if any spread source's fields can't be resolved
                // (prevents generating incomplete structs that silently drop fields).
                let merged = match self.merge_object_fields(&spread_types, &explicit_fields) {
                    Some(fields) if !fields.is_empty() => fields,
                    _ => return ResolvedType::Unknown,
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
                                if let Some(params) = method_name.as_deref().and_then(|name| {
                                    self.lookup_method_params(inner_ty, name, n_args, &[])
                                }) {
                                    for (arg, param_ty) in opt_call.args.iter().zip(params.iter()) {
                                        let arg_span = Span::from_swc(arg.expr.span());
                                        self.result
                                            .expected_types
                                            .insert(arg_span, param_ty.clone());
                                        self.propagate_expected(&arg.expr, param_ty);
                                    }
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
                // Wrap in Option<T>. OptChain always produces an optional result,
                // even if the inner type is unknown.
                match inner_result {
                    ResolvedType::Known(RustType::Option(_)) => inner_result,
                    ResolvedType::Known(ty) => ResolvedType::Known(RustType::Option(Box::new(ty))),
                    ResolvedType::Unknown => {
                        ResolvedType::Known(RustType::Option(Box::new(RustType::Any)))
                    }
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
            LogicalAnd | LogicalOr => {
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
            NullishCoalescing => {
                let left = self.resolve_expr(&bin.left);
                // If left is Option<T>, set inner T as expected on RHS
                if let ResolvedType::Known(RustType::Option(ref inner)) = left {
                    let rhs_span = Span::from_swc(bin.right.span());
                    self.result
                        .expected_types
                        .insert(rhs_span, inner.as_ref().clone());
                    self.propagate_expected(&bin.right, inner);
                }
                let right = self.resolve_expr(&bin.right);
                if !matches!(right, ResolvedType::Unknown) {
                    right
                } else {
                    left
                }
            }
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
        // Array/tuple indexing
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
                _ => {}
            }
        }

        // Named field access
        let field_name = match prop {
            ast::MemberProp::Ident(ident) => ident.sym.to_string(),
            _ => return ResolvedType::Unknown,
        };

        // Special case: .length on String/Vec
        if field_name == "length" && matches!(obj_rust_type, RustType::String | RustType::Vec(_)) {
            return ResolvedType::Known(RustType::F64);
        }

        // Lookup in TypeRegistry
        let (type_name, type_args) = match extract_type_name_for_registry(obj_rust_type) {
            Some(pair) => pair,
            None => return ResolvedType::Unknown,
        };

        let type_def = if type_args.is_empty() {
            self.registry.get(type_name).cloned()
        } else {
            self.registry.instantiate(type_name, type_args)
        };

        match &type_def {
            Some(TypeDef::Struct { fields, .. }) => fields
                .iter()
                .find(|(name, _)| name == &field_name)
                .map(|(_, ty)| ResolvedType::Known(ty.clone()))
                .unwrap_or(ResolvedType::Unknown),
            _ => ResolvedType::Unknown,
        }
    }

    fn resolve_new_expr(&mut self, new_expr: &ast::NewExpr) -> ResolvedType {
        let class_name = match new_expr.callee.as_ref() {
            ast::Expr::Ident(ident) => ident.sym.to_string(),
            _ => return ResolvedType::Unknown,
        };

        if let Some(type_def) = self.registry.get(&class_name) {
            if let Some(args) = &new_expr.args {
                // Resolve parameter types: constructor signature first, then field fallback
                let param_types: Option<Vec<RustType>> = match type_def {
                    TypeDef::Struct {
                        constructor: Some(sigs),
                        ..
                    } if !sigs.is_empty() => {
                        let sig = select_overload(sigs, args.len(), &[]);
                        Some(sig.params.iter().map(|(_, ty)| ty.clone()).collect())
                    }
                    TypeDef::Struct { fields, .. } => {
                        // Fallback: no constructor defined, use field types
                        Some(fields.iter().map(|(_, ty)| ty.clone()).collect())
                    }
                    _ => None,
                };
                if let Some(param_types) = param_types {
                    self.propagate_arg_expected_types(args, &param_types);
                }
            }
            ResolvedType::Known(RustType::Named {
                name: class_name,
                type_args: vec![],
            })
        } else {
            ResolvedType::Unknown
        }
    }

    /// Resolves an arrow function expression, walking its body.
    fn resolve_arrow_expr(&mut self, arrow: &ast::ArrowExpr) -> ResolvedType {
        self.enter_scope();

        // Save and set return type (Promise<T> → T, void → None, trait → Box<dyn Trait>)
        let prev_return_type = self.current_fn_return_type.take();
        if let Some(return_ann) = &arrow.return_type {
            if let Ok(ty) = convert_ts_type(&return_ann.type_ann, self.synthetic, self.registry) {
                self.current_fn_return_type = unwrap_promise_and_unit(ty)
                    .map(|ty| wrap_trait_for_position(ty, TypePosition::Value, self.registry));
            }
        }

        // If no explicit return annotation, check expected type from parent context
        // (e.g., variable type annotation: `const f: FnType = () => ...`)
        let expected_param_types = if self.current_fn_return_type.is_none() {
            let arrow_span = Span::from_swc(arrow.span);
            if let Some(expected) = self.result.expected_types.get(&arrow_span).cloned() {
                let (ret, params) = resolve_fn_type_info(&expected, self.registry);
                if let Some(ret_ty) = ret {
                    self.current_fn_return_type = unwrap_promise_and_unit(ret_ty);
                }
                params
            } else {
                None
            }
        } else {
            None
        };

        // Register parameters
        let mut param_types = Vec::new();
        for (i, param) in arrow.params.iter().enumerate() {
            match param {
                ast::Pat::Ident(ident) => {
                    let name = ident.id.sym.to_string();
                    let span = Span::from_swc(ident.id.span);
                    // Arrow's own annotation takes priority; fall back to expected param type
                    let ty = ident
                        .type_ann
                        .as_ref()
                        .and_then(|ann| {
                            convert_ts_type(&ann.type_ann, self.synthetic, self.registry).ok()
                        })
                        .or_else(|| {
                            expected_param_types
                                .as_ref()
                                .and_then(|types| types.get(i).cloned())
                        })
                        .map(|ty| wrap_trait_for_position(ty, TypePosition::Param, self.registry));
                    let resolved = ty
                        .clone()
                        .map(ResolvedType::Known)
                        .unwrap_or(ResolvedType::Unknown);
                    self.declare_var(&name, resolved, span, false);
                    param_types.push(ty.unwrap_or(RustType::Any));
                }
                // Destructuring patterns: extract type from type annotation
                _ => {
                    let ty: Option<RustType> = match param {
                        ast::Pat::Array(arr) => arr.type_ann.as_ref().and_then(|ann| {
                            convert_ts_type(&ann.type_ann, self.synthetic, self.registry).ok()
                        }),
                        ast::Pat::Object(obj) => obj.type_ann.as_ref().and_then(|ann| {
                            convert_ts_type(&ann.type_ann, self.synthetic, self.registry).ok()
                        }),
                        _ => None,
                    };
                    let ty = ty
                        .or_else(|| {
                            expected_param_types
                                .as_ref()
                                .and_then(|types| types.get(i).cloned())
                        })
                        .map(|ty| wrap_trait_for_position(ty, TypePosition::Param, self.registry));
                    // Register sub-pattern variables
                    self.visit_param_pat(param);
                    param_types.push(ty.unwrap_or(RustType::Any));
                } // Ident is handled above; remaining patterns are covered by the
                  // destructuring branch. This arm is unreachable but kept for safety.
            }
        }

        // Walk body
        match &*arrow.body {
            ast::BlockStmtOrExpr::BlockStmt(block) => {
                for stmt in &block.stmts {
                    self.visit_stmt(stmt);
                }
            }
            ast::BlockStmtOrExpr::Expr(expr) => {
                // Propagate return type as expected type to expression body
                if let Some(return_ty) = self.current_fn_return_type.clone() {
                    let span = Span::from_swc(expr.span());
                    self.result.expected_types.insert(span, return_ty.clone());
                    self.propagate_expected(expr, &return_ty);
                }
                let span = Span::from_swc(expr.span());
                let ty = self.resolve_expr(expr);
                self.result.expr_types.insert(span, ty);
            }
        }

        let return_type = self.current_fn_return_type.take().unwrap_or(RustType::Unit);
        self.current_fn_return_type = prev_return_type;
        self.leave_scope();

        ResolvedType::Known(RustType::Fn {
            params: param_types,
            return_type: Box::new(return_type),
        })
    }

    /// Resolves a function expression, walking its body.
    fn resolve_fn_expr(&mut self, fn_expr: &ast::FnExpr) -> ResolvedType {
        self.enter_scope();

        let prev_return_type = self.current_fn_return_type.take();
        if let Some(return_ann) = &fn_expr.function.return_type {
            if let Ok(ty) = convert_ts_type(&return_ann.type_ann, self.synthetic, self.registry) {
                self.current_fn_return_type = unwrap_promise_and_unit(ty)
                    .map(|ty| wrap_trait_for_position(ty, TypePosition::Value, self.registry));
            }
        }

        // If no explicit return annotation, check expected type from parent context
        if self.current_fn_return_type.is_none() {
            let fn_span = Span::from_swc(fn_expr.function.span);
            if let Some(expected) = self.result.expected_types.get(&fn_span).cloned() {
                let (ret, _params) = resolve_fn_type_info(&expected, self.registry);
                if let Some(ret_ty) = ret {
                    self.current_fn_return_type = unwrap_promise_and_unit(ret_ty);
                }
            }
        }

        // Register parameters and collect their types
        let mut param_types = Vec::new();
        for param in &fn_expr.function.params {
            if let ast::Pat::Ident(ident) = &param.pat {
                let ty = ident
                    .type_ann
                    .as_ref()
                    .and_then(|ann| {
                        convert_ts_type(&ann.type_ann, self.synthetic, self.registry).ok()
                    })
                    .map(|ty| wrap_trait_for_position(ty, TypePosition::Param, self.registry));
                param_types.push(ty.unwrap_or(RustType::Any));
            }
            self.visit_param_pat(&param.pat);
        }

        if let Some(body) = &fn_expr.function.body {
            for stmt in &body.stmts {
                self.visit_stmt(stmt);
            }
        }

        let return_type = self.current_fn_return_type.take().unwrap_or(RustType::Unit);
        self.current_fn_return_type = prev_return_type;
        self.leave_scope();

        ResolvedType::Known(RustType::Fn {
            params: param_types,
            return_type: Box::new(return_type),
        })
    }

    /// Resolves an array literal expression.
    fn resolve_array_expr(&mut self, arr: &ast::ArrayLit) -> ResolvedType {
        if arr.elems.is_empty() {
            return ResolvedType::Unknown;
        }

        let mut element_type: Option<RustType> = None;
        let mut all_same = true;

        for elem in arr.elems.iter().flatten() {
            let span = Span::from_swc(elem.expr.span());
            let ty = self.resolve_expr(&elem.expr);
            self.result.expr_types.insert(span, ty.clone());

            if let ResolvedType::Known(rust_ty) = &ty {
                match &element_type {
                    None => element_type = Some(rust_ty.clone()),
                    Some(existing) if existing != rust_ty => all_same = false,
                    _ => {}
                }
            }
        }

        if all_same {
            if let Some(elem_ty) = element_type {
                return ResolvedType::Known(RustType::Vec(Box::new(elem_ty)));
            }
        }

        ResolvedType::Unknown
    }
}
