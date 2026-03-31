# TODO Prioritization Criteria

## When to Apply

When prioritizing TODO or backlog items, reordering plan.md batches, or selecting next work.

## Core Principle

> "If we don't do this now, will the cost per unit of future development increase?"

Always address root causes, not surface symptoms. Prioritize based on the nature of the root cause (design flaw, reliability risk, etc.), not the visible symptom (compile error, unsupported syntax, etc.).

## Step 1: Root Cause Clustering

Do not prioritize individual issues by symptom. First group them by root cause.

- Identify issues sharing the same function, module, or design flaw
- Determine priority at the cluster level and address as a batch
- Standalone issues are treated as clusters of size 1

## Step 2: Priority Level Assignment

Classify each cluster into one of 4 levels. Priority order: L1 > L2 > L3 > L4.

### L1: Reliability Foundation

**If left unaddressed, all other development output becomes untrustworthy.**

Criteria (any of):
- **S1 (Silent semantic change)**: Code compiles but behaves differently from TypeScript. Tests may not detect it
- **Test infrastructure compromise**: Tests pass but quality is not guaranteed (e.g., E2E stdout comparison polluted by S1 bugs)

### L2: Design Foundation

**No immediate breakage, but the same class of problem recurs with every future development.**

Criteria (any of):
- **Responsibility violation / DRY violation**: Each feature addition propagates the same anti-pattern
- **Foundational logic deficiency**: Root cause blocking multiple downstream issues (e.g., narrowing infrastructure deficiency → 6+ issues blocked)
- **Lack of structural equivalence**: Name-by-occurrence instead of name-by-structure, correctness not guaranteed by design

### L3: Expanding Technical Debt

**Fix cost increases over time, but not as fundamental as L1/L2.**

Criteria (any of):
- **Blocker for other issues**: Resolving this unblocks downstream issues
- **Expanding fix scope**: Each new code addition increases affected locations
- **Gate issue**: A prerequisite for feature extensions

### L4: Localized Problem

**No impact on other development, fix cost does not change over time.**

Criteria:
- Impact limited to a specific syntax or pattern
- Not a prerequisite for any other issue
- Explicitly skipped or error-reported (unsupported syntax, etc.)

## Step 3: Ordering Within the Same Level

Within the same level, determine order by:

1. **Leverage**: Resolving this simplifies/eliminates N other issues → higher N goes first
2. **Expansion rate**: Fix cost increases proportionally with delay → faster expansion goes first
3. **Fix cost**: If the above are equal, smaller cost goes first (eliminate risk sooner)

## Prohibited

- Prioritizing based solely on surface symptoms (compile error / unsupported syntax)
- Ordering individual issues without root cause clustering
- Demoting L1/L2 issues to L3/L4 based on fix cost
- Deferring L2 issues because "effort is large"
