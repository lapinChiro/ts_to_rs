---
name: tdd
description: TDD development procedure for new features or bug fixes. Implement in order: test design → RED → GREEN → REFACTOR → E2E
user-invocable: true
---

# TDD Development Procedure

## Trigger

When starting a new feature or bug fix.

## Actions

Implement in the following order:

1. **Test design**: Before writing test code, enumerate verification items
   - List normal cases, error cases, and boundary values (MECE)
   - For each item, define input and expected output/behavior
   - Express this in test names: `test_<target>_<condition>_<expected_result>`
   - **Determine E2E test necessity** (see "E2E Test Decision Criteria" below)
   - **For pipeline-crossing features, include IR structure verification tests**: Verify not just final output (snapshots) but that IR has the correct structure. This detects "correct output generated from incorrect IR"
   - **For new methods, include integration tests**: Beyond unit tests, verify the method is used in the actual conversion pipeline
2. **RED**: Write the designed verification items as test code and confirm they fail
   - Run only target tests with `cargo test -- <test_name>`
   - Follow `.claude/rules/command-output-verification.md` for output verification
3. **GREEN**: Write minimal code to pass the tests
   - Follow `.claude/rules/command-output-verification.md` for output verification
4. **REFACTOR**: Refactor while maintaining passing tests
   - Ensure `cargo clippy` also passes
5. **E2E**: For conversion feature changes, add/expand corresponding E2E test scripts
   - Add TS scripts to `tests/e2e/scripts/` or expand existing scripts
   - Add test functions to `tests/e2e_test.rs` (for new scripts)
   - Confirm E2E tests pass with `cargo test -- test_e2e_<name>`

## E2E Tests

See `.claude/rules/testing.md` "E2E Tests" section for decision criteria and writing guidelines. E2E test addition/expansion is mandatory for conversion feature changes.

## Prohibited

- Writing implementation code first and tests afterward
- Writing test code without test design (enumerating verification items)
- Proceeding to GREEN without confirming RED (failure). If a test passes immediately, it may not be verifying anything
- Not defining expected results in advance and making post-hoc "this is correct" judgments based on actual results
- **Completing conversion feature changes without writing E2E tests**

## Verification

- Test code diffs exist before implementation code diffs (verifiable via git history)
- All tests follow the `test_<target>_<condition>_<expected_result>` naming convention
- For conversion feature changes, corresponding E2E test script diffs exist
