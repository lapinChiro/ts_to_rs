---
paths:
  - "Cargo.toml"
---

# Dependency Version Management

## When to Apply

When adding or updating dependencies in `Cargo.toml`.

## Constraints

1. Verify the latest version of the dependency being added/updated
2. Specify the latest version
3. Verify compatibility with existing dependencies (`cargo check` must pass)
4. If incompatible, align related dependencies to their latest versions as well

## Prohibited

- Specifying an older version without justification
- Pinning versions with `=` (allow compatible ranges)

## Related Rules

| Rule | Relation |
|------|----------|
| [pipeline-integrity.md](pipeline-integrity.md) | Cargo.toml は pipeline の前提条件 (build-time integrity) |
