//! `const` value registration: when a `VarDeclarator` passes the
//! [`super::placeholder::is_registrable_const_decl`] predicate, these
//! helpers extract the value shape into a
//! [`TypeDef<TsTypeInfo>::ConstValue`](crate::registry::TypeDef) entry.
//!
//! Two source-shape paths:
//!
//! - [`collect_const_value_def`] dispatches to
//!   [`collect_const_value_from_type_annotation`] (explicit
//!   `const X: Type = init`) or
//!   [`collect_const_value_from_as_const`] (`const X = expr as const`)
//! - [`extract_const_array_elements`] /
//!   [`extract_const_object_fields`] walk the initializer to build the
//!   element / field list

use swc_ecma_ast as ast;

use crate::registry::{ConstElement, ConstField, TypeDef};
use crate::ts_type_info::{convert_to_ts_type_info, TsTypeInfo};

/// `as const` 宣言または型注釈付き const 宣言から `TypeDef::ConstValue<TsTypeInfo>` を構築する。
///
/// 対象パターン:
/// - `const X = ['a', 'b'] as const` → 文字列リテラル配列
/// - `const X = { key: 'value' } as const` → オブジェクトリテラル
pub(super) fn collect_const_value_def(d: &ast::VarDeclarator) -> Option<TypeDef<TsTypeInfo>> {
    // Case 1: Type annotation — resolve fields from the annotated type
    if let ast::Pat::Ident(ident) = &d.name {
        if let Some(type_ann) = &ident.type_ann {
            return collect_const_value_from_type_annotation(&type_ann.type_ann);
        }
    }

    // Case 2: `as const` assertion
    let init = d.init.as_ref()?;
    if let ast::Expr::TsConstAssertion(assertion) = init.as_ref() {
        return collect_const_value_from_as_const(&assertion.expr);
    }

    None
}

/// 型注釈から `ConstValue<TsTypeInfo>` を構築する。
///
/// 型注釈がオブジェクト型リテラルの場合、フィールドを直接変換する。
/// 型参照（`const x: MyType = ...`）の場合は、参照名を保持した ConstValue を生成し、
/// 後続の型解決（TsTypeQuery ハンドラ）で参照先から間接的にフィールドを取得する。
fn collect_const_value_from_type_annotation(type_ann: &ast::TsType) -> Option<TypeDef<TsTypeInfo>> {
    // Inline object type → convert fields directly
    if let ast::TsType::TsTypeLit(lit) = type_ann {
        let mut fields = Vec::new();
        for member in &lit.members {
            if let ast::TsTypeElement::TsPropertySignature(prop) = member {
                let field_name = match &*prop.key {
                    ast::Expr::Ident(ident) => ident.sym.to_string(),
                    _ => continue,
                };
                let field_type = prop
                    .type_ann
                    .as_ref()
                    .and_then(|ann| convert_to_ts_type_info(&ann.type_ann).ok())
                    .unwrap_or(TsTypeInfo::Any);
                fields.push(ConstField {
                    name: field_name,
                    ty: field_type,
                    string_literal_value: None,
                });
            }
        }
        if !fields.is_empty() {
            return Some(TypeDef::ConstValue {
                fields,
                elements: vec![],
                type_ref_name: None,
            });
        }
    }

    // Type reference → store the referenced type name for redirect at typeof resolution time
    if let ast::TsType::TsTypeRef(type_ref) = type_ann {
        let ref_name = match &type_ref.type_name {
            swc_ecma_ast::TsEntityName::Ident(ident) => Some(ident.sym.to_string()),
            _ => None,
        };
        return Some(TypeDef::ConstValue {
            fields: vec![],
            elements: vec![],
            type_ref_name: ref_name,
        });
    }

    None
}

/// `as const` アサーション内の式から `ConstValue<TsTypeInfo>` を構築する。
fn collect_const_value_from_as_const(expr: &ast::Expr) -> Option<TypeDef<TsTypeInfo>> {
    match expr {
        ast::Expr::Array(array_lit) => {
            let elements = extract_const_array_elements(array_lit);
            if elements.is_empty() {
                return None;
            }
            Some(TypeDef::ConstValue {
                fields: vec![],
                elements,
                type_ref_name: None,
            })
        }
        ast::Expr::Object(obj_lit) => {
            let fields = extract_const_object_fields(obj_lit);
            if fields.is_empty() {
                return None;
            }
            Some(TypeDef::ConstValue {
                fields,
                elements: vec![],
                type_ref_name: None,
            })
        }
        _ => None,
    }
}

/// 配列リテラルからリテラル要素を抽出する（TsTypeInfo 版）。
///
/// 各要素のリテラル型と、文字列リテラルの場合はその値を保持する。
/// すべての要素がリテラルの場合のみ返す。
fn extract_const_array_elements(array_lit: &ast::ArrayLit) -> Vec<ConstElement<TsTypeInfo>> {
    let mut elements = Vec::new();
    for elem in &array_lit.elems {
        match elem {
            Some(ast::ExprOrSpread { expr, .. }) => match expr.as_ref() {
                ast::Expr::Lit(ast::Lit::Str(s)) => {
                    elements.push(ConstElement {
                        ty: TsTypeInfo::String,
                        string_literal_value: Some(s.value.to_string_lossy().into_owned()),
                    });
                }
                ast::Expr::Lit(ast::Lit::Num(_)) => {
                    elements.push(ConstElement {
                        ty: TsTypeInfo::Number,
                        string_literal_value: None,
                    });
                }
                ast::Expr::Lit(ast::Lit::Bool(_)) => {
                    elements.push(ConstElement {
                        ty: TsTypeInfo::Boolean,
                        string_literal_value: None,
                    });
                }
                _ => return vec![],
            },
            None => return vec![],
        }
    }
    elements
}

/// オブジェクトリテラルからフィールド情報を抽出する（TsTypeInfo 版）。
fn extract_const_object_fields(obj_lit: &ast::ObjectLit) -> Vec<ConstField<TsTypeInfo>> {
    let mut fields = Vec::new();
    for prop in &obj_lit.props {
        if let ast::PropOrSpread::Prop(prop) = prop {
            if let ast::Prop::KeyValue(kv) = prop.as_ref() {
                let field_name = match &kv.key {
                    ast::PropName::Ident(id) => id.sym.to_string(),
                    ast::PropName::Str(s) => s.value.to_string_lossy().into_owned(),
                    _ => continue,
                };
                let (field_type, string_value) = match kv.value.as_ref() {
                    ast::Expr::Lit(ast::Lit::Str(s)) => (
                        TsTypeInfo::String,
                        Some(s.value.to_string_lossy().into_owned()),
                    ),
                    ast::Expr::Lit(ast::Lit::Num(_)) => (TsTypeInfo::Number, None),
                    ast::Expr::Lit(ast::Lit::Bool(_)) => (TsTypeInfo::Boolean, None),
                    _ => (TsTypeInfo::Any, None),
                };
                fields.push(ConstField {
                    name: field_name,
                    ty: field_type,
                    string_literal_value: string_value,
                });
            }
        }
    }
    fields
}
