//! interface のフィールド・メソッドシグネチャ収集。

use std::collections::HashMap;

use anyhow::Result;
use swc_ecma_ast as ast;

use super::{MethodSignature, TypeRegistry};
use crate::ir::RustType;
use crate::pipeline::type_converter::convert_ts_type;
use crate::pipeline::SyntheticTypeRegistry;

/// interface のフィールド名・型を収集する。
pub(super) fn collect_interface_fields(
    iface: &ast::TsInterfaceDecl,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<Vec<(String, RustType)>> {
    let mut fields = Vec::new();
    for member in &iface.body.body {
        if let ast::TsTypeElement::TsPropertySignature(prop) = member {
            if let Some((name, ty)) = collect_property_signature(prop, lookup, synthetic) {
                fields.push((name, ty));
            }
        }
    }
    Ok(fields)
}

/// interface のメソッドシグネチャを収集する。
pub(super) fn collect_interface_methods(
    iface: &ast::TsInterfaceDecl,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> HashMap<String, Vec<MethodSignature>> {
    let mut methods: HashMap<String, Vec<MethodSignature>> = HashMap::new();
    for member in &iface.body.body {
        if let ast::TsTypeElement::TsMethodSignature(method) = member {
            let name = match method.key.as_ref() {
                ast::Expr::Ident(ident) => ident.sym.to_string(),
                _ => continue,
            };
            let params: Vec<(String, RustType)> = method
                .params
                .iter()
                .filter_map(|param| match param {
                    ast::TsFnParam::Ident(ident) => {
                        let name = ident.id.sym.to_string();
                        let ty = ident.type_ann.as_ref().and_then(|ann| {
                            convert_ts_type(&ann.type_ann, synthetic, lookup).ok()
                        })?;
                        Some((name, ty))
                    }
                    ast::TsFnParam::Rest(rest) => {
                        let name = match rest.arg.as_ref() {
                            ast::Pat::Ident(ident) => ident.id.sym.to_string(),
                            _ => "rest".to_string(),
                        };
                        let type_ann = rest.type_ann.as_ref().or_else(|| {
                            if let ast::Pat::Ident(ident) = rest.arg.as_ref() {
                                ident.type_ann.as_ref()
                            } else {
                                None
                            }
                        });
                        let ty = type_ann.and_then(|ann| {
                            convert_ts_type(&ann.type_ann, synthetic, lookup).ok()
                        })?;
                        Some((name, ty))
                    }
                    _ => None,
                })
                .collect();
            let return_type = method
                .type_ann
                .as_ref()
                .and_then(|ann| convert_ts_type(&ann.type_ann, synthetic, lookup).ok());
            let has_rest = method
                .params
                .iter()
                .any(|p| matches!(p, ast::TsFnParam::Rest(_)));
            methods.entry(name).or_default().push(MethodSignature {
                params,
                return_type,
                has_rest,
            });
        }
    }
    methods
}

/// TsPropertySignature からフィールド名と型を取得する。
pub(super) fn collect_property_signature(
    prop: &ast::TsPropertySignature,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Option<(String, RustType)> {
    let name = match prop.key.as_ref() {
        ast::Expr::Ident(ident) => ident.sym.to_string(),
        _ => return None,
    };
    let ty = prop
        .type_ann
        .as_ref()
        .and_then(|ann| convert_ts_type(&ann.type_ann, synthetic, lookup).ok())?;

    // Optional fields are wrapped in Option
    let ty = if prop.optional {
        RustType::Option(Box::new(ty))
    } else {
        ty
    };

    Some((name, ty))
}
