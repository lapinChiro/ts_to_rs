//! Class declaration conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC class declarations into IR [`Item::Struct`] + [`Item::Impl`].

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{AssocConst, Expr, Item, Method, Param, RustType, Stmt, StructField, Visibility};
use crate::registry::TypeRegistry;
use crate::transformer::expressions::convert_expr;
use crate::transformer::extract_prop_name;
use crate::transformer::functions::convert_last_return_to_tail;
use crate::transformer::statements::convert_stmt;
use crate::transformer::types::convert_ts_type;
use crate::transformer::TypeEnv;

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

/// Extracts [`ClassInfo`] from an SWC class declaration without generating IR items.
///
/// Used in the first pass to collect class metadata for inheritance resolution.
pub fn extract_class_info(
    class_decl: &ast::ClassDecl,
    vis: Visibility,
    reg: &TypeRegistry,
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
                if let Some(ac) = convert_static_prop(prop, &vis, reg)? {
                    static_consts.push(ac);
                }
            }
            ast::ClassMember::ClassProp(prop) => {
                fields.push(convert_class_prop(prop, &vis, reg)?);
            }
            ast::ClassMember::Constructor(ctor) => {
                constructor = Some(convert_constructor(ctor, &vis, reg)?);
            }
            ast::ClassMember::Method(method) => {
                methods.push(convert_class_method(method, &vis, reg)?);
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
pub fn convert_class_decl(
    class_decl: &ast::ClassDecl,
    vis: Visibility,
    reg: &TypeRegistry,
) -> Result<Vec<Item>> {
    let info = extract_class_info(class_decl, vis, reg)?;
    if info.is_abstract {
        generate_abstract_class_items(&info)
    } else {
        generate_items_for_class(&info, None)
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

/// Converts a static class property to an associated constant.
///
/// Returns `None` if the property has no initializer (cannot become a const without a value).
fn convert_static_prop(
    prop: &ast::ClassProp,
    vis: &Visibility,
    reg: &TypeRegistry,
) -> Result<Option<AssocConst>> {
    let name = extract_prop_name(&prop.key)
        .map_err(|_| anyhow!("unsupported static property key (only identifiers)"))?;

    let type_ann = prop
        .type_ann
        .as_ref()
        .ok_or_else(|| anyhow!("static property '{}' has no type annotation", name))?;
    let ty = convert_ts_type(&type_ann.type_ann, &mut Vec::new(), reg)?;

    let value = match &prop.value {
        Some(init) => convert_expr(
            init,
            &crate::registry::TypeRegistry::new(),
            None,
            &TypeEnv::new(),
        )?,
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
    prop: &ast::ClassProp,
    class_vis: &Visibility,
    reg: &TypeRegistry,
) -> Result<StructField> {
    let field_name = extract_prop_name(&prop.key)
        .map_err(|_| anyhow!("unsupported class property key (only identifiers)"))?;

    let type_ann = prop
        .type_ann
        .as_ref()
        .ok_or_else(|| anyhow!("class property '{}' has no type annotation", field_name))?;

    let ty = convert_ts_type(&type_ann.type_ann, &mut Vec::new(), reg)?;
    let member_vis = resolve_member_visibility(prop.accessibility, class_vis);

    Ok(StructField {
        vis: Some(member_vis),
        name: field_name,
        ty,
    })
}

/// Converts a constructor to a `new()` associated function.
fn convert_constructor(
    ctor: &ast::Constructor,
    vis: &Visibility,
    reg: &TypeRegistry,
) -> Result<Method> {
    let mut params = Vec::new();
    for param in &ctor.params {
        match param {
            ast::ParamOrTsParamProp::Param(p) => {
                let param = convert_param_pat(&p.pat, reg)?;
                params.push(param);
            }
            ast::ParamOrTsParamProp::TsParamProp(_) => {
                return Err(anyhow!("TypeScript parameter properties are not supported"));
            }
        }
    }

    let body = match &ctor.body {
        Some(block) => Some(convert_constructor_body(&block.stmts, reg, &params)?),
        None => None,
    };

    Ok(Method {
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
    })
}

/// Converts constructor body statements.
///
/// Recognizes the pattern of `this.field = value` assignments and converts them
/// into a `Self { field: value, ... }` tail expression. Statements that don't
/// match this pattern are converted as normal statements.
fn convert_constructor_body(
    stmts: &[ast::Stmt],
    reg: &TypeRegistry,
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
            let value = convert_expr(value_expr, reg, None, &type_env)?;
            fields.push((field_name, value));
        } else {
            other_stmts.extend(convert_stmt(stmt, reg, None, &mut type_env)?);
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

/// Converts a class method (including getters/setters) to an impl method.
///
/// - `MethodKind::Getter` → `fn name(&self) -> T { ... }`
/// - `MethodKind::Setter` → `fn set_name(&mut self, v: T) { ... }`
/// - `MethodKind::Method` → `fn name(&self, ...) -> T { ... }`
fn convert_class_method(
    method: &ast::ClassMethod,
    vis: &Visibility,
    reg: &TypeRegistry,
) -> Result<Method> {
    let raw_name = extract_prop_name(&method.key)
        .map_err(|_| anyhow!("unsupported method key (only identifiers)"))?;

    let is_setter = method.kind == ast::MethodKind::Setter;

    let name = if is_setter {
        format!("set_{raw_name}")
    } else {
        raw_name
    };

    let mut params = Vec::new();
    for param in &method.function.params {
        let p = convert_param_pat(&param.pat, reg)?;
        params.push(p);
    }

    let return_type = method
        .function
        .return_type
        .as_ref()
        .map(|ann| convert_ts_type(&ann.type_ann, &mut Vec::new(), reg))
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
            let mut stmts = Vec::new();
            for stmt in &block.stmts {
                stmts.extend(convert_stmt(
                    stmt,
                    reg,
                    return_type.as_ref(),
                    &mut method_env,
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

/// Converts a parameter pattern into an IR [`Param`].
fn convert_param_pat(pat: &ast::Pat, reg: &TypeRegistry) -> Result<Param> {
    match pat {
        ast::Pat::Ident(ident) => crate::transformer::convert_ident_to_param(ident, reg),
        _ => Err(anyhow!("unsupported parameter pattern")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Item, Param, RustType, StructField, Visibility};
    use crate::parser::parse_typescript;
    use crate::registry::TypeRegistry;
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
        let decl = parse_class_decl("class Foo { x: number; y: string; }");
        let items = convert_class_decl(&decl, Visibility::Private, &TypeRegistry::new()).unwrap();

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
        let decl =
            parse_class_decl("class Foo { x: number; constructor(x: number) { this.x = x; } }");
        let items = convert_class_decl(&decl, Visibility::Private, &TypeRegistry::new()).unwrap();

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
        let decl =
            parse_class_decl("class Foo { name: string; greet(): string { return this.name; } }");
        let items = convert_class_decl(&decl, Visibility::Private, &TypeRegistry::new()).unwrap();

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
        let decl = parse_class_decl("class Foo { x: number; greet(): string { return this.x; } }");
        let items = convert_class_decl(&decl, Visibility::Public, &TypeRegistry::new()).unwrap();

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
        let decl = parse_class_decl("class Foo { x: number; static bar(): number { return 1; } }");
        let items = convert_class_decl(&decl, Visibility::Private, &TypeRegistry::new()).unwrap();

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
        let decl =
            parse_class_decl("class Foo implements Greeter { greet(): string { return 'hi'; } }");
        let info = extract_class_info(&decl, Visibility::Private, &TypeRegistry::new()).unwrap();

        assert_eq!(info.implements, vec!["Greeter".to_string()]);
    }

    #[test]
    fn test_extract_class_info_implements_multiple() {
        let decl = parse_class_decl("class Foo implements A, B { foo(): void {} bar(): void {} }");
        let info = extract_class_info(&decl, Visibility::Private, &TypeRegistry::new()).unwrap();

        assert_eq!(info.implements, vec!["A".to_string(), "B".to_string()]);
    }

    #[test]
    fn test_extract_class_info_no_implements() {
        let decl = parse_class_decl("class Foo { x: number; }");
        let info = extract_class_info(&decl, Visibility::Private, &TypeRegistry::new()).unwrap();

        assert!(info.implements.is_empty());
    }

    #[test]
    fn test_convert_class_this_to_self() {
        let decl = parse_class_decl(
            "class Foo { name: string; constructor(name: string) { this.name = name; } }",
        );
        let items = convert_class_decl(&decl, Visibility::Private, &TypeRegistry::new()).unwrap();

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
        let decl = parse_class_decl("abstract class Shape { abstract area(): number; }");
        let info = extract_class_info(&decl, Visibility::Private, &TypeRegistry::new()).unwrap();
        assert!(info.is_abstract);
    }

    #[test]
    fn test_convert_abstract_class_abstract_only_generates_trait() {
        let decl = parse_class_decl("abstract class Shape { abstract area(): number; }");
        let items = convert_class_decl(&decl, Visibility::Public, &TypeRegistry::new()).unwrap();
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
        let decl = parse_class_decl(
            "abstract class Shape { abstract area(): number; describe(): string { return \"shape\"; } }",
        );
        let items = convert_class_decl(&decl, Visibility::Public, &TypeRegistry::new()).unwrap();
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
        let decl = parse_class_decl("abstract class Foo { bar(): number { return 1; } }");
        let items = convert_class_decl(&decl, Visibility::Public, &TypeRegistry::new()).unwrap();
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
        let decl = parse_class_decl(
            "class Config { static readonly MAX_SIZE: number = 100; value: number; }",
        );
        let items = convert_class_decl(&decl, Visibility::Public, &TypeRegistry::new()).unwrap();
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
        let decl = parse_class_decl("class Foo { static NAME: string = \"hello\"; x: number; }");
        let items = convert_class_decl(&decl, Visibility::Public, &TypeRegistry::new()).unwrap();
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
        let decl = parse_class_decl("class Foo { protected greet(): string { return 'hi'; } }");
        let items = convert_class_decl(&decl, Visibility::Public, &TypeRegistry::new()).unwrap();
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
        let decl = parse_class_decl("class Foo { protected x: number; }");
        let items = convert_class_decl(&decl, Visibility::Public, &TypeRegistry::new()).unwrap();
        // Verify via generator output since StructField doesn't have vis yet
        let output = crate::generator::generate(&items);
        assert!(output.contains("pub(crate) x: f64"));
    }
}
