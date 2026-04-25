---
paths:
  - "src/transformer/**"
  - "src/generator/**"
  - "src/ir.rs"
---

# Pipeline Integrity

## When to Apply

When adding or modifying code related to the conversion pipeline (parser → transformer → generator).

## Constraints

- **IR must be represented as structured data**: Do not store display-formatted strings in IR types (`Item::*`, `RustType`, etc.). String formatting is the generator's responsibility
- **Maintain pipeline dependency direction**: Transformer produces IR. Generator consumes IR. Transformer must not import `crate::generator` (except in test code)
- **When adding new fields to IR, apply consistently across all Item variants**: For example, if adding `type_params` to `Item::Trait`, also add it with the same structured type to `Item::Struct`, `Item::Fn`, `Item::TypeAlias`
- **When implementing a new resolution mechanism (e.g., `instantiate`), write integration tests for usage sites**: Unit tests alone cannot detect integration gaps

## Prohibited

- Calling generator functions like `crate::generator::types::generate_type` from within the transformer
- Storing pre-formatted strings like `"T: Bound"` in `Vec<String>` and treating them as IR (use structs instead)
- Implementing a new method (e.g., `instantiate`) with only unit tests and no integration tests for the conversion pipeline

## Related Rules

| Rule | Relation |
|------|----------|
| [design-integrity.md](design-integrity.md) | Pipeline architecture の higher-level design consistency |
| [testing.md](testing.md) | Integration test の placement (`tests/*.rs`) と本ルールの "integration tests for usage sites" 要件 |
| [dependencies.md](dependencies.md) | Cargo.toml の build-time pipeline 前提条件 |
