# R-1: コンポーネント責務境界の正常化

## Background

`report/component-boundary-audit-2026-03-30.md` の監査で以下の構造的問題が発見された:

1. **逆方向依存**: `transformer/any_narrowing.rs` が registry と pipeline の両方から使われる共有ユーティリティであるにもかかわらず transformer に配置されており、`registry/enums.rs` → `transformer/any_narrowing` という正しいパイプライン依存方向（`parser → registry → type_resolver → transformer → generator`）に反する逆方向依存が発生している
2. **any-narrowing enum 登録の二重処理**: `registry/enums.rs` と `pipeline/any_enum_analyzer.rs` の両方で同様の制約収集・enum 登録処理が実行されている。registry 側はスコープ情報なし・TypeRegistry への直接登録、pipeline 側はスコープ付き・SyntheticTypeRegistry 経由
3. **`transformer/types/mod.rs` の re-export 残骸**: 型変換ロジックが `pipeline/type_converter/` に移行済みだが、re-export のみのモジュールが残存。本番コードからの利用は 1 箇所のみ（`classes/members.rs:437`）で、233 個のテストケースが `use super::*;` 経由で依存

## Goal

- パイプラインの依存方向が `parser → registry → pipeline → transformer → generator` で一方向になっている
- any-narrowing の制約収集ユーティリティが pipeline に配置され、registry からの逆方向依存が解消されている
- any-narrowing enum 登録処理が一箇所に統合され、重複がない
- `transformer/types/` の re-export 層が除去され、テストが `pipeline::type_converter` を直接テストしている
- 既存テストが全て通り、Hono ベンチマーク結果が変化しない（純粋な構造リファクタリング）

## Scope

### In Scope

- `transformer/any_narrowing.rs` の `pipeline/` への移動
- `registry/enums.rs` の any-narrowing 処理と `pipeline/any_enum_analyzer.rs` の統合
- `transformer/types/mod.rs` re-export 層の除去とテスト移動
- 移動対象モジュールのユニットテスト追加（テストギャップ解消）
- 影響を受ける import パスの一括更新

### Out of Scope

- registry での型表現決定（`Option<T>` ラップ）の是正 — TypeDef の設計変更が必要であり、独立した PRD として **最優先で** 実施する。TODO に記録
- `I-256`（any-narrowing の typeof 検出範囲拡張）— 機能拡張は構造変更と分離
- 変換ロジックの変更 — 純粋な構造リファクタリングのみ

## Design

### Technical Approach

#### Phase 1: `any_narrowing` モジュールの移動

`src/transformer/any_narrowing.rs` を `src/pipeline/any_narrowing.rs` に移動する。

**移動対象の公開関数**:
- `collect_any_constraints(body, any_param_names) -> HashMap<String, AnyTypeConstraints>`
- `collect_any_constraints_from_expr(expr, any_param_names) -> HashMap<String, AnyTypeConstraints>`
- `collect_any_local_var_names(body) -> Vec<String>`
- `build_any_enum_variants(constraints) -> Vec<EnumVariant>`
- `to_pascal_case(s) -> String`
- `AnyTypeConstraints` struct

**import 更新が必要な箇所**（6 ファイル）:
1. `src/pipeline/any_enum_analyzer.rs:13` — `use crate::transformer::any_narrowing` → `use crate::pipeline::any_narrowing`（ただし同モジュール内になるため `use super::any_narrowing` に変更可能）
2. `src/registry/enums.rs:57-58, 101-102` — `use crate::transformer::any_narrowing::*` → `use crate::pipeline::any_narrowing::*`
3. `src/pipeline/synthetic_registry.rs:364` — `use crate::transformer::any_narrowing::to_pascal_case` → `use super::any_narrowing::to_pascal_case`
4. `src/transformer/functions/mod.rs:28` — `use crate::transformer::any_narrowing::to_pascal_case` → `use crate::pipeline::any_narrowing::to_pascal_case`
5. `src/registry/tests/unions.rs:485` — テスト内の import 更新
6. `src/transformer/mod.rs` — `pub(crate) mod any_narrowing` 宣言の削除

