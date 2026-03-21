---
name: correctness-audit
description: 変換ロジックとテストの論理的正当性を徹底的に監査し、問題を PRD 化する。定期的な品質ゲートとして使用
user-invocable: true
---

# 変換正当性の監査

## トリガー

- ユーザーから正当性チェック・監査を依頼されたとき
- 大きな機能追加サイクル（5 件以上の PRD 消化）が完了したとき

## アクション

以下の 3 つの調査を**並列**で実施し、結果を `report/` にレポートとして保存する。

### 1. 型変換の正確性監査

対象ファイル: `src/transformer/types/`, `src/ir.rs`, `src/generator/types.rs`

全ての型マッピングについて以下を検証する:

- **型の等価性**: TS の型が Rust の型に正しく対応しているか。情報が欠落・追加されていないか
- **コンパイル可能性**: 生成される Rust コードが rustc でコンパイルできるか
- **エッジケース**: 型の組み合わせ（union 内の特殊型、ネストしたジェネリクス等）で壊れないか

具体的なチェック項目:
- 各 `TsKeywordTypeKind` → `RustType` マッピングの妥当性
- union/intersection の全パターン（nullable, 非 nullable, 複合）
- ジェネリクス・型パラメータの伝搬
- 型注記位置 vs type alias 位置での挙動差異

### 2. 文・式のセマンティクス監査

対象ファイル: `src/transformer/statements/`, `src/transformer/expressions/`, `src/transformer/functions/`, `src/transformer/classes.rs`, `src/generator/statements.rs`, `src/generator/expressions.rs`

全ての変換パターンについて以下を検証する:

- **制御フローの保持**: break/continue/return/throw が正しいスコープで動作するか
- **式の型安全性**: 生成される式がコンパイル可能か（型の不一致、メソッドの有無）
- **ランタイム挙動の等価性**: パニック vs NaN、ミュータビリティ、所有権等の差異
- **エッジケース**: ネストした構造、複合パターン、暗黙の型変換

### 3. テスト品質の監査

対象ファイル: `src/**/tests.rs`, `tests/integration_test.rs`, `tests/compile_test.rs`, `tests/snapshots/`

全テストについて以下を検証する:

- **期待値の正確性**: 期待される Rust コードは実際にコンパイル可能か。セマンティクスは正しいか
- **アサーションの強度**: `matches!()` や `is_ok()` のみで内容を検証していないテストはないか
- **欠落テスト**: 各変換パターンに対して正常系・異常系・境界値のテストがあるか
- **スナップショットの正当性**: compile_test でスキップされているスナップショットのコードは正しいか
- **テストが本来テストすべきことをテストしているか**: テスト名と実際の検証内容が一致しているか

### 4. レポート作成

調査結果を `report/conversion-correctness-audit.md` に保存する。レポートには:

- 基準コミットを記載する
- 問題を深刻度別（Critical / High / Medium）に分類する
- 各問題に ID を付与し、具体的なコード箇所（ファイル:行番号）を記載する
- 前回の監査結果がある場合、前回からの変化（解消・新規・変化なし）を記録する

### 5. PRD 化

発見した全ての Critical / High の問題について:

- 既存の backlog に該当する PRD がないか確認する
- ない場合は `/prd-template` に従って PRD を作成し `backlog/` に配置する
- `plan.md` の消化順序に挿入する
- Medium の問題は `TODO` に記録する（PRD 化は任意）

## 禁止事項

- 一部のファイルだけ読んで「全体を確認した」と報告すること
- 推測や一般論だけで問題を報告すること（具体的なコード箇所で裏付ける）
- 前回の監査で報告済みの問題を見落とすこと（前回レポートを参照して差分を出す）
- 「問題なし」と報告して終わること（全ての変換パスについて明示的に OK/NG を判定する）
- テストの期待値を「テストが通っているから正しい」と判断すること（期待値自体の正しさを独立に検証する）

## 検証

- `report/conversion-correctness-audit.md` が作成・更新されている
- 全ての型マッピング、文/式変換パターンについて検証結果が記載されている
- 発見された Critical / High の問題が全て `backlog/` に PRD として存在する
- 前回の監査結果との差分が記録されている（初回の場合は不要）
