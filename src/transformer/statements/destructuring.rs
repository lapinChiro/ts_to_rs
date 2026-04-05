//! Object and array destructuring conversion.
//!
//! Converts TypeScript destructuring patterns (`const { a, b } = obj`,
//! `const [x, y] = arr`) into sequences of IR `Stmt::Let` statements.

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use crate::ir::{Expr, RustType, Stmt};
use crate::transformer::expressions::member_access::{
    build_safe_index_expr_unwrapped, convert_index_to_usize,
};
use crate::transformer::{
    extract_pat_ident_name, extract_prop_name, single_declarator, Transformer,
};

impl<'a> Transformer<'a> {
    /// Tries to convert a variable declaration with object destructuring pattern.
    pub(super) fn try_convert_object_destructuring(
        &mut self,
        var_decl: &ast::VarDecl,
    ) -> Result<Option<Vec<Stmt>>> {
        let declarator = match single_declarator(var_decl) {
            Ok(d) => d,
            Err(_) => return Ok(None),
        };

        let obj_pat = match &declarator.name {
            ast::Pat::Object(obj_pat) => obj_pat,
            _ => return Ok(None),
        };

        let source = declarator
            .init
            .as_ref()
            .ok_or_else(|| anyhow!("object destructuring requires an initializer"))?;
        let source_expr = self.convert_expr(source)?;

        let mutable = false;
        let source_type = self.get_expr_type(source);
        let mut stmts = Vec::new();

        self.expand_object_pat_props(
            &obj_pat.props,
            &source_expr,
            mutable,
            &mut stmts,
            source_type,
        )?;

        Ok(Some(stmts))
    }

    /// Recursively expands object destructuring pattern properties into `let` statements.
    pub(super) fn expand_object_pat_props(
        &mut self,
        props: &[ast::ObjectPatProp],
        source_expr: &Expr,
        mutable: bool,
        stmts: &mut Vec<Stmt>,
        source_type: Option<&RustType>,
    ) -> Result<()> {
        for prop in props {
            match prop {
                ast::ObjectPatProp::Assign(assign) => {
                    let field_name = assign.key.sym.to_string();
                    let field_access = Expr::FieldAccess {
                        object: Box::new(source_expr.clone()),
                        field: field_name.clone(),
                    };
                    let init_expr = if let Some(default_expr) = &assign.value {
                        let default_ir = self.convert_expr(default_expr)?;
                        crate::transformer::build_option_unwrap_with_default(
                            field_access,
                            default_ir,
                        )
                    } else {
                        field_access
                    };
                    stmts.push(Stmt::Let {
                        mutable,
                        name: field_name,
                        ty: None,
                        init: Some(init_expr),
                    });
                }
                ast::ObjectPatProp::KeyValue(kv) => {
                    let field_name = extract_prop_name(&kv.key)
                        .map_err(|_| anyhow!("unsupported destructuring key"))?;
                    let nested_source = Expr::FieldAccess {
                        object: Box::new(source_expr.clone()),
                        field: field_name,
                    };
                    match kv.value.as_ref() {
                        ast::Pat::Object(inner_pat) => {
                            self.expand_object_pat_props(
                                &inner_pat.props,
                                &nested_source,
                                mutable,
                                stmts,
                                None,
                            )?;
                        }
                        _ => {
                            let binding_name = extract_pat_ident_name(kv.value.as_ref())
                                .map_err(|_| anyhow!("unsupported destructuring value pattern"))?;
                            stmts.push(Stmt::Let {
                                mutable,
                                name: binding_name,
                                ty: None,
                                init: Some(nested_source),
                            });
                        }
                    }
                }
                ast::ObjectPatProp::Rest(_rest) => {
                    let explicit_fields: Vec<String> = props
                        .iter()
                        .filter_map(|p| match p {
                            ast::ObjectPatProp::Assign(a) => Some(a.key.sym.to_string()),
                            ast::ObjectPatProp::KeyValue(kv) => extract_prop_name(&kv.key).ok(),
                            _ => None,
                        })
                        .collect();

                    let type_name = source_type.and_then(|ty| match ty {
                        RustType::Named { name, .. } => Some(name.as_str()),
                        _ => None,
                    });
                    if let Some(crate::registry::TypeDef::Struct { fields, .. }) =
                        type_name.and_then(|n| self.reg().get(n))
                    {
                        for field in fields {
                            if !explicit_fields.contains(&field.name) {
                                stmts.push(Stmt::Let {
                                    mutable,
                                    name: field.name.clone(),
                                    ty: None,
                                    init: Some(Expr::FieldAccess {
                                        object: Box::new(source_expr.clone()),
                                        field: field.name.clone(),
                                    }),
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Tries to convert a variable declaration with array destructuring pattern.
    pub(super) fn try_convert_array_destructuring(
        &mut self,
        var_decl: &ast::VarDecl,
    ) -> Result<Option<Vec<Stmt>>> {
        let declarator = match single_declarator(var_decl) {
            Ok(d) => d,
            Err(_) => return Ok(None),
        };

        let arr_pat = match &declarator.name {
            ast::Pat::Array(arr_pat) => arr_pat,
            _ => return Ok(None),
        };

        let source = declarator
            .init
            .as_ref()
            .ok_or_else(|| anyhow!("array destructuring requires an initializer"))?;
        // Cat A: destructuring source
        let source_expr = self.convert_expr(source)?;

        let mutable = false;
        let mut stmts = Vec::new();

        for (i, elem) in arr_pat.elems.iter().enumerate() {
            let pat = match elem {
                Some(pat) => pat,
                None => continue, // skip hole: `[a, , b]`
            };

            // Rest element: `[first, ...rest]`
            if let ast::Pat::Rest(rest_pat) = pat {
                let name = extract_pat_ident_name(&rest_pat.arg)?;
                stmts.push(Stmt::Let {
                    mutable,
                    name,
                    ty: None,
                    init: Some(Expr::MethodCall {
                        object: Box::new(Expr::Index {
                            object: Box::new(source_expr.clone()),
                            index: Box::new(Expr::Range {
                                start: Some(Box::new(Expr::NumberLit(i as f64))),
                                end: None,
                            }),
                        }),
                        method: "to_vec".to_string(),
                        args: vec![],
                    }),
                });
                break; // rest must be last
            }

            let name = extract_pat_ident_name(pat)?;
            let safe_index = convert_index_to_usize(Expr::NumberLit(i as f64));
            stmts.push(Stmt::Let {
                mutable,
                name,
                ty: None,
                init: Some(build_safe_index_expr_unwrapped(
                    source_expr.clone(),
                    safe_index,
                )),
            });
        }

        Ok(Some(stmts))
    }
}
