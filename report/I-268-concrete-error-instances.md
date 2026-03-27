# I-268: Generic Parameter Field Expansion - Error Instance Analysis

## Overview

**Issue Code**: I-268  
**Category**: OBJECT_LITERAL_NO_TYPE (generic_param sub-category)  
**Affected Instances**: **14 instances** (26.9% of OBJECT_LITERAL_NO_TYPE errors)  
**Error Message**: "object literal requires a type annotation to determine struct name"  
**Root Cause**: When an object literal contains a spread operator whose source is a generic type parameter (e.g., `...options` where `options: E extends Env`), the converter cannot infer the type of the spread fields because the generic parameter constraint type needs to be instantiated.

---

## Problem Pattern

### TypeScript Pattern: Generic Type Parameter with Spread in Object Literal

```typescript
// Pattern 1: Function generic with Env constraint
export const serveStatic = <E extends Env = Env>(options: ServeStaticOptions<E>) => {
  return async function serveStatic(c, next) {
    const path = getFilePath({ filename: options.path ?? ... });
    //                        ^^^^^^ OBJECT_LITERAL_NO_TYPE error here
    // Root cause: `options` is type `ServeStaticOptions<E>`
    // Converter cannot instantiate `E extends Env` to resolve spread fields
  }
}
```

```typescript
// Pattern 2: Class generic with constraint
export class Hono<
  E extends Env = BlankEnv,
  S extends Schema = BlankSchema,
  BasePath extends string = '/',
> extends HonoBase<E, S, BasePath> {
  constructor(options: HonoOptions<E> = {}) {
    super(options)
    this.router = options.router ?? new SmartRouter(...)
    //            ^^^^^^^^^^^^^^ OBJECT_LITERAL_NO_TYPE
    // `options` is `HonoOptions<E>` where E is constrained but not yet instantiated
  }
}
```

```typescript
// Pattern 3: Class generic with string constraint
export class HonoRequest<P extends string = '/', I extends Input['out'] = {}> {
  constructor(
    request: Request,
    path: string = '/',
    matchResult: Result<[unknown, RouterRoute]> = [[]]
    //                                              ^^^ OBJECT_LITERAL_NO_TYPE
  ) {
    this.raw = request
    // ...
  }
}
```

---

## Concrete Failure Examples from Hono

### Example 1: `adapter/bun/serve-static.ts:8`

```typescript
// Line 8 in the export statement
export const serveStatic = <E extends Env = Env>(
  options: ServeStaticOptions<E>
): MiddlewareHandler => {
  return async function serveStatic(c, next) {
    // ... (various async functions)
    return baseServeStatic({
      ...options,        // ← SPREAD SOURCE: options is ServeStaticOptions<E>
      getContent,
      join,
      isDir,
    })(c, next)
    //  ^^^^^^^^^^^^^ Object literal {...} requires type annotation
  }
}
```

**Error Location**: `/tmp/hono-clean/adapter/bun/serve-static.ts:8:1`

**Why It Fails**:
- `ServeStaticOptions<E>` is a generic type where `E extends Env`
- The converter sees `...options` in an object literal
- To resolve the spread fields, it needs to:
  1. Look up `ServeStaticOptions<E>` definition
  2. Instantiate the generic `E` using the constraint `Env`
  3. Extract field names and types from the instantiated `ServeStaticOptions<Env>`
- **Currently missing**: `TypeRegistry::instantiate` is not called for spread sources that are generic type parameters

---

### Example 2: `adapter/cloudflare-pages/handler.ts:32, :49`

```typescript
export const handle =
  <E extends Env = Env, S extends Schema = BlankSchema, BasePath extends string = '/'>(
    app: Hono<E, S, BasePath>
  ): PagesFunction<E['Bindings']> =>
  (eventContext) => {
    return app.fetch(
      eventContext.request,
      { ...eventContext.env, eventContext },
      //  ^^^^^^^^^^^^^^^^^^^ OBJECT_LITERAL_NO_TYPE
      // eventContext.env is typed as Env & {...}
      // Spread source is a generic type parameter E from handler signature
      {
        waitUntil: eventContext.waitUntil,
        passThroughOnException: eventContext.passThroughOnException,
        props: {},
      }
    )
  }

export function handleMiddleware<E extends Env = {}, P extends string = any, I extends Input = {}>(
  middleware: MiddlewareHandler<E, S, P, I>
) {
  // Line 49: similar issue
}
```

