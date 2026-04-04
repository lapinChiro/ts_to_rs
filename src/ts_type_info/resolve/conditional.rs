//! TsTypeInfo::Conditional → RustType 解決。
//!
//! TypeScript の条件型を Rust 型に変換する。
//! - `T extends Foo<infer U> ? U : never` → `<T as Foo>::Output`
//! - `T extends X ? true : false` → `bool`
//! - その他 → true ブランチ型にフォールバック

use crate::ir::RustType;
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::TypeRegistry;
use crate::ts_type_info::{TsLiteralKind, TsTypeInfo};

use super::resolve_ts_type;

/// 条件型を解決する。
pub(crate) fn resolve_conditional(
    check: &TsTypeInfo,
    extends: &TsTypeInfo,
    true_type: &TsTypeInfo,
    false_type: &TsTypeInfo,
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<RustType> {
    // パターン 1: infer パターン `T extends Container<infer U> ? U : never`
    if let Some(rust_type) = try_resolve_infer_pattern(check, extends, true_type, false_type)? {
        return Ok(rust_type);
    }

    // パターン 2: 型述語 `T extends X ? true : false` → bool
    if is_true_false_literal(true_type, false_type) {
        return Ok(RustType::Bool);
    }

    // パターン 3: フォールバック → true ブランチ
    resolve_ts_type(true_type, reg, synthetic)
        .or_else(|_| resolve_ts_type(false_type, reg, synthetic))
}

/// `T extends Container<infer U> ? U : never` パターンを検出・変換する。
///
/// 成功時は `<T as Container>::Output` 形式の associated type を返す。
fn try_resolve_infer_pattern(
    check: &TsTypeInfo,
    extends: &TsTypeInfo,
    _true_type: &TsTypeInfo,
    false_type: &TsTypeInfo,
) -> anyhow::Result<Option<RustType>> {
    // false branch が never でなければ infer パターンではない
    if !matches!(false_type, TsTypeInfo::Never) {
        return Ok(None);
    }

    // extends が TypeRef で infer を含むか（TsTypeInfo レベルでは infer は表現できないため、
    // 現時点ではこのパターンは convert_to_ts_type_info 段階で情報が失われる）
    // TODO: TsTypeInfo に Infer variant を追加して完全対応
    //
    // 暫定: check が TypeRef かつ extends が TypeRef の場合、
    // associated type パターンとして解決を試みる
    let check_name = match check {
        TsTypeInfo::TypeRef { name, .. } => name,
        _ => return Ok(None),
    };

    let container_name = match extends {
        TsTypeInfo::TypeRef { name, .. } => name,
        _ => return Ok(None),
    };

    // `<CheckType as Container>::Output` 形式を生成
    Ok(Some(RustType::Named {
        name: format!("<{check_name} as {container_name}>::Output"),
        type_args: vec![],
    }))
}

/// true/false リテラルペアか判定する（型述語パターン）。
fn is_true_false_literal(true_type: &TsTypeInfo, false_type: &TsTypeInfo) -> bool {
    matches!(
        (true_type, false_type),
        (
            TsTypeInfo::Literal(TsLiteralKind::Boolean(true)),
            TsTypeInfo::Literal(TsLiteralKind::Boolean(false))
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn true_false_literal_becomes_bool() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let result = resolve_conditional(
            &TsTypeInfo::TypeRef {
                name: "T".to_string(),
                type_args: vec![],
            },
            &TsTypeInfo::String,
            &TsTypeInfo::Literal(TsLiteralKind::Boolean(true)),
            &TsTypeInfo::Literal(TsLiteralKind::Boolean(false)),
            &reg,
            &mut syn,
        )
        .unwrap();
        assert_eq!(result, RustType::Bool);
    }

    #[test]
    fn fallback_to_true_branch() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let result = resolve_conditional(
            &TsTypeInfo::TypeRef {
                name: "T".to_string(),
                type_args: vec![],
            },
            &TsTypeInfo::String,
            &TsTypeInfo::Number,
            &TsTypeInfo::Boolean,
            &reg,
            &mut syn,
        )
        .unwrap();
        assert_eq!(result, RustType::F64);
    }

    #[test]
    fn infer_pattern_false_type_not_never_returns_none() {
        // false_type が Never でなければ infer パターンとして認識しない
        let result = try_resolve_infer_pattern(
            &TsTypeInfo::TypeRef {
                name: "T".to_string(),
                type_args: vec![],
            },
            &TsTypeInfo::TypeRef {
                name: "Container".to_string(),
                type_args: vec![],
            },
            &TsTypeInfo::String,
            &TsTypeInfo::Boolean, // Never ではない
        )
        .unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn infer_pattern_check_not_type_ref_returns_none() {
        // check_type が TypeRef でなければ infer パターンとして認識しない
        let result = try_resolve_infer_pattern(
            &TsTypeInfo::String, // TypeRef ではない
            &TsTypeInfo::TypeRef {
                name: "Container".to_string(),
                type_args: vec![],
            },
            &TsTypeInfo::String,
            &TsTypeInfo::Never,
        )
        .unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn infer_pattern_extends_not_type_ref_returns_none() {
        // extends_type が TypeRef でなければ infer パターンとして認識しない
        let result = try_resolve_infer_pattern(
            &TsTypeInfo::TypeRef {
                name: "T".to_string(),
                type_args: vec![],
            },
            &TsTypeInfo::String, // TypeRef ではない
            &TsTypeInfo::String,
            &TsTypeInfo::Never,
        )
        .unwrap();
        assert_eq!(result, None);
    }
}
