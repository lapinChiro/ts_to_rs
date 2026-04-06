# Batch 11c (I-371) レビュー後修正タスク

スコープ: コミット前 self-review で発見された 12 個の問題（問題 1〜10 + 追加発見 11, 12）を、原理的な理想状態へ構造解消する。

判断基準: 「実害の有無」ではなく「原理的な理想状態との乖離」。規模を度外視。既存課題かつ修正コスト大は TODO に詳細記載。

進め方: `/large-scale-refactor` skill 規定に従い、Step 1 (Analysis) → Step 2 (Design) → Step 3 (Task Breakdown) → Step 4 (Review) → Step 5 (Implementation)。

---

## Step 1: Analysis

### 1.1 問題と修正対象の対応表

| # | 問題 | 修正対象（file:line） | 種別 |
|---|------|----------------------|------|
| 1 | inline → shared 推移インポート未生成 | `src/pipeline/output_writer.rs:87 resolve_synthetic_placement` | 振る舞い |
| 2 | `let _ = output.synthetic_items;` 不明瞭 | `src/lib.rs:173` | 可読性 |
| 3 | `synthetic_item_name` / `synthetic_type_name` 重複 | `src/lib.rs:239`, `src/pipeline/output_writer.rs:339` | DRY |
| 4 | `collect_undefined_type_references` / `collect_all_undefined_references` API 非対称 | `src/pipeline/external_struct_generator/mod.rs:40, 84` | 設計 |
| 5 | substring scan の脆弱性 | `src/pipeline/output_writer.rs:112, 119` , `src/lib.rs:217` | 設計 |
| 6 | `PerFileTransformed` 関数内ローカル定義 | `src/pipeline/mod.rs:127` | 構造 |
| 7 | `extract_single_output` の推移閉包スキャン重複 | `src/lib.rs:196 collect_referenced_synthetic_code` | DRY |
| 8 | `FileOutput::file_synthetic_items` 死蔵 | `src/pipeline/types.rs:105` | 設計 |
| 9 | パイプライン統合テスト欠落 | `tests/` 直下に新規 | テスト |
| 10 | snapshot 6 件を一括 accept | `tests/snapshots/integration_test__{basic_types,inline_type_literal_param,typeof_const,instanceof_builtin,external_type_struct,instanceof_builtin_with_builtins}.snap` | 検証 |
| 11 | `collect_type_refs_from_item` が Impl/Trait/TypeAlias 未対応 | `src/pipeline/external_struct_generator/mod.rs:260` | 振る舞い |
| 12 | `OutputWriter::write_to_directory` API が IR を渡せない | `src/pipeline/output_writer.rs:183` | 設計 |

### 1.2 関数シグネチャ現状（grep 結果）

```
src/pipeline/output_writer.rs:87
  pub fn resolve_synthetic_placement(
      &self,
      file_outputs: &[(PathBuf, String)],
      synthetic_items: &[Item],
  ) -> SyntheticPlacement

src/pipeline/output_writer.rs:183
  pub fn write_to_directory(
      &self,
      output_dir: &Path,
      file_outputs: &[(PathBuf, String)],
      synthetic_items: &[Item],
      run_rustfmt: bool,
  ) -> Result<()>

src/pipeline/output_writer.rs:339
  fn synthetic_type_name(item: &Item) -> String

src/lib.rs:172
  fn extract_single_output(output: pipeline::TranspileOutput) -> Result<pipeline::FileOutput>

src/lib.rs:196
  fn collect_referenced_synthetic_code(rust_source: &str, synthetic_items: &[ir::Item]) -> String

src/lib.rs:239
  fn synthetic_item_name(item: &ir::Item) -> String

src/pipeline/external_struct_generator/mod.rs:40
  pub fn collect_undefined_type_references(
      items: &[Item],
      scan_context: &[Item],
      registry: &TypeRegistry,
  ) -> HashSet<String>

src/pipeline/external_struct_generator/mod.rs:84
  pub fn collect_all_undefined_references(
      items: &[Item],
      scan_context: &[Item],
      defined_only: &[Item],
  ) -> HashSet<String>

src/pipeline/external_struct_generator/mod.rs:158
  pub fn generate_stub_structs(
      items: &mut Vec<Item>,
      scan_context: &[Item],
      defined_only: &[Item],
      registry: &TypeRegistry,
      synthetic: &SyntheticTypeRegistry,
  )

src/pipeline/external_struct_generator/mod.rs:260
  fn collect_type_refs_from_item(item: &Item, refs: &mut HashSet<String>)
  // 現状: Enum, Struct, Fn のみ walk。Impl/Trait/TypeAlias 未対応

src/pipeline/mod.rs:127 (関数内ローカル定義)
  struct PerFileTransformed {
      path: PathBuf,
      source: String,
      items: Vec<Item>,
      unsupported: Vec<UnsupportedSyntaxError>,
      file_synthetic_items: Vec<Item>,
  }

src/pipeline/mod.rs:290
  fn generate_external_structs_to_fixpoint(
      items: &mut Vec<Item>,
      scan_context: &[Item],
      registry: &TypeRegistry,
      synthetic: &SyntheticTypeRegistry,
  )

src/pipeline/types.rs:92
  pub struct FileOutput {
      pub path: PathBuf,
      pub source: String,
      pub rust_source: String,
      pub unsupported: Vec<UnsupportedSyntaxError>,
      pub file_synthetic_items: Vec<Item>,  // ← 削除対象 (#8)
  }
```

### 1.3 呼び出し元一覧

**`resolve_synthetic_placement` の呼び出し元（4 箇所）**:
- `src/pipeline/output_writer.rs:190` (`write_to_directory` 内)
- `src/pipeline/output_writer.rs:535, 561, 575, 771, 797, 822` (test, 計 6 箇所)

**`write_to_directory` の呼び出し元（11 箇所）**:
- `src/main.rs:234` (production)
- `src/pipeline/output_writer.rs:597, 617, 651, 687, 731, 854, 886, 926` (test 8 箇所)

**`extract_single_output` の呼び出し元（3 箇所、すべて lib.rs 内）**:
- `src/lib.rs:41` (`transpile_collecting`)
- `src/lib.rs:61` (`transpile_collecting` の別経路)
- `src/lib.rs:87` (`transpile_with_builtins`)

