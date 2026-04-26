# I-177-D Investigation: FileTypeResolution::narrowed_type Closure-Reassign Suppression Scope

**Investigation Date**: 2026-04-25  
**Scope**: Very thorough codebase analysis  
**Target**: Architectural refactor of `narrowed_type(var, position)` suppression scope for PRD I-177-D

---

## 1. `narrowed_type` Definition and Signature

### Location
**File**: `/home/kyohei/ts_to_rs/src/pipeline/type_resolution.rs`  
**Lines**: 173–199 (implementation), 188–199 (signature and body)

### Full Signature and Implementation

```rust
// type_resolution.rs:188–199
pub fn narrowed_type(&self, var_name: &str, position: u32) -> Option<&RustType> {
    if self.is_var_closure_reassigned(var_name, position) {
        return None;
    }
    self.narrow_events
        .iter()
        .filter_map(NarrowEvent::as_narrow)
        .rfind(|n| {
            n.var_name == var_name && n.scope_start <= position && position < n.scope_end
        })
        .map(|n| n.narrowed_type)
}
```

### Documentation

From the doc comment (lines 173–187):

```rust
/// Gets the narrowed type for a variable at a given byte position.
///
/// Returns the innermost (most specific) narrowing that applies,
/// or `None` if no narrowing is active for this variable at this position.
///
/// Only consults [`NarrowEvent::Narrow`] variants; `Reset` /
/// `ClosureCapture` events carry no type and are skipped.
///
/// I-144 T6-2 closure-reassign suppression (I-169 follow-up: position-
/// aware): when `var_name` is reassigned inside any closure body whose
/// `enclosing_fn_body` contains `position`, the narrow is suppressed
/// and `None` is returned. Callers fall back to the variable's declared
/// `Option<T>` type, which matches the Transformer's narrow-guard
/// suppression so reads see a consistent type and the `coerce_default`
/// wrapper can be applied at arithmetic / string-concat sites.
```

### Key Semantics

1. **Innermost scope wins**: Uses `rfind()` to get the most specific (rightmost) narrowing event
2. **Scope membership test**: `n.scope_start <= position && position < n.scope_end` (half-open interval)
3. **Closure-reassign suppression**: Early return on `is_var_closure_reassigned(var_name, position)` (line 189)
4. **Event filtering**: Only `NarrowEvent::Narrow` variants are considered (skips Reset/ClosureCapture via `as_narrow()`)

---

## 2. Current Closure-Reassign Suppression Scope

### Suppression Mechanism: is_var_closure_reassigned

**File**: `/home/kyohei/ts_to_rs/src/pipeline/type_resolution.rs`  
**Lines**: 253–264

```rust
pub fn is_var_closure_reassigned(&self, var_name: &str, position: u32) -> bool {
    self.narrow_events.iter().any(|e| match e {
        NarrowEvent::ClosureCapture {
            var_name: v,
            enclosing_fn_body,
            ..
        } => {
            v == var_name && enclosing_fn_body.lo <= position && position < enclosing_fn_body.hi
        }
        _ => false,
    })
}
```

### Boundary Definition

The suppression boundary is defined by `NarrowEvent::ClosureCapture.enclosing_fn_body: Span`, which is set by the analyzer:

**File**: `/home/kyohei/ts_to_rs/src/pipeline/narrowing_analyzer.rs`  
**Lines**: 142–162 (analyze_function)

```rust
pub fn analyze_function(body: &ast::BlockStmt, params: &[&ast::Pat]) -> AnalysisResult {
    let mut result = AnalysisResult::default();
    analyze_stmt_list(&body.stmts, &mut result);
    // I-169 T6-2 follow-up: collect closure-capture events with scope-aware
    // candidate enumeration + walker shadow tracking. `enclosing_fn_body`
    // is set to the function body's span so downstream
    // `is_var_closure_reassigned(name, position)` queries filter events by
    // function-scope membership (multi-fn isolation P1).
    let candidates = closure_captures::collect_outer_candidates(params, &body.stmts);
    let captured =
        closure_captures::collect_closure_capture_pairs_for_candidates(&body.stmts, &candidates);
    let enclosing = crate::pipeline::type_resolution::Span::from_swc(body.span);
    result.closure_captures = captured
        .into_iter()
        .map(|var_name| NarrowEvent::ClosureCapture {
            var_name,
            enclosing_fn_body: enclosing,
        })
        .collect();
    result
}
```

