# Mapped Types Analysis: Current Handling and Requirements for Full Support

**Date**: 2026-03-28 (updated 2026-03-31)
**Status**: Partially implemented
**Scope**: How TypeScript mapped types are currently handled and what's needed for correct conversion

> **2026-03-31 更新**:
> - B-2（`62fa279`）で identity mapped type（`{ [K in keyof T]: T[K] }`）の検出と `T` への簡約を実装済み。Pattern 1（Simplify）は解消
> - C-0〜C-4 で `resolve_struct_fields`、expected type 基盤、型引数推論、TypeRegistry 改善が完了。高度なパターンの基盤は整備済み
> - 残りの高度なパターン（P2: key remapping, P3: nested key remap, P4: conditional value, P5: array indexed access）は未実装

---

## Executive Summary

Mapped types (`{ [K in keyof T]: V }`) are TypeScript's mechanism for dynamically generating object types from another type's keys. The current implementation in `ts_to_rs` applies a **universal fallback**: all mapped types are converted to `HashMap<String, V>` (line 159-170 in `src/pipeline/type_converter/mod.rs`).

This fallback is **semantically incorrect** for most mapped type patterns found in Hono:
- **5 mapped type errors** in the error analysis (4.4% of 114 total errors)
- Multiple patterns with **indexed access**, **key remapping**, and **conditional filtering**
- Several patterns with **nested mapped types** that cannot be flattened to HashMap semantics

---

## Current Implementation: HashMap Fallback

### Code Location

**File**: `/home/kyohei/ts_to_rs/src/pipeline/type_converter/mod.rs` (lines 161-177)

```rust
TsType::TsMappedType(mapped) => {
    // Try identity simplification: { [K in keyof T]: T[K] } → T
    if let Some(simplified) = try_simplify_identity_mapped_type(mapped) {
        return Ok(simplified);
    }
    // Fallback: treat mapped types as HashMap<String, V>
    let value_type = mapped
        .type_ann
        .as_ref()
        .map(|ann| convert_ts_type(ann, synthetic, reg))
        .transpose()?
        .unwrap_or(RustType::Any);
    Ok(RustType::Named {
        name: "HashMap".to_string(),
        type_args: vec![RustType::String, value_type],
    })
}
```

### What's Extracted

- **Identity detection** (B-2 追加): `try_simplify_identity_mapped_type` が `{ [K in keyof T]: T[K] }` パターンを検出し `T` に簡約
- **`type_ann`**: The value type of the mapped type (righthand side)
- **Not examined** (non-identity patterns):
  - `type_param` (the `K in keyof T` part)
  - `type_param_in` (optional indexing, `as` clause)
  - `readonly` modifier
  - `optional` modifier

### Limitations

1. **Single-valued**: All keys map to the same value type `V`
2. **Generic key type**: Always assumes `String` keys (no index signature keys)
3. **Loses structure**: Cannot represent specific property mappings
4. **Ignores constraints**: Key filtering via `as` clauses is discarded
5. **Ignores readonly/optional**: Modifiers are ignored

---

## Mapped Type Patterns in Hono (Real Error Cases)

### Pattern 1: Simple Simplify Type (Most Common)

**Source**: `/tmp/hono-src/src/utils/types.ts:89, 95, 98`

```typescript
// Example 1: Basic simplification
export type Simplify<T> = { [KeyType in keyof T]: T[KeyType] } & {}

// Example 2: Deep simplification for arrays
export type SimplifyDeepArray<T> = T extends any[]
  ? { [E in keyof T]: SimplifyDeepArray<T[E]> }
  : Simplify<T>

// Example 3: Recursive type transformation
export type InterfaceToType<T> = T extends Function 
  ? T 
  : { [K in keyof T]: InterfaceToType<T[K]> }
```

**Pattern Analysis**:
- **Type Parameter**: `K in keyof T` (iterates over all keys of `T`)
- **Value Type**: `T[K]` (indexed access to get type of each property)
- **Modifiers**: None
- **Nesting**: Values are recursive or derived from indexed access

