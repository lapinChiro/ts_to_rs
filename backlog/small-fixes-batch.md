# 小修正バッチ: トップレベル式文 + デフォルト値 + 単項プラス

対象 TODO: I-180, I-146, I-15

## 背景・動機

Hono ベンチマークに残存する 1 件ずつの小さなエラーを一括で解消する。いずれも独立した修正だが、個別に PRD 化するほどの規模ではない。

1. **I-180**: トップレベル式文（1 インスタンス）— `globalThis.crypto ??= crypto` がトップレベルで失敗
2. **I-146**: デフォルト値の残存パターン（1 インスタンス）— 型注釈なしのデフォルト値
3. **I-15**: `+x`（単項プラス / 数値変換）— TypeScript の `+x` は `Number(x)` と同等

## ゴール

1. トップレベルの式文（関数外の式）が変換される
2. 型注釈なしのデフォルトパラメータが型推論で変換される
3. `+x` が Rust の数値変換に対応する
4. 既存テストに退行がない

## スコープ

### 対象

- トップレベル式文の変換（`transformer/mod.rs` の `transform_module_item`）
- 型注釈なしデフォルトパラメータの型推論（リテラル値から型を推定）
- 単項プラス `+x` → `x as f64` または `x.parse::<f64>().unwrap()`（型に応じて）
- ユニットテスト + スナップショットテスト

### 対象外

- トップレベルの `await`（async top-level）
- デフォルト値の複雑な式（関数呼び出し等）
- `+x` の BigInt 対応

## 設計

### I-180: トップレベル式文

`ModuleItem::Stmt(Stmt::Expr(expr_stmt))` をモジュールレベルで処理。現在は関数内のみ `convert_stmt` で処理される。

修正: `transform_module_item` に `Stmt::Expr` のハンドラを追加。式を `Item::Fn` のラッパーなしで直接出力するか、`lazy_static!` / `static` ブロック等で包む。

`globalThis.crypto ??= crypto` は `static` 初期化として変換可能。

### I-146: 型注釈なしデフォルト値

`function foo(x = 0)` の `x` に型注釈がない場合。リテラル値からの型推定:
- 数値 → `f64`
- 文字列 → `String`
- boolean → `bool`

### I-15: 単項プラス

`+x` は TypeScript で `Number(x)` と同等。
- `x` が既に `f64` → そのまま `x`
- `x` が `String` → `x.parse::<f64>().unwrap()`
- 不明な型 → `x as f64`（フォールバック）

### 影響範囲

| ファイル | 変更内容 |
|---------|---------|
| `src/transformer/mod.rs` | トップレベル式文のハンドラ追加 |
| `src/transformer/expressions/mod.rs` | 単項プラスの変換 |
| `src/transformer/functions/mod.rs` | デフォルトパラメータの型推論 |
| テストファイル | ユニットテスト + スナップショット |

## 作業ステップ

- [ ] 1: トップレベル式文のユニットテスト（RED）
- [ ] 2: トップレベル式文の実装（GREEN）
- [ ] 3: 型注釈なしデフォルト値のテスト（RED）
- [ ] 4: リテラル値からの型推定実装（GREEN）
- [ ] 5: 単項プラスのテスト（RED）
- [ ] 6: 単項プラスの変換実装（GREEN）
- [ ] 7: 退行チェック

## テスト計画

| テスト | 入力 | 期待出力 |
|-------|------|---------|
| トップレベル式文 | `globalThis.crypto ??= crypto` | lazy_static / static 初期化 |
| デフォルト値 数値 | `function foo(x = 0)` | `fn foo(x: f64)` with default |
| デフォルト値 文字列 | `function foo(s = "hi")` | `fn foo(s: String)` with default |
| 単項プラス f64 | `+x` where x: number | `x` |
| 単項プラス 文字列 | `+x` where x: string | `x.parse::<f64>().unwrap()` |

## 完了条件

- [ ] トップレベル式文が変換される
- [ ] 型注釈なしデフォルトパラメータが型推論で変換される
- [ ] `+x` が適切な数値変換に変換される
- [ ] 既存テストに退行がない
- [ ] clippy 0 警告、fmt PASS、全テスト PASS