### The Problem: "enclosing fn body 全体" is Too Broad

**Current behavior (PROBLEM)**:
- When a closure reassigns `x`, a `ClosureCapture` event is emitted with `enclosing_fn_body = [fn_start, fn_end)`
- **Every** call to `narrowed_type(x, pos)` for **any** `pos` inside `[fn_start, fn_end)` returns `None`
- This includes positions inside the `if` condition's consequent block, where the narrow is still valid

**Architectural issue** (from design-decisions.md, line 661–668):

> **T7-3 cell で architectural cohesion gap を発見**: `FileTypeResolution::narrowed_type(var, position)`
> の closure-reassign suppression scope が enclosing fn body 全体で broad すぎ、
> cons-span 内 (if-body 内、narrow が valid な scope) も含めて narrow を suppress
> → IR shadow form (cons-span 内 x: T) と TypeResolver Option<T> view の不整合 →

**Root cause of I-161 T7-3 cell failure** (from e2e_test.rs:1831–1834):

```
`FileTypeResolution::narrowed_type(var, position)` の closure-reassign
suppression scope が enclosing fn body 全体で broad すぎ、cons-span
内 (if-body 内、narrow が valid な scope) も含めて narrow を
suppress すること (TODO I-177-D 参照)
```

---

## 3. ClosureCapture Event: Definition, Push, Consumption

### Event Definition

**File**: `/home/kyohei/ts_to_rs/src/pipeline/narrowing_analyzer/events.rs`  
**Lines**: 67–89

```rust
/// A closure captures the outer narrow for `var_name`.
///
/// Emitted when the closure either reads or reassigns a variable that is
/// narrowed in the enclosing scope. Consumers drive narrow suppression
/// (`FileTypeResolution::is_var_closure_reassigned`) from this event,
/// using [`enclosing_fn_body`](Self::ClosureCapture::enclosing_fn_body)
/// for position-aware narrow suppression queries.
ClosureCapture {
    /// Variable captured by the closure.
    var_name: String,
    /// Span of the enclosing function body where this capture event was
    /// detected.
    ///
    /// Defines the position range (`[lo, hi)`) within which this event is
    /// observable for narrow suppression queries
    /// (`FileTypeResolution::is_var_closure_reassigned`,
    /// `FileTypeResolution::narrowed_type`). The analyzer
    /// (`analyze_function(body, params)`) populates this with the function
    /// body's span passed to it. Multi-function scope isolation (I-169 P1)
    /// is structurally guaranteed by this field: a query at a position
    /// outside `enclosing_fn_body` does not match this event.
    enclosing_fn_body: Span,
},
```

### Where ClosureCapture is Pushed

**Single emission site** (by design):

**File**: `/home/kyohei/ts_to_rs/src/pipeline/narrowing_analyzer.rs`  
**Lines**: 150–160

The `analyze_function` free function creates `ClosureCapture` events and places them into `AnalysisResult.closure_captures`, which are then merged into `FileTypeResolution.narrow_events` during type resolution.

**Actual push to FileTypeResolution** happens in `TypeResolver`:

**File**: `/home/kyohei/ts_to_rs/src/pipeline/type_resolver/mod.rs` (implicit, via trait)  
The `TypeResolver` implements `NarrowTypeContext` trait (defined in `narrow_context.rs`), which receives closure_captures from the analyzer's `AnalysisResult` and merges them into the final `FileTypeResolution.narrow_events`.

### How suppress_scopes Field Works (if it existed)

**NOTE**: There is **no `suppress_scopes` field** in the current code. The term in the TODO refers to the broader architectural concept of "suppression scope boundaries."

Current architecture uses:
- **`NarrowEvent::ClosureCapture.enclosing_fn_body`** as the suppression boundary span
- **`narrowed_type()` early return** on `is_var_closure_reassigned()` check (line 189)

The "scope" is implicitly captured in the `Span` field, not as a separate field.

