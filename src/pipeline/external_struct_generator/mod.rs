//! 参照されるビルトイン外部型の struct 定義を自動生成する。
//!
//! 変換出力（IR）内で参照されているが定義が存在しない外部型を検出し、
//! `TypeRegistry` のフィールド情報から `Item::Struct` を生成する。

use std::collections::HashSet;

use crate::ir::visit::IrVisitor;
use crate::ir::{
    camel_to_snake, sanitize_field_name, Expr, Item, RustType, StructField, Visibility,
};
use crate::pipeline::SyntheticTypeRegistry;
use crate::registry::{TypeDef, TypeRegistry};
use crate::ts_type_info::resolve::typedef::monomorphize_type_params;

/// Rust の標準ライブラリ型・serde 型など、struct 生成が不要な型名のセット。
///
/// 型 (type name) レベルのフィルタ。
///
/// # 歴史と責務分離
///
/// I-375 で `Expr::FnCall` が `CallTarget` で構造化され、FnCall 経由で `Some` /
/// `None` / `Ok` / `Err` が walker に登録されることはなくなった。I-377 → I-380 で
/// `Pattern::TupleStruct` / `Struct` / `UnitStruct` の `path: Vec<String>` を
/// 構造化 `PatternCtor` に置換し、`PatternCtor::Builtin(_)` は walker の
/// `visit_user_type_ref` フックを発火しないようにしたことで、pattern 経由の
/// builtin 流入も構造的に遮断された。`Some` / `None` / `Ok` / `Err` は型名ではなく
/// `Option` / `Result` の **variant コンストラクタ** であるため、型名フィルタ
/// である本定数からは除外する。
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
/// `scan_context` の役割:
/// - **定義済み判定**: scan_context 内の型は「定義済み」として扱われる。
/// - **参照走査**: scan_context 内の参照も undefined 候補に加える。
///
/// この関数は外部型 struct 生成 ([`generate_external_struct`]) のための候補名を返す。
/// `is_external` フィルタが効くため、ユーザー定義型が誤って取り込まれる心配はない。
/// 合成型のフィールドが参照する外部型を漏れなく検出する目的で scan_context をスキャンする。
///
/// 以下を除外する:
/// - `items`、`scan_context`、`defined_only` 内に既に定義が存在する型
///   （struct/enum/trait/type alias）
/// - Rust 標準ライブラリ型（`String`, `Vec`, `HashMap` 等）
/// - `serde_json::Value`
/// - 外部型でない型（ユーザー定義型）
///
/// `scan_context` は **定義+走査** の追加 items（per-file 合成型など）。
/// `defined_only` は **定義済み判定のみ** に使う items（他ファイルの合成型など）。
/// 走査対象から外すことで、無関係な型まで外部型生成の起点になる雪だるま現象を防ぐ。
pub fn collect_undefined_type_references(
    items: &[Item],
    scan_context: &[Item],
    defined_only: &[Item],
    registry: &TypeRegistry,
) -> HashSet<String> {
    let scope = UndefinedRefScope::new(items, scan_context, defined_only);
    scope
        .collect()
        .into_iter()
        .filter(|name| registry.is_external(name))
        .collect()
}

/// IR items を走査し、参照されているが定義がない型名を **全て** 収集する。
///
/// [`collect_undefined_type_references`] と異なり、`is_external` フィルタを適用しない。
/// shared_types.rs のスタブ生成で使用する — モジュール内の全未定義参照を解決するため。
///
/// `scan_context` は **定義+走査** の追加 items（per-file 合成型など）。
/// `defined_only` は **定義済み判定のみ** に使う items（他ファイルの合成型など）。
/// 走査対象から外すことで、無関係な型までスタブ化される雪だるま現象を防ぐ。
pub fn collect_all_undefined_references(
    items: &[Item],
    scan_context: &[Item],
    defined_only: &[Item],
) -> HashSet<String> {
    UndefinedRefScope::new(items, scan_context, defined_only).collect()
}

