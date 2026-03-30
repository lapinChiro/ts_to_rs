# Type Fallback Safety

## When to Apply

When implementing type resolution fallbacks — any code path where an exact TypeScript type cannot be resolved and an approximate/wider Rust type is substituted (e.g., `RustType::Any`, union over-approximation, `HashMap` fallback for mapped types).

## Core Principle

TypeScript-to-Rust transpilation must preserve observable behavior. A type fallback is safe **only if** the generated Rust code either:

1. **Behaves identically** to the TypeScript source at runtime, OR
2. **Fails to compile** (Rust compiler catches the type mismatch)

A fallback that **compiles but produces different runtime behavior** is a silent semantic change — the most dangerous class of bug (Tier 1 in `conversion-correctness-priority.md`).

## Mandatory Analysis Before Introducing Type Fallbacks

Before introducing any new type fallback (returning `RustType::Any`, a wider union, or a generic container like `HashMap`), perform this 3-step analysis:

### Step 1: Identify All Usage Sites

How will the fallback type be used in generated code?

- **Function return type**: Will callers receive `serde_json::Value` where they expect a concrete type?
- **Field type**: Will struct field access produce `Value::get()` instead of direct field access?
- **Variable type**: Will assignments, comparisons, or method calls operate on the fallback type?

### Step 2: Classify Each Usage Site

For each usage site, determine the outcome:

| Outcome | Classification | Action |
|---------|---------------|--------|
| Compile error (type mismatch) | **Safe** | Acceptable — developer is alerted |
| Identical runtime behavior | **Safe** | Acceptable — no semantic divergence |
| Different runtime behavior, silently compiles | **UNSAFE** | Must not introduce this fallback |

### Step 3: Document the Fallback

If the fallback is safe, document in the code:
- What the exact type should be (and why it can't be resolved)
- What the fallback type is
- Why the fallback is safe (which safety condition it satisfies)

## Common Safe Patterns

- **Error → Any**: Previously the file wasn't converted at all. No prior semantics exist to diverge from.
- **Error → wider union**: Same as above. Over-approximation causes compile errors when used in type-specific contexts.
- **Identity simplification** (e.g., `{ [K in keyof T]: T[K] }` → `T`): Semantically correct by definition.

## Common UNSAFE Patterns

- **Specific type → Any** when the specific type was previously resolved correctly: The generated code may silently accept values of the wrong type.
- **Narrowing loss**: If type narrowing previously produced a specific type and the fallback produces a wider type, code in the narrowed scope may silently operate on wrong types.
- **Fallback in value-producing context**: If the fallback type is used to construct values (not just type annotations), the constructed value may have different semantics.

## Prohibited

- Introducing a type fallback without performing the 3-step analysis above
- Returning `RustType::Any` as a "quick fix" without documenting why it's safe
- Assuming "it was an error before, so any output is an improvement" without verifying that the approximate output doesn't silently change behavior of SURROUNDING correctly-converted code
- Using `serde_json::Value` in contexts where it could silently satisfy type constraints via trait implementations (e.g., `impl Display for Value` produces JSON representation, not TypeScript's string coercion)