**Semantics**:
```
Input:  { a: string; b: number }
Output: { a: string; b: number }  (same structure, different representation)
```

**Current Conversion**: ✅ **RESOLVED** (B-2) — `try_simplify_identity_mapped_type` が identity パターンを検出し `T` に簡約

---

### Pattern 2: Symbol Key Filtering (Conditional Key Remapping)

**Source**: `/tmp/hono-src/src/utils/types.ts:37`

```typescript
type OmitSymbolKeys<T> = { [K in keyof T as K extends symbol ? never : K]: T[K] }
```

**Pattern Analysis**:
- **Type Parameter**: `K in keyof T`
- **Key Remapping**: `as K extends symbol ? never : K` 
  - Filters out symbol keys
  - Keeps string keys
- **Value Type**: `T[K]`
- **Effect**: Removes symbol properties from object type

**Semantics**:
```
Input:  { name: string; [Symbol.hidden]: boolean; age: number }
Output: { name: string; age: number }  (symbol properties removed)
```

**Current Conversion**: `HashMap<String, T[K]>` ✗ **WRONG** - cannot express conditional filtering

---

### Pattern 3: Nested Mapped Types with Complex Key Remapping

**Source**: `/tmp/hono-src/src/types.ts:2290-2304`

```typescript
type MergeEndpointParamsWithPath<T extends Endpoint, SubPath extends string> = T extends unknown
  ? {
      input: T['input'] extends { param: infer _ }
        ? ...
        : T['input'] & {
            // Maps extracted keys, stripping braces, to a string-typed record
            param: {
              [K in keyof ExtractParams<SubPath> as K extends `${infer Prefix}{${infer _}}`
                ? Prefix
                : K]: string
            }
          }
      // ... more properties ...
    }
  : never
```

**Pattern Analysis**:
- **Nested Structure**: Mapped type within object literal
- **Complex Indexing**: `ExtractParams<SubPath>` is a conditional/recursive type
- **Key Remapping**: `as K extends `${infer Prefix}{${infer _}}` ? Prefix : K`
  - Matches template literals: `{id}` → `id`
  - Extracts prefix from path parameter names
- **Value Type**: `string` (literal)

**Semantics**:
```
Input:  ExtractParams<"/users/:id/posts/:postId"> 
        // → { "id": unknown; "postId": unknown }
Output: param: { id: string; postId: string }
        // (parameter names extracted from route pattern)
```

**Current Conversion**: `HashMap<String, string>` ✗ **WRONG** - loses parameter name mapping, loses path structure

---

### Pattern 4: Nested Mapped with Conditional Type in Values

**Source**: `/tmp/hono-src/src/validator/utils.ts:25-45`

```typescript
type InferInputInner<
  Output,
  Target extends keyof ValidationTargets,
  T extends FormValue,
> = SimplifyDeep<{
  [K in keyof Output]: IsLiteralUnion<Output[K], string> extends true
    ? Output[K]
    : IsOptionalUnion<Output[K]> extends true
      ? Output[K]
      : Target extends 'form'
        ? T | T[]
        : Target extends 'query'
          ? string | string[]
          : Target extends 'param'
            ? string
            : Target extends 'header'
              ? string
              : Target extends 'cookie'
                ? string
                : unknown
}>
```

**Pattern Analysis**:
- **Mapped Type**: `[K in keyof Output]`
- **Conditional Value**: `IsLiteralUnion<Output[K]> ? ... : (IsOptionalUnion<Output[K]> ? ... : ...)`
- **Nested Conditionals**: 6-level deep conditional based on `Target` parameter
- **Nesting Inside**: All within `SimplifyDeep<{ ... }>`

**Semantics**:
```
Input:  { orderBy: 'asc' | 'desc'; page: number; limit?: number }
        with Target = 'query'
Output: { orderBy: 'asc' | 'desc'; page: string | string[]; limit: string | string[] | undefined }
        (literal unions preserved, others converted to string/string[])
```

**Current Conversion**: `HashMap<String, (complex conditional type)>` ✗ **WRONG** - loses property-specific overrides

---