**`collect_undefined_type_references` の呼び出し元**:
- `src/pipeline/mod.rs:298` (`generate_external_structs_to_fixpoint` 内)
- `src/pipeline/external_struct_generator/tests.rs` 16 箇所（line: 56, 91, 115, 148, 171, 194, 218, 255, 302, 332, 362, 398, 429, 458, 488, 517）

**`collect_all_undefined_references` の呼び出し元**:
- `src/pipeline/external_struct_generator/mod.rs:166` (`generate_stub_structs` 内)
- `src/pipeline/external_struct_generator/tests.rs:1129, 1158`

**`generate_stub_structs` の呼び出し元**:
- `src/pipeline/mod.rs:197` (per-file)
- `src/pipeline/mod.rs:227` (post-loop)
- `src/pipeline/external_struct_generator/tests.rs:1180`

**`generate_external_structs_to_fixpoint` の呼び出し元**:
- `src/pipeline/mod.rs:185` (per-file)
- `src/pipeline/mod.rs:224` (post-loop)

**`FileOutput::file_synthetic_items` の使用箇所**:
- `src/pipeline/types.rs:105` (定義)
- `src/pipeline/mod.rs:212` (構築)
- `src/lib.rs:183` (`extract_single_output` 内で消費)
- `src/pipeline/mod.rs:339` (`transpile_single` 内で消費)

**`collect_type_refs_from_item` の呼び出し元**:
- `src/pipeline/external_struct_generator/mod.rs:61, 135` (private)
- 修正後: `src/pipeline/placement.rs` から呼び出すため `pub(crate)` 化

### 1.4 依存グラフ

```
A-1 (canonical_name)        ─┐
A-2 (collect_type_refs 強化) ─┼─→ A-3 (placement モジュール)
                              │
A-3 (placement)              ─┼─→ B-2 (resolve_placement IR 化)
                              │     ↓
                              │   B-3 (推移インポート)
                              │
B-1 (OutputFile API 変更)    ─┴─→ C-2 (FileOutput.items 化)
                                    ↓
                                  C-1 (PerFileTransformed 外出し)
                                    ↓
                                  D-1 (extract_single_output IR 統合)
                                    ↓
                                  D-2 (let _ 削除)
                                    ↓
                                  E (API 対称化, 独立)
                                    ↓
                                  F-1 (統合テスト)
                                    ↓
                                  F-2 (snapshot 個別検証)
                                    ↓
                                  G (検証)
```

依存上、A → B → C → D → E → F → G の順序を厳守。

### 1.5 既知の TODO 化対象（本スコープ外）

| ID | 問題 | 修正コスト | 残留理由 |
|----|------|-----------|---------|
| T1 | クロスファイル外部型重複（jws.rs に `pub struct Algorithm {}` stub） | 大 | user 型 + builtin 外部型のクロスモジュール解決機構が必要。`pipeline::placement` を user 型まで拡張 + `ModuleGraph` との連携 |
| T2 | `Item::Fn::body` 内型参照を IR walk しない | 中 | 現状の signature-level walk で実用上問題なし。Stmt の網羅 walk が必要 |

→ G-4 で TODO ファイルに追記する。

---

## Step 2: Design

### 2.1 `Item::canonical_name()` (#3)

**場所**: `src/ir/mod.rs` の `impl Item` ブロック

```rust
impl Item {
    /// Item の識別名を返す。命名対象の Item は `Some(name)` を、Comment / RawCode /
    /// Use のように単一の識別名を持たない Item は `None` を返す。
    ///
    /// 合成型の参照グラフ構築や placement 判定など、Item を名前で索引する用途で使用する。
    pub fn canonical_name(&self) -> Option<&str> {
        match self {
            Item::Struct { name, .. }
            | Item::Enum { name, .. }
            | Item::Trait { name, .. }
            | Item::TypeAlias { name, .. }
            | Item::Fn { name, .. } => Some(name),
            Item::Impl { struct_name, .. } => Some(struct_name),
            Item::Comment(_) | Item::RawCode(_) | Item::Use { .. } => None,
        }
    }
}
```

**置換対象**:
- `src/lib.rs:239 synthetic_item_name` → 削除、`item.canonical_name().unwrap_or("")` で代替（呼び出し元で `is_empty()` チェック済）
- `src/pipeline/output_writer.rs:339 synthetic_type_name` → 削除、同様に置換

**注**: `synthetic_type_name` は `Use { path }` に対し `path.clone()`、`Comment(text)` / `RawCode(text)` に対し `text.clone()` を返していたが、これらは合成型では発生しないため `None` 扱いで safe（呼び出し側で名前空文字 → スキップ）。

### 2.2 `collect_type_refs_from_item` 強化 (#11)

**場所**: `src/pipeline/external_struct_generator/mod.rs:260`

**追加対応**:

```rust
pub(crate) fn collect_type_refs_from_item(item: &Item, refs: &mut HashSet<String>) {
    match item {
        Item::Enum { variants, .. } => { /* 既存 */ }
        Item::Struct { fields, .. } => { /* 既存 */ }
        Item::Fn { return_type, params, .. } => { /* 既存 */ }
        // ↓ 追加
        Item::TypeAlias { ty, .. } => {
            collect_type_refs_from_rust_type(ty, refs);
        }
        Item::Impl { for_trait, methods, .. } => {
            if let Some(tref) = for_trait {
                refs.insert(tref.name.clone());
                for arg in &tref.type_args {
                    collect_type_refs_from_rust_type(arg, refs);
                }
            }
            for method in methods {
                if let Some(rt) = &method.return_type {
                    collect_type_refs_from_rust_type(rt, refs);
                }
                for param in &method.params {
                    if let Some(ty) = &param.ty {
                        collect_type_refs_from_rust_type(ty, refs);
                    }
                }
            }
        }
        Item::Trait { methods, supertraits, .. } => {
            for sup in supertraits {
                refs.insert(sup.name.clone());
                for arg in &sup.type_args {
                    collect_type_refs_from_rust_type(arg, refs);
                }
            }
            for method in methods {
                if let Some(rt) = &method.return_type {
                    collect_type_refs_from_rust_type(rt, refs);
                }
                for param in &method.params {
                    if let Some(ty) = &param.ty {
                        collect_type_refs_from_rust_type(ty, refs);
                    }
                }
            }
        }
        Item::Use { .. } | Item::Comment(_) | Item::RawCode(_) => {}
    }
}
```

