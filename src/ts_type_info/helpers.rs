//! Subtree converters invoked by the main `convert_to_ts_type_info`
//! dispatcher:
//!
//! - [`convert_type_lit_members`] — `TsTypeElement[]` →
//!   [`super::TsTypeLiteralInfo`] (fields / methods / call sigs /
//!   construct sigs / index sigs)
//! - [`extract_sig_params`] — `TsFnParam[]` → `(Vec<TsParamInfo>,
//!   has_rest)` for method / call / construct sigs
//! - [`extract_fn_params`] — `TsFnParam[]` → `Vec<TsParamInfo>` for
//!   the `Function` variant (no `has_rest` return)
//! - [`extract_entity_name`] — `TsEntityName` → dot-joined `String`
//!
//! These live in a separate file purely for file-size reasons — they
//! form a single logical cluster with `convert_to_ts_type_info` and
//! remain `pub(super)` since they are not part of the crate's public
//! API.

use super::{
    convert_to_ts_type_info, TsFieldInfo, TsFnSigInfo, TsIndexSigInfo, TsMethodInfo, TsParamInfo,
    TsTypeInfo, TsTypeLiteralInfo,
};

/// TsTypeLit のメンバーリストから TsTypeLiteralInfo を構築する。
pub(super) fn convert_type_lit_members(
    members: &[swc_ecma_ast::TsTypeElement],
) -> anyhow::Result<TsTypeLiteralInfo> {
    use swc_ecma_ast as ast;

    let mut fields = Vec::new();
    let mut methods = Vec::new();
    let mut call_signatures = Vec::new();
    let mut construct_signatures = Vec::new();
    let mut index_signatures = Vec::new();

    for member in members {
        match member {
            ast::TsTypeElement::TsPropertySignature(prop) => {
                let name = match prop.key.as_ref() {
                    ast::Expr::Ident(ident) => ident.sym.to_string(),
                    _ => continue,
                };
                let ty = prop
                    .type_ann
                    .as_ref()
                    .map(|ann| convert_to_ts_type_info(&ann.type_ann))
                    .transpose()?
                    .unwrap_or(TsTypeInfo::Any);
                fields.push(TsFieldInfo {
                    name,
                    ty,
                    optional: prop.optional,
                });
            }
            ast::TsTypeElement::TsMethodSignature(sig) => {
                let name = match sig.key.as_ref() {
                    ast::Expr::Ident(ident) => ident.sym.to_string(),
                    _ => continue,
                };
                let (params, has_rest) = extract_sig_params(&sig.params);
                let return_type = sig
                    .type_ann
                    .as_ref()
                    .map(|ann| convert_to_ts_type_info(&ann.type_ann))
                    .transpose()?;
                let type_params = sig
                    .type_params
                    .as_ref()
                    .map(|tp| tp.params.iter().map(|p| p.name.sym.to_string()).collect())
                    .unwrap_or_default();
                methods.push(TsMethodInfo {
                    name,
                    params,
                    return_type,
                    type_params,
                    optional: sig.optional,
                    has_rest,
                    kind: crate::registry::MethodKind::Method,
                });
            }
            ast::TsTypeElement::TsCallSignatureDecl(decl) => {
                let (params, has_rest) = extract_sig_params(&decl.params);
                let return_type = decl
                    .type_ann
                    .as_ref()
                    .map(|ann| convert_to_ts_type_info(&ann.type_ann))
                    .transpose()?;
                call_signatures.push(TsFnSigInfo {
                    params,
                    return_type,
                    has_rest,
                });
            }
            ast::TsTypeElement::TsConstructSignatureDecl(decl) => {
                let (params, has_rest) = extract_sig_params(&decl.params);
                let return_type = decl
                    .type_ann
                    .as_ref()
                    .map(|ann| convert_to_ts_type_info(&ann.type_ann))
                    .transpose()?;
                construct_signatures.push(TsFnSigInfo {
                    params,
                    return_type,
                    has_rest,
                });
            }
            ast::TsTypeElement::TsIndexSignature(idx) => {
                // インデックスパラメータの抽出
                if let Some(param) = idx.params.first() {
                    let param_name = match param {
                        ast::TsFnParam::Ident(ident) => ident.id.sym.to_string(),
                        _ => "key".to_string(),
                    };
                    let param_type = match param {
                        ast::TsFnParam::Ident(ident) => ident
                            .type_ann
                            .as_ref()
                            .map(|ann| convert_to_ts_type_info(&ann.type_ann))
                            .transpose()?
                            .unwrap_or(TsTypeInfo::String),
                        _ => TsTypeInfo::String,
                    };
                    let value_type = idx
                        .type_ann
                        .as_ref()
                        .map(|ann| convert_to_ts_type_info(&ann.type_ann))
                        .transpose()?
                        .unwrap_or(TsTypeInfo::Any);
                    index_signatures.push(TsIndexSigInfo {
                        param_name,
                        param_type,
                        value_type,
                        readonly: idx.readonly,
                    });
                }
            }
            // getter/setter は現時点では非対応（変換パイプラインでもスキップされている）
            _ => continue,
        }
    }

    Ok(TsTypeLiteralInfo {
        fields,
        methods,
        call_signatures,
        construct_signatures,
        index_signatures,
    })
}

