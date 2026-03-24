//! Class declaration conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC class declarations into IR [`Item::Struct`] + [`Item::Impl`].

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{AssocConst, Expr, Item, Method, Param, RustType, Stmt, StructField, Visibility};
use crate::pipeline::type_converter::convert_ts_type;
use crate::pipeline::SyntheticTypeRegistry;
use crate::transformer::context::TransformContext;
use crate::transformer::expressions::convert_expr;
use crate::transformer::extract_prop_name;
use crate::transformer::functions::convert_last_return_to_tail;
use crate::transformer::statements::convert_stmt;
use crate::transformer::{Transformer, TypeEnv};

/// Extracted class information for resolving inheritance relationships.
#[derive(Debug, Clone)]
pub struct ClassInfo {
    /// Class name
    pub name: String,
    /// Parent class name (from `extends`)
    pub parent: Option<String>,
    /// Struct fields
    pub fields: Vec<StructField>,
    /// Constructor method (if any)
    pub constructor: Option<Method>,
    /// Regular methods (excluding constructor)
    pub methods: Vec<Method>,
    /// Visibility
    pub vis: Visibility,
    /// Interface names from `implements` clause
    pub implements: Vec<String>,
    /// Whether this class is abstract
    pub is_abstract: bool,
    /// Static properties (converted to associated constants)
    pub static_consts: Vec<AssocConst>,
}

impl<'a> Transformer<'a> {
    /// Extracts [`ClassInfo`] from an SWC class declaration without generating IR items.
    ///
    /// Used in the first pass to collect class metadata for inheritance resolution.
    pub(crate) fn extract_class_info(
        &mut self,
        class_decl: &ast::ClassDecl,
        vis: Visibility,
    ) -> Result<ClassInfo> {
        let name = class_decl.ident.sym.to_string();
        let parent = class_decl.class.super_class.as_ref().and_then(|sc| {
            if let ast::Expr::Ident(ident) = sc.as_ref() {
                Some(ident.sym.to_string())
            } else {
                None
            }
        });

        let implements: Vec<String> = class_decl
            .class
            .implements
            .iter()
            .filter_map(|impl_clause| {
                if let ast::Expr::Ident(ident) = impl_clause.expr.as_ref() {
                    Some(ident.sym.to_string())
                } else {
                    None
                }
            })
            .collect();

        let mut fields = Vec::new();
        let mut static_consts = Vec::new();
        let mut constructor = None;
        let mut methods = Vec::new();

        for member in &class_decl.class.body {
            match member {
                ast::ClassMember::ClassProp(prop) if prop.is_static => {
                    if let Some(ac) = self.convert_static_prop(prop, &vis)? {
                        static_consts.push(ac);
                    }
                }
                ast::ClassMember::ClassProp(prop) => {
                    fields.push(self.convert_class_prop(prop, &vis)?);
                }
                ast::ClassMember::Constructor(ctor) => {
                    let (method, param_prop_fields) =
                        self.convert_constructor(ctor, &vis)?;
                    constructor = Some(method);
                    fields.extend(param_prop_fields);
                }
                ast::ClassMember::Method(method) => {
                    methods.push(self.convert_class_method(method, &vis)?);
                }
                _ => {}
            }
        }

        Ok(ClassInfo {
            name,
            parent,
            fields,
            constructor,
            methods,
            vis,
            implements,
            is_abstract: class_decl.class.is_abstract,
            static_consts,
        })
    }

    /// Converts an SWC [`ast::ClassDecl`] into IR items (struct + impl).
    ///
    /// Properties become struct fields, methods become impl methods,
    /// and `constructor` becomes `pub fn new() -> Self`.
    ///
    /// # Errors
    ///
    /// Returns an error if unsupported class members are encountered.
    pub(crate) fn convert_class_decl(
        &mut self,
        class_decl: &ast::ClassDecl,
        vis: Visibility,
    ) -> Result<Vec<Item>> {
        let info = self.extract_class_info(class_decl, vis)?;
        if info.is_abstract {
            generate_abstract_class_items(&info)
        } else {
            generate_items_for_class(&info, None)
        }
    }
}

// --- Helpers for class item generation ---

/// Creates an `Item::Struct` from a name, visibility, and fields.
pub(crate) fn make_struct(name: &str, vis: &Visibility, fields: Vec<StructField>) -> Item {
    Item::Struct {
        vis: vis.clone(),
        name: name.to_string(),
        type_params: vec![],
        fields,
    }
}

/// Strips visibility from methods for use in trait impl blocks.
pub(crate) fn strip_method_visibility(methods: &[Method]) -> Vec<Method> {
    methods
        .iter()
        .map(|m| Method {
            vis: Visibility::Private,
            ..m.clone()
        })
        .collect()
}

/// Creates an `Item::Impl` block from constants, constructor, and/or methods.
///
/// Returns `None` if constants, constructor, and methods are all empty.
pub(crate) fn make_impl(
    struct_name: &str,
    for_trait: Option<&str>,
    consts: Vec<AssocConst>,
    ctor: Option<&Method>,
    methods: Vec<Method>,
) -> Option<Item> {
    let mut all_methods = Vec::new();
    if let Some(c) = ctor {
        all_methods.push(c.clone());
    }
    all_methods.extend(methods);

    if all_methods.is_empty() && consts.is_empty() {
        return None;
    }

    Some(Item::Impl {
        struct_name: struct_name.to_string(),
        for_trait: for_trait.map(|s| s.to_string()),
        consts,
        methods: all_methods,
    })
}

/// Generates IR items for a class, optionally with a parent class for inheritance.
///
/// When `parent` is `Some`, the class is treated as a child:
/// - Parent fields are copied to the child struct
/// - A trait is generated from the parent's methods
/// - Both parent and child get trait impls
pub fn generate_items_for_class(info: &ClassInfo, parent: Option<&ClassInfo>) -> Result<Vec<Item>> {
    match parent {
        None => generate_standalone_class(info),
        Some(parent_info) => generate_child_class(info, parent_info),
    }
}

/// Generates items for a parent class that is extended by another class.
///
/// Produces: struct + trait + impl (constructor) + impl trait for struct
pub fn generate_parent_class_items(info: &ClassInfo) -> Result<Vec<Item>> {
    let trait_name = format!("{}Trait", info.name);
    let mut items = vec![make_struct(&info.name, &info.vis, info.fields.clone())];

    // Trait with method signatures (no bodies)
    let trait_methods: Vec<Method> = info
        .methods
        .iter()
        .map(|m| Method {
            vis: Visibility::Private,
            body: None,
            ..m.clone()
        })
        .collect();
    items.push(Item::Trait {
        vis: info.vis.clone(),
        name: trait_name.clone(),
        type_params: vec![],
        supertraits: vec![],
        methods: trait_methods,
        associated_types: vec![],
    });

    // impl (constructor + static consts)
    items.extend(make_impl(
        &info.name,
        None,
        info.static_consts.clone(),
        info.constructor.as_ref(),
        vec![],
    ));

    // impl Trait for Struct (method bodies)
    items.extend(make_impl(
        &info.name,
        Some(&trait_name),
        vec![],
        None,
        strip_method_visibility(&info.methods),
    ));

    Ok(items)
}

