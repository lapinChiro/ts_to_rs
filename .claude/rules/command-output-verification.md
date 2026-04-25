# Command Output Verification

## When to Apply

When verifying output of build/test commands such as `cargo test`, `cargo clippy`, `cargo fmt --check`.

## Constraints

- **Targeted test runs** (`cargo test -- <test_name>`): Output is small enough to verify directly on stdout. Filter with `grep -E "test result:|FAILED|panicked"`
- **Full test/check runs**: Redirect output to a file and verify the full content with the Read tool. Example: `cargo test > /tmp/test-result.txt 2>&1`
- Obtain all necessary information in a single command execution. Determine output filters before execution

## Prohibited

- Running the same command twice with different output filters (test execution is time-consuming and this is inefficient)
- Using `tail` to get output end (line count is unpredictable and necessary information may be lost)
- Judging output as "looks fine" without reviewing it

## Related Rules

| Rule | Relation |
|------|----------|
| [bulk-edit-safety.md](bulk-edit-safety.md) | Bulk edit 後の verify 手順で本ルールを適用 |
| [check-job-review-layers.md](check-job-review-layers.md) | Layer 1 (Mechanical) の verify artifact (cargo test / clippy / fmt 出力) で本ルールに準拠 |
| [testing.md](testing.md) | Test 実行結果 verify と test placement の整合 |