`src/pipeline/mod.rs` に `pub(crate) mod any_narrowing;` を追加。

#### Phase 2: any-narrowing enum 登録の統合

**現状の二重処理**:

| 観点 | `registry/enums.rs` | `pipeline/any_enum_analyzer.rs` |
|------|---------------------|--------------------------------|
| 呼び出し元 | `registry/collection.rs` (Pass 2: 型収集時) | `pipeline/mod.rs` (Pass 2.5: 型収集後) |
| 入力 | TypeDef の `any` パラメータ | AST の `any` 型注釈 |
| 出力先 | TypeRegistry のみ | FileTypeResolution + SyntheticTypeRegistry |
| スコープ情報 | なし | あり（scope_start/scope_end） |
| 重複排除 | なし | SyntheticTypeRegistry 経由 |

**統合方針**: registry 側の処理（`register_any_narrowing_enums`, `register_any_narrowing_enums_from_expr`）を削除し、pipeline 側（`any_enum_analyzer.rs`）に一本化する。

**根拠**:
- pipeline 側はスコープ情報を保持し、SyntheticTypeRegistry で重複排除も行う — 機能的に上位互換
- registry 側で TypeRegistry に登録していた enum は、`pipeline/mod.rs:108-113` の `register_synthetic_structs_in_registry` と同様のパターンで、pipeline 実行後に SyntheticTypeRegistry から TypeRegistry へ転記できる
- `pipeline/mod.rs` の Pass 2.5（any_enum_analyzer 呼び出し）は Pass 2（型収集）の直後に実行されるため、タイミングの問題はない

**具体的変更**:
1. `src/registry/enums.rs` から `register_any_narrowing_enums()` と `register_any_narrowing_enums_from_expr()` を削除
2. `src/registry/collection.rs` からこれらの呼び出し（L234, L254-255, L259-260）を削除
3. `src/pipeline/mod.rs` の any_enum_analyzer 実行後に、生成された any-enum を TypeRegistry にも登録するコードを追加（`register_any_enums_in_registry` 関数）

#### Phase 3: `transformer/types/` re-export 層の除去

1. `src/transformer/classes/members.rs:437` の 1 箇所を `crate::pipeline::type_converter::convert_type_for_position` に変更
2. `src/transformer/types/tests/` 配下の 8 テストファイル（233 テストケース）を `src/pipeline/type_converter/tests/` に移動
   - テスト内容は `pipeline::type_converter` のロジックをテストしているため、テストの配置先として正しい
   - `use super::*;` は `pipeline::type_converter` の公開 API を参照するように自動的に切り替わる
   - IR 型（`RustType`, `Item` 等）と `TypeRegistry` の import は明示的な `use crate::ir::*;` / `use crate::registry::*;` に変更
3. `src/transformer/types/mod.rs` を削除
4. `src/transformer/mod.rs` から `pub(crate) mod types;` 宣言を削除

### Design Integrity Review

- **Higher-level consistency**: パイプラインの依存方向（`parser → registry → pipeline → transformer → generator`）と整合。registry が pipeline の関数を使うのは既に `type_converter` で前例がある（`registry/collection.rs`, `registry/interfaces.rs` 等）
- **DRY**: any-narrowing 処理の二重実装を一本化。知識の重複を排除
- **Orthogonality**: 各モジュールの責務が明確化 — `any_narrowing` は「制約収集ユーティリティ」、`any_enum_analyzer` は「パイプラインパスとしての分析・登録」
- **Coupling**: registry → transformer の結合を除去。registry → pipeline の結合は既存パターンと一致

### Impact Area

