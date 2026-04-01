//! 関数型・アロー関数定義の収集。

use anyhow::Result;
use swc_ecma_ast as ast;

use super::{ParamDef, TypeDef, TypeRegistry};
use crate::ir::RustType;
use crate::pipeline::type_converter::convert_ts_type;
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::collect_type_params;

/// Rest パラメータ（`...args: T[]`）から名前と型を抽出する。
///
/// `TsFnParam::Rest` と `Pat::Rest` の両方で使われる共通ロジック。
/// arg から名前を取得し、type_ann（fallback: arg の type_ann）から型を変換する。
fn extract_rest_param(
    rest: &ast::RestPat,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Option<(String, RustType)> {
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
    let ty = type_ann.and_then(|ann| convert_ts_type(&ann.type_ann, synthetic, lookup).ok())?;
    Some((name, ty))
}

/// `BindingIdent` から名前と型を抽出する。
///
/// `TsFnParam::Ident` と `Pat::Ident` の共通ロジック。
fn extract_ident_param(
    ident: &ast::BindingIdent,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Option<(String, RustType)> {
    let name = ident.id.sym.to_string();
    let ty = ident
        .type_ann
        .as_ref()
        .and_then(|ann| convert_ts_type(&ann.type_ann, synthetic, lookup).ok())?;
    Some((name, ty))
}

/// `TsFnParam`（interface メソッド・call signature のパラメータ）から名前と型を抽出する。
///
/// - `TsFnParam::Ident`: 名前 + 型注釈から変換
/// - `TsFnParam::Rest`: arg から名前、type_ann から型を取得（fallback: arg の type_ann）
/// - その他: `None`
pub(super) fn extract_ts_fn_param(
    param: &ast::TsFnParam,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Option<ParamDef> {
    match param {
        ast::TsFnParam::Ident(ident) => {
            let (name, ty) = extract_ident_param(ident, lookup, synthetic)?;
            Some(ParamDef {
                name,
                ty,
                optional: ident.id.optional,
                has_default: false,
            })
        }
        ast::TsFnParam::Rest(rest) => {
            let (name, ty) = extract_rest_param(rest, lookup, synthetic)?;
            Some(ParamDef {
                name,
                ty,
                optional: false,
                has_default: false,
            })
        }
        _ => None,
    }
}

/// `Pat`（関数宣言・アロー関数のパラメータ）から名前と型を抽出する。
///
/// - `Pat::Ident`: 名前 + 型注釈から変換
/// - `Pat::Assign`: デフォルトパラメータ。左辺の Ident から型を取得し `Option<T>` でラップ
/// - `Pat::Rest`: arg から名前、type_ann から型を取得（fallback: arg の type_ann）
/// - その他: `None`
pub(super) fn extract_pat_param(
    pat: &ast::Pat,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Option<ParamDef> {
    match pat {
        ast::Pat::Ident(ident) => {
            let (name, ty) = extract_ident_param(ident, lookup, synthetic)?;
            Some(ParamDef {
                name,
                ty,
                optional: ident.id.optional,
                has_default: false,
            })
        }
        ast::Pat::Assign(assign) => {
            if let ast::Pat::Ident(ident) = assign.left.as_ref() {
                let (name, ty) = extract_ident_param(ident, lookup, synthetic)?;
                Some(ParamDef {
                    name,
                    ty: RustType::Option(Box::new(ty)),
                    optional: false,
                    has_default: true,
                })
            } else {
                None
            }
        }
        ast::Pat::Rest(rest) => {
            let (name, ty) = extract_rest_param(rest, lookup, synthetic)?;
            Some(ParamDef {
                name,
                ty,
                optional: false,
                has_default: false,
            })
        }
        _ => None,
    }
}

/// 関数型エイリアス (`type F = (x: T) => U`) を `TypeDef::Function` として収集する。
pub(super) fn try_collect_fn_type_alias(
    alias: &ast::TsTypeAliasDecl,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Option<TypeDef> {
    match alias.type_ann.as_ref() {
        ast::TsType::TsFnOrConstructorType(ast::TsFnOrConstructorType::TsFnType(fn_type)) => {
            let params: Vec<ParamDef> = fn_type
                .params
                .iter()
                .filter_map(|p| extract_ts_fn_param(p, lookup, synthetic))
                .collect();
            let has_rest = fn_type
                .params
                .iter()
                .any(|p| matches!(p, ast::TsFnParam::Rest(_)));
            let return_type = convert_ts_type(&fn_type.type_ann.type_ann, synthetic, lookup).ok();
            let type_params = collect_type_params(alias.type_params.as_deref(), lookup, synthetic);
            Some(TypeDef::Function {
                type_params,
                params,
                return_type,
                has_rest,
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
    if !super::interfaces::is_callable_only(&lit.members) {
        return None;
    }

    let call_sigs: Vec<&ast::TsCallSignatureDecl> = lit
        .members
        .iter()
        .filter_map(|m| match m {
            ast::TsTypeElement::TsCallSignatureDecl(sig) => Some(sig),
            _ => None,
        })
        .collect();

    // Pick the overload with the most parameters
    let sig = call_sigs.iter().max_by_key(|s| s.params.len())?;

    let params: Vec<ParamDef> = sig
        .params
        .iter()
        .filter_map(|p| extract_ts_fn_param(p, lookup, synthetic))
        .collect();
    let has_rest = sig
        .params
        .iter()
        .any(|p| matches!(p, ast::TsFnParam::Rest(_)));

    let return_type = sig
        .type_ann
        .as_ref()
        .and_then(|ann| convert_ts_type(&ann.type_ann, synthetic, lookup).ok());

    let type_params = collect_type_params(sig.type_params.as_deref(), lookup, synthetic);

    Some(TypeDef::Function {
        type_params,
        params,
        return_type,
        has_rest,
    })
}

/// 関数宣言からパラメータ型と戻り値型を収集する。インライン union で生成された enum を synthetic に収集する。
pub(super) fn collect_fn_def_with_extras(
    func: &ast::Function,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> Result<TypeDef> {
    let params: Vec<ParamDef> = func
        .params
        .iter()
        .filter_map(|param| extract_pat_param(&param.pat, lookup, synthetic))
        .collect();
    let has_rest = func
        .params
        .iter()
        .any(|param| matches!(&param.pat, ast::Pat::Rest(_)));

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
    let params: Vec<ParamDef> = arrow
        .params
        .iter()
        .filter_map(|param| extract_pat_param(param, lookup, synthetic))
        .collect();
    let has_rest = arrow
        .params
        .iter()
        .any(|param| matches!(param, ast::Pat::Rest(_)));

    let return_type = arrow
        .return_type
        .as_ref()
        .and_then(|ann| convert_ts_type(&ann.type_ann, synthetic, lookup).ok());

    let type_params = collect_type_params(arrow.type_params.as_deref(), lookup, synthetic);

    Ok(TypeDef::Function {
        type_params,
        params,
        return_type,
        has_rest,
    })
}
