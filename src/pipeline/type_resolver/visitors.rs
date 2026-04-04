//! AST visitor methods for TypeResolver.
//!
//! Walks module items, declarations, statements, and control flow structures,
//! dispatching to specialized resolvers (narrowing, expected types, expressions).

use swc_common::Spanned;
use swc_ecma_ast as ast;

use super::*;
use crate::pipeline::type_converter::convert_ts_type;
use crate::pipeline::type_resolution::Span;
use crate::transformer::type_position::{wrap_trait_for_position, TypePosition};

/// Extracts the type annotation from any pattern variant.
///
/// For `Pat::Assign`, recurses into the left-side pattern (the annotation
/// is on the inner ident/object/array, not on the AssignPat itself).
pub(super) fn extract_type_ann_from_pat(pat: &ast::Pat) -> Option<&ast::TsTypeAnn> {
    match pat {
        ast::Pat::Ident(ident) => ident.type_ann.as_deref(),
        ast::Pat::Object(obj) => obj.type_ann.as_deref(),
        ast::Pat::Array(arr) => arr.type_ann.as_deref(),
        ast::Pat::Assign(assign) => extract_type_ann_from_pat(&assign.left),
        _ => None,
    }
}

impl<'a> TypeResolver<'a> {
    /// Extracts parameter name and type from a pattern, handling default values.
    ///
    /// Returns `(name, type)` for `Pat::Ident` and `Pat::Assign` wrapping `Pat::Ident`.
    /// Returns `None` for destructuring patterns (which don't have a single name).
    fn extract_param_name_and_type(&mut self, pat: &ast::Pat) -> Option<(String, RustType)> {
        match pat {
            ast::Pat::Ident(ident) => {
                let name = ident.id.sym.to_string();
                let ty = ident
                    .type_ann
                    .as_ref()
                    .and_then(|ann| {
                        convert_ts_type(&ann.type_ann, self.synthetic, self.registry).ok()
                    })
                    .map(|ty| wrap_trait_for_position(ty, TypePosition::Param, self.registry))
                    .unwrap_or(RustType::Any);
                Some((name, ty))
            }
            ast::Pat::Assign(assign) => self.extract_param_name_and_type(&assign.left),
            _ => None,
        }
    }
    pub(super) fn visit_module_item(&mut self, item: &ast::ModuleItem) {
        match item {
            ast::ModuleItem::Stmt(stmt) => self.visit_stmt(stmt),
            ast::ModuleItem::ModuleDecl(ast::ModuleDecl::ExportDecl(export)) => {
                self.visit_decl(&export.decl);
            }
            _ => {}
        }
    }

    fn visit_decl(&mut self, decl: &ast::Decl) {
        match decl {
            ast::Decl::Fn(fn_decl) => self.visit_fn_decl(fn_decl),
            ast::Decl::Var(var_decl) => self.visit_var_decl(var_decl),
            ast::Decl::Class(class_decl) => self.visit_class_decl(class_decl),
            _ => {} // TsInterface, TsTypeAlias, TsEnum handled by TypeCollector
        }
    }

    fn visit_fn_decl(&mut self, fn_decl: &ast::FnDecl) {
        // Register function name in current scope (before enter_scope) so that
        // the function is visible to sibling statements (TS hoisting semantics).
        let fn_name = fn_decl.ident.sym.to_string();
        let fn_span = Span::from_swc(fn_decl.ident.span);

        let params: Vec<(String, RustType)> = fn_decl
            .function
            .params
            .iter()
            .filter_map(|p| self.extract_param_name_and_type(&p.pat))
            .collect();
        let return_type = fn_decl
            .function
            .return_type
            .as_ref()
            .and_then(|ann| convert_ts_type(&ann.type_ann, self.synthetic, self.registry).ok())
            .and_then(unwrap_promise_and_unit)
            .map(|ty| wrap_trait_for_position(ty, TypePosition::Value, self.registry));
        let fn_type = RustType::Fn {
            params: params.iter().map(|(_, ty)| ty.clone()).collect(),
            return_type: Box::new(return_type.clone().unwrap_or(RustType::Unit)),
        };
        self.declare_var(&fn_name, ResolvedType::Known(fn_type), fn_span, false);

        self.enter_scope();

        // Register type parameter constraints after entering scope.
        // Merge with parent constraints so nested generics can access outer type params.
        let prev_constraints = if let Some(type_params) = &fn_decl.function.type_params {
            let inner_constraints =
                collect_type_param_constraints(type_params, self.synthetic, self.registry);
            let mut merged = self.type_param_constraints.clone();
            merged.extend(inner_constraints);
            let prev = std::mem::replace(&mut self.type_param_constraints, merged);
            Some(prev)
        } else {
            None
        };

        // Record return type for expected_types on return statements
        // Promise<T> → T, void → None (Rust omits `-> ()`)
        let prev_return_type = self.current_fn_return_type.take();
        self.current_fn_return_type = return_type;

        // Register parameters in scope
        for param in &fn_decl.function.params {
            self.visit_param_pat(&param.pat);
        }

        // Visit body
        if let Some(body) = &fn_decl.function.body {
            self.visit_block_stmt(body);
        }

        self.current_fn_return_type = prev_return_type;
        if let Some(prev) = prev_constraints {
            self.type_param_constraints = prev;
        }
        self.leave_scope();
    }