**TraitRef / Method の構造確認**: A-2 着手前に `src/ir/mod.rs` で `TraitRef`, `Method` フィールドを再確認すること。

`pub(crate)` に変更（`pipeline::placement` から呼び出すため）。

### 2.3 `pipeline::placement` モジュール (#5, #7)

**新規ファイル**: `src/pipeline/placement.rs`

```rust
//! IR ベースで合成型の参照グラフを構築・参照するヘルパ。
//!
//! OutputWriter の合成型配置決定および単一ファイル API の合成型選択に使用する。
//! substring scan を排除し、IR レベルで参照関係を一貫して扱う。

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use crate::ir::Item;
use crate::pipeline::external_struct_generator::collect_type_refs_from_item;

/// 合成型の参照グラフ。
///
/// - 各合成型がどの user file から直接参照されているか
/// - 各合成型が他のどの合成型から参照されているか
/// - 各合成型の生成済みコード文字列
pub struct SyntheticReferenceGraph {
    /// 合成型名 → 直接参照している user file の集合
    direct_referencers: HashMap<String, BTreeSet<PathBuf>>,
    /// 合成型 A → A から参照される他の合成型の集合
    synthetic_dependencies: HashMap<String, BTreeSet<String>>,
    /// 合成型名 → 生成済みコード文字列
    code: HashMap<String, String>,
    /// 合成型名のみの順序保持リスト（決定的出力のため）
    names_in_order: Vec<String>,
}

impl SyntheticReferenceGraph {
    /// 全 user file の items と全 synthetic items から参照グラフを構築する。
    ///
    /// `per_file_items` は (rel_path, items) のタプル。`items` は user file の
    /// rust_source を生成した IR 全体（user code + per-file 外部型 struct を含む）。
    pub fn build(
        per_file_items: &[(PathBuf, &[Item])],
        synthetic_items: &[Item],
    ) -> Self {
        // 1. 合成型ごとの code とエントリ初期化
        let mut code = HashMap::new();
        let mut names_in_order = Vec::new();
        for item in synthetic_items {
            if let Some(name) = item.canonical_name() {
                let generated = crate::generator::generate(std::slice::from_ref(item));
                code.insert(name.to_string(), generated);
                names_in_order.push(name.to_string());
            }
        }

        // 2. user file ごとに、その items を walk して合成型名への参照を収集
        let mut direct_referencers: HashMap<String, BTreeSet<PathBuf>> = HashMap::new();
        for (path, items) in per_file_items {
            let mut refs = std::collections::HashSet::new();
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
            let Some(name) = item.canonical_name() else { continue; };
            let mut refs = std::collections::HashSet::new();
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
        self.direct_referencers.get(name).cloned().unwrap_or_default()
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

    /// inline 配置情報を受け取り、ファイル `file` が（推移的に）参照する shared
    /// 配置合成型の集合を返す。
    ///
    /// アルゴリズム:
    ///   visited = file の inline 合成型の集合
    ///   queue = visited のコピー
    ///   while queue not empty:
    ///     n = queue.pop()
    ///     for d in synthetic_dependencies[n]:
    ///       if d not in visited:
    ///         visited.insert(d)
    ///         queue.push(d)
    ///   return { d in visited if d in shared_names }
    pub fn transitive_shared_refs_for_file(
        &self,
        inline_for_file: &BTreeSet<String>,
        shared_names: &BTreeSet<String>,
    ) -> BTreeSet<String> {
        let mut visited: BTreeSet<String> = inline_for_file.clone();
        let mut queue: Vec<String> = inline_for_file.iter().cloned().collect();
        while let Some(n) = queue.pop() {
            if let Some(deps) = self.synthetic_dependencies.get(&n) {
                for d in deps {
                    if visited.insert(d.clone()) {
                        queue.push(d.clone());
                    }
                }
            }
        }
        visited.intersection(shared_names).cloned().collect()
    }
}
```

**unit test 設計**:

```rust
// src/pipeline/placement.rs 内 mod tests
test_build_direct_referencers_simple        // 1 ファイル → 1 合成型の参照
test_build_direct_referencers_multi_file    // 2 ファイル → 同じ合成型の参照
test_is_referenced_by_synthetic_yes         // A の field が B を参照
test_is_referenced_by_synthetic_no          // 独立した合成型
test_synthetic_dependencies_chain           // A → B → C の連鎖
test_transitive_shared_refs_direct          // inline → shared 直接参照
test_transitive_shared_refs_chain           // inline → shared → shared 連鎖
test_transitive_shared_refs_no_inline       // inline 空の場合は空集合
```

### 2.4 `OutputFile<'a>` 構造体と OutputWriter API 変更 (#12)

**場所**: `src/pipeline/output_writer.rs`

```rust
/// OutputWriter に渡すファイル情報のビュー。
///
/// IR ベース placement のために `items` を保持する。
pub struct OutputFile<'a> {
    pub rel_path: PathBuf,
    pub source: &'a str,
    pub items: &'a [Item],
}

impl OutputWriter<'_> {
    pub fn resolve_synthetic_placement(
        &self,
        file_outputs: &[OutputFile<'_>],
        synthetic_items: &[Item],
    ) -> SyntheticPlacement { /* ... */ }

    pub fn write_to_directory(
        &self,
        output_dir: &Path,
        file_outputs: &[OutputFile<'_>],
        synthetic_items: &[Item],
        run_rustfmt: bool,
    ) -> Result<()> { /* ... */ }
}
```

**main.rs (`src/main.rs:234`) の更新**:

```rust
let outputs: Vec<OutputFile<'_>> = pipeline_output
    .files
    .iter()
    .zip(ts_files.iter())
    .map(|(fo, ts_path)| {
        let rs_path = directory::compute_output_path(ts_path, input_dir, &output_dir)
            .unwrap_or_else(|_| ts_path.with_extension("rs"));
        let rel_path = rs_path.strip_prefix(&output_dir).unwrap_or(&rs_path).to_path_buf();
        OutputFile {
            rel_path,
            source: &fo.rust_source,
            items: &fo.items,
        }
    })
    .collect();
writer.write_to_directory(&output_dir, &outputs, &pipeline_output.synthetic_items, true)?;
```

**output_writer.rs 内 test の更新**: 8 箇所すべての `&[(PathBuf, String)]` を `&[OutputFile<'_>]` に書き換え。空の `items` を渡せるようヘルパを用意:

```rust
fn make_outputs<'a>(items_storage: &'a Vec<Vec<Item>>, files: &'a [(&str, &str)]) -> Vec<OutputFile<'a>> {
    files.iter().enumerate().map(|(i, (path, src))| OutputFile {
        rel_path: PathBuf::from(path),
        source: src,
        items: &items_storage[i],
    }).collect()
}
```

### 2.5 `resolve_synthetic_placement` IR 化 (#5, #7) と推移インポート追加 (#1)

**現状（substring scan）**:
```rust
let referencing_files: Vec<&PathBuf> = file_outputs
    .iter()
    .filter(|(_, source)| source.contains(name))
    .map(|(path, _)| path)
    .collect();
let referenced_by_synthetic = generated.iter()
    .any(|(other_name, other_code)| other_name != name && other_code.contains(name));
```

**変更後（IR ベース）**:
```rust
let graph = SyntheticReferenceGraph::build(
    &file_outputs.iter().map(|f| (f.rel_path.clone(), f.items)).collect::<Vec<_>>(),
    synthetic_items,
);

for name in graph.names() {
    let referencing_files = graph.direct_referencers(name);
    let referenced_by_synthetic = graph.is_referenced_by_synthetic(name);

    match referencing_files.len() {
        0 if referenced_by_synthetic => {
            shared_items.push(graph.code_of(name).to_string());
            shared_type_refs.push((name.clone(), Vec::new()));
        }
        0 => { /* 未使用 */ }
        1 if !referenced_by_synthetic => {
            inline.entry(referencing_files.iter().next().unwrap().clone())
                .or_default()
                .push(graph.code_of(name).to_string());
        }
        _ => {
            shared_items.push(graph.code_of(name).to_string());
            shared_type_refs.push((name.clone(), referencing_files.into_iter().collect()));
        }
    }
}
```

**推移インポート処理（B-3）**:

```rust
// inline 配置決定後、各ファイルの inline 合成型集合を構築
let mut inline_assignments: HashMap<PathBuf, BTreeSet<String>> = HashMap::new();
for (file, codes) in &inline {
    // codes は code 文字列なので、name を逆引きする必要あり
    // → inline を Vec<String> ではなく Vec<(name, code)> に変更
}

// shared に配置された型の名前集合
let shared_names: BTreeSet<String> = shared_type_refs.iter().map(|(n, _)| n.clone()).collect();

// 各ファイルについて推移参照を計算し、shared_imports に追加
for (file, inline_names) in &inline_assignments {
    let transitive = graph.transitive_shared_refs_for_file(inline_names, &shared_names);
    for shared_name in transitive {
        // shared_type_refs[shared_name] のファイル一覧に file を追加
        if let Some(refs) = shared_type_refs.iter_mut().find(|(n, _)| n == &shared_name) {
            if !refs.1.contains(file) {
                refs.1.push(file.clone());
            }
        }
    }
}
```

**inline placement の構造変更**: `inline: HashMap<PathBuf, Vec<(String, String)>>` (name, code) に変更し、name から逆引きできるようにする。public field なので **API 破壊変更**。これも本スコープで対応。

### 2.6 `extract_single_output` の placement 統合 (#7)

**場所**: `src/lib.rs`

```rust
fn extract_single_output(output: pipeline::TranspileOutput) -> Result<pipeline::FileOutput> {
    let pipeline::TranspileOutput { files, synthetic_items, .. } = output;
    let mut file = files.into_iter().next()
        .ok_or_else(|| anyhow::anyhow!("pipeline returned no output files"))?;

    if synthetic_items.is_empty() {
        return Ok(file);
    }

    // IR ベースで「このファイルが参照する合成型」と「その推移閉包」を計算
    let graph = pipeline::placement::SyntheticReferenceGraph::build(
        &[(file.path.clone(), &file.items)],
        &synthetic_items,
    );
    let mut included = std::collections::BTreeSet::new();
    // 直接参照
    for name in graph.names() {
        if !graph.direct_referencers(name).is_empty() {
            included.insert(name.clone());
        }
    }
    // 推移閉包: 合成型の依存も追加
    let mut queue: Vec<String> = included.iter().cloned().collect();
    while let Some(n) = queue.pop() {
        // synthetic_dependencies は private なので getter 経由
        for dep in graph.transitive_shared_refs_for_file(
            &std::iter::once(n.clone()).collect(),
            &graph.names().iter().cloned().collect(),
        ) {
            if included.insert(dep.clone()) {
                queue.push(dep);
            }
        }
    }

    // 順序保持された names から included を抽出してコード結合
    let parts: Vec<&str> = graph.names().iter()
        .filter(|n| included.contains(n.as_str()))
        .map(|n| graph.code_of(n))
        .collect();
    let prepended = parts.join("\n\n");
    if !prepended.is_empty() {
        file.rust_source = if file.rust_source.is_empty() {
            prepended
        } else {
            format!("{prepended}\n\n{}", file.rust_source)
        };
    }
    Ok(file)
}
```

`collect_referenced_synthetic_code` と `synthetic_item_name` を削除。

### 2.7 `PerFileTransformed` 外出し (#6)

**場所**: `src/pipeline/types.rs` に追加、`src/pipeline/mod.rs` から削除

```rust
// src/pipeline/types.rs
pub(crate) struct PerFileTransformed {
    pub path: PathBuf,
    pub source: String,
    pub items: Vec<crate::ir::Item>,
    pub unsupported: Vec<UnsupportedSyntaxError>,
    pub file_synthetic_items: Vec<crate::ir::Item>,
}
```

C-2 完了後、`file_synthetic_items` フィールドは `FileOutput` から削除されるが `PerFileTransformed` には残すか検討。実装内部で per-file pass の scan_context として必要なので残す。

### 2.8 `FileOutput` の `items` 化 (#8)

**変更**:
```rust
// src/pipeline/types.rs
pub struct FileOutput {
    pub path: PathBuf,
    pub source: String,
    pub rust_source: String,
    pub unsupported: Vec<UnsupportedSyntaxError>,
    pub items: Vec<crate::ir::Item>,  // 新規
    // file_synthetic_items: Vec<Item>  ← 削除
}
```

`pipeline::transpile_pipeline` 内で `all_items` を `FileOutput.items` として保存。

`extract_single_output` と `transpile_single` は `file.items` + `output.synthetic_items` から IR ベース計算（D-1 で実装）。

