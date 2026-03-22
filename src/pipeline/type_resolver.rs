//! TypeResolver: pre-computes type information for all expressions in a file.
//!
//! Walks the AST independently of the Transformer, resolving expression types,
//! expected types, narrowing events, and variable mutability. The results are
//! stored in [`FileTypeResolution`] which the Transformer reads as immutable data.

use std::collections::HashMap;

use swc_common::Spanned;
use swc_ecma_ast as ast;

use crate::ir::RustType;
use crate::pipeline::type_converter::convert_ts_type;
use crate::pipeline::type_resolution::{FileTypeResolution, NarrowingEvent, Span, VarId};
use crate::pipeline::ModuleGraph;
use crate::pipeline::ResolvedType;
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{TypeDef, TypeRegistry};

/// Pre-computes type information for a single file.
///
/// The resolver walks the AST top-down, maintaining a scope stack for variable
/// types and a parent stack for expected type computation. It produces a
/// [`FileTypeResolution`] that the Transformer can query.
pub struct TypeResolver<'a> {
    registry: &'a TypeRegistry,
    synthetic: &'a mut SyntheticTypeRegistry,
    #[allow(dead_code)]
    module_graph: &'a ModuleGraph,

    // Internal state during resolution
    scope_stack: Vec<Scope>,
    current_fn_return_type: Option<RustType>,
    result: FileTypeResolution,
}

/// A scope containing variable bindings.
#[derive(Debug, Default)]
struct Scope {
    vars: HashMap<String, VarInfo>,
}

/// Information about a variable in scope.
#[derive(Debug, Clone)]
struct VarInfo {
    ty: ResolvedType,
    var_id: VarId,
}

impl<'a> TypeResolver<'a> {
    /// Creates a new TypeResolver.
    pub fn new(
        registry: &'a TypeRegistry,
        synthetic: &'a mut SyntheticTypeRegistry,
        module_graph: &'a ModuleGraph,
    ) -> Self {
        Self {
            registry,
            synthetic,
            module_graph,
            scope_stack: vec![Scope::default()],
            current_fn_return_type: None,
            result: FileTypeResolution::empty(),
        }
    }

    /// Resolves type information for an entire file.
    pub fn resolve_file(&mut self, file: &crate::pipeline::ParsedFile) -> FileTypeResolution {
        for item in &file.module.body {
            self.visit_module_item(item);
        }
        std::mem::replace(&mut self.result, FileTypeResolution::empty())
    }

    // --- Scope management ---

    fn enter_scope(&mut self) {
        self.scope_stack.push(Scope::default());
    }

    fn leave_scope(&mut self) {
        if self.scope_stack.len() > 1 {
            self.scope_stack.pop();
        }
    }

    fn declare_var(&mut self, name: &str, ty: ResolvedType, span: Span, mutable: bool) {
        let var_id = VarId {
            name: name.to_string(),
            declared_at: span,
        };
        self.result.var_mutability.insert(var_id.clone(), mutable);
        if let Some(scope) = self.scope_stack.last_mut() {
            scope.vars.insert(name.to_string(), VarInfo { ty, var_id });
        }
    }

    fn lookup_var(&self, name: &str) -> ResolvedType {
        for scope in self.scope_stack.iter().rev() {
            if let Some(info) = scope.vars.get(name) {
                return info.ty.clone();
            }
        }
        ResolvedType::Unknown
    }

    fn mark_var_mutable(&mut self, name: &str) {
        for scope in self.scope_stack.iter().rev() {
            if let Some(info) = scope.vars.get(name) {
                self.result.var_mutability.insert(info.var_id.clone(), true);
                return;
            }
        }
    }

    // --- AST visitors ---

    fn visit_module_item(&mut self, item: &ast::ModuleItem) {
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
        self.enter_scope();

        // Record return type for expected_types on return statements
        // Promise<T> → T, void → None (Rust omits `-> ()`)
        let prev_return_type = self.current_fn_return_type.take();
        if let Some(return_ann) = fn_decl.function.return_type.as_ref() {
            if let Ok(ty) = convert_ts_type(&return_ann.type_ann, self.synthetic, self.registry) {
                self.current_fn_return_type = unwrap_promise_and_unit(ty);
            }
        }

        // Register parameters in scope
        for param in &fn_decl.function.params {
            self.visit_param_pat(&param.pat);
        }

        // Visit body
        if let Some(body) = &fn_decl.function.body {
            self.visit_block_stmt(body);
        }

        self.current_fn_return_type = prev_return_type;
        self.leave_scope();
    }

