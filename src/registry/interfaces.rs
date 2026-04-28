//! interface のフィールド・メソッドシグネチャ収集。

use std::collections::HashMap;

use anyhow::Result;
use swc_ecma_ast as ast;

use super::{FieldDef, MethodSignature, ParamDef};
use crate::registry::MethodKind;
use crate::ts_type_info::{convert_to_ts_type_info, TsTypeInfo};

/// interface のフィールド名・型を収集する。
pub(super) fn collect_interface_fields(
    iface: &ast::TsInterfaceDecl,
) -> Result<Vec<FieldDef<TsTypeInfo>>> {
    let mut fields = Vec::new();
    for member in &iface.body.body {
        if let ast::TsTypeElement::TsPropertySignature(prop) = member {
            if let Some(field) = collect_property_signature(prop) {
                fields.push(field);
            }
        }
    }
    Ok(fields)
}

/// interface から収集されたシグネチャ情報。
pub(super) struct InterfaceSignatures {
    /// メソッドシグネチャ（メソッド名 → オーバーロード）
    pub methods: HashMap<String, Vec<MethodSignature<TsTypeInfo>>>,
    /// Call signatures（`(x: T): U` 形式）
    pub call_signatures: Vec<MethodSignature<TsTypeInfo>>,
    /// Construct signatures（`new (x: T): U` 形式）
    pub constructor: Option<Vec<MethodSignature<TsTypeInfo>>>,
}

/// interface body のメンバーが call signature のみかどうかを判定する。
///
/// call signature が 1 つ以上あり、メソッド・プロパティ・インデックスシグネチャ等が
/// 一切ない場合に `true` を返す。
pub(crate) fn is_callable_only(members: &[ast::TsTypeElement]) -> bool {
    let mut has_call = false;
    for member in members {
        match member {
            ast::TsTypeElement::TsCallSignatureDecl(_) => has_call = true,
            _ => return false,
        }
    }
    has_call
}

/// `TsFnParam` のリストと return type annotation、generic 型パラメータ宣言から
/// `MethodSignature<TsTypeInfo>` を生成する。
///
/// `TsMethodSignature`, `TsCallSignatureDecl`, `TsConstructSignatureDecl` の
/// 3 種のシグネチャ型で共通する params + return_type + has_rest + type_params の収集ロジック。
///
/// I-383 T8': `type_params` 引数を追加。call signature や method signature が固有の
/// generic (例: `interface I { <T>(x: T): T }`) を持つ場合、その情報を MethodSignature
/// に保持し、後続の `resolve_method_sig` が scope に push する。
fn build_method_signature(
    ts_params: &[ast::TsFnParam],
    type_ann: Option<&ast::TsTypeAnn>,
    type_params_decl: Option<&ast::TsTypeParamDecl>,
) -> MethodSignature<TsTypeInfo> {
    let params: Vec<ParamDef<TsTypeInfo>> = ts_params
        .iter()
        .filter_map(super::functions::extract_ts_fn_param)
        .collect();
    let return_type = type_ann.and_then(|ann| convert_to_ts_type_info(&ann.type_ann).ok());
    let has_rest = ts_params
        .iter()
        .any(|p| matches!(p, ast::TsFnParam::Rest(_)));
    let type_params = super::collection::collect_type_params(type_params_decl);
    MethodSignature {
        params,
        return_type,
        has_rest,
        type_params,
        kind: MethodKind::Method,
    }
}

/// interface のメソッド・call signature・construct signature を収集する。
///
/// 返り値:
/// - `methods`: メソッド名 → オーバーロードシグネチャ
/// - `call_signatures`: call signature（`(x: T): U` 形式）
/// - `constructor`: construct signature（`new (x: T): U` 形式）
pub(super) fn collect_interface_signatures(iface: &ast::TsInterfaceDecl) -> InterfaceSignatures {
    let mut methods: HashMap<String, Vec<MethodSignature<TsTypeInfo>>> = HashMap::new();
    let mut call_signatures: Vec<MethodSignature<TsTypeInfo>> = Vec::new();
    let mut construct_signatures: Vec<MethodSignature<TsTypeInfo>> = Vec::new();

    for member in &iface.body.body {
        match member {
            ast::TsTypeElement::TsMethodSignature(method) => {
                let name = match method.key.as_ref() {
                    ast::Expr::Ident(ident) => ident.sym.to_string(),
                    _ => continue,
                };
                let sig = build_method_signature(
                    &method.params,
                    method.type_ann.as_deref(),
                    method.type_params.as_deref(),
                );
                methods.entry(name).or_default().push(sig);
            }
            ast::TsTypeElement::TsCallSignatureDecl(decl) => {
                call_signatures.push(build_method_signature(
                    &decl.params,
                    decl.type_ann.as_deref(),
                    decl.type_params.as_deref(),
                ));
            }
            ast::TsTypeElement::TsConstructSignatureDecl(decl) => {
                construct_signatures.push(build_method_signature(
                    &decl.params,
                    decl.type_ann.as_deref(),
                    decl.type_params.as_deref(),
                ));
            }
            _ => {}
        }
    }

    let constructor = if construct_signatures.is_empty() {
        None
    } else {
        Some(construct_signatures)
    };
    InterfaceSignatures {
        methods,
        call_signatures,
        constructor,
    }
}

/// TsPropertySignature からフィールド名と型を取得する。
///
/// optional フラグは `FieldDef.optional` に保持し、`Option<T>` ラップは行わない。
/// Option ラップは resolve フェーズ（`resolve_field_def`）で適用される。
pub(super) fn collect_property_signature(
    prop: &ast::TsPropertySignature,
) -> Option<FieldDef<TsTypeInfo>> {
    let name = match prop.key.as_ref() {
        ast::Expr::Ident(ident) => ident.sym.to_string(),
        _ => return None,
    };
    let ty = prop
        .type_ann
        .as_ref()
        .and_then(|ann| convert_to_ts_type_info(&ann.type_ann).ok())?;

    Some(FieldDef {
        name,
        ty,
        optional: prop.optional,
    })
}
