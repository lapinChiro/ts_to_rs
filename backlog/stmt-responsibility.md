# convert_stmt と convert_stmt_list の責務整理

## 背景・動機

`convert_stmt_list` が分割代入展開・for ループ特殊パターンを前処理し `convert_stmt` に fallback する構造になっている。`convert_stmt` を直接呼ぶと分割代入が処理されない。これは内部 API の安全性問題であり、呼び出し側が「どちらを使うべきか」を知っている必要がある。

`src/transformer/statements/mod.rs` の現状:
- `convert_stmt`（line 30）: 個別の文を処理
- `convert_stmt_list`（line 384）: 以下の前処理を追加するラッパー:
  - 分割代入付き変数宣言（オブジェクト/配列デストラクチャリング）
  - for 文の単純カウンターパターン検出
  - ラベル付き for ループ

参照: `report/design-review.md` #8

## ゴール

`convert_stmt` が単独で呼ばれても全てのステートメント変換を正しく処理する状態にする。`convert_stmt_list` は単純なループ＋リストレベルの関心事（ラベル付きループの展開等）のみを担う。

## スコープ

### 対象

- 分割代入の前処理を `convert_stmt_list` から `convert_stmt` の `Decl::Var` アームに移動
- for ループ特殊パターン検出を `convert_stmt_list` から `convert_stmt` の `For` アームに移動
- `convert_stmt_list` の簡素化

### 対象外

- 外部 API の変更（両関数とも public のまま維持）
- 新機能の追加（純粋なリファクタリング）

## 設計

### 技術的アプローチ

1. `convert_stmt` の `Decl::Var` アーム内で、各宣言子についてオブジェクト/配列デストラクチャリングを先に試行し、失敗時に通常の変数宣言処理へ fallback する
2. `convert_stmt` の `For` アーム内で、単純カウンターパターンを先に試行し、失敗時に通常の for 文処理へ fallback する
3. `convert_stmt_list` からこれらの前処理コードを削除し、ループ＋ラベル付きステートメント展開のみに簡素化する

### 影響範囲

- `src/transformer/statements/mod.rs` — `convert_stmt` と `convert_stmt_list` の両方

## 作業ステップ

- [ ] ステップ1: `convert_stmt` の `Decl::Var` アームに分割代入チェックを移動
- [ ] ステップ2: `convert_stmt` の `For` アームに単純カウンターパターン検出を移動
- [ ] ステップ3: `convert_stmt_list` から移動済みコードを削除し簡素化
- [ ] ステップ4: 全既存テストがパスすることを確認（純粋リファクタリング）
- [ ] ステップ5: Quality check

## テスト計画

- 全既存テストがパスすること（純粋リファクタリングのため新規テスト不要）
- `convert_stmt` を直接呼んだ場合でも分割代入パターンが正しく処理されることを確認する単体テストの追加を検討

## 完了条件

- `convert_stmt` が単独で全てのステートメント変換を処理できる
- `convert_stmt_list` はループ＋リストレベル関心事のみに簡素化されている
- 全テストがパスし、`cargo test`, `cargo clippy`, `cargo fmt --check` が 0 エラー・0 警告
