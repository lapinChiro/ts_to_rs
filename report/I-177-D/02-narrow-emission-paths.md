# I-177-D: Narrow-Related Emission Paths — Comprehensive Investigation

**Date**: 2026-04-25  
**Investigation Scope**: All narrow emission code paths in TS→Rust transpiler  
**Context**: PRD I-177-D involves TypeResolver suppression scope refactor; T7 patch reverted in `control_flow.rs`

---

## Executive Summary

Three narrow emission forms exist per TODO I-171 Layer 2:

- **F1 (cons-span shadow form)**: `if let Some(x) = x { body }`
- **F2 (always-exit Let-WRAP form)**: `let x = match x { Some(v) => v, _ => exit };`
- **F3 (simple match)**: `match x { Some(x) => then, None => else }`

Additionally, a **predicate form** (F4) exists: `if x.is_some() { ... }` (used only for non-Option types and when closure-reassign suppression prevents narrow).

This report traces all code paths that select these forms, identifies guard conditions, and documents closure-reassign suppression scope behavior.

---

## 1. `try_generate_narrowing_match` Definition & Dispatch

**File**: `/home/kyohei/ts_to_rs/src/transformer/statements/control_flow.rs:348-521`

### Entry Point
Called from `convert_if_stmt` (line 50-57) when:
1. Single narrowing guard exists (no compound `&&` chain)
2. `can_generate_if_let(guard)` returns `true` (pattern resolvable)
3. Else body is optional

### Dispatch Logic (Guard Kind)

The function uses `self.resolve_if_let_pattern(guard)` and `self.resolve_complement_pattern(guard)` to determine the narrowed type. The `guard` enum type (`NarrowingGuard`) has 4 variants:

| Guard Variant | Detection | Pattern Resolution |
|---|---|---|
| `NonNullish { is_neq }` | `x !== null` / `x === null` | For `Option<T>`: `Some(x)` positive, `None` complement |
| `Truthy { }` | `if (x)` bare ident | For `Option<T>`: `Some(x)` positive, `None` complement |
| `Typeof { type_name, is_eq }` | `typeof x === "type"` | For union enum: variant pattern |
| `InstanceOf { class_name }` | `x instanceof Foo` | For union enum variant |

### Output Emission Shapes (Lines 372–520)

#### Branch 1: Early Return (F2 Let-WRAP) — Lines 377–398

**Conditions**:
- `else_body.is_none()` — no else block
- `then_body` always exits (return/break/continue)
- `!complement_pattern.is_none_unit()` — complement is NOT `None`

**Output Shape**:
```rust
let var = match var {
    Positive(v) => { exit_body },
    Complement(v) => v
};
```

**Emission Code** (line 378–397):
```rust
let positive_arm = MatchArm {
    patterns: vec![positive_pattern.clone()],
    guard: None,
    body: positive_body,  // = then_body
};
let complement_arm = MatchArm {
    patterns: vec![complement_pattern.clone()],
    guard: None,
    body: vec![Stmt::TailExpr(Expr::Ident(var_name.clone()))],
};
return Ok(Some(vec![Stmt::Let {
    mutable: false,
    name: var_name,
    ty: None,
    init: Some(Expr::Match {
        expr: Box::new(expr),
        arms: vec![positive_arm, complement_arm],
    }),
}]));
```

**TypeResolver Query**: None (type inferred from match arms).

#### Branch 2: Early Return with Non-Exit Else (F2 Deep-Fix) — Lines 400–422

**Conditions** (line 400):
- `complement_is_none && is_early_return && is_swap`
- Specifically: `else_body` absent, then exits, BUT `is_swap=true` (guard is `=== null`)

**Closure-Reassign Suppression** (lines 411–421):
```rust
if self.is_var_closure_reassigned(&var_name, guard_position) {
    // Emit predicate form instead: `if var.is_none() { exit }`
    return Ok(Some(vec![Stmt::If {
        condition: Expr::MethodCall {
            object: Box::new(Expr::Ident(var_name.clone())),
            method: "is_none".to_string(),
            args: vec![],
        },
        then_body: complement_body,
        else_body: None,
    }]));
}
```

