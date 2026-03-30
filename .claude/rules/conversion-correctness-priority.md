# Conversion Correctness Priority

## When to Apply

- When prioritizing conversion bugs or reordering TODO items
- When designing PRD conversion strategies (type fallback, approximation, erasure)
- When reviewing conversion code changes for semantic safety

## Constraints

Classify conversion problems into the following 3 tiers and always resolve higher-tier problems first:

1. **Silent semantic changes** — The compiler cannot detect these; they cause incorrect behavior at runtime. Most dangerous
   - Example: Non-literal switch case becomes a variable binding, always matching
   - Example: const doesn't become let mut, making a TS-mutable variable immutable in Rust
2. **Compile errors** — Conversion result is unusable, but the compiler detects the problem
   - Example: Missing `.to_string()` causes type mismatch error
   - Example: `NaN` not converted to `f64::NAN` causes undefined error
3. **Unsupported syntax** — Conversion itself is not performed; notified via error message or skip
   - Example: `in` operator unsupported, producing error output

Rationale: Generating Rust code with different semantics has no value. Compile errors are noticeable by developers, but silent semantic differences are not.

## PRD Design Requirement

Every PRD that changes type resolution or introduces type fallbacks must include a **semantic safety analysis** section that:

1. Lists all affected output patterns (function returns, field types, variable types)
2. Classifies each as Safe (compile error or identical behavior) or UNSAFE (silent semantic change)
3. Proves that no UNSAFE pattern exists, or redesigns to eliminate it

See `type-fallback-safety.md` for the detailed analysis procedure.

## Prohibited

- Prioritizing compile error fixes over silent semantic changes because "it's low effort"
- Deprioritizing compile errors over unsupported syntax because "the impact scope is wide"
- Determining priority by discovery order or ID order without evaluating problem severity (silent > compile error > unsupported)
- Introducing type fallbacks (Any, wider union, HashMap) without performing the 3-step safety analysis defined in `type-fallback-safety.md`