    pub(super) fn visit_param_pat(&mut self, pat: &ast::Pat) {
        match pat {
            ast::Pat::Ident(ident) => {
                let name = ident.id.sym.to_string();
                let span = Span::from_swc(ident.id.span);
                let ty = ident
                    .type_ann
                    .as_ref()
                    .and_then(|ann| {
                        convert_ts_type(&ann.type_ann, self.synthetic, self.registry).ok()
                    })
                    .map(|ty| wrap_trait_for_position(ty, TypePosition::Param, self.registry))
                    .map(ResolvedType::Known)
                    .unwrap_or(ResolvedType::Unknown);
                self.declare_var(&name, ty, span, false);
            }
            ast::Pat::Assign(assign) => {
                // Default value parameter: `x: Type = defaultExpr`
                // 1. Register the left-side variable(s) in scope
                self.visit_param_pat(&assign.left);
                // 2. Extract type annotation from the left pattern
                let ann_ty = extract_type_ann_from_pat(&assign.left).and_then(|ann| {
                    convert_ts_type(&ann.type_ann, self.synthetic, self.registry)
                        .ok()
                        .map(|ty| wrap_trait_for_position(ty, TypePosition::Param, self.registry))
                });
                // 3. Set expected type on the default value expression and propagate
                //    Resolve type params so that `T extends Options` becomes `Options`
                if let Some(ref ty) = ann_ty {
                    let resolved = self.resolve_type_params_in_type(ty);
                    let rhs_span = Span::from_swc(assign.right.span());
                    self.result
                        .expected_types
                        .insert(rhs_span, resolved.clone());
                    self.propagate_expected(&assign.right, &resolved);
                }
                // 4. Resolve the default value expression
                let rhs_span = Span::from_swc(assign.right.span());
                let rhs_ty = self.resolve_expr(&assign.right);
                self.result.expr_types.insert(rhs_span, rhs_ty);
            }
            ast::Pat::Object(_) | ast::Pat::Array(_) => {
                // Destructuring parameters: register nested variables as Unknown.
                // TODO: extract field types from type annotation and register with
                // proper types (e.g., `{ x, y }: Point` → x: f64, y: f64).
                self.register_pat_vars(pat, false);
            }
            ast::Pat::Rest(rest) => {
                self.visit_param_pat(&rest.arg);
            }
            _ => {}
        }
    }

    pub(super) fn visit_var_decl(&mut self, var_decl: &ast::VarDecl) {
        let is_const = matches!(var_decl.kind, ast::VarDeclKind::Const);

        for decl in &var_decl.decls {
            // Register variables from the pattern
            self.register_pat_vars(&decl.name, is_const);

            if let ast::Pat::Ident(ident) = &decl.name {
                let name = ident.id.sym.to_string();
                let span = Span::from_swc(ident.id.span);

                // Resolve type from annotation (Value position: trait → Box<dyn Trait>)
                let annotation_type = ident.type_ann.as_ref().and_then(|ann| {
                    convert_ts_type(&ann.type_ann, self.synthetic, self.registry)
                        .ok()
                        .map(|ty| wrap_trait_for_position(ty, TypePosition::Value, self.registry))
                });

                // Set expected type BEFORE resolving the initializer, so that
                // resolve_arrow_expr / resolve_fn_expr can read it from the map.
                // Resolve type params so generic types use constraint types.
                if let (Some(ann_ty), Some(init)) = (&annotation_type, &decl.init) {
                    let resolved = self.resolve_type_params_in_type(ann_ty);
                    let init_span = Span::from_swc(init.span());
                    self.result
                        .expected_types
                        .insert(init_span, resolved.clone());
                    self.propagate_expected(init, &resolved);
                }

                // Resolve initializer once (after expected types are set)
                let init_type = decl.init.as_ref().map(|init| {
                    let init_span = Span::from_swc(init.span());
                    let ty = self.resolve_expr(init);
                    self.result.expr_types.insert(init_span, ty.clone());
                    ty
                });

                let var_type = if let Some(ann_ty) = &annotation_type {
                    ResolvedType::Known(ann_ty.clone())
                } else if let Some(init_ty) = init_type {
                    init_ty
                } else {
                    ResolvedType::Unknown
                };

                // const + object type → mut (TS const allows field mutation)
                // let → initially false, mark_var_mutable() sets true on reassignment
                let mutable = is_const && is_object_type(&var_type);
                // Register Fn type on the variable's Ident span so that
                // get_expr_type(ident) returns the Fn type directly
                if matches!(&var_type, ResolvedType::Known(RustType::Fn { .. })) {
                    self.result.expr_types.insert(span, var_type.clone());
                }
                self.declare_var(&name, var_type, span, mutable);
            } else {
                // Non-ident patterns (destructuring): resolve init without expected type
                if let Some(init) = &decl.init {
                    let init_span = Span::from_swc(init.span());
                    let init_type = self.resolve_expr(init);
                    self.result.expr_types.insert(init_span, init_type);
                }
            }
        }
    }

