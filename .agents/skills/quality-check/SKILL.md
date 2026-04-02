---
name: quality-check
description: Use when implementation is complete and before reporting completion. Run the repository quality gate, verify outputs, and ensure the change leaves 0 errors and 0 warnings.
---

# Quality Check

## Run

```bash
cargo fix --allow-dirty --allow-staged
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
./scripts/check-file-lines.sh
```

必要な場合:

```bash
cargo llvm-cov --ignore-filename-regex 'main\.rs' --fail-under-lines 89
```

## Requirements

- 大きい出力はファイルにリダイレクトして確認する
- exit code と出力内容の両方を見る
- 発見した warning/error は current change 起因でなくても修正可能なら直す

## Do Not

- `#[allow(...)]` で握り潰さない
- チェック未実行で完了報告しない
- 「見た感じ通っている」で済ませない