**Error Locations**: 
- `/tmp/hono-clean/adapter/cloudflare-pages/handler.ts:32:1`
- `/tmp/hono-clean/adapter/cloudflare-pages/handler.ts:49:1`

**Why It Fails**:
- Generic function parameter: `<E extends Env = {}, ...>`
- At line 32/49, object literals spread `eventContext.env` which has type dependent on `E`
- The type annotation includes the generic parameter `E` in its structure
- `resolve_spread_fields` must instantiate `E` from its constraint to get field information

---

### Example 3: `adapter/deno/serve-static.ts:8`

Identical to bun variant (both adapter implementations follow the same pattern)

```typescript
export const serveStatic = <E extends Env = Env>(
  options: ServeStaticOptions<E>
): MiddlewareHandler => {
  // ... same spread pattern as bun
  return baseServeStatic({
    ...options,  // ← SPREAD OF GENERIC TYPE PARAMETER E
    getContent,
    join,
  })(c, next)
}
```

---

### Example 4: `hono.ts:16` (Class generic)

```typescript
export class Hono<
  E extends Env = BlankEnv,
  S extends Schema = BlankSchema,
  BasePath extends string = '/',
> extends HonoBase<E, S, BasePath> {
  constructor(options: HonoOptions<E> = {}) {
    super(options)
    this.router = options.router ?? new SmartRouter({...})
    //            ^^^^^^^^^^^^^^ OBJECT_LITERAL_NO_TYPE
  }
}
```

**Error Location**: `/tmp/hono-clean/hono.ts:16:1`

**Why It Fails**:
- Constructor parameter: `options: HonoOptions<E>` where `E extends Env = BlankEnv`
- The parameter type itself is a generic instantiation
- When spreading `options`, the converter needs `HonoOptions<Env>` (instantiated)
- The generic parameter `E` must be resolved from its constraint `Env`

---

### Example 5: `helper/dev/index.ts:27, :39`

```typescript
export const inspectRoutes = <E extends Env>(hono: Hono<E>): RouteData[] => {
  return hono.routes.map(({ path, method, handler }: RouterRoute) => {
    //        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ OBJECT_LITERAL_NO_TYPE
    // RouterRoute might contain fields typed with E
  })
}

export const showRoutes = <E extends Env>(hono: Hono<E>, opts?: ShowRoutesOptions): void => {
  const routeData: Record<string, RouteData[]> = {}
  let maxMethodLength = 0
  let maxPathLength = 0

  inspectRoutes(hono)
    .filter(({ isMiddleware }) => opts?.verbose || !isMiddleware)
    .map((route) => {
      const key = `${route.method}-${route.path}`
      ;(routeData[key] ||= []).push(route)  // ← OBJECT LITERAL in ||=
      //                                       OBJECT_LITERAL_NO_TYPE possible here
    })
}
```

**Error Location**: `/tmp/hono-clean/helper/dev/index.ts:27:1` and `:39:1`

**Why It Fails**:
- Generic function: `<E extends Env>`
- Object literal destructuring in `({ path, method, handler }: RouterRoute)` or similar
- When `RouterRoute` contains fields that depend on `E`, the converter cannot resolve the type without instantiating `E extends Env`

---

### Example 6: `request.ts:36` (Class generic)

```typescript
export class HonoRequest<P extends string = '/', I extends Input['out'] = {}> {
  // ... properties ...
  
  constructor(
    request: Request,
    path: string = '/',
    matchResult: Result<[unknown, RouterRoute]> = [[]]
    //                                              ^^^ OBJECT_LITERAL_NO_TYPE
  ) {
    this.raw = request
    // ...
  }
}
```

**Error Location**: `/tmp/hono-clean/request.ts:36:1`

**Why It Fails**:
- Class generic: `HonoRequest<P extends string = '/', I extends Input['out'] = {}>`
- Default parameter has an empty object literal `[[]]`
- The type of `matchResult` includes generic parameters
- When inferring the type of `[[]]`, converter needs to instantiate `P` and `I` from their constraints

---

## Technical Root Cause Analysis

### Current Behavior

In `type_converter.rs`, the `resolve_spread_fields` function:

