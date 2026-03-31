//! Class declaration conversion from SWC TypeScript AST to IR.
//!
//! Converts SWC class declarations into IR [`Item::Struct`] + [`Item::Impl`].

mod generation;
mod helpers;
mod inheritance;
mod members;

#[cfg(test)]
mod tests;

use anyhow::Result;
use swc_ecma_ast as ast;

use crate::ir::{RustType, TraitRef, TypeParam, Visibility};
use crate::pipeline::type_converter::convert_ts_type;
use crate::transformer::Transformer;

pub(super) use helpers::pre_scan_interface_methods;

/// Extracted class information for resolving inheritance relationships.
#[derive(Debug, Clone)]
pub struct ClassInfo {
    /// Class name
    pub name: String,
    /// Generic type parameters
    pub type_params: Vec<TypeParam>,
    /// Parent class name (from `extends`)
    pub parent: Option<String>,
    /// Parent class type arguments (e.g., `extends Parent<string>` → `[String]`)
    pub parent_type_args: Vec<RustType>,
    /// Struct fields
    pub fields: Vec<crate::ir::StructField>,
    /// Constructor method (if any)
    pub constructor: Option<crate::ir::Method>,
    /// Regular methods (excluding constructor)
    pub methods: Vec<crate::ir::Method>,
    /// Visibility
    pub vis: Visibility,
    /// Interface references from `implements` clause (name + type arguments)
    pub implements: Vec<TraitRef>,
    /// Whether this class is abstract
    pub is_abstract: bool,
    /// Static properties (converted to associated constants)
    pub static_consts: Vec<crate::ir::AssocConst>,
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
        let name = crate::pipeline::type_converter::sanitize_rust_type_name(&class_decl.ident.sym);
        let parent = class_decl.class.super_class.as_ref().and_then(|sc| {
            if let ast::Expr::Ident(ident) = sc.as_ref() {
                Some(crate::pipeline::type_converter::sanitize_rust_type_name(
                    &ident.sym,
                ))
            } else {
                None
            }
        });
        let parent_type_args: Vec<RustType> = class_decl
            .class
            .super_type_params
            .as_ref()
            .map(|tp| {
                tp.params
                    .iter()
                    .filter_map(|t| convert_ts_type(t, self.synthetic, self.reg()).ok())
                    .collect()
            })
            .unwrap_or_default();

        let implements: Vec<TraitRef> = class_decl
            .class
            .implements
            .iter()
            .filter_map(|impl_clause| {
                if let ast::Expr::Ident(ident) = impl_clause.expr.as_ref() {
                    let type_args = impl_clause
                        .type_args
                        .as_ref()
                        .map(|ta| {
                            ta.params
                                .iter()
                                .filter_map(|t| convert_ts_type(t, self.synthetic, self.reg()).ok())
                                .collect()
                        })
                        .unwrap_or_default();
                    Some(TraitRef {
                        name: crate::pipeline::type_converter::sanitize_rust_type_name(&ident.sym),
                        type_args,
                    })
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
                    let (method, param_prop_fields) = self.convert_constructor(ctor, &vis)?;
                    constructor = Some(method);
                    fields.extend(param_prop_fields);
                }
                ast::ClassMember::Method(method) => {
                    methods.push(self.convert_class_method(method, &vis)?);
                }
                ast::ClassMember::PrivateMethod(pm) => {
                    methods.push(self.convert_private_method(pm)?);
                }
                ast::ClassMember::PrivateProp(pp) => {
                    if pp.is_static {
                        // Static private props — skip for now (rare pattern)
                    } else {
                        fields.push(self.convert_private_prop(pp)?);
                    }
                }
                ast::ClassMember::StaticBlock(sb) => {
                    methods.push(self.convert_static_block(sb)?);
                }
                ast::ClassMember::TsIndexSignature(_) | ast::ClassMember::Empty(_) => {}
                ast::ClassMember::AutoAccessor(aa) => {
                    return Err(crate::transformer::UnsupportedSyntaxError::new(
                        "AutoAccessor",
                        aa.span,
                    )
                    .into());
                }
            }
        }

        let type_params = crate::registry::collect_type_params(
            class_decl.class.type_params.as_deref(),
            self.reg(),
            self.synthetic,
        );

        Ok(ClassInfo {
            name,
            type_params,
            parent,
            parent_type_args,
            fields,
            constructor,
            methods,
            vis,
            implements,
            is_abstract: class_decl.class.is_abstract,
            static_consts,
        })
    }
}