When NOT closure-reassigned, emits **F2 Let-WRAP** (lines 423–441):
```rust
let none_arm = MatchArm {
    patterns: vec![Pattern::none()],
    guard: None,
    body: complement_body,
};
let some_arm = MatchArm {
    patterns: vec![Pattern::some_binding(&var_name)],
    guard: None,
    body: vec![Stmt::TailExpr(Expr::Ident(var_name.clone()))],
};
return Ok(Some(vec![Stmt::Let {
    mutable: false,
    name: var_name,
    ty: None,
    init: Some(Expr::Match { ... }),
}]));
```

#### Branch 3: Early Return with Non-Exit Else, Swap & Then-Exits, Else Non-Exit (F2 Deep-Fix with Else) — Lines 458–500

**Conditions** (line 458):
- `complement_is_none && is_swap && else_body.is_some() && then_exits && !else_exits`

This is the **T5 deep-fix form** mentioned in `option_truthy_complement.rs:30-36`.

**Closure-Reassign Suppression** (lines 465–477):
```rust
if self.is_var_closure_reassigned(&var_name, guard_position) {
    // Emit predicate form + positional body
    let condition = Expr::MethodCall {
        object: Box::new(Expr::Ident(var_name.clone())),
        method: "is_none".to_string(),
        args: vec![],
    };
    let mut combined = vec![Stmt::If {
        condition,
        then_body: complement_body.clone(),
        else_body: None,
    }];
    combined.extend(positive_body);
    return Ok(Some(combined));
}
```

When NOT closure-reassigned, emits **F2 Let-WRAP** with else body unwrapped inside Some arm (lines 479–499):
```rust
let none_arm = MatchArm {
    patterns: vec![Pattern::none()],
    guard: None,
    body: complement_body,
};
let mut some_body = positive_body;
some_body.push(Stmt::TailExpr(Expr::Ident(var_name.clone())));
let some_arm = MatchArm {
    patterns: vec![Pattern::some_binding(&var_name)],
    guard: None,
    body: some_body,
};
return Ok(Some(vec![Stmt::Let {
    mutable: false,
    name: var_name,
    ty: None,
    init: Some(Expr::Match { ... }),
}]));
```

#### Branch 4: Else Block Pattern (F3 Simple Match) — Lines 502–518

**Conditions** (line 502):
- `else_body.is_some() && !complement_is_none`

**Output Shape**:
```rust
match var {
    Positive(v) => { then_body },
    Complement(v) => { else_body }
}
```

**Code** (lines 502–518):
```rust
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
return Ok(Some(vec![Stmt::Match {
    expr,
    arms: vec![positive_arm, complement_arm],
}]));
```

#### Default: None (Fallback to `generate_if_let`)

If no branch condition matches, returns `Ok(None)` (line 520) → caller falls back to `generate_if_let` (line 58).

---

## 2. `generate_if_let` Definition & Emission

**File**: `/home/kyohei/ts_to_rs/src/transformer/statements/control_flow.rs:530-553`

### Signature
```rust
pub(super) fn generate_if_let(
    &self,
    guard: &crate::transformer::expressions::patterns::NarrowingGuard,
    then_body: Vec<Stmt>,
    else_body: Option<Vec<Stmt>>,
) -> Stmt
```

### Output Shape (F1 cons-span shadow)

**Code** (lines 536–552):
```rust
let (pattern, is_swap) = self.resolve_if_let_pattern(guard).unwrap();
let expr = Expr::Ident(guard.var_name().to_string());
if is_swap {
    Stmt::IfLet {
        pattern,
        expr,
        then_body: else_body.unwrap_or_default(),
        else_body: Some(then_body),
    }
} else {
    Stmt::IfLet {
        pattern,
        expr,
        then_body,
        else_body,
    }
}
```