### Pattern 5: Array Indexed Access in Mapped Types

**Source**: `/tmp/hono-src/src/utils/types.ts:66`

```typescript
type JSONParsed<T, TError = bigint | ReadonlyArray<bigint>> = 
  T extends ReadonlyArray<unknown>
    ? { [K in keyof T]: JSONParsed<InvalidToNull<T[K]>, TError> }
    : ...
```

**Pattern Analysis**:
- **Applies to Arrays**: When `T` is `ReadonlyArray<U>`
- **Indexed Access**: `T[K]` where K is array index
- **Recursive**: Applies `JSONParsed` to each element
- **Type Parameter**: Array indices (0, 1, 2, ..., length-1)

**Semantics**:
```
Input:  [string, number | Date, undefined]
Output: [string, number, null]  (maps InvalidJSONValue → null)
```

**Current Conversion**: `HashMap<String, JSONParsed<T[K]>>` ✗ **WRONG** - loses tuple structure, treats as homogeneous map

---

## Why HashMap Fallback Fails for These Cases

### Issue 1: Loss of Property Identity

HashMap stores arbitrary key-value pairs; mapped types create **specific properties**.

```typescript
// TypeScript: specific properties
Simplify<{ a: string; b: number }> → { a: string; b: number }

// Current Rust: loses property names
HashMap<String, ???>  // What's the value type? Can't be both String and Number
```

### Issue 2: Key Filtering Cannot Be Expressed

The `as` clause in mapped types performs **conditional key remapping**, which has no HashMap equivalent.

```typescript
// TypeScript: conditional key filtering
OmitSymbolKeys<T> = { [K in keyof T as K extends symbol ? never : K]: T[K] }
// Removes all symbol properties

// HashMap: no filtering
HashMap<String, T[K]>  // Still includes symbol properties somehow (but we lost the info)
```

### Issue 3: Value Type is Not Uniform

When values are derived from indexed access (`T[K]`), different keys have different types.

```typescript
// TypeScript: each key has its own type
Simplify<{ a: string; b: number; c: boolean }>
// Property 'a' has type string
// Property 'b' has type number
// Property 'c' has type boolean

// HashMap: all values same type
HashMap<String, T[K]>  // What type is T[K]? Union of all possible types?
// Would be HashMap<String, string | number | boolean>
// But then you lose which property maps to which type
```

### Issue 4: Nested Structure Cannot Be Flattened

Mapped types can create nested hierarchies; HashMap is always flat.

```typescript
// TypeScript: nested structure
type JSONParsed<T extends ReadonlyArray<unknown>> = 
  { [K in keyof T]: JSONParsed<T[K]> }
// For input [string, { a: Date }]:
// { 0: string; 1: { a: number } }  (nested object)

// HashMap: must flatten
HashMap<String, JSONParsed<T[K]>>  // How do we represent nested object types?
```

---

## Required Rust Representations

To handle mapped types correctly, the converter needs to support:

### Option A: Structural Representation (Recommended)

Create an **anonymous struct** with **dynamically generated fields** based on the mapped type's source type.

```rust
pub enum RustType {
    // ... existing variants ...
    
    /// Mapped type: iteration over keys with conditional transformation
    MappedType {
        source_type: Box<RustType>,        // T in { [K in keyof T]: ... }
        value_transform: Box<RustType>,    // Result of transform expression
        key_filter: Option<String>,        // "as" clause condition (if any)
        readonly: bool,
        optional: bool,
    },
}
```

### Option B: Synthetic Struct Generation (Current Approach for Utilities)

For mapped types with **known source types**, generate a synthetic struct at type conversion time.

**Steps**:
1. Resolve the source type `T` to a `TypeDef::Struct`
2. Extract all fields from the source struct
3. Transform each field value according to the mapped type's expression
4. Apply key filtering (if `as` clause present)
5. Create a synthetic struct with transformed fields
6. Register in `SyntheticTypeRegistry`

