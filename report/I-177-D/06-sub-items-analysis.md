# I-177-D: Sub-Items Natural Resolution Analysis

**Date**: 2026-04-25  
**Analyst**: Code search (thorough codebase trace)  
**Goal**: Determine whether I-177-D (TypeResolver suppression scope refactor, case-C) naturally resolves I-177 sub-items A, B, C.

## Executive Summary

**I-177-D does NOT naturally resolve A, B, or C.** All three sub-items are independent of the suppression scope refactor and require separate fixes:

- **I-177-A**: Let-wrap match form for else_block_pattern (architectural fix, ~20-30 LOC)
- **I-177-B**: Query order inconsistency in collect_expr_leaf_types (~5-10 LOC, **batch-viable**)
- **I-177-C**: Symmetric early-return narrow detection (logic fix, ~10-15 LOC)

**Recommended approach**: Option β — **I-177-D + B batched**, A and C deferred to separate PRDs post-I-177-D.

---

## Detailed Trace Analysis

### I-177-A: typeof/instanceof/OptChain × then_exit + else_non_exit × post-narrow

**Problem Statement** (from TODO):
- `if (typeof x === "string") return 0; else { console.log("ne"); } return x + 1;`
- Post-if `x` remains as outer enum (F64OrString) instead of narrowed String
- E0369: binary op on wrong type → IR shadow INV-2 violation

#### Code Path: try_generate_narrowing_match else_block_pattern

**Location**: `/home/kyohei/ts_to_rs/src/transformer/statements/control_flow.rs:502-518`

```rust
if else_body.is_some() && !complement_is_none {
    // Else block pattern: `match var { Pos(v) => { then }, Comp(v) => { else } }`
    let positive_arm = MatchArm {
        patterns: vec![positive_pattern],
        guard: None,
        body: positive_body,
    };
    let complement_arm = MatchArm {
        patterns: vec![complement_pattern],
        guard: None,
        body: complement_body,
    };
    return Ok(Some(vec![Stmt::Match {  // ← Bare match, no outer let-wrap
        expr,
        arms: vec![positive_arm, complement_arm],
    }]));
}
```

**Key observation**: This is a **bare `Stmt::Match`** without IR shadow (no `let x = match x { ... }`). 

Compare with early-return forms (lines 389-397, 433-441, 491-499):
```rust
// Early-return form: WRAPPED in let
return Ok(Some(vec![Stmt::Let {
    mutable: false,
    name: var_name,
    ty: None,
    init: Some(Expr::Match { /* ... */ }),
}]));
```

The let-wrap is crucial: it **rebinds the outer variable to the narrowed type** after the match, threading the narrow into post-if scope.

Without let-wrap (bare match):
- Narrowed type bindings exist **inside each match arm only**
- Post-match scope sees `x` as the **declared type (outer union)**, not the narrowed type
- Post-if `return x + 1` sees F64OrString, not String → E0369

#### Why I-177-D Does NOT Fix I-177-A

I-177-D suppression scope refactor:
- Changes **when** `ClosureCapture` events suppress `narrowed_type` queries
- Narrows suppression from enclosing_fn_body → post-if only
- Affects `FileTypeResolution::narrowed_type(var_name, position)` lookup semantics

The else_block_pattern bare match issue is **orthogonal**:
- It's an **IR emission form choice** (bare match vs. let-wrap), not a narrowed_type lookup
- The suppression scope change doesn't affect whether the match is bare or wrapped
- Even with narrowed_type suppression removed entirely, bare match post-scope would still see outer type

#### Fix Required

Change else_block_pattern from bare match to let-wrap form:

```rust
// After fix: wrapped in let like early-return cases
return Ok(Some(vec![Stmt::Let {
    mutable: false,
    name: var_name.clone(),
    ty: None,
    init: Some(Expr::Match {
        expr: Box::new(expr),
        arms: vec![positive_arm, complement_arm],
    }),
}]));
```

**Scope**: Independent fix in `try_generate_narrowing_match`  
**Effort**: ~20-30 LOC  
**Batch Viable**: No (separate architectural decision, not suppression scope)