### Emission Shape
Always emits **F1 cons-span shadow** (`Stmt::IfLet`):
- Pattern shadows the original variable with narrow type
- Post-`if let` scope: variable retains narrowed type ONLY inside then branch
- Post-`if let` scope (after entire `if let`): variable reverts to declared type

### Guard Resolution
Calls `resolve_if_let_pattern(guard)` which returns `(pattern, is_swap)`:
- `is_swap = true` for negated guards (`is_neq=true`, `is_eq=false`)
- `is_swap = false` for positive guards

### No TypeResolver Query
The narrow type is **implicit** in the pattern (e.g., `Some(x)` shadows `x` to `T` within the then arm).

---

## 3. `OptionTruthyShape` Enum — Three Emission Forms

**File**: `/home/kyohei/ts_to_rs/src/transformer/statements/option_truthy_complement.rs:45-57`

### Definition
```rust
pub(super) enum OptionTruthyShape {
    EarlyReturn {
        exit_body: Vec<Stmt>,
    },
    EarlyReturnFromExitWithElse {
        else_body: Vec<Stmt>,
        exit_body: Vec<Stmt>,
    },
    ElseBranch {
        positive_body: Vec<Stmt>,
        wildcard_body: Vec<Stmt>,
    },
}
```

### Selection Logic (Lines 124–137)

```rust
let shape = match (else_body, then_exits, else_exits) {
    (None, true, _) => OptionTruthyShape::EarlyReturn {
        exit_body: then_body.to_vec(),
    },
    (Some(else_stmts), true, false) => OptionTruthyShape::EarlyReturnFromExitWithElse {
        else_body: else_stmts.to_vec(),
        exit_body: then_body.to_vec(),
    },
    (Some(else_stmts), _, _) => OptionTruthyShape::ElseBranch {
        positive_body: else_stmts.to_vec(),
        wildcard_body: then_body.to_vec(),
    },
    (None, false, _) => return Ok(None),  // Predicate form (Matrix C-4)
};
```

### Production Sites

#### Form 1: EarlyReturn (F2 Let-WRAP)
**Condition**: `else_body.is_none() && then_exits`

**Emission** (line 149–155):
```rust
vec![Stmt::Let {
    mutable: false,
    name: var_name,
    ty: None,
    init: Some(Expr::Match {
        expr: Box::new(Expr::Ident(var_name.clone())),
        arms,  // [Some(v) if truthy => v, _ => exit_body]
    }),
}]
```

This is the "I-144 cell-i024" form mentioned in docstring (line 25-28).

#### Form 2: EarlyReturnFromExitWithElse (F2 Deep-Fix with Else)
**Condition**: `else_body.is_some() && then_exits && !else_exits`

**Emission** (lines 149–155, same Let-WRAP wrapper):
```rust
vec![Stmt::Let {
    mutable: false,
    name: var_name,
    ty: None,
    init: Some(expr),  // Match with [Some(...) => { else_body; x }, _ => exit]
}]
```

See `build_some_arm_body` (line 361–372) for composition.

#### Form 3: ElseBranch (F3 Simple Match)
**Condition**: `else_body.is_some() && !((then_exits && !else_exits))`

**Emission** (lines 156–161):
```rust
let Expr::Match { expr, arms } = expr else {
    unreachable!("just constructed an Expr::Match above");
};
vec![Stmt::Match { expr: *expr, arms }]
```

The match is **bare** (no Let wrapper) because post-if is unreachable or the falsy branch can fall through without useful narrow.

---

## 4. `visitors.rs` — AST Visitor & Deep-Deep-Fix-1 Condition

**File**: `/home/kyohei/ts_to_rs/src/pipeline/type_resolver/visitors.rs`

### Purpose
Walks module items, declarations, statements, and control-flow structures. Dispatches to specialized resolvers (narrowing, expected types, expressions) to populate `FileTypeResolution`.

### Key Narrow-Related Logic

#### `visit_if_stmt` (lines 699–755)

