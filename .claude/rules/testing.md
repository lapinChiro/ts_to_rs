---
paths:
  - "tests/**"
  - "src/**/tests.rs"
---

# Testing Conventions

## Trigger

When creating or modifying test code.

## Actions

Follow these conventions for test placement and writing:

### Placement

| Type | Location | Purpose |
|------|----------|---------|
| **Unit tests** | `#[cfg(test)] mod tests` in `src/**/*.rs` | Module internal logic |
| **Integration tests** | `tests/*.rs` | Public API E2E testing |
| **Snapshot tests** | `tests/` (using insta) | TS → Rust conversion output verification |
| **E2E tests** | `tests/e2e/scripts/*.ts` + `tests/e2e_test.rs` | Runtime correctness verification of converted Rust |

### Snapshot Tests

- Fixture files: `tests/fixtures/<name>.input.ts`
- Verify output with `insta::assert_snapshot!`
- Update snapshots: `cargo insta review`

### E2E Tests

E2E tests verify that "stdout from running the same TS code with tsx" matches "stdout from cargo run of the converted Rust".

- Scripts: `tests/e2e/scripts/<name>.ts` (define `function main(): void { ... }`)
- Test functions: Add a function calling `run_e2e_test("<name>")` in `tests/e2e_test.rs`
- Rust runner: `tests/e2e/rust-runner/` (conversion results are written here and executed)
- **When modifying conversion features, adding/expanding corresponding E2E tests is mandatory**

#### E2E Test Decision Criteria

E2E test addition/expansion is **mandatory** when any of the following apply:

| Change Type | E2E Test Response |
|-------------|-------------------|
| Adding a new TS syntax handler | New script or add cases to existing script |
| Fixing an existing conversion logic bug | Add bug-reproducing input to E2E script |
| Adding/changing a built-in API | Add E2E case using that API |
| Changing type conversion logic | Verify converted result works correctly at runtime via E2E |

E2E tests are not required when:

- Refactoring (no change in external behavior)
- Parser-only changes (no impact on IR generation)
- Documentation/comment-only changes

### Code Conventions

- `unwrap()` / `expect()` are only allowed in test code (use `Result` propagation in library code)
- Each test must be independently runnable. Do not share state between tests

## Prohibited

- Placing unit tests in `tests/` directory (use `#[cfg(test)]` in `src/`)
- Sharing mutable state (files, global variables, etc.) between tests
- Using `unwrap()` / `expect()` in library code
- Completing conversion feature changes without writing E2E tests