| ファイル | 変更種別 |
|---------|---------|
| `src/transformer/any_narrowing.rs` | 削除（移動元） |
| `src/pipeline/any_narrowing.rs` | 新規作成（移動先） |
| `src/pipeline/mod.rs` | mod 宣言追加 + any-enum TypeRegistry 登録追加 |
| `src/pipeline/any_enum_analyzer.rs` | import パス変更 |
| `src/pipeline/synthetic_registry.rs` | import パス変更 |
| `src/registry/enums.rs` | `register_any_narrowing_enums*` 削除 |
| `src/registry/collection.rs` | 呼び出し削除（3 箇所） |
| `src/registry/tests/unions.rs` | import パス変更 + テスト調整 |
| `src/transformer/mod.rs` | mod 宣言削除（any_narrowing, types） |
| `src/transformer/functions/mod.rs` | import パス変更 |
| `src/transformer/classes/members.rs` | import パス変更（1 行） |
| `src/transformer/types/mod.rs` | 削除 |
| `src/transformer/types/tests/*.rs` | `src/pipeline/type_converter/tests/` に移動 |
| `src/pipeline/type_converter/tests/mod.rs` | 移動テストの統合 |

### Semantic Safety Analysis

Not applicable — no type fallback changes. 純粋な構造リファクタリングであり、変換ロジックは変更しない。

## Test Coverage Review

### 影響領域のテストギャップ分析

#### `src/transformer/any_narrowing.rs` — ユニットテスト **完全欠如**

266 行のコードに `#[cfg(test)]` ブロックなし。間接テスト（`any_enum_analyzer.rs` の 6 テスト）のみ。

| Gap | Missing Pattern | Technique | Severity |
|-----|----------------|-----------|----------|
| G1 | `collect_any_constraints`: typeof "boolean" パーティション | 同値分割 | High |
| G2 | `collect_any_constraints`: typeof "object" パーティション | 同値分割 | High |
| G3 | `collect_any_constraints`: typeof "function" パーティション | 同値分割 | High |
| G4 | `collect_any_constraints`: instanceof ブランチ | C1 分岐 | High |
| G5 | `collect_any_constraints`: `!=`/`!==` 演算子（否定チェック） | C1 分岐 | Medium |
| G6 | `collect_any_constraints`: if-else の else ブランチ内の typeof | C1 分岐 | Medium |
| G7 | `collect_any_local_var_names`: any 型ローカル変数検出 | C1 分岐 | High |
| G8 | `collect_any_local_var_names`: 非 any 型変数の除外 | C1 分岐 | Medium |
| G9 | `build_any_enum_variants`: 重複バリアント排除 | 境界値 | Medium |
| G10 | `build_any_enum_variants`: instanceof クラスバリアント | 同値分割 | High |
| G11 | `build_any_enum_variants`: 空の制約（バリアント 0 個） | 境界値 | Medium |
| G12 | `to_pascal_case`: snake_case 入力 | 同値分割 | Low |
| G13 | `to_pascal_case`: camelCase 入力 | 同値分割 | Low |
| G14 | `to_pascal_case`: kebab-case 入力 | 同値分割 | Low |
| G15 | `to_pascal_case`: 空文字列 | 境界値 | Low |
| G16 | `collect_any_constraints_from_expr`: 三項演算子内の typeof | C1 分岐 | Medium |

#### `src/pipeline/any_enum_analyzer.rs` — 既存 6 テスト

| Gap | Missing Pattern | Technique | Severity |
|-----|----------------|-----------|----------|
| G17 | expression-body アロー関数（`(x: any) => typeof x === "string" ? x : 0`） | C1 分岐 | High |
| G18 | クラスメソッドの any パラメータ | AST バリアント | Medium |
| G19 | コンストラクタの any パラメータ | AST バリアント | Medium |
| G20 | any パラメータなし関数（空結果確認） | 境界値 | Low |
| G21 | ネストした関数/アローの再帰走査 | 再帰終了 | Medium |

#### `src/registry/enums.rs` — ユニットテスト **なし**

統合後は削除されるコードだが、統合先（any_enum_analyzer）で同等機能がカバーされていることを確認するテストが必要。

| Gap | Missing Pattern | Technique | Severity |
|-----|----------------|-----------|----------|
| G22 | 統合後: any-enum が TypeRegistry に登録されることの確認 | 統合テスト | High |

#### `src/pipeline/type_converter/tests/` — テスト移動後の整合性

| Gap | Missing Pattern | Technique | Severity |
|-----|----------------|-----------|----------|
| G23 | 移動後の全 233 テストのコンパイル・パス確認 | 回帰 | High |