    /// Registers variables from a destructuring pattern into the current scope.
    fn register_pat_vars(&mut self, pat: &ast::Pat, is_const: bool) {
        match pat {
            ast::Pat::Ident(_) => {} // Handled by the main var_decl path
            ast::Pat::Object(obj_pat) => {
                for prop in &obj_pat.props {
                    match prop {
                        ast::ObjectPatProp::KeyValue(kv) => {
                            self.register_pat_vars(&kv.value, is_const);
                        }
                        ast::ObjectPatProp::Assign(assign) => {
                            let name = assign.key.sym.to_string();
                            let span = Span::from_swc(assign.key.span);
                            self.declare_var(&name, ResolvedType::Unknown, span, !is_const);
                        }
                        ast::ObjectPatProp::Rest(rest) => {
                            self.register_pat_vars(&rest.arg, is_const);
                        }
                    }
                }
            }
            ast::Pat::Array(arr_pat) => {
                for elem in arr_pat.elems.iter().flatten() {
                    self.register_pat_vars(elem, is_const);
                }
            }
            ast::Pat::Rest(rest) => {
                self.register_pat_vars(&rest.arg, is_const);
            }
            ast::Pat::Assign(assign) => {
                // Default value pattern: { x = 0 } — register the left side
                self.register_pat_vars(&assign.left, is_const);
            }
            _ => {}
        }
    }

    fn visit_class_decl(&mut self, class_decl: &ast::ClassDecl) {
        let class_name = class_decl.ident.sym.to_string();
        let class_span = Span::from_swc(class_decl.ident.span);
        self.visit_class_body(&class_decl.class, &class_name, class_span);
    }

    /// Walks a class body (shared by class declarations and class expressions).
    ///
    /// Registers `this` as the class's Named type at class scope, so that
    /// `this.field` / `this.method()` can be resolved via TypeRegistry.
    /// Static methods shadow `this` with Unknown to prevent incorrect resolution.
    pub(super) fn visit_class_body(
        &mut self,
        class: &ast::Class,
        class_name: &str,
        class_span: Span,
    ) {
        self.enter_scope();

        // Register class type parameter constraints after entering scope
        // (consistent with visit_fn_decl / resolve_arrow_expr / resolve_fn_expr)
        let prev_constraints = if let Some(type_params) = &class.type_params {
            let constraints =
                collect_type_param_constraints(type_params, self.synthetic, self.registry);
            let prev = std::mem::replace(&mut self.type_param_constraints, constraints);
            Some(prev)
        } else {
            None
        };

        // Register `this` as the class's Named type.
        let this_type = ResolvedType::Known(RustType::Named {
            name: class_name.to_string(),
            type_args: vec![],
        });
        if let Some(scope) = self.scope_stack.last_mut() {
            scope.vars.insert(
                "this".to_string(),
                VarInfo {
                    ty: this_type,
                    var_id: VarId {
                        name: "this".to_string(),
                        declared_at: class_span,
                    },
                },
            );
        }
        for member in &class.body {
            match member {
                ast::ClassMember::Method(m) => {
                    self.visit_method_function(&m.function, m.is_static, class_span);
                }
                ast::ClassMember::PrivateMethod(pm) => {
                    self.visit_method_function(&pm.function, pm.is_static, class_span);
                }
                ast::ClassMember::Constructor(ctor) => {
                    if let Some(body) = &ctor.body {
                        self.enter_scope();
                        for param in &ctor.params {
                            if let ast::ParamOrTsParamProp::Param(param) = param {
                                self.visit_param_pat(&param.pat);
                            }
                        }
                        for stmt in &body.stmts {
                            self.visit_stmt(stmt);
                        }
                        self.leave_scope();
                    }
                }
                ast::ClassMember::ClassProp(prop) => {
                    self.visit_class_prop_init(prop.value.as_deref(), prop.type_ann.as_deref());
                }
                ast::ClassMember::PrivateProp(pp) => {
                    self.visit_class_prop_init(pp.value.as_deref(), pp.type_ann.as_deref());
                }
                _ => {}
            }
        }
        if let Some(prev) = prev_constraints {
            self.type_param_constraints = prev;
        }
        self.leave_scope();
    }

