# TODO Prioritization Criteria

## When to Apply

When prioritizing TODO or backlog items.

## Constraints

Evaluate each item on 3 axes and determine overall priority:

### Axis 1: Direct Value (existing criteria)

Follow project-specific criteria. Conversion correctness is highest priority, further divided into 3 severity tiers (see `conversion-correctness-priority.md`):

1. Silent semantic changes (highest priority)
2. Compile errors
3. Unsupported syntax

### Axis 2: Leverage (synergy)

Does solving this issue also resolve or simplify other issues?

- Identify groups of issues sharing the same foundation/pattern
- Solving "foundation-building" issues first reduces effort for downstream issues
- Consider whether multiple small issues can be batched into a single PRD

### Axis 3: Propagation Prevention (tech debt accumulation)

Does leaving this issue unresolved cause accumulating negative impact on future development?

- Will future features require workarounds for this unresolved issue?
- After workarounds accumulate, does fixing the original issue require N-fold more changes?
- Conversely, isolated problems (no impact on other development) are safe to defer

### Axis 4: Temporal Scope Trajectory

Predict how the fix scope changes over time if deferred, and use this to determine development order.

- **Expanding (prioritize)**: As other development progresses, the number of locations requiring changes for this issue increases. Example: a foundational type conversion rule bug — each new feature adds more code affected by the bug, inflating the fix scope
- **Shrinking (safe to defer)**: As other development progresses, the fix scope naturally decreases. Example: upstream type inference improvements may eliminate the need for downstream workarounds
- **Stable (decide by other axes)**: Fix scope does not change over time. Determine order using Axes 1–3

Concrete examples:
- Hardcoded list in mutability.rs (I-335): Each new class method added increases detection misses → **Expanding** → Prioritize
- `never` → `Infallible` (I-328): A single line in generator/types.rs, unrelated to other development → **Stable** → Decide by other axes
- typeof narrowing (I-334): Type inference improvements may simplify resolve_typeof_match fixes → **Shrinking** → Safe to defer

### Integrated Judgment

- High direct value alone is not sufficient for prioritization. An Important issue with high propagation risk may deserve higher priority than an isolated Critical issue with low propagation risk, improving overall development efficiency
- **Temporal scope trajectory overrides other axes when decisive**: An issue with expanding scope should be prioritized even over a higher-severity issue with stable scope, because delay compounds the cost
- Explicitly state the reasoning for judgments (e.g., "Deferred I-XX because: impact is limited to Y and does not propagate to other development")
- When stating reasoning, include the temporal trajectory assessment (e.g., "I-XX: scope is expanding because...")