#### Resolution Answer

**I-177-A: INDEPENDENT FIX REQUIRED** ✗

---

### I-177-B: collect_expr_leaf_types query order inconsistency

**Problem Statement** (from TODO):
- `collect_expr_leaf_types` at `return_wrap.rs:413` queries `expr_type` first / `narrowed_type` fallback
- Transformer general path (`get_expr_type`) queries `narrowed_type` first
- Inconsistency causes synthetic union return-wrap to miss typeof narrow

#### Code Path 1: return_wrap.rs collect_expr_leaf_types

**Location**: `/home/kyohei/ts_to_rs/src/transformer/return_wrap.rs:410-425`

```rust
leaf => {
    let swc_span = leaf.span();
    let span = Span::from_swc(swc_span);
    let ty = match type_resolution.expr_type(span) {  // ← expr_type FIRST
        ResolvedType::Known(ty) => Some(ty.clone()),
        ResolvedType::Unknown => {
            // Also check narrowed_type for Ident expressions
            if let ast::Expr::Ident(ident) = leaf {
                type_resolution
                    .narrowed_type(ident.sym.as_ref(), ident.span.lo.0)  // ← narrowed_type FALLBACK
                    .cloned()
            } else {
                None
            }
        }
    };
    out.push(ReturnLeafType { ty, span: (swc_span.lo.0, swc_span.hi.0) });
}
```

Query order: **expr_type → narrowed_type**

#### Code Path 2: Transformer get_expr_type (general path)

**Location**: `/home/kyohei/ts_to_rs/src/transformer/expressions/type_resolution.rs:39-57`

```rust
pub(crate) fn get_expr_type(&self, expr: &ast::Expr) -> Option<&'a RustType> {
    // Ident 式の場合、narrowed_type を優先参照（型ナローイング後の型）
    if let ast::Expr::Ident(ident) = expr {
        if let Some(narrowed) = self
            .tctx
            .type_resolution
            .narrowed_type(ident.sym.as_ref(), ident.span.lo.0)  // ← narrowed_type FIRST
        {
            return Some(narrowed);
        }
    }
    match self
        .tctx
        .type_resolution
        .expr_type(Span::from_swc(expr.span()))  // ← expr_type FALLBACK
    {
        ResolvedType::Known(ty) => Some(ty),
        ResolvedType::Unknown => None,
    }
}
```

Query order: **narrowed_type → expr_type**

#### The Inconsistency

- `collect_expr_leaf_types` (return wrap): expr_type first
- `get_expr_type` (Transformer general): narrowed_type first

For an Ident in typeof-narrowed context:
- If expr_type returns Unknown, both paths eventually query narrowed_type → OK
- If expr_type returns a stale/union type, return_wrap uses it; Transformer gets narrowed_type → **INCONSISTENCY**

Symptom: Return type wrapping in narrow context misses the narrow type, uses synthetic union instead.

#### Why I-177-D Does NOT Fix I-177-B

I-177-D changes **when narrowed_type is suppressed** (by closure-capture), not the **query order**.

After I-177-D:
- Suppression scope narrows to post-if only
- More narrowed_type lookups succeed overall
- BUT if expr_type returns a value before narrowed_type is queried, the stale type is still used

The fix is purely **query order reordering**, independent of suppression scope.

#### Fix Required

Reorder `collect_expr_leaf_types` to match `get_expr_type` pattern:

```rust
leaf => {
    let swc_span = leaf.span();
    let span = Span::from_swc(swc_span);
    let ty = if let ast::Expr::Ident(ident) = leaf {
        // Ident: try narrowed_type FIRST
        if let Some(narrowed) = type_resolution.narrowed_type(ident.sym.as_ref(), ident.span.lo.0) {
            Some(narrowed.clone())
        } else {
            match type_resolution.expr_type(span) {
                ResolvedType::Known(ty) => Some(ty.clone()),
                ResolvedType::Unknown => None,
            }
        }
    } else {
        // Non-Ident: expr_type only
        match type_resolution.expr_type(span) {
            ResolvedType::Known(ty) => Some(ty.clone()),
            ResolvedType::Unknown => None,
        }
    };
    out.push(ReturnLeafType { ty, span: (swc_span.lo.0, swc_span.hi.0) });
}
```

