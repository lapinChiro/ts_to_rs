//! TsTypeInfo::Union → RustType 解決。
//!
//! TypeScript の union 型を Rust 型に変換する。
//! nullable union → Option<T>、string literal union → String、
//! 複数型 union → synthetic enum 登録。

use crate::ir::{EnumVariant, Item, RustType, Visibility};
use crate::pipeline::synthetic_registry::{variant_name_for_type, SyntheticTypeKind};
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::TypeRegistry;
use crate::ts_type_info::{TsLiteralKind, TsTypeInfo};

use super::resolve_ts_type;

/// Union 型を解決する。
///
/// nullable メンバー（null, undefined, void）を除去し、
/// 残りが単一なら Option<T>、string literal union なら String enum、
/// 複数型なら synthetic union enum を登録する。
pub(crate) fn resolve_union(
    members: &[TsTypeInfo],
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<RustType> {
    // null / undefined / void を分離
    let has_nullable = members.iter().any(is_nullable);
    let non_nullable: Vec<&TsTypeInfo> = members.iter().filter(|m| !is_nullable(m)).collect();

    // never を除去
    let non_nullable: Vec<&TsTypeInfo> = non_nullable
        .into_iter()
        .filter(|m| !matches!(m, TsTypeInfo::Never))
        .collect();

    let inner = match non_nullable.len() {
        0 => RustType::Unit,
        1 => resolve_ts_type(non_nullable[0], reg, synthetic)?,
        _ => resolve_multi_member_union(&non_nullable, reg, synthetic)?,
    };

    if has_nullable {
        Ok(RustType::Option(Box::new(inner)))
    } else {
        Ok(inner)
    }
}

/// 複数メンバーの union を解決する。
fn resolve_multi_member_union(
    members: &[&TsTypeInfo],
    reg: &TypeRegistry,
    synthetic: &mut SyntheticTypeRegistry,
) -> anyhow::Result<RustType> {
    // 全メンバーが string literal → String（TypeDef レベルで enum 化される）
    let all_string_lit = members
        .iter()
        .all(|m| matches!(m, TsTypeInfo::Literal(TsLiteralKind::String(_))));
    if all_string_lit {
        return Ok(RustType::String);
    }

    // 各メンバーを解決して synthetic union enum を登録
    let mut resolved = Vec::new();
    let mut name_parts = Vec::new();

    for member in members {
        let ty = resolve_ts_type(member, reg, synthetic)?;
        let ty = unwrap_promise_result(ty);

        let variant_name = variant_name_for_type(&ty);
        // 重複バリアント名をスキップ
        if name_parts.contains(&variant_name) {
            continue;
        }
        name_parts.push(variant_name);
        resolved.push(ty);
    }

    if resolved.len() == 1 {
        return Ok(resolved.into_iter().next().expect("len == 1"));
    }

    // AST 順でバリアント名を結合した enum 名を生成
    let enum_name = name_parts.join("Or");

    // バリアントを構築
    let variants = resolved
        .iter()
        .map(|ty| EnumVariant {
            name: variant_name_for_type(ty),
            value: None,
            data: Some(ty.clone()),
            fields: vec![],
        })
        .collect();

    let item = Item::Enum {
        vis: Visibility::Public,
        name: enum_name.clone(),
        serde_tag: None,
        variants,
    };

    synthetic.push_item(enum_name.clone(), SyntheticTypeKind::UnionEnum, item);

    Ok(RustType::Named {
        name: enum_name,
        type_args: vec![],
    })
}

/// Promise/Result の内部型をアンラップする。
///
/// `Named("Promise", [T])` → `T`。union のバリアントでは async の外皮を除去する。
fn unwrap_promise_result(ty: RustType) -> RustType {
    match &ty {
        RustType::Named { name, type_args } if name == "Promise" && type_args.len() == 1 => {
            type_args[0].clone()
        }
        _ => ty,
    }
}

/// nullable 型（null, undefined, void）か判定する。
fn is_nullable(info: &TsTypeInfo) -> bool {
    matches!(
        info,
        TsTypeInfo::Null | TsTypeInfo::Undefined | TsTypeInfo::Void
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nullable_union_option() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let members = vec![TsTypeInfo::String, TsTypeInfo::Null];
        assert_eq!(
            resolve_union(&members, &reg, &mut syn).unwrap(),
            RustType::Option(Box::new(RustType::String))
        );
    }

    #[test]
    fn nullable_undefined_union_option() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let members = vec![TsTypeInfo::Number, TsTypeInfo::Undefined];
        assert_eq!(
            resolve_union(&members, &reg, &mut syn).unwrap(),
            RustType::Option(Box::new(RustType::F64))
        );
    }

    #[test]
    fn all_nullable_unit() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let members = vec![TsTypeInfo::Null, TsTypeInfo::Undefined];
        assert_eq!(
            resolve_union(&members, &reg, &mut syn).unwrap(),
            RustType::Option(Box::new(RustType::Unit))
        );
    }

    #[test]
    fn string_literal_union() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let members = vec![
            TsTypeInfo::Literal(TsLiteralKind::String("a".to_string())),
            TsTypeInfo::Literal(TsLiteralKind::String("b".to_string())),
        ];
        assert_eq!(
            resolve_union(&members, &reg, &mut syn).unwrap(),
            RustType::String
        );
    }

    #[test]
    fn multi_type_union_registers_synthetic() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let members = vec![TsTypeInfo::String, TsTypeInfo::Number];
        let result = resolve_union(&members, &reg, &mut syn).unwrap();
        match result {
            RustType::Named { name, .. } => {
                assert!(name.contains("String"));
                assert!(name.contains("F64"));
            }
            _ => panic!("expected Named type for multi-type union"),
        }
    }

    #[test]
    fn never_filtered_from_union() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        let members = vec![TsTypeInfo::String, TsTypeInfo::Never];
        assert_eq!(
            resolve_union(&members, &reg, &mut syn).unwrap(),
            RustType::String
        );
    }

    #[test]
    fn duplicate_variants_deduplicated() {
        let reg = TypeRegistry::new();
        let mut syn = SyntheticTypeRegistry::new();
        // string | string → should deduplicate to just String
        let members = vec![TsTypeInfo::String, TsTypeInfo::String];
        assert_eq!(
            resolve_union(&members, &reg, &mut syn).unwrap(),
            RustType::String
        );
    }
}
