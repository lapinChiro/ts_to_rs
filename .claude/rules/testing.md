---
paths:
  - "tests/**"
  - "src/**/tests.rs"
---

# Testing Conventions

## When to Apply

When creating or modifying test code.

## Constraints

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

Apply these techniques systematically when designing test cases. Each addresses a different defect class — relying on a single technique leaves blind spots.

#### Equivalence Partitioning

Partition inputs into classes producing the same kind of output. Write at least one test per partition, including **invalid partitions**.

Project-specific partitions:
- **AST node variants**: Each `match` arm on SWC AST enums is a partition. Test every handled variant; test unhandled variants for graceful error/None
- **Type partitions**: `RustType::F64`, `String`, `Bool`, `Option(_)`, `Named{..}`, `Vec(_)`, `Any`, `Fn{..}`, `Tuple(..)` — each distinct for type-dependent logic
- **Operator partitions**: `==`/`===` vs `!=`/`!==`, arithmetic vs comparison vs logical

#### Boundary Value Analysis

Test at boundaries of ordered inputs:
- Empty collections (`[]`, `{}`, empty `Vec`)
- Single-element vs multi-element collections
- Numeric extremes (`i32::MAX`, `f64::NAN`, `f64::INFINITY`)
- Nesting depth: 0 (flat), 1 (nested), 2+ (deeply nested)
- Parameter counts: 0, 1, many; especially for rest parameters

#### Branch Coverage (C1)

Every `if`, `match` arm, `if let Some/None`, and early `return` must have at least one test exercising each branch direction:
1. Count decision points (if/else, match arms, `?` operator, `.ok()?`)
2. Ensure test cases cover both true/false or each arm
3. Pay special attention to `_ => return None` and `_ => continue` — easy to miss

#### Decision Table

When behavior depends on **2+ independent conditions**, enumerate condition combinations. Especially relevant for:
- Type conversion rules (optional × type × mutability)
- Pattern matching with multiple checks (is_eq × has_type × has_variant)
- Destructuring (has_default × is_nested × is_rest)

#### Transpiler-Specific: Recursive Function Termination

For functions that recurse on type structures (e.g., `resolve_type_params_in_type`, `convert_ts_type`, `unify_type`):
- **Self-referential input**: Test with input that maps back to itself (e.g., type param constraint `"T" → Named("T")`)
- **Mutual recursion**: Test with types that reference each other (e.g., `A<B>` where `B` contains `A`)
- **Deep nesting**: Test with nesting depth exceeding expected limits (e.g., `Option<Option<Option<...>>>`)
- **HashMap/map-based lookups**: When a function looks up a key and recurses on the result, test that the result doesn't contain the same key (circular reference)

Incidents: `resolve_type_params_in_type` caused an infinite loop in directory mode when `type_param_constraints` contained `"T" → Named("T")` (self-referential). The function recursively resolved `Named("T")` → looked up `"T"` → got `Named("T")` → infinite recursion. Fixed by adding depth limit and self-reference detection.

#### Transpiler-Specific: AST Variant Exhaustiveness

For functions that `match` on SWC AST enums:
- List all variants the function claims to handle
- Write one test per handled variant with representative TS input
- Write one test for an unhandled variant verifying graceful failure (error or skip)
- When a new variant is added to handling, add a corresponding test

### Test Coverage Review in PRDs

When creating a PRD, a test coverage review of the impact area is **mandatory** before writing the task list. See `prd-template` skill (step 2: Test Coverage Review) for the procedure. The review uses the techniques above to identify:
- **Incorrect expectations**: Tests that pass but assert wrong behavior (bug-affirming tests)
- **Missing branch coverage**: Decision points with no exercising test
- **Missing partitions**: Input classes with no representative test
- **Missing error paths**: Error-returning branches with no test

**All** gaps found must be included in the PRD's task list, regardless of severity. No gap is too small to test.

### Code Conventions

- `unwrap()` / `expect()` are only allowed in test code (use `Result` propagation in library code)
- Each test must be independently runnable. Do not share state between tests

## Prohibited

- Placing unit tests in `tests/` directory (use `#[cfg(test)]` in `src/`)
- Sharing mutable state (files, global variables, etc.) between tests
- Using `unwrap()` / `expect()` in library code
- Completing conversion feature changes without writing E2E tests
- **Creating a PRD without reviewing existing test coverage** in the impact area using the techniques above

## Related Rules

| Rule | Relation |
|------|----------|
| [pipeline-integrity.md](pipeline-integrity.md) | Test placement (unit / integration / E2E) と pipeline 構成の整合 |
| [check-job-review-layers.md](check-job-review-layers.md) | Layer 1 (Mechanical) で test name 形式 / bug-affirming test 等を verify |
| [problem-space-analysis.md](problem-space-analysis.md) | Test は問題空間マトリクスの全 cell から導出 (本ルール test design techniques と相補) |
| [spec-stage-adversarial-checklist.md](spec-stage-adversarial-checklist.md) | Rule "E2E readiness" で per-cell E2E fixture 準備を verify |
