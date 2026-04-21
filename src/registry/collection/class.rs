//! Class-specific declaration collection: extract constructor, fields,
//! and methods from a TS `class` declaration and build the
//! [`TypeDef<TsTypeInfo>`](crate::registry::TypeDef) that Pass 2 then
//! resolves via [`super::resolvers::resolve_struct_for_registry`].
//!
//! Used exclusively by [`super::decl::collect_decl`] for the
//! `ast::Decl::Class` branch.

use std::collections::HashMap;

use swc_ecma_ast as ast;

use crate::registry::{FieldDef, MethodSignature, ParamDef, TypeDef};
use crate::ts_type_info::{convert_to_ts_type_info, TsTypeInfo};

use super::collect_type_params;

pub(super) fn collect_class_info(class: &ast::ClassDecl) -> TypeDef<TsTypeInfo> {
    let mut fields = Vec::new();
    let mut methods: HashMap<String, Vec<MethodSignature<TsTypeInfo>>> = HashMap::new();
    let mut constructor_sigs: Vec<MethodSignature<TsTypeInfo>> = Vec::new();

    for member in &class.class.body {
        match member {
            ast::ClassMember::ClassProp(prop) => {
                let name = match &prop.key {
                    ast::PropName::Ident(ident) => ident.sym.to_string(),
                    _ => continue,
                };
                if let Some(ann) = &prop.type_ann {
                    if let Ok(ty) = convert_to_ts_type_info(&ann.type_ann) {
                        fields.push(FieldDef {
                            name,
                            ty,
                            optional: prop.is_optional,
                        });
                    }
                }
            }
            ast::ClassMember::PrivateProp(prop) => {
                let name = prop.key.name.to_string();
                if let Some(ann) = &prop.type_ann {
                    if let Ok(ty) = convert_to_ts_type_info(&ann.type_ann) {
                        fields.push(FieldDef {
                            name,
                            ty,
                            optional: prop.is_optional,
                        });
                    }
                }
            }
            ast::ClassMember::Constructor(ctor) => {
                let params: Vec<ParamDef<TsTypeInfo>> = ctor
                    .params
                    .iter()
                    .filter_map(|p| match p {
                        ast::ParamOrTsParamProp::Param(param) => {
                            let ident = match &param.pat {
                                ast::Pat::Ident(ident) => ident,
                                _ => return None,
                            };
                            let ty = ident
                                .type_ann
                                .as_ref()
                                .and_then(|ann| convert_to_ts_type_info(&ann.type_ann).ok())?;
                            Some(ParamDef {
                                name: ident.id.sym.to_string(),
                                ty,
                                optional: ident.id.optional,
                                has_default: false,
                            })
                        }
                        ast::ParamOrTsParamProp::TsParamProp(param_prop) => {
                            let ident = match &param_prop.param {
                                ast::TsParamPropParam::Ident(ident) => ident,
                                _ => return None,
                            };
                            let ty = ident
                                .type_ann
                                .as_ref()
                                .and_then(|ann| convert_to_ts_type_info(&ann.type_ann).ok())?;
                            Some(ParamDef {
                                name: ident.id.sym.to_string(),
                                ty,
                                optional: ident.id.optional,
                                has_default: false,
                            })
                        }
                    })
                    .collect();
                let has_rest = ctor.params.iter().any(|p| {
                    matches!(
                        p,
                        ast::ParamOrTsParamProp::Param(param) if matches!(&param.pat, ast::Pat::Rest(_))
                    )
                });
                constructor_sigs.push(MethodSignature {
                    params,
                    return_type: None,
                    has_rest,
                    // I-383 T8': constructor は通常 generic を持たないが、TS の `class C<T> {
                    // constructor<U>(...) }` のような構文があれば ctor.function.type_params から
                    // 抽出する。現状の SWC AST の Constructor では type_params は直接持てないため
                    // 空 vec で OK。
                    type_params: vec![],
                });
            }
            ast::ClassMember::Method(method) => {
                let name = match &method.key {
                    ast::PropName::Ident(ident) => ident.sym.to_string(),
                    _ => continue,
                };
                if let Some(func) = &method.function.body {
                    let _ = func; // body exists, collect params
                }
                let params: Vec<ParamDef<TsTypeInfo>> = method
                    .function
                    .params
                    .iter()
                    .filter_map(|param| crate::registry::functions::extract_pat_param(&param.pat))
                    .collect();
                let return_type = method
                    .function
                    .return_type
                    .as_ref()
                    .and_then(|ann| convert_to_ts_type_info(&ann.type_ann).ok());
                let has_rest = method
                    .function
                    .params
                    .iter()
                    .any(|param| matches!(&param.pat, ast::Pat::Rest(_)));
                // I-383 T8': メソッド自身の generic 型パラメータを抽出する。
                // 例: `class C<S> { foo<M extends string>(x: M | M[]) }` の `<M>`。
                // 抽出した type_params は `resolve_method_sig` で scope に push され、
                // 戻り値型・パラメータ型解決中の anonymous union の generic 化に使われる。
                let method_type_params =
                    collect_type_params(method.function.type_params.as_deref());
                methods.entry(name).or_default().push(MethodSignature {
                    params,
                    return_type,
                    has_rest,
                    type_params: method_type_params,
                });
            }
            _ => {}
        }
    }

    let type_params = collect_type_params(class.class.type_params.as_deref());
    let constructor = if constructor_sigs.is_empty() {
        None
    } else {
        Some(constructor_sigs)
    };
    TypeDef::Struct {
        type_params,
        fields,
        methods,
        constructor,
        call_signatures: vec![],
        extends: vec![],
        is_interface: false,
    }
}
