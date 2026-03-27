---
name: correctness-audit
description: Thorough audit of conversion logic and test correctness, PRD-ifying issues found. Used as a periodic quality gate
user-invocable: true
---

# Conversion Correctness Audit

## Trigger

- User requests a correctness check or audit
- After a major feature addition cycle (5+ PRDs completed)

## Actions

Execute the following 3 investigations **in parallel**, saving results as a report in `report/`.

### 1. Type Conversion Accuracy Audit

Target files: `src/transformer/types/`, `src/ir.rs`, `src/generator/types.rs`

Verify the following for all type mappings:

- **Type equivalence**: Does the TS type correctly map to the Rust type? No information lost or added?
- **Compilability**: Can the generated Rust code compile with rustc?
- **Edge cases**: Do type combinations (special types in unions, nested generics, etc.) break anything?

Specific check items:
- Validity of each `TsKeywordTypeKind` → `RustType` mapping
- All union/intersection patterns (nullable, non-nullable, compound)
- Generics and type parameter propagation
- Behavior differences between type annotation positions vs type alias positions

### 2. Statement/Expression Semantics Audit

Target files: `src/transformer/statements/`, `src/transformer/expressions/`, `src/transformer/functions/`, `src/transformer/classes.rs`, `src/generator/statements.rs`, `src/generator/expressions.rs`

Verify the following for all conversion patterns:

- **Control flow preservation**: Do break/continue/return/throw operate in the correct scope?
- **Expression type safety**: Are generated expressions compilable? (type mismatches, method availability)
- **Runtime behavior equivalence**: Differences in panic vs NaN, mutability, ownership, etc.
- **Edge cases**: Nested structures, compound patterns, implicit type conversions

### 3. Test Quality Audit

Target files: `src/**/tests.rs`, `tests/integration_test.rs`, `tests/compile_test.rs`, `tests/snapshots/`

Verify the following for all tests:

- **Expected value accuracy**: Can the expected Rust code actually compile? Are the semantics correct?
- **Assertion strength**: Are there tests using only `matches!()` or `is_ok()` without verifying content?
- **Missing tests**: Do normal cases, error cases, and boundary value tests exist for each conversion pattern?
- **Snapshot validity**: Is the code in snapshots skipped in compile_test correct?
- **Tests verify what they should**: Do test names match actual verification content?

### 4. Report Creation

Save findings to `report/conversion-correctness-audit.md`. The report must:

- Include the base commit
- Classify problems by severity (Critical / High / Medium)
- Assign IDs to each problem with specific code locations (file:line_number)
- If a previous audit exists, record changes from last time (resolved, new, unchanged)

### 5. PRD Creation

For all discovered Critical / High problems:

- Check if a related PRD already exists in backlog
- If not, create a PRD per `/prd-template` and place in `backlog/`
- Insert into `plan.md` execution order
- Record Medium problems in `TODO` (PRD creation is optional)

## Prohibited

- Reading only some files and reporting "confirmed the whole thing"
- Reporting problems based only on speculation or generalities (support with specific code locations)
- Missing problems reported in the previous audit (reference the previous report and produce diffs)
- Reporting "no issues" and stopping (explicitly judge OK/NG for every conversion path)
- Judging test expected values as "correct because tests pass" (independently verify the expected values themselves)

## Verification

- `report/conversion-correctness-audit.md` is created/updated
- Verification results are documented for all type mappings and statement/expression conversion patterns
- All discovered Critical / High problems exist as PRDs in `backlog/`
- Diffs from previous audit results are recorded (not required for first audit)