**Scope**: `return_wrap.rs`, ~10 LOC  
**Effort**: Low (straightforward reorder)  
**Risk**: Minimal (aligns with existing Transformer pattern)  
**Batch Viable**: **YES** (independent, trivial, aligns with case-C's "unified narrow query")

#### Resolution Answer

**I-177-B: INDEPENDENT FIX REQUIRED, BATCH-VIABLE WITH I-177-D** ✗ (natural) / ✓ (batch)

---

### I-177-C: Symmetric early-return narrow detection (then_exit XOR else_exit)

**Problem Statement** (from TODO):
- `if (x !== null) <non-exit>; else return;` — then non-exit + else exit (missing)
- Truthy `if (x) <non-exit>; else return;` — truthy + then non-exit + else exit (missing)
- Current `detect_early_return_narrowing` only triggers for `then_exits && !else_exits`
- Need symmetric `(then_exits XOR else_exits) && useful narrow`

#### Code Path: TypeResolver::visit_if_stmt early-return detection

**Location**: `/home/kyohei/ts_to_rs/src/pipeline/type_resolver/visitors.rs:733-740`

```rust
// Early return narrowing: post-if scope sees the *complement* of
// `if_stmt.test` whenever the then-block always exits AND post-if
// remains reachable through the else-side path.
//
// ... comments describe two cases (a) and (b) ...
let then_exits = stmt_always_exits(&if_stmt.cons);
let else_exits = if_stmt.alt.as_deref().is_some_and(stmt_always_exits);
if then_exits && !else_exits {  // ← ONLY THIS CONDITION
    if let Some(block_end) = self.current_block_end {
        let if_end = if_stmt.cons.span().hi.0;
        detect_early_return_narrowing(&if_stmt.test, if_end, block_end, self);
    }
}
```

Current logic: **then_exits && !else_exits**

This covers:
- `if (test) { return 0; }` with no else (else implicitly non-exit)
- `if (test) { return 0; } else { use_x(); }` (else non-exit)

Missing case (symmetric):
- `if (test) { use_x(); } else { return 0; }` where else exits but then doesn't
  - Post-if is reachable ONLY via the then-branch (else exits)
  - Post-if should see **the complement narrowing** (not the test narrowing)

Example:
```typescript
if (x !== null) { console.log(x); }  // then non-exit
else { return 0; }                   // else exit
return x + 1;  // Should narrow to String, but currently stays Option<string>
```

OR (negated test):
```typescript
if (!x) { console.log("x is falsy"); }  // then non-exit
else { return 0; }                       // else exit
return x + 1;  // Should narrow to String (x is truthy), but currently stays Option<string>
```

#### Implementation in detect_early_return_narrowing

**Location**: `/home/kyohei/ts_to_rs/src/pipeline/narrowing_analyzer/guards.rs:247-396`

The function handles **only the primary direction** (test positive case):
```rust
// if (typeof x === "string") { return; }
// → x is NOT string after → complement type
```

To support symmetric direction, we need:
```rust
// if (typeof x === "string") { use_x; } else { return; }
// → x IS string after (reached from then-non-exit path, opposite direction)
// → apply PRIMARY type (not complement)

// if (!typeof x === "string") { use_x; } else { return; }
// → x is NOT string after (reached from negated-then-non-exit path)
// → apply complement type
```

#### Why I-177-D Does NOT Fix I-177-C

I-177-D suppression scope refactor affects:
- **When** narrowed_type queries are suppressed (by closure-capture detection)
- The filtering logic in `FileTypeResolution::is_var_closure_reassigned`

I-177-C is about:
- **When** early-return narrows are detected (guard detection phase)
- The condition in `visit_if_stmt` that triggers `detect_early_return_narrowing`

These operate on **different pipelines**:
- I-177-D: type_resolution.rs (narrowed_type suppression, late Transformer phase)
- I-177-C: visitors.rs + narrowing_analyzer/guards.rs (early-return detection, TypeResolver phase)

Case-C suppression scope refactor doesn't affect when early-return narrows are detected. The two are independent.

#### Fix Required

1. Change condition in `visit_if_stmt` (visitors.rs:735):
   ```rust
   if then_exits != else_exits {  // XOR: one must exit, other must not
       // Detect early-return narrowing (direction handled by call)
   }
   ```

2. Extend `detect_early_return_narrowing` to handle opposite direction:
   - When else exits (then doesn't): apply primary narrow in post-if scope
   - When then exits (else doesn't): apply complement narrow in post-if scope (current)

**Scope**: visitors.rs + guards.rs, ~10-15 LOC  
**Effort**: Medium (logic change in two files, requires symmetric direction handling)  
**Batch Viable**: No (separate guard detection logic, not suppression scope)

#### Resolution Answer

**I-177-C: INDEPENDENT FIX REQUIRED** ✗

---

## Sub-Items Resolution Matrix

| Sub-Item | Issue | Current Impl Path | Root Cause Category | I-177-D Impact | Fix Type | Est. LOC | Batch? | Sequential Prereq? |
|----------|-------|-------------------|-------------------|----------------|----------|---------|--------|-------------------|
| **A** | Bare match in else_block_pattern (no post-scope narrow) | `control_flow.rs:502-518` | Emission form choice | None (orthogonal) | IR form change (let-wrap) | 20-30 | No | None (after D) |
| **B** | Query order: expr_type before narrowed_type in return_wrap | `return_wrap.rs:410-425` | Query order inconsistency | None (suppression scope independent) | Reorder (narrowed first) | 5-10 | **YES** | None (sibling to D) |
| **C** | Missing symmetric early-return narrow (else_exits case) | `visitors.rs:733-740 + guards.rs:247-396` | Condition coverage gap | None (different pipeline) | Guard detection logic | 10-15 | No | None (after D) |

---

## Batching Recommendation

### Recommended: Option β (I-177-D + B Batched)

**Composition**:
- **I-177-D**: Suppression scope refactor (main PRD)
- **I-177-B**: Query order fix in return_wrap (batched)
- **I-177-A, I-177-C deferred**: Separate PRDs post-I-177-D

**Rationale**:

1. **I-177-D is prerequisite** — architectural suppression scope change affects the semantic foundation for all narrowing queries. It's the logical first step.

2. **I-177-B is trivial & synergistic** — The query order inconsistency will still exist after I-177-D, but batch-fixing it establishes "unified narrowed_type-first query strategy" (consistent across return_wrap and Transformer general path). Adding ~10 LOC to I-177-D is zero risk, zero effort.

3. **I-177-A and I-177-C are substantial & independent**:
   - I-177-A: IR form decision (bare match → let-wrap) orthogonal to suppression scope
   - I-177-C: Guard detection logic (early-return condition) on separate pipeline
   - Both benefit from I-177-D foundation but are not naturally resolved by it
   - Both introduce new test coverage needs; batch with D would over-scope the PRD

### Implementation Sequence

**Phase 1: I-177-D + B (single PRD)**
```
1. I-177-D main: Suppress ClosureCapture events to post-if scope only
   - FileTypeResolution::is_var_closure_reassigned scope narrowing
   - Update enclosing_fn_body filtering
   - Tests: closure-reassign suppression scope isolation

2. I-177-B piggyback: Reorder return_wrap query
   - collect_expr_leaf_types: narrowed_type first
   - Tests: typeof-narrowed return type inclusion
```

**Phase 2: I-177 (separate PRD post-D)**
```
- Mutation propagation core + A + C as sub-tasks
```

---

## Summary

| Aspect | Finding |
|--------|---------|
| **Natural resolution of A** | No — Let-wrap form is orthogonal to suppression scope |
| **Natural resolution of B** | No — Query order independent, but batch-viable |
| **Natural resolution of C** | No — Early-return detection on separate pipeline |
| **Recommended approach** | Option β: I-177-D + B batched, A/C deferred |
| **Key insight** | I-177-D is architectural foundation but not a "silver bullet" — A/B/C are separate concerns across IR emission, query order, and guard detection |