### Consumption Sites for ClosureCapture

1. **`FileTypeResolution::narrowed_type()`** (line 189): Suppresses narrow lookup if closure-reassign detected
2. **`FileTypeResolution::is_var_closure_reassigned()`** (line 253): Direct membership test
3. **Transformer** via `get_type_for_var()` (see section 5 below)

---

## 4. NarrowEvent::Reset: Definition and Unconsumed Status

### Definition

**File**: `/home/kyohei/ts_to_rs/src/pipeline/narrowing_analyzer/events.rs`  
**Lines**: 58–66

```rust
/// Narrow is invalidated at `position` due to `cause`.
Reset {
    /// Variable whose narrow is reset.
    var_name: String,
    /// Byte position of the operation causing the reset.
    position: u32,
    /// Classification of the reset cause (see [`ResetCause`]).
    cause: ResetCause,
},
```

### Current Status: Produced but Not Consumed

**TODO I-168 reference** (from design-decisions.md, mentioned in context):

The `Reset` event is produced by the narrowing analyzer (via `classifier::classify_reset_in_stmts`) to populate `EmissionHint` decisions (E1 vs E2a at `??=` sites), but **the events themselves are not consumed by the Transformer** for narrow-guard emission suppression.

**Why it exists but is unused for guards**:

From `narrowing_analyzer.rs` (lines 64–70):

```rust
//! - **T6**: The legacy `pre_check_narrowing_reset` + `has_narrowing_reset_in_stmts`
//!   transformer scanner is retired. `Transformer::try_convert_nullish_assign_stmt`
//!   consults [`AnalysisResult::emission_hints`] (populated by
//!   [`TypeResolver::collect_emission_hints`](crate::pipeline::type_resolver::TypeResolver))
//!   to pick between [`EmissionHint::ShadowLet`] (E1) and
//!   [`EmissionHint::GetOrInsertWith`] (E2a) at each `??=` site.
```

**What was planned (from design-decisions.md context)**:

The `Reset` event was intended to be a first-class citizen in the analyzer output for emission decisions, but the architecture settled on `EmissionHint` as the channel for `??=` dispatch. Reset events are internal to the classifier and flow into `EmissionHint` computation, not directly to `FileTypeResolution`.

**Current consumers**:
- `narrowing_analyzer::classifier::classify_nullish_assign()` uses reset classification to pick between `ShadowLet` vs `GetOrInsertWith`
- **NOT consumed** by the Transformer for general narrow suppression (only `ClosureCapture` is used for that)

---

## 5. Callers of narrowed_type: Transformer General Path vs Return-Wrap Path

### All Call Sites

```
/home/kyohei/ts_to_rs/src/transformer/return_wrap.rs:419
/home/kyohei/ts_to_rs/src/transformer/expressions/type_resolution.rs:25
/home/kyohei/ts_to_rs/src/transformer/expressions/type_resolution.rs:45
```

Plus test-only sites in `type_resolution.rs`.

### Path 1: Transformer General Path (Expression Type Resolution)

**File**: `/home/kyohei/ts_to_rs/src/transformer/expressions/type_resolution.rs`

#### Site 1a: `get_type_for_var()` (line 20–32)

```rust
pub(crate) fn get_type_for_var(
    &self,
    name: &str,
    span: swc_common::Span,
) -> Option<&'a RustType> {
    if let Some(narrowed) = self.tctx.type_resolution.narrowed_type(name, span.lo.0) {
        return Some(narrowed);
    }
    match self.tctx.type_resolution.expr_type(Span::from_swc(span)) {
        ResolvedType::Known(ty) => Some(ty),
        ResolvedType::Unknown => None,
    }
}
```

**Purpose**: Variable-name-based type resolution (for cases where the AST Expr reference is unavailable but var name and span are known).

**Consumers**: Used by narrowing guard emission and closure capture analysis code that doesn't have direct expr access.

#### Site 1b: `get_expr_type()` (line 39–58)

