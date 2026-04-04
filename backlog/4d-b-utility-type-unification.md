# Batch 4d-B: ユーティリティ型変換の統一

## Background

ユーティリティ型（Partial, Required, Pick, Omit, NonNullable）の変換ロジックが `type_converter/utilities.rs` と `resolve/utility.rs` に二重実装されている。

現在の実行パスは2系統ある:
- **通常パス**: `convert_ts_type()` → `resolve_ts_type()` → `resolve_type_ref()` → `resolve_partial()` 等（resolve 経由）
- **バイパスパス**: `intersections.rs`/`unions.rs` の SWC AST 直接処理 → `convert_type_ref()` → `convert_utility_partial()` 等（type_converter 経由）

バイパスパスの原因は、intersection/union の declaration 変換が SWC AST を直接操作しており、型参照を `convert_type_ref()` で処理するためである。`convert_type_ref()` 内部の Array/Record/Readonly 処理は `convert_ts_type()` に委譲しているが、ユーティリティ型は `convert_utility_*()` を直接呼び出している。

## Goal

1. `convert_type_ref()` を廃止し、呼び出し元を `convert_ts_type()` 経由に統一する
2. `type_converter/utilities.rs` からユーティリティ型変換関数（5関数）とそのヘルパー（4関数）を削除する
3. utilities.rs に残る関数（`convert_property_signature`, `extract_type_params`, `convert_unsupported_union_member`, `convert_fn_type_to_rust`）は維持する
4. 全テストパス、clippy 0 warnings、ベンチマーク変化なし

## Scope

### In Scope

- `convert_type_ref()` の廃止（呼び出し元を `convert_ts_type()` に置換）
- `convert_utility_partial/required/pick/omit/non_nullable` の削除
- `resolve_utility_inner_fields`, `resolve_utility_inner_with_conversion`, `extract_string_keys`, `capitalize_first` の削除
- 呼び出し元（intersections.rs:209,292、unions.rs:388）の修正

### Out of Scope

- `convert_property_signature`, `extract_type_params` の移動（PRD-C で対応）
- intersection/union の declaration 変換そのものの移行（PRD-C で対応）
- 機能追加

## Design

### Technical Approach

**核心**: `convert_type_ref()` の3箇所の呼び出し元は、SWC `TsTypeRef` を持っている。これを `TsType::TsTypeRef(type_ref.clone())` にラップして `convert_ts_type()` に渡せば、resolve パイプライン経由で統一的に処理される。

**T1: convert_type_ref 呼び出し元の修正**

`intersections.rs:209`, `intersections.rs:292`, `unions.rs:388` の3箇所を以下のように変更:

```rust
// Before:
let rust_type = convert_type_ref(type_ref, synthetic, reg)?;

// After:
let rust_type = convert_ts_type(
    &TsType::TsTypeRef(type_ref.clone()),
    synthetic,
    reg,
)?;
```

`TsTypeRef` は SWC AST の `Box<TsTypeRef>` ではなく直接参照されている箇所もあるため、各呼び出しサイトの型を確認して適切に変換する。

**T2: convert_type_ref の削除**

呼び出し元の修正後、`convert_type_ref()` を `mod.rs` から削除する。

**T3: ユーティリティ変換関数の削除**

`utilities.rs` から以下を削除:
- `convert_utility_partial` (行152-197)
- `convert_utility_required` (行200-243)
- `convert_utility_pick` (行264-310)
- `convert_utility_omit` (行313-359)
- `convert_utility_non_nullable` (行362-381)
- `resolve_utility_inner_fields` (行67-107)
- `resolve_utility_inner_with_conversion` (行110-149)
- `extract_string_keys` (行245-261)
- `capitalize_first` (行384-389)

残す関数:
- `convert_property_signature` — intersection/interface の field 変換で使用中
- `extract_type_params` — interface/type_alias の型パラメータ抽出で使用中
- `convert_unsupported_union_member` — union の fallback variant 生成で使用中
- `convert_fn_type_to_rust` — union の関数型 variant 変換で使用中

### vis フィールドの差異

type_converter utilities は `vis: None` で StructField を生成し、resolve utilities は `vis: Some(Visibility::Public)` で生成する。resolve に統一することで `Some(Visibility::Public)` に統一される。この差異が最終出力に影響するか確認が必要。

**確認方法**: ベンチマークの前後比較。変換結果のスナップショットテスト差分を確認。