/// 未定義型参照の収集ロジック共通骨格。
///
/// `collect_undefined_type_references` と `collect_all_undefined_references` は
/// 「`is_external` フィルタを最後に追加で掛けるかどうか」のみ異なる。骨格は同一:
/// 1. 定義済み・インポート済み・型パラメータ名・標準型・`serde_json::Value`・パス形式
///    (`A::B`) の型名を除外集合に集める
/// 2. `items + scan_context` を walker で歩いて参照名を収集
/// 3. 除外集合を引いた残りを返す
struct UndefinedRefScope<'a> {
    items: &'a [Item],
    scan_context: &'a [Item],
    defined_only: &'a [Item],
}

impl<'a> UndefinedRefScope<'a> {
    fn new(items: &'a [Item], scan_context: &'a [Item], defined_only: &'a [Item]) -> Self {
        Self {
            items,
            scan_context,
            defined_only,
        }
    }

    /// 定義+判定+定義のみ をまとめた iterator。
    fn definition_pool(&self) -> impl Iterator<Item = &Item> {
        self.items
            .iter()
            .chain(self.scan_context.iter())
            .chain(self.defined_only.iter())
    }

    /// `items + scan_context` を返す（参照走査と型パラメータ収集の共通入力）。
    fn scan_pool(&self) -> impl Iterator<Item = &Item> {
        self.items.iter().chain(self.scan_context.iter())
    }

