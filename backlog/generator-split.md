# generator.rs のファイル分割

## 背景・動機

`src/generator.rs` が 1187 行に肥大化しており、保守性が低下している。Item 生成・文生成・式生成・型生成の責務が 1 ファイルに混在している。

## ゴール

`generator.rs` を責務ごとに 4 モジュールに分割する。外部から見た公開 API（`generate`, `generate_type`）は変更しない。

### 分割後の構成

```
src/generator/
├── mod.rs          # 公開 API + Item 生成
├── types.rs        # 型の生成
├── statements.rs   # 文の生成
└── expressions.rs  # 式の生成
```

| モジュール | 責務 | 主な関数 |
|---|---|---|
| `mod.rs` | 公開 API、Item 生成、ユーティリティ | `generate`, `generate_item`, `generate_method`, `generate_vis`, `generate_type_params`, `indent_str` |
| `types.rs` | 型の生成 | `generate_type` |
| `statements.rs` | 文の生成 | `generate_stmt`, `generate_range_bound` |
| `expressions.rs` | 式の生成 | `generate_expr`, `generate_closure` |

## スコープ

### 対象

- `src/generator.rs` を `src/generator/` ディレクトリに分割
- テストを各モジュールの `#[cfg(test)]` に移動
- 内部関数の可視性調整（モジュール間で呼び出す関数を `pub(crate)` または `pub(super)` にする）

### 対象外

- 公開 API の変更
- ロジックの変更やリファクタリング（純粋なファイル分割のみ）

## 設計

### 技術的アプローチ

純粋なファイル移動 + 可視性調整。ロジックの変更は行わない。

1. `src/generator.rs` を `src/generator/mod.rs` にリネーム
2. 各責務の関数群を新ファイルに切り出す
3. モジュール間の依存を `pub(super)` で解決する
4. テストを対応するモジュールに移動する

### モジュール間依存

- `statements.rs` → `expressions.rs`（`generate_expr` を呼ぶ）
- `statements.rs` → `types.rs`（`generate_type` を呼ぶ）
- `mod.rs` → `statements.rs`, `types.rs`, `expressions.rs`
- `expressions.rs` → `types.rs`（`generate_type` を呼ぶ可能性）

### 影響範囲

- `src/generator.rs` → `src/generator/mod.rs`, `types.rs`, `statements.rs`, `expressions.rs` に分割
- `src/lib.rs` — `mod generator` の宣言は変更不要（ディレクトリモジュールは自動解決）

## 作業ステップ

- [ ] ステップ1: `src/generator.rs` を `src/generator/mod.rs` に移動
- [ ] ステップ2: `types.rs` を切り出し — `generate_type` とその関連テストを移動
- [ ] ステップ3: `expressions.rs` を切り出し — `generate_expr`, `generate_closure` とその関連テストを移動
- [ ] ステップ4: `statements.rs` を切り出し — `generate_stmt`, `generate_range_bound` とその関連テストを移動
- [ ] ステップ5: 可視性調整 — モジュール間で必要な関数を `pub(super)` にする
- [ ] ステップ6: 全テスト通過を確認

## テスト計画

- 既存テストが全て通ること（ロジック変更がないため、新規テストは不要）
- スナップショットテストの結果が変わらないこと

## 完了条件

- `src/generator/` ディレクトリに 4 ファイルが存在する
- `src/generator.rs` が存在しない
- 公開 API（`generate`, `generate_type`）のシグネチャが変わっていない
- `cargo fmt --all --check` / `cargo clippy --all-targets --all-features -- -D warnings` / `cargo test` が全て 0 エラー・0 警告