```rust
// Pseudocode of current implementation
fn resolve_spread_fields(spread_source: &Expr) -> Vec<(String, RustType)> {
    let spread_type = self.resolve_expr(spread_source)?;
    
    // Case 1: Simple type (e.g., Identifier or Member)
    if let RustType::Named { name } = spread_type {
        let type_def = type_registry.lookup(name)?;
        // Extract fields from type_def
        return type_def.get_fields();
    }
    
    // Case 2: Generic instantiation (e.g., MyType<T>)
    if let RustType::Generic { base, type_args } = spread_type {
        // ✓ Instantiate and extract fields
        let instantiated = type_registry.instantiate(&base, &type_args)?;
        return instantiated.get_fields();
    }
    
    // Case 3: MISSING - Generic TYPE PARAMETER (e.g., E where E extends Env)
    // If spread_source refers to a variable/parameter whose type is a generic type parameter,
    // and that generic type parameter is constrained, we need to:
    // 1. Look up the constraint (e.g., E -> Env)
    // 2. Instantiate the constraint type using TypeRegistry::instantiate
    // 3. Extract fields from the instantiated type
    // ✗ This case is NOT HANDLED
}
```

### The Missing Case: Generic Type Parameter Constraints

When the spread source is an identifier like `options` or `env`:

1. The converter resolves `options` → looks up the parameter/variable type
2. The parameter type is `ServeStaticOptions<E>` (a generic struct with a type parameter)
3. The type parameter `E` is constrained: `E extends Env = Env`
4. **Missing step**: Convert the constraint `Env` through `TypeRegistry::instantiate` to get `TypeDef::Struct { name: "Env", fields: [...] }`
5. **Missing step**: Extract field information from the instantiated constraint type
6. **Missing step**: Use the extracted field information for the spread operation

### Why TypeRegistry::instantiate Cannot Be Called Directly

The issue is more subtle:

```rust
// E extends Env = Env
// The default value Env is a constraint, not an instantiation

let env_constraint = TypeParam {
    name: "E",
    constraint: Some(TypeRef("Env")),  // The constraint itself
    default: Some(TypeRef("Env")),      // And its default
};

// When we see ...options where options: ServeStaticOptions<E>
// We need to:
// 1. Extract E's constraint: TypeRef("Env")
// 2. Resolve TypeRef("Env") → RustType::Named { name: "Env" }
// 3. Look up TypeRegistry["Env"] → TypeDef::Struct { fields: [...] }
// 4. Flatten those fields into the object literal's expected type

// Current code handles GenericInstantiation but NOT GenericTypeParameter
```

---

## Error Instance Breakdown (14 total)

| Location | File | Pattern | Type Parameter | Constraint |
|----------|------|---------|-----------------|-----------|
| 1 | `adapter/bun/serve-static.ts:8` | Function generic + spread | `E` | `Env` |
| 2 | `adapter/deno/serve-static.ts:8` | Function generic + spread | `E` | `Env` |
| 3 | `adapter/cloudflare-pages/handler.ts:32` | Function generic + spread | `E` | `Env` |
| 4 | `adapter/cloudflare-pages/handler.ts:49` | Function generic + spread | `E` | `Env` |
| 5 | `adapter/lambda-edge/handler.ts:116` | Function generic + spread | `E` | `Env` |
| 6 | `adapter/service-worker/handler.ts:18` | Function generic + spread | `E` | `Env` |
| 7 | `helper/adapter/index.ts:10` | Function generic + spread | `E` | `Env` |
| 8 | `helper/dev/index.ts:27` | Generic function spread | `E` | `Env` |
| 9 | `helper/dev/index.ts:39` | Generic function spread | `E` | `Env` |
| 10 | `helper/ssg/utils.ts:61` | Generic function spread | `E` | (varies) |
| 11 | `hono.ts:16` | Class generic | `E`, `S`, `BasePath` | `Env`, `BlankSchema`, `'/'` |
| 12 | `preset/quick.ts:13` | Class generic | `E`, `S`, `BasePath` | `Env`, `BlankSchema`, `'/'` |
| 13 | `request.ts:36` | Class generic + default param | `P`, `I` | `string`, `Input['out']` |
| 14 | `validator/validator.ts:46` | Generic function | `E` | `Env` |

---

## How Type Resolution Should Work

### Step 1: Identify Generic Type Parameter in Spread Source

