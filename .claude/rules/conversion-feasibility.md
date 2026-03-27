# Conversion Feasibility Criteria

## When to Apply

- When about to judge a TypeScript syntax/pattern as "cannot be auto-converted" or "cannot be expressed in Rust"
- When using "difficult to express in Rust" as a factor in TODO/backlog prioritization
- When considering deferral or exclusion in plan.md due to technical difficulty
- When designing conversion strategies in PRD design sections

## Core Principle

This project aims to build a **perfect TypeScript → Rust transpiler**. Judging any TS pattern as "impossible to convert" is equivalent to denying the project's feasibility. "Difficult" is never a reason to defer — it is a trigger for deeper design investigation.

## Constraints

- Conversion feasibility is determined by **purpose alignment**, not **syntax matching**. No direct Rust equivalent does not mean auto-conversion is impossible
- Before concluding "cannot be expressed in Rust", always follow these 3 steps:
  1. **Identify the purpose**: What is the TS code trying to achieve? (type safety, code generation, constraint expression, etc.)
  2. **Research Rust alternatives**: Investigate idioms/patterns achieving the same purpose (trait, associated type, proc macro, serde, generics, enum, etc.)
  3. **Design conversion strategy**: Concretely design the conversion path from TS syntax to the Rust alternative
- Only after all 3 steps, if no strategy is found, record as "no auto-conversion method found at this time". Never write "impossible"
- **In TODO/backlog prioritization**: "Implementation cost is high" and "Rust expression is difficult" are NOT valid reasons to lower priority or defer. Priority is determined solely by the axes in TODO (direct value, leverage, propagation prevention)
- **In plan.md**: Items must not be excluded due to technical difficulty. If design investigation is needed, schedule the investigation — do not skip the item

## Prohibited

- Definitive statements like "cannot be expressed in Rust's type system" or "auto-conversion is impossible"
- Judging feasibility based on absence of syntax-level correspondence
- Concluding "too complex" without analyzing the purpose
- Directly porting TS-specific constraint workarounds to Rust and judging "cannot be ported" — if the constraint doesn't exist in Rust, the code is unnecessary
- Using "difficult in Rust" or "high cost" as grounds for lowering TODO priority, deferring items, or excluding from plan.md
- Describing any TS feature as "difficult to express in Rust" in TODO, plan.md, or PRD without providing concrete conversion strategy candidates
