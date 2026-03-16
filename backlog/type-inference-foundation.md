# 型推論基盤の整備（I-38 + I-16 + I-22）

## 背景・動機

現在、`resolve_expr_type` は識別子と 1 レベルのフィールドアクセスしか型解決できない。型情報が不足するたびに `RustType::Any` にフォールバックしており、生成される Rust コードの品質を広範囲に劣化させている。

TODO に列挙された 12 箇所の `Any` フォールバックのうち、大半は TypeEnv の解決能力不足と expected 型の伝搬欠落に起因する。これらは個別に対処するのではなく、型推論基盤として一括で整備する必要がある。

副次効果として I-16（空配列の型推論）と I-22（型注記なしパラメータ）も解消される。

## ゴール

1. `resolve_expr_type` が以下の式ノードで型を解決できる:
   - 多段メンバーアクセス（`a.b.c`）
   - 関数呼び出しの戻り値（TypeRegistry に登録された関数）
   - 配列要素の型（`arr[0]` → `Vec<T>` の `T`）
   - 二項演算の結果型（算術 → `f64`、比較 → `bool`、論理 → operand 型）
   - `new` 式の結果型（コンストラクタ → 対応する struct 型）
2. `expected` 型が以下の式ノードに伝搬される:
   - 二項演算の両辺
   - optional chain の継続部分
   - 関数呼び出しの引数（TypeRegistry からパラメータ型を取得）
   - 単項演算のオペランド
3. TypeEnv がスコープ境界を正しく追跡する:
   - ブロックスコープでの変数シャドウイングを正しく反映
   - 変数への再代入で型が更新される
4. 以下のヒューリスティックが不要になり、型推論ベースの判定に置き換わる:
   - `is_string_like`（IR レベルのメソッド名ベース推定）
   - `is_string_type` の `Option<String>` → `String` 扱いハック
5. 既存テスト（cargo test）が全て通り、スナップショットの変更は生成コード品質の向上（`Any` → 具体型）のみ

## スコープ

### 対象

- `resolve_expr_type` の拡張（多段アクセス、関数戻り値、配列要素、二項演算、new 式）
- `convert_expr` の `expected` 型伝搬の拡張（二項演算、optional chain、関数引数、単項演算）
- TypeEnv のスコープ管理改善（シャドウイング、再代入の型更新）
- `is_string_like` / `is_string_type` ヒューリスティックの型推論ベースへの置換
- I-16（空配列の型推論: `const arr = []` で型注記なしの場合）の解消
- I-22（arrow 関数パラメータの型注記なし）の解消
- 12 箇所の `Any` フォールバックのうち、TypeEnv 拡張と expected 伝搬で解消可能なものの修正

### 対象外

- チェーンメソッド呼び出しの戻り値型追跡（`arr.filter(...).map(...)` 等）→ I-61 で別途対応
- ビルトイン API の戻り値型テーブル → I-61 の前提として別途構築
- conditional type の分岐型改善（I-28）→ 型推論基盤完成後に別 PRD
- 型 narrowing（`typeof` / `instanceof` 後の型絞り込み）→ I-45 完了後に別 PRD
- `any` / `unknown` の実用的な変換先の決定 → ユーザーの設計方針決定待ち

## 設計

### 技術的アプローチ

#### 1. TypeEnv のスコープ管理改善

現在の `TypeEnv` は flat な `HashMap<String, RustType>` で、スコープ境界がない。スコープチェーンを導入する:

```rust
pub struct TypeEnv {
    scopes: Vec<HashMap<String, RustType>>,
}
```

- `push_scope()` / `pop_scope()` でブロックスコープの出入りを管理
- 変数参照時は最内スコープから順に探索
- 再代入時は変数が存在するスコープの値を更新

#### 2. resolve_expr_type の拡張

現在は `Expr::Ident` と 1 レベルの `Expr::Member` のみ対応。以下を追加:

- **多段メンバーアクセス**: 再帰的に base の型を解決し、TypeRegistry でフィールド型を引く
- **関数呼び出し**: TypeEnv またはTypeRegistry から関数の戻り値型を取得
- **配列インデックス**: base 型が `Vec<T>` なら `T` を返す
- **二項演算**: 算術 → `F64`、比較/等値 → `Bool`、論理 → operand 型の合成
- **new 式**: TypeRegistry からコンストラクタの型を取得

#### 3. expected 型伝搬の拡張

`convert_expr` で `expected` を受け取っているが、一部の式ノードに伝搬されていない。以下を追加:

- **二項演算**: 両辺に expected を伝搬（算術演算の場合は `F64`、文字列結合の場合は `String`）
- **optional chain**: 継続部分に base の型情報を伝搬
- **関数呼び出し引数**: TypeRegistry からパラメータ型を取得し、各引数の expected として渡す
- **単項演算**: オペランドに expected を伝搬