/// Generates IR items for an abstract class as a trait.
///
/// Abstract methods become trait method signatures (no body).
/// Concrete methods become default implementations.
pub fn generate_abstract_class_items(info: &ClassInfo) -> Result<Vec<Item>> {
    let methods: Vec<Method> = info
        .methods
        .iter()
        .map(|m| Method {
            vis: Visibility::Private,
            has_self: true,
            has_mut_self: m.has_mut_self,
            ..m.clone()
        })
        .collect();

    Ok(vec![Item::Trait {
        vis: info.vis.clone(),
        name: info.name.clone(),
        type_params: vec![],
        supertraits: vec![],
        methods,
        associated_types: vec![],
    }])
}

/// Generates IR items for a concrete class that extends an abstract class.
///
/// Produces: struct + impl (constructor) + impl Trait for Struct (methods)
pub fn generate_child_of_abstract(
    info: &ClassInfo,
    abstract_trait_name: &str,
) -> Result<Vec<Item>> {
    let mut items = vec![make_struct(&info.name, &info.vis, info.fields.clone())];

    items.extend(make_impl(
        &info.name,
        None,
        info.static_consts.clone(),
        info.constructor.as_ref(),
        vec![],
    ));
    items.extend(make_impl(
        &info.name,
        Some(abstract_trait_name),
        vec![],
        None,
        strip_method_visibility(&info.methods),
    ));

    Ok(items)
}

/// Generates IR items for a standalone class (no inheritance).
fn generate_standalone_class(info: &ClassInfo) -> Result<Vec<Item>> {
    let mut items = vec![make_struct(&info.name, &info.vis, info.fields.clone())];

    items.extend(make_impl(
        &info.name,
        None,
        info.static_consts.clone(),
        info.constructor.as_ref(),
        info.methods.clone(),
    ));

    Ok(items)
}

/// Generates IR items for a child class that extends a parent.
///
/// Produces: struct (with parent fields) + impl (constructor + own methods) + impl trait
fn generate_child_class(info: &ClassInfo, parent: &ClassInfo) -> Result<Vec<Item>> {
    let trait_name = format!("{}Trait", parent.name);

    // Struct with parent + child fields
    let mut fields = parent.fields.clone();
    fields.extend(info.fields.clone());
    let mut items = vec![make_struct(&info.name, &info.vis, fields)];

    // impl (rewritten constructor + own methods + static consts)
    let ctor = info
        .constructor
        .as_ref()
        .map(|c| rewrite_super_constructor(c, parent))
        .transpose()?;
    items.extend(make_impl(
        &info.name,
        None,
        info.static_consts.clone(),
        ctor.as_ref(),
        info.methods.clone(),
    ));

    // impl ParentTrait for Child (parent method bodies)
    items.extend(make_impl(
        &info.name,
        Some(&trait_name),
        vec![],
        None,
        strip_method_visibility(&parent.methods),
    ));

    Ok(items)
}

/// Generates IR items for a child class that also implements interfaces.
///
/// Produces: struct (parent+child fields) + impl Child + impl ParentTrait for Child
/// + impl Interface for Child (per interface).
pub fn generate_child_class_with_implements(
    info: &ClassInfo,
    parent: Option<&ClassInfo>,
    iface_methods: &std::collections::HashMap<String, Vec<String>>,
) -> Result<Vec<Item>> {
    // Start with child class items (struct + impl + impl ParentTrait)
    let mut items = generate_items_for_class(info, parent)?;

    // Add interface trait impls by moving matching methods from impl Child to impl Interface
    let mut claimed_methods: std::collections::HashSet<String> = std::collections::HashSet::new();

    for iface_name in &info.implements {
        if let Some(method_names) = iface_methods.get(iface_name) {
            let trait_methods = strip_method_visibility(
                &info
                    .methods
                    .iter()
                    .filter(|m| method_names.contains(&m.name))
                    .cloned()
                    .collect::<Vec<_>>(),
            );

            if !trait_methods.is_empty() {
                for m in &trait_methods {
                    claimed_methods.insert(m.name.clone());
                }
                items.extend(make_impl(
                    &info.name,
                    Some(iface_name),
                    vec![],
                    None,
                    trait_methods,
                ));
            }
        }
    }

    // Remove claimed methods from the own impl block
    if !claimed_methods.is_empty() {
        for item in &mut items {
            if let Item::Impl {
                struct_name,
                for_trait: None,
                methods,
                ..
            } = item
            {
                if struct_name == &info.name {
                    methods.retain(|m| !claimed_methods.contains(&m.name));
                }
            }
        }
    }

    Ok(items)
}

/// Generates IR items for a class that implements one or more interfaces.
///
/// Methods matching interface method names go into `impl Trait for Struct`.
/// Remaining methods (including constructor) go into `impl Struct`.
pub fn generate_class_with_implements(
    info: &ClassInfo,
    iface_methods: &std::collections::HashMap<String, Vec<String>>,
) -> Result<Vec<Item>> {
    let mut items = vec![make_struct(&info.name, &info.vis, info.fields.clone())];

    let mut claimed_methods: std::collections::HashSet<String> = std::collections::HashSet::new();

    for iface_name in &info.implements {
        if let Some(method_names) = iface_methods.get(iface_name) {
            let trait_methods = strip_method_visibility(
                &info
                    .methods
                    .iter()
                    .filter(|m| method_names.contains(&m.name))
                    .cloned()
                    .collect::<Vec<_>>(),
            );

            if !trait_methods.is_empty() {
                for m in &trait_methods {
                    claimed_methods.insert(m.name.clone());
                }
                items.extend(make_impl(
                    &info.name,
                    Some(iface_name),
                    vec![],
                    None,
                    trait_methods,
                ));
            }
        }
    }

    let unclaimed: Vec<_> = info
        .methods
        .iter()
        .filter(|m| !claimed_methods.contains(&m.name))
        .cloned()
        .collect();
    items.extend(make_impl(
        &info.name,
        None,
        info.static_consts.clone(),
        info.constructor.as_ref(),
        unclaimed,
    ));

    Ok(items)
}

/// Rewrites a child constructor to handle `super()` calls.
///
/// `super(args)` in the child constructor is removed, and the parent's field
/// initialization pattern from the constructor arguments is applied.
fn rewrite_super_constructor(ctor: &Method, parent: &ClassInfo) -> Result<Method> {
    let mut new_body = Vec::new();
    let mut super_fields = Vec::new();

    // Extract super() call arguments and map to parent fields
    let body_stmts = ctor.body.as_deref().unwrap_or(&[]);
    for stmt in body_stmts {
        if let Some(args) = try_extract_super_call(stmt) {
            if args.len() != parent.fields.len() {
                return Err(anyhow!(
                    "super() has {} arguments but parent '{}' has {} fields",
                    args.len(),
                    parent.name,
                    parent.fields.len(),
                ));
            }
            for (field, arg) in parent.fields.iter().zip(args.iter()) {
                super_fields.push((field.name.clone(), arg.clone()));
            }
        } else {
            new_body.push(stmt.clone());
        }
    }

    // Build Self { parent_fields..., child_fields... } at the end
    // If the body ends with a TailExpr(StructInit) or Return(StructInit), merge super fields into it
    let has_struct_init = new_body.iter().any(|s| {
        matches!(
            s,
            Stmt::TailExpr(Expr::StructInit { .. }) | Stmt::Return(Some(Expr::StructInit { .. }))
        )
    });

    if has_struct_init {
        // Merge super fields into existing StructInit
        new_body = new_body
            .into_iter()
            .map(|s| match s {
                Stmt::TailExpr(Expr::StructInit {
                    name, mut fields, ..
                }) => {
                    let mut merged = super_fields.clone();
                    merged.append(&mut fields);
                    Stmt::TailExpr(Expr::StructInit {
                        name,
                        fields: merged,
                        base: None,
                    })
                }
                Stmt::Return(Some(Expr::StructInit {
                    name, mut fields, ..
                })) => {
                    let mut merged = super_fields.clone();
                    merged.append(&mut fields);
                    Stmt::Return(Some(Expr::StructInit {
                        name,
                        fields: merged,
                        base: None,
                    }))
                }
                other => other,
            })
            .collect();
    } else if !super_fields.is_empty() {
        // No existing StructInit — create one with super fields
        new_body.push(Stmt::TailExpr(Expr::StructInit {
            name: "Self".to_string(),
            fields: super_fields,
            base: None,
        }));
    }

    Ok(Method {
        body: Some(new_body),
        ..ctor.clone()
    })
}

