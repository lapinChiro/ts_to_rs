# I-371: 合成型の単一正準配置（Single Canonical Placement）

## 背景

ts_to_rs は TypeScript の構造的型（特に union, intersection, type literal）を Rust の名前的型に変換するため、`SyntheticTypeRegistry` で合成型（synthetic type）を管理している。同一の構造を持つ合成型は dedup により同じ名前を共有する。

しかし現在の実装では、合成型の **定義** が複数の Rust モジュールに同時に存在しうるため、Rust の名前的型システム上で別の型として扱われ、以下の問題が発生する。

## 問題

### 問題 1: 同一ファイル内重複定義（コンパイルエラー）

File A の transformer が合成型 X を生成すると、X は以下の 2 経路で File A の出力に到達する:

1. **直接経路**: `file_synthetic_items` → `all_items` → `generate(&all_items)` → `rust_source` に定義として埋め込まれる
2. **OutputWriter 経路**: `synthetic.merge(file_synthetic)` で shared プールに入り、`resolve_synthetic_placement` が「X が File A から参照されている」と判定して File A にインライン

結果として File A に X が **2 回定義** され、E0428（重複定義）+ 連鎖的に E0119（trait 実装重複）が発生する。

**実測**: Hono `types.rs` で 5 件の E0428 + 12 件の E0119 = 17 エラー（dir compile 158/158 のゲートの一つ）。

### 問題 2: クロスファイル冗長性（正準性違反）

File A と File B が独立に同じ構造の合成型（例: `string | number` → `StringOrF64`）を生成すると:

- File A の `rust_source` に `pub enum StringOrF64`
- File B の `rust_source` にも `pub enum StringOrF64`
- `shared_types.rs` にも `pub enum StringOrF64`（OutputWriter が 2+ ファイル参照と判定）

→ 構造的に同一の型が **3 つの Rust path** に独立して定義される（`crate::a::StringOrF64`, `crate::b::StringOrF64`, `crate::shared_types::StringOrF64`）。

Rust の名前的型システムでは、これらは **すべて異なる型**。File A の関数が `crate::a::StringOrF64` を返し、File B の関数が `crate::b::StringOrF64` を引数に取る場合、互換性のない型として扱われる。

現状は各ファイルが自身のローカル定義を使うため即座のコンパイルエラーには至らないが、ファイル境界をまたぐ値の受け渡しで型エラーが発生する潜在リスクがある。

### 問題 3: shared_types.rs からのインポート欠如

OutputWriter が `shared_types.rs` に配置した合成型を、参照ファイルは `use crate::shared_types::X;` でインポートしない。問題 2 によりローカル定義が存在するため動作しているが、問題 2 を解消すると即座に未解決参照エラーになる。

## 根本原因

**合成型の「正準配置（canonical location）」がアーキテクチャ上で定義されていない。**

現在のパイプラインでは:

- 合成型はトランスフォーマーが `file_synthetic` に登録し、同時に `file_synthetic_items` として `rust_source` に埋め込まれる
- 同じ合成型が `synthetic.merge()` で shared プールにも入る
- OutputWriter は shared プールから配置を決定するが、`rust_source` に既に存在することを認識しない

合成型の「定義の場所」が **トランスフォーマー** と **OutputWriter** の 2 箇所で独立に決定される。両者の判断が衝突した結果が問題 1, 2 である。

## 設計

### 原則: 単一正準配置（Single Canonical Placement）

各合成型は出力 crate 全体で **唯一の Rust モジュール** に定義され、その型を参照するすべてのコードは同じ Rust path（`crate::module::Type`）を指す。

### 配置ルール

合成型 X の参照ファイル数（X の名前を field 型・関数 signature 等で参照するファイル数）に基づき:

| 参照数 | 配置先 | 参照側の対応 |
|--------|--------|-------------|
| 0 | 配置しない | — |
| 1 | そのファイルにインライン | ローカル定義として参照 |
| 2+ | `shared_types.rs` | `use crate::shared_types::X;` でインポート |

合成型 X は **どのケースでも 1 箇所のみ** に定義される。

### アーキテクチャ変更

#### 変更 1: トランスフォーマーは合成型を `rust_source` に埋め込まない

`pipeline/mod.rs` の per-file loop で `file_synthetic_items` を `all_items` に追加しない。`rust_source` には **ユーザー定義型 + 外部型 struct のみ** を含める。

合成型は `synthetic` プールに登録されるのみで、最終配置は OutputWriter が決定する。

