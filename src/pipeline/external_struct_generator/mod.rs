//! 参照されるビルトイン外部型の struct 定義を自動生成する。
//!
//! 変換出力（IR）内で参照されているが定義が存在しない外部型を検出し、
//! `TypeRegistry` のフィールド情報から `Item::Struct` を生成する。

use std::collections::HashSet;

use crate::ir::{camel_to_snake, sanitize_field_name, Item, RustType, StructField, Visibility};
use crate::registry::{TypeDef, TypeRegistry};

/// Rust の標準ライブラリ型・serde 型など、struct 生成が不要な型名のセット。
const RUST_BUILTIN_TYPES: &[&str] = &[
    "String", "Vec", "HashMap", "HashSet", "Option", "Box", "Result", "Rc", "Arc", "Mutex", "bool",
    "f64", "i64", "i128", "u8", "u32", "usize",
];

/// `serde_json::Value` のフルパス。
const SERDE_JSON_VALUE: &str = "serde_json::Value";

/// IR items を走査し、参照されているが定義がない外部型名を収集する。
///
/// 外部型（JSON ビルトイン定義）のみを対象とし、ユーザー定義型（TS ソースから登録された型）は除外する。
/// `TypeRegistry::is_external` で外部型かどうかを判定する。
///
/// 以下を除外する:
/// - `items` 内に既に定義が存在する型（struct/enum/trait/type alias）
/// - Rust 標準ライブラリ型（`String`, `Vec`, `HashMap` 等）
/// - `serde_json::Value`
/// - 外部型でない型（ユーザー定義型）
pub fn collect_undefined_type_references(
    items: &[Item],
    registry: &TypeRegistry,
) -> HashSet<String> {
    // 1. items 内で定義されている型名を収集
    let defined_types: HashSet<String> = items
        .iter()
        .filter_map(|item| match item {
            Item::Struct { name, .. }
            | Item::Enum { name, .. }
            | Item::Trait { name, .. }
            | Item::TypeAlias { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect();

    // 2. items 内で参照されている型名を収集
    let mut referenced_types = HashSet::new();
    for item in items {
        collect_type_refs_from_item(item, &mut referenced_types);
    }

    // 3. フィルタリング: 定義済み・標準型・serde_json::Value・外部型以外を除外
    let builtin_set: HashSet<&str> = RUST_BUILTIN_TYPES.iter().copied().collect();

    referenced_types
        .into_iter()
        .filter(|name| !defined_types.contains(name))
        .filter(|name| !builtin_set.contains(name.as_str()))
        .filter(|name| name != SERDE_JSON_VALUE)
        .filter(|name| registry.is_external(name))
        .collect()
}

/// IR items を走査し、参照されているが定義がない型名を **全て** 収集する。
///
/// [`collect_undefined_type_references`] と異なり、`is_external` フィルタを適用しない。
/// types.rs のスタブ生成で使用する — types.rs 内の全未定義参照を解決するため。
pub fn collect_all_undefined_references(items: &[Item]) -> HashSet<String> {
    // 定義済み型名（struct, enum, trait, type alias）
    let defined_types: HashSet<String> = items
        .iter()
        .filter_map(|item| match item {
            Item::Struct { name, .. }
            | Item::Enum { name, .. }
            | Item::Trait { name, .. }
            | Item::TypeAlias { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect();

    // インポート済み型名（`use path::{Name};` の names）
    let imported_types: HashSet<String> = items
        .iter()
        .filter_map(|item| match item {
            Item::Use { names, .. } => Some(names.clone()),
            _ => None,
        })
        .flatten()
        .collect();

    // 型パラメータ名（struct/trait/fn/impl の type_params）
    let type_param_names: HashSet<String> = items
        .iter()
        .flat_map(|item| match item {
            Item::Struct { type_params, .. }
            | Item::Trait { type_params, .. }
            | Item::Fn { type_params, .. }
            | Item::Impl { type_params, .. }
            | Item::TypeAlias { type_params, .. } => type_params
                .iter()
                .map(|tp| tp.name.clone())
                .collect::<Vec<_>>(),
            _ => vec![],
        })
        .collect();

    let mut referenced_types = HashSet::new();
    for item in items {
        collect_type_refs_from_item(item, &mut referenced_types);
    }

    let builtin_set: HashSet<&str> = RUST_BUILTIN_TYPES.iter().copied().collect();

    referenced_types
        .into_iter()
        .filter(|name| !defined_types.contains(name))
        .filter(|name| !imported_types.contains(name))
        .filter(|name| !type_param_names.contains(name))
        .filter(|name| !builtin_set.contains(name.as_str()))
        .filter(|name| name != SERDE_JSON_VALUE)
        // パス形式の型名（例: E::Bindings, serde_json::Value）は struct 名にならない
        .filter(|name| !name.contains("::"))
        .collect()
}

/// 未定義型に対する空スタブ struct を生成し、items に追加する。
///
/// types.rs のコンパイルを通すため、参照されているが定義がない型にスタブを追加する。
/// TypeRegistry に struct 情報がある型はフル生成（[`generate_external_struct`] 経由）、
/// それ以外は空のユニット struct `pub struct TypeName;` を生成する。
/// フル生成した struct が新たな未定義参照を生む場合に備え、固定点に達するまで反復する。
pub fn generate_stub_structs(items: &mut Vec<Item>, registry: &TypeRegistry) {
    for _ in 0..10 {
        let undefined = collect_all_undefined_references(items);
        if undefined.is_empty() {
            break;
        }
        // 出力順序を決定的にするためソート
        let mut sorted: Vec<String> = undefined.into_iter().collect();
        sorted.sort();
        for name in sorted {
            if let Some(full) = generate_external_struct(&name, registry) {
                items.push(full);
            } else {
                items.push(Item::Struct {
                    vis: Visibility::Public,
                    name,
                    type_params: vec![],
                    fields: vec![],
                });
            }
        }
    }
}

/// `TypeRegistry` のフィールド情報から外部型の `Item::Struct` を生成する。
///
/// `TypeDef::Struct` 以外（`TypeDef::Enum`, `TypeDef::Function`）の場合は `None` を返す。
pub fn generate_external_struct(name: &str, registry: &TypeRegistry) -> Option<Item> {
    let typedef = registry.get(name)?;
    match typedef {
        TypeDef::Struct {
            type_params,
            fields,
            ..
        } => {
            let struct_fields: Vec<StructField> = fields
                .iter()
                .map(|field| {
                    // 自己参照フィールドを Box でラップ（再帰型の infinite size 防止）
                    let ty = if references_type_name(&field.ty, name) {
                        RustType::Named {
                            name: "Box".to_string(),
                            type_args: vec![field.ty.clone()],
                        }
                    } else {
                        field.ty.clone()
                    };
                    StructField {
                        vis: Some(Visibility::Public),
                        name: sanitize_field_name(&camel_to_snake(&field.name)),
                        ty,
                    }
                })
                .collect();

            Some(Item::Struct {
                vis: Visibility::Public,
                name: name.to_string(),
                type_params: type_params.clone(),
                fields: struct_fields,
            })
        }
        TypeDef::Enum { .. } | TypeDef::Function { .. } | TypeDef::ConstValue { .. } => None,
    }
}

/// `RustType` が指定された型名を直接参照しているか判定する。
fn references_type_name(ty: &RustType, target: &str) -> bool {
    match ty {
        RustType::Named { name, type_args } => {
            name == target || type_args.iter().any(|a| references_type_name(a, target))
        }
        RustType::Option(inner) | RustType::Vec(inner) | RustType::Ref(inner) => {
            references_type_name(inner, target)
        }
        RustType::Result { ok, err } => {
            references_type_name(ok, target) || references_type_name(err, target)
        }
        RustType::Tuple(elems) => elems.iter().any(|e| references_type_name(e, target)),
        _ => false,
    }
}

/// `Item` 内で参照されている `RustType::Named` の型名を再帰的に収集する。
fn collect_type_refs_from_item(item: &Item, refs: &mut HashSet<String>) {
    match item {
        Item::Enum { variants, .. } => {
            for variant in variants {
                if let Some(data) = &variant.data {
                    collect_type_refs_from_rust_type(data, refs);
                }
                for field in &variant.fields {
                    collect_type_refs_from_rust_type(&field.ty, refs);
                }
            }
        }
        Item::Struct { fields, .. } => {
            for field in fields {
                collect_type_refs_from_rust_type(&field.ty, refs);
            }
        }
        Item::Fn {
            return_type,
            params,
            ..
        } => {
            if let Some(rt) = return_type {
                collect_type_refs_from_rust_type(rt, refs);
            }
            for param in params {
                if let Some(ty) = &param.ty {
                    collect_type_refs_from_rust_type(ty, refs);
                }
            }
        }
        _ => {}
    }
}

/// `RustType` を再帰的に走査し、`Named` の型名を収集する。
fn collect_type_refs_from_rust_type(ty: &RustType, refs: &mut HashSet<String>) {
    match ty {
        RustType::Named { name, type_args } => {
            refs.insert(name.clone());
            for arg in type_args {
                collect_type_refs_from_rust_type(arg, refs);
            }
        }
        RustType::Option(inner) | RustType::Vec(inner) | RustType::Ref(inner) => {
            collect_type_refs_from_rust_type(inner, refs);
        }
        RustType::Result { ok, err } => {
            collect_type_refs_from_rust_type(ok, refs);
            collect_type_refs_from_rust_type(err, refs);
        }
        RustType::Tuple(elems) => {
            for elem in elems {
                collect_type_refs_from_rust_type(elem, refs);
            }
        }
        RustType::Fn {
            params,
            return_type,
        } => {
            for param in params {
                collect_type_refs_from_rust_type(param, refs);
            }
            collect_type_refs_from_rust_type(return_type, refs);
        }
        RustType::DynTrait(_)
        | RustType::Unit
        | RustType::String
        | RustType::F64
        | RustType::Bool
        | RustType::Any
        | RustType::Never => {}
    }
}

#[cfg(test)]
mod tests;
