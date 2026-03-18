# instanceof のランタイム解決

## 背景・動機

`x instanceof Y` が TypeEnv の型名比較で静的に `true`/`false` に解決される。継承関係が無視され、型不明時は `true` がハードコードされる。

## ゴール

`x instanceof Y` が Rust の型システムで正しく判定される。

## スコープ

### 対象

- enum バリアントの判定: `x instanceof Foo` where x は discriminated union → `matches!(x, Enum::Foo { .. })`
- クラス継承の判定: trait object のダウンキャスト、または enum バリアントチェック
- 型不明時のエラー（`true` ハードコードの除去）

### 対象外

- JavaScript の prototype chain の完全な再現

## 設計

### 技術的アプローチ

TypeScript の `instanceof` は主に 2 つのパターンで使われる:

1. **discriminated union のナローイング**: `if (x instanceof Error)` → `matches!(x, Enum::Error(..))`
2. **クラス階層のチェック**: `if (x instanceof Base)` → trait bound check

現在の IR に `matches!` マクロ呼び出しの表現がないため、`Expr::FnCall { name: "matches!", .. }` で代用するか、新しい IR バリアントを追加する。

### 影響範囲

- `src/transformer/expressions/mod.rs` — instanceof 処理

## 作業ステップ

- [ ] ステップ1: enum バリアントチェックへの変換テスト（RED → GREEN）
- [ ] ステップ2: 型不明時のエラー化（`true` ハードコード除去）（RED → GREEN）
- [ ] ステップ3: E2E テスト

## 完了条件

- [ ] `instanceof` が静的 `true`/`false` を返さない
- [ ] discriminated union に対して `matches!` で正しく判定