    fn collect(&self) -> HashSet<String> {
        let defined_types: HashSet<String> = self
            .definition_pool()
            .filter_map(|item| match item {
                Item::Struct { name, .. }
                | Item::Enum { name, .. }
                | Item::Trait { name, .. }
                | Item::TypeAlias { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect();

        let imported_types: HashSet<String> = self
            .definition_pool()
            .filter_map(|item| match item {
                Item::Use { names, .. } => Some(names.clone()),
                _ => None,
            })
            .flatten()
            .collect();

        let type_param_names: HashSet<String> = self
            .scan_pool()
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
        for item in self.scan_pool() {
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
}

/// 未定義型に対する空スタブ struct を生成し、items に追加する。
///
/// types.rs のコンパイルを通すため、参照されているが定義がない型にスタブを追加する。
/// TypeRegistry に struct 情報がある型はフル生成（[`generate_external_struct`] 経由）、
/// それ以外は空のユニット struct `pub struct TypeName;` を生成する。
/// フル生成した struct が新たな未定義参照を生む場合に備え、固定点に達するまで反復する。
pub fn generate_stub_structs(
    items: &mut Vec<Item>,
    scan_context: &[Item],
    defined_only: &[Item],
    registry: &TypeRegistry,
    synthetic: &SyntheticTypeRegistry,
) {
    for _ in 0..10 {
        let undefined = collect_all_undefined_references(items, scan_context, defined_only);
        if undefined.is_empty() {
            break;
        }
        // 出力順序を決定的にするためソート
        let mut sorted: Vec<String> = undefined.into_iter().collect();
        sorted.sort();
        for name in sorted {
            if let Some(full) = generate_external_struct(&name, registry, synthetic) {
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
/// 非 trait 制約を持つ型パラメータはモノモーフィゼーションで除去し、
/// フィールド型に制約型を置換する。
///
/// `TypeDef::Struct` 以外（`TypeDef::Enum`, `TypeDef::Function`）の場合は `None` を返す。
pub fn generate_external_struct(
    name: &str,
    registry: &TypeRegistry,
    synthetic: &SyntheticTypeRegistry,
) -> Option<Item> {
    let typedef = registry.get(name)?;
    match typedef {
        TypeDef::Struct {
            type_params,
            fields,
            ..
        } => {
            // モノモーフィゼーション: 非 trait 制約の型パラメータを具象型に置換
            let (mono_params, mono_subs) =
                monomorphize_type_params(type_params.clone(), registry, synthetic);

            let struct_fields: Vec<StructField> = fields
                .iter()
                .map(|field| {
                    let ty = field.ty.substitute(&mono_subs);
                    // 自己参照フィールドを Box でラップ（再帰型の infinite size 防止）
                    let ty = if references_type_name(&ty, name) {
                        RustType::Named {
                            name: "Box".to_string(),
                            type_args: vec![ty],
                        }
                    } else {
                        ty
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
                type_params: mono_params,
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

/// `Item` 内で参照されている `RustType::Named` 等の型名を再帰的に収集する。
///
/// I-380 で 8 個の手書き `collect_type_refs_from_*` 関数群を撤廃し、
/// `TypeRefCollector` (`IrVisitor` 実装) の単一 entrypoint に集約した。
/// 走査対象 (Enum/Struct/Fn/TypeAlias/Impl/Trait の各 variant、type_params の
/// constraint、method body) はすべて `walk_item` および `walk_*` 関数群に
/// 集約されており、新 IR variant 追加時の更新点は `walk_*` のみ。
///
/// 型レベルで user type 参照と保証された [`crate::ir::UserTypeRef`] は
/// `visit_user_type_ref` フック経由で登録される。`Pattern::TupleStruct` /
/// `Struct` / `UnitStruct` の `PatternCtor::Builtin(_)` (Some/None/Ok/Err) は
/// 型レベルで除外されるため、`PATTERN_LANG_BUILTINS` 等のハードコード除外
/// リストは不要 (I-380)。
pub(crate) fn collect_type_refs_from_item(item: &Item, refs: &mut HashSet<String>) {
    let mut collector = TypeRefCollector { refs };
    collector.visit_item(item);
}

/// テスト専用: `RustType` 単独を走査して refs を収集する。
///
/// 本体実装は `TypeRefCollector::visit_rust_type` に集約済み。テスト群が
/// `RustType` をスタンドアロンで検証するための薄い entrypoint。プロダクション
/// コードは [`collect_type_refs_from_item`] を経由する。
#[cfg(test)]
pub(crate) fn collect_refs_from_ty_for_test(ty: &RustType, refs: &mut HashSet<String>) {
    let mut collector = TypeRefCollector { refs };
    collector.visit_rust_type(ty);
}

/// IR 走査用の `IrVisitor` 実装で、ユーザー定義型参照を `refs` に収集する。
///
/// I-378 で `visit_user_type_ref` フックを、I-380 で `walk_pattern_ctor` 経由の
/// `Pattern` walker 統合を導入したことで、`Expr` / `Pattern` / `RustType` の
/// すべての user type 参照点を単一 visitor で拾える。`Expr::StructInit::name` /
/// `RustType::Named { name }` / `RustType::DynTrait(name)` のような
/// `UserTypeRef` ではない文字列型参照は対応する `visit_*` で個別処理する。
struct TypeRefCollector<'a> {
    refs: &'a mut HashSet<String>,
}

impl<'a> IrVisitor for TypeRefCollector<'a> {
    fn visit_user_type_ref(&mut self, r: &crate::ir::UserTypeRef) {
        // 構造的に user type 参照と保証されているため無条件登録。
        // builtin variant / プリミティブ / std module path は型レベルで除外
        // されている (`UserTypeRef` には格納できない)。
        self.refs.insert(r.as_str().to_string());
    }

    fn visit_expr(&mut self, expr: &Expr) {
        // `Expr::StructInit::name: String` は `UserTypeRef` 型ではないため
        // visit_user_type_ref フックでは拾えない。`Self` は impl 文脈の
        // implicit type なので登録しない (`pub struct Self {}` は予約語衝突)。
        if let Expr::StructInit { name, .. } = expr {
            if name != "Self" {
                self.refs.insert(name.clone());
            }
        }
        crate::ir::visit::walk_expr(self, expr);
    }

    fn visit_trait_ref(&mut self, tref: &crate::ir::TraitRef) {
        // `TraitRef::name` は `String` で `UserTypeRef` ではないため、ここで
        // 直接登録する。`type_args` の再帰は `walk_trait_ref` に委譲。
        self.refs.insert(tref.name.clone());
        crate::ir::visit::walk_trait_ref(self, tref);
    }

    fn visit_rust_type(&mut self, ty: &RustType) {
        // `RustType::Named { name }` / `RustType::DynTrait(name)` の型名は
        // `UserTypeRef` ではないため walk_rust_type の汎用 hook では拾えない。
        // ここで直接登録 + Self 除外 (impl 文脈の implicit type、struct stub
        // 生成しても予約語衝突でコンパイル不可)。子ノード (type_args 等) は
        // `walk_rust_type` に委譲。
        match ty {
            RustType::Named { name, .. } if name != "Self" => {
                self.refs.insert(name.clone());
            }
            RustType::DynTrait(name) => {
                self.refs.insert(name.clone());
            }
            _ => {}
        }
        crate::ir::visit::walk_rust_type(self, ty);
    }
}

#[cfg(test)]
mod tests;
