---
name: quality-check
description: Post-completion quality check procedure. Run cargo fix, fmt, clippy, test and verify 0 errors, 0 warnings
user-invocable: true
---

# Quality Check on Completion

## Trigger

When user-requested work is complete (before commit).

## Actions

Run all of the following and verify **0 errors, 0 warnings**:

```bash
cargo fix --allow-dirty --allow-staged > /tmp/fix-result.txt 2>&1
cargo fmt --all --check > /tmp/fmt-result.txt 2>&1
cargo clippy --all-targets --all-features -- -D warnings > /tmp/clippy-result.txt 2>&1
cargo test > /tmp/test-result.txt 2>&1
./scripts/check-file-lines.sh > /tmp/file-lines-result.txt 2>&1
```

`cargo fix` auto-fixes compiler warnings like unused imports. Running it before `cargo fmt` / `cargo clippy` reduces manual fixes. `check-file-lines.sh` verifies that `.rs` files under `src/` are 1000 lines or fewer.

Follow `.claude/rules/command-output-verification.md` for command output verification.

On errors:

1. Fix all errors, including those not caused by the current changes
2. If an error cannot be fixed, document the cause and impact and report to the user

## Prohibited

- Deleting or weakening tests to eliminate errors
- Suppressing clippy warnings with `#[allow(...)]` (fix the root cause)
- Reporting "complete" without running quality checks
- Skimming output and judging it as "looks fine". Verify through each command's final message
- Deferring discovered warnings/errors because they're "not caused by this change" or "out of scope". If fixable when discovered, fix it; if it's a false positive, investigate and report accordingly

## Verification

- All commands completed with exit code 0
- Full output of each output file has been reviewed via the Read tool