/// Tries to extract arguments from a `super(args)` call statement.
fn try_extract_super_call(stmt: &Stmt) -> Option<Vec<Expr>> {
    match stmt {
        Stmt::Expr(Expr::FnCall { name, args }) if name == "super" => Some(args.clone()),
        _ => None,
    }
}

impl<'a> Transformer<'a> {
    /// Converts a static class property to an associated constant.
    ///
    /// Returns `None` if the property has no initializer (cannot become a const without a value).
    fn convert_static_prop(
        &mut self,
        prop: &ast::ClassProp,
        vis: &Visibility,
    ) -> Result<Option<AssocConst>> {
        let reg = self.reg();
        let name = extract_prop_name(&prop.key)
            .map_err(|_| anyhow!("unsupported static property key (only identifiers)"))?;

        let type_ann = prop
            .type_ann
            .as_ref()
            .ok_or_else(|| anyhow!("static property '{}' has no type annotation", name))?;
        let ty = convert_ts_type(&type_ann.type_ann, self.synthetic, reg)?;

        let value = match &prop.value {
            Some(init) => convert_expr(init, self.tctx, &TypeEnv::new(), self.synthetic)?,
            None => return Ok(None), // No initializer — skip
        };

        Ok(Some(AssocConst {
            vis: vis.clone(),
            name,
            ty,
            value,
        }))
    }

    /// Converts a class property to a struct field.
    fn convert_class_prop(
        &mut self,
        prop: &ast::ClassProp,
        class_vis: &Visibility,
    ) -> Result<StructField> {
        let reg = self.reg();
        let field_name = extract_prop_name(&prop.key)
            .map_err(|_| anyhow!("unsupported class property key (only identifiers)"))?;

        let ty = match prop.type_ann.as_ref() {
            Some(ann) => convert_ts_type(&ann.type_ann, self.synthetic, reg)?,
            None => RustType::Any, // Fallback to Any for unannotated class properties
        };
        let member_vis = resolve_member_visibility(prop.accessibility, class_vis);

        Ok(StructField {
            vis: Some(member_vis),
            name: field_name,
            ty,
        })
    }

    /// Converts a constructor to a `new()` associated function.
    ///
    /// Returns the method and any additional struct fields extracted from
    /// `TsParamProp` (parameter properties like `constructor(public x: number)`).
    fn convert_constructor(
        &mut self,
        ctor: &ast::Constructor,
        vis: &Visibility,
    ) -> Result<(Method, Vec<StructField>)> {
        let mut params = Vec::new();
        let mut param_prop_fields = Vec::new();
        // Names of parameter properties — used to inject `this.field = param`
        // assignments into the constructor body.
        let mut param_prop_names = Vec::new();

        let mut default_expansion_stmts = Vec::new();
        for param in &ctor.params {
            match param {
                ast::ParamOrTsParamProp::Param(p) => {
                    let (param, expansion) = self.convert_param_pat(&p.pat)?;
                    default_expansion_stmts.extend(expansion);
                    params.push(param);
                }
                ast::ParamOrTsParamProp::TsParamProp(prop) => {
                    let (ir_param, field) = self.convert_ts_param_prop(prop, vis)?;
                    param_prop_names.push(ir_param.name.clone());
                    params.push(ir_param);
                    param_prop_fields.push(field);
                }
            }
        }

        let body = match &ctor.body {
            Some(block) => {
                // Prepend synthetic `this.field = field` for each param prop
                // before the original body statements, then run through the
                // existing constructor body conversion which recognises
                // `this.field = value` patterns and folds them into `Self { ... }`.
                let synthetic_stmts = build_param_prop_assignments(&param_prop_names);
                let all_stmts: Vec<ast::Stmt> = synthetic_stmts
                    .into_iter()
                    .chain(block.stmts.iter().cloned())
                    .collect();
                let mut body = self.convert_constructor_body(&all_stmts, &params)?;
                // Insert default parameter expansion stmts at the beginning
                for (i, stmt) in default_expansion_stmts.into_iter().enumerate() {
                    body.insert(i, stmt);
                }
                Some(body)
            }
            None if !param_prop_names.is_empty() => {
                let synthetic_stmts = build_param_prop_assignments(&param_prop_names);
                let mut body = self.convert_constructor_body(&synthetic_stmts, &params)?;
                for (i, stmt) in default_expansion_stmts.into_iter().enumerate() {
                    body.insert(i, stmt);
                }
                Some(body)
            }
            None if !default_expansion_stmts.is_empty() => {
                // No body but has default params — need to create body with expansion stmts
                Some(default_expansion_stmts)
            }
            None => None,
        };

        let method = Method {
            vis: vis.clone(),
            name: "new".to_string(),
            has_self: false,
            has_mut_self: false,
            params,
            return_type: Some(RustType::Named {
                name: "Self".to_string(),
                type_args: vec![],
            }),
            body,
        };
        Ok((method, param_prop_fields))
    }

    /// Converts a `TsParamProp` into an IR parameter and a struct field.
    fn convert_ts_param_prop(
        &mut self,
        prop: &ast::TsParamProp,
        class_vis: &Visibility,
    ) -> Result<(Param, StructField)> {
        let (name, ty) = match &prop.param {
            ast::TsParamPropParam::Ident(ident) => {
                let ir_param = self.convert_ident_to_param(ident)?;
                (ir_param.name, ir_param.ty)
            }
            ast::TsParamPropParam::Assign(assign) => {
                // `public x: number = 42` — extract name and type from the left side
                match assign.left.as_ref() {
                    ast::Pat::Ident(ident) => {
                        let ir_param = self.convert_ident_to_param(ident)?;
                        (ir_param.name, ir_param.ty)
                    }
                    _ => return Err(anyhow!("unsupported parameter property pattern")),
                }
            }
        };

        let field_vis = resolve_member_visibility(prop.accessibility, class_vis);

        let field = StructField {
            vis: Some(field_vis),
            name: name.clone(),
            ty: ty.clone().unwrap_or(RustType::Any),
        };

        let param = Param { name, ty };

        Ok((param, field))
    }
}

/// Builds synthetic `this.<name> = <name>` assignment statements (SWC AST nodes).
///
/// These are prepended to the constructor body before conversion, so the
/// existing `try_extract_this_assignment` logic handles them uniformly.
fn build_param_prop_assignments(names: &[String]) -> Vec<ast::Stmt> {
    use swc_common::DUMMY_SP;

    names
        .iter()
        .map(|name| {
            ast::Stmt::Expr(ast::ExprStmt {
                span: DUMMY_SP,
                expr: Box::new(ast::Expr::Assign(ast::AssignExpr {
                    span: DUMMY_SP,
                    op: ast::AssignOp::Assign,
                    left: ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(
                        ast::MemberExpr {
                            span: DUMMY_SP,
                            obj: Box::new(ast::Expr::This(ast::ThisExpr { span: DUMMY_SP })),
                            prop: ast::MemberProp::Ident(ast::IdentName {
                                span: DUMMY_SP,
                                sym: name.clone().into(),
                            }),
                        },
                    )),
                    right: Box::new(ast::Expr::Ident(ast::Ident {
                        span: DUMMY_SP,
                        ctxt: swc_common::SyntaxContext::empty(),
                        sym: name.clone().into(),
                        optional: false,
                    })),
                })),
            })
        })
        .collect()
}