**Example**:
```rust
fn convert_mapped_type(
    mapped: &TsMappedType,
    synthetic: &mut SyntheticTypeRegistry,
    reg: &TypeRegistry,
) -> Result<RustType> {
    // Step 1: Get source type T from type_param
    let source_type = convert_ts_type(&mapped.type_param_in, synthetic, reg)?;
    
    // Step 2: Resolve T to struct fields (if possible)
    let fields = match resolve_type_to_fields(&source_type, reg, synthetic) {
        Some(fields) => fields,
        None => {
            // Fallback to HashMap for unresolvable types
            return fallback_to_hashmap(mapped, synthetic, reg);
        }
    };
    
    // Step 3: Transform each field value
    let mut transformed_fields = Vec::new();
    for (key, value_type) in fields {
        let transformed_value = convert_ts_type(&mapped.type_ann, synthetic, reg)?;
        
        // Apply conditional transform if key_filter present
        if should_include_key(&key, mapped) {
            transformed_fields.push((key, transformed_value));
        }
    }
    
    // Step 4: Create synthetic struct
    create_synthetic_struct(transformed_fields, synthetic)
}
```

---

## Concrete Conversion Examples

### Example: OmitSymbolKeys Pattern

**TypeScript**:
```typescript
type OmitSymbolKeys<T> = { [K in keyof T as K extends symbol ? never : K]: T[K] }
```

**Application**:
```typescript
type Result = OmitSymbolKeys<{ name: string; [sym]: boolean; age: number }>
// Expected: { name: string; age: number }
```

**Current Rust Output**: 
```rust
type OmitSymbolKeys = HashMap<String, (string | boolean | number)>  // WRONG
```

**Correct Rust Output**:
```rust
pub struct OmitSymbolKeysResult {
    pub name: String,
    pub age: i32,
    // [sym] is filtered out by the "as K extends symbol ? never : K" clause
}
```

**Conversion Algorithm**:
1. Source type: `{ name: string; [sym]: boolean; age: number }`
2. Iterate fields: `[("name", String), (symbol, Boolean), ("age", i32)]`
3. Apply filter `K extends symbol ? never : K`:
   - `"name"` is not symbol → **keep** with type `String`
   - `symbol` is symbol → **filter out**
   - `"age"` is not symbol → **keep** with type `i32`
4. Create struct: `struct OmitSymbolKeysResult { name: String, age: i32 }`

---

### Example: Simplify Pattern

**TypeScript**:
```typescript
export type Simplify<T> = { [K in keyof T]: T[K] } & {}
```

**Application**:
```typescript
type Example = Simplify<{ a: string; b: number }>
// Expected: { a: string; b: number } (same, but "simplified" for editor display)
```

**Current Rust Output**:
```rust
type Simplify = HashMap<String, (string | number)>  // WRONG
```

**Correct Rust Output**:
```rust
pub struct SimplifyResult {
    pub a: String,
    pub b: i32,
}
```

**Conversion Algorithm**:
1. Source type: `{ a: string; b: number }`
2. Iterate fields: `[("a", String), ("b", i32)]`
3. No filter (no `as` clause)
4. Transform values: Each `T[K]` stays as-is
5. Create struct: `struct SimplifyResult { a: String, b: i32 }`

---

### Example: Nested Array Mapped Type

**TypeScript**:
```typescript
type ArrayMap<T extends any[]> = { [K in keyof T]: T[K] }
```

**Application**:
```typescript
type Example = ArrayMap<[string, number, boolean]>
// Expected: { 0: string; 1: number; 2: boolean } (numeric indices)
```

**Current Rust Output**:
```rust
type ArrayMap = HashMap<String, (string | number | boolean)>  // WRONG
// Loses index structure and numeric key identity
```

**Correct Rust Output**:
```rust
pub struct ArrayMapResult(String, i32, bool);  // Tuple struct
// Or
pub struct ArrayMapResult {
    pub field_0: String,
    pub field_1: i32,
    pub field_2: bool,
}
```

**Conversion Algorithm**:
1. Source type: `[string, number, boolean]` (array/tuple)
2. Extract types: `[String, i32, bool]`
3. Create tuple or struct with numbered fields
4. Preserve order and index relationship

---

