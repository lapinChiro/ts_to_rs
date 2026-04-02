# Quality Gates

## Completion Standard

完了報告前に `0 errors, 0 warnings` を満たす。

## Required Commands

通常の完了前チェック:

```bash
cargo fix --allow-dirty --allow-staged
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
./scripts/check-file-lines.sh
```

CI 整合性やカバレッジ確認が必要なとき:

```bash
cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89
```

## Verification Rules

- 単に exit code だけで済ませず、失敗内容を確認する
- 出力が大きい場合はファイルへリダイレクトして読む
- 「見た感じ大丈夫」は不可
- その変更と直接無関係でも、見つけたエラーや warning は修正可能なら直す

## E2E Requirement

以下では E2E を追加または拡張する。

- 新しい TS 構文 handler を追加した
- 既存変換ロジックのバグ修正をした
- built-in API を追加/変更した
- 型変換ロジックを変更した

## Prohibitions

- テストを弱めて通す
- `#[allow(...)]` で clippy を黙らせる
- quality check を省いて完了報告する
