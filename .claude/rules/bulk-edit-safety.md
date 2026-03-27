# Bulk Edit Safety Procedure

## When to Apply

When editing files via Python scripts, sed, awk, or regex-based bulk replacements.

## Constraints

The following steps **must** be executed:

1. **Identify targets**: List all target locations with `grep` and record the count
2. **Dry run**: Output the transformation result to stdout without writing to files:
   - Python scripts: Read files and display results, but do not write
   - sed: Output to stdout without `-i`
3. **Diff review**: Inspect the dry run output and verify:
   - All targets are correctly transformed (no omissions)
   - Non-target locations are untouched (no false matches)
   - If patterns are ambiguous, visually verify 3-5 representative cases
4. **Execute**: Write to files only after confirming no issues in the dry run
5. **Verify**: Run `cargo check` / `cargo test` after the transformation

## Prohibited

- Running bulk replacements without a dry run
- Bulk replacements using short generic patterns like `, reg,` (argument names appear in other contexts — use function-name-qualified patterns)
- Multi-line regex (DOTALL, cross-line patterns) (causes unexpected multiple matches or infinite insertions — handle cross-line changes manually)
- Using regex for Rust syntax-level decisions (function body start positions, match statement detection, etc.)
- Judging output as "looks fine" without reviewing the transformation script's output
- Building exclusion lists using `grep` patterns only (misses notation variations like `reg: &TypeRegistry` vs `reg: &crate::registry::TypeRegistry` — visually verify all actual signatures)