impl<'a> Transformer<'a> {
    /// Converts constructor body statements.
    ///
    /// Recognizes the pattern of `this.field = value` assignments and converts them
    /// into a `Self { field: value, ... }` tail expression. Statements that don't
    /// match this pattern are converted as normal statements.
    fn convert_constructor_body(
        &mut self,
        stmts: &[ast::Stmt],
        params: &[Param],
    ) -> Result<Vec<Stmt>> {
        let mut type_env = TypeEnv::new();
        for p in params {
            if let Some(ty) = &p.ty {
                type_env.insert(p.name.clone(), ty.clone());
            }
        }
        let mut fields = Vec::new();
        let mut other_stmts = Vec::new();

        for stmt in stmts {
            if let Some((field_name, value_expr)) = try_extract_this_assignment(stmt) {
                // Cat B: field type could be looked up from class definition
                let value = convert_expr(value_expr, self.tctx, &type_env, self.synthetic)?;
                fields.push((field_name, value));
            } else {
                other_stmts.extend(convert_stmt(
                    stmt,
                    self.tctx,
                    None,
                    &mut type_env,
                    self.synthetic,
                )?);
            }
        }

        if !fields.is_empty() {
            other_stmts.push(Stmt::Return(Some(Expr::StructInit {
                name: "Self".to_string(),
                fields,
                base: None,
            })));
        }

        convert_last_return_to_tail(&mut other_stmts);
        Ok(other_stmts)
    }
}

/// Tries to extract a `this.field = value` pattern from a statement.
///
/// Returns `Some((field_name, value_expr))` if the statement matches,
/// `None` otherwise.
fn try_extract_this_assignment(stmt: &ast::Stmt) -> Option<(String, &ast::Expr)> {
    let expr_stmt = match stmt {
        ast::Stmt::Expr(e) => e,
        _ => return None,
    };
    let assign = match expr_stmt.expr.as_ref() {
        ast::Expr::Assign(a) => a,
        _ => return None,
    };
    let member = match &assign.left {
        ast::AssignTarget::Simple(ast::SimpleAssignTarget::Member(m)) => m,
        _ => return None,
    };
    // Check that the object is `this`
    if !matches!(member.obj.as_ref(), ast::Expr::This(_)) {
        return None;
    }
    let field_name = match &member.prop {
        ast::MemberProp::Ident(ident) => ident.sym.to_string(),
        _ => return None,
    };
    Some((field_name, assign.right.as_ref()))
}

impl<'a> Transformer<'a> {
    /// Converts a class method (including getters/setters) to an impl method.
    ///
    /// - `MethodKind::Getter` → `fn name(&self) -> T { ... }`
    /// - `MethodKind::Setter` → `fn set_name(&mut self, v: T) { ... }`
    /// - `MethodKind::Method` → `fn name(&self, ...) -> T { ... }`
    fn convert_class_method(
        &mut self,
        method: &ast::ClassMethod,
        vis: &Visibility,
    ) -> Result<Method> {
        let reg = self.reg();
        let raw_name = extract_prop_name(&method.key)
            .map_err(|_| anyhow!("unsupported method key (only identifiers)"))?;

        let is_setter = method.kind == ast::MethodKind::Setter;

        let name = if is_setter {
            format!("set_{raw_name}")
        } else {
            raw_name
        };

        let mut params = Vec::new();
        let mut default_expansion_stmts = Vec::new();
        for param in &method.function.params {
            let (p, expansion) = self.convert_param_pat(&param.pat)?;
            default_expansion_stmts.extend(expansion);
            params.push(p);
        }

        let return_type = method
            .function
            .return_type
            .as_ref()
            .map(|ann| convert_ts_type(&ann.type_ann, self.synthetic, reg))
            .transpose()?;

        // void → None (Rust omits `-> ()`)
        let return_type = return_type.and_then(|ty| {
            if matches!(ty, RustType::Unit) {
                None
            } else {
                Some(ty)
            }
        });

        let body = match &method.function.body {
            Some(block) => {
                let mut method_env = TypeEnv::new();
                for p in &params {
                    if let Some(ty) = &p.ty {
                        method_env.insert(p.name.clone(), ty.clone());
                    }
                }
                let mut stmts = default_expansion_stmts;
                for stmt in &block.stmts {
                    stmts.extend(convert_stmt(
                        stmt,
                        self.tctx,
                        return_type.as_ref(),
                        &mut method_env,
                        self.synthetic,
                    )?);
                }
                convert_last_return_to_tail(&mut stmts);
                Some(stmts)
            }
            None => None,
        };

        // Setter or method that assigns to `this.field` needs `&mut self`
        let body_stmts = body.as_deref().unwrap_or(&[]);
        let needs_mut = is_setter || body_has_self_assignment(body_stmts);

        let member_vis = resolve_member_visibility(method.accessibility, vis);

        Ok(Method {
            vis: member_vis,
            name,
            has_self: !method.is_static,
            has_mut_self: !method.is_static && needs_mut,
            params,
            return_type,
            body,
        })
    }
}

/// Resolves the effective visibility of a class member based on its TypeScript accessibility modifier.
///
/// `protected` maps to `pub(crate)`, `private` maps to `Private`, and `public` (or unspecified)
/// inherits the class-level visibility.
fn resolve_member_visibility(
    accessibility: Option<ast::Accessibility>,
    class_vis: &Visibility,
) -> Visibility {
    match accessibility {
        Some(ast::Accessibility::Protected) => Visibility::PubCrate,
        Some(ast::Accessibility::Private) => Visibility::Private,
        _ => class_vis.clone(),
    }
}

/// Returns `true` if the method body contains an assignment to `self.field`.
fn body_has_self_assignment(body: &[Stmt]) -> bool {
    body.iter().any(|stmt| match stmt {
        Stmt::Expr(Expr::Assign { target, .. }) => is_self_field_access(target),
        _ => false,
    })
}

/// Returns `true` if the expression is `self.field`.
fn is_self_field_access(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::FieldAccess {
            object,
            ..
        } if matches!(object.as_ref(), Expr::Ident(name) if name == "self")
    )
}

