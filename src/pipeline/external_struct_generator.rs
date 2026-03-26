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
                .map(|(field_name, ty)| {
                    // 自己参照フィールドを Box でラップ（再帰型の infinite size 防止）
                    let ty = if references_type_name(ty, name) {
                        RustType::Named {
                            name: "Box".to_string(),
                            type_args: vec![ty.clone()],
                        }
                    } else {
                        ty.clone()
                    };
                    StructField {
                        vis: Some(Visibility::Public),
                        name: sanitize_field_name(&camel_to_snake(field_name)),
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
        TypeDef::Enum { .. } | TypeDef::Function { .. } => None,
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
mod tests {
    use super::*;
    use crate::ir::{EnumVariant, TypeParam};
    use std::collections::HashMap;

    /// テスト用に TypeRegistry に外部型としてフィールド付き struct 型を登録するヘルパー。
    fn register_external_struct(
        registry: &mut TypeRegistry,
        name: &str,
        fields: Vec<(&str, RustType)>,
        type_params: Vec<TypeParam>,
    ) {
        registry.register_external(
            name.to_string(),
            TypeDef::Struct {
                type_params,
                fields: fields
                    .into_iter()
                    .map(|(n, ty)| (n.to_string(), ty))
                    .collect(),
                methods: HashMap::new(),
                extends: vec![],
                is_interface: true,
            },
        );
    }

    // =========================================================================
    // T1: collect_undefined_type_references
    // =========================================================================

    #[test]
    fn test_collect_refs_enum_variant_named_type_detected() {
        let mut registry = TypeRegistry::new();
        register_external_struct(&mut registry, "Date", vec![], vec![]);

        let items = vec![Item::Enum {
            vis: Visibility::Public,
            name: "MyEnum".to_string(),
            serde_tag: None,
            variants: vec![EnumVariant {
                name: "Date".to_string(),
                value: None,
                data: Some(RustType::Named {
                    name: "Date".to_string(),
                    type_args: vec![],
                }),
                fields: vec![],
            }],
        }];

        let refs = collect_undefined_type_references(&items, &registry);
        assert_eq!(refs, HashSet::from(["Date".to_string()]));
    }

    #[test]
    fn test_collect_refs_rust_stdlib_types_excluded() {
        let registry = TypeRegistry::new();

        let items = vec![Item::Enum {
            vis: Visibility::Public,
            name: "MyEnum".to_string(),
            serde_tag: None,
            variants: vec![
                EnumVariant {
                    name: "S".to_string(),
                    value: None,
                    data: Some(RustType::String),
                    fields: vec![],
                },
                EnumVariant {
                    name: "N".to_string(),
                    value: None,
                    data: Some(RustType::F64),
                    fields: vec![],
                },
                EnumVariant {
                    name: "B".to_string(),
                    value: None,
                    data: Some(RustType::Bool),
                    fields: vec![],
                },
            ],
        }];

        let refs = collect_undefined_type_references(&items, &registry);
        assert!(refs.is_empty());
    }

    #[test]
    fn test_collect_refs_serde_json_value_excluded() {
        let registry = TypeRegistry::new();

        let items = vec![Item::Enum {
            vis: Visibility::Public,
            name: "MyEnum".to_string(),
            serde_tag: None,
            variants: vec![EnumVariant {
                name: "Other".to_string(),
                value: None,
                data: Some(RustType::Named {
                    name: "serde_json::Value".to_string(),
                    type_args: vec![],
                }),
                fields: vec![],
            }],
        }];

        let refs = collect_undefined_type_references(&items, &registry);
        assert!(refs.is_empty());
    }

    #[test]
    fn test_collect_refs_defined_struct_excluded() {
        let mut registry = TypeRegistry::new();
        register_external_struct(&mut registry, "Foo", vec![], vec![]);

        let items = vec![
            Item::Struct {
                vis: Visibility::Public,
                name: "Foo".to_string(),
                type_params: vec![],
                fields: vec![],
            },
            Item::Enum {
                vis: Visibility::Public,
                name: "MyEnum".to_string(),
                serde_tag: None,
                variants: vec![EnumVariant {
                    name: "Foo".to_string(),
                    value: None,
                    data: Some(RustType::Named {
                        name: "Foo".to_string(),
                        type_args: vec![],
                    }),
                    fields: vec![],
                }],
            },
        ];

        let refs = collect_undefined_type_references(&items, &registry);
        assert!(refs.is_empty());
    }

    #[test]
    fn test_collect_refs_nested_type_args_detected() {
        let mut registry = TypeRegistry::new();
        register_external_struct(&mut registry, "ArrayBuffer", vec![], vec![]);

        let items = vec![Item::Struct {
            vis: Visibility::Public,
            name: "MyStruct".to_string(),
            type_params: vec![],
            fields: vec![StructField {
                vis: Some(Visibility::Public),
                name: "data".to_string(),
                ty: RustType::Vec(Box::new(RustType::Named {
                    name: "ArrayBuffer".to_string(),
                    type_args: vec![],
                })),
            }],
        }];

        let refs = collect_undefined_type_references(&items, &registry);
        assert_eq!(refs, HashSet::from(["ArrayBuffer".to_string()]));
    }

    #[test]
    fn test_collect_refs_struct_field_named_type_detected() {
        let mut registry = TypeRegistry::new();
        register_external_struct(&mut registry, "Headers", vec![], vec![]);

        let items = vec![Item::Struct {
            vis: Visibility::Public,
            name: "MyStruct".to_string(),
            type_params: vec![],
            fields: vec![StructField {
                vis: Some(Visibility::Public),
                name: "headers".to_string(),
                ty: RustType::Named {
                    name: "Headers".to_string(),
                    type_args: vec![],
                },
            }],
        }];

        let refs = collect_undefined_type_references(&items, &registry);
        assert_eq!(refs, HashSet::from(["Headers".to_string()]));
    }

    #[test]
    fn test_collect_refs_not_in_registry_excluded() {
        let registry = TypeRegistry::new();

        let items = vec![Item::Enum {
            vis: Visibility::Public,
            name: "MyEnum".to_string(),
            serde_tag: None,
            variants: vec![EnumVariant {
                name: "Unknown".to_string(),
                value: None,
                data: Some(RustType::Named {
                    name: "Unknown".to_string(),
                    type_args: vec![],
                }),
                fields: vec![],
            }],
        }];

        let refs = collect_undefined_type_references(&items, &registry);
        assert!(refs.is_empty());
    }

    #[test]
    fn test_collect_refs_user_defined_type_excluded() {
        let mut registry = TypeRegistry::new();
        // register（not register_external）で登録 → ユーザー定義型
        registry.register(
            "Bindings".to_string(),
            TypeDef::Struct {
                type_params: vec![],
                fields: vec![("db_url".to_string(), RustType::String)],
                methods: HashMap::new(),
                extends: vec![],
                is_interface: true,
            },
        );

        let items = vec![Item::Enum {
            vis: Visibility::Public,
            name: "MyEnum".to_string(),
            serde_tag: None,
            variants: vec![EnumVariant {
                name: "Bindings".to_string(),
                value: None,
                data: Some(RustType::Named {
                    name: "Bindings".to_string(),
                    type_args: vec![],
                }),
                fields: vec![],
            }],
        }];

        let refs = collect_undefined_type_references(&items, &registry);
        assert!(refs.is_empty(), "user-defined types should not be included");
    }

    #[test]
    fn test_collect_refs_multiple_types_collected() {
        let mut registry = TypeRegistry::new();
        register_external_struct(&mut registry, "Date", vec![], vec![]);
        register_external_struct(&mut registry, "Error", vec![], vec![]);
        register_external_struct(&mut registry, "RegExp", vec![], vec![]);

        let items = vec![Item::Enum {
            vis: Visibility::Public,
            name: "MyEnum".to_string(),
            serde_tag: None,
            variants: vec![
                EnumVariant {
                    name: "Date".to_string(),
                    value: None,
                    data: Some(RustType::Named {
                        name: "Date".to_string(),
                        type_args: vec![],
                    }),
                    fields: vec![],
                },
                EnumVariant {
                    name: "Error".to_string(),
                    value: None,
                    data: Some(RustType::Named {
                        name: "Error".to_string(),
                        type_args: vec![],
                    }),
                    fields: vec![],
                },
                EnumVariant {
                    name: "RegExp".to_string(),
                    value: None,
                    data: Some(RustType::Named {
                        name: "RegExp".to_string(),
                        type_args: vec![],
                    }),
                    fields: vec![],
                },
            ],
        }];

        let refs = collect_undefined_type_references(&items, &registry);
        assert_eq!(
            refs,
            HashSet::from([
                "Date".to_string(),
                "Error".to_string(),
                "RegExp".to_string()
            ])
        );
    }

    #[test]
    fn test_collect_refs_option_nested_type_detected() {
        let mut registry = TypeRegistry::new();
        register_external_struct(&mut registry, "Blob", vec![], vec![]);

        let items = vec![Item::Struct {
            vis: Visibility::Public,
            name: "MyStruct".to_string(),
            type_params: vec![],
            fields: vec![StructField {
                vis: Some(Visibility::Public),
                name: "data".to_string(),
                ty: RustType::Option(Box::new(RustType::Named {
                    name: "Blob".to_string(),
                    type_args: vec![],
                })),
            }],
        }];

        let refs = collect_undefined_type_references(&items, &registry);
        assert_eq!(refs, HashSet::from(["Blob".to_string()]));
    }

    #[test]
    fn test_collect_refs_fn_item_params_and_return_detected() {
        let mut registry = TypeRegistry::new();
        register_external_struct(&mut registry, "Request", vec![], vec![]);
        register_external_struct(&mut registry, "Response", vec![], vec![]);

        let items = vec![Item::Fn {
            vis: Visibility::Public,
            attributes: vec![],
            is_async: false,
            name: "handle".to_string(),
            type_params: vec![],
            params: vec![crate::ir::Param {
                name: "req".to_string(),
                ty: Some(RustType::Named {
                    name: "Request".to_string(),
                    type_args: vec![],
                }),
            }],
            return_type: Some(RustType::Named {
                name: "Response".to_string(),
                type_args: vec![],
            }),
            body: vec![],
        }];

        let refs = collect_undefined_type_references(&items, &registry);
        assert_eq!(
            refs,
            HashSet::from(["Request".to_string(), "Response".to_string()])
        );
    }

    #[test]
    fn test_collect_refs_defined_trait_excluded() {
        let mut registry = TypeRegistry::new();
        register_external_struct(&mut registry, "MyTrait", vec![], vec![]);

        let items = vec![
            Item::Trait {
                vis: Visibility::Public,
                name: "MyTrait".to_string(),
                type_params: vec![],
                supertraits: vec![],
                methods: vec![],
                associated_types: vec![],
            },
            Item::Struct {
                vis: Visibility::Public,
                name: "MyStruct".to_string(),
                type_params: vec![],
                fields: vec![StructField {
                    vis: Some(Visibility::Public),
                    name: "t".to_string(),
                    ty: RustType::Named {
                        name: "MyTrait".to_string(),
                        type_args: vec![],
                    },
                }],
            },
        ];

        let refs = collect_undefined_type_references(&items, &registry);
        assert!(refs.is_empty());
    }

    #[test]
    fn test_collect_refs_defined_type_alias_excluded() {
        let mut registry = TypeRegistry::new();
        register_external_struct(&mut registry, "MyAlias", vec![], vec![]);

        let items = vec![
            Item::TypeAlias {
                vis: Visibility::Public,
                name: "MyAlias".to_string(),
                type_params: vec![],
                ty: RustType::String,
            },
            Item::Struct {
                vis: Visibility::Public,
                name: "MyStruct".to_string(),
                type_params: vec![],
                fields: vec![StructField {
                    vis: Some(Visibility::Public),
                    name: "a".to_string(),
                    ty: RustType::Named {
                        name: "MyAlias".to_string(),
                        type_args: vec![],
                    },
                }],
            },
        ];

        let refs = collect_undefined_type_references(&items, &registry);
        assert!(refs.is_empty());
    }

    #[test]
    fn test_collect_refs_enum_variant_struct_fields_detected() {
        let mut registry = TypeRegistry::new();
        register_external_struct(&mut registry, "FormData", vec![], vec![]);

        let items = vec![Item::Enum {
            vis: Visibility::Public,
            name: "MyEnum".to_string(),
            serde_tag: None,
            variants: vec![EnumVariant {
                name: "Upload".to_string(),
                value: None,
                data: None,
                fields: vec![StructField {
                    vis: Some(Visibility::Public),
                    name: "form".to_string(),
                    ty: RustType::Named {
                        name: "FormData".to_string(),
                        type_args: vec![],
                    },
                }],
            }],
        }];

        let refs = collect_undefined_type_references(&items, &registry);
        assert_eq!(refs, HashSet::from(["FormData".to_string()]));
    }

    #[test]
    fn test_collect_refs_result_type_both_ok_and_err_detected() {
        let mut registry = TypeRegistry::new();
        register_external_struct(&mut registry, "Response", vec![], vec![]);
        register_external_struct(&mut registry, "HttpError", vec![], vec![]);

        let items = vec![Item::Struct {
            vis: Visibility::Public,
            name: "MyStruct".to_string(),
            type_params: vec![],
            fields: vec![StructField {
                vis: Some(Visibility::Public),
                name: "result".to_string(),
                ty: RustType::Result {
                    ok: Box::new(RustType::Named {
                        name: "Response".to_string(),
                        type_args: vec![],
                    }),
                    err: Box::new(RustType::Named {
                        name: "HttpError".to_string(),
                        type_args: vec![],
                    }),
                },
            }],
        }];

        let refs = collect_undefined_type_references(&items, &registry);
        assert_eq!(
            refs,
            HashSet::from(["Response".to_string(), "HttpError".to_string()])
        );
    }

    #[test]
    fn test_collect_refs_tuple_type_detected() {
        let mut registry = TypeRegistry::new();
        register_external_struct(&mut registry, "Headers", vec![], vec![]);

        let items = vec![Item::Struct {
            vis: Visibility::Public,
            name: "MyStruct".to_string(),
            type_params: vec![],
            fields: vec![StructField {
                vis: Some(Visibility::Public),
                name: "pair".to_string(),
                ty: RustType::Tuple(vec![
                    RustType::String,
                    RustType::Named {
                        name: "Headers".to_string(),
                        type_args: vec![],
                    },
                ]),
            }],
        }];

        let refs = collect_undefined_type_references(&items, &registry);
        assert_eq!(refs, HashSet::from(["Headers".to_string()]));
    }

    // =========================================================================
    // T2: generate_external_struct
    // =========================================================================

    #[test]
    fn test_generate_struct_error_with_fields() {
        let mut registry = TypeRegistry::new();
        register_external_struct(
            &mut registry,
            "Error",
            vec![
                ("name", RustType::String),
                ("message", RustType::String),
                ("stack", RustType::Option(Box::new(RustType::String))),
            ],
            vec![],
        );

        let item = generate_external_struct("Error", &registry).unwrap();
        match item {
            Item::Struct {
                vis,
                name,
                fields,
                type_params,
            } => {
                assert_eq!(vis, Visibility::Public);
                assert_eq!(name, "Error");
                assert!(type_params.is_empty());
                assert_eq!(fields.len(), 3);
                assert_eq!(fields[0].name, "name");
                assert_eq!(fields[0].ty, RustType::String);
                assert_eq!(fields[1].name, "message");
                assert_eq!(fields[1].ty, RustType::String);
                assert_eq!(fields[2].name, "stack");
                assert_eq!(fields[2].ty, RustType::Option(Box::new(RustType::String)));
            }
            _ => panic!("expected Item::Struct"),
        }
    }

    #[test]
    fn test_generate_struct_field_names_snake_case() {
        let mut registry = TypeRegistry::new();
        register_external_struct(
            &mut registry,
            "RegExp",
            vec![("lastIndex", RustType::F64), ("ignoreCase", RustType::Bool)],
            vec![],
        );

        let item = generate_external_struct("RegExp", &registry).unwrap();
        match item {
            Item::Struct { fields, .. } => {
                assert_eq!(fields[0].name, "last_index");
                assert_eq!(fields[1].name, "ignore_case");
            }
            _ => panic!("expected Item::Struct"),
        }
    }

    #[test]
    fn test_generate_struct_typedef_function_returns_none() {
        let mut registry = TypeRegistry::new();
        registry.register(
            "fetch".to_string(),
            TypeDef::Function {
                params: vec![],
                return_type: None,
                has_rest: false,
            },
        );

        assert!(generate_external_struct("fetch", &registry).is_none());
    }

    #[test]
    fn test_generate_struct_generic_type_params_preserved() {
        let mut registry = TypeRegistry::new();
        register_external_struct(
            &mut registry,
            "ReadableStream",
            vec![("locked", RustType::Bool)],
            vec![TypeParam {
                name: "R".to_string(),
                constraint: None,
            }],
        );

        let item = generate_external_struct("ReadableStream", &registry).unwrap();
        match item {
            Item::Struct { type_params, .. } => {
                assert_eq!(type_params.len(), 1);
                assert_eq!(type_params[0].name, "R");
            }
            _ => panic!("expected Item::Struct"),
        }
    }

    #[test]
    fn test_generate_struct_fields_all_public() {
        let mut registry = TypeRegistry::new();
        register_external_struct(
            &mut registry,
            "URL",
            vec![("href", RustType::String), ("hostname", RustType::String)],
            vec![],
        );

        let item = generate_external_struct("URL", &registry).unwrap();
        match item {
            Item::Struct { fields, .. } => {
                for field in &fields {
                    assert_eq!(field.vis, Some(Visibility::Public));
                }
            }
            _ => panic!("expected Item::Struct"),
        }
    }

    #[test]
    fn test_generate_struct_nonexistent_type_returns_none() {
        let registry = TypeRegistry::new();
        assert!(generate_external_struct("NonExistent", &registry).is_none());
    }

    #[test]
    fn test_generate_struct_empty_fields() {
        let mut registry = TypeRegistry::new();
        register_external_struct(&mut registry, "Date", vec![], vec![]);

        let item = generate_external_struct("Date", &registry).unwrap();
        match item {
            Item::Struct { name, fields, .. } => {
                assert_eq!(name, "Date");
                assert!(fields.is_empty());
            }
            _ => panic!("expected Item::Struct"),
        }
    }

    #[test]
    fn test_generate_struct_typedef_enum_returns_none() {
        let mut registry = TypeRegistry::new();
        registry.register(
            "Status".to_string(),
            TypeDef::Enum {
                type_params: vec![],
                variants: vec!["Ok".to_string(), "Error".to_string()],
                string_values: HashMap::new(),
                tag_field: None,
                variant_fields: HashMap::new(),
            },
        );

        assert!(generate_external_struct("Status", &registry).is_none());
    }

    #[test]
    fn test_generate_struct_optional_field_preserved() {
        let mut registry = TypeRegistry::new();
        register_external_struct(
            &mut registry,
            "Error",
            vec![("stack", RustType::Option(Box::new(RustType::String)))],
            vec![],
        );

        let item = generate_external_struct("Error", &registry).unwrap();
        match item {
            Item::Struct { fields, .. } => {
                assert_eq!(fields[0].ty, RustType::Option(Box::new(RustType::String)));
            }
            _ => panic!("expected Item::Struct"),
        }
    }

    #[test]
    fn test_generate_struct_nested_named_type_in_field() {
        let mut registry = TypeRegistry::new();
        register_external_struct(
            &mut registry,
            "Request",
            vec![(
                "headers",
                RustType::Named {
                    name: "Headers".to_string(),
                    type_args: vec![],
                },
            )],
            vec![],
        );

        let item = generate_external_struct("Request", &registry).unwrap();
        match item {
            Item::Struct { fields, .. } => {
                assert_eq!(
                    fields[0].ty,
                    RustType::Named {
                        name: "Headers".to_string(),
                        type_args: vec![],
                    }
                );
            }
            _ => panic!("expected Item::Struct"),
        }
    }

    // =========================================================================
    // camel_to_snake
    // =========================================================================

    #[test]
    fn test_camel_to_snake_basic() {
        assert_eq!(camel_to_snake("byteLength"), "byte_length");
        assert_eq!(camel_to_snake("lastIndex"), "last_index");
        assert_eq!(camel_to_snake("ignoreCase"), "ignore_case");
    }

    #[test]
    fn test_camel_to_snake_acronym() {
        assert_eq!(camel_to_snake("toISOString"), "to_iso_string");
        assert_eq!(camel_to_snake("bodyUsed"), "body_used");
    }

    #[test]
    fn test_camel_to_snake_already_lowercase() {
        assert_eq!(camel_to_snake("name"), "name");
        assert_eq!(camel_to_snake("source"), "source");
    }

    #[test]
    fn test_camel_to_snake_single_char() {
        assert_eq!(camel_to_snake("x"), "x");
    }

    #[test]
    fn test_camel_to_snake_pascal_case() {
        // PascalCase は先頭を小文字にする
        assert_eq!(camel_to_snake("ByteLength"), "byte_length");
    }

    #[test]
    fn test_camel_to_snake_all_uppercase() {
        assert_eq!(camel_to_snake("URL"), "url");
    }

    #[test]
    fn test_camel_to_snake_consecutive_acronyms() {
        assert_eq!(camel_to_snake("XMLHTTPRequest"), "xmlhttp_request");
    }

    #[test]
    fn test_camel_to_snake_empty() {
        assert_eq!(camel_to_snake(""), "");
    }

    // =========================================================================
    // T5: collect_all_undefined_references / generate_stub_structs
    // =========================================================================

    #[test]
    fn test_collect_all_undefined_refs_includes_non_external() {
        // registry に登録されていない型も検出する（is_external フィルタなし）
        let items = vec![Item::Enum {
            vis: Visibility::Public,
            name: "MyEnum".to_string(),
            serde_tag: None,
            variants: vec![EnumVariant {
                name: "A".to_string(),
                data: Some(RustType::Named {
                    name: "UnknownType".to_string(),
                    type_args: vec![],
                }),
                fields: vec![],
                value: None,
            }],
        }];
        let refs = collect_all_undefined_references(&items);
        assert!(refs.contains("UnknownType"));
    }

    #[test]
    fn test_collect_all_undefined_refs_excludes_defined() {
        let items = vec![
            Item::Struct {
                vis: Visibility::Public,
                name: "Foo".to_string(),
                type_params: vec![],
                fields: vec![],
            },
            Item::Enum {
                vis: Visibility::Public,
                name: "MyEnum".to_string(),
                serde_tag: None,
                variants: vec![EnumVariant {
                    name: "A".to_string(),
                    data: Some(RustType::Named {
                        name: "Foo".to_string(),
                        type_args: vec![],
                    }),
                    fields: vec![],
                    value: None,
                }],
            },
        ];
        let refs = collect_all_undefined_references(&items);
        assert!(!refs.contains("Foo"), "defined types should be excluded");
    }

    #[test]
    fn test_generate_stub_structs_creates_empty_stubs() {
        let registry = TypeRegistry::new();
        let mut items = vec![Item::Enum {
            vis: Visibility::Public,
            name: "MyEnum".to_string(),
            serde_tag: None,
            variants: vec![EnumVariant {
                name: "A".to_string(),
                data: Some(RustType::Named {
                    name: "MissingType".to_string(),
                    type_args: vec![],
                }),
                fields: vec![],
                value: None,
            }],
        }];
        generate_stub_structs(&mut items, &registry);
        let has_stub = items.iter().any(|item| {
            matches!(item, Item::Struct { name, fields, .. } if name == "MissingType" && fields.is_empty())
        });
        assert!(has_stub, "should generate stub for MissingType");
    }
}