## Task List

### T1: `any_narrowing.rs` のユニットテスト追加

- **Work**: `src/transformer/any_narrowing.rs` に `#[cfg(test)] mod tests` を追加。G1-G16 のテストギャップを解消する。移動前にテストを書くことで、移動後の挙動保証のベースラインとなる
  - `collect_any_constraints`: typeof "string"/"number"/"boolean"/"object"/"function" の全パーティション（G1-G3）、instanceof（G4）、否定演算子（G5）、else ブランチ（G6）
  - `collect_any_local_var_names`: any 型検出（G7）、非 any 除外（G8）
  - `build_any_enum_variants`: instanceof バリアント（G10）、重複排除（G9）、空制約（G11）
  - `to_pascal_case`: snake_case/camelCase/kebab-case/空文字列（G12-G15）
  - `collect_any_constraints_from_expr`: 三項演算子内 typeof（G16）
- **Completion criteria**: `cargo test -- any_narrowing` で全テスト通過。上記 G1-G16 の全ギャップにテストが存在する
- **Depends on**: None

### T2: `any_enum_analyzer.rs` のテストギャップ解消

- **Work**: `src/pipeline/any_enum_analyzer.rs` の既存テストセクションに G17-G21 のテストを追加
  - expression-body アロー関数のテスト（G17）
  - クラスメソッド・コンストラクタのテスト（G18, G19）
  - any パラメータなし関数のテスト（G20）
  - ネスト関数の再帰走査テスト（G21）
- **Completion criteria**: `cargo test -- any_enum_analyzer` で全テスト通過
- **Depends on**: None

### T3: `any_narrowing.rs` の `pipeline/` への移動

- **Work**:
  1. `src/transformer/any_narrowing.rs` を `src/pipeline/any_narrowing.rs` に移動
  2. `src/pipeline/mod.rs` に `pub(crate) mod any_narrowing;` 追加
  3. `src/transformer/mod.rs` から `pub(crate) mod any_narrowing;` 削除
  4. 全 import パスを更新（6 ファイル）:
     - `src/pipeline/any_enum_analyzer.rs:13`: `use super::any_narrowing;`
     - `src/registry/enums.rs:57-58, 101-102`: `use crate::pipeline::any_narrowing::*`
     - `src/pipeline/synthetic_registry.rs:364`: `use super::any_narrowing::to_pascal_case`
     - `src/transformer/functions/mod.rs:28`: `use crate::pipeline::any_narrowing::to_pascal_case`
     - `src/registry/tests/unions.rs`: import 更新
  5. T1 で追加したユニットテストも一緒に移動される
- **Completion criteria**: `cargo test` 全テスト通過。`grep -r "crate::transformer::any_narrowing" src/` が 0 件
- **Depends on**: T1, T2

### T4: any-narrowing enum 登録の統合

- **Work**:
  1. `src/pipeline/mod.rs` に `register_any_enums_in_registry` 関数を追加 — `SyntheticTypeRegistry` 内の any-enum を `TypeRegistry` にも登録する。パターンは既存の `register_synthetic_structs_in_registry`（`src/pipeline/mod.rs:184-216`）と同一
  2. `src/pipeline/mod.rs` の any_enum_analyzer 実行ループ（L77-87）の直後に、生成された any-enum の TypeRegistry 登録を追加
  3. `src/registry/enums.rs` から `register_any_narrowing_enums()` と `register_any_narrowing_enums_from_expr()` を削除（L46-132）
  4. `src/registry/collection.rs` の呼び出し箇所を削除:
     - L234: `super::enums::register_any_narrowing_enums(reg, &fn_name, &func_def, body);`
     - L254-255: `super::enums::register_any_narrowing_enums(reg, &name, &func_def, body);`
     - L259-260: `super::enums::register_any_narrowing_enums_from_expr(reg, &name, &func_def, expr);`
  5. `src/registry/enums.rs` の `use crate::pipeline::any_narrowing::*` import を削除（不要になる）
  6. G22 のテスト追加: `src/pipeline/any_enum_analyzer.rs` のテストに「any-enum が TypeRegistry にも登録される」ことを検証する統合テストを追加