/// TsFnParam のリストからシグネチャパラメータ情報を抽出する。
///
/// (params, has_rest) のタプルを返す。
pub(super) fn extract_sig_params(params: &[swc_ecma_ast::TsFnParam]) -> (Vec<TsParamInfo>, bool) {
    let mut result = Vec::new();
    let mut has_rest = false;

    for p in params {
        match p {
            swc_ecma_ast::TsFnParam::Ident(ident) => {
                let ty = ident
                    .type_ann
                    .as_ref()
                    .and_then(|a| convert_to_ts_type_info(&a.type_ann).ok())
                    .unwrap_or(TsTypeInfo::Any);
                result.push(TsParamInfo {
                    name: ident.id.sym.to_string(),
                    ty,
                    optional: ident.optional,
                });
            }
            swc_ecma_ast::TsFnParam::Rest(rest) => {
                has_rest = true;
                let ty = rest
                    .type_ann
                    .as_ref()
                    .and_then(|a| convert_to_ts_type_info(&a.type_ann).ok())
                    .unwrap_or(TsTypeInfo::Any);
                let name = match rest.arg.as_ref() {
                    swc_ecma_ast::Pat::Ident(ident) => ident.id.sym.to_string(),
                    _ => "rest".to_string(),
                };
                result.push(TsParamInfo {
                    name,
                    ty,
                    optional: false,
                });
            }
            _ => {
                // Object/Array パターンのパラメータはスキップ
                continue;
            }
        }
    }

    (result, has_rest)
}

/// TsFnParam のリストからパラメータ情報を抽出する（Function variant 用）。
///
/// I-040: `Ident` パラメータの `optional` フラグを `TsParamInfo.optional` として
/// 保持する。`Rest` パラメータは optional ではない (rest と optional は TS 文法上
/// 排他)。
pub(super) fn extract_fn_params(params: &[swc_ecma_ast::TsFnParam]) -> Vec<TsParamInfo> {
    params
        .iter()
        .filter_map(|p| {
            let (ann, optional, name) = match p {
                swc_ecma_ast::TsFnParam::Ident(ident) => (
                    ident.type_ann.as_ref(),
                    ident.id.optional,
                    ident.id.sym.to_string(),
                ),
                swc_ecma_ast::TsFnParam::Rest(rest) => {
                    let name = match rest.arg.as_ref() {
                        swc_ecma_ast::Pat::Ident(i) => i.id.sym.to_string(),
                        _ => "rest".to_string(),
                    };
                    (rest.type_ann.as_ref(), false, name)
                }
                _ => return None,
            };
            let ty = convert_to_ts_type_info(&ann?.type_ann).ok()?;
            Some(TsParamInfo { name, ty, optional })
        })
        .collect()
}

/// TsEntityName からドット区切りの型名を抽出する。
pub(super) fn extract_entity_name(entity: &swc_ecma_ast::TsEntityName) -> std::string::String {
    match entity {
        swc_ecma_ast::TsEntityName::Ident(ident) => ident.sym.to_string(),
        swc_ecma_ast::TsEntityName::TsQualifiedName(q) => {
            format!("{}.{}", extract_entity_name(&q.left), q.right.sym)
        }
    }
}
