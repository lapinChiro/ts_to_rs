//! 継承・super constructor 処理、クラス走査。

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use swc_ecma_ast as ast;

use super::generation::{
    generate_abstract_class_items, generate_child_class_with_implements,
    generate_child_of_abstract, generate_class_with_implements, generate_items_for_class,
    generate_parent_class_items,
};
use super::helpers::find_parent_class_names;
use super::ClassInfo;
use crate::ir::{Expr, Item, Method, Stmt, Visibility};
use crate::transformer::Transformer;

/// Rewrites a child constructor to handle `super()` calls.
///
/// `super(args)` in the child constructor is removed, and the parent's field
/// initialization pattern from the constructor arguments is applied.
pub(super) fn rewrite_super_constructor(ctor: &Method, parent: &ClassInfo) -> Result<Method> {
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
    /// Pre-scans all class declarations in the module to collect inheritance info.
    ///
    /// Returns a map from class name to [`ClassInfo`]. Only classes that can be
    /// successfully parsed are included; parse failures are silently skipped
    /// (they will be reported during the main transformation pass).
    pub(crate) fn pre_scan_classes(&mut self, module: &ast::Module) -> HashMap<String, ClassInfo> {
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
