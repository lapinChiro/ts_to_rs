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

### Test Case Design Techniques

When designing test cases, apply the following techniques systematically. Each technique addresses a different class of defects — relying on a single technique leaves blind spots.

#### Equivalence Partitioning (同値分割)

Partition inputs into classes that should produce the same kind of output. Write at least one test per partition, including **invalid partitions**.

For this project's common partitions:
- **AST node variants**: Each `match` arm on SWC AST enums is a partition. Ensure every handled variant has a test, and unhandled variants have explicit "returns error/None" tests
- **Type partitions**: `RustType::F64`, `String`, `Bool`, `Option(_)`, `Named{..}`, `Vec(_)`, `Any`, `Fn{..}`, `Tuple(..)` — each is a distinct partition for type-dependent logic
- **Operator partitions**: `==`/`===` vs `!=`/`!==`, arithmetic vs comparison vs logical

#### Boundary Value Analysis (境界値分析)

For ordered inputs, test at boundaries. Apply to:
- Empty collections (`[]`, `{}`, empty `Vec`)
- Single-element vs multi-element collections
- Numeric extremes (`i32::MAX`, `f64::NAN`, `f64::INFINITY`)
- Nesting depth: 0 (flat), 1 (nested), 2+ (deeply nested)
- Parameter counts: 0, 1, many; especially for rest parameters

#### Branch Coverage (分岐網羅 / C1)

Every `if`, `match` arm, `if let Some/None`, and early `return` must have at least one test exercising each branch direction. When writing tests for a function:
1. Count the decision points (if/else, match arms, `?` operator, `.ok()?`)
2. Ensure test cases cover both true/false or each arm
3. Pay special attention to `_ => return None` and `_ => continue` — these are easy to miss

#### Decision Table (デシジョンテーブル)

When a function's behavior depends on **2+ independent conditions**, enumerate the condition combinations. Especially relevant for:
- Type conversion rules (optional × type × mutability)
- Pattern matching with multiple checks (is_eq × has_type × has_variant)
- Destructuring (has_default × is_nested × is_rest)

#### Transpiler-Specific: AST Variant Exhaustiveness

For functions that `match` on SWC AST enums:
- List all variants the function claims to handle
- Write one test per handled variant with representative TS input
- Write one test for an unhandled variant verifying graceful failure (error or skip)
- When a new variant is added to handling, add a corresponding test

### Code Conventions

- `unwrap()` / `expect()` are only allowed in test code (use `Result` propagation in library code)
- Each test must be independently runnable. Do not share state between tests

## Prohibited

- Placing unit tests in `tests/` directory (use `#[cfg(test)]` in `src/`)
- Sharing mutable state (files, global variables, etc.) between tests
- Using `unwrap()` / `expect()` in library code
- Completing conversion feature changes without writing E2E tests
