//! IR ベースで合成型の参照グラフを構築・参照するヘルパ。
//!
//! `OutputWriter` の合成型配置決定および単一ファイル API の合成型選択に使用する。
//! substring scan を排除し、IR レベルで参照関係を一貫して扱う。
//!
//! # 用語
//!
//! - **直接参照 (direct reference)**: user file の items が `RustType::Named { name }`
//!   経由で合成型を参照しているケース
//! - **合成型間依存 (synthetic dependency)**: 合成型 A の field 等が別の合成型 B を
//!   参照しているケース
//! - **推移参照 (transitive reference)**: ファイルが inline 配置された合成型を経由して
//!   shared 配置された合成型を間接的に参照しているケース
//!
//! # 設計原則
//!
//! 各合成型の「参照しているファイル」と「依存している合成型」を別々のマップで保持し、
//! 推移閉包の計算は呼び出し側が必要に応じて実行する。グラフ自体は immutable で、
//! `OutputWriter` と `extract_single_output` の両方から共通利用される。

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

use crate::ir::Item;
use crate::pipeline::external_struct_generator::collect_type_refs_from_item;

/// IR ベースで構築された合成型の参照グラフ。
pub struct SyntheticReferenceGraph {
    /// 合成型名 → 直接参照している user file の集合
    direct_referencers: HashMap<String, BTreeSet<PathBuf>>,
    /// 合成型 A → A から参照される他の合成型の集合
    synthetic_dependencies: HashMap<String, BTreeSet<String>>,
    /// 合成型名 → 生成済みコード文字列
    code: HashMap<String, String>,
    /// 合成型名の順序保持リスト（決定的出力のため）
    names_in_order: Vec<String>,
}

impl SyntheticReferenceGraph {
    /// 全 user file の items と全 synthetic items から参照グラフを構築する。
    ///
    /// # Arguments
    ///
    /// - `per_file_items`: 各 user file の (rel_path, items) のスライス。
    ///   `items` は user file の rust_source を生成した IR 全体。
    /// - `synthetic_items`: 全合成型の items。順序が出力順序を決める。
    pub fn build(per_file_items: &[(PathBuf, &[Item])], synthetic_items: &[Item]) -> Self {
        // 1. 合成型ごとの code とエントリ初期化
        let mut code: HashMap<String, String> = HashMap::new();
        let mut names_in_order: Vec<String> = Vec::new();
        for item in synthetic_items {
            let Some(name) = item.canonical_name() else {
                continue;
            };
            // 同名重複は最初のものを優先（pipeline 側で dedup されている前提）
            if code.contains_key(name) {
                continue;
            }
            let generated = crate::generator::generate(std::slice::from_ref(item));
            code.insert(name.to_string(), generated);
            names_in_order.push(name.to_string());
        }

        // 2. user file ごとに、その items を walk して合成型名への参照を収集
        let mut direct_referencers: HashMap<String, BTreeSet<PathBuf>> = HashMap::new();
        for (path, items) in per_file_items {
            let mut refs: HashSet<String> = HashSet::new();
            for item in *items {
                collect_type_refs_from_item(item, &mut refs);
            }
            for r in refs {
                if code.contains_key(&r) {
                    direct_referencers
                        .entry(r)
                        .or_default()
                        .insert(path.clone());
                }
            }
        }

        // 3. 合成型同士の依存関係（A の field 等から B を参照）
        let mut synthetic_dependencies: HashMap<String, BTreeSet<String>> = HashMap::new();
        for item in synthetic_items {
            let Some(name) = item.canonical_name() else {
                continue;
            };
            let mut refs: HashSet<String> = HashSet::new();
            collect_type_refs_from_item(item, &mut refs);
            for r in refs {
                if r != name && code.contains_key(&r) {
                    synthetic_dependencies
                        .entry(name.to_string())
                        .or_default()
                        .insert(r);
                }
            }
        }

        Self {
            direct_referencers,
            synthetic_dependencies,
            code,
            names_in_order,
        }
    }

    /// 合成型 `name` を直接参照しているファイルの集合（空集合あり）。
    pub fn direct_referencers(&self, name: &str) -> BTreeSet<PathBuf> {
        self.direct_referencers
            .get(name)
            .cloned()
            .unwrap_or_default()
    }

    /// 合成型 `name` が他のいずれかの合成型から参照されているか。
    pub fn is_referenced_by_synthetic(&self, name: &str) -> bool {
        self.synthetic_dependencies
            .values()
            .any(|deps| deps.contains(name))
    }

