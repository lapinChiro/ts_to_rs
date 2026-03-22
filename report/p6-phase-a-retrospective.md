# P6 Phase A 振り返りレポート

**基準コミット**: ec981cc（未コミットの変更多数あり。Phase A のシグネチャ変更 + Step 6 の Generator 移動が未コミット状態）

## 要約

P6 の Phase A（108 関数への `tctx: &TransformContext` パラメータ追加）において、3 回のリセット（`git checkout`）を経て最終的に成功した。サブエージェント委任 1 回、スクリプト実行 3 回（うち 2 回失敗）。主な問題は、(1) 中間コミットの欠如、(2) 一括置換スクリプトの検証不足、(3) 変換対象外関数の洗い出し漏れの 3 点。

## 時系列と各試行の分析

### 試行 0: サブエージェント委任（失敗）

- **結果**: 49 ファイル変更、+3,711/-12,547 行。テストコード大量削除、types/mod.rs に 2,522 行追加
- **根本原因**:
  - サブエージェントへの指示が「機械的な変更」と記述したが、エージェントが独自判断でスコープ外の変更を実施
  - 成果物をマージ前に検証しなかった
- **教訓**: サブエージェントの成果物は必ず `git diff --stat` で変更規模を確認し、スコープ外の変更がないか検証する。テストコードの削除は特に危険信号

### 試行 1: v2 スクリプト（失敗 → リセット）

- **結果**: 180 エラー → 手動修正で 93 エラーまで減少 → リセット
- **問題点**:
  1. **短すぎるパターン `, reg,`** による誤マッチ（`convert_ts_type`, `convert_type_for_position` 等の非対象関数にも tctx が追加された）
  2. **import 挿入位置の誤り**: ファイル内の最後の `use` 文を探すロジックが、テストモジュール内の `use` に引っかかった（`classes.rs:1184`, `statements/mod.rs:2665`）
  3. **公開 API の変更**: `transform_module`, `transform_module_collecting` にも tctx パラメータが追加された（これらは内部でデフォルト tctx を生成すべき）
  4. **multi-line regex の暴走**: `DOTALL` 相当の動作で tctx が複数回挿入された
- **根本原因**: dry run で変更数のサマリーのみ確認し、実際の差分（特にエッジケース）を目視確認しなかった

### 試行 2: v2 スクリプト + reg 削除スクリプト（失敗 → リセット）

- **結果**: 234 エラー
- **問題点**:
  1. **`let reg = tctx.type_registry;` の挿入位置が不正**: 行末が `{` なら関数本体と判定するロジックが、`match module_item {` や `match pat {` 内にも挿入
  2. **`convert_ts_type` 等の非 tctx 関数からも `reg` が削除**: `tctx, reg,` → `tctx,` の置換が非対象関数にも適用
- **根本原因**: 正規表現では Rust の構文（関数本体の開始位置 vs match 文の開始）を区別できない。`reg` パラメータ削除という構文レベルの変更にはパーサが必要

### 試行 3: v4 スクリプト + multi-line fix（成功）

- **結果**: 0 エラー
- **アプローチ**:
  1. v4 スクリプト: single-line パターンのみ（multi-line regex を排除）
  2. 別スクリプト: `reg,` 行の前に `tctx,` 行を挿入（standalone `reg,` 行のみ対象）
  3. 手動修正: 15 箇所（`convert_ident_to_param` の tctx 除去、`if_let_pattern` への tctx 追加、`lib.rs` の tctx 生成、`convert_ts_type_with_fallback` への tctx 追加、`generate_if_let` への tctx 追加等）
- **成功要因**: スクリプトの責務を「安全に自動化できるパターンのみ」に限定し、残りは手動で対応

## git checkout による巻き戻しの問題

### 発生したこと

`git checkout -- src/transformer/` で Phase A の失敗した変更をリセットするたびに、**Step 6 の変更（Generator 移動）も一緒にリセット**された。これにより、3 回のリセットそれぞれで以下の 5 箇所を手動復元する必要が生じた:

1. `src/transformer/mod.rs`: `pub mod context;` 宣言
2. `src/transformer/mod.rs`: `transform_module_with_context` 関数
3. `src/transformer/mod.rs`: `inject_regex_import_if_needed` とヘルパー関数
4. `src/transformer/statements/mod.rs`: match 文の `.as_str()` ラッピング
5. `src/lib.rs`: デフォルト tctx 生成

### 根本原因

Step 6 完了時点でコミットしていなかったため、`git checkout` が Step 6 の変更と Phase A の変更を区別できなかった。

### 教訓

**段階的にコミットすべきだった**。具体的には:
- Step 6（Generator 移動）完了時にコミット
- Phase A（シグネチャ変更）完了時にコミット
- Phase B（テスト修正）完了時にコミット

コミットしていれば、`git checkout -- src/transformer/` ではなく `git stash` や `git diff` で特定の変更のみ取り消せた。

## 一括置換スクリプトに関する教訓

### やってはいけないこと

1. **短い汎用パターンでの一括置換**: `, reg,` → `, tctx, reg,` のようなパターンは、関数呼び出し以外のコンテキスト（`let (a, reg, b) = ...` 等）にもマッチする可能性がある
2. **multi-line regex の使用**: 行をまたぐパターンマッチは予期しない複数マッチや無限挿入を引き起こす
3. **Rust 構文レベルの変更を正規表現で行うこと**: 関数本体の開始位置の特定、`match` 文とブロックの区別等は正規表現では信頼性が低い
4. **dry run で変更数のみ確認**: 実際の差分（特にエッジケース）を目視確認しなければ、誤変換を検出できない

### やるべきこと

1. **関数名を含む具体的なパターン**で置換する（例: `convert_expr(` を含む行内で `, reg,` を置換）
2. **single-line パターンのみ**をスクリプトで処理し、multi-line は手動対応
3. **dry run で代表的な 3-5 件の差分を目視確認**してから適用
4. **除外リストを事前に網羅的に作成**: `grep` で全シグネチャパターンを確認し、非対象関数を特定
5. **`reg: &TypeRegistry` と `reg: &crate::registry::TypeRegistry`** のような表記揺れを事前に把握

## Phase B への適用

Phase B（テストファイル修正、462 箇所）で同じ失敗を繰り返さないために:

1. **Phase A の変更をコミットしてからPhase B に着手する**: リセットが必要になっても Phase A の変更が保全される
2. **テストファイル用のスクリプトは Phase A の成功パターン（v4 + multi-line fix）を踏襲**
3. **テスト固有のパターン**:
   - テストでは `&TypeRegistry::new()` が `reg` 変数の代わりに使われることが多い → `&TypeRegistry::new()` の前にも tctx を追加する必要がある
   - テスト用の `TctxFixture` ヘルパーを各テストモジュールに定義する
4. **1 テストファイルずつ処理し、各ファイル完了後に `cargo test --lib <module>` で検証**

## 既存ルール・スキルへの反映提案

1. **`bulk-edit-safety.md`** に以下を追記:
   - 「multi-line regex を使用しない。行をまたぐパターンは手動対応」
   - 「除外リストは grep でシグネチャの表記揺れを確認して作成する」

2. **`large-scale-refactor` スキル** に以下を追記:
   - 「Step 5（実装）の途中で安定した状態に到達したら中間コミットを作成する。コミットメッセージには [WIP] を付ける」
   - 「リセット（git checkout）を行う前に、保全すべき変更がコミット済みか確認する」

3. **新規ルール提案**: 「段階的コミットの原則」
   - 大規模変更では、論理的に独立した単位（Step 6, Phase A, Phase B 等）の完了時にコミットする
   - コミットしていない状態で `git checkout` / `git stash` を使わない