    /// Visits a class method body (shared by Method and PrivateMethod).
    fn visit_method_function(
        &mut self,
        function: &ast::Function,
        is_static: bool,
        class_span: Span,
    ) {
        if let Some(body) = &function.body {
            self.enter_scope();
            // Static methods don't have `this` bound to the instance.
            if is_static {
                if let Some(scope) = self.scope_stack.last_mut() {
                    scope.vars.insert(
                        "this".to_string(),
                        VarInfo {
                            ty: ResolvedType::Unknown,
                            var_id: VarId {
                                name: "this".to_string(),
                                declared_at: class_span,
                            },
                        },
                    );
                }
            }
            // Merge method type params with class type params (method params shadow class params).
            // Without merging, `resolve_type_params_in_type` can't resolve class-level
            // type params (e.g., T in `class Foo<T extends Base>`) inside method bodies.
            let prev_method_constraints = if let Some(type_params) = &function.type_params {
                let method_constraints =
                    collect_type_param_constraints(type_params, self.synthetic, self.registry);
                let mut merged = self.type_param_constraints.clone();
                merged.extend(method_constraints);
                let prev = std::mem::replace(&mut self.type_param_constraints, merged);
                Some(prev)
            } else {
                None
            };
            for param in &function.params {
                self.visit_param_pat(&param.pat);
            }
            let prev_return_type = self.current_fn_return_type.take();
            if let Some(return_ann) = &function.return_type {
                if let Ok(ty) = convert_ts_type(&return_ann.type_ann, self.synthetic, self.registry)
                {
                    self.current_fn_return_type = unwrap_promise_and_unit(ty)
                        .map(|ty| wrap_trait_for_position(ty, TypePosition::Value, self.registry));
                }
            }
            for stmt in &body.stmts {
                self.visit_stmt(stmt);
            }
            self.current_fn_return_type = prev_return_type;
            if let Some(prev) = prev_method_constraints {
                self.type_param_constraints = prev;
            }
            self.leave_scope();
        }
    }

    /// Visits a class property initializer (shared by ClassProp and PrivateProp).
    fn visit_class_prop_init(
        &mut self,
        init: Option<&ast::Expr>,
        type_ann: Option<&ast::TsTypeAnn>,
    ) {
        if let Some(init) = init {
            let span = Span::from_swc(init.span());
            // Set expected type BEFORE resolving the initializer, so that
            // resolve_expr can use it (same pattern as visit_var_decl).
            if let Some(type_ann) = type_ann {
                if let Ok(raw_ty) =
                    convert_ts_type(&type_ann.type_ann, self.synthetic, self.registry)
                {
                    let ann_ty =
                        wrap_trait_for_position(raw_ty, TypePosition::Value, self.registry);
                    let resolved = self.resolve_type_params_in_type(&ann_ty);
                    self.result.expected_types.insert(span, resolved.clone());
                    self.propagate_expected(init, &resolved);
                }
            }
            let ty = self.resolve_expr(init);
            self.result.expr_types.insert(span, ty);
        }
    }

    pub(super) fn visit_block_stmt(&mut self, block: &ast::BlockStmt) {
        self.enter_scope();
        for stmt in &block.stmts {
            self.visit_stmt(stmt);
        }
        self.leave_scope();
    }