    fn visit_param_pat(&mut self, pat: &ast::Pat) {
        if let ast::Pat::Ident(ident) = pat {
            let name = ident.id.sym.to_string();
            let span = Span::from_swc(ident.id.span);
            let ty = ident
                .type_ann
                .as_ref()
                .and_then(|ann| convert_ts_type(&ann.type_ann, self.synthetic, self.registry).ok())
                .map(ResolvedType::Known)
                .unwrap_or(ResolvedType::Unknown);
            self.declare_var(&name, ty, span, false);
        }
    }

    fn visit_var_decl(&mut self, var_decl: &ast::VarDecl) {
        let is_const = matches!(var_decl.kind, ast::VarDeclKind::Const);

        for decl in &var_decl.decls {
            // Resolve initializer expression regardless of pattern type
            if let Some(init) = &decl.init {
                let init_span = Span::from_swc(init.span());
                let init_type = self.resolve_expr(init);
                self.result.expr_types.insert(init_span, init_type);
            }

            // Register variables from the pattern
            self.register_pat_vars(&decl.name, is_const);

            if let ast::Pat::Ident(ident) = &decl.name {
                let name = ident.id.sym.to_string();
                let span = Span::from_swc(ident.id.span);

                // Resolve type from annotation or initializer
                let annotation_type = ident.type_ann.as_ref().and_then(|ann| {
                    convert_ts_type(&ann.type_ann, self.synthetic, self.registry).ok()
                });

                let init_type = decl.init.as_ref().map(|init| self.resolve_expr(init));

                let var_type = if let Some(ann_ty) = &annotation_type {
                    ResolvedType::Known(ann_ty.clone())
                } else if let Some(init_ty) = init_type {
                    init_ty
                } else {
                    ResolvedType::Unknown
                };

                // Set expected type for initializer
                if let (Some(ann_ty), Some(init)) = (&annotation_type, &decl.init) {
                    let init_span = Span::from_swc(init.span());
                    self.result.expected_types.insert(init_span, ann_ty.clone());
                }

                // Record expression type for initializer
                if let Some(init) = &decl.init {
                    let init_span = Span::from_swc(init.span());
                    let init_type = self.resolve_expr(init);
                    self.result.expr_types.insert(init_span, init_type);
                }

                // const + object type → mut (TS const allows field mutation)
                // let → initially false, mark_var_mutable() sets true on reassignment
                let mutable = is_const && is_object_type(&var_type);
                self.declare_var(&name, var_type, span, mutable);
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
        self.enter_scope();
        for member in &class_decl.class.body {
            match member {
                ast::ClassMember::Method(method) => {
                    if let Some(body) = &method.function.body {
                        self.enter_scope();
                        // Register parameters
                        for param in &method.function.params {
                            self.visit_param_pat(&param.pat);
                        }
                        // Set return type (Promise<T> → T, void → None)
                        let prev_return_type = self.current_fn_return_type.take();
                        if let Some(return_ann) = &method.function.return_type {
                            if let Ok(ty) =
                                convert_ts_type(&return_ann.type_ann, self.synthetic, self.registry)
                            {
                                self.current_fn_return_type = unwrap_promise_and_unit(ty);
                            }
                        }
                        // Walk body
                        for stmt in &body.stmts {
                            self.visit_stmt(stmt);
                        }
                        self.current_fn_return_type = prev_return_type;
                        self.leave_scope();
                    }
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
                    if let Some(init) = &prop.value {
                        let span = Span::from_swc(init.span());
                        let ty = self.resolve_expr(init);
                        self.result.expr_types.insert(span, ty);
                    }
                }
                _ => {}
            }
        }
        self.leave_scope();
    }

    fn visit_block_stmt(&mut self, block: &ast::BlockStmt) {
        self.enter_scope();
        for stmt in &block.stmts {
            self.visit_stmt(stmt);
        }
        self.leave_scope();
    }

    fn visit_stmt(&mut self, stmt: &ast::Stmt) {
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
                    let ty = self.resolve_expr(arg);
                    self.result.expr_types.insert(span, ty);

                    // Set expected type from function return type
                    if let Some(return_ty) = &self.current_fn_return_type {
                        self.result.expected_types.insert(span, return_ty.clone());
                    }
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
                self.result.expr_types.insert(span, ty);
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
        self.detect_narrowing_guard(&if_stmt.test, &if_stmt.cons);

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

    // --- Narrowing detection ---

    fn detect_narrowing_guard(&mut self, test: &ast::Expr, consequent: &ast::Stmt) {
        let scope_span = Span::from_swc(consequent.span());

        // typeof x === "string"
        if let ast::Expr::Bin(bin) = test {
            if matches!(bin.op, ast::BinaryOp::EqEqEq | ast::BinaryOp::EqEq) {
                if let Some((var_name, narrowed_type)) = self.extract_typeof_narrowing(bin) {
                    self.result.narrowing_events.push(NarrowingEvent {
                        scope_start: scope_span.lo,
                        scope_end: scope_span.hi,
                        var_name,
                        narrowed_type,
                    });
                }
            }
            // x !== null
            if matches!(bin.op, ast::BinaryOp::NotEqEq | ast::BinaryOp::NotEq) {
                if let Some((var_name, narrowed_type)) = self.extract_null_check_narrowing(bin) {
                    self.result.narrowing_events.push(NarrowingEvent {
                        scope_start: scope_span.lo,
                        scope_end: scope_span.hi,
                        var_name,
                        narrowed_type,
                    });
                }
            }
        }

        // x instanceof Foo
        if let ast::Expr::Bin(bin) = test {
            if matches!(bin.op, ast::BinaryOp::InstanceOf) {
                if let (ast::Expr::Ident(var_ident), ast::Expr::Ident(class_ident)) =
                    (bin.left.as_ref(), bin.right.as_ref())
                {
                    self.result.narrowing_events.push(NarrowingEvent {
                        scope_start: scope_span.lo,
                        scope_end: scope_span.hi,
                        var_name: var_ident.sym.to_string(),
                        narrowed_type: RustType::Named {
                            name: class_ident.sym.to_string(),
                            type_args: vec![],
                        },
                    });
                }
            }
        }
    }

    fn extract_typeof_narrowing(&self, bin: &ast::BinExpr) -> Option<(String, RustType)> {
        // typeof x === "string" → (x, String)
        let (typeof_expr, type_str) = self.extract_typeof_and_string(bin)?;
        let var_name = match typeof_expr {
            ast::Expr::Ident(ident) => ident.sym.to_string(),
            _ => return None,
        };
        let narrowed_type = match type_str.as_str() {
            "string" => RustType::String,
            "number" => RustType::F64,
            "boolean" => RustType::Bool,
            _ => return None,
        };
        Some((var_name, narrowed_type))
    }

    fn extract_typeof_and_string<'b>(
        &self,
        bin: &'b ast::BinExpr,
    ) -> Option<(&'b ast::Expr, String)> {
        // typeof x === "string"
        if let ast::Expr::Unary(unary) = bin.left.as_ref() {
            if matches!(unary.op, ast::UnaryOp::TypeOf) {
                if let ast::Expr::Lit(ast::Lit::Str(s)) = bin.right.as_ref() {
                    return Some((&unary.arg, s.value.to_string_lossy().into_owned()));
                }
            }
        }
        // "string" === typeof x
        if let ast::Expr::Unary(unary) = bin.right.as_ref() {
            if matches!(unary.op, ast::UnaryOp::TypeOf) {
                if let ast::Expr::Lit(ast::Lit::Str(s)) = bin.left.as_ref() {
                    return Some((&unary.arg, s.value.to_string_lossy().into_owned()));
                }
            }
        }
        None
    }

    fn extract_null_check_narrowing(&self, bin: &ast::BinExpr) -> Option<(String, RustType)> {
        // x !== null → remove Option wrapper from x's type
        let (var_expr, is_null) = if is_null_literal(&bin.right) {
            (bin.left.as_ref(), true)
        } else if is_null_literal(&bin.left) {
            (bin.right.as_ref(), true)
        } else {
            return None;
        };

        if !is_null {
            return None;
        }

        let var_name = match var_expr {
            ast::Expr::Ident(ident) => ident.sym.to_string(),
            _ => return None,
        };

        // Get current type and unwrap Option
        let current_type = self.lookup_var(&var_name);
        match current_type {
            ResolvedType::Known(RustType::Option(inner)) => {
                Some((var_name, inner.as_ref().clone()))
            }
            _ => None,
        }
    }

    // --- Expression type resolution ---

    fn resolve_expr(&mut self, expr: &ast::Expr) -> ResolvedType {
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
                }
                self.resolve_expr(&assign.right)
            }
            ast::Expr::Cond(cond) => {
                // Ternary: resolve both branches, prefer non-Unknown
                let cons = self.resolve_expr(&cond.cons);
                if !matches!(cons, ResolvedType::Unknown) {
                    cons
                } else {
                    self.resolve_expr(&cond.alt)
                }
            }
            ast::Expr::Unary(unary) => match unary.op {
                ast::UnaryOp::TypeOf => ResolvedType::Known(RustType::String),
                ast::UnaryOp::Bang => ResolvedType::Known(RustType::Bool),
                ast::UnaryOp::Minus | ast::UnaryOp::Plus => ResolvedType::Known(RustType::F64),
                _ => ResolvedType::Unknown,
            },
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
                convert_ts_type(&assertion.type_ann, self.synthetic, self.registry)
                    .map(ResolvedType::Known)
                    .unwrap_or(ResolvedType::Unknown)
            }
            ast::Expr::TsConstAssertion(const_assertion) => {
                // x as const — return inner expression's type
                self.resolve_expr(&const_assertion.expr)
            }
            ast::Expr::Object(obj) => {
                // Walk property values to resolve their types
                for prop in &obj.props {
                    match prop {
                        ast::PropOrSpread::Prop(prop) => {
                            if let ast::Prop::KeyValue(kv) = prop.as_ref() {
                                let span = Span::from_swc(kv.value.span());
                                let ty = self.resolve_expr(&kv.value);
                                self.result.expr_types.insert(span, ty);
                            }
                        }
                        ast::PropOrSpread::Spread(spread) => {
                            let span = Span::from_swc(spread.expr.span());
                            let ty = self.resolve_expr(&spread.expr);
                            self.result.expr_types.insert(span, ty);
                        }
                    }
                }
                // Object literal's own type depends on expected type context
                ResolvedType::Unknown
            }
            ast::Expr::OptChain(opt) => {
                // Optional chaining: x?.y — resolve the base, return Unknown for safety
                // The actual type depends on whether the base is None/Some
                if let ast::OptChainBase::Member(member) = &*opt.base {
                    let obj_type = self.resolve_expr(&member.obj);
                    let _ = obj_type; // Walk for side effects (registering expr_types)
                }
                ResolvedType::Unknown
            }
            ast::Expr::Update(_) => {
                // i++ / i-- → f64
                ResolvedType::Known(RustType::F64)
            }
            ast::Expr::This(_) => {
                // `this` — type depends on class context; Unknown for now
                ResolvedType::Unknown
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
            _ => ResolvedType::Unknown,
        }
    }

    fn resolve_bin_expr(&mut self, bin: &ast::BinExpr) -> ResolvedType {
        use ast::BinaryOp::*;
        match bin.op {
            Lt | LtEq | Gt | GtEq | EqEq | NotEq | EqEqEq | NotEqEq | In | InstanceOf => {
                ResolvedType::Known(RustType::Bool)
            }
            Add => {
                let left_ty = self.resolve_expr(&bin.left);
                if matches!(left_ty, ResolvedType::Known(RustType::String)) {
                    return ResolvedType::Known(RustType::String);
                }
                let right_ty = self.resolve_expr(&bin.right);
                if matches!(right_ty, ResolvedType::Known(RustType::String)) {
                    return ResolvedType::Known(RustType::String);
                }
                ResolvedType::Known(RustType::F64)
            }
            Sub | Mul | Div | Mod | Exp | BitAnd | BitOr | BitXor | LShift | RShift
            | ZeroFillRShift => ResolvedType::Known(RustType::F64),
            LogicalAnd | LogicalOr | NullishCoalescing => {
                let right = self.resolve_expr(&bin.right);
                if !matches!(right, ResolvedType::Unknown) {
                    right
                } else {
                    self.resolve_expr(&bin.left)
                }
            }
        }
    }

    fn resolve_member_expr(&mut self, member: &ast::MemberExpr) -> ResolvedType {
        let obj_type = self.resolve_expr(&member.obj);
        let obj_rust_type = match &obj_type {
            ResolvedType::Known(ty) => ty,
            ResolvedType::Unknown => return ResolvedType::Unknown,
        };

        // Array/tuple indexing
        if let ast::MemberProp::Computed(computed) = &member.prop {
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
        let field_name = match &member.prop {
            ast::MemberProp::Ident(ident) => ident.sym.to_string(),
            _ => return ResolvedType::Unknown,
        };

        // Special case: .length on String/Vec
        if field_name == "length" && matches!(obj_rust_type, RustType::String | RustType::Vec(_)) {
            return ResolvedType::Known(RustType::F64);
        }

        // Lookup in TypeRegistry
        let (type_name, type_args) = match obj_rust_type {
            RustType::Named { name, type_args } => (name.as_str(), type_args.as_slice()),
            _ => return ResolvedType::Unknown,
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

    fn resolve_call_expr(&mut self, call: &ast::CallExpr) -> ResolvedType {
        let callee = match &call.callee {
            ast::Callee::Expr(expr) => expr.as_ref(),
            _ => return ResolvedType::Unknown,
        };

        // Set expected types for arguments based on callee's parameter types
        self.set_call_arg_expected_types(callee, &call.args);

        match callee {
            ast::Expr::Ident(ident) => {
                let fn_name = ident.sym.to_string();
                // Check scope for Fn type
                if let ResolvedType::Known(RustType::Fn { return_type, .. }) =
                    self.lookup_var(&fn_name)
                {
                    return ResolvedType::Known(return_type.as_ref().clone());
                }
                // Check TypeRegistry
                if let Some(TypeDef::Function { return_type, .. }) = self.registry.get(&fn_name) {
                    return ResolvedType::Known(return_type.clone().unwrap_or(RustType::Unit));
                }
                ResolvedType::Unknown
            }
            ast::Expr::Member(member) => {
                let obj_type = self.resolve_expr(&member.obj);
                let obj_rust_type = match &obj_type {
                    ResolvedType::Known(ty) => ty,
                    ResolvedType::Unknown => return ResolvedType::Unknown,
                };
                let method_name = match &member.prop {
                    ast::MemberProp::Ident(ident) => ident.sym.to_string(),
                    _ => return ResolvedType::Unknown,
                };
                self.resolve_method_return_type(obj_rust_type, &method_name)
            }
            _ => ResolvedType::Unknown,
        }
    }

    /// Sets expected types for function call arguments based on the callee's parameter types.
    fn set_call_arg_expected_types(&mut self, callee: &ast::Expr, args: &[ast::ExprOrSpread]) {
        let param_types: Option<Vec<RustType>> = match callee {
            ast::Expr::Ident(ident) => {
                let fn_name = ident.sym.to_string();
                // Check TypeRegistry for function parameter types
                if let Some(TypeDef::Function { params, .. }) = self.registry.get(&fn_name) {
                    Some(params.iter().map(|(_, ty)| ty.clone()).collect())
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(param_types) = param_types {
            for (arg, param_ty) in args.iter().zip(param_types.iter()) {
                let arg_span = Span::from_swc(arg.expr.span());
                self.result
                    .expected_types
                    .insert(arg_span, param_ty.clone());
            }
        }
    }

    fn resolve_method_return_type(&self, obj_type: &RustType, method_name: &str) -> ResolvedType {
        let (type_name, type_args) = match obj_type {
            RustType::String => ("String", &[] as &[RustType]),
            RustType::Vec(_) => ("Vec", &[] as &[RustType]),
            RustType::Named { name, type_args } => (name.as_str(), type_args.as_slice()),
            _ => return ResolvedType::Unknown,
        };

        let type_def = if type_args.is_empty() {
            self.registry.get(type_name).cloned()
        } else {
            self.registry.instantiate(type_name, type_args)
        };

        match &type_def {
            Some(TypeDef::Struct { methods, .. }) => methods
                .get(method_name)
                .and_then(|sig| sig.return_type.clone())
                .map(ResolvedType::Known)
                .unwrap_or(ResolvedType::Unknown),
            _ => ResolvedType::Unknown,
        }
    }

    fn resolve_new_expr(&self, new_expr: &ast::NewExpr) -> ResolvedType {
        let class_name = match new_expr.callee.as_ref() {
            ast::Expr::Ident(ident) => ident.sym.to_string(),
            _ => return ResolvedType::Unknown,
        };

        if self.registry.get(&class_name).is_some() {
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

        // Save and set return type (Promise<T> → T, void → None)
        let prev_return_type = self.current_fn_return_type.take();
        if let Some(return_ann) = &arrow.return_type {
            if let Ok(ty) = convert_ts_type(&return_ann.type_ann, self.synthetic, self.registry) {
                self.current_fn_return_type = unwrap_promise_and_unit(ty);
            }
        }

        // Register parameters
        let mut param_types = Vec::new();
        for param in &arrow.params {
            if let ast::Pat::Ident(ident) = param {
                let name = ident.id.sym.to_string();
                let span = Span::from_swc(ident.id.span);
                let ty = ident.type_ann.as_ref().and_then(|ann| {
                    convert_ts_type(&ann.type_ann, self.synthetic, self.registry).ok()
                });
                let resolved = ty
                    .clone()
                    .map(ResolvedType::Known)
                    .unwrap_or(ResolvedType::Unknown);
                self.declare_var(&name, resolved, span, false);
                param_types.push(ty.unwrap_or(RustType::Any));
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
                self.current_fn_return_type = unwrap_promise_and_unit(ty);
            }
        }

        for param in &fn_expr.function.params {
            self.visit_param_pat(&param.pat);
        }

        if let Some(body) = &fn_expr.function.body {
            for stmt in &body.stmts {
                self.visit_stmt(stmt);
            }
        }

        self.current_fn_return_type = prev_return_type;
        self.leave_scope();

        ResolvedType::Unknown // Function expression type is complex; return Unknown for now
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

fn is_null_literal(expr: &ast::Expr) -> bool {
    matches!(expr, ast::Expr::Lit(ast::Lit::Null(_)))
}

/// Returns true if the resolved type is an object type (struct, named, vec, etc.).
/// Used for const-mut detection: TypeScript's `const` allows field mutation on objects.
fn is_object_type(ty: &ResolvedType) -> bool {
    match ty {
        ResolvedType::Known(rust_type) => matches!(
            rust_type,
            RustType::Named { .. } | RustType::Vec(_) | RustType::Tuple(_) | RustType::Any
        ),
        ResolvedType::Unknown => false,
    }
}

/// Promise<T> → T に展開し、Unit（void）は None にする。
/// TypeResolver が expected_type / return_type として登録する前に適用する。
fn unwrap_promise_and_unit(ty: RustType) -> Option<RustType> {
    let unwrapped = match &ty {
        RustType::Named { name, type_args } if name == "Promise" && type_args.len() == 1 => {
            type_args[0].clone()
        }
        _ => ty,
    };
    if matches!(unwrapped, RustType::Unit) {
        None
    } else {
        Some(unwrapped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::module_graph::ModuleGraph;
    use crate::pipeline::{parse_files, SyntheticTypeRegistry};
    use crate::registry::build_registry;
    use std::path::PathBuf;

    fn resolve(source: &str) -> FileTypeResolution {
        let files = parse_files(vec![(PathBuf::from("test.ts"), source.to_string())]).unwrap();
        let file = &files.files[0];
        let reg = build_registry(&file.module);
        let mut synthetic = SyntheticTypeRegistry::new();
        let module_graph = ModuleGraph::empty();
        let mut resolver = TypeResolver::new(&reg, &mut synthetic, &module_graph);
        resolver.resolve_file(file)
    }

    #[test]
    fn test_resolve_const_with_type_annotation() {
        let res = resolve("const x: number = 42;");
        // The initializer `42` should have type f64
        assert!(
            !res.expr_types.is_empty(),
            "should have at least one expr type"
        );
        // Check that at least one entry is Known(F64)
        let has_f64 = res
            .expr_types
            .values()
            .any(|t| matches!(t, ResolvedType::Known(RustType::F64)));
        assert!(has_f64, "initializer 42 should resolve to f64");
    }

    #[test]
    fn test_resolve_let_string_literal() {
        let res = resolve(r#"let y = "hello";"#);
        let has_string = res
            .expr_types
            .values()
            .any(|t| matches!(t, ResolvedType::Known(RustType::String)));
        assert!(has_string, "string literal should resolve to String");
    }

    #[test]
    fn test_resolve_let_with_reassignment_is_mutable() {
        let res = resolve("let z = 1; z = 2;");
        let is_mut = res.var_mutability.values().any(|&m| m);
        assert!(is_mut, "reassigned variable should be mutable");
    }

    #[test]
    fn test_resolve_const_is_not_mutable() {
        let res = resolve("const x = 1;");
        let all_immutable = res.var_mutability.values().all(|&m| !m);
        assert!(all_immutable, "const variable should not be mutable");
    }

    #[test]
    fn test_resolve_function_param_type() {
        let res = resolve("function foo(x: string): number { return 0; }");
        // x should be in scope as String
        // return 0 should have expected type f64
        let has_expected = !res.expected_types.is_empty();
        assert!(has_expected, "return statement should have expected type");
    }

    #[test]
    fn test_resolve_expected_type_var_decl() {
        let res = resolve("const x: number = 42;");
        let has_expected = res
            .expected_types
            .values()
            .any(|t| matches!(t, RustType::F64));
        assert!(
            has_expected,
            "initializer should have expected type from annotation"
        );
    }

    #[test]
    fn test_resolve_expected_type_return_stmt() {
        let res = resolve("function foo(): string { return 42; }");
        let has_string_expected = res
            .expected_types
            .values()
            .any(|t| matches!(t, RustType::String));
        assert!(
            has_string_expected,
            "return expression should have expected type String"
        );
    }

    #[test]
    fn test_narrowing_typeof_string() {
        let res = resolve(
            r#"
            function foo(x: any) {
                if (typeof x === "string") {
                    console.log(x);
                }
            }
            "#,
        );
        let has_string_narrowing = res
            .narrowing_events
            .iter()
            .any(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::String));
        assert!(
            has_string_narrowing,
            "typeof guard should create String narrowing event"
        );
    }

    #[test]
    fn test_narrowing_instanceof() {
        let res = resolve(
            r#"
            function foo(x: any) {
                if (x instanceof Error) {
                    console.log(x);
                }
            }
            "#,
        );
        let has_error_narrowing = res.narrowing_events.iter().any(|e| {
            e.var_name == "x"
                && matches!(&e.narrowed_type, RustType::Named { name, .. } if name == "Error")
        });
        assert!(
            has_error_narrowing,
            "instanceof guard should create Error narrowing event"
        );
    }

    #[test]
    fn test_narrowing_null_check() {
        let res = resolve(
            r#"
            function foo(x: string | null) {
                if (x !== null) {
                    console.log(x);
                }
            }
            "#,
        );
        let has_non_null_narrowing = res
            .narrowing_events
            .iter()
            .any(|e| e.var_name == "x" && matches!(e.narrowed_type, RustType::String));
        assert!(
            has_non_null_narrowing,
            "null check should narrow Option<String> to String"
        );
    }

    #[test]
    fn test_unknown_expr() {
        let res = resolve("const x = unknownFunc();");
        let has_unknown = res
            .expr_types
            .values()
            .any(|t| matches!(t, ResolvedType::Unknown));
        assert!(has_unknown, "unknown function call should be Unknown");
    }

    #[test]
    fn test_binary_add_string_context() {
        let res = resolve(r#"const x = "hello" + " world";"#);
        let has_string = res
            .expr_types
            .values()
            .any(|t| matches!(t, ResolvedType::Known(RustType::String)));
        assert!(has_string, "string + string should resolve to String");
    }

    #[test]
    fn test_resolve_member_access_field() {
        let res = resolve(
            r#"
            interface Foo { name: string; }
            function bar(f: Foo) { return f.name; }
            "#,
        );
        let has_string = res
            .expr_types
            .values()
            .any(|t| matches!(t, ResolvedType::Known(RustType::String)));
        assert!(has_string, "f.name should resolve to String");
    }

    #[test]
    fn test_expected_type_call_arg() {
        let res = resolve(
            r#"
            function greet(name: string): void {}
            greet("hello");
            "#,
        );
        // The argument "hello" at the call site should have expected_type = String
        let has_string_expected = res
            .expected_types
            .values()
            .any(|t| matches!(t, RustType::String));
        assert!(
            has_string_expected,
            "call argument should have expected type String from parameter"
        );
    }

    #[test]
    fn test_mutability_let_without_assign() {
        let res = resolve("let z = 1;");
        // z is declared with let but never reassigned
        let all_immutable = res.var_mutability.values().all(|&m| !m);
        assert!(
            all_immutable,
            "let without reassignment should not be mutable"
        );
    }

    #[test]
    fn test_synthetic_registration_in_body() {
        let files = parse_files(vec![(
            PathBuf::from("test.ts"),
            "function foo() { const x: string | number = 42; }".to_string(),
        )])
        .unwrap();
        let file = &files.files[0];
        let reg = build_registry(&file.module);
        let mut synthetic = SyntheticTypeRegistry::new();
        let module_graph = ModuleGraph::empty();
        let mut resolver = TypeResolver::new(&reg, &mut synthetic, &module_graph);
        let _res = resolver.resolve_file(file);
        // The union type string | number in the body should have registered a synthetic enum
        assert!(
            !synthetic.all_items().is_empty(),
            "body union type annotation should register synthetic enum"
        );
    }

    #[test]
    fn test_resolve_arrow_body() {
        let res = resolve(
            r#"
            const f = (x: string) => x.length;
            "#,
        );
        // Arrow body should be walked; x.length should be in expr_types
        let has_f64 = res
            .expr_types
            .values()
            .any(|t| matches!(t, ResolvedType::Known(RustType::F64)));
        assert!(
            has_f64,
            "arrow body expression x.length should resolve to f64"
        );
    }

    #[test]
    fn test_resolve_arrow_param_type() {
        let res = resolve(
            r#"
            const greet = (name: string) => name;
            "#,
        );
        // name should be resolved to String inside the arrow body
        let has_string = res
            .expr_types
            .values()
            .any(|t| matches!(t, ResolvedType::Known(RustType::String)));
        assert!(
            has_string,
            "arrow param name should resolve to String in body"
        );
    }

    #[test]
    fn test_resolve_array_literal_numbers() {
        let res = resolve("const arr = [1, 2, 3];");
        let has_vec_f64 = res
            .expr_types
            .values()
            .any(|t| matches!(t, ResolvedType::Known(RustType::Vec(inner)) if matches!(inner.as_ref(), RustType::F64)));
        assert!(has_vec_f64, "[1, 2, 3] should resolve to Vec<f64>");
    }

    #[test]
    fn test_resolve_array_literal_strings() {
        let res = resolve(r#"const arr = ["a", "b"];"#);
        let has_vec_string = res
            .expr_types
            .values()
            .any(|t| matches!(t, ResolvedType::Known(RustType::Vec(inner)) if matches!(inner.as_ref(), RustType::String)));
        assert!(
            has_vec_string,
            r#"["a", "b"] should resolve to Vec<String>"#
        );
    }

    #[test]
    fn test_resolve_array_literal_empty() {
        let res = resolve("const arr = [];");
        let has_unknown = res
            .expr_types
            .values()
            .any(|t| matches!(t, ResolvedType::Unknown));
        assert!(has_unknown, "[] should resolve to Unknown");
    }

    #[test]
    fn test_resolve_class_method_body() {
        let res = resolve(
            r#"
            class Foo {
                bar(x: number): string {
                    return "hello";
                }
            }
            "#,
        );
        // "hello" inside the class method body should be resolved
        let has_string = res
            .expr_types
            .values()
            .any(|t| matches!(t, ResolvedType::Known(RustType::String)));
        assert!(has_string, "class method body should be walked");
    }

    #[test]
    fn test_resolve_class_constructor() {
        let res = resolve(
            r#"
            class Foo {
                constructor(x: number) {
                    const y = x;
                }
            }
            "#,
        );
        // x inside constructor should be f64
        let has_f64 = res
            .expr_types
            .values()
            .any(|t| matches!(t, ResolvedType::Known(RustType::F64)));
        assert!(
            has_f64,
            "constructor body should be walked and params registered"
        );
    }

    #[test]
    fn test_resolve_method_call_return_type() {
        // Register a type with a method in TypeRegistry
        let files = parse_files(vec![(
            PathBuf::from("test.ts"),
            r#"
            interface Greeter { greet(): string; }
            function use_greeter(g: Greeter) { return g.greet(); }
            "#
            .to_string(),
        )])
        .unwrap();
        let file = &files.files[0];
        let reg = build_registry(&file.module);
        let mut synthetic = SyntheticTypeRegistry::new();
        let module_graph = ModuleGraph::empty();
        let mut resolver = TypeResolver::new(&reg, &mut synthetic, &module_graph);
        let res = resolver.resolve_file(file);

        // g.greet() should resolve to String (from Greeter.greet return type)
        let has_string = res
            .expr_types
            .values()
            .any(|t| matches!(t, ResolvedType::Known(RustType::String)));
        assert!(has_string, "method call g.greet() should resolve to String");
    }

    #[test]
    fn test_resolve_string_length() {
        let res = resolve(
            r#"
            function foo(s: string) { return s.length; }
            "#,
        );
        let has_f64 = res
            .expr_types
            .values()
            .any(|t| matches!(t, ResolvedType::Known(RustType::F64)));
        assert!(has_f64, "s.length on String should resolve to f64");
    }

    #[test]
    fn test_resolve_object_literal_values() {
        let res = resolve(r#"const obj = { x: 42, y: "hello" };"#);
        let has_f64 = res
            .expr_types
            .values()
            .any(|t| matches!(t, ResolvedType::Known(RustType::F64)));
        let has_string = res
            .expr_types
            .values()
            .any(|t| matches!(t, ResolvedType::Known(RustType::String)));
        assert!(has_f64, "object literal value 42 should be resolved");
        assert!(
            has_string,
            "object literal value 'hello' should be resolved"
        );
    }

    #[test]
    fn test_resolve_destructuring_object() {
        let res = resolve(
            r#"
            const obj = { x: 1, y: 2 };
            const { x, y } = obj;
            "#,
        );
        // x and y should be registered as variables (Unknown type since no annotation)
        // The key test is that this doesn't crash
        assert!(
            !res.var_mutability.is_empty(),
            "destructured variables should be registered"
        );
    }

    #[test]
    fn test_resolve_throw_stmt() {
        let res = resolve(
            r#"
            function foo() {
                throw new Error("fail");
            }
            "#,
        );
        // The throw expression should be walked
        assert!(
            !res.expr_types.is_empty(),
            "throw expression should be resolved"
        );
    }
}