Detects narrowing guards and early-return narrowing:

```rust
fn visit_if_stmt(&mut self, if_stmt: &ast::IfStmt) {
    // Line 710-711: Main guard detection
    detect_narrowing_guard(&if_stmt.test, &if_stmt.cons, if_stmt.alt.as_deref(), self);
    
    // Lines 733-740: Early-return narrowing detection
    let then_exits = stmt_always_exits(&if_stmt.cons);
    let else_exits = if_stmt.alt.as_deref().is_some_and(stmt_always_exits);
    if then_exits && !else_exits {
        if let Some(block_end) = self.current_block_end {
            let if_end = if_stmt.cons.span().hi.0;
            detect_early_return_narrowing(&if_stmt.test, if_end, block_end, self);
        }
    }
    // ...
}
```

### Deep-Deep-Fix-1 Condition

Referenced in TODO I-171 T5 (line 730-731), this condition is:

```
then_exits && !else_exits
```

**Meaning**:
- The then-branch always exits (return/break/continue)
- The else-branch does NOT always exit (or is absent)
- Therefore, post-if scope is **unreachable via then-branch**, only reachable via else-branch or absence of else

**Usage**: Enables the early-return narrowing in TypeResolver and selects the **EarlyReturnFromExitWithElse** emission form in OptionTruthyShape (line 128-130 of `option_truthy_complement.rs`).

**Key Comment** (lines 714–740):
> "The then-exit branch contributes nothing to post-if scope (it either returns or unwinds), so the only path that flows past the `if` is the one matching the test-complement."

---

## 5. IR Node Types Involved in Narrow Emission

**File**: `/home/kyohei/ts_to_rs/src/ir/stmt.rs:26-130`

### Three Stmt Variants

| Variant | Code Shape | Narrow Scope | Post-Scope Type |
|---|---|---|---|
| `Stmt::IfLet { pattern, expr, then_body, else_body }` | `if let Some(x) = x { ... } [else ...]` | Shadow pattern within then arm | Reverts to declared type |
| `Stmt::Let { name, ty, init: Some(Expr::Match { ... }) }` | `let x = match x { Some(v) => v, _ => exit };` | Narrow materialized in let binding | Stays narrowed in post-scope |
| `Stmt::Match { expr, arms }` | `match x { Some(x) => then, None => else }` | Shadow pattern per arm | Reverts after match (no post-scope narrow) |

### Emission Sites in Transformer

#### `generate_if_let` (control_flow.rs:530–553)
→ Emits **Stmt::IfLet** (F1)

#### `try_generate_narrowing_match` (control_flow.rs:348–521)
→ Emits **Stmt::Let { init: Expr::Match }** (F2, branches 1–3)
→ Emits **Stmt::Match** (F3, branch 4)

#### `try_generate_option_truthy_complement_match` (option_truthy_complement.rs:84–163)
→ Emits **Stmt::Let { init: Expr::Match }** (F2 shapes EarlyReturn & EarlyReturnFromExitWithElse, lines 149–155)
→ Emits **Stmt::Match** (F3 shape ElseBranch, lines 156–161)

---

## 6. All Production Code Paths Reading `is_var_closure_reassigned`

**Setter**: `FileTypeResolution::is_var_closure_reassigned` (type_resolution.rs:253–264)

**All Callers** (6 locations):

### 1. `try_generate_narrowing_match` (control_flow.rs:411)
```rust
if self.is_var_closure_reassigned(&var_name, guard_position) {
    // Suppress F2 Let-WRAP, emit predicate form instead
    let condition = Expr::MethodCall { method: "is_none", ... };
    return Ok(Some(vec![Stmt::If { condition, ... }]));
}
```
**Position**: Line 411 (Option early return swap form)

### 2. `try_generate_narrowing_match` (control_flow.rs:465)
```rust
if self.is_var_closure_reassigned(&var_name, guard_position) {
    // Suppress F2 Let-WRAP, emit predicate form + positional body
    let condition = Expr::MethodCall { method: "is_none", ... };
    let mut combined = vec![Stmt::If { condition, ... }];
    combined.extend(positive_body);
    return Ok(Some(combined));
}
```
**Position**: Line 465 (Option early return with non-exit else form)

