//! Module-level `const` variable declaration conversion.
//!
//! Handles arrow function initializers (`const double = (x: number) => x * 2`)
//! and literal initializers (`const N: number = 42`).
//! Other init types (call, ident, object, array) are currently skipped.

use super::*;

impl<'a> Transformer<'a> {
    /// Converts module-level `const` variable declarations into IR items.
    ///
    /// - Arrow function init → `Item::Fn`
    /// - Literal init → `Item::Const`
    /// - Other init types → skipped (follow-up PRD)
    pub(crate) fn convert_var_decl_module_level(
        &mut self,
        var_decl: &ast::VarDecl,
        vis: Visibility,
        resilient: bool,
    ) -> Result<(Vec<Item>, Vec<String>)> {
        let mut items = Vec::new();
        let mut all_warnings = Vec::new();
        for decl in &var_decl.decls {
            let init = match &decl.init {
                Some(init) => init,
                None => continue,
            };
            // Dispatch by init expression type
            match init.as_ref() {
                ast::Expr::Arrow(arrow) => {
                    let result = self.convert_arrow_var_decl(decl, arrow, vis, resilient)?;
                    items.extend(result.0);
                    all_warnings.extend(result.1);
                    continue;
                }
                ast::Expr::Lit(lit) => {
                    if let Some(item) = self.convert_lit_var_decl(decl, lit, vis)? {
                        items.push(item);
                    }
                    continue;
                }
                _ => continue, // Call, Ident, Object, Array → follow-up PRD
            }
        }
        Ok((items, all_warnings))
    }

    /// Converts a const-safe literal `const` declaration to `Item::Const`.
    ///
    /// Only handles literal types that produce valid Rust `const` values:
    /// `Num`, `Bool`, `Null`. String and Regex literals are not const-safe
    /// in Rust (`to_string()` and `Regex::new()` are not const fn) and are skipped.
    fn convert_lit_var_decl(
        &mut self,
        decl: &ast::VarDeclarator,
        lit: &ast::Lit,
        vis: Visibility,
    ) -> Result<Option<Item>> {
        // Only const-safe literals: Num, Bool, Null
        // Str → "x".to_string() is not const; Regex → Regex::new() is not const;
        // BigInt → not supported; JSXText → not applicable
        match lit {
            ast::Lit::Num(_) | ast::Lit::Bool(_) | ast::Lit::Null(_) => {}
            _ => return Ok(None),
        }

        let ident = match &decl.name {
            ast::Pat::Ident(ident) => ident,
            _ => return Ok(None),
        };
        let name = ident.id.sym.to_string();
        let ty = match ident.type_ann.as_ref() {
            Some(ann) => convert_ts_type(&ann.type_ann, self.synthetic, self.reg())?,
            None => Self::infer_const_type(lit),
        };
        // Skip if type resolves to Any (serde_json::Value) — not const-constructible from literal
        if matches!(ty, RustType::Any) {
            return Ok(None);
        }
        let value = self.convert_expr(&ast::Expr::Lit(lit.clone()))?;
        Ok(Some(Item::Const {
            vis,
            name,
            ty,
            value,
        }))
    }