```rust
pub(crate) fn get_expr_type(&self, expr: &ast::Expr) -> Option<&'a RustType> {
    // Ident 式の場合、narrowed_type を優先参照（型ナローイング後の型）
    if let ast::Expr::Ident(ident) = expr {
        if let Some(narrowed) = self
            .tctx
            .type_resolution
            .narrowed_type(ident.sym.as_ref(), ident.span.lo.0)
        {
            return Some(narrowed);
        }
    }
    match self
        .tctx
        .type_resolution
        .expr_type(Span::from_swc(expr.span()))
    {
        ResolvedType::Known(ty) => Some(ty),
        ResolvedType::Unknown => None,
    }
}
```

**Purpose**: Expression type resolution with narrowed-type priority for Ident expressions.

**Callers**: Used throughout expression conversion to determine whether a variable has a narrowed type.

**Context**: This is the **general Transformer path** — used for binary operations, assignments, function calls, etc. where the expression type (including narrowing) is needed to make emission decisions.

### Path 2: Return-Wrap Path (collect_expr_leaf_types)

**File**: `/home/kyohei/ts_to_rs/src/transformer/return_wrap.rs`  
**Lines**: 391–432

```rust
/// the type from `FileTypeResolution`.
fn collect_expr_leaf_types(
    expr: &ast::Expr,
    type_resolution: &FileTypeResolution,
    out: &mut Vec<ReturnLeafType>,
) {
    match expr {
        // Ternary: recurse into both branches
        ast::Expr::Cond(cond) => {
            collect_expr_leaf_types(&cond.cons, type_resolution, out);
            collect_expr_leaf_types(&cond.alt, type_resolution, out);
        }
        // Parenthesized: unwrap
        ast::Expr::Paren(paren) => {
            collect_expr_leaf_types(&paren.expr, type_resolution, out);
        }
        // Note: SeqExpr (comma operator) は IR にサポートされておらず,
        // Transformer で変換エラーになるため collect しない。
        //
        // Leaf expression: resolve type from TypeResolver
        leaf => {
            let swc_span = leaf.span();
            let span = Span::from_swc(swc_span);
            let ty = match type_resolution.expr_type(span) {
                ResolvedType::Known(ty) => Some(ty.clone()),
                ResolvedType::Unknown => {
                    // Also check narrowed_type for Ident expressions
                    if let ast::Expr::Ident(ident) = leaf {
                        type_resolution
                            .narrowed_type(ident.sym.as_ref(), ident.span.lo.0)
                            .cloned()
                    } else {
                        None
                    }
                }
            };
            out.push(ReturnLeafType {
                ty,
                span: (swc_span.lo.0, swc_span.hi.0),
            });
        }
    }
}
```

**Purpose**: Multi-return-value typing for function return position (I-144 T6-3 guard, per comment at return_wrap.rs:150).

**Context**: This collects return leaf types from ternary / parenthesized expressions to determine if a union return wrapper is needed. It calls `narrowed_type()` as a fallback when `expr_type()` is `Unknown` for an Ident.

**Called by**: `build_return_type_variants()` at line 303, which is invoked during return statement transformation.

### Distinction Summary

| Path | File | Function | Purpose | Scope |
|------|------|----------|---------|-------|
| **General** | `type_resolution.rs` | `get_expr_type()`, `get_type_for_var()` | All expression type queries | Transformer general expressions (binary ops, assignments, calls, etc.) |
| **Return-wrap** | `return_wrap.rs` | `collect_expr_leaf_types()` | Multi-return union typing | Return statement transformation (line 413-424 fallback for Ident Unknown) |

---

## 6. Design-Decisions Document References

### I-144 Control-Flow Narrowing Analyzer (design-decisions.md section)

**File**: `/home/kyohei/ts_to_rs/doc/handoff/design-decisions.md`  
**Lines**: 386–556

Key sections referenced:

#### Section 1: 2-Channel Architecture (lines 392–427)

Establishes the split between:
- `NarrowEvent::Narrow` (scope-based override via TypeResolver)
- `EmissionHint` (per-stmt dispatch in Transformer for `??=`)
- DU analysis (tag-based pattern matching)

**YAGNI principle**: Only actually-populated enum variants (no dead code for unimplemented narrowing types).

#### Section 5: Closure Reassign Policy A (lines 466–484)