### 3. `try_generate_option_truthy_complement_match` (option_truthy_complement.rs:118)
```rust
if self.is_var_closure_reassigned(&var_name, if_stmt_position) {
    return Ok(None);  // Fall back to predicate form
}
```
**Position**: Line 118

**Effect**: When called with closure-reassign flag set, returns `None`, causing the caller (`convert_if_stmt` line 138–145) to fall through to the predicate-form emission (`try_generate_primitive_truthy_condition` → `falsy_predicate`).

### 4. `narrowed_type` (type_resolution.rs:189)
```rust
pub fn narrowed_type(&self, var_name: &str, position: u32) -> Option<&RustType> {
    if self.is_var_closure_reassigned(var_name, position) {
        return None;
    }
    self.narrow_events.iter()...
}
```
**Position**: Line 189

**Effect**: Suppresses narrow type queries when closure-reassign is detected, forcing callers to use the declared `Option<T>` type instead.

### 5. `maybe_coerce_for_arith` (expressions/binary.rs:~line unknown, search showed usage)
```rust
if !self.is_var_closure_reassigned(id.sym.as_ref(), ast_expr.span().lo.0) {
    return ir_expr;
}
// Apply coerce_default wrapper
```

**Effect**: Determines whether to wrap Option-typed arithmetic operands in a default coercion (e.g., `Option<f64> + 1` → `Some(x).unwrap_or(0) + 1`).

### 6. `maybe_coerce_for_string_concat` (expressions/binary.rs:~line unknown)
```rust
if !self.is_var_closure_reassigned(id.sym.as_ref(), ast_expr.span().lo.0) {
    return ir_expr;
}
// Apply coerce_default wrapper
```

**Effect**: Similar to #5, but for string concatenation contexts.

---

## 7. IR Shadow Form vs Predicate Form — Narrow Scope Difference

### F1 Shadow Form: `if let Some(x) = x { body }`

**File**: control_flow.rs:530–553 (`generate_if_let`)

**Code**:
```rust
Stmt::IfLet {
    pattern: Pattern::some_binding("x"),  // Binds local x: T
    expr: Expr::Ident("x"),              // From outer Option<T>
    then_body,
    else_body,
}
```

**Narrow Scope Behavior**:
- **Inside then-arm**: `x` is shadowed to `T` by the pattern binding
- **After if-let block**: `x` reverts to `Option<T>` (the original outer binding)

**Post-`if let` Reachability**:
- If no else branch: post-if `x` is `Option<T>` (fallthrough from unmatched case)
- If else branch exists: post-if `x` is `Option<T>` (after either branch)

**Example**:
```rust
let x: Option<i32> = Some(42);
if let Some(x) = x {
    // x: i32 (shadowed)
    println!("{}", x + 1);  // ✓ OK, x is i32
}
// x: Option<i32> (reverted)
let y = x.map(|v| v * 2);  // ✓ OK, x is Option<i32>
```

---

### Predicate Form: `if x.is_some() { ... }`

**Emission Sites**:
- `try_generate_primitive_truthy_condition` (control_flow.rs:197–213) for bare `if (x)` and `if (!x)`
- `falsy_predicate` / `truthy_predicate` (helpers/truthy.rs) for method-call emission

**Code** (truthy.rs example, conceptual):
```rust
Expr::MethodCall {
    object: Box::new(Expr::Ident("x")),
    method: "is_some".to_string(),
    args: vec![],
}
```

**Narrow Scope Behavior**:
- **No variable shadowing**: `x` remains `Option<T>` throughout
- **No narrow type available**: TypeResolver's `narrowed_type` suppression applies
- **Post-if scope**: `x` is still `Option<T>`; no narrow type available for coercions

