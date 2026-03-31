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

/// interface から収集されたシグネチャ情報。
pub(super) struct InterfaceSignatures {
    /// メソッドシグネチャ（メソッド名 → オーバーロード）
    pub methods: HashMap<String, Vec<MethodSignature>>,
    /// Call signatures（`(x: T): U` 形式）
    pub call_signatures: Vec<MethodSignature>,
    /// Construct signatures（`new (x: T): U` 形式）
    pub constructor: Option<Vec<MethodSignature>>,
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

/// `TsFnParam` のリストと return type annotation から `MethodSignature` を生成する。
///
/// `TsMethodSignature`, `TsCallSignatureDecl`, `TsConstructSignatureDecl` の
/// 3 種のシグネチャ型で共通する params + return_type + has_rest の収集ロジック。
fn build_method_signature(
    ts_params: &[ast::TsFnParam],
    type_ann: Option<&ast::TsTypeAnn>,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> MethodSignature {
    let params: Vec<(String, RustType)> = ts_params
        .iter()
        .filter_map(|param| super::functions::extract_ts_fn_param(param, lookup, synthetic))
        .collect();
    let return_type =
        type_ann.and_then(|ann| convert_ts_type(&ann.type_ann, synthetic, lookup).ok());
    let has_rest = ts_params
        .iter()
        .any(|p| matches!(p, ast::TsFnParam::Rest(_)));
    MethodSignature {
        params,
        return_type,
        has_rest,
    }
}

/// interface のメソッド・call signature・construct signature を収集する。
///
/// 返り値:
/// - `methods`: メソッド名 → オーバーロードシグネチャ
/// - `call_signatures`: call signature（`(x: T): U` 形式）
/// - `constructor`: construct signature（`new (x: T): U` 形式）
pub(super) fn collect_interface_signatures(
    iface: &ast::TsInterfaceDecl,
    lookup: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> InterfaceSignatures {
    let mut methods: HashMap<String, Vec<MethodSignature>> = HashMap::new();
    let mut call_signatures: Vec<MethodSignature> = Vec::new();
    let mut construct_signatures: Vec<MethodSignature> = Vec::new();

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
                    lookup,
                    synthetic,
                );
                methods.entry(name).or_default().push(sig);
            }
            ast::TsTypeElement::TsCallSignatureDecl(decl) => {
                call_signatures.push(build_method_signature(
                    &decl.params,
                    decl.type_ann.as_deref(),
                    lookup,
                    synthetic,
                ));
            }
            ast::TsTypeElement::TsConstructSignatureDecl(decl) => {
                construct_signatures.push(build_method_signature(
                    &decl.params,
                    decl.type_ann.as_deref(),
                    lookup,
                    synthetic,
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