    pub(super) fn visit_stmt(&mut self, stmt: &ast::Stmt) {
        match stmt {
            ast::Stmt::Decl(decl) => self.visit_decl(decl),
            ast::Stmt::Expr(expr_stmt) => {
                let span = Span::from_swc(expr_stmt.expr.span());
                let ty = self.resolve_expr(&expr_stmt.expr);
                self.result.expr_types.insert(span, ty);
            }
            ast::Stmt::Return(ret) => {
                if let Some(arg) = &ret.arg {
                    let span = Span::from_swc(arg.span());

                    // Set expected type BEFORE resolving the expression, so that
                    // anonymous struct generation in resolve_expr_inner can see
                    // that an expected type exists and avoid unnecessary generation.
                    if let Some(return_ty) = self.current_fn_return_type.clone() {
                        let resolved = self.resolve_type_params_in_type(&return_ty);
                        self.result.expected_types.insert(span, resolved.clone());
                        self.propagate_expected(arg, &resolved);
                    }

                    let ty = self.resolve_expr(arg);
                    self.result.expr_types.insert(span, ty);
                }
            }
            ast::Stmt::If(if_stmt) => self.visit_if_stmt(if_stmt),
            ast::Stmt::Block(block) => self.visit_block_stmt(block),
            ast::Stmt::For(for_stmt) => {
                self.enter_scope();
                if let Some(ast::VarDeclOrExpr::VarDecl(var_decl)) = &for_stmt.init {
                    self.visit_var_decl(var_decl);
                }
                if let Some(body) = for_stmt.body.as_block() {
                    for s in &body.stmts {
                        self.visit_stmt(s);
                    }
                }
                self.leave_scope();
            }
            ast::Stmt::ForOf(for_of) => {
                self.enter_scope();
                if let Some(body) = for_of.body.as_block() {
                    for s in &body.stmts {
                        self.visit_stmt(s);
                    }
                }
                self.leave_scope();
            }
            ast::Stmt::ForIn(for_in) => {
                self.enter_scope();
                if let Some(body) = for_in.body.as_block() {
                    for s in &body.stmts {
                        self.visit_stmt(s);
                    }
                }
                self.leave_scope();
            }
            ast::Stmt::While(while_stmt) => {
                // Resolve condition to register expr_types for assignment patterns
                self.resolve_expr(&while_stmt.test);
                if let Some(body) = while_stmt.body.as_block() {
                    self.visit_block_stmt(body);
                }
            }
            ast::Stmt::Try(try_stmt) => {
                self.visit_block_stmt(&try_stmt.block);
                if let Some(handler) = &try_stmt.handler {
                    self.visit_block_stmt(&handler.body);
                }
                if let Some(finalizer) = &try_stmt.finalizer {
                    self.visit_block_stmt(finalizer);
                }
            }
            ast::Stmt::Switch(switch_stmt) => {
                let span = Span::from_swc(switch_stmt.discriminant.span());
                let ty = self.resolve_expr(&switch_stmt.discriminant);
                self.result.expr_types.insert(span, ty.clone());
                // Propagate discriminant type to case test values (Named types only).
                // String/F64 etc. are NOT propagated — string case values should remain
                // as literal patterns, not get .to_string() added.
                if let ResolvedType::Known(ref rust_ty @ RustType::Named { ref name, .. }) = ty {
                    if self.registry.get(name).is_some() {
                        for case in &switch_stmt.cases {
                            if let Some(test) = &case.test {
                                let test_span = Span::from_swc(test.span());
                                self.result
                                    .expected_types
                                    .insert(test_span, rust_ty.clone());
                            }
                        }
                    }
                }
                // Detect DU switch and record field bindings
                self.detect_du_switch_bindings(switch_stmt);
                for case in &switch_stmt.cases {
                    for s in &case.cons {
                        self.visit_stmt(s);
                    }
                }
            }
            ast::Stmt::DoWhile(do_while) => {
                if let Some(body) = do_while.body.as_block() {
                    self.visit_block_stmt(body);
                }
            }
            ast::Stmt::Throw(throw_stmt) => {
                let span = Span::from_swc(throw_stmt.arg.span());
                let ty = self.resolve_expr(&throw_stmt.arg);
                self.result.expr_types.insert(span, ty);
            }
            ast::Stmt::Labeled(labeled) => {
                self.visit_stmt(&labeled.body);
            }
            _ => {}
        }
    }

    fn visit_if_stmt(&mut self, if_stmt: &ast::IfStmt) {
        // Resolve test expression type
        let test_span = Span::from_swc(if_stmt.test.span());
        let test_type = self.resolve_expr(&if_stmt.test);
        self.result.expr_types.insert(test_span, test_type);

        // Detect narrowing guards
        self.detect_narrowing_guard(&if_stmt.test, &if_stmt.cons, if_stmt.alt.as_deref());

        // Visit then branch
        match if_stmt.cons.as_ref() {
            ast::Stmt::Block(block) => self.visit_block_stmt(block),
            other => self.visit_stmt(other),
        }

        // Visit else branch
        if let Some(alt) = &if_stmt.alt {
            match alt.as_ref() {
                ast::Stmt::Block(block) => self.visit_block_stmt(block),
                other => self.visit_stmt(other),
            }
        }
    }
}