**Example**:
```rust
let x: Option<i32> = Some(42);
if x.is_some() {
    // x: Option<i32> (NOT narrowed; no shadow)
    let v = x + 1;  // ✗ Type error: Option<i32> + i32
                    // Must use: x.unwrap() + 1 or x.map(|v| v + 1)
}
// x: Option<i32>
```

---

### Key Difference Summary

| Aspect | F1 Shadow (if let) | Predicate (is_some) |
|---|---|---|
| Variable Rebinding | Yes, shadows with `T` inside then | No, stays `Option<T>` |
| Post-If Narrow Available | No, reverts to `Option<T>` | No, suppressed by closure-reassign |
| Closure-Reassign Suppression | Suppresses entire branch (uses predicate) | Naturally compatible (no shadow to invalidate) |
| Arithmetic Coerce Wrapper | Not needed (narrow `T` in scope) | Required (via `coerce_default`) |
| Ideal For | Non-closure code paths | Closure-reassigned variables |

---

## 8. Closure-Reassign Suppression Scope Architecture

### Detection Pipeline

**File**: `/home/kyohei/ts_to_rs/src/pipeline/narrowing_analyzer.rs` + `/src/pipeline/narrowing_analyzer/closure_captures.rs`

**Entry**: `analyze_function(body, params)` scans for:
1. Outer variables captured by closures
2. Reassignment to those variables inside closure bodies
3. Emits `NarrowEvent::ClosureCapture { var_name, enclosing_fn_body }`

**Storage**: `FileTypeResolution::narrow_events: Vec<NarrowEvent>`

**Query**: `FileTypeResolution::is_var_closure_reassigned(var_name, position: u32) -> bool`

```rust
pub fn is_var_closure_reassigned(&self, var_name: &str, position: u32) -> bool {
    self.narrow_events.iter().any(|e| match e {
        NarrowEvent::ClosureCapture {
            var_name: v,
            enclosing_fn_body,
            ..
        } => {
            v == var_name 
            && enclosing_fn_body.lo <= position 
            && position < enclosing_fn_body.hi
        }
        _ => false,
    })
}
```

### Scope Isolation (I-169 P1 Fix)

**Key**: Position membership against `enclosing_fn_body` ensures **multi-fn scope isolation**.

**Example**:
```rust
fn f() {
    let x: Option<i32> = Some(42);
    let closure = || { x = None; };  // Closure reassigns x in fn f
    // Narrow suppression applies here (inside fn f's body)
}

fn g() {
    let x: Option<i32> = Some(99);
    // x: narrow available here (different fn, different closure capture event)
}
```

**Code** (narrowing_analyzer.rs, conceptual):
```rust
let mut candidates = closure_captures::collect_outer_candidates(params, &body.stmts);
// candidates: Vec of (var_name, positions where captured)

// For each candidate, check if it's reassigned inside closure
// If reassigned, emit:
NarrowEvent::ClosureCapture {
    var_name,
    enclosing_fn_body: Span::from_swc(fn_decl.span()),  // Entire function scope
    // ...
}
```

---

## Dispatch Flow Diagram