### 2.9 `let _ = output.synthetic_items` 削除 (#2)

D-1 で `extract_single_output` を書き換える際、分解パターン:
```rust
let pipeline::TranspileOutput { files, synthetic_items, .. } = output;
```
で `let _` を不要にする。

### 2.10 API 対称化 (#4)

**変更**:
```rust
// src/pipeline/external_struct_generator/mod.rs
pub fn collect_undefined_type_references(
    items: &[Item],
    scan_context: &[Item],
    defined_only: &[Item],  // 新規
    registry: &TypeRegistry,
) -> HashSet<String>
```

`defined_only` セマンティクス: 「定義済み判定のみ、参照走査しない」。`collect_all_undefined_references` と同じ。

`generate_external_structs_to_fixpoint` (`src/pipeline/mod.rs:290`) も同様にシグネチャ拡張:
```rust
fn generate_external_structs_to_fixpoint(
    items: &mut Vec<Item>,
    scan_context: &[Item],
    defined_only: &[Item],
    registry: &TypeRegistry,
    synthetic: &SyntheticTypeRegistry,
)
```

呼び出し元:
- per-file: `defined_only=&[]`
- post-loop: `defined_only=&[]`
- test 16 箇所: `defined_only=&[]`

### 2.11 統合テスト設計 (#9)

`tests/pipeline_placement_test.rs` を新規作成。

```rust
// 1. transitive 推移インポート
#[test]
fn test_transitive_inline_to_shared_imports() {
    // 入力: 1 ファイルが Y を inline 配置、Y が X を参照、X は別ファイルからも参照されて shared 配置
    // 期待: そのファイルの出力に X の use 文が含まれる
}

// 2. クロスファイル合成型 dedup
#[test]
fn test_cross_file_synthetic_no_duplicate_stub() {
    // 入力: 2 ファイル、両方が同じ union (string|number) を含む
    // 期待: shared_types.rs に 1 個だけ enum、各ファイルに use crate::shared_types::... が生成
    //       各ファイルに pub struct/enum のスタブが生成されない
}

// 3. 未参照合成型は出力されない
#[test]
fn test_unreferenced_synthetic_not_emitted() {
    // 入力: 単一ファイルで union を作るが結果的にどこからも参照しない
    // 期待: その合成型が出力に含まれない
}

// 4. 連鎖合成型 A→B→C
#[test]
fn test_synthetic_chain_a_to_b_to_c() {
    // 入力: A union が B union を含み、B が C を含む。user は A のみ参照
    // 期待: A, B, C が全て出力 / 配置されており、参照側に適切な use 文
}

// 5. サブディレクトリからの crate::shared_types 参照
#[test]
fn test_shared_types_imports_from_subdirectory() {
    // 入力: utils/x.ts と root/y.ts で同じ union を共有
    // 期待: utils/x.rs に use crate::shared_types::T; が生成
}

// 6. 自己参照 inline （安全性チェック）
#[test]
fn test_inline_self_reference_does_not_loop() {
    // 入力: union が自分自身を間接参照する（再帰型）
    // 期待: 無限ループせず、適切に配置
}
```

### 2.12 snapshot 6 件の個別検証 (#10)

**手順**:
1. 各 `.snap` の最新版を `cat`
2. **本タスク開始前の状態**（Phase 1〜5 を経て accept 済み版）と比較
3. 差分を以下のいずれかに分類:
   - **dead code 削除**: 元々参照されていない synthetic が IR ベース化により正しく除去
   - **並べ替え**: 内容同一、生成順だけ変化
   - **機能後退**: NG → 即修正
4. 全件の判定を最終コミットメッセージに記載

検証対象:
- `tests/snapshots/integration_test__basic_types.snap`
- `tests/snapshots/integration_test__inline_type_literal_param.snap`
- `tests/snapshots/integration_test__typeof_const.snap`
- `tests/snapshots/integration_test__instanceof_builtin.snap`
- `tests/snapshots/integration_test__external_type_struct.snap`
- `tests/snapshots/integration_test__instanceof_builtin_with_builtins.snap`

`typeof_const` の差分は本 self-review 段階では未確認のため、F-2 で改めて確認する。

---

## Step 3: Implementation Tasks

各タスクは原則 1 ファイル変更。Phase 末に `[WIP]` コミットを user に提案。

### Phase A: 共通基盤の整備

- [x] **A-1-1**: `src/ir/mod.rs` に `impl Item { pub fn canonical_name(&self) -> Option<&str> }` を追加
  - 完了基準: `cargo check` パス
- [x] **A-1-2**: `src/lib.rs:239` の `synthetic_item_name` を削除し、`src/lib.rs:203` で `item.canonical_name()` を使用
  - 完了基準: `cargo check --tests` パス（本 Phase 中は OutputWriter は未変更で残る）
- [x] **A-1-3**: `src/pipeline/output_writer.rs:339` の `synthetic_type_name` を削除し、`src/pipeline/output_writer.rs:102` で `item.canonical_name()` を使用
  - 完了基準: `cargo check --tests` パス
- [x] **A-2-1**: `src/pipeline/external_struct_generator/mod.rs:260` の `collect_type_refs_from_item` に `Item::Impl`, `Item::Trait`, `Item::TypeAlias` 分岐を追加し、`pub(crate)` 化。`collect_type_refs_from_rust_type` も `pub(crate)` 化
  - 完了基準: `cargo test pipeline::external_struct_generator` パス
- [x] **A-3-1**: `src/pipeline/placement.rs` を新規作成。`SyntheticReferenceGraph` 実装 + 8 件の unit test
  - 完了基準: `cargo test pipeline::placement` パス
- [x] **A-3-2**: `src/pipeline/mod.rs` の `pub mod placement;` 宣言を追加
  - 完了基準: `cargo check` パス