    /// Converts an arrow function `const` declaration to `Item::Fn` or callable trait items.
    fn convert_arrow_var_decl(
        &mut self,
        decl: &ast::VarDeclarator,
        arrow: &ast::ArrowExpr,
        vis: Visibility,
        resilient: bool,
    ) -> Result<(Vec<Item>, Vec<String>)> {
        let ident = match &decl.name {
            ast::Pat::Ident(ident) => ident,
            _ => return Ok((vec![], vec![])),
        };
        let name = ident.id.sym.to_string();
        let var_rust_type = ident
            .type_ann
            .as_ref()
            .and_then(|ann| convert_ts_type(&ann.type_ann, self.synthetic, self.reg()).ok());

        // Check if type annotation refers to a callable interface → route to trait const
        if let Some((trait_name, trait_type_args)) =
            self.callable_trait_name_and_args(var_rust_type.as_ref())
        {
            let items = self.convert_callable_trait_const(
                &name,
                &trait_name,
                &trait_type_args,
                arrow,
                vis,
                resilient,
            )?;
            return Ok((items, vec![]));
        }

        let var_return_type = var_rust_type
            .as_ref()
            .and_then(|ty| self.extract_fn_return_type(ty));
        let var_param_types = var_rust_type
            .as_ref()
            .and_then(|ty| self.extract_fn_param_types(ty));

        let mut fallback_warnings = Vec::new();
        let arrow_scope_start = arrow.span.lo.0;

        let closure = self
            .spawn_nested_scope()
            .convert_arrow_expr_with_return_type(
                arrow,
                resilient,
                &mut fallback_warnings,
                var_return_type.as_ref(),
                var_param_types.as_deref(),
            )?;

        let mut items = Vec::new();
        if let Expr::Closure {
            mut params,
            return_type,
            body,
        } = closure
        {
            let ret = return_type;
            let mut fn_body = match body {
                crate::ir::ClosureBody::Expr(expr) => {
                    vec![Stmt::Return(Some(*expr))]
                }
                crate::ir::ClosureBody::Block(stmts) => stmts,
            };
            convert_last_return_to_tail(&mut fn_body);
            for p in &mut params {
                if p.ty.is_none() {
                    p.ty = Some(RustType::Any);
                }
                if matches!(&p.ty, Some(RustType::Any)) {
                    if let Some(enum_ty) = self
                        .tctx
                        .type_resolution
                        .any_enum_override(&p.name, arrow_scope_start)
                    {
                        p.ty = Some(enum_ty.clone());
                    }
                }
            }

            let (type_params, mono_subs) =
                extract_type_params(arrow.type_params.as_deref(), self.synthetic, self.reg());
            let item = Item::Fn {
                vis,
                attributes: vec![],
                is_async: arrow.is_async,
                name,
                type_params,
                params,
                return_type: ret,
                body: fn_body,
            };
            items.push(if mono_subs.is_empty() {
                item
            } else {
                item.substitute(&mono_subs)
            });
        }
        Ok((items, fallback_warnings))
    }

    /// Returns (trait_name, type_args) if `var_rust_type` refers to a callable interface.
    ///
    /// Looks up the type name in the registry and classifies it. Returns `None` if
    /// the type is not a callable interface.
    fn callable_trait_name_and_args(
        &self,
        var_rust_type: Option<&RustType>,
    ) -> Option<(String, Vec<RustType>)> {
        let (name, type_args) = match var_rust_type? {
            RustType::Named { name, type_args } => (name.as_str(), type_args.clone()),
            _ => return None,
        };
        let def = self.reg().get(name)?;
        use crate::registry::collection::{classify_callable_interface, CallableInterfaceKind};
        let kind = classify_callable_interface(def);
        match kind {
            CallableInterfaceKind::NonCallable => None,
            _ => Some((name.to_string(), type_args)),
        }
    }