## Impact on Hono Error Analysis

### Mapped Type Errors (残存: P2-P5 の 4 パターン)

> **2026-03-31 更新**: P1（Simplify/identity）は B-2 で解消済み。以下は残存エラー。エラー件数は最新ベンチマークで再計測が必要。

| Location | Pattern | Current Behavior | Correct Behavior |
|----------|---------|------------------|-----------------|
| `client/types.ts:352` | Simplify-like type | `HashMap<String, T>` | Struct with specific fields |
| `client/types.ts:371` | InterfaceToType | `HashMap<String, T[K]>` | Recursively transformed struct |
| `types.ts:2273` | MergeEndpointParams with key remap | `HashMap<String, V>` | Struct with remapped keys |
| `types.ts:2451` | ExtractSchemaForStatusCode | `HashMap<String, Extract<...>>` | Nested struct with filtered branches |
| `utils/types.ts:37` | OmitSymbolKeys | `HashMap<String, T[K]>` | Struct with filtered properties |

### Estimated Fix Impact

- **Eliminates 5 errors** from the error count
- **Estimated complexity**: High (requires new RustType variant and conversion logic)
- **Related TODOs**: 
  - I-200: TYPE_ALIAS_MAPPED_TYPE
  - I-219: TYPE_ALIAS_COND_TYPE (conditional types also need proper handling)

---

## Implementation Roadmap

### Phase 1: Infrastructure (Small)
- Add `RustType::MappedType` variant (or use synthetic struct approach)
- Extend `convert_mapped_type` to handle type resolution
- Add helper function `resolve_type_to_fields(RustType) -> Vec<(String, RustType)>`

### Phase 2: Key Types (Medium)
- Implement simple mapped types (`Simplify<T>`)
- Add support for indexed access in values (`T[K]`)
- Handle `keyof T` iteration

### Phase 3: Advanced Patterns (Medium)
- Implement `as` clause key filtering/remapping
- Support conditional value transforms
- Handle nested mapped types

### Phase 4: Edge Cases (Small)
- Support `readonly` modifier
- Support `optional` modifier
- Handle array/tuple index keys

---

## Summary Table: TsTypeRef Patterns and Failures

| Utility | Pattern | Params | Status | Reason for TsTypeRef Failure |
|---------|---------|--------|--------|------------------------------|
| `Record` | `Record<K, V>` | 2 (key type, value type) | ✓ Works | Simple type params, no nesting |
| `Partial` | `Partial<T>` | 1 | ✓ Works | Utility handler extracts fields |
| `Pick` | `Pick<T, K>` | 2 | ✓ Works | Second param is literal union |
| `Exclude` | `Exclude<T, U>` | 2 | ✗ Fails | No utility handler for Exclude |
| `Mapped Types` | `{ [K in keyof T]: V }` | N/A (special) | ✗ Fails | Type structure not resolved (fallback to HashMap) |
| Complex Generic | `InferInputInner<Output, Target, T>` | 3+ with conditionals | ✗ Fails | Nested conditionals in type parameters not evaluated |

---

## References

- **Current Implementation**: `/home/kyohei/ts_to_rs/src/pipeline/type_converter/mod.rs:159-170`
- **Utility Types Module**: `/home/kyohei/ts_to_rs/src/pipeline/type_converter/utilities.rs`
- **Type Conversion Tests**: `/home/kyohei/ts_to_rs/src/pipeline/type_converter/tests.rs`
- **Hono Patterns**: 
  - Simplify: `/tmp/hono-src/src/utils/types.ts:89-98`
  - OmitSymbolKeys: `/tmp/hono-src/src/utils/types.ts:37`
  - Complex Mapped: `/tmp/hono-src/src/types.ts:2290-2304`
  - Nested: `/tmp/hono-src/src/validator/utils.ts:25-45`
- **Error Analysis**: `/home/kyohei/ts_to_rs/report/hono-error-analysis-2026-03-25.md`
- **TODO**: I-200 (TYPE_ALIAS_MAPPED_TYPE), I-219 (TYPE_ALIAS_COND_TYPE)