```
convert_if_stmt (control_flow.rs:19)
│
├─> extract_conditional_assignment? (line 24)
│   └─> convert_if_with_conditional_assignment (line 31)
│
├─> extract_narrowing_guards (line 34)
│   └─> compound guards extraction
│
├─> Single guard, resolvable?  (line 40-59)
│   │
│   ├─YES─> try_generate_narrowing_match (line 50-57)
│   │        ├─> resolve_if_let_pattern(guard)
│   │        │   └─> Positive pattern + is_swap flag
│   │        ├─> resolve_complement_pattern(guard)
│   │        │   └─> Complement pattern
│   │        │
│   │        ├─> Branch 1: is_early_return && !complement_is_none
│   │        │   └─> F2 Let-WRAP: `let x = match x { Pos => exit, Comp => x }`
│   │        │
│   │        ├─> Branch 2: complement_is_none && is_early_return && is_swap
│   │        │   ├─> is_var_closure_reassigned()?
│   │        │   │   ├─YES─> F4 Predicate: `if x.is_none() { exit }`
│   │        │   │   └─NO──> F2 Let-WRAP: `let x = match x { None => exit, Some => x }`
│   │        │   │
│   │        │
│   │        ├─> Branch 3: complement_is_none && is_swap && else_body && then_exits && !else_exits
│   │        │   ├─> is_var_closure_reassigned()?
│   │        │   │   ├─YES─> F4 Predicate: `if x.is_none() { exit }` + positional_body
│   │        │   │   └─NO──> F2 Let-WRAP: `let x = match x { None => exit, Some => else+x }`
│   │        │   │
│   │        │
│   │        ├─> Branch 4: else_body.is_some() && !complement_is_none
│   │        │   └─> F3 Simple Match: `match x { Pos => then, Comp => else }`
│   │        │
│   │        └─> None (fallback)
│   │            └─> generate_if_let (line 58)
│   │                 └─> F1 Shadow: `if let pattern = x { then } [else]`
│   │
│   └─NO──> Fall through to compound guards path
│
├─> try_generate_option_truthy_complement_match (line 138-145)
│   ├─> Detects: `if (!x)` where x: Option<T>
│   ├─> is_var_closure_reassigned()?
│   │   ├─YES─> return None → fall through to predicate form
│   │   └─NO──> Build OptionTruthyShape
│   │           │
│   │           ├─> Shape EarlyReturn (no else, then exits)
│   │           │   └─> F2 Let-WRAP
│   │           │
│   │           ├─> Shape EarlyReturnFromExitWithElse (else, then exits, else non-exit)
│   │           │   └─> F2 Let-WRAP (Deep-Fix-1)
│   │           │
│   │           └─> Shape ElseBranch (else, or both exit, or then non-exit)
│   │               └─> F3 Simple Match
│   │
│
└─> try_generate_primitive_truthy_condition (line 147-151)
    ├─> Detects: `if (x)` / `if (!x)` on simple identifiers
    └─> F4 Predicate: `Expr::MethodCall { method: "is_some"/"is_none", ... }`
        └─> If constant-foldable: Expr::BoolLit(b)
```

---

## Summary Table: Emission Paths

| Form | Emitted By | Guard Kind | Condition | Output | Post-Scope Narrow | Closure-Reassign Behavior |
|---|---|---|---|---|---|---|
| F1 | `generate_if_let` | Any | Fallback | `if let Some(x) = x { ... }` | Reverts to Option<T> | Suppressed (uses F4 instead) |
| F2 | `try_generate_narrowing_match` (Branch 1–3) | NonNullish, Typeof, InstanceOf | Early return or swap+else | `let x = match x { Pos => body, Comp => body }` | Stays narrowed | Switches to F4 (predicate) |
| F3 | `try_generate_narrowing_match` (Branch 4) or `option_truthy` | Any | Else block present + no early-return | `match x { Pos => then, Comp => else }` | Reverts to Option<T> | Suppresses entire emission |
| F4 | Predicate emission | NonNullish, Typeof (negated) | Closure-reassign detected | `if x.is_none() / is_some()` | No narrow available | Designed for this case |

---

## References

- **control_flow.rs**: Lines 19–175 (convert_if_stmt), 348–521 (try_generate_narrowing_match), 530–553 (generate_if_let)
- **option_truthy_complement.rs**: Lines 45–57 (OptionTruthyShape), 84–163 (try_generate_option_truthy_complement_match)
- **visitors.rs**: Lines 699–755 (visit_if_stmt)
- **type_resolution.rs**: Lines 173–199 (narrowed_type), 240–264 (is_var_closure_reassigned)
- **narrowing_analyzer.rs**: Documentation of closure-capture events and enclosing_fn_body scope
- **patterns.rs**: Lines 494–542 (resolve_if_let_pattern), 431–452 (resolve_complement_pattern)

---

**End of Report**