- [x] **A-fix-1**: `src/lib.rs::collect_referenced_synthetic_code` で canonical_name None を skip（filter_map 化）
- [x] **A-fix-2**: `src/pipeline/output_writer.rs::resolve_synthetic_placement` で同様に skip
- [x] **A-fix-3**: `placement.rs::transitive_shared_refs_for_file` の未使用 `_file` パラメータを削除し `transitive_shared_refs` にリネーム
- [x] **A-fix-4**: `external_struct_generator/tests.rs` に Impl/Trait/TypeAlias walking テスト 6 件追加
- [x] **A-fix-5**: `src/ir/tests/mod.rs` に `Item::canonical_name()` テスト 9 件追加
- [x] **A-fix-6**: `placement.rs` に edge case テスト 7 件追加（dedup priority, unnamed skip, names 順序, 直接 vs 推移依存判定）
- [x] **A-fix-self-keyword**: `collect_type_refs_from_rust_type` で `Self` を ref 収集対象から除外（impl method の `-> Self` で `pub struct Self {}` が生成される regression を防ぐ）
- [x] **A-fix-7**: `cargo test` 全体実行 → 2009 + 派生テスト全 pass、`cargo clippy --all-targets -- -D warnings` パス、`cargo fmt --all` パス
- [x] **A-fix-8**: `Item::Impl` 分岐に `consts[i].ty` walking を追加（A-2-1 取りこぼし）
- [x] **A-fix-9**: `test_collect_type_refs_from_impl_consts` テスト追加
- [x] **A-fix-10**: `test_collect_type_refs_excludes_self` テスト追加（A-fix-self-keyword の regression 防止）
- [x] **A-fix-11**: `test_resolve_synthetic_placement_skips_unnamed_items` テスト追加（A-fix-2 の regression 防止）
- [x] **A-fix-12**: `TODO` に I-374（Rust 予約語と衝突する型名のサニタイズ）を追加。user メモに完了マーク
- [x] **A-fix-13**: 全テスト 2012 件 + clippy/fmt クリーン再確認
- [ ] **A-COMMIT**: `[WIP] Batch 11c-fix Phase A: 共通基盤（canonical_name, type_refs 強化, placement モジュール）`

### Phase B: OutputWriter リファクタ

実行順序メモ: B-1-5 で main.rs から `&fo.items` を渡す必要があるため、依存上 C-2-1
（FileOutput.items 追加）と C-2-2（pipeline での構築）を Phase B に **前倒し** した。

- [x] **B-1-1**: `src/pipeline/types.rs` に `pub struct OutputFile<'a>` を追加、`pipeline/mod.rs` から re-export
  - 完了基準: `cargo check` パス
- [x] **C-2-1 (前倒し)**: `FileOutput` に `pub items: Vec<Item>` を追加（`file_synthetic_items` は Phase D まで残置）
- [x] **C-2-2 (前倒し)**: `src/pipeline/mod.rs::transpile_pipeline` で `FileOutput.items: all_items` を構築
- [x] **B-1-2**: `SyntheticPlacement.inline` の型を `HashMap<PathBuf, Vec<(String, String)>>` に変更（name, code）
- [x] **B-1-3 + B-1-4**: `OutputWriter::resolve_synthetic_placement` と `write_to_directory` のシグネチャを `OutputFile<'_>` ベースに変更
- [x] **B-1-5 + C-2-3**: `src/main.rs` の呼び出しを `OutputFile` ベースに更新（`&fo.items` を渡す）
- [x] **B-2-1**: `resolve_synthetic_placement` の本体を IR ベース（`SyntheticReferenceGraph::build`）に置換。`source.contains(name)` と `other_code.contains(name)` を削除
- [x] **B-3-1**: `resolve_synthetic_placement` 末尾に推移インポート処理を追加。`SyntheticReferenceGraph::transitive_shared_refs` を呼び、shared_imports に推移参照を merge
- [x] **B-test-rewrite**: `output_writer.rs` の test 22 件を `OutputFile` API に追従。`TestFile` / `outputs_from` / `fn_returning` / `fn_with_param_type` ヘルパを導入
  - 完了基準: `cargo test output_writer` 全 22 件パス
- [x] **B-quality**: cargo test 全体（lib 2012 + 統合 + snapshot）/ cargo clippy / cargo fmt クリーン
- [ ] **B-COMMIT**: `[WIP] Batch 11c-fix Phase B: OutputWriter を IR ベース placement と推移インポートに移行 (+ 依存により C-2-1/2 前倒し)`

### Phase C: pipeline 変更

C-2-1〜2-3 は Phase B に前倒し済（Phase B の依存解消のため）。Phase C では C-1（PerFileTransformed
外出し）と、`FileOutput::file_synthetic_items` の最終削除のみ実行。

- [ ] **C-1-1**: `src/pipeline/types.rs` に `pub(crate) struct PerFileTransformed` を追加
  - 完了基準: `cargo check` パス
- [ ] **C-1-2**: `src/pipeline/mod.rs` から関数内ローカルの `struct PerFileTransformed` 定義を削除し、import に書き換え
  - 完了基準: `cargo check` パス
- [x] **~~C-2-1~~**: Phase B-1-1 で前倒し完了
- [x] **~~C-2-2~~**: Phase B-1-1 で前倒し完了
- [x] **~~C-2-3~~**: Phase B-1-5 で前倒し完了
- [ ] **C-2-final**: Phase D-1 完了後、`FileOutput::file_synthetic_items` を削除（D-1 で extract_single_output が `file.items` 経由に切り替わったあと）
- [ ] **C-COMMIT**: `[WIP] Batch 11c-fix Phase C: PerFileTransformed 外出し + FileOutput.file_synthetic_items 削除`

### Phase D: 単一ファイル API 整理

- [ ] **D-1-1**: `src/lib.rs` の `extract_single_output` を `pipeline::placement::SyntheticReferenceGraph` を使う実装に置換。`collect_referenced_synthetic_code` 削除、`synthetic_item_name` 削除
  - 完了基準: `cargo test --test integration_test` パス（snapshot 差異は F-2 で扱う）
- [ ] **D-1-2**: `src/pipeline/mod.rs::transpile_single` を同じ手法（IR ベース）で書き換え。`file.file_synthetic_items` 参照を削除し、`output.synthetic_items` + `file.items` から IR で計算
  - 完了基準: `cargo test pipeline::tests::test_pipeline_single_interface_produces_struct` 等 既存 test pass
- [ ] **D-2-1**: `extract_single_output` の `let _ = output.synthetic_items;` を分解パターンに置換（D-1-1 の実装内で対応済の場合はスキップ）
  - 完了基準: `cargo check` パス
- [ ] **D-COMMIT**: `[WIP] Batch 11c-fix Phase D: 単一ファイル API を pipeline::placement に統合`

### Phase E: external_struct_generator API 対称化

- [ ] **E-1-1**: `src/pipeline/external_struct_generator/mod.rs:40` の `collect_undefined_type_references` シグネチャに `defined_only: &[Item]` を追加。実装の defined_types セットに defined_only を chain
  - 完了基準: `cargo check` パス