impl<'a> Transformer<'a> {
    /// Converts a parameter pattern into an IR [`Param`] and optional expansion statements.
    ///
    /// Supports:
    /// - `x: number` → simple parameter
    /// - `x: number = 0` → `Option<f64>` + `let x = x.unwrap_or(0.0);`
    /// - `options: Options = {}` → `Option<Options>` + `let options = options.unwrap_or_default();`
    fn convert_param_pat(
        &mut self,
        pat: &ast::Pat,
    ) -> Result<(Param, Vec<Stmt>)> {
        match pat {
            ast::Pat::Ident(ident) => {
                let param = self.convert_ident_to_param(ident)?;
                Ok((param, vec![]))
            }
            ast::Pat::Assign(assign) => {
                // Default value parameter: x: T = value
                match assign.left.as_ref() {
                    ast::Pat::Ident(ident) => {
                        let inner_param = self.convert_ident_to_param(ident)?;
                        let param_name = inner_param.name.clone();
                        let inner_type = inner_param
                            .ty
                            .ok_or_else(|| {
                                anyhow!("default parameter requires a type annotation")
                            })?;
                        let option_type = RustType::Option(Box::new(inner_type));

                        let (default_expr, use_unwrap_or_default) =
                            crate::transformer::functions::convert_default_value(
                                &assign.right,
                                self.synthetic,
                            )?;

                        let unwrap_call = if use_unwrap_or_default {
                            Expr::MethodCall {
                                object: Box::new(Expr::Ident(param_name.clone())),
                                method: "unwrap_or_default".to_string(),
                                args: vec![],
                            }
                        } else {
                            Expr::MethodCall {
                                object: Box::new(Expr::Ident(param_name.clone())),
                                method: "unwrap_or".to_string(),
                                args: vec![default_expr.unwrap()],
                            }
                        };

                        let expansion_stmt = Stmt::Let {
                            mutable: false,
                            name: param_name.clone(),
                            ty: None,
                            init: Some(unwrap_call),
                        };

                        Ok((
                            Param {
                                name: param_name,
                                ty: Some(option_type),
                            },
                            vec![expansion_stmt],
                        ))
                    }
                    _ => Err(anyhow!("unsupported parameter pattern")),
                }
            }
            _ => Err(anyhow!("unsupported parameter pattern")),
        }
    }

    /// Converts an identifier parameter pattern into an IR [`Param`].
    ///
    /// Extracts name and type annotation from a `BindingIdent`, converts the type,
    /// and returns a `Param`. Used by both function and class method parameter conversion.
    fn convert_ident_to_param(
        &mut self,
        ident: &ast::BindingIdent,
    ) -> Result<Param> {
        let reg = self.reg();
        let name = ident.id.sym.to_string();
        let ty = ident
            .type_ann
            .as_ref()
            .ok_or_else(|| anyhow!("parameter '{}' has no type annotation", name))?;
        let rust_type = crate::transformer::types::convert_type_for_position(
            &ty.type_ann,
            crate::transformer::TypePosition::Param,
            self.synthetic,
            reg,
        )?;
        Ok(Param {
            name,
            ty: Some(rust_type),
        })
    }
}

/// Pre-scans all interface declarations to collect method names per interface.
///
/// Used by `implements` processing to determine which class methods belong to
/// which trait impl block.
pub(super) fn pre_scan_interface_methods(module: &ast::Module) -> HashMap<String, Vec<String>> {
    let mut map = HashMap::new();

    for module_item in &module.body {
        let decl = match module_item {
            ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::TsInterface(d))) => d,
            ast::ModuleItem::ModuleDecl(ast::ModuleDecl::ExportDecl(export)) => {
                if let ast::Decl::TsInterface(d) = &export.decl {
                    d
                } else {
                    continue;
                }
            }
            _ => continue,
        };

        let name = decl.id.sym.to_string();
        let method_names: Vec<String> = decl
            .body
            .body
            .iter()
            .filter_map(|member| {
                if let ast::TsTypeElement::TsMethodSignature(method) = member {
                    if let ast::Expr::Ident(ident) = method.key.as_ref() {
                        return Some(ident.sym.to_string());
                    }
                }
                None
            })
            .collect();

        if !method_names.is_empty() {
            map.insert(name, method_names);
        }
    }

    map
}

/// Identifies which classes are parents (are extended by another class).
fn find_parent_class_names(
    class_map: &HashMap<String, ClassInfo>,
) -> std::collections::HashSet<String> {
    class_map
        .values()
        .filter_map(|info| info.parent.clone())
        .collect()
}

impl<'a> Transformer<'a> {
    /// Pre-scans all class declarations in the module to collect inheritance info.
    ///
    /// Returns a map from class name to [`ClassInfo`]. Only classes that can be
    /// successfully parsed are included; parse failures are silently skipped
    /// (they will be reported during the main transformation pass).
    pub(crate) fn pre_scan_classes(
        &mut self,
        module: &ast::Module,
    ) -> HashMap<String, ClassInfo> {
        let mut map = HashMap::new();

        for module_item in &module.body {
            let (decl, vis) = match module_item {
                ast::ModuleItem::Stmt(ast::Stmt::Decl(ast::Decl::Class(cd))) => {
                    (cd, Visibility::Private)
                }
                ast::ModuleItem::ModuleDecl(ast::ModuleDecl::ExportDecl(export)) => {
                    if let ast::Decl::Class(cd) = &export.decl {
                        (cd, Visibility::Public)
                    } else {
                        continue;
                    }
                }
                _ => continue,
            };
            if let Ok(info) = self.extract_class_info(decl, vis) {
                map.insert(info.name.clone(), info);
            }
        }

        map
    }

    /// Transforms a class declaration, handling inheritance and `implements` if applicable.
    ///
    /// - If the class is a parent (extended by another class): generates struct + trait + impls
    /// - If the class is a child (extends another class): generates struct + impl + trait impl
    /// - If the class implements interfaces: generates struct + impl + impl Trait for Struct
    /// - Otherwise: generates struct + impl (no trait)
    pub(crate) fn transform_class_with_inheritance(
        &mut self,
        class_decl: &ast::ClassDecl,
        vis: Visibility,
        class_map: &HashMap<String, ClassInfo>,
        iface_methods: &HashMap<String, Vec<String>>,
    ) -> Result<Vec<Item>> {
        let info = self.extract_class_info(class_decl, vis)?;
        let parent_names = find_parent_class_names(class_map);

        if info.is_abstract {
            // Abstract class — generate trait (not struct)
            generate_abstract_class_items(&info)
        } else if parent_names.contains(&info.name) {
            // This class is a parent — generate struct + trait + impls
            generate_parent_class_items(&info)
        } else if let Some(parent_name) = &info.parent {
            let parent_info = class_map.get(parent_name);
            if parent_info.is_some_and(|p| p.is_abstract) {
                // Parent is abstract — generate struct + impl AbstractParent for Child
                generate_child_of_abstract(&info, parent_name)
            } else if !info.implements.is_empty() {
                // Child class with interface implementations
                generate_child_class_with_implements(&info, parent_info, iface_methods)
            } else {
                // This class is a child — generate struct + impl + trait impl
                generate_items_for_class(&info, parent_info)
            }
        } else if !info.implements.is_empty() {
            // Class implements interfaces — split methods into trait impls
            generate_class_with_implements(&info, iface_methods)
        } else {
            // Standalone class — no inheritance
            generate_items_for_class(&info, None)
        }
    }
}

// --- Wrapper free functions (transition period — will be removed in Phase D-2-F) ---

/// Wrapper: delegates to [`Transformer::extract_class_info`].
pub fn extract_class_info(
    class_decl: &ast::ClassDecl,
    vis: Visibility,
    synthetic: &mut SyntheticTypeRegistry,
    tctx: &TransformContext<'_>,
) -> Result<ClassInfo> {
    let type_env = TypeEnv::new();
    Transformer {
        tctx,
        type_env: type_env,
        synthetic,
    }
    .extract_class_info(class_decl, vis)
}

/// Wrapper: delegates to [`Transformer::convert_class_decl`].
pub fn convert_class_decl(
    class_decl: &ast::ClassDecl,
    vis: Visibility,
    synthetic: &mut SyntheticTypeRegistry,
    tctx: &TransformContext<'_>,
) -> Result<Vec<Item>> {
    let type_env = TypeEnv::new();
    Transformer {
        tctx,
        type_env: type_env,
        synthetic,
    }
    .convert_class_decl(class_decl, vis)
}