    /// Converts a callable interface const declaration to the full trait-based structure.
    ///
    /// Generates: ZST marker struct, inner fn (widest signature), return wrap (divergent returns),
    /// delegate impl (per-overload trait methods), and const instance.
    fn convert_callable_trait_const(
        &mut self,
        value_name: &str,
        trait_name: &str,
        _trait_type_args: &[RustType],
        arrow: &ast::ArrowExpr,
        vis: Visibility,
        resilient: bool,
    ) -> Result<Vec<Item>> {
        // --- Compute widest signature from registry ---
        let call_sigs = match self.reg().get(trait_name) {
            Some(crate::registry::TypeDef::Struct {
                call_signatures, ..
            }) => call_signatures.clone(),
            _ => {
                return Err(anyhow::anyhow!(
                    "callable interface '{trait_name}' not found"
                ))
            }
        };
        let widest = crate::pipeline::type_converter::overloaded_callable::compute_widest_signature(
            &call_sigs,
            self.synthetic,
        );

        // --- Marker struct name (INV-1) ---
        let base_marker = Self::marker_struct_name(trait_name, value_name);
        let marker_name = self.allocate_marker_name(&base_marker);

        // --- ZST marker struct (P5.2) ---
        let marker_struct = Item::Struct {
            vis: Visibility::Private,
            name: marker_name.clone(),
            type_params: vec![],
            fields: vec![],
            is_unit_struct: true,
        };

        // --- Inner fn body from arrow (P5.4) ---
        // fallback_warnings は callable interface path では使用しない
        // (型注釈付きのため fallback が少なく、呼び出し元で空 vec を返す)
        let mut fallback_warnings = Vec::new();
        let widest_return = widest.return_type.clone();
        let widest_param_types: Vec<RustType> =
            widest.params.iter().map(|p| p.ty.clone()).collect();

        let closure = self
            .spawn_nested_scope()
            .convert_arrow_expr_with_return_type(
                arrow,
                resilient,
                &mut fallback_warnings,
                widest_return.as_ref(),
                Some(&widest_param_types),
            )?;

        // Extract body and params from closure.
        // Closure params use arrow's names + widest types (applied by convert_arrow_expr_with_return_type).
        // We must use arrow param names for inner fn because the body references them.
        let (mut fn_body, closure_params) = if let Expr::Closure { body, params, .. } = closure {
            let stmts = match body {
                crate::ir::ClosureBody::Expr(expr) => vec![Stmt::Return(Some(*expr))],
                crate::ir::ClosureBody::Block(stmts) => stmts,
            };
            (stmts, params)
        } else {
            (vec![], vec![])
        };
        convert_last_return_to_tail(&mut fn_body);

        // --- Return wrap for divergent returns (P7.0) ---
        // When overloads have different return types, the inner fn returns a synthetic
        // union enum. Each return expression must be wrapped in the appropriate variant.
        // Types are pre-collected from the SWC AST before conversion, then applied
        // positionally to the IR return expressions.
        let wrap_ctx = widest_return.as_ref().and_then(|ret_ty| {
            if let RustType::Named { name, .. } = ret_ty {
                crate::transformer::return_wrap::build_return_wrap_context(&call_sigs, name)
            } else {
                None
            }
        });
        if let Some(ref ctx) = wrap_ctx {
            let leaf_types = crate::transformer::return_wrap::collect_return_leaf_types(
                arrow,
                self.tctx.type_resolution,
            );
            wrap_body_returns(&mut fn_body, &mut leaf_types.into_iter(), ctx)?;
        }

        // Inner method params: use closure params (arrow names + widest types) for positions
        // within arrow arity, then append widest params for positions beyond arrow arity.
        // This ensures body variable references match param names.
        let mut inner_params = closure_params;
        // Ensure all params have types (fallback to widest type if closure left ty as None)
        for (i, p) in inner_params.iter_mut().enumerate() {
            if p.ty.is_none() {
                if let Some(wp) = widest.params.get(i) {
                    p.ty = Some(wp.ty.clone());
                } else {
                    p.ty = Some(RustType::Any);
                }
            }
        }
        // Append widest params beyond arrow arity
        for i in inner_params.len()..widest.params.len() {
            inner_params.push(crate::ir::Param {
                name: widest.params[i].name.clone(),
                ty: Some(widest.params[i].ty.clone()),
            });
        }

        // Inner fn return type: compute_union_return already unwraps Promise<T> → T
        // (because trait methods unwrap Promise to async fn → T). No additional
        // unwrap needed here.
        let inner_return_type = widest.return_type.clone();

        let inner_method = Method {
            vis: Visibility::Private,
            name: "inner".to_string(),
            is_async: arrow.is_async,
            has_self: true,
            has_mut_self: false,
            params: inner_params,
            return_type: inner_return_type,
            body: Some(fn_body),
        };

        // Marker impl with inner method
        let marker_impl = Item::Impl {
            struct_name: marker_name.clone(),
            type_params: vec![],
            for_trait: None,
            consts: vec![],
            methods: vec![inner_method],
        };

        // --- Delegate impl: trait impl with call_N methods (P7.1) ---
        let delegate_impl = build_delegate_impl(
            &marker_name,
            trait_name,
            &call_sigs,
            &widest,
            wrap_ctx.as_ref(),
            arrow.is_async,
        )?;

        // --- Const instance (P8.1) ---
        let const_instance = Item::Const {
            vis,
            name: value_name.to_string(),
            ty: RustType::Named {
                name: marker_name.clone(),
                type_args: vec![],
            },
            value: Expr::StructInit {
                name: marker_name.clone(),
                fields: vec![],
                base: None,
            },
        };

        Ok(vec![
            marker_struct,
            marker_impl,
            delegate_impl,
            const_instance,
        ])
    }

