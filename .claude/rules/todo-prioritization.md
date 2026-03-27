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

### Integrated Judgment

- High direct value alone is not sufficient for prioritization. An Important issue with high propagation risk may deserve higher priority than an isolated Critical issue with low propagation risk, improving overall development efficiency
- Explicitly state the reasoning for judgments (e.g., "Deferred I-XX because: impact is limited to Y and does not propagate to other development")
