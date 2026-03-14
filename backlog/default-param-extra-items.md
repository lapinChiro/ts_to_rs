# デフォルト引数内 extra_items の破棄修正

## 背景・動機

`convert_default_param`（`src/transformer/functions/mod.rs` line ~242）が内部で `convert_param` を再帰呼び出しする際、内部呼び出しが生成する `extra_items`（例: インライン型リテラルから生成される構造体定義）を `_extra` として破棄している。

以下の TypeScript コードで問題が発生する:
```typescript
function f(x: { a: string } = {}) { ... }
```
インライン型リテラル `{ a: string }` は構造体定義を生成するが、`_extra` として破棄されるため出力に含まれない。

参照: `report/design-review.md` #9

## ゴール

`convert_default_param` 内の `convert_param` 呼び出しで生成される `extra_items` が呼び出し元に正しく伝播され、最終出力に含まれる。

## スコープ

### 対象

- `convert_default_param` の `_extra` 破棄を修正し、`extra_items` を戻り値に含める
- `convert_default_param` の戻り値型を extra items を含む形に更新
- 呼び出し元での extra items の受け取り・統合

### 対象外

- `convert_default_param` の外部的な振る舞いの変更（データフロー修正のみ）
- デフォルト値の変換ロジック自体の変更

## 設計

### 技術的アプローチ

1. `convert_default_param` 内で `convert_param` の戻り値から `extra_items` を受け取る（`_extra` → 変数束縛）
2. `convert_default_param` の戻り値型に `extra_items: Vec<...>` を追加（またはタプルで返す）
3. `convert_default_param` の呼び出し元で受け取った extra items を親の extra items に統合する

### 影響範囲

- `src/transformer/functions/mod.rs` — `convert_default_param` 関数の修正
- `convert_default_param` の呼び出し元（同ファイル内）

## 作業ステップ

- [ ] ステップ1（RED）: `function f(x: { a: string } = {})` の変換テストを追加し、構造体が生成されないことを確認（現状の不具合を再現）
- [ ] ステップ2（GREEN）: `convert_default_param` で `_extra` を変数束縛に変更し、戻り値に含める
- [ ] ステップ3（GREEN）: 呼び出し元で extra items を受け取り統合する
- [ ] ステップ4: Quality check

## テスト計画

- `function f(x: { a: string } = {})` → 構造体定義が出力に含まれること
- 回帰: デフォルト引数なしのインライン型リテラルが変更なく動作すること
- 回帰: 単純なデフォルト引数（`x: number = 0`）が変更なく動作すること

## 完了条件

- デフォルト引数内のインライン型リテラルから生成される構造体定義が最終出力に含まれる
- 既存のデフォルト引数テストがすべてパスする
- `cargo test`, `cargo clippy`, `cargo fmt --check` が 0 エラー・0 警告
