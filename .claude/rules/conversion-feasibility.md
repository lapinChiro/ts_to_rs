---
paths:
  - "src/transformer/**"
---

# Conversion Feasibility Criteria

## When to Apply

Any situation where you are about to judge a TypeScript syntax/pattern as "cannot be auto-converted" or "cannot be expressed in Rust".

## Constraints

- Conversion feasibility is determined by **purpose alignment**, not **syntax matching**. The absence of a direct Rust equivalent for a TS syntax does not mean auto-conversion is impossible
- Before concluding "cannot be expressed in Rust", always follow these 3 steps:
  1. **Identify the purpose**: What is the TypeScript code trying to achieve? (type safety, code generation, constraint expression, etc.)
  2. **Research Rust alternatives**: Investigate Rust idioms/patterns that achieve the same purpose (trait, associated type, proc macro, serde, generics, enum, etc.)
  3. **Design conversion strategy**: Concretely design the conversion path from TS syntax to the Rust alternative
- Only after completing all 3 steps and still finding no conversion strategy, record it as "no auto-conversion method found at this time". Never write "impossible"

## Prohibited

- Making definitive statements like "cannot be expressed in Rust's type system" or "auto-conversion is impossible"
- Judging conversion as infeasible based on the absence of syntax-level correspondence
- Concluding "too complex, can't be done" without analyzing the purpose
- Attempting to directly port TS-specific constraint workaround code (IDE display improvements, `any` avoidance, etc.) to Rust and judging it as "cannot be ported". If the constraint doesn't exist in Rust, the code is unnecessary