```rust
// 変更前
let mut all_items = file_synthetic_items;  // ← 削除
all_items.extend(items);
let rust_source = generate(&all_items);

// 変更後
let mut all_items = items;
// 外部型 struct 生成は file_synthetic_items を「scanning context」として参照
// （rust_source には含めないが、外部型参照の検出には必要）
generate_external_structs_to_fixpoint_with_scan_context(
    &mut all_items,
    &shared_registry,
    &synthetic,
    &file_synthetic_items,  // 参照検出のための scan のみ
);
let rust_source = generate(&all_items);
```

#### 変更 2: OutputWriter が全ての合成型を配置

`resolve_synthetic_placement` のロジックは現状維持（参照ファイル数で配置を決定）。`rust_source` に定義が含まれなくなったため、参照検出は **使用箇所** のみを検出する（定義との混同が解消）。

#### 変更 3: shared_types.rs からのインポート生成

OutputWriter は配置決定後、`shared_types.rs` に配置された合成型を参照するファイルに対して、ファイル先頭に `use crate::shared_types::{X, Y, ...};` を追加する。

```rust
// write_to_directory 内
for (rel_path, source) in file_outputs {
    let mut content = String::new();

    // インライン合成型
    if let Some(inline_items) = placement.inline.get(rel_path) {
        for item_code in inline_items { content.push_str(item_code); content.push_str("\n\n"); }
    }

    // shared_types からのインポート
    if let Some(imports) = placement.shared_imports.get(rel_path) {
        for import in imports { content.push_str(import); content.push_str("\n"); }
        if !imports.is_empty() { content.push_str("\n"); }
    }

    content.push_str(source);
    std::fs::write(&out_path, &content)?;
}
```

`SyntheticPlacement` 構造体に `shared_imports: HashMap<PathBuf, Vec<String>>` を追加する。

#### 変更 4: 単一ファイル API の対応

`transpile_single` および `transformer::transform_module` は OutputWriter を経由しないため、合成型を結合する処理を追加する:

```rust
pub fn transpile_single(source: &str) -> Result<String> {
    let output = transpile_pipeline(input)?;
    let file = output.files.into_iter().next().unwrap_or_default();
    // 単一ファイルモードでは全合成型を file 先頭に結合
    let synthetic_code = crate::generator::generate(&output.synthetic_items);
    if synthetic_code.is_empty() {
        Ok(file.rust_source)
    } else {
        Ok(format!("{}\n\n{}", synthetic_code, file.rust_source))
    }
}
```

`transformer::transform_module`（公開 API、テスト用）は内部で `synthetic.into_items() + items` を返す現在の動作を維持する（テストの利便性のため）。これはパイプラインの per-file loop の動作とは独立。

#### 変更 5: 外部型 struct 生成の scan context 拡張

`generate_external_structs_to_fixpoint` は `items` を走査して未定義型参照を検出する。`file_synthetic_items` が `items` に含まれなくなるため、合成型から参照される外部型を検出できなくなる。

解決: 走査時に `file_synthetic_items` も含める（出力には含めない）。

```rust
fn generate_external_structs_to_fixpoint(
    output_items: &mut Vec<Item>,        // 出力対象（更新される）
    scan_context: &[Item],               // 走査のみ（更新されない）
    registry: &TypeRegistry,
    synthetic: &SyntheticTypeRegistry,
) {
    for _ in 0..MAX_ITERATIONS {
        // output_items + scan_context から未定義参照を収集
        let undefined_refs = collect_undefined_refs_from_both(output_items, scan_context, registry);
        if undefined_refs.is_empty() { break; }
        // 新規外部型を output_items に追加
        for type_name in &sorted(undefined_refs) {
            if let Some(item) = generate_external_struct(type_name, registry, synthetic) {
                output_items.push(item);
            }
        }
    }
}
```

### 設計レビュー

- **凝集度**: 合成型の配置決定が OutputWriter に集約される。トランスフォーマーは「合成型を生成して registry に登録する」責務のみ。配置は OutputWriter の責務。明確な責務分離
- **責務分離**: トランスフォーマー（生成）、SyntheticTypeRegistry（保管・dedup）、OutputWriter（配置）、Generator（コード生成）の 4 つの責務が完全に分離される
- **DRY**: 合成型は **唯一の場所** に定義される。重複なし
- **正準性**: 各合成型は出力 crate 全体で 1 つの Rust path を持つ。Rust の名前的型システムと整合

### 意味論的安全性分析

本変更は合成型の **配置場所** を変えるが、**生成内容** は変えない。各変更を分類:

| 変更点 | 安全性 |
|--------|--------|
| `file_synthetic_items` を `rust_source` から除外 | Safe — 同じ型定義が OutputWriter により別の場所（インラインまたは shared）に配置される。最終出力に同じ型が同じ構造で存在することは保証される |
| 外部型 scan に `file_synthetic_items` を含める | Safe — 走査範囲の拡張のみ。検出対象が増え、必要な外部型 struct が漏れなく生成される |
| `shared_types.rs` 用 `use` 文の生成 | Safe — Rust では `use` 文の追加は型解決の補助のみ。値の意味は変わらない |
| `transpile_single` で synthetic を明示的に結合 | Safe — 現状の出力と等価（合成型がファイル先頭に配置される） |

**Tier 1（silent semantic change）リスクなし**: 合成型の生成内容、フィールド型、enum バリアントは一切変更しない。配置先のみが変わる。

## タスク

### Phase 1: パイプライン変更

1. `pipeline/mod.rs` の per-file loop で `file_synthetic_items` を `all_items` から除外
2. `generate_external_structs_to_fixpoint` のシグネチャ変更（scan_context パラメータ追加）
3. `generate_stub_structs` のシグネチャ変更（scan_context パラメータ追加）
4. `collect_undefined_type_references` および `collect_all_undefined_references` の scan_context 対応
5. パイプライン後段（共有 synthetic items 生成）の対応

### Phase 2: OutputWriter 変更

6. `SyntheticPlacement` に `shared_imports: HashMap<PathBuf, Vec<String>>` フィールド追加
7. `resolve_synthetic_placement` で shared module に配置された型を参照するファイルを検出し、`shared_imports` を構築
8. `write_to_directory` で `shared_imports` をファイル先頭に書き出し
9. `pub mod shared_types;` 生成は既存ロジックを継続使用

### Phase 3: 単一ファイル API 対応

10. `transpile_single` で `output.synthetic_items` を `rust_source` の先頭に結合する処理を追加
11. `transformer::transform_module`（公開 API）は現状維持（テスト互換性のため）

### Phase 4: テスト

12. パイプラインテスト: 同一合成型が出力中に 1 回のみ定義されることを検証
13. パイプラインテスト: shared_types.rs に配置された型を参照するファイルに `use` 文が生成されることを検証
14. クロスファイル合成型のテスト: 2 ファイルが同じ union を使う場合、shared_types.rs に 1 つだけ定義され、両ファイルが import することを検証
15. `transpile_single` のテスト: 単一ファイルモードで合成型がファイル先頭に含まれることを検証
16. `OutputWriter` テスト: `inline` placement と shared module placement の両方で重複が発生しないことを検証
17. 外部型 scan テスト: 合成型から参照される外部型が `scan_context` 経由で検出されることを検証

### Phase 5: 検証

18. Hono ベンチマーク実行
19. types.rs の E0428（重複定義）+ E0119（trait 重複）解消確認
20. 既存テスト全 pass
21. dir compile 改善確認（157 → ?）

## 完了条件

1. 各合成型が出力 crate 全体で **唯一の Rust モジュール** に定義される
2. 同一ファイル内重複定義（E0428）が発生しない
3. クロスファイル冗長定義が発生しない（同じ構造の型が複数ファイルに重複しない）
4. `shared_types.rs` に配置された型を参照するファイルが `use` 文でインポートする
5. Hono types.rs の E0428 + E0119（合計 17 エラー）が解消
6. 既存テスト全 pass
7. 新規テスト（合計 6 件以上）追加

## スコープ外

- **E0405 (`Input['out']`)**: I-370 で別途追跡。indexed access の解決問題で本 PRD とは独立
- **E0107（generic 引数不一致）**: 型引数推論の問題。RC-9 関連で別途追跡
- **E0072（再帰型 infinite size）**: Box ラッピング判定の問題で別途追跡
- **E0432（unresolved imports）**: クロスファイルインポート解決の問題で別途追跡

## 関連

- I-368（OutputWriter `types.rs` 衝突）: Batch 11a で完了。本 PRD はその次の段階
- I-369（ビルトイン型モノモーフィゼーション）: Batch 11a で完了
- `pipeline/mod.rs` の per-file loop: `src/pipeline/mod.rs:121-171`
- `resolve_synthetic_placement`: `src/pipeline/output_writer.rs:83-155`
- `write_to_directory`: `src/pipeline/output_writer.rs:159-237`
- `transpile_single`: `src/pipeline/mod.rs:273-287`
- `Transformer::transform_module`（公開 API）: `src/transformer/mod.rs:198-209`
- `generate_external_structs_to_fixpoint`: `src/pipeline/mod.rs:241-262`
- `SyntheticTypeRegistry::merge`: `src/pipeline/synthetic_registry/mod.rs:390`
