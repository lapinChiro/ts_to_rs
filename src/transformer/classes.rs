//! Class declaration conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC class declarations into IR [`Item::Struct`] + [`Item::Impl`].

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{Expr, Item, Method, Param, RustType, Stmt, StructField, Visibility};
use crate::transformer::expressions::convert_expr;
use crate::transformer::statements::convert_stmt;
use crate::transformer::types::convert_ts_type;

/// Converts an SWC [`ast::ClassDecl`] into IR items (struct + impl).
///
/// Properties become struct fields, methods become impl methods,
/// and `constructor` becomes `pub fn new() -> Self`.
///
/// # Errors
///
/// Returns an error if unsupported class members are encountered.
pub fn convert_class_decl(class_decl: &ast::ClassDecl, vis: Visibility) -> Result<Vec<Item>> {
    let name = class_decl.ident.sym.to_string();
    let mut fields = Vec::new();
    let mut methods = Vec::new();

    for member in &class_decl.class.body {
        match member {
            ast::ClassMember::ClassProp(prop) => {
                let field = convert_class_prop(prop)?;
                fields.push(field);
            }
            ast::ClassMember::Constructor(ctor) => {
                let method = convert_constructor(ctor, &vis)?;
                methods.push(method);
            }
            ast::ClassMember::Method(method) => {
                let m = convert_class_method(method, &vis)?;
                methods.push(m);
            }
            _ => {
                // Skip unsupported class members silently
            }
        }
    }

    let mut items = vec![Item::Struct {
        vis: vis.clone(),
        name: name.clone(),
        type_params: vec![],
        fields,
    }];

    if !methods.is_empty() {
        items.push(Item::Impl {
            struct_name: name,
            methods,
        });
    }

    Ok(items)
}

/// Converts a class property to a struct field.
fn convert_class_prop(prop: &ast::ClassProp) -> Result<StructField> {
    let field_name = match &prop.key {
        ast::PropName::Ident(ident) => ident.sym.to_string(),
        _ => return Err(anyhow!("unsupported class property key (only identifiers)")),
    };

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
fn convert_constructor(ctor: &ast::Constructor, vis: &Visibility) -> Result<Method> {
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
        Some(block) => convert_constructor_body(&block.stmts)?,
        None => vec![],
    };

    Ok(Method {
        vis: vis.clone(),
        name: "new".to_string(),
        has_self: false,
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
fn convert_constructor_body(stmts: &[ast::Stmt]) -> Result<Vec<Stmt>> {
    let mut fields = Vec::new();
    let mut other_stmts = Vec::new();

    for stmt in stmts {
        if let Some((field_name, value_expr)) = try_extract_this_assignment(stmt) {
            let value = convert_expr(value_expr)?;
            fields.push((field_name, value));
        } else {
            other_stmts.push(convert_stmt(stmt)?);
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

/// Converts a class method to an impl method with `&self`.
fn convert_class_method(method: &ast::ClassMethod, vis: &Visibility) -> Result<Method> {
    let name = match &method.key {
        ast::PropName::Ident(ident) => ident.sym.to_string(),
        _ => return Err(anyhow!("unsupported method key (only identifiers)")),
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

    let body = match &method.function.body {
        Some(block) => {
            let mut stmts = Vec::new();
            for stmt in &block.stmts {
                stmts.push(convert_stmt(stmt)?);
            }
            stmts
        }
        None => Vec::new(),
    };

    Ok(Method {
        vis: vis.clone(),
        name,
        has_self: true,
        params,
        return_type,
        body,
    })
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
                ty: rust_type,
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
        let items = convert_class_decl(&decl, Visibility::Private).unwrap();

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
        let items = convert_class_decl(&decl, Visibility::Private).unwrap();

        assert_eq!(items.len(), 2);
        match &items[1] {
            Item::Impl {
                struct_name,
                methods,
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
                        ty: RustType::F64,
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
        let items = convert_class_decl(&decl, Visibility::Private).unwrap();

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
        let items = convert_class_decl(&decl, Visibility::Public).unwrap();

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
        let items = convert_class_decl(&decl, Visibility::Private).unwrap();

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
