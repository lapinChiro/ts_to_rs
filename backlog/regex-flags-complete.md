# 正規表現フラグの完全対応

## 背景・動機

`i` と `m` のみ変換。`g`（グローバル）、`s`（dotAll）、`u`（unicode）、`y`（sticky）フラグが欠落。特に `g` フラグの欠落は全マッチ → 単一マッチに変わり、深刻。

## ゴール

正規表現のフラグが可能な限り正確に Rust の `regex` クレート等価に変換される。

## スコープ

### 対象

- `g` フラグ: `find_all` / `find` の使い分けに反映
- `s` フラグ: `(?s)` (dotAll) に変換
- `u` フラグ: Rust の regex はデフォルト Unicode 対応のため注記のみ
- `y` フラグ: sticky は Rust regex に直接対応なし — コメント付きで生成

### 対象外

- `d` フラグ (indices) — 稀少

## 設計

### 技術的アプローチ

`g` フラグは正規表現リテラル自体ではなく、**使用箇所**のメソッド選択に影響する:
- `str.match(regex)` → `g` あり: `regex.find_iter(str)`, `g` なし: `regex.find(str)`
- `str.replace(regex, repl)` → `g` あり: `regex.replace_all(str, repl)`, `g` なし: `regex.replace(str, repl)`

IR に正規表現のフラグ情報を保持し、メソッド呼び出し時に参照する。

### 影響範囲

- `src/transformer/expressions/mod.rs` — 正規表現リテラル変換
- `src/ir.rs` — `Expr::RegexLit` にフラグフィールド追加（必要に応じて）

## 作業ステップ

- [ ] ステップ1: `s` フラグの `(?s)` 変換テスト（RED → GREEN）
- [ ] ステップ2: `g` フラグ情報の IR 保持（RED → GREEN）
- [ ] ステップ3: `g` フラグに基づくメソッド選択（replace → replace_all 等）
- [ ] ステップ4: E2E テスト

## 完了条件

- [ ] `s` フラグが `(?s)` に変換される
- [ ] `g` フラグ付き regex の `replace` が `replace_all` に変換される