    /// 順序保持された合成型名一覧。
    pub fn names(&self) -> &[String] {
        &self.names_in_order
    }

    /// 合成型 `name` の生成済みコード。存在しない場合は空文字列。
    pub fn code_of(&self, name: &str) -> &str {
        self.code.get(name).map(String::as_str).unwrap_or("")
    }

    /// `start_names` から到達可能な全ての合成型名（依存合成型の推移閉包）を返す。
    ///
    /// `start_names` 自身も結果に含まれる。`shared_names` でフィルタする場合は呼び出し
    /// 側で intersection を取ること。
    pub fn reachable_synthetics(&self, start_names: &BTreeSet<String>) -> BTreeSet<String> {
        let mut visited: BTreeSet<String> = start_names.clone();
        let mut queue: Vec<String> = start_names.iter().cloned().collect();
        while let Some(n) = queue.pop() {
            if let Some(deps) = self.synthetic_dependencies.get(&n) {
                for d in deps {
                    if visited.insert(d.clone()) {
                        queue.push(d.clone());
                    }
                }
            }
        }
        visited
    }

    /// inline 配置された合成型の集合 `inline_for_file` が（推移的に）参照する
    /// shared 配置合成型の集合を返す。`inline_for_file` 自身は結果から除外する。
    ///
    /// 用途: ファイル A に inline 配置された合成型 Y が shared 配置合成型 X を field
    /// 等で参照している場合、ファイル A は X の `use` 文を必要とする。本関数で X を
    /// 列挙する。
    pub fn transitive_shared_refs(
        &self,
        inline_for_file: &BTreeSet<String>,
        shared_names: &BTreeSet<String>,
    ) -> BTreeSet<String> {
        let reachable = self.reachable_synthetics(inline_for_file);
        reachable
            .difference(inline_for_file)
            .filter(|n| shared_names.contains(n.as_str()))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{EnumVariant, RustType, StructField, Visibility};
    use std::path::Path;

    fn make_enum(name: &str, variant_types: &[(&str, &str)]) -> Item {
        Item::Enum {
            vis: Visibility::Public,
            name: name.to_string(),
            type_params: vec![],
            serde_tag: None,
            variants: variant_types
                .iter()
                .map(|(vname, ty_name)| EnumVariant {
                    name: vname.to_string(),
                    value: None,
                    data: Some(RustType::Named {
                        name: ty_name.to_string(),
                        type_args: vec![],
                    }),
                    fields: vec![],
                })
                .collect(),
        }
    }

    fn make_struct(name: &str, field_types: &[(&str, &str)]) -> Item {
        Item::Struct {
            vis: Visibility::Public,
            name: name.to_string(),
            type_params: vec![],
            fields: field_types
                .iter()
                .map(|(fname, ty_name)| StructField {
                    vis: Some(Visibility::Public),
                    name: fname.to_string(),
                    ty: RustType::Named {
                        name: ty_name.to_string(),
                        type_args: vec![],
                    },
                })
                .collect(),
        }
    }

    fn make_fn_with_param_type(name: &str, param_ty: &str) -> Item {
        Item::Fn {
            vis: Visibility::Public,
            attributes: vec![],
            is_async: false,
            name: name.to_string(),
            type_params: vec![],
            params: vec![crate::ir::Param {
                name: "x".to_string(),
                ty: Some(RustType::Named {
                    name: param_ty.to_string(),
                    type_args: vec![],
                }),
            }],
            return_type: None,
            body: vec![],
        }
    }

    #[test]
    fn test_build_direct_referencers_simple() {
        let synthetic = vec![make_enum("StringOrF64", &[])];
        let user_items = vec![make_fn_with_param_type("foo", "StringOrF64")];
        let per_file: Vec<(PathBuf, &[Item])> =
            vec![(PathBuf::from("a.rs"), user_items.as_slice())];
        let graph = SyntheticReferenceGraph::build(&per_file, &synthetic);

        let refs = graph.direct_referencers("StringOrF64");
        assert_eq!(refs.len(), 1);
        assert!(refs.contains(Path::new("a.rs")));
    }

    #[test]
    fn test_build_direct_referencers_multi_file() {
        let synthetic = vec![make_enum("StringOrF64", &[])];
        let a_items = vec![make_fn_with_param_type("foo", "StringOrF64")];
        let b_items = vec![make_fn_with_param_type("bar", "StringOrF64")];
        let per_file: Vec<(PathBuf, &[Item])> = vec![
            (PathBuf::from("a.rs"), a_items.as_slice()),
            (PathBuf::from("b.rs"), b_items.as_slice()),
        ];
        let graph = SyntheticReferenceGraph::build(&per_file, &synthetic);

        let refs = graph.direct_referencers("StringOrF64");
        assert_eq!(refs.len(), 2);
        assert!(refs.contains(Path::new("a.rs")));
        assert!(refs.contains(Path::new("b.rs")));
    }

    #[test]
    fn test_is_referenced_by_synthetic_yes() {
        // A の field が B を参照
        let synthetic = vec![make_struct("A", &[("b_field", "B")]), make_struct("B", &[])];
        let graph = SyntheticReferenceGraph::build(&[], &synthetic);
        assert!(graph.is_referenced_by_synthetic("B"));
        assert!(!graph.is_referenced_by_synthetic("A"));
    }

    #[test]
    fn test_is_referenced_by_synthetic_no() {
        // 独立した 2 つの合成型
        let synthetic = vec![make_struct("A", &[]), make_struct("B", &[])];
        let graph = SyntheticReferenceGraph::build(&[], &synthetic);
        assert!(!graph.is_referenced_by_synthetic("A"));
        assert!(!graph.is_referenced_by_synthetic("B"));
    }

    #[test]
    fn test_synthetic_dependencies_chain() {
        // A → B → C の連鎖
        let synthetic = vec![
            make_struct("A", &[("b", "B")]),
            make_struct("B", &[("c", "C")]),
            make_struct("C", &[]),
        ];
        let graph = SyntheticReferenceGraph::build(&[], &synthetic);

        // 直接依存
        assert!(graph.synthetic_dependencies.get("A").unwrap().contains("B"));
        assert!(graph.synthetic_dependencies.get("B").unwrap().contains("C"));
        assert!(!graph.synthetic_dependencies.contains_key("C"));

        // 推移閉包
        let start: BTreeSet<String> = std::iter::once("A".to_string()).collect();
        let reachable = graph.reachable_synthetics(&start);
        assert!(reachable.contains("A"));
        assert!(reachable.contains("B"));
        assert!(reachable.contains("C"));
    }

    #[test]
    fn test_transitive_shared_refs_direct() {
        // inline Y in file A → shared X
        let synthetic = vec![make_struct("Y", &[("x", "X")]), make_struct("X", &[])];
        let graph = SyntheticReferenceGraph::build(&[], &synthetic);

        let inline: BTreeSet<String> = std::iter::once("Y".to_string()).collect();
        let shared: BTreeSet<String> = std::iter::once("X".to_string()).collect();
        let result = graph.transitive_shared_refs(&inline, &shared);
        assert!(result.contains("X"));
        assert!(!result.contains("Y")); // Y 自身は除外
    }

    #[test]
    fn test_transitive_shared_refs_chain() {
        // inline A → shared B → shared C （A は inline、B/C は shared）
        let synthetic = vec![
            make_struct("A", &[("b", "B")]),
            make_struct("B", &[("c", "C")]),
            make_struct("C", &[]),
        ];
        let graph = SyntheticReferenceGraph::build(&[], &synthetic);

        let inline: BTreeSet<String> = std::iter::once("A".to_string()).collect();
        let shared: BTreeSet<String> = ["B", "C"].iter().map(|s| s.to_string()).collect();
        let result = graph.transitive_shared_refs(&inline, &shared);
        assert!(result.contains("B"));
        assert!(result.contains("C"));
    }

    #[test]
    fn test_transitive_shared_refs_no_inline() {
        // inline 集合が空 → 結果も空
        let synthetic = vec![make_struct("X", &[])];
        let graph = SyntheticReferenceGraph::build(&[], &synthetic);

        let inline: BTreeSet<String> = BTreeSet::new();
        let shared: BTreeSet<String> = std::iter::once("X".to_string()).collect();
        let result = graph.transitive_shared_refs(&inline, &shared);
        assert!(result.is_empty());
    }

    // ===== A-fix-6: 追加カバレッジ =====

    #[test]
    fn test_code_of_existing_and_missing() {
        let synthetic = vec![make_struct("Foo", &[])];
        let graph = SyntheticReferenceGraph::build(&[], &synthetic);
        // existing: 生成コードを返す（"struct Foo" を含むはず）
        let code = graph.code_of("Foo");
        assert!(!code.is_empty());
        assert!(code.contains("Foo"));
        // missing: 空文字列
        assert_eq!(graph.code_of("Bar"), "");
    }

    #[test]
    fn test_direct_referencers_unknown_returns_empty() {
        let synthetic = vec![make_struct("Foo", &[])];
        let graph = SyntheticReferenceGraph::build(&[], &synthetic);
        // 存在しない名前 → 空集合
        assert!(graph.direct_referencers("Unknown").is_empty());
        // 存在するが参照されていない → 空集合
        assert!(graph.direct_referencers("Foo").is_empty());
    }

    #[test]
    fn test_names_preserves_input_order() {
        // 入力順序が names() に保持されることを確認（決定的出力のため）
        let synthetic = vec![
            make_struct("Charlie", &[]),
            make_struct("Alpha", &[]),
            make_struct("Bravo", &[]),
        ];
        let graph = SyntheticReferenceGraph::build(&[], &synthetic);
        assert_eq!(graph.names(), &["Charlie", "Alpha", "Bravo"]);
    }

    #[test]
    fn test_build_empty_inputs() {
        let graph = SyntheticReferenceGraph::build(&[], &[]);
        assert!(graph.names().is_empty());
        assert!(graph.direct_referencers("Anything").is_empty());
        assert!(!graph.is_referenced_by_synthetic("Anything"));
    }

    #[test]
    fn test_build_duplicate_canonical_name_keeps_first() {
        // 同名の Item が複数渡されたとき、最初のものだけが採用される。
        // 2 個目はコメントを追加した別バージョンとし、生成コードで識別する。
        let first = Item::Struct {
            vis: Visibility::Public,
            name: "Foo".to_string(),
            type_params: vec![],
            fields: vec![StructField {
                vis: Some(Visibility::Public),
                name: "first_field".to_string(),
                ty: RustType::String,
            }],
        };
        let second = Item::Struct {
            vis: Visibility::Public,
            name: "Foo".to_string(),
            type_params: vec![],
            fields: vec![StructField {
                vis: Some(Visibility::Public),
                name: "second_field".to_string(),
                ty: RustType::String,
            }],
        };
        let graph = SyntheticReferenceGraph::build(&[], &[first, second]);
        assert_eq!(graph.names().len(), 1, "重複は dedup される");
        let code = graph.code_of("Foo");
        assert!(code.contains("first_field"));
        assert!(!code.contains("second_field"));
    }

    #[test]
    fn test_build_skips_unnamed_items() {
        // canonical_name() == None の Item（Use/Comment/RawCode）は skip される。
        // synthetic_items に Comment が含まれても names() には現れない。
        let synthetic = vec![
            Item::Comment("a comment".to_string()),
            make_struct("Foo", &[]),
            Item::Use {
                vis: Visibility::Private,
                path: "crate::bar".to_string(),
                names: vec!["Bar".to_string()],
            },
            Item::RawCode("fn raw() {}".to_string()),
        ];
        let graph = SyntheticReferenceGraph::build(&[], &synthetic);
        assert_eq!(graph.names(), &["Foo"]);
        // 空文字列での lookup は誤マッチしない
        assert_eq!(graph.code_of(""), "");
    }

    #[test]
    fn test_is_referenced_by_synthetic_indirect_does_not_match() {
        // is_referenced_by_synthetic は **直接依存** のみを判定する。
        // A → B → C のとき、C は B から「直接」参照される。
        // A から C は推移参照だが、is_referenced_by_synthetic("C") は B 由来で true を返す。
        // 一方 A は誰からも直接参照されていないので false を返す。
        let synthetic = vec![
            make_struct("A", &[("b", "B")]),
            make_struct("B", &[("c", "C")]),
            make_struct("C", &[]),
        ];
        let graph = SyntheticReferenceGraph::build(&[], &synthetic);
        assert!(!graph.is_referenced_by_synthetic("A"));
        assert!(graph.is_referenced_by_synthetic("B"));
        assert!(graph.is_referenced_by_synthetic("C"));
    }

    #[test]
    fn test_self_reference_does_not_loop() {
        // 自己参照（A の field が A 自身を参照、再帰型）。
        // canonical_name() == name のケースは synthetic_dependencies から除外されるため
        // reachable_synthetics は無限ループしない。
        let synthetic = vec![make_struct("A", &[("self_ref", "A")])];
        let graph = SyntheticReferenceGraph::build(&[], &synthetic);

        let start: BTreeSet<String> = std::iter::once("A".to_string()).collect();
        let reachable = graph.reachable_synthetics(&start);
        assert_eq!(reachable.len(), 1);
        assert!(reachable.contains("A"));
    }
}