- [ ] **E-1-2**: `src/pipeline/mod.rs:290 generate_external_structs_to_fixpoint` のシグネチャに `defined_only: &[Item]` を追加し、内部呼び出しに渡す
  - 完了基準: `cargo check` パス
- [ ] **E-1-3**: `src/pipeline/mod.rs:185` (per-file) と `src/pipeline/mod.rs:224` (post-loop) の呼び出しに `&[]` を追加
  - 完了基準: `cargo check` パス
- [ ] **E-1-4**: `src/pipeline/external_struct_generator/tests.rs` の 16 箇所に `&[]` を追加（`replace_all` で一括）
  - 完了基準: `cargo test pipeline::external_struct_generator` パス
- [ ] **E-COMMIT**: `[WIP] Batch 11c-fix Phase E: collect_undefined_type_references API 対称化`

### Phase F: テスト追加と snapshot 検証

- [ ] **F-1-1**: `tests/pipeline_placement_test.rs` を新規作成し、テスト 1 (transitive 推移) を実装
  - 完了基準: 該当テストがパス
- [ ] **F-1-2**: テスト 2 (cross-file dedup) を追加
- [ ] **F-1-3**: テスト 3 (未参照 synthetic 除外) を追加
- [ ] **F-1-4**: テスト 4 (連鎖 A→B→C) を追加
- [ ] **F-1-5**: テスト 5 (サブディレクトリ参照) を追加
- [ ] **F-1-6**: テスト 6 (自己参照非ループ) を追加
  - 各完了基準: 対応テストがパス
- [ ] **F-2-1**: 6 件の snapshot を `cargo test --test integration_test` で再生成し、差分を1件ずつ確認
  - 完了基準: 差分が「dead code 削除」「並べ替え」のみで、機能後退がないことを確認
- [ ] **F-2-2**: 6 件の snapshot 判定結果をコミットメッセージ草案に記録（`tasks.md` 末尾に記録）
- [ ] **F-COMMIT**: `[WIP] Batch 11c-fix Phase F: パイプライン統合テスト 6 件追加 + snapshot 個別検証`

### Phase G: 検証とドキュメント更新