    /// Generates the marker struct name for a callable interface const.
    ///
    /// `marker_struct_name("GetCookie", "getCookie")` → `"GetCookieGetCookieImpl"`
    /// `marker_struct_name("Handler", "request_handler")` → `"HandlerRequestHandlerImpl"`
    ///
    /// value_name の先頭を大文字化して trait_name と結合する。
    /// snake_case は `string_to_pascal_case` で PascalCase 化し、
    /// camelCase はそのまま先頭大文字化する。
    pub(crate) fn marker_struct_name(trait_name: &str, value_name: &str) -> String {
        let capitalized = if value_name.contains('_') {
            // snake_case → PascalCase
            crate::ir::string_to_pascal_case(value_name)
        } else {
            // camelCase / single word → 先頭大文字化
            let mut chars = value_name.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        };
        format!("{trait_name}{capitalized}Impl")
    }

    /// Infers the Rust type for a const-safe literal without type annotation.
    fn infer_const_type(lit: &ast::Lit) -> RustType {
        match lit {
            ast::Lit::Num(_) => RustType::F64,
            ast::Lit::Bool(_) => RustType::Bool,
            // Null without type annotation → Option<serde_json::Value> (best-effort)
            ast::Lit::Null(_) => RustType::Option(Box::new(RustType::Any)),
            // Unreachable: const-safe filter in convert_lit_var_decl rejects other literals
            _ => RustType::Any,
        }
    }

