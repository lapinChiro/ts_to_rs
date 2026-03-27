---
name: todo-replenishment
description: Replenishment procedure when TODO is empty and user requests work. Analyze current implementation and propose with user hearing
user-invocable: true
---

# TODO Replenishment

## Trigger

When `TODO` is empty (no unrefined ideas) and the user requests work.

## Actions

1. Analyze the current implementation (supported conversions, unsupported syntax, known limitations)
2. Considering the repo's purpose (practical TS → Rust conversion), propose next valuable features/improvements:
   - Unsupported TS syntax (conversion feature expansion)
   - Generated code quality improvements (ownership, error handling, etc.)
   - Development infrastructure (tests, CI, DX)
3. Interview the user to confirm priorities and direction
4. Write agreed items to `TODO`

## Prohibited

- Writing items to `TODO` without user interview
- Asking only "What should we do?" without presenting analysis results (must include concrete proposals)
- Excluding TS syntax from proposals because "Rust has no direct syntax equivalent" — if no conversion method is found, interview the user (do not independently judge "impossible")