/// Wrapper: delegates to [`Transformer::pre_scan_classes`].
pub(super) fn pre_scan_classes(
    module: &ast::Module,
    synthetic: &mut SyntheticTypeRegistry,
    tctx: &TransformContext<'_>,
) -> HashMap<String, ClassInfo> {
    let type_env = TypeEnv::new();
    Transformer {
        tctx,
        type_env: type_env,
        synthetic,
    }
    .pre_scan_classes(module)
}

/// Wrapper: delegates to [`Transformer::transform_class_with_inheritance`].
pub(super) fn transform_class_with_inheritance(
    class_decl: &ast::ClassDecl,
    vis: Visibility,
    tctx: &TransformContext<'_>,
    class_map: &HashMap<String, ClassInfo>,
    iface_methods: &HashMap<String, Vec<String>>,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Vec<Item>> {
    let type_env = TypeEnv::new();
    Transformer {
        tctx,
        type_env: type_env,
        synthetic,
    }
    .transform_class_with_inheritance(class_decl, vis, class_map, iface_methods)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Item, Param, RustType, StructField, Visibility};
    use crate::parser::parse_typescript;
    use crate::registry::TypeRegistry;
    use crate::transformer::test_fixtures::TctxFixture;
    use swc_ecma_ast::{Decl, ModuleItem};
    /// Helper: parse TS source and extract the first ClassDecl.
    fn parse_class_decl(source: &str) -> ast::ClassDecl {
        let module = parse_typescript(source).expect("parse failed");
        match &module.body[0] {
            ModuleItem::Stmt(ast::Stmt::Decl(Decl::Class(decl))) => decl.clone(),
            _ => panic!("expected ClassDecl"),
        }
    }

    #[test]
    fn test_convert_class_properties_only() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl = parse_class_decl("class Foo { x: number; y: string; }");
        let items = convert_class_decl(
            &decl,
            Visibility::Private,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0],
            Item::Struct {
                vis: Visibility::Private,
                name: "Foo".to_string(),
                type_params: vec![],
                fields: vec![
                    StructField {
                        vis: Some(Visibility::Private),
                        name: "x".to_string(),
                        ty: RustType::F64,
                    },
                    StructField {
                        vis: Some(Visibility::Private),
                        name: "y".to_string(),
                        ty: RustType::String,
                    },
                ],
            }
        );
    }

    #[test]
    fn test_convert_class_constructor() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl =
            parse_class_decl("class Foo { x: number; constructor(x: number) { this.x = x; } }");
        let items = convert_class_decl(
            &decl,
            Visibility::Private,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();

        assert_eq!(items.len(), 2);
        match &items[1] {
            Item::Impl {
                struct_name,
                methods,
                ..
            } => {
                assert_eq!(struct_name, "Foo");
                assert_eq!(methods.len(), 1);
                assert_eq!(methods[0].name, "new");
                assert!(!methods[0].has_self);
                assert_eq!(
                    methods[0].return_type,
                    Some(RustType::Named {
                        name: "Self".to_string(),
                        type_args: vec![]
                    })
                );
                assert_eq!(
                    methods[0].params,
                    vec![Param {
                        name: "x".to_string(),
                        ty: Some(RustType::F64),
                    }]
                );
            }
            _ => panic!("expected Item::Impl"),
        }
    }

    #[test]
    fn test_convert_class_method_with_self() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl =
            parse_class_decl("class Foo { name: string; greet(): string { return this.name; } }");
        let items = convert_class_decl(
            &decl,
            Visibility::Private,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();

        assert_eq!(items.len(), 2);
        match &items[1] {
            Item::Impl { methods, .. } => {
                assert_eq!(methods.len(), 1);
                assert_eq!(methods[0].name, "greet");
                assert!(methods[0].has_self);
                assert_eq!(methods[0].return_type, Some(RustType::String));
            }
            _ => panic!("expected Item::Impl"),
        }
    }

    #[test]
    fn test_convert_class_export_visibility() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl = parse_class_decl("class Foo { x: number; greet(): string { return this.x; } }");
        let items = convert_class_decl(
            &decl,
            Visibility::Public,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();

        match &items[0] {
            Item::Struct { vis, .. } => assert_eq!(*vis, Visibility::Public),
            _ => panic!("expected Struct"),
        }
        match &items[1] {
            Item::Impl { methods, .. } => {
                assert_eq!(methods[0].vis, Visibility::Public);
            }
            _ => panic!("expected Impl"),
        }
    }

    #[test]
    fn test_convert_class_static_method_has_no_self() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl = parse_class_decl("class Foo { x: number; static bar(): number { return 1; } }");
        let items = convert_class_decl(
            &decl,
            Visibility::Private,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();

        assert_eq!(items.len(), 2);
        match &items[1] {
            Item::Impl { methods, .. } => {
                assert_eq!(methods.len(), 1);
                assert_eq!(methods[0].name, "bar");
                assert!(
                    !methods[0].has_self,
                    "static method should not have self, got has_self=true"
                );
                assert!(
                    !methods[0].has_mut_self,
                    "static method should not have mut self"
                );
                assert_eq!(methods[0].return_type, Some(RustType::F64));
            }
            _ => panic!("expected Item::Impl"),
        }
    }

    #[test]
    fn test_extract_class_info_implements_single() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl =
            parse_class_decl("class Foo implements Greeter { greet(): string { return 'hi'; } }");
        let info = extract_class_info(
            &decl,
            Visibility::Private,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();

        assert_eq!(info.implements, vec!["Greeter".to_string()]);
    }

    #[test]
    fn test_extract_class_info_implements_multiple() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl = parse_class_decl("class Foo implements A, B { foo(): void {} bar(): void {} }");
        let info = extract_class_info(
            &decl,
            Visibility::Private,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();

        assert_eq!(info.implements, vec!["A".to_string(), "B".to_string()]);
    }

    #[test]
    fn test_extract_class_info_no_implements() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl = parse_class_decl("class Foo { x: number; }");
        let info = extract_class_info(
            &decl,
            Visibility::Private,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();

        assert!(info.implements.is_empty());
    }

    #[test]
    fn test_convert_class_this_to_self() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl = parse_class_decl(
            "class Foo { name: string; constructor(name: string) { this.name = name; } }",
        );
        let items = convert_class_decl(
            &decl,
            Visibility::Private,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();

        match &items[1] {
            Item::Impl { methods, .. } => {
                // Constructor body should contain `self.name = name`
                // which would be an Expr statement with assignment
                assert!(methods[0].body.as_ref().is_some_and(|b| !b.is_empty()));
            }
            _ => panic!("expected Impl"),
        }
    }

    #[test]
    fn test_extract_class_info_abstract_flag_is_true() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl = parse_class_decl("abstract class Shape { abstract area(): number; }");
        let info = extract_class_info(
            &decl,
            Visibility::Private,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();
        assert!(info.is_abstract);
    }

    #[test]
    fn test_convert_abstract_class_abstract_only_generates_trait() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl = parse_class_decl("abstract class Shape { abstract area(): number; }");
        let items = convert_class_decl(
            &decl,
            Visibility::Public,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();
        // Should produce a single Trait item, not Struct + Impl
        assert_eq!(items.len(), 1);
        match &items[0] {
            Item::Trait {
                vis, name, methods, ..
            } => {
                assert_eq!(*vis, Visibility::Public);
                assert_eq!(name, "Shape");
                assert_eq!(methods.len(), 1);
                assert_eq!(methods[0].name, "area");
                assert!(
                    methods[0].body.is_none(),
                    "abstract method should have no body"
                );
                assert_eq!(methods[0].return_type, Some(RustType::F64));
            }
            _ => panic!("expected Item::Trait, got {:?}", items[0]),
        }
    }

    #[test]
    fn test_convert_abstract_class_mixed_generates_trait_with_defaults() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl = parse_class_decl(
            "abstract class Shape { abstract area(): number; describe(): string { return \"shape\"; } }",
        );
        let items = convert_class_decl(
            &decl,
            Visibility::Public,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();
        assert_eq!(items.len(), 1);
        match &items[0] {
            Item::Trait { methods, .. } => {
                assert_eq!(methods.len(), 2);
                // abstract method: no body
                assert_eq!(methods[0].name, "area");
                assert!(methods[0].body.is_none());
                // concrete method: has body (default impl)
                assert_eq!(methods[1].name, "describe");
                assert!(methods[1].body.as_ref().is_some_and(|b| !b.is_empty()));
            }
            _ => panic!("expected Item::Trait, got {:?}", items[0]),
        }
    }

    #[test]
    fn test_convert_abstract_class_concrete_only_generates_trait_with_defaults() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl = parse_class_decl("abstract class Foo { bar(): number { return 1; } }");
        let items = convert_class_decl(
            &decl,
            Visibility::Public,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();
        assert_eq!(items.len(), 1);
        match &items[0] {
            Item::Trait { methods, .. } => {
                assert_eq!(methods.len(), 1);
                assert_eq!(methods[0].name, "bar");
                assert!(methods[0].body.as_ref().is_some_and(|b| !b.is_empty()));
            }
            _ => panic!("expected Item::Trait, got {:?}", items[0]),
        }
    }

    #[test]
    fn test_rewrite_super_constructor_arg_count_mismatch_returns_error() {
        // Parent has 2 fields but child's super() only passes 1 arg
        let parent_info = ClassInfo {
            name: "Parent".to_string(),
            parent: None,
            fields: vec![
                StructField {
                    vis: None,
                    name: "a".to_string(),
                    ty: RustType::F64,
                },
                StructField {
                    vis: None,
                    name: "b".to_string(),
                    ty: RustType::String,
                },
            ],
            constructor: None,
            methods: vec![],
            vis: Visibility::Private,
            implements: vec![],
            is_abstract: false,
            static_consts: vec![],
        };

        let child_ctor = Method {
            vis: Visibility::Public,
            name: "new".to_string(),
            has_self: false,
            has_mut_self: false,
            params: vec![Param {
                name: "x".to_string(),
                ty: Some(RustType::F64),
            }],
            return_type: Some(RustType::Named {
                name: "Self".to_string(),
                type_args: vec![],
            }),
            body: Some(vec![Stmt::Expr(Expr::FnCall {
                name: "super".to_string(),
                args: vec![Expr::Ident("x".to_string())], // only 1 arg, parent has 2 fields
            })]),
        };

        let result = rewrite_super_constructor(&child_ctor, &parent_info);
        assert!(
            result.is_err(),
            "expected error for arg count mismatch, got: {:?}",
            result
        );
    }

    #[test]
    fn test_convert_class_static_prop_generates_assoc_const() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl = parse_class_decl(
            "class Config { static readonly MAX_SIZE: number = 100; value: number; }",
        );
        let items = convert_class_decl(
            &decl,
            Visibility::Public,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();
        // Should have: Struct (only value field) + Impl (with const MAX_SIZE)
        match &items[0] {
            Item::Struct { fields, .. } => {
                assert_eq!(
                    fields.len(),
                    1,
                    "static prop should not be in struct fields"
                );
                assert_eq!(fields[0].name, "value");
            }
            _ => panic!("expected Item::Struct, got {:?}", items[0]),
        }
        match &items[1] {
            Item::Impl { consts, .. } => {
                assert_eq!(consts.len(), 1);
                assert_eq!(consts[0].name, "MAX_SIZE");
                assert_eq!(consts[0].ty, RustType::F64);
                assert_eq!(consts[0].value, Expr::NumberLit(100.0));
            }
            _ => panic!("expected Item::Impl, got {:?}", items[1]),
        }
    }

    #[test]
    fn test_convert_class_static_string_prop_generates_assoc_const() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl = parse_class_decl("class Foo { static NAME: string = \"hello\"; x: number; }");
        let items = convert_class_decl(
            &decl,
            Visibility::Public,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();
        match &items[1] {
            Item::Impl { consts, .. } => {
                assert_eq!(consts.len(), 1);
                assert_eq!(consts[0].name, "NAME");
                assert_eq!(consts[0].ty, RustType::String);
            }
            _ => panic!("expected Item::Impl, got {:?}", items[1]),
        }
    }

    #[test]
    fn test_convert_class_protected_method_generates_pub_crate() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl = parse_class_decl("class Foo { protected greet(): string { return 'hi'; } }");
        let items = convert_class_decl(
            &decl,
            Visibility::Public,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();
        match &items[1] {
            Item::Impl { methods, .. } => {
                assert_eq!(methods.len(), 1);
                assert_eq!(methods[0].name, "greet");
                assert_eq!(methods[0].vis, Visibility::PubCrate);
            }
            _ => panic!("expected Item::Impl"),
        }
    }

    #[test]
    fn test_convert_class_protected_property_generates_pub_crate_field() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl = parse_class_decl("class Foo { protected x: number; }");
        let items = convert_class_decl(
            &decl,
            Visibility::Public,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();
        // Verify via generator output since StructField doesn't have vis yet
        let output = crate::generator::generate(&items);
        assert!(output.contains("pub(crate) x: f64"));
    }

    // --- TsParamProp (constructor parameter properties) ---

    #[test]
    fn test_param_prop_basic_public_generates_field_and_new() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl = parse_class_decl("class Foo { constructor(public x: number) {} }");
        let items = convert_class_decl(
            &decl,
            Visibility::Public,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();

        // Struct should have field `x`
        match &items[0] {
            Item::Struct { fields, .. } => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, "x");
                assert_eq!(fields[0].ty, RustType::F64);
                assert_eq!(fields[0].vis, Some(Visibility::Public));
            }
            _ => panic!("expected Item::Struct"),
        }

        // Impl should have `new(x: f64) -> Self`
        match &items[1] {
            Item::Impl { methods, .. } => {
                assert_eq!(methods.len(), 1);
                assert_eq!(methods[0].name, "new");
                assert_eq!(
                    methods[0].params,
                    vec![Param {
                        name: "x".to_string(),
                        ty: Some(RustType::F64),
                    }]
                );
            }
            _ => panic!("expected Item::Impl"),
        }
    }

    #[test]
    fn test_param_prop_private_generates_private_field() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl = parse_class_decl("class Foo { constructor(private x: number) {} }");
        let items = convert_class_decl(
            &decl,
            Visibility::Public,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();

        match &items[0] {
            Item::Struct { fields, .. } => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, "x");
                assert_eq!(fields[0].vis, Some(Visibility::Private));
            }
            _ => panic!("expected Item::Struct"),
        }
    }

    #[test]
    fn test_param_prop_readonly_generates_field() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl = parse_class_decl("class Foo { constructor(public readonly x: string) {} }");
        let items = convert_class_decl(
            &decl,
            Visibility::Public,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();

        match &items[0] {
            Item::Struct { fields, .. } => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, "x");
                assert_eq!(fields[0].ty, RustType::String);
                assert_eq!(fields[0].vis, Some(Visibility::Public));
            }
            _ => panic!("expected Item::Struct"),
        }
    }

    #[test]
    fn test_param_prop_with_default_value_generates_field_and_param() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl = parse_class_decl("class Foo { constructor(public x: number = 42) {} }");
        let items = convert_class_decl(
            &decl,
            Visibility::Public,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();

        match &items[0] {
            Item::Struct { fields, .. } => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, "x");
                assert_eq!(fields[0].ty, RustType::F64);
            }
            _ => panic!("expected Item::Struct"),
        }

        match &items[1] {
            Item::Impl { methods, .. } => {
                assert_eq!(methods[0].name, "new");
                assert_eq!(methods[0].params.len(), 1);
                assert_eq!(methods[0].params[0].name, "x");
            }
            _ => panic!("expected Item::Impl"),
        }
    }

    #[test]
    fn test_param_prop_mixed_with_regular_param() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl = parse_class_decl(
            "class Foo { constructor(public x: number, y: string) { console.log(y); } }",
        );
        let items = convert_class_decl(
            &decl,
            Visibility::Public,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();

        // Struct should only have field `x` (not `y`)
        match &items[0] {
            Item::Struct { fields, .. } => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, "x");
            }
            _ => panic!("expected Item::Struct"),
        }

        // new() should have both params
        match &items[1] {
            Item::Impl { methods, .. } => {
                assert_eq!(methods[0].params.len(), 2);
                assert_eq!(methods[0].params[0].name, "x");
                assert_eq!(methods[0].params[1].name, "y");
            }
            _ => panic!("expected Item::Impl"),
        }
    }

    #[test]
    fn test_param_prop_multiple_generates_multiple_fields() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl =
            parse_class_decl("class Foo { constructor(public x: number, private y: string) {} }");
        let items = convert_class_decl(
            &decl,
            Visibility::Public,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();

        match &items[0] {
            Item::Struct { fields, .. } => {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].name, "x");
                assert_eq!(fields[0].vis, Some(Visibility::Public));
                assert_eq!(fields[1].name, "y");
                assert_eq!(fields[1].vis, Some(Visibility::Private));
            }
            _ => panic!("expected Item::Struct"),
        }
    }

    #[test]
    fn test_param_prop_with_existing_this_assignment() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl = parse_class_decl(
            "class Foo { z: boolean; constructor(public x: number) { this.z = true; } }",
        );
        let items = convert_class_decl(
            &decl,
            Visibility::Public,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();

        // Struct should have both `z` (explicit) and `x` (param prop)
        match &items[0] {
            Item::Struct { fields, .. } => {
                assert_eq!(fields.len(), 2);
                let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
                assert!(names.contains(&"x"));
                assert!(names.contains(&"z"));
            }
            _ => panic!("expected Item::Struct"),
        }
    }

    #[test]
    fn test_param_prop_with_body_logic_preserves_statements() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        let decl =
            parse_class_decl("class Foo { constructor(public x: number) { console.log(x); } }");
        let items = convert_class_decl(
            &decl,
            Visibility::Public,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();

        match &items[1] {
            Item::Impl { methods, .. } => {
                let body = methods[0].body.as_ref().unwrap();
                // Should have both the console.log and the Self init
                assert!(
                    body.len() >= 2,
                    "body should have logic + Self init, got {:?}",
                    body
                );
            }
            _ => panic!("expected Item::Impl"),
        }
    }

    #[test]
    fn test_convert_class_constructor_default_number_param() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        // constructor(x: number = 0) should produce Option<f64> param + unwrap_or
        let decl =
            parse_class_decl("class Foo { x: number; constructor(x: number = 0) { this.x = x; } }");
        let items = convert_class_decl(
            &decl,
            Visibility::Private,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();

        // Find the Impl item
        let impl_item = items.iter().find(|i| matches!(i, Item::Impl { .. }));
        assert!(impl_item.is_some(), "expected Impl item");

        match impl_item.unwrap() {
            Item::Impl { methods, .. } => {
                let new_method = methods.iter().find(|m| m.name == "new");
                assert!(new_method.is_some(), "expected 'new' method");
                let method = new_method.unwrap();
                // Parameter should be Option<f64>
                assert_eq!(method.params.len(), 1);
                assert_eq!(method.params[0].name, "x");
                assert_eq!(
                    method.params[0].ty,
                    Some(RustType::Option(Box::new(RustType::F64)))
                );
                // Body should contain unwrap_or expansion as first statement
                assert!(
                    method.body.as_ref().unwrap().len() >= 2,
                    "expected unwrap_or expansion + Self init, got {:?}",
                    method.body.as_ref().unwrap()
                );
            }
            _ => panic!("expected Item::Impl"),
        }
    }

    #[test]
    fn test_convert_class_constructor_default_empty_object_param() {
        let f = TctxFixture::new();
        let tctx = f.tctx();
        // constructor(options: Options = {}) should produce Option<Options> + unwrap_or_default
        let decl = parse_class_decl("class Foo { constructor(options: Options = {}) {} }");
        let items = convert_class_decl(
            &decl,
            Visibility::Private,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();

        let impl_item = items.iter().find(|i| matches!(i, Item::Impl { .. }));
        assert!(impl_item.is_some(), "expected Impl item");

        match impl_item.unwrap() {
            Item::Impl { methods, .. } => {
                let new_method = methods.iter().find(|m| m.name == "new");
                assert!(new_method.is_some(), "expected 'new' method");
                let method = new_method.unwrap();
                assert_eq!(method.params.len(), 1);
                assert_eq!(method.params[0].name, "options");
                assert_eq!(
                    method.params[0].ty,
                    Some(RustType::Option(Box::new(RustType::Named {
                        name: "Options".to_string(),
                        type_args: vec![],
                    })))
                );
            }
            _ => panic!("expected Item::Impl"),
        }
    }

    // --- Expected type propagation ---

    /// Step 7: Static property initializer should propagate type annotation.
    /// `static config: Config = { name: "default" }` should produce StructInit, not error.
    #[test]
    fn test_convert_static_prop_propagates_type_annotation() {
        let mut reg = TypeRegistry::new();
        reg.register(
            "Config".to_string(),
            crate::registry::TypeDef::new_struct(
                vec![("name".to_string(), RustType::String)],
                std::collections::HashMap::new(),
                vec![],
            ),
        );

        let source = r#"class Foo { static config: Config = { name: "default" }; }"#;
        let f = TctxFixture::from_source_with_reg(source, reg);
        let tctx = f.tctx();

        let decl = match &f.module().body[0] {
            ModuleItem::Stmt(ast::Stmt::Decl(Decl::Class(decl))) => decl.clone(),
            _ => panic!("expected ClassDecl"),
        };
        let items = convert_class_decl(
            &decl,
            Visibility::Private,
            &mut SyntheticTypeRegistry::new(),
            &tctx,
        )
        .unwrap();

        // Find the Impl item with static consts
        let impl_item = items
            .iter()
            .find(|item| matches!(item, Item::Impl { .. }))
            .expect("expected Item::Impl");

        match impl_item {
            Item::Impl { consts, .. } => {
                assert_eq!(consts.len(), 1);
                assert_eq!(consts[0].name, "config");
                match &consts[0].value {
                    Expr::StructInit { name, fields, .. } => {
                        assert_eq!(name, "Config");
                        assert_eq!(fields[0].0, "name");
                        assert!(
                            matches!(&fields[0].1, Expr::MethodCall { method, .. } if method == "to_string"),
                            "expected .to_string() on string field, got {:?}",
                            fields[0].1
                        );
                    }
                    other => panic!("expected StructInit, got {other:?}"),
                }
            }
            _ => unreachable!(),
        }
    }
}
