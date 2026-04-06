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

        // 2. 合成型の中で「定義系」(`Item::Impl`) は、文法上 struct/enum と密結合
        //    である。`impl Foo` を生成し、`struct Foo` が user file F で定義されて
        //    いるなら、その impl は文法上 F に属さなければ Rust 上意味を成さない。
        //    したがって impl item の `canonical_name`（= `struct_name`）を file F が
        //    定義していれば、F を impl の "referencer" として扱う。
        //
        //    これを per_file 走査前に計算し、後段の `direct_referencers` に集約する
        //    ことで、`render_referenced_synthetics_for_file`（単一ファイル API）と
        //    `OutputWriter::resolve_synthetic_placement`（マルチファイル API）が同じ
        //    semantics を共有する。
        let synthetic_impl_targets: HashSet<&str> = synthetic_items
            .iter()
            .filter_map(|i| match i {
                Item::Impl { struct_name, .. } => Some(struct_name.as_str()),
                _ => None,
            })
            .collect();

        // 3. user file ごとに、その items を walk して合成型名への参照を収集
        let mut direct_referencers: HashMap<String, BTreeSet<PathBuf>> = HashMap::new();
        for (path, items) in per_file_items {
            let mut refs: HashSet<String> = HashSet::new();
            for item in *items {
                collect_type_refs_from_item(item, &mut refs);
                // file が定義する型が synthetic impl の対象なら、その型名を参照と
                // して扱う（impl 配置を struct 定義に追従させるため）。
                if let Some(name) = item.canonical_name() {
                    if synthetic_impl_targets.contains(name) {
                        refs.insert(name.to_string());
                    }
                }
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

/// 単一ファイル API 向けに、`file_items` から（推移的に）参照される合成型のコードを
/// 生成して返す。
///
/// アルゴリズム:
/// 1. IR ベース参照グラフで `file_items` から直接参照される合成型を起点に、合成型間の
///    依存関係の推移閉包を取り「必要な合成型名集合」を求める。`collect_type_refs_from_item`
///    は fn body / impl method body / struct literal を含む全 IR を歩くため、ここで
///    substring scan は不要。
/// 2. struct/enum/trait/type alias の合成 Item は `file_items` に同名定義が存在する場合
///    は出力しない（per-file 外部型 struct 生成と post-loop の重複対策）。impl ブロック
///    のような非定義 Item は同名 struct があっても必ず emit する。
/// 3. emit 順は `synthetic_items` の元順序を保持する。
pub fn render_referenced_synthetics_for_file(
    file_path: &std::path::Path,
    file_items: &[Item],
    synthetic_items: &[Item],
) -> String {
    if synthetic_items.is_empty() {
        return String::new();
    }
    let graph =
        SyntheticReferenceGraph::build(&[(file_path.to_path_buf(), file_items)], synthetic_items);
    let direct: BTreeSet<String> = graph
        .names()
        .iter()
        .filter(|n| !graph.direct_referencers(n).is_empty())
        .cloned()
        .collect();
    let needed = graph.reachable_synthetics(&direct);

    let already_defined: HashSet<String> = file_items
        .iter()
        .filter(|i| is_definition_item(i))
        .filter_map(|i| i.canonical_name().map(str::to_string))
        .collect();

    let mut emit: Vec<Item> = Vec::new();
    for item in synthetic_items {
        let Some(name) = item.canonical_name() else {
            continue;
        };
        // graph::build が impl 対象 struct を直接参照として登録するため、impl ブロックは
        // needed に含まれる（その struct を file が定義していれば、direct_referencers が
        // file を返す）。よってここでは「needed に含まれているか」のみで判定すれば良く、
        // impl の特別扱いは不要。
        if !needed.contains(name) {
            continue;
        }
        if is_definition_item(item) && already_defined.contains(name) {
            continue;
        }
        emit.push(item.clone());
    }
    if emit.is_empty() {
        String::new()
    } else {
        crate::generator::generate(&emit)
    }
}

/// 「named な定義系」Item を判定する。
///
/// これらは同名でファイル内と synthetic 双方に存在すると Rust コンパイル時に
/// 衝突するため、`render_referenced_synthetics_for_file` で dedup の対象になる。
/// `Item::Impl` は文法上 struct と独立に複数併存できるため対象外（impl block は
/// `is_definition_item == false` で同名 struct があっても emit される）。
fn is_definition_item(item: &Item) -> bool {
    matches!(
        item,
        Item::Struct { .. }
            | Item::Enum { .. }
            | Item::Trait { .. }
            | Item::TypeAlias { .. }
            | Item::Fn { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{EnumVariant, Expr, Method, RustType, Stmt, StructField, Visibility};
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

    // ===== render_referenced_synthetics_for_file =====

    fn fn_returning(name: &str, ret_ty: &str) -> Item {
        Item::Fn {
            vis: Visibility::Public,
            attributes: vec![],
            is_async: false,
            name: name.to_string(),
            type_params: vec![],
            params: vec![],
            return_type: Some(RustType::Named {
                name: ret_ty.to_string(),
                type_args: vec![],
            }),
            body: vec![],
        }
    }

    #[test]
    fn test_render_empty_synthetic_returns_empty() {
        let file_items = vec![fn_returning("foo", "Bar")];
        let result = render_referenced_synthetics_for_file(Path::new("a.rs"), &file_items, &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_render_unreferenced_synthetic_omitted() {
        // ファイルが何も参照しない → unused synthetic は emit しない
        let file_items = vec![fn_returning("foo", "String")];
        let synthetic = vec![make_enum("Unused", &[("X", "String")])];
        let result =
            render_referenced_synthetics_for_file(Path::new("a.rs"), &file_items, &synthetic);
        assert!(result.is_empty());
    }

    #[test]
    fn test_render_direct_referenced_synthetic_emitted() {
        // ファイルが Foo を参照 → synthetic の Foo enum が emit される
        let file_items = vec![fn_returning("foo", "Bar")];
        let synthetic = vec![make_enum("Bar", &[("X", "String")])];
        let result =
            render_referenced_synthetics_for_file(Path::new("a.rs"), &file_items, &synthetic);
        assert!(result.contains("enum Bar"));
    }

    #[test]
    fn test_render_transitive_synthetic_emitted() {
        // file → A → B → C 推移閉包で全部 emit
        let file_items = vec![fn_returning("foo", "A")];
        let synthetic = vec![
            make_struct("A", &[("b", "B")]),
            make_struct("B", &[("c", "C")]),
            make_struct("C", &[]),
        ];
        let result =
            render_referenced_synthetics_for_file(Path::new("a.rs"), &file_items, &synthetic);
        assert!(result.contains("struct A"));
        assert!(result.contains("struct B"));
        assert!(result.contains("struct C"));
    }

    #[test]
    fn test_render_definition_dedup_when_already_in_file() {
        // file.items に既に struct Foo がある場合、synthetic の struct Foo は emit しない
        let file_items = vec![make_struct("Foo", &[]), fn_returning("g", "Foo")];
        let synthetic = vec![make_struct("Foo", &[])];
        let result =
            render_referenced_synthetics_for_file(Path::new("a.rs"), &file_items, &synthetic);
        assert!(result.is_empty(), "should dedup struct Foo");
    }

    #[test]
    fn test_render_impl_for_defined_struct_emitted() {
        // file.items に struct Foo、synthetic に impl Foo がある → impl は emit される
        let file_items = vec![make_struct("Foo", &[])];
        let synthetic = vec![Item::Impl {
            struct_name: "Foo".to_string(),
            type_params: vec![],
            for_trait: None,
            consts: vec![],
            methods: vec![Method {
                vis: Visibility::Public,
                name: "greet".to_string(),
                has_self: true,
                has_mut_self: false,
                params: vec![],
                return_type: None,
                body: Some(vec![]),
            }],
        }];
        let result =
            render_referenced_synthetics_for_file(Path::new("a.rs"), &file_items, &synthetic);
        assert!(
            result.contains("impl Foo"),
            "impl block must be emitted even if struct is already_defined"
        );
    }

    #[test]
    fn test_render_emit_order_preserves_synthetic_order() {
        // synthetic_items の元順序が emit 順に保持される
        let file_items = vec![fn_returning("foo", "A"), fn_returning("bar", "B")];
        let synthetic = vec![make_struct("B", &[]), make_struct("A", &[])];
        let result =
            render_referenced_synthetics_for_file(Path::new("a.rs"), &file_items, &synthetic);
        let pos_b = result.find("struct B").expect("B emitted");
        let pos_a = result.find("struct A").expect("A emitted");
        assert!(
            pos_b < pos_a,
            "B should come before A (synthetic_items order)"
        );
    }

    #[test]
    fn test_render_self_referential_synthetic_does_not_loop() {
        // 自己参照型: file → A、A の field は A 自身を参照
        let file_items = vec![fn_returning("foo", "A")];
        let synthetic = vec![make_struct("A", &[("self_ref", "A")])];
        let result =
            render_referenced_synthetics_for_file(Path::new("a.rs"), &file_items, &synthetic);
        assert!(result.contains("struct A"));
    }

    #[test]
    fn test_render_referenced_via_fn_body_struct_init() {
        // fn body の StructInit から参照される合成型も emit される（fn body walker の効果）
        let file_items = vec![Item::Fn {
            vis: Visibility::Public,
            attributes: vec![],
            is_async: false,
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::TailExpr(Expr::StructInit {
                name: "_TypeLit0".to_string(),
                fields: vec![],
                base: None,
            })],
        }];
        let synthetic = vec![make_struct("_TypeLit0", &[])];
        let result =
            render_referenced_synthetics_for_file(Path::new("a.rs"), &file_items, &synthetic);
        assert!(
            result.contains("_TypeLit0"),
            "synthetic referenced via StructInit in fn body should be emitted"
        );
    }

    #[test]
    fn test_render_referenced_via_fn_body_qualified_call() {
        // fn body の `Color::Red(x)` 呼び出しから Color を参照と判定して emit
        let file_items = vec![Item::Fn {
            vis: Visibility::Public,
            attributes: vec![],
            is_async: false,
            name: "foo".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![Stmt::Expr(Expr::FnCall {
                // Synthetic enum variant constructor — references `Color`.
                target: crate::ir::CallTarget::assoc("Color", "Red"),
                args: vec![],
            })],
        }];
        let synthetic = vec![make_enum("Color", &[("Red", "String")])];
        let result =
            render_referenced_synthetics_for_file(Path::new("a.rs"), &file_items, &synthetic);
        assert!(
            result.contains("enum Color"),
            "synthetic enum referenced via variant constructor should be emitted"
        );
    }

    #[test]
    fn test_is_definition_item_true_for_definitions() {
        assert!(is_definition_item(&make_struct("X", &[])));
        assert!(is_definition_item(&make_enum("Y", &[])));
        assert!(is_definition_item(&Item::Trait {
            vis: Visibility::Public,
            name: "T".to_string(),
            type_params: vec![],
            supertraits: vec![],
            methods: vec![],
            associated_types: vec![],
        }));
        assert!(is_definition_item(&Item::TypeAlias {
            vis: Visibility::Public,
            name: "A".to_string(),
            type_params: vec![],
            ty: RustType::String,
        }));
    }

    #[test]
    fn test_is_definition_item_true_for_fn() {
        // Item::Fn も同名衝突対象（同名関数が file と synthetic に併存すると Rust の
        // duplicate definition 違反になる）
        assert!(is_definition_item(&Item::Fn {
            vis: Visibility::Public,
            attributes: vec![],
            is_async: false,
            name: "f".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: None,
            body: vec![],
        }));
    }

    #[test]
    fn test_is_definition_item_false_for_impl_and_others() {
        // Impl は定義系ではない（impl は struct/enum と独立に emit される）
        assert!(!is_definition_item(&Item::Impl {
            struct_name: "Foo".to_string(),
            type_params: vec![],
            for_trait: None,
            consts: vec![],
            methods: vec![],
        }));
        assert!(!is_definition_item(&Item::Comment("c".to_string())));
        assert!(!is_definition_item(&Item::RawCode("r".to_string())));
        assert!(!is_definition_item(&Item::Use {
            vis: Visibility::Private,
            path: "p".to_string(),
            names: vec![],
        }));
    }
}