- **Completion criteria**: `cargo test` 全テスト通過。`grep -r "register_any_narrowing_enums" src/` が 0 件。Hono ベンチマーク結果が変化しない
- **Depends on**: T3

### T5: `transformer/types/` re-export 層の除去

- **Work**:
  1. `src/transformer/types/tests/` 配下の 8 テストファイルを `src/pipeline/type_converter/tests/` に移動:
     - `primitives.rs`, `collections.rs`, `interfaces.rs`, `type_aliases.rs`, `unions.rs`, `intersections.rs`, `structural_transforms.rs` を移動
     - `mod.rs` のヘルパー関数（`parse_interface`, `parse_type_alias`, `parse_type_ann`, `reg_with_point`）を `src/pipeline/type_converter/tests/mod.rs` に統合
  2. 移動したテストの import を更新:
     - `use super::*;` は `pipeline::type_converter` の公開 API を指すようになる
     - IR 型は `use crate::ir::*;` に変更
     - `TypeRegistry` は `use crate::registry::TypeRegistry;` に変更
     - `SyntheticTypeRegistry` は `use crate::pipeline::SyntheticTypeRegistry;` に変更
     - SWC 型は `use swc_ecma_ast::*;` に変更
  3. `src/transformer/classes/members.rs:437` を `crate::pipeline::type_converter::convert_type_for_position` に変更
  4. `src/transformer/types/mod.rs` を削除
  5. `src/transformer/mod.rs` から `pub(crate) mod types;` を削除
- **Completion criteria**: `cargo test` 全テスト通過（移動した 233 テスト含む）。`src/transformer/types/` ディレクトリが存在しない。G23 が解消
- **Depends on**: None（T1-T4 と独立して実施可能）

### T6: 回帰検証とクリーンアップ

- **Work**:
  1. `cargo clippy --all-targets --all-features -- -D warnings` が 0 warnings
  2. `cargo fmt --all --check` が pass
  3. `cargo test` が全テスト通過
  4. Hono ベンチマーク（`./scripts/hono-bench.sh`）を実行し、結果が変化しないことを確認
  5. `grep -r "crate::transformer::any_narrowing\|crate::transformer::types::" src/` が 0 件であることを確認
  6. 不要になった `use` 文や dead code の `cargo fix --allow-dirty --allow-staged` による自動削除
- **Completion criteria**: 上記 6 項目全てクリア
- **Depends on**: T3, T4, T5

## Test Plan

### 新規テスト（T1: any_narrowing ユニットテスト）

- typeof 全パーティション（string/number/boolean/object/function）の制約収集
- instanceof の制約収集
- 否定演算子（`!=`, `!==`）の制約収集
- else ブランチ内の typeof 検出
- any 型ローカル変数名の収集
- 重複バリアント排除
- instanceof クラスバリアント生成
- 空制約の処理
- `to_pascal_case` の全パーティション + 境界値
- expression-body の制約収集

### 新規テスト（T2: any_enum_analyzer 拡張）

- expression-body アロー関数
- クラスメソッド・コンストラクタ
- any パラメータなし関数
- ネスト関数の再帰走査

### 統合テスト（T4: 統合検証）

- pipeline 経由で any-enum が TypeRegistry に登録されることの確認

### 回帰テスト（T5, T6）

- 移動した 233 テストケースの全通過
- Hono ベンチマーク結果の不変確認

## Completion Criteria

- [ ] `cargo test` 全テスト通過
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` が 0 warnings
- [ ] `cargo fmt --all --check` が pass
- [ ] Hono ベンチマーク結果が変化しない（クリーン率 69.6%、61 エラーインスタンス）
- [ ] `grep -r "crate::transformer::any_narrowing" src/` が 0 件
- [ ] `grep -r "crate::transformer::types::" src/` が 0 件
- [ ] `src/transformer/types/` ディレクトリが存在しない
- [ ] `src/registry/enums.rs` に `register_any_narrowing_enums` が存在しない
- [ ] テストギャップ G1-G23 の全てにテストが存在する
