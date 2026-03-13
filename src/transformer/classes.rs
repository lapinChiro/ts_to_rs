//! Class declaration conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC class declarations into IR [`Item::Struct`] + [`Item::Impl`].

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{Expr, Item, Method, Param, RustType, Stmt, StructField, Visibility};
use crate::registry::TypeRegistry;
use crate::transformer::expressions::convert_expr;
use crate::transformer::extract_prop_name;
use crate::transformer::statements::convert_stmt;
use crate::transformer::types::convert_ts_type;

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

    let mut fields = Vec::new();
    let mut constructor = None;
    let mut methods = Vec::new();

    for member in &class_decl.class.body {
        match member {
            ast::ClassMember::ClassProp(prop) => {
                fields.push(convert_class_prop(prop)?);
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
    generate_items_for_class(&info, None)
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
    let mut items = Vec::new();

    // 1. Struct
    items.push(Item::Struct {
        vis: info.vis.clone(),
        name: info.name.clone(),
        type_params: vec![],
        fields: info.fields.clone(),
    });

    // 2. Trait with method signatures
    let trait_methods: Vec<Method> = info
        .methods
        .iter()
        .map(|m| Method {
            vis: Visibility::Private, // trait methods have no visibility
            name: m.name.clone(),
            has_self: m.has_self,
            has_mut_self: m.has_mut_self,
            params: m.params.clone(),
            return_type: m.return_type.clone(),
            body: vec![], // signature only
        })
        .collect();

    items.push(Item::Trait {
        vis: info.vis.clone(),
        name: trait_name.clone(),
        methods: trait_methods,
    });

    // 3. impl (constructor only)
    if let Some(ctor) = &info.constructor {
        items.push(Item::Impl {
            struct_name: info.name.clone(),
            for_trait: None,
            methods: vec![ctor.clone()],
        });
    }

    // 4. impl Trait for Struct (method bodies)
    if !info.methods.is_empty() {
        let trait_impl_methods: Vec<Method> = info
            .methods
            .iter()
            .map(|m| Method {
                vis: Visibility::Private,
                ..m.clone()
            })
            .collect();
        items.push(Item::Impl {
            struct_name: info.name.clone(),
            for_trait: Some(trait_name),
            methods: trait_impl_methods,
        });
    }

    Ok(items)
}

/// Generates IR items for a standalone class (no inheritance).
fn generate_standalone_class(info: &ClassInfo) -> Result<Vec<Item>> {
    let mut items = vec![Item::Struct {
        vis: info.vis.clone(),
        name: info.name.clone(),
        type_params: vec![],
        fields: info.fields.clone(),
    }];

    let mut all_methods = Vec::new();
    if let Some(ctor) = &info.constructor {
        all_methods.push(ctor.clone());
    }
    all_methods.extend(info.methods.clone());

    if !all_methods.is_empty() {
        items.push(Item::Impl {
            struct_name: info.name.clone(),
            for_trait: None,
            methods: all_methods,
        });
    }

    Ok(items)
}

/// Generates IR items for a child class that extends a parent.
///
/// Produces: struct (with parent fields) + impl (constructor + own methods) + impl trait
fn generate_child_class(info: &ClassInfo, parent: &ClassInfo) -> Result<Vec<Item>> {
    let trait_name = format!("{}Trait", parent.name);
    let mut items = Vec::new();

    // 1. Struct with parent fields copied
    let mut fields = parent.fields.clone();
    fields.extend(info.fields.clone());
    items.push(Item::Struct {
        vis: info.vis.clone(),
        name: info.name.clone(),
        type_params: vec![],
        fields,
    });

    // 2. impl (constructor + own methods)
    let mut own_methods = Vec::new();
    if let Some(ctor) = &info.constructor {
        // Rewrite super() calls in constructor body
        let rewritten_ctor = rewrite_super_constructor(ctor, parent);
        own_methods.push(rewritten_ctor);
    }
    own_methods.extend(info.methods.clone());

    if !own_methods.is_empty() {
        items.push(Item::Impl {
            struct_name: info.name.clone(),
            for_trait: None,
            methods: own_methods,
        });
    }

    // 3. impl ParentTrait for Child (copy parent method bodies)
    if !parent.methods.is_empty() {
        let trait_impl_methods: Vec<Method> = parent
            .methods
            .iter()
            .map(|m| Method {
                vis: Visibility::Private,
                ..m.clone()
            })
            .collect();
        items.push(Item::Impl {
            struct_name: info.name.clone(),
            for_trait: Some(trait_name),
            methods: trait_impl_methods,
        });
    }

    Ok(items)
}

/// Rewrites a child constructor to handle `super()` calls.
///
/// `super(args)` in the child constructor is removed, and the parent's field
/// initialization pattern from the constructor arguments is applied.
fn rewrite_super_constructor(ctor: &Method, parent: &ClassInfo) -> Method {
    let mut new_body = Vec::new();
    let mut super_fields = Vec::new();

    // Extract super() call arguments and map to parent fields
    for stmt in &ctor.body {
        if let Some(args) = try_extract_super_call(stmt) {
            // Map super(arg1, arg2, ...) to parent field initialization
            for (i, field) in parent.fields.iter().enumerate() {
                if let Some(arg) = args.get(i) {
                    super_fields.push((field.name.clone(), arg.clone()));
                }
            }
        } else {
            new_body.push(stmt.clone());
        }
    }

    // Build Self { parent_fields..., child_fields... } at the end
    // If the body ends with a Return(StructInit), merge super fields into it
    let has_struct_init = new_body
        .iter()
        .any(|s| matches!(s, Stmt::Return(Some(Expr::StructInit { .. }))));

    if has_struct_init {
        // Merge super fields into existing StructInit
        new_body = new_body
            .into_iter()
            .map(|s| match s {
                Stmt::Return(Some(Expr::StructInit {
                    name, mut fields, ..
                })) => {
                    let mut merged = super_fields.clone();
                    merged.append(&mut fields);
                    Stmt::Return(Some(Expr::StructInit {
                        name,
                        fields: merged,
                    }))
                }
                other => other,
            })
            .collect();
    } else if !super_fields.is_empty() {
        // No existing StructInit — create one with super fields
        new_body.push(Stmt::Return(Some(Expr::StructInit {
            name: "Self".to_string(),
            fields: super_fields,
        })));
    }

    Method {
        body: new_body,
        ..ctor.clone()
    }
}

/// Tries to extract arguments from a `super(args)` call statement.
fn try_extract_super_call(stmt: &Stmt) -> Option<Vec<Expr>> {
    match stmt {
        Stmt::Expr(Expr::FnCall { name, args }) if name == "super" => Some(args.clone()),
        _ => None,
    }
}

/// Converts a class property to a struct field.
fn convert_class_prop(prop: &ast::ClassProp) -> Result<StructField> {
    let field_name = extract_prop_name(&prop.key)
        .map_err(|_| anyhow!("unsupported class property key (only identifiers)"))?;

    let type_ann = prop
        .type_ann
        .as_ref()
        .ok_or_else(|| anyhow!("class property '{}' has no type annotation", field_name))?;

    let ty = convert_ts_type(&type_ann.type_ann)?;

    Ok(StructField {
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
                let param = convert_param_pat(&p.pat)?;
                params.push(param);
            }
            ast::ParamOrTsParamProp::TsParamProp(_) => {
                return Err(anyhow!("TypeScript parameter properties are not supported"));
            }
        }
    }

    let body = match &ctor.body {
        Some(block) => convert_constructor_body(&block.stmts, reg)?,
        None => vec![],
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
fn convert_constructor_body(stmts: &[ast::Stmt], reg: &TypeRegistry) -> Result<Vec<Stmt>> {
    let mut fields = Vec::new();
    let mut other_stmts = Vec::new();

    for stmt in stmts {
        if let Some((field_name, value_expr)) = try_extract_this_assignment(stmt) {
            let value = convert_expr(value_expr, reg, None)?;
            fields.push((field_name, value));
        } else {
            other_stmts.push(convert_stmt(stmt, reg, None)?);
        }
    }

    if !fields.is_empty() {
        other_stmts.push(Stmt::Return(Some(Expr::StructInit {
            name: "Self".to_string(),
            fields,
        })));
    }

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
        let p = convert_param_pat(&param.pat)?;
        params.push(p);
    }

    let return_type = method
        .function
        .return_type
        .as_ref()
        .map(|ann| convert_ts_type(&ann.type_ann))
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
            let mut stmts = Vec::new();
            for stmt in &block.stmts {
                stmts.push(convert_stmt(stmt, reg, return_type.as_ref())?);
            }
            stmts
        }
        None => Vec::new(),
    };

    // Setter or method that assigns to `this.field` needs `&mut self`
    let needs_mut = is_setter || body_has_self_assignment(&body);

    Ok(Method {
        vis: vis.clone(),
        name,
        has_self: true,
        has_mut_self: needs_mut,
        params,
        return_type,
        body,
    })
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
fn convert_param_pat(pat: &ast::Pat) -> Result<Param> {
    match pat {
        ast::Pat::Ident(ident) => {
            let name = ident.id.sym.to_string();
            let ty = ident
                .type_ann
                .as_ref()
                .ok_or_else(|| anyhow!("parameter '{}' has no type annotation", name))?;
            let rust_type = convert_ts_type(&ty.type_ann)?;
            Ok(Param {
                name,
                ty: Some(rust_type),
            })
        }
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
                        name: "x".to_string(),
                        ty: RustType::F64,
                    },
                    StructField {
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
    fn test_convert_class_this_to_self() {
        let decl = parse_class_decl(
            "class Foo { name: string; constructor(name: string) { this.name = name; } }",
        );
        let items = convert_class_decl(&decl, Visibility::Private, &TypeRegistry::new()).unwrap();

        match &items[1] {
            Item::Impl { methods, .. } => {
                // Constructor body should contain `self.name = name`
                // which would be an Expr statement with assignment
                assert!(!methods[0].body.is_empty());
            }
            _ => panic!("expected Impl"),
        }
    }
}