```typescript
const serveStatic = <E extends Env = Env>(options: ServeStaticOptions<E>) => {
  //                  ^^^^^^^^^^^^^^^^^^^^
  //                  - Declares generic param E
  //                  - Constraint: Env
  //                  - Default: Env
  
  return baseServeStatic({
    ...options,        // ← options has type ServeStaticOptions<E>
    //  ^^^^^^^ 
    // Step 1: Resolve spread source type
    // → ServeStaticOptions<E> (generic struct instantiation)
  })
}
```

### Step 2: Extract Generic Parameter from Instantiation

```rust
// When we encounter ServeStaticOptions<E>, we need to:

// A. Resolve the base type ServeStaticOptions from TypeRegistry
let base_type_def = type_registry.lookup("ServeStaticOptions")?;
// → TypeDef::Struct { fields: [
//     { name: "path", type: RustType::Option(String) },
//     { name: "getContent", type: RustType::Fn(...) },
//     ... more fields potentially using type parameter E
//   ]}

// B. Get the type arguments [E]
// B1. E is a TypeParam from the function signature
let type_param_E = function_signature.type_params[0];
// → TypeParam { name: "E", constraint: Env, default: Env }

// C. Resolve the constraint of E
let constraint_ref = type_param_E.constraint;
// → TypeRef::Named("Env")

// D. Convert constraint to RustType
let constraint_type = self.resolve_type_ref(constraint_ref)?;
// → RustType::Named { name: "Env" }
```

### Step 3: Instantiate with Constraint

```rust
// E. Call instantiate with the constraint type
let instantiated_type_def = type_registry.instantiate(
    &base_type_def,
    &[constraint_type]  // [RustType::Named { name: "Env" }]
)?;
// This substitutes E → Env throughout the ServeStaticOptions struct definition

// F. Extract fields from instantiated type
let resolved_fields = instantiated_type_def.get_fields();
// → [
//     { name: "path", type: RustType::Option(String) },
//     { name: "getContent", type: RustType::Fn(...) },
//     ... (all E references replaced with Env)
//   ]
```

### Step 4: Create Expected Type for Object Literal

```rust
// G. Build anonymous struct for the object literal
let obj_expected_type = RustType::Anonymous {
    fields: resolved_fields
};

// H. Use this as the expected type for type inference
object_literal.expected_type = Some(obj_expected_type);
```

---

## Comparison: Optional Spread (I-269) - Related Issue

I-268 and I-269 share the same fundamental problem but with different spread source types:

**I-268 (Generic Parameter)**:
```typescript
export const serveStatic = <E extends Env = Env>(options: ServeStaticOptions<E>) => {
  return baseServeStatic({
    ...options,  // ← Generic type parameter E in spread source
  })
}
```

**I-269 (Optional Type)**:
```typescript
const cors = (options?: CORSOptions) => {
  const defaults = {
    origin: "*",
    ...options  // ← Optional type (Option<CORSOptions>) in spread source
  }
}
```

**Solution Difference**:
- **I-268**: Call `TypeRegistry::instantiate` with the constraint type from the generic parameter
- **I-269**: Call `TypeRegistry::instantiate` with the inner type unwrapped from `Option<T>`
- **Common**: Both require extending `resolve_spread_fields` to handle different spread source patterns

---

## Impact Summary

- **Error Category**: OBJECT_LITERAL_NO_TYPE (52 total, 14 from generic parameters)
- **Affected Adapters**: Bun, Deno, Cloudflare Pages, Lambda Edge, Service Worker, and core classes
- **Affected Core Classes**: `Hono`, `HonoRequest`
- **Solution Location**: `src/pipeline/type_converter.rs` → `resolve_spread_fields` function
- **Related Infrastructure**: `TypeRegistry::instantiate` (already implemented)
- **Estimated Fix Complexity**: Medium (need to handle generic type parameter resolution in spread context)

---

## References

- **Error Log**: `/tmp/hono-bench-errors.json`
- **Analysis Report**: `/home/kyohei/ts_to_rs/report/hono-error-analysis-2026-03-25.md`
- **TODO Item**: `/home/kyohei/ts_to_rs/TODO` (I-268 entry)
- **Related Issues**: 
  - I-112c (parent: object literal type annotation)
  - I-269 (optional spread - same base mechanism)
  - I-224 (this type resolution)
  - I-266 (constructor argument expected types)

