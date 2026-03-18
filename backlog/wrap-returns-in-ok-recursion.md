# wrap_returns_in_ok のループ・match 内再帰

## 背景・動機

try/catch 変換で return を `Ok(...)` に包む `wrap_stmt_return` が `if` 分岐のみ再帰する。`while`/`for`/`match`/`loop` 内の return が `Ok(...)` に包まれず、型不一致になる。

## ゴール

全ての制御フロー構造内の return 文が `Ok(...)` に包まれる。

## スコープ

### 対象

- `While` / `ForIn` / `ForRange` / `Loop` 内の再帰
- `Match` アーム内の再帰
- `LabeledBlock` 内の再帰

### 対象外

- ネストした関数/クロージャ内の return（これは外側の関数の return ではない）

## 設計

### 技術的アプローチ

`wrap_stmt_return` の match に `Stmt::While`, `Stmt::ForIn`, `Stmt::ForRange`, `Stmt::Loop`, `Stmt::Match`, `Stmt::LabeledBlock` の分岐を追加し、body を再帰的に走査する。

### 影響範囲

- `src/transformer/functions/mod.rs` — `wrap_stmt_return`

## 作業ステップ

- [ ] ステップ1: While 内の return テスト（RED → GREEN）
- [ ] ステップ2: Match アーム内の return テスト（RED → GREEN）
- [ ] ステップ3: ForRange, Loop 等のテスト
- [ ] ステップ4: E2E テスト

## 完了条件

- [ ] 全制御フロー内の return が `Ok(...)` に包まれる
- [ ] `cargo test` 全テスト通過