```
Closure が外側 narrow 変数を reassign するケース (C-2a/b/c) の Rust emission は
**Policy A (FnMut + `let mut`)** を default に採用
```

Describes closure capture algorithm in `narrowing_analyzer/closure_captures.rs` with scope-aware shadow tracking.

#### Section T7-3 Cohesion Gap Finding (lines 661–682)

**THE CORE ISSUE** (lines 661–668):

```
`FileTypeResolution::narrowed_type(var, position)` の closure-reassign
suppression scope が enclosing fn body 全体で broad すぎ、
cons-span 内 (if-body 内、narrow が valid な scope) も含めて narrow を suppress
→ IR shadow form (cons-span 内 x: T) と TypeResolver Option<T> view の不整合
```

**Proposed I-177-D fix** (lines 679–682):

```
**architectural fix を I-177-D PRD に委譲**: `narrowed_type` suppression scope
refactor (案 C: cons-span 内 narrow 保持 + post-if scope のみ suppress) で
IR shadow form と TypeResolver narrow が agree → narrow-T-shape body と
Option-shape body 両方で works
```

#### I-177 Umbrella (lines 698–720)

```
- **I-177**: narrow emission v2 umbrella (mutation propagation defect、T6-3
  inherited) + sub-items A/B/C (typeof/instanceof/OptChain × post-narrow / query
  順序 / symmetric direction) + sub-item D (suppression scope refactor、T7
  architectural fix の本体)
```