- [ ] **G-1**: quality-check
  - `cargo fix --allow-dirty --allow-staged --tests`
  - `cargo fmt --all`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test`
  - 完了基準: 全て 0 error / 0 warning
- [ ] **G-2**: Hono ベンチ
  - `cargo build --release && ./scripts/hono-bench.sh`
  - 完了基準: dir compile ≥ 157/158、E0428+E0119 = 0
- [ ] **G-3-1**: Hono 出力スキャン用の Python script を `/tmp/scan_transitive_refs.py` に作成
- [ ] **G-3-2**: Phase 1.1 の手法で 38 件の transitive ref が 0 件になったことを検証
  - 完了基準: 「実害のある問題 1 ケース: 0」
- [ ] **G-4-1**: `TODO` に I-372 (クロスファイル外部型重複) と I-373 (Fn body IR walk) を追加
- [ ] **G-5-1**: `plan.md` の Batch 11c 行を「I-371 + 12 問題の構造解消」に書き換え。ベースライン表に IR ベース placement 注記
- [ ] **G-6-1**: コミットメッセージ草案を作成し user に提示
- [ ] **G-FINAL**: `tasks.md` を削除（git history 参照）
  - 注: 削除は user による最終 commit 後に実施

---

## Step 4: Review Results

このセクションは Step 1〜3 の自己レビュー結果を記載する。Step 5 着手前に必須。

### 4.1 完備性チェック

| 問題 | tasks.md 内の対応 | 確認 |
|------|------------------|------|
| 1 | B-3-1 | ✓ |
| 2 | D-2-1 | ✓ |
| 3 | A-1-1, A-1-2, A-1-3 | ✓ |
| 4 | E-1-1〜4 | ✓ |
| 5 | B-2-1（OutputWriter）, D-1-1（lib.rs） | ✓ |
| 6 | C-1-1, C-1-2 | ✓ |
| 7 | D-1-1（共通モジュール経由で重複解消） | ✓ |
| 8 | C-2-1, C-2-2, C-2-3 | ✓ |
| 9 | F-1-1〜6 | ✓ |
| 10 | F-2-1, F-2-2 | ✓ |
| 11 | A-2-1 | ✓ |
| 12 | B-1-1〜5 | ✓ |

### 4.2 依存順序チェック

- A-3 (placement) → B-2 (resolve を IR 化): A 完了後に B 着手 ✓
- B-1 (FileOutput API) → C-2-3 (main.rs 更新): C-2-3 で `&fo.items` を参照するため、C-2-1 (FileOutput.items 追加) が先行 ✓
- C-2-1 (FileOutput.items 追加) → D-1-1 (extract_single_output 改修): D は file.items を読むため C-2-1 後 ✓
- E (API 対称化) は A〜D と独立だが、`generate_external_structs_to_fixpoint` の caller を変えるため C 完了後が安全 ✓

### 4.3 コンパイル可能性チェック

各 Phase 末で `cargo check` が通過するか:

- **Phase A 末**: 新しいモジュール追加と既存関数の中身変更のみ。シグネチャ変更なし。`cargo check` 通過想定 ✓
- **Phase B 末**: OutputWriter API が変わるが、main.rs と test を本 Phase 内で同時修正するため Phase 末では通過 ✓
- **Phase C 末**: FileOutput / PerFileTransformed の構造変更。`pipeline::mod` と `main.rs` で同時修正 ✓
- **Phase D 末**: lib.rs の単一ファイル API 内のみ。シグネチャ変更なし ✓
- **Phase E 末**: 関数シグネチャ追加 + caller 全箇所更新を本 Phase 内で完結 ✓

### 4.4 Edge case カバレッジ

- 自己参照 union（再帰型）: F-1-6 でテスト
- inline 配置 0 個のファイル: B-3-1 で `inline_for_file` が空集合 → transitive_shared_refs も空 → 動作問題なし
- shared モジュール衝突 (`shared_types_0.rs`): 既存テスト `test_write_to_directory_shared_synthetic_avoids_collision` で検証済、影響なし
- 単一ファイルに 1 個の合成型のみ: D-1-1 / F-1-3 で検証

### 4.5 テスト影響

- 既存 OutputWriter test 21 件: B-1-3〜5 で API 変更に追従、B-2 / B-3 で内容を IR ベース化に追従。すべて Phase B 内で更新
- 既存 external_struct_generator test 多数: E-1-4 で `&[]` 引数追加、replace_all で一括対応
- snapshot test 6 件: F-2 で個別検証

### 4.6 リスク

| リスク | 対策 |
|--------|------|
| `Item::Trait` の `Method` フィールド構造が想定と異なる | A-2-1 着手時に `src/ir/mod.rs` の `Method` / `TraitRef` 定義を再確認 |
| `SyntheticPlacement.inline` の型変更による外部依存破壊 | grep で外部参照を確認（OutputWriter 内 + main.rs のみのはず） |
| Hono ベンチで dir compile 数値が悪化 | G-2 で必ず確認、悪化したら原因を分析し本スコープで修正 |

### 4.7 結論

レビュー結果: **修正なし。Step 5 (Implementation) に着手可能**。

ただし以下は Step 5 着手時に再確認:
- A-2-1 着手前に `Method` / `TraitRef` のフィールド名を確認
- B-1-2 (`SyntheticPlacement.inline` の型変更) の外部依存を最終確認

---

## Step 5: Implementation

着手時に各タスクのチェックボックスを更新する。
完了 → `- [x]`、進行中 → そのまま、ブロック → コメント追記。

進捗状況: **全 Phase 完了**（A〜G、+ self-review で発見した追加問題の構造解消）。

### Phase 完了サマリ

| Phase | 状態 | テスト数 | クリーン |
|-------|------|---------|---------|
| A | 完了・コミット済 (6564e02) | lib 2012, placement 16, canonical_name 9, type_refs 8 | clippy/fmt OK |
| B | 完了・コミット済 (140285e) | output_writer 22 件全 pass | clippy/fmt OK |
| C | 完了 | PerFileTransformed 外出し + FileOutput.file_synthetic_items 削除 | clippy/fmt OK |
| D | 完了 | extract_single_output / transpile_single を `pipeline::placement::render_referenced_synthetics_for_file` 経由に統一 | clippy/fmt OK |
| E | 完了 | `UndefinedRefScope` 共通骨格抽出、`collect_undefined_type_references` に `defined_only` 追加 | clippy/fmt OK |
| F | 完了 | `tests/pipeline_placement_test.rs` 8 件 + 単体テスト多数 | clippy/fmt OK |
| G | 完了 | quality-check / Hono ベンチ後退ゼロ確認 / TODO 更新 / plan.md 更新 | — |

### Self-review で追加解消した問題（本セッション）

| 課題 | 解消方法 |
|------|---------|
| `pipeline-integrity.md` ルール違反: `RustType::Named { name: "<T as Promise>::Output" }` の文字列詰め込み | `RustType::QSelf { qself, trait_ref, item }` 構造化変数を新設。construction site 2 箇所、generator/substitute/uses_param/walker/synthetic_registry の全 match site を更新 |
| substring scan の単一ファイル API 残存 | `collect_type_refs_from_item` を fn body / impl body / closure / Cast / StructInit / FnCall まで再帰させる完全な IR walker を実装 |
| `TypeParam::constraint` walker 漏れ（correctness バグ） | `collect_type_refs_from_type_params` を新設し、Struct/Enum/Trait/Fn/Impl/TypeAlias 全種別で walking |
| `MatchArm::patterns` walker 漏れ（correctness バグ） | `collect_type_refs_from_match_arm` を新設、`MatchPattern::EnumVariant.path` の uppercase head を抽出 |
| `Stmt::IfLet/WhileLet` `Expr::IfLet/Matches` の verbatim pattern 走査漏れ | `collect_type_refs_from_verbatim_pattern` を新設、パターン文字列先頭の identifier を抽出 |
| impl-block 配置ロジックの単一/マルチファイル間非対称 | `SyntheticReferenceGraph::build` で「synthetic impl の対象 struct を file が定義していれば file を referencer として登録」semantics を組み込み、両 API で共通化 |
| `Item::Fn` が `is_definition_item` から漏れ | 追加（同名 Fn 衝突防止） |
| テスト品質: `test_unreferenced_synthetic_not_emitted` の弱い検証、`test_synthetic_chain` の連鎖でない検証 | 8 件の統合テストに分割・厳密化 |
| `collect_undefined_type_references` / `collect_all_undefined_references` API 非対称 | `UndefinedRefScope` 構造体に共通骨格を抽出 |
| 新規ロジックに対する自動テスト不在 | 単体テスト +72 件 + 統合テスト +8 件 = +80 件追加 |

### 残課題（次セッションへ申し送り）

本セッションで scope 拡大による回帰リスク累積を避けるため、以下 3 件は別バッチ（Batch 11c-fix-2）として分離した。**本来は本セッションで構造解消するべきだった課題**であり、`TODO` に詳細記載済。次セッションで最優先対応する。

| ID | 概要 | 詳細 |
|----|------|------|
| **I-375** | `Expr::FnCall::name` の意味論的多義性（IR 構造化負債） | uppercase head ヒューリスティック / `RUST_BUILTIN_TYPES` への variant constructor ハードコードの workaround を、`enum CallTarget` 構造化で完全解消する |
| **I-376** | クロスファイル外部型 stub の構造的重複 | per-file 生成と post-loop の二重生成を、pipeline 段階で構造的 dedup する |
| **I-377** | walker / substitute / generator の手書き再帰の visitor pattern 化 | `IrVisitor` trait を導入して全再帰を統一 |

これら 3 件の修正方針・影響範囲・テスト戦略は `TODO` に詳細記載済。次セッション開始時に Batch 11c-fix-2 として PRD 化または直接着手する。

---

## 付録: snapshot 個別検証結果

リファクタの結果、全 89 件の integration_test snapshot は **差分なしで pass**。Batch 11c-fix の構造解消は意味論的に snapshot 出力を変えなかった。

| snapshot | 差分の種類 | 妥当性判定 |
|----------|----------|----------|
| basic_types | 差分なし | OK |
| inline_type_literal_param | 差分なし | OK |
| typeof_const | 差分なし | OK |
| instanceof_builtin | 差分なし | OK |
| external_type_struct | 差分なし | OK |
| instanceof_builtin_with_builtins | 差分なし | OK |
| その他 83 件 | 差分なし | OK |