#### 4. ヒューリスティックの置換

- `is_string_like`: `resolve_expr_type` が `String` を返せば文字列結合と判定。フォールバックとして現行ロジックを残し、型推論で解決できた場合はそちらを優先
- `is_string_type` の `Option<String>` ハック: TypeEnv のシャドウイング追跡で、`unwrap_or` 後の変数型が `String` に更新されるため不要になる

### 影響範囲

- `src/transformer/mod.rs` — TypeEnv 構造体の変更
- `src/transformer/expressions/mod.rs` — resolve_expr_type 拡張、expected 伝搬、ヒューリスティック置換
- `src/transformer/statements/mod.rs` — TypeEnv スコープ管理の呼び出し追加
- `src/transformer/functions/mod.rs` — TypeEnv スコープ管理の呼び出し追加
- `src/transformer/types/mod.rs` — Any フォールバック箇所の改善（型情報が利用可能な場合）
- `tests/` — スナップショット更新（`Any` → 具体型への改善）

## 作業ステップ

- [ ] ステップ 1: TypeEnv のスコープチェーン導入
  - `TypeEnv` を `Vec<HashMap>` ベースに変更
  - `push_scope()` / `pop_scope()` / `insert()` / `get()` を実装
  - 既存の TypeEnv 利用箇所を新 API に移行
  - テスト: 既存テストが全て通ることを確認（振る舞い変更なし）

- [ ] ステップ 2: resolve_expr_type の多段メンバーアクセス対応
  - `Expr::Member` で再帰的に base 型を解決
  - TypeRegistry でフィールド型を引く処理を多段に拡張
  - テスト: `a.b.c` パターンの型解決テスト

- [ ] ステップ 3: resolve_expr_type の関数呼び出し戻り値対応
  - 関数名から TypeEnv → TypeRegistry の順で `RustType::Fn` を探索
  - `Fn.return_type` を返す
  - テスト: `getValue()` の結果型が解決されるテスト

- [ ] ステップ 4: resolve_expr_type の残りの式ノード対応
  - 配列インデックス（`Vec<T>` → `T`）
  - 二項演算の結果型
  - new 式
  - テスト: 各パターンの型解決テスト

- [ ] ステップ 5: expected 型伝搬の拡張
  - 二項演算の両辺への伝搬
  - optional chain への伝搬
  - 関数呼び出し引数への伝搬
  - 単項演算への伝搬
  - テスト: 伝搬によって `Any` が具体型に置き換わるケース

- [ ] ステップ 6: ヒューリスティックの型推論ベースへの置換
  - `is_string_like` を型推論優先に変更（フォールバックとして現行ロジック残留）
  - `is_string_type` の `Option<String>` ハックを除去
  - TypeEnv のシャドウイング追跡でハックが不要になることを確認
  - テスト: 文字列結合の既存テストが全て通る

- [ ] ステップ 7: Any フォールバック箇所の改善
  - 12 箇所の Any フォールバックを再点検
  - TypeEnv 拡張で解決可能になった箇所を具体型に置換
  - 解決不可能な箇所（外部要因）はそのまま残し、理由をコメント
  - テスト: スナップショット更新の差分が `Any` → 具体型のみであることを確認

- [ ] ステップ 8: I-16（空配列）と I-22（型注記なしパラメータ）の解消確認
  - expected 伝搬で空配列に型が付くことを確認
  - コンテキストからの型推論で arrow パラメータに型が付くことを確認
  - テスト: 両パターンの専用テスト追加

## テスト計画

- **単体テスト**: TypeEnv のスコープ管理（push/pop/insert/get/shadowing）
- **単体テスト**: resolve_expr_type の各拡張パターン（多段アクセス、関数戻り値、配列要素、二項演算、new 式）
- **統合テスト（スナップショット）**: 既存の変換テストケースで `Any` が具体型に改善されることを確認
- **回帰テスト**: 全既存テストが通ること（意図しない振る舞い変更がないこと）
- **境界値**: TypeRegistry 未登録の型に対する resolve が `None` を返すこと（パニックしない）
- **境界値**: 循環参照（`a.b` が `a` 型を参照）で無限ループしないこと

## 完了条件

1. `cargo test` 全テスト通過
2. `cargo clippy --all-targets --all-features -- -D warnings` 0 警告
3. `cargo fmt --all --check` 通過
4. 12 箇所の `Any` フォールバックのうち、TypeEnv/expected 拡張で解消可能なものが全て具体型に置換されている
5. `is_string_like` が型推論優先で動作し、フォールバックとしてのみ残っている
6. `is_string_type` の `Option<String>` ハックが除去されている
7. I-16、I-22 の対象パターンで `Any` ではなく具体型が生成される
8. スナップショットの差分が `Any` → 具体型の改善のみ（意図しない退行がない）
