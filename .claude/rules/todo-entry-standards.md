# TODO Entry Standards

## When to Apply

When adding new items to TODO or modifying existing items.

## Constraints

- **Instance counts must be benchmark-measured values**: Use values aggregated from the `kind` field in `/tmp/hono-bench-errors.json` after running `./scripts/hono-bench.sh`. Do not write from estimates or past memory. Use `scripts/inspect-errors.py` for error inspection (`--category`, `--discriminant --source`, etc.)
- **Source code references must include `file_path:line_number`**: Specify concrete locations, not just function/variable names (e.g., `src/registry.rs:482`). Readers must be able to immediately navigate to the location
- **Error messages must quote actual output**: Use the benchmark `kind` field values or compiler output verbatim
- **Include solution direction**: Beyond describing the problem, document the resolution direction (specific function names, approaches). This provides material for later priority judgments, not to skip Discovery during PRD creation
- **Document dependencies**: When other TODO items are prerequisites, mark with `🔗`. If the reference target doesn't exist in TODO (completed, etc.), write self-contained context
- **Delete completed items immediately**: Completion records are traceable via git history. Only add a one-line summary to the "Completed features (reference)" section

## Prohibited

- Vague impact descriptions like "affects N files" (use measured instance counts)
- Documenting instance counts without running the benchmark
- Referring to "this function" or "this process" without file path or line number
- Keeping completed PRD items in "PRD created → backlog/..." format (delete them)
