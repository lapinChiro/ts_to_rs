# optional chaining 内のメンバーアクセス特殊変換バグ修正

## 背景・動機

`.length` → `.len() as f64` 等の特殊変換は `convert_member_expr` で処理されるが、optional chaining（`x?.length`）内では `convert_opt_chain_expr` が直接 `Expr::FieldAccess` を生成するため、特殊変換が適用されない。

Hono のソースコードでは optional chaining と `.length` の組み合わせが使われており、正しく変換できないとコンパイルエラーになる。

## ゴール

`x?.length` が `x.as_ref().map(|_v| _v.len() as f64)` に変換される。`convert_member_expr` で処理される全ての特殊変換（`.length`、enum アクセス）が optional chaining 内でも動作する。

## スコープ

### 対象

- `convert_opt_chain_expr` のプロパティアクセス部分を `convert_member_expr` と同等のロジックに修正
- `.length` → `.len() as f64` が optional chaining 内で動作する
- enum メンバーアクセス（`x?.Color.Red` 等）が optional chaining 内で動作する

### 対象外

- optional chaining の `Option` ネスト問題（型推論インフラが前提）
- optional chaining 内のメソッドコール変換（`x?.includes("a")` 等。別の問題として切り分け）

## 設計

### 技術的アプローチ

`convert_opt_chain_expr`（`expressions.rs:130`）で `MemberProp::Ident` のケースが直接 `Expr::FieldAccess` を生成している箇所を、`convert_member_expr` と同等の判定ロジック（`.length` チェック、enum チェック）を経由するように修正する。

具体的には、optional chaining の closure body 内で `_v.field` を生成する際に、field 名に応じた特殊変換を適用するヘルパー関数を抽出し、`convert_member_expr` と `convert_opt_chain_expr` の双方から呼び出す。

### 影響範囲

- `src/transformer/expressions.rs` — `convert_opt_chain_expr` の修正、ヘルパー関数の抽出

## 作業ステップ

- [ ] ステップ1（RED）: `x?.length` が `.len() as f64` に変換されることを検証するユニットテストを追加し、失敗を確認
- [ ] ステップ2（GREEN）: `convert_opt_chain_expr` のプロパティアクセス部分に `.length` の特殊変換を追加
- [ ] ステップ3（REFACTOR）: `convert_member_expr` と共通のヘルパー関数に抽出し、重複を排除
- [ ] ステップ4: E2E テスト（fixture）を追加

## テスト計画

- 正常系: `x?.length` → `x.as_ref().map(|_v| _v.len() as f64)`
- 正常系: `x?.y` → `x.as_ref().map(|_v| _v.y)`（既存の動作が壊れないこと）
- 正常系: `x?.y?.length` → チェーンされた optional chaining 内でも `.length` 変換が動作
- 境界値: `x?.y.length`（optional chaining と通常のメンバーアクセスの混在）

## 完了条件

- `x?.length` が正しく `.len() as f64` に変換される
- 既存の optional chaining テストが全て通る
- `cargo fmt --all --check` / `cargo clippy` / `cargo test` が 0 エラー・0 警告
