# 文字列メソッド変換の改善

対象 TODO: I-186, I-19

## 背景・動機

`map_method_call` の文字列メソッド変換に 2 つの問題がある:

1. **I-186**: `split()` が `.collect::<Vec<&str>>()` をハードコードしているため、`const parts: string[] = s.split("\n")` が `let parts: Vec<String> = ...collect::<Vec<&str>>()` となり型不一致のコンパイルエラーになる
2. **I-19**: `.substring()` / `.slice()` が未対応で、Rust の文字列スライスに変換されない

両方とも `map_method_call` 内の文字列メソッドマッピングの修正・追加。

## ゴール

1. `s.split(sep)` の結果が型注釈と整合する Rust コードを生成する（`Vec<String>` 注釈時は `.map(|s| s.to_string()).collect::<Vec<String>>()`）
2. `.substring(start, end)` / `.slice(start, end)` が Rust の文字列スライス操作に変換される
3. 既存テストに退行がない

## スコープ

### 対象

- `map_method_call` の `split` 変換修正（型注釈に応じた collect 生成）
- `map_method_call` に `substring` / `slice` マッピング追加
- ユニットテスト + スナップショットテスト + E2E テスト

### 対象外

- `split` の limit 引数（第2引数）対応
- 正規表現による `split`
- `.substr()` （非推奨メソッド）

## 設計

### split の修正

現在: `.split(sep).collect::<Vec<&str>>()`
修正後: `.split(sep).map(|s| s.to_string()).collect::<Vec<String>>()`

TypeScript の `string.split()` は `string[]` を返す。Rust の `str::split()` は `Iterator<Item=&str>` を返す。
`string[]` → `Vec<String>` なので、常に `.map(|s| s.to_string()).collect::<Vec<String>>()` を生成するのが正しい。

### substring / slice

- `s.substring(start, end)` → `s[start..end].to_string()` （バイト境界の問題は TODO として記録）
- `s.slice(start, end)` → 同上
- `s.slice(start)` → `s[start..].to_string()`
- 負のインデックスは `s.len() - abs(idx)` で計算

### 影響範囲

| ファイル | 変更内容 |
|---------|---------|
| `src/transformer/expressions/mod.rs` | `map_method_call` の split/substring/slice |
| `src/transformer/expressions/tests.rs` | ユニットテスト追加 |
| `tests/fixtures/` | スナップショットフィクスチャ |
| `tests/e2e/scripts/` | E2E スクリプト |

## 作業ステップ

- [ ] 1: split 修正のユニットテスト（RED）
- [ ] 2: split 修正の実装（GREEN）
- [ ] 3: substring のユニットテスト（RED）
- [ ] 4: substring/slice の実装（GREEN）
- [ ] 5: E2E テストスクリプト作成
- [ ] 6: 退行チェック

## テスト計画

| テスト | 入力 | 期待出力 |
|-------|------|---------|
| split 基本 | `s.split(",")` | `.split(",").map(\|s\| s.to_string()).collect::<Vec<String>>()` |
| substring 2引数 | `s.substring(1, 3)` | `s[1..3].to_string()` |
| slice 2引数 | `s.slice(1, 3)` | `s[1..3].to_string()` |
| slice 1引数 | `s.slice(1)` | `s[1..].to_string()` |

## 完了条件

- [ ] `split()` の変換結果が `Vec<String>` 型注釈と整合する
- [ ] `substring()` / `slice()` が Rust の文字列スライスに変換される
- [ ] E2E テスト PASS
- [ ] 既存テストに退行がない
- [ ] clippy 0 警告、fmt PASS、全テスト PASS