### Design Integrity Review

- **Higher-level consistency**: convert_ts_type → resolve_ts_type の統一パスに合わせる変更。パイプライン設計に沿う
- **DRY**: 5関数 + 4ヘルパーの重複を完全に解消
- **Coupling**: type_converter → resolve の一方向依存が強化される（適切な方向）
- **Broken windows**: `convert_type_ref` 内の Array/Record/Readonly 処理も resolve_type_ref と重複しているが、convert_type_ref 自体を廃止するため解消される

### Impact Area

- `src/pipeline/type_converter/mod.rs` — convert_type_ref 削除
- `src/pipeline/type_converter/utilities.rs` — 9関数削除
- `src/pipeline/type_converter/intersections.rs` — 2箇所の呼び出し修正
- `src/pipeline/type_converter/unions.rs` — 1箇所の呼び出し修正
- `src/pipeline/type_converter/tests/` — 削除された関数のテストの整理

### Semantic Safety Analysis

ユーティリティ型の変換先が type_converter → resolve に変わるが、resolve の実装は type_converter と同一アルゴリズム（調査済み）。差異は vis フィールドのみ。ベンチマーク前後比較で安全性を確認する。

## Task List

### T1: convert_type_ref 呼び出し元の修正

- **Work**:
  - `src/pipeline/type_converter/intersections.rs:209` の `convert_type_ref(type_ref, synthetic, reg)` を `convert_ts_type(&TsType::TsTypeRef(type_ref.clone()), synthetic, reg)` に変更
  - `src/pipeline/type_converter/intersections.rs:292` — 同上
  - `src/pipeline/type_converter/unions.rs:388` — 同上
  - 各箇所で `type_ref` の型（参照 vs 値）を確認し、適切にラップ
- **Completion criteria**: `cargo check` パス
- **Depends on**: PRD-A 完了

### T2: convert_type_ref の削除

- **Work**: `src/pipeline/type_converter/mod.rs` から `convert_type_ref` 関数を削除。不要になる import も削除
- **Completion criteria**: `cargo check` パス
- **Depends on**: T1

### T3: ユーティリティ変換関数・ヘルパーの削除

- **Work**:
  - `src/pipeline/type_converter/utilities.rs` から 9 関数を削除（上記 Design 参照）
  - 不要になる import を削除
  - ファイル冒頭のモジュールドキュメントを更新（残る関数の責務に合わせる）
- **Completion criteria**: `cargo check` パス
- **Depends on**: T2

### T4: テスト整理

- **Work**:
  - 削除された関数を直接テストしていたテストケースを削除（resolve/utility.rs のテストでカバーされているため）
  - 残るテスト（convert_property_signature, extract_type_params 等）は維持
  - `cargo test` 全パス確認
- **Completion criteria**: `cargo test` 全パス、削除された関数を参照するテストが残っていないこと
- **Depends on**: T3

### T5: vis フィールド差異の確認とベンチマーク

- **Work**:
  - Hono ベンチマーク実行（`./scripts/hono-bench.sh`）
  - 変換率に変化がないことを確認（110/158 維持）
  - スナップショットテスト（`cargo insta test`）で差分を確認
  - vis 差異による出力変化がある場合、resolve/utility.rs の vis を調整
- **Completion criteria**: ベンチマーク結果が変化なし、スナップショットが一致（または意図的な改善のみ）
- **Depends on**: T4

### T6: 品質確認

- **Work**: `cargo fix && cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test`
- **Completion criteria**: 0 errors, 0 warnings, 全テストパス
- **Depends on**: T5

## Test Plan

- 既存の resolve/utility.rs テスト（14件）がユーティリティ型変換をカバー
- 既存の type_converter テスト（convert_property_signature 等）は維持
- 削除される関数のテストは resolve 側のテストで代替されていることを確認
- ベンチマークで回帰がないことを確認

## Completion Criteria

1. `cargo test` 全テストパス
2. `cargo clippy --all-targets --all-features -- -D warnings` 0 warnings
3. `cargo fmt --all --check` パス
4. `convert_type_ref` が type_converter/mod.rs に存在しない
5. `convert_utility_*` が type_converter/utilities.rs に存在しない
6. `grep -rn "convert_utility_" src/` がテスト以外でヒット 0 件
7. Hono ベンチマーク: 110/158 クリーン維持（±0）