    /// Extracts the return type from a function type.
    ///
    /// Handles two cases:
    /// - `RustType::Fn { return_type, .. }` → returns the return_type directly
    /// - `RustType::Named { name, .. }` → looks up TypeRegistry for `TypeDef::Function`
    pub(crate) fn extract_fn_return_type(&self, ty: &RustType) -> Option<RustType> {
        match ty {
            RustType::Fn { return_type, .. } => {
                let rt = return_type.as_ref();
                if matches!(rt, RustType::Unit) {
                    None
                } else {
                    Some(rt.clone())
                }
            }
            RustType::Named { name, .. } => {
                if let Some(crate::registry::TypeDef::Function { return_type, .. }) =
                    self.reg().get(name)
                {
                    return_type.clone()
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Extracts parameter types from a function type.
    ///
    /// Handles two cases:
    /// - `RustType::Fn { params, .. }` → returns the params directly
    /// - `RustType::Named { name, .. }` → looks up TypeRegistry for `TypeDef::Function`
    pub(crate) fn extract_fn_param_types(&self, ty: &RustType) -> Option<Vec<RustType>> {
        match ty {
            RustType::Fn { params, .. } => Some(params.clone()),
            RustType::Named { name, .. } => {
                if let Some(crate::registry::TypeDef::Function { params, .. }) =
                    self.reg().get(name)
                {
                    Some(params.iter().map(|p| p.ty.clone()).collect())
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// Wraps a delegate method argument to match the widest param type (P7.2).
///
/// - Types match → bare `Ident(name)`
/// - Widest is `Option<T>`, overload is `T` → `Some(name)`
/// - Widest is a synthetic union enum, overload is one variant → `Enum::Variant(name)`
///
/// NOTE: `RustType::Named` is treated as a synthetic union enum. This is safe because
/// `wrap_delegate_arg` is only called from `build_delegate_method` with widest params
/// from `compute_widest_signature`. Identical types are caught by the equality check
/// above, so `Named` is reached only when `unify_types` produced a synthetic union.
/// - Widest is `Option<union>`, overload is variant → `Some(Enum::Variant(name))`
fn wrap_delegate_arg(param_name: &str, overload_ty: &RustType, widest_ty: &RustType) -> Expr {
    let arg_expr = Expr::Ident(param_name.to_string());

    if overload_ty == widest_ty {
        // Types match exactly → bare arg
        return arg_expr;
    }

    // Widest is Option<inner>
    if let RustType::Option(inner) = widest_ty {
        if inner.as_ref() == overload_ty {
            // Option<T> vs T → Some(arg)
            return Expr::FnCall {
                target: crate::ir::CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Some),
                args: vec![arg_expr],
            };
        }
        // Option<union_enum> vs variant_type → Some(Enum::Variant(arg))
        if let RustType::Named { name, .. } = inner.as_ref() {
            let variant = crate::pipeline::synthetic_registry::variant_name_for_type(overload_ty);
            return Expr::FnCall {
                target: crate::ir::CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Some),
                args: vec![Expr::FnCall {
                    target: crate::ir::CallTarget::UserEnumVariantCtor {
                        enum_ty: crate::ir::UserTypeRef::new(name),
                        variant,
                    },
                    args: vec![arg_expr],
                }],
            };
        }
    }

    // Widest is a union enum, overload is one variant → Enum::Variant(arg)
    if let RustType::Named { name, .. } = widest_ty {
        let variant = crate::pipeline::synthetic_registry::variant_name_for_type(overload_ty);
        return Expr::FnCall {
            target: crate::ir::CallTarget::UserEnumVariantCtor {
                enum_ty: crate::ir::UserTypeRef::new(name),
                variant,
            },
            args: vec![arg_expr],
        };
    }

    // Fallback: bare arg (types should match but don't — best effort)
    arg_expr
}

/// Builds the delegate `impl TraitName for MarkerStruct` block.
///
/// Each overload's `call_N` method calls `self.inner(...)` with arg wrapping,
/// and for divergent returns, matches the result to extract the correct variant.
fn build_delegate_impl(
    marker_name: &str,
    trait_name: &str,
    call_sigs: &[crate::registry::MethodSignature],
    widest: &crate::pipeline::type_converter::overloaded_callable::WidestSignature,
    wrap_ctx: Option<&crate::transformer::return_wrap::ReturnWrapContext>,
    is_async: bool,
) -> anyhow::Result<Item> {
    let methods: Vec<Method> = call_sigs
        .iter()
        .enumerate()
        .map(|(i, sig)| build_delegate_method(i, sig, widest, wrap_ctx, is_async))
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(Item::Impl {
        struct_name: marker_name.to_string(),
        type_params: vec![],
        for_trait: Some(crate::ir::TraitRef {
            name: trait_name.to_string(),
            type_args: vec![],
        }),
        consts: vec![],
        methods,
    })
}

/// Builds a single delegate method `call_N` for one overload.
///
/// - Params: the overload's own params (not widest)
/// - Body: `self.inner(args...)` with arg wrapping for widest compatibility
/// - Return: for divergent returns, `match` unwrap; for non-divergent, direct return
fn build_delegate_method(
    index: usize,
    sig: &crate::registry::MethodSignature,
    widest: &crate::pipeline::type_converter::overloaded_callable::WidestSignature,
    wrap_ctx: Option<&crate::transformer::return_wrap::ReturnWrapContext>,
    is_async: bool,
) -> anyhow::Result<Method> {
    let method_name = format!("call_{index}");

    // Build params matching the overload signature
    let params: Vec<crate::ir::Param> = sig
        .params
        .iter()
        .map(|p| crate::ir::Param {
            name: p.name.clone(),
            ty: Some(p.ty.clone()),
        })
        .collect();

    // Build inner call args (P7.2):
    // For each widest param position:
    //   - Overload has this param, types match → bare arg
    //   - Overload has this param, widest is Option<T>, overload is T → Some(arg)
    //   - Overload has this param, widest is union enum → EnumName::Variant(arg)
    //   - Overload has this param, widest is Option<union>, overload is variant → Some(Variant(arg))
    //   - Beyond overload arity → None
    let inner_args: Vec<Expr> = widest
        .params
        .iter()
        .enumerate()
        .map(|(j, wp)| {
            if let Some(op) = sig.params.get(j) {
                wrap_delegate_arg(&op.name, &op.ty, &wp.ty)
            } else {
                // Beyond overload arity → None
                Expr::BuiltinVariantValue(crate::ir::BuiltinVariant::None)
            }
        })
        .collect();

    // Build inner call: self.inner(args...) [.await if async] (P7.3)
    let raw_inner_call = Expr::MethodCall {
        object: Box::new(Expr::Ident("self".to_string())),
        method: "inner".to_string(),
        args: inner_args,
    };
    let inner_call = if is_async {
        Expr::Await(Box::new(raw_inner_call))
    } else {
        raw_inner_call
    };

    // Build body: match unwrap for divergent, direct return for non-divergent
    let return_type = sig
        .return_type
        .clone()
        .map(|ty| ty.unwrap_promise())
        .and_then(|ty| {
            if matches!(ty, RustType::Unit) {
                None
            } else {
                Some(ty)
            }
        });

    let body_expr = if let (Some(ctx), Some(ret_ty)) = (wrap_ctx, &return_type) {
        // Divergent: match self.inner(...) { Variant(v) => v, _ => unreachable!() }
        let variant_name = ctx
            .variant_for(ret_ty)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "no variant found for return type {ret_ty:?} in union {}",
                    ctx.enum_name,
                )
            })?
            .to_string();
        Expr::Match {
            expr: Box::new(inner_call),
            arms: vec![
                crate::ir::MatchArm {
                    patterns: vec![crate::ir::Pattern::TupleStruct {
                        ctor: crate::ir::PatternCtor::UserEnumVariant {
                            enum_ty: crate::ir::UserTypeRef::new(&ctx.enum_name),
                            variant: variant_name,
                        },
                        fields: vec![crate::ir::Pattern::Binding {
                            name: "v".to_string(),
                            is_mut: false,
                            subpat: None,
                        }],
                    }],
                    guard: None,
                    body: vec![Stmt::TailExpr(Expr::Ident("v".to_string()))],
                },
                crate::ir::MatchArm {
                    patterns: vec![crate::ir::Pattern::Wildcard],
                    guard: None,
                    body: vec![Stmt::TailExpr(Expr::MacroCall {
                        name: "unreachable".to_string(),
                        args: vec![],
                        use_debug: vec![],
                    })],
                },
            ],
        }
    } else {
        // Non-divergent: self.inner(args...) directly
        inner_call
    };

    // Detect async from overload signature
    let sig_is_async = sig.return_type.as_ref().is_some_and(|ty| ty.is_promise());

    Ok(Method {
        vis: Visibility::Public,
        name: method_name,
        is_async: is_async && sig_is_async,
        has_self: true,
        has_mut_self: false,
        params,
        return_type,
        body: Some(vec![Stmt::TailExpr(body_expr)]),
    })
}

/// Wraps return expressions in the inner fn body with union variant constructors.
///
/// Walks `Stmt::Return(Some(expr))` and `Stmt::TailExpr(expr)` and applies
/// `wrap_expr_tail`. Recursively descends into all block-containing statement
/// structures (If, IfLet, While, ForIn, Loop, Match, LabeledBlock) to find
/// nested return statements.
///
/// Must mirror the SWC-side walk in `collect_stmt_return_leaf_types` to
/// maintain the positional invariant.
///
/// `types` is an iterator of pre-collected `ReturnLeafType` from
/// `collect_return_leaf_types`. Each leaf expression consumes one entry.
pub(super) fn wrap_body_returns(
    stmts: &mut [Stmt],
    types: &mut impl Iterator<Item = crate::transformer::return_wrap::ReturnLeafType>,
    ctx: &crate::transformer::return_wrap::ReturnWrapContext,
) -> anyhow::Result<()> {
    for stmt in stmts.iter_mut() {
        match stmt {
            Stmt::Return(Some(ref mut expr)) => {
                *expr = wrap_expr_tail(expr.clone(), types, ctx)?;
            }
            Stmt::TailExpr(ref mut expr) => {
                *expr = wrap_expr_tail(expr.clone(), types, ctx)?;
            }
            Stmt::If {
                then_body,
                else_body,
                ..
            }
            | Stmt::IfLet {
                then_body,
                else_body,
                ..
            } => {
                wrap_body_returns(then_body, types, ctx)?;
                if let Some(else_stmts) = else_body {
                    wrap_body_returns(else_stmts, types, ctx)?;
                }
            }
            Stmt::While { body, .. }
            | Stmt::WhileLet { body, .. }
            | Stmt::ForIn { body, .. }
            | Stmt::Loop { body, .. } => {
                wrap_body_returns(body, types, ctx)?;
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    wrap_body_returns(&mut arm.body, types, ctx)?;
                }
            }
            Stmt::LabeledBlock { body, .. } => {
                wrap_body_returns(body, types, ctx)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Recursively wraps an expression for return position.
///
/// For `Expr::If`/`Expr::IfLet` (ternary/narrowing), wraps each branch
/// individually (P6.3/P6.4). For leaf expressions, consumes one type from
/// the iterator and delegates to `wrap_leaf`.
fn wrap_expr_tail(
    expr: Expr,
    types: &mut impl Iterator<Item = crate::transformer::return_wrap::ReturnLeafType>,
    ctx: &crate::transformer::return_wrap::ReturnWrapContext,
) -> anyhow::Result<Expr> {
    match expr {
        // P6.3: ternary → wrap each branch
        Expr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            let wrapped_then = wrap_expr_tail(*then_expr, types, ctx)?;
            let wrapped_else = wrap_expr_tail(*else_expr, types, ctx)?;
            Ok(Expr::If {
                condition,
                then_expr: Box::new(wrapped_then),
                else_expr: Box::new(wrapped_else),
            })
        }
        // P6.4: IfLet at return position (P0.1: occurs in ternary narrowing)
        Expr::IfLet {
            pattern,
            expr: scrutinee,
            then_expr,
            else_expr,
        } => {
            let wrapped_then = wrap_expr_tail(*then_expr, types, ctx)?;
            let wrapped_else = wrap_expr_tail(*else_expr, types, ctx)?;
            Ok(Expr::IfLet {
                pattern,
                expr: scrutinee,
                then_expr: Box::new(wrapped_then),
                else_expr: Box::new(wrapped_else),
            })
        }
        // Leaf expression → consume type and wrap in variant
        leaf => {
            let leaf_type = types.next().ok_or_else(|| {
                anyhow::anyhow!(
                    "return leaf type iterator exhausted: SWC/IR positional invariant violated \
                     (more IR return leaves than SWC return leaves in {})",
                    ctx.enum_name,
                )
            })?;
            crate::transformer::return_wrap::wrap_leaf(
                leaf,
                leaf_type.ty.as_ref(),
                Some(leaf_type.span),
                ctx,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- wrap_delegate_arg (P7.2) ---

    #[test]
    fn wrap_delegate_arg_bare_when_types_match() {
        let result = wrap_delegate_arg("c", &RustType::String, &RustType::String);
        assert_eq!(result, Expr::Ident("c".to_string()));
    }

    #[test]
    fn wrap_delegate_arg_some_when_widest_is_option() {
        let result = wrap_delegate_arg(
            "key",
            &RustType::String,
            &RustType::Option(Box::new(RustType::String)),
        );
        match &result {
            Expr::FnCall { target, args } => {
                assert!(matches!(
                    target,
                    crate::ir::CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Some)
                ));
                assert_eq!(args.len(), 1);
                assert_eq!(args[0], Expr::Ident("key".to_string()));
            }
            _ => panic!("expected Some(key), got {result:?}"),
        }
    }

    #[test]
    fn wrap_delegate_arg_variant_when_widest_is_union() {
        let widest_ty = RustType::Named {
            name: "F64OrString".to_string(),
            type_args: vec![],
        };
        let result = wrap_delegate_arg("c", &RustType::String, &widest_ty);
        match &result {
            Expr::FnCall { target, args } => {
                assert!(
                    matches!(target, crate::ir::CallTarget::UserEnumVariantCtor { variant, .. } if variant == "String")
                );
                assert_eq!(args.len(), 1);
                assert_eq!(args[0], Expr::Ident("c".to_string()));
            }
            _ => panic!("expected F64OrString::String(c), got {result:?}"),
        }
    }

    #[test]
    fn wrap_delegate_arg_some_variant_when_widest_is_option_union() {
        let widest_ty = RustType::Option(Box::new(RustType::Named {
            name: "F64OrString".to_string(),
            type_args: vec![],
        }));
        let result = wrap_delegate_arg("c", &RustType::String, &widest_ty);
        // Should be Some(F64OrString::String(c))
        match &result {
            Expr::FnCall { target, args } => {
                assert!(matches!(
                    target,
                    crate::ir::CallTarget::BuiltinVariant(crate::ir::BuiltinVariant::Some)
                ));
                assert_eq!(args.len(), 1);
                match &args[0] {
                    Expr::FnCall { target, args } => {
                        assert!(
                            matches!(target, crate::ir::CallTarget::UserEnumVariantCtor { variant, .. } if variant == "String")
                        );
                        assert_eq!(args.len(), 1);
                        assert_eq!(args[0], Expr::Ident("c".to_string()));
                    }
                    _ => panic!("expected F64OrString::String(c), got {:?}", args[0]),
                }
            }
            _ => panic!("expected Some(F64OrString::String(c)), got {result:?}"),
        }
    }
}