**Sub-item D is I-177-D** (this investigation's target).

---

## 7. Public/Private API Boundaries in TypeResolver

### `FileTypeResolution` Public API

**File**: `/home/kyohei/ts_to_rs/src/pipeline/type_resolution.rs`

#### Public Methods

```rust
pub fn empty() -> Self                                      // line 148
pub fn expr_type(&self, span: Span) -> &ResolvedType      // line 163
pub fn expected_type(&self, span: Span) -> Option<&RustType> // line 169
pub fn narrowed_type(&self, var_name: &str, position: u32) -> Option<&RustType> // line 188
pub fn is_mutable(&self, var_id: &VarId) -> Option<bool>  // line 202
pub fn any_enum_override(&self, var_name: &str, position: u32) -> Option<&RustType> // line 210
pub fn is_du_field_binding(&self, var_name: &str, position: u32) -> bool // line 224
pub fn emission_hint(&self, stmt_lo: u32) -> Option<EmissionHint> // line 236
pub fn is_var_closure_reassigned(&self, var_name: &str, position: u32) -> bool // line 253
```

#### Public Fields (Immutable after construction)

```rust
pub expr_types: HashMap<Span, ResolvedType>       // line 86
pub expected_types: HashMap<Span, RustType>       // line 94
pub narrow_events: Vec<NarrowEvent>               // line 101
pub var_mutability: HashMap<VarId, bool>         // line 107
pub du_field_bindings: Vec<DuFieldBinding>        // line 114
pub any_enum_overrides: Vec<AnyEnumOverride>      // line 122
pub spread_fields: HashMap<Span, Vec<(String, RustType)>> // line 130
pub emission_hints: HashMap<u32, EmissionHint>    // line 143
```

### TypeResolver Internal API (src/pipeline/type_resolver/)

**Module structure**:

```
type_resolver/
├── mod.rs                 // Main TypeResolver struct + entry point
├── narrow_context.rs      // NarrowTypeContext trait impl
├── expressions.rs         // Expression type resolution
├── statements.rs          // Statement traversal
├── call_resolution.rs     // Function call type inference
├── expected_types.rs      // Expected-type propagation
├── du_analysis.rs         // Discriminated union narrowing
├── fn_exprs.rs           // Arrow/fn expr handling
├── helpers.rs            // Type utilities
├── visitors.rs           // AST visitors
└── emission_hints.rs     // EmissionHint collection
```

### Narrow-Related Boundaries

#### Inbound (to TypeResolver)

1. **`NarrowTypeContext` trait** (narrow_context.rs):
   - `lookup_var()` → reads scope stack (private resolver state)
   - `synthetic_enum_variants()` → reads synthetic registry (private)
   - `register_sub_union()` → writes to synthetic registry (private)
   - `push_narrow_event()` → writes to `result.narrow_events` (line 36)

2. **`analyze_function()` returns** `AnalysisResult`:
   - `emission_hints: HashMap<u32, EmissionHint>` (T6-1 dispatch)
   - `closure_captures: Vec<NarrowEvent>` (T6-2 suppression)

#### Outbound (from FileTypeResolution)

1. **Transformer** (`src/transformer/expressions/type_resolution.rs`):
   - `get_expr_type()` → calls `narrowed_type()` (line 45)
   - `get_type_for_var()` → calls `narrowed_type()` (line 25)
   - `is_var_closure_reassigned()` → calls closure-reassign check (line 85)

2. **Return wrapper** (`src/transformer/return_wrap.rs`):
   - `collect_expr_leaf_types()` → calls `narrowed_type()` (line 419)

### Trait Boundary: NarrowTypeContext

**File**: `/home/kyohei/ts_to_rs/src/pipeline/narrowing_analyzer/type_context.rs`

```rust
pub trait NarrowTypeContext {
    fn lookup_var(&self, name: &str) -> ResolvedType;
    fn synthetic_enum_variants(&self, enum_name: &str) -> Option<Vec<EnumVariant>>;
    fn register_sub_union(&mut self, member_types: &[RustType]) -> String;
    fn push_narrow_event(&mut self, event: NarrowEvent);
}
```

**Implementation** in TypeResolver:

**File**: `/home/kyohei/ts_to_rs/src/pipeline/type_resolver/narrow_context.rs`

```rust
impl<'a> NarrowTypeContext for TypeResolver<'a> {
    fn lookup_var(&self, name: &str) -> ResolvedType {
        TypeResolver::lookup_var(self, name)
    }
    fn synthetic_enum_variants(&self, enum_name: &str) -> Option<Vec<EnumVariant>> {
        self.synthetic
            .get(enum_name)
            .and_then(|def| match &def.item {
                Item::Enum { variants, .. } => Some(variants.clone()),
                _ => None,
            })
    }
    fn register_sub_union(&mut self, member_types: &[RustType]) -> String {
        self.synthetic.register_union(member_types)
    }
    fn push_narrow_event(&mut self, event: NarrowEvent) {
        self.result.narrow_events.push(event);  // ← line 36
    }
}
```

---

## 8. Architectural Decision Summary for I-177-D Design

### Current Suppression Model (PROBLEM)

**Boundary**: `enclosing_fn_body` span `[fn_start, fn_end)`

**Behavior**: Any position in `[fn_start, fn_end)` suppresses the narrow if a closure reassigns the variable anywhere in that function.

**Problem**: Over-suppression

- If-body narrow scope `[cons_start, cons_end)` is **subsumed entirely** into suppression
- IR `narrowed_type()` returns `None` for positions inside the if-body where the narrow **is actually valid**
- Transformer can't emit narrow-aware code (e.g., `x &&= 3`) because `get_expr_type()` sees `Option<T>` instead of `T`

### Proposed Solution (I-177-D, "案 C" from design-decisions.md:680–681)

**Refactor suppression scope from enclosing fn body to post-if position**:

```
cons-span 内 narrow 保持 + post-if scope のみ suppress
```

**Interpretation**:
1. **Inside if-body** (`[cons_start, cons_end)`): Keep the narrow active, don't suppress
2. **After if-body** (`[cons_end, fn_end)`): Suppress the narrow due to closure-reassign risk

**Benefit**: 
- IR shadow form (narrow-T-shape inside if-body) and TypeResolver narrow agree
- Both narrow-aware emission (arithmetic, concat) and closure-safe emission work correctly
- Fixes Scenario A regression (E0308 mismatch in return-without-?? bodies)

### Implementation Approach

**Key question**: How to distinguish post-if scope from if-body scope?

Current `NarrowEvent::ClosureCapture` has:
- `var_name: String`
- `enclosing_fn_body: Span`

**Options**:

1. **Modify ClosureCapture to carry additional scope info**:
   - Add `first_capture_position: u32` (earliest position of closure reassign)
   - Or add `if_body_bounds: Vec<Span>` (all if-bodies containing narrows for this var)

2. **Change suppression logic in narrowed_type()**:
   - Instead of binary "fn_body contains position", implement "position is post-narrow-scope"
   - Requires knowing the narrow scope boundaries from the narrow event itself (already available!)

3. **Two-level suppression**:
   - `is_var_closure_reassigned(var, pos)` returns a tri-state: `None` / `Suppress` / `SuppressPost`
   - `narrowed_type()` checks if position is inside the narrow's scope_end before suppressing

### References for Implementation

- Narrow scope bounds: `NarrowEvent::Narrow { scope_start, scope_end, ... }` (events.rs:46–56)
- Closure capture metadata: `NarrowEvent::ClosureCapture { enclosing_fn_body, ... }` (events.rs:74–89)
- Current suppression test: `test_narrowed_type_suppressed_when_closure_reassign_present()` (type_resolution.rs:419–449)
- Multi-fn isolation test: `narrowed_type_suppress_only_fires_inside_enclosing_fn_body()` (type_resolution.rs:476–510)
- Closure capture events emission: `analyze_function()` (narrowing_analyzer.rs:142–162)

---

## 9. Critical Test Cases Demonstrating Current Problem

### Test: I-161 T7-3 Cell (IGNORED, awaiting I-177-D)

**File**: `/home/kyohei/ts_to_rs/tests/e2e_test.rs`  
**Lines**: 1822–1848

```rust
#[test]
#[ignore = "I-161 T7-3 RED — narrow × `&&=` × closure-reassign の architectural \
           IR/TypeResolver cohesion gap、I-177-D 完了で GREEN 化見込み。...
           本 cell の root cause は \
           `FileTypeResolution::narrowed_type(var, position)` の closure-reassign \
           suppression scope が enclosing fn body 全体で broad すぎ、cons-span \
           内 (if-body 内、narrow が valid な scope) も含めて narrow を \
           suppress すること (TODO I-177-D 参照)。..."]
fn test_e2e_cell_i161_t7_3_and_closure_reassign() {
    run_cell_e2e_test("i161-i171", "cell-t7-3-and-closure-reassign");
}
```

**Problem**: The if-body contains `x &&= 3`, which needs to know that `x: f64` (narrowed), but `narrowed_type()` returns `None` due to the closure-reassign event suppressing the entire function body's narrowing.

### Unit Test: Multi-Function Isolation

**File**: `/home/kyohei/ts_to_rs/src/pipeline/type_resolution.rs`  
**Lines**: 476–510

```rust
#[test]
fn narrowed_type_suppress_only_fires_inside_enclosing_fn_body() {
    // I-169 P1 (matrix cell #3): when two Narrow events for `x` exist
    // in different functions and one has a ClosureCapture event, the
    // other's narrow must NOT be suppressed.
    use crate::pipeline::narrowing_analyzer::{NarrowTrigger, PrimaryTrigger};
    let mut resolution = FileTypeResolution::empty();
    // Function f at [0, 100): has Narrow event + ClosureCapture event.
    resolution.narrow_events.push(NarrowEvent::Narrow { /* ... */ });
    resolution.narrow_events.push(NarrowEvent::ClosureCapture {
        var_name: "x".to_string(),
        enclosing_fn_body: Span { lo: 0, hi: 100 },
    });
    // Function g at [200, 300): has Narrow event, NO ClosureCapture.
    resolution.narrow_events.push(NarrowEvent::Narrow { /* ... */ });

    // Query inside f (position 60): suppress → None
    assert!(resolution.narrowed_type("x", 60).is_none());
    // Query inside g (position 250): narrow fires normally → Some
    assert!(matches!(
        resolution.narrowed_type("x", 250),
        Some(RustType::F64)
    ));
}
```

This test validates **multi-function scope isolation** (I-169 P1 fix) but doesn't test **sub-function scope isolation** (the if-body vs post-if distinction that I-177-D needs).

---

## 10. Conclusion: Refactor Scope Summary

### What Needs to Change

1. **Suppression boundary from `[fn_start, fn_end)` to `[if_end, fn_end)`**
   - File: `src/pipeline/type_resolution.rs`, function `narrowed_type()`
   - Current: Early return on any `is_var_closure_reassigned()` match
   - Needed: Check if position is inside the narrow's scope before suppressing

2. **Optional: ClosureCapture event enrichment**
   - Current: `enclosing_fn_body: Span` only
   - Future option: Add first_reassign_position or if-body-scope list for clarity

3. **Test coverage expansion**
   - Current: Multi-fn isolation (I-169 P1) tested
   - Needed: Sub-fn scope isolation (if-body vs post-if) test

### Files Affected

- **Core change**: `src/pipeline/type_resolution.rs` lines 188–199
- **Test update**: Add new test case for if-body scope preservation
- **Potential refactoring**: `src/pipeline/narrowing_analyzer/events.rs` if enriching ClosureCapture

### Design-Decisions Document Reference

Already prepared in design-decisions.md (lines 661–682) under "T7-3 cell で architectural cohesion gap を発見" and "I-177 narrow emission v2 umbrella" sections.

---

## Appendix A: NarrowEvent Type Hierarchy

### Complete NarrowEvent Enum

```rust
pub enum NarrowEvent {
    /// Narrow is active for `var_name` across `[scope_start, scope_end)`.
    Narrow {
        var_name: String,
        scope_start: u32,
        scope_end: u32,
        narrowed_type: RustType,
        trigger: NarrowTrigger,
    },
    /// Narrow is invalidated at `position` due to `cause`.
    Reset {
        var_name: String,
        position: u32,
        cause: ResetCause,
    },
    /// A closure captures the outer narrow for `var_name`.
    ClosureCapture {
        var_name: String,
        enclosing_fn_body: Span,
    },
}
```

### NarrowTrigger (2-layer to prevent nesting)

```rust
pub enum NarrowTrigger {
    /// Direct narrow from a primary guard in the consequent / alternate scope.
    Primary(PrimaryTrigger),
    /// Narrow in the fall-through scope after an early-exiting primary guard.
    EarlyReturnComplement(PrimaryTrigger),
}

pub enum PrimaryTrigger {
    TypeofGuard(String),
    InstanceofGuard(String),
    NullCheck(NullCheckKind),
    Truthy,
    OptChainInvariant,
}
```

---

## Appendix B: Full List of References

### Code Files
- `/home/kyohei/ts_to_rs/src/pipeline/type_resolution.rs` (FileTypeResolution definition)
- `/home/kyohei/ts_to_rs/src/pipeline/type_resolver/narrow_context.rs` (NarrowTypeContext impl)
- `/home/kyohei/ts_to_rs/src/pipeline/type_resolver/expressions/type_resolution.rs` (get_expr_type, get_type_for_var)
- `/home/kyohei/ts_to_rs/src/transformer/return_wrap.rs` (collect_expr_leaf_types)
- `/home/kyohei/ts_to_rs/src/pipeline/narrowing_analyzer.rs` (analyze_function, AnalysisResult)
- `/home/kyohei/ts_to_rs/src/pipeline/narrowing_analyzer/events.rs` (NarrowEvent, NarrowTrigger)
- `/home/kyohei/ts_to_rs/src/pipeline/narrowing_analyzer/closure_captures.rs` (capture detection)
- `/home/kyohei/ts_to_rs/src/pipeline/narrowing_analyzer/guards.rs` (narrow event emission)
- `/home/kyohei/ts_to_rs/src/pipeline/narrowing_analyzer/classifier.rs` (reset classification)

### Documentation Files
- `/home/kyohei/ts_to_rs/doc/handoff/design-decisions.md` (I-144, I-161/I-171, I-177 decisions)
- `/home/kyohei/ts_to_rs/tests/e2e_test.rs` (I-161 T7-3 test with detailed TODO)

### Related PRDs (git history)
- I-144 (closed 2026-04-21): Control-flow narrowing analyzer baseline
- I-161 + I-171 (closed 2026-04-25): Truthy emission batch, discovered T7-3 cohesion gap
- I-177-D (current): Suppression scope refactor (architectural fix for I-161 T7-3)
- I-169 (follow-up): Position-aware multi-function scope isolation
- I-177 (umbrella): Narrow emission v2 (includes I-177-D as sub-item D)

