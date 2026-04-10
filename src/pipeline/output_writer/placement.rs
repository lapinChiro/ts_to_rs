//! 合成型の配置先決定ロジック。
//!
//! IR ベースの参照グラフ ([`SyntheticReferenceGraph`]) から、各合成型を以下のいずれかに
//! 振り分ける:
//!
//! - `inline`: 1 ファイルのみから参照される → そのファイルの先頭に追加
//! - `shared_module`: 2+ ファイルから参照、または他 synthetic item から参照 → 専用モジュール
//! - 未配置: 完全に未使用
//!
//! 配置決定後、inline 配置された合成型が shared 配置型を（field 等を介して）参照する場合、
//! 参照側ファイルに推移インポートを追加する（I-371 criterion 4）。

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use super::super::module_graph::file_path_to_module_path;
use super::super::placement::SyntheticReferenceGraph;
use super::super::types::OutputFile;
use super::{OutputWriter, SyntheticPlacement};
use crate::ir::Item;

impl<'a> OutputWriter<'a> {
    /// 合成型の配置先を決定する。
    ///
    /// IR ベースの参照グラフ ([`SyntheticReferenceGraph`]) を構築し、各合成型の参照
    /// ファイル数で配置先を決定する:
    /// - 1 ファイルのみで参照 → `inline`（そのファイルの先頭に追加）
    /// - 2+ ファイルで参照 → `shared_module`（専用 `.rs` ファイルに配置）
    /// - 0 ファイル + 他 synthetic から参照 → `shared_module`（相互依存解決）
    /// - 0 ファイル → 未使用（どちらにも含まない）
    ///
    /// 配置決定後、inline 配置された合成型が（field 等を介して）shared 配置型を
    /// 参照する場合、参照側ファイルに推移インポートを追加する（I-371 criterion 4）。
    pub fn resolve_synthetic_placement(
        &self,
        file_outputs: &[OutputFile<'_>],
        synthetic_items: &[Item],
    ) -> SyntheticPlacement {
        // 1. IR ベース参照グラフを構築
        let per_file_items: Vec<(PathBuf, &[Item])> = file_outputs
            .iter()
            .map(|f| (f.rel_path.clone(), f.items))
            .collect();
        let graph = SyntheticReferenceGraph::build(&per_file_items, synthetic_items);

        // 2. 各合成型を inline / shared / unused に振り分け
        let mut inline: HashMap<PathBuf, Vec<(String, String)>> = HashMap::new();
        let mut shared_items: Vec<String> = Vec::new();
        // 共有モジュールに配置された型ごとに、その「直接」参照ファイル一覧を記録する。
        // 後続で推移インポート計算と shared_imports 構築に使用する。
        let mut shared_type_refs: Vec<(String, Vec<PathBuf>)> = Vec::new();
        // 各ファイルに inline 配置された合成型の集合（推移インポート計算用）。
        let mut inline_by_file: HashMap<PathBuf, BTreeSet<String>> = HashMap::new();

        for name in graph.names() {
            let referencing_files = graph.direct_referencers(name);
            let referenced_by_synthetic = graph.is_referenced_by_synthetic(name);
            let code = graph.code_of(name).to_string();

            match referencing_files.len() {
                0 if referenced_by_synthetic => {
                    // file_outputs からは未参照だが、他の synthetic item から参照される
                    // → shared module に配置（相互依存を解決）
                    shared_items.push(code);
                    shared_type_refs.push((name.clone(), Vec::new()));
                }
                0 => {
                    // 完全に未使用 — 配置しない
                }
                1 if !referenced_by_synthetic => {
                    // 1 ファイルのみ → inline
                    let file = referencing_files.iter().next().unwrap().clone();
                    inline
                        .entry(file.clone())
                        .or_default()
                        .push((name.clone(), code));
                    inline_by_file.entry(file).or_default().insert(name.clone());
                }
                _ => {
                    // 2+ ファイル、または 1 ファイル + synthetic 参照 → shared module
                    shared_items.push(code);
                    shared_type_refs.push((name.clone(), referencing_files.into_iter().collect()));
                }
            }
        }

        // 3. user 定義型名 → 定義ファイル rel_path のマッピング構築 (I-382)
        let user_type_def_map: HashMap<String, PathBuf> = file_outputs
            .iter()
            .flat_map(|f| {
                f.items.iter().filter_map(move |item| match item {
                    Item::Struct { name, .. }
                    | Item::Enum { name, .. }
                    | Item::Trait { name, .. }
                    | Item::TypeAlias { name, .. } => Some((name.clone(), f.rel_path.clone())),
                    _ => None,
                })
            })
            .collect();

        // shared 配置された型名の集合（推移計算・user 型 import 共通で使用）
        let shared_names: BTreeSet<String> =
            shared_type_refs.iter().map(|(n, _)| n.clone()).collect();

        // 4. shared module の合成
        let shared_module = if shared_items.is_empty() {
            None
        } else {
            let body = shared_items.join("\n\n");
            let stdlib_imports = generate_shared_module_imports(&body);

            // I-382: shared module が参照する user 定義型の import を生成
            let shared_user_types = graph.user_types_in_scope(&shared_names, &shared_names);
            let user_imports = build_user_type_imports(&shared_user_types, &user_type_def_map);

            let all_imports = combine_import_sections(&stdlib_imports, &user_imports);
            let content = if all_imports.is_empty() {
                body
            } else {
                format!("{all_imports}\n\n{body}")
            };
            let module_path = choose_shared_module_path(file_outputs);
            Some((module_path, content))
        };

        // 5. 推移インポート計算: inline 配置された合成型 → shared 配置型への参照を辿り、
        //    参照側ファイルに shared 型を追加する（I-371 criterion 4）。
        for (file, inline_names) in &inline_by_file {
            let transitive = graph.transitive_shared_refs(inline_names, &shared_names);
            for shared_name in transitive {
                if let Some((_, files)) =
                    shared_type_refs.iter_mut().find(|(n, _)| n == &shared_name)
                {
                    if !files.contains(file) {
                        files.push(file.clone());
                    }
                }
            }
        }

        // 6. shared_imports: 各参照ファイルに対して `use crate::<stem>::{Type, ...};` を構築。
        let mut shared_imports: HashMap<PathBuf, Vec<String>> =
            if let Some((ref module_path, _)) = shared_module {
                build_shared_imports(module_path, &shared_type_refs)
            } else {
                HashMap::new()
            };

        // 7. I-382: inline 配置された合成型が参照する user 定義型の import を追加。
        //    同一ファイルの型は import 不要（同一モジュール scope）。
        for (file, inline_names) in &inline_by_file {
            let inline_user_types = graph.user_types_in_scope(inline_names, inline_names);
            let mut needed: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
            for user_type in inline_user_types {
                if let Some(def_file) = user_type_def_map.get(&user_type) {
                    if def_file != file {
                        let module = file_path_to_module_path(def_file);
                        needed.entry(module).or_default().insert(user_type);
                    }
                }
            }
            if !needed.is_empty() {
                let imports = shared_imports.entry(file.clone()).or_default();
                for (module, types) in &needed {
                    let names: Vec<&str> = types.iter().map(String::as_str).collect();
                    imports.push(format_use_statement(module, &names));
                }
            }
        }

        SyntheticPlacement {
            inline,
            shared_module,
            shared_imports,
        }
    }
}

/// 共有モジュールに配置された型の参照ファイル群から、ファイル別の `use` 文を構築する。
///
/// 各ファイルが参照する型を `use crate::<stem>::{T1, T2, ...};` の単一文に集約する。
/// 参照ファイルが共有モジュール自身の場合は除外する（自己インポートを防ぐ）。
fn build_shared_imports(
    module_path: &Path,
    shared_type_refs: &[(String, Vec<PathBuf>)],
) -> HashMap<PathBuf, Vec<String>> {
    let stem = module_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("shared_types");

    // file → 参照する型名の集合
    let mut by_file: HashMap<PathBuf, BTreeSet<String>> = HashMap::new();
    for (type_name, files) in shared_type_refs {
        for file in files {
            if file == module_path {
                continue;
            }
            by_file
                .entry(file.clone())
                .or_default()
                .insert(type_name.clone());
        }
    }

    let module = format!("crate::{stem}");
    by_file
        .into_iter()
        .map(|(file, types)| {
            let names: Vec<&str> = types.iter().map(String::as_str).collect();
            let import = format_use_statement(&module, &names);
            (file, vec![import])
        })
        .collect()
}

/// 共有合成型モジュールのファイルパスを決定する。
///
/// ユーザーの出力ファイルと衝突しない名前を選択する。
/// デフォルトは `shared_types.rs`。衝突する場合はサフィックスを付与する。
pub(super) fn choose_shared_module_path(file_outputs: &[OutputFile<'_>]) -> PathBuf {
    let existing: std::collections::HashSet<&Path> =
        file_outputs.iter().map(|f| f.rel_path.as_path()).collect();

    let base = PathBuf::from("shared_types.rs");
    if !existing.contains(base.as_path()) {
        return base;
    }

    // 衝突回避: サフィックスを付与
    for i in 0u32.. {
        let candidate = PathBuf::from(format!("shared_types_{i}.rs"));
        if !existing.contains(candidate.as_path()) {
            return candidate;
        }
    }

    unreachable!("infinite counter guarantees a unique name")
}

/// user 定義型名の集合と定義ファイルマッピングから `use` import 文を生成する。
///
/// module path ごとに型名をグループ化し `use <module>::{T1, T2, ...};` にまとめる。
/// module path 辞書順でソート（決定的出力）。
fn build_user_type_imports(
    user_types: &BTreeSet<String>,
    user_type_def_map: &HashMap<String, PathBuf>,
) -> Vec<String> {
    // module_path → 型名のグループ化
    let mut by_module: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for user_type in user_types {
        if let Some(def_file) = user_type_def_map.get(user_type) {
            let module = file_path_to_module_path(def_file);
            by_module
                .entry(module)
                .or_default()
                .insert(user_type.clone());
        }
    }

    by_module
        .iter()
        .map(|(module, types)| {
            let names: Vec<&str> = types.iter().map(String::as_str).collect();
            format_use_statement(module, &names)
        })
        .collect()
}

/// stdlib import 文字列と user 型 import リストを結合する。
fn combine_import_sections(stdlib_imports: &str, user_imports: &[String]) -> String {
    let mut parts = Vec::new();
    if !stdlib_imports.is_empty() {
        parts.push(stdlib_imports.to_string());
    }
    if !user_imports.is_empty() {
        parts.push(user_imports.join("\n"));
    }
    parts.join("\n")
}

/// module path と型名リストから `use <module>::{T1, T2};` 形式のインポート文を生成する。
///
/// # Panics
///
/// `names` が空の場合。呼び出し側は BTreeMap/BTreeSet の走査結果から型名を渡すため、
/// 空になることは構造的にないが、万一の場合は無効な Rust コード生成を防ぐ。
fn format_use_statement(module: &str, names: &[&str]) -> String {
    assert!(
        !names.is_empty(),
        "format_use_statement: names must not be empty"
    );
    if names.len() == 1 {
        format!("use {module}::{};", names[0])
    } else {
        format!("use {module}::{{{}}};", names.join(", "))
    }
}

/// 共有合成型モジュールに必要なインポート文を生成する。
///
/// 生成されたコード内で使用されているが、モジュール自身ではインポートされていない
/// クレートや標準ライブラリ型のインポートを検出して生成する。
fn generate_shared_module_imports(code: &str) -> String {
    let mut imports = Vec::new();

    if code.contains("serde_json::") {
        imports.push("use serde_json;");
    }
    if code.contains("HashMap<") {
        imports.push("use std::collections::HashMap;");
    }

    imports.join("\n")
}
