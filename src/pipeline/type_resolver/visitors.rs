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

use super::helpers::{lookup_array_element_type, unwrap_option_for_default};

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
            .map(|ty| ty.unwrap_promise())
            .filter(|ty| !matches!(ty, RustType::Unit))
            .map(|ty| wrap_trait_for_position(ty, TypePosition::Value, self.registry));
        let fn_type = RustType::Fn {
            params: params.iter().map(|(_, ty)| ty.clone()).collect(),
            return_type: Box::new(return_type.clone().unwrap_or(RustType::Unit)),
        };
        self.declare_var(&fn_name, ResolvedType::Known(fn_type), fn_span, false);

        self.enter_scope();

        // I-383 T2.A-ii: enter_type_param_scope also pushes the param names into
        // `synthetic.type_param_scope` so that synthetic types registered while walking
        // the body inherit the correct scope (avoiding dangling external ref leaks).
        let prev_state = self.push_type_param_constraints(fn_decl.function.type_params.as_deref());

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
        self.restore_type_param_constraints(prev_state);
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
            ast::Pat::Object(obj_pat) => {
                let source_rust_type = obj_pat.type_ann.as_ref().and_then(|ann| {
                    convert_ts_type(&ann.type_ann, self.synthetic, self.registry).ok()
                });
                self.register_pat_vars(pat, false, source_rust_type.as_ref());
                // Propagate expected types to default expressions within the pattern
                // using the type annotation (e.g., `{ color = "black" }: Options`).
                let source_type = source_rust_type
                    .map(ResolvedType::Known)
                    .unwrap_or(ResolvedType::Unknown);
                self.propagate_destructuring_defaults(pat, &source_type);
            }
            ast::Pat::Array(arr_pat) => {
                let source_rust_type = arr_pat.type_ann.as_ref().and_then(|ann| {
                    convert_ts_type(&ann.type_ann, self.synthetic, self.registry).ok()
                });
                self.register_pat_vars(pat, false, source_rust_type.as_ref());
                // Propagate expected types to default expressions within the pattern,
                // symmetric with the Object case above.
                let source_type = source_rust_type
                    .map(ResolvedType::Known)
                    .unwrap_or(ResolvedType::Unknown);
                self.propagate_destructuring_defaults(pat, &source_type);
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
            if let ast::Pat::Ident(ident) = &decl.name {
                // Simple variable declaration: full type resolution
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
                // Destructuring patterns: resolve source type, then register
                // variables with derived types and propagate defaults.
                let (init_type, source_rust_type) = if let Some(init) = &decl.init {
                    let init_span = Span::from_swc(init.span());
                    let init_type = self.resolve_expr(init);
                    self.result.expr_types.insert(init_span, init_type.clone());
                    let rust_type = match &init_type {
                        ResolvedType::Known(ty) => Some(ty.clone()),
                        ResolvedType::Unknown => None,
                    };
                    (init_type, rust_type)
                } else {
                    (ResolvedType::Unknown, None)
                };

                // Try type annotation on the pattern first, fall back to init type
                let ann_type = extract_type_ann_from_pat(&decl.name).and_then(|ann| {
                    convert_ts_type(&ann.type_ann, self.synthetic, self.registry).ok()
                });
                let effective_source = ann_type.as_ref().or(source_rust_type.as_ref());

                self.register_pat_vars(&decl.name, is_const, effective_source);
                // Use the same effective source for default propagation.
                // Type annotation takes precedence over init expression type.
                let effective_resolved = effective_source
                    .map(|t| ResolvedType::Known(t.clone()))
                    .unwrap_or(init_type);
                self.propagate_destructuring_defaults(&decl.name, &effective_resolved);
            }
        }
    }

    /// Registers variables from a destructuring pattern into the current scope.
    ///
    /// When `source_type` is provided, derives field/element types from it
    /// so that destructured variables are registered with correct types
    /// instead of `Unknown`.
    fn register_pat_vars(
        &mut self,
        pat: &ast::Pat,
        is_const: bool,
        source_type: Option<&RustType>,
    ) {
        match pat {
            ast::Pat::Ident(ident) => {
                // In nested contexts (KeyValue value, array element, rest arg),
                // the Ident is not handled by the main var_decl/param path.
                // register_pat_vars is never called for top-level Idents
                // (visit_var_decl and visit_param_pat handle them directly),
                // so any Pat::Ident reaching here is always a nested binding.
                let name = ident.id.sym.to_string();
                let span = Span::from_swc(ident.id.span);
                let ty = source_type
                    .map(|t| ResolvedType::Known(t.clone()))
                    .unwrap_or(ResolvedType::Unknown);
                self.declare_var(&name, ty, span, !is_const);
            }
            ast::Pat::Object(obj_pat) => {
                for prop in &obj_pat.props {
                    match prop {
                        ast::ObjectPatProp::KeyValue(kv) => {
                            let field_name = match &kv.key {
                                ast::PropName::Ident(id) => Some(id.sym.to_string()),
                                ast::PropName::Str(s) => {
                                    Some(s.value.to_string_lossy().into_owned())
                                }
                                _ => None,
                            };
                            let field_type = field_name.as_deref().and_then(|name| {
                                source_type.and_then(|st| self.lookup_struct_field(st, name))
                            });
                            self.register_pat_vars(&kv.value, is_const, field_type.as_ref());
                        }
                        ast::ObjectPatProp::Assign(assign) => {
                            let name = assign.key.sym.to_string();
                            let span = Span::from_swc(assign.key.span);
                            let field_type =
                                source_type.and_then(|st| self.lookup_struct_field(st, &name));
                            let var_type = match field_type {
                                Some(ty) if assign.value.is_some() => {
                                    ResolvedType::Known(unwrap_option_for_default(ty))
                                }
                                Some(ty) => ResolvedType::Known(ty),
                                None => ResolvedType::Unknown,
                            };
                            self.declare_var(&name, var_type, span, !is_const);
                        }
                        ast::ObjectPatProp::Rest(rest) => {
                            // Rest type computation (remaining fields) is complex;
                            // register with Unknown for now.
                            self.register_pat_vars(&rest.arg, is_const, None);
                        }
                    }
                }
            }
            ast::Pat::Array(arr_pat) => {
                for (i, elem) in arr_pat.elems.iter().enumerate() {
                    if let Some(elem_pat) = elem {
                        let elem_type = source_type.and_then(|st| lookup_array_element_type(st, i));
                        self.register_pat_vars(elem_pat, is_const, elem_type.as_ref());
                    }
                }
            }
            ast::Pat::Rest(rest) => {
                // Rest in array context: register with Unknown
                self.register_pat_vars(&rest.arg, is_const, None);
            }
            ast::Pat::Assign(assign) => {
                // Default value pattern (e.g., `{ x: y = 0 }` via KeyValue → Pat::Assign).
                // Unwrap Option<T> → T because the default replaces None.
                let unwrapped = source_type.map(|t| unwrap_option_for_default(t.clone()));
                self.register_pat_vars(&assign.left, is_const, unwrapped.as_ref());
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
        // I-383 T2.A-ii: also pushes names into `synthetic.type_param_scope`.
        let prev_state = if let Some(type_params) = &class.type_params {
            let (constraints, prev_scope) =
                enter_type_param_scope(type_params, self.synthetic, self.registry);
            let prev_constraints = std::mem::replace(&mut self.type_param_constraints, constraints);
            Some((prev_constraints, prev_scope))
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
        self.restore_type_param_constraints(prev_state);
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
            // I-383 T2.A-ii: also pushes method-level names into `synthetic.type_param_scope`
            // (append-merge with class scope active from `visit_class_body`).
            let prev_method_state =
                self.push_type_param_constraints(function.type_params.as_deref());
            for param in &function.params {
                self.visit_param_pat(&param.pat);
            }
            let prev_return_type = self.setup_fn_return_type(function.return_type.as_deref());
            for stmt in &body.stmts {
                self.visit_stmt(stmt);
            }
            self.current_fn_return_type = prev_return_type;
            self.restore_type_param_constraints(prev_method_state);
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
        let block_end = Span::from_swc(block.span).hi;
        let prev_block_end = self.current_block_end.replace(block_end);
        for stmt in &block.stmts {
            self.visit_stmt(stmt);
        }
        self.current_block_end = prev_block_end;
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
        use super::narrowing::block_always_exits;

        // Resolve test expression type
        let test_span = Span::from_swc(if_stmt.test.span());
        let test_type = self.resolve_expr(&if_stmt.test);
        self.result.expr_types.insert(test_span, test_type);

        // Detect narrowing guards (positive + complement in else)
        self.detect_narrowing_guard(&if_stmt.test, &if_stmt.cons, if_stmt.alt.as_deref());

        // Early return narrowing: if the then-block always exits and there's no else,
        // the complement type is valid for the rest of the enclosing block.
        if if_stmt.alt.is_none() && block_always_exits(&if_stmt.cons) {
            if let Some(block_end) = self.current_block_end {
                let if_end = if_stmt.cons.span().hi.0;
                self.detect_early_return_narrowing(&if_stmt.test, if_end, block_end);
            }
        }

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
