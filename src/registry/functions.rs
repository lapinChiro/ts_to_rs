//! 関数型・アロー関数定義の収集。

use anyhow::Result;
use swc_ecma_ast as ast;

use super::{TypeDef, TypeRegistry};
use crate::ir::RustType;
use crate::pipeline::type_converter::convert_ts_type;
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::collect_type_params;

/// 関数型エイリアス (`type F = (x: T) => U`) を `TypeDef::Function` として収集する。
pub(super) fn try_collect_fn_type_alias(
    alias: &ast::TsTypeAliasDecl,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Option<TypeDef> {
    match alias.type_ann.as_ref() {
        ast::TsType::TsFnOrConstructorType(ast::TsFnOrConstructorType::TsFnType(fn_type)) => {
            let mut params = Vec::new();
            for param in &fn_type.params {
                if let ast::TsFnParam::Ident(ident) = param {
                    let name = ident.id.sym.to_string();
                    if let Some(ann) = &ident.type_ann {
                        if let Ok(ty) = convert_ts_type(&ann.type_ann, synthetic, lookup) {
                            params.push((name, ty));
                        }
                    }
                }
            }
            let return_type = convert_ts_type(&fn_type.type_ann.type_ann, synthetic, lookup).ok();
            let type_params = collect_type_params(alias.type_params.as_deref(), lookup, synthetic);
            Some(TypeDef::Function {
                type_params,
                params,
                return_type,
                has_rest: false,
            })
        }
        // Object literal type with call signatures only: `type F = { (x: T): U }`
        ast::TsType::TsTypeLit(lit) => try_collect_call_signature_fn(lit, lookup, synthetic),
        _ => None,
    }
}

/// Extracts a `TypeDef::Function` from a type literal that contains only call signatures.
///
/// `type Handler = { (c: string): number }` → `TypeDef::Function { params: [(c, String)], return_type: Some(f64) }`
///
/// If the type literal contains any non-call-signature members (properties, methods, index
/// signatures), returns `None` so the type is handled as a struct instead.
/// For overloaded call signatures, picks the one with the most parameters.
fn try_collect_call_signature_fn(
    lit: &ast::TsTypeLit,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Option<TypeDef> {
    let mut call_sigs = Vec::new();
    for member in &lit.members {
        match member {
            ast::TsTypeElement::TsCallSignatureDecl(sig) => call_sigs.push(sig),
            // Non-call-signature member → not a pure function type
            _ => return None,
        }
    }

    // Pick the overload with the most parameters
    let sig = call_sigs.iter().max_by_key(|s| s.params.len())?;

    let mut params = Vec::new();
    for param in &sig.params {
        if let ast::TsFnParam::Ident(ident) = param {
            let name = ident.id.sym.to_string();
            let ty = ident
                .type_ann
                .as_ref()
                .and_then(|ann| convert_ts_type(&ann.type_ann, synthetic, lookup).ok())
                .unwrap_or(RustType::Any);
            params.push((name, ty));
        }
    }

    let return_type = sig
        .type_ann
        .as_ref()
        .and_then(|ann| convert_ts_type(&ann.type_ann, synthetic, lookup).ok());

    let type_params = collect_type_params(sig.type_params.as_deref(), lookup, synthetic);

    Some(TypeDef::Function {
        type_params,
        params,
        return_type,
        has_rest: false,
    })
}

/// 関数宣言からパラメータ型と戻り値型を収集する。インライン union で生成された enum を synthetic に収集する。
pub(super) fn collect_fn_def_with_extras(
    func: &ast::Function,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<TypeDef> {
    let mut params = Vec::new();
    let mut has_rest = false;
    for param in &func.params {
        match &param.pat {
            ast::Pat::Ident(ident) => {
                let name = ident.id.sym.to_string();
                if let Some(ann) = &ident.type_ann {
                    if let Ok(ty) = convert_ts_type(&ann.type_ann, synthetic, lookup) {
                        params.push((name, ty));
                    }
                }
            }
            ast::Pat::Assign(assign) => {
                // Default parameter: `name: Type = value` → Option<Type>
                if let ast::Pat::Ident(ident) = assign.left.as_ref() {
                    let name = ident.id.sym.to_string();
                    if let Some(ann) = &ident.type_ann {
                        if let Ok(ty) = convert_ts_type(&ann.type_ann, synthetic, lookup) {
                            params.push((name, RustType::Option(Box::new(ty))));
                        }
                    }
                }
            }
            ast::Pat::Rest(rest) => {
                has_rest = true;
                if let ast::Pat::Ident(ident) = rest.arg.as_ref() {
                    let name = ident.id.sym.to_string();
                    let type_ann = rest.type_ann.as_ref().or(ident.type_ann.as_ref());
                    if let Some(ann) = type_ann {
                        if let Ok(ty) = convert_ts_type(&ann.type_ann, synthetic, lookup) {
                            params.push((name, ty));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let return_type = func
        .return_type
        .as_ref()
        .and_then(|ann| convert_ts_type(&ann.type_ann, synthetic, lookup).ok());

    let type_params = collect_type_params(func.type_params.as_deref(), lookup, synthetic);

    Ok(TypeDef::Function {
        type_params,
        params,
        return_type,
        has_rest,
    })
}

/// アロー関数からパラメータ型と戻り値型を収集する。インライン union enum を synthetic に収集する。
pub(super) fn collect_arrow_def_with_extras(
    arrow: &ast::ArrowExpr,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<TypeDef> {
    let mut params = Vec::new();
    for param in &arrow.params {
        if let ast::Pat::Ident(ident) = param {
            let name = ident.id.sym.to_string();
            if let Some(ann) = &ident.type_ann {
                if let Ok(ty) = convert_ts_type(&ann.type_ann, synthetic, lookup) {
                    params.push((name, ty));
                }
            }
        }
    }

    let return_type = arrow
        .return_type
        .as_ref()
        .and_then(|ann| convert_ts_type(&ann.type_ann, synthetic, lookup).ok());

    let type_params = collect_type_params(arrow.type_params.as_deref(), lookup, synthetic);

    Ok(TypeDef::Function {
        type_params,
        params,
        return_type,
        has_rest: false,
    })
}
