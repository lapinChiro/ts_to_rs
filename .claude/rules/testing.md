---
paths:
  - "tests/**"
  - "src/**/tests.rs"
---

# テスト規約

## トリガー

テストコードを新規作成または変更するとき。

## アクション

以下の規約に従ってテストを配置・記述する:

### 配置

| 種類 | 配置 | 用途 |
|------|------|------|
| **ユニットテスト** | `src/**/*.rs` 内の `#[cfg(test)] mod tests` | モジュール内部ロジック |
| **統合テスト** | `tests/*.rs` | 公開 API の E2E テスト |
| **スナップショットテスト** | `tests/` (insta 使用) | TS → Rust 変換の出力検証 |
| **E2E テスト** | `tests/e2e/scripts/*.ts` + `tests/e2e_test.rs` | 変換後 Rust の実行時正確性検証 |

### スナップショットテスト

- fixture ファイル: `tests/fixtures/<name>.input.ts`
- `insta::assert_snapshot!` で出力を検証
- スナップショット更新: `cargo insta review`

### E2E テスト

E2E テストは「同じ TS コードを tsx で実行した stdout」と「変換後の Rust を cargo run した stdout」が一致することを検証する。

- スクリプト: `tests/e2e/scripts/<name>.ts`（`function main(): void { ... }` を定義）
- テスト関数: `tests/e2e_test.rs` に `run_e2e_test("<name>")` を呼ぶ関数を追加
- Rust ランナー: `tests/e2e/rust-runner/`（変換結果をここに書き出して実行）
- **変換機能を変更したら、対応する E2E テストの追加・拡充が必須**

### コード規約

- `unwrap()` / `expect()` はテストコード内でのみ許可（ライブラリコードでは `Result` で伝播）
- 各テストは独立して実行可能であること。テスト間で状態を共有しない

## 禁止事項

- ユニットテストを `tests/` ディレクトリに配置すること（`src/` 内の `#[cfg(test)]` に置く）
- テスト間で可変な状態（ファイル、グローバル変数等）を共有すること
- ライブラリコードで `unwrap()` / `expect()` を使用すること
- 変換機能の変更で E2E テストを書かずに完了とすること
