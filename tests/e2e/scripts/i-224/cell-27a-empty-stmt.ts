// Cell 27-a: A5a + B0 — top-level Stmt::Empty (`;` standalone)
// Spec: A5a = Stmt::Empty (semicolon-only no-op), B0 = no user main, C0 = no top-await
// Ideal: silent skip (no fn main necessary if Empty is the only top-level "stmt"; library mode if no other exec)
// Empirical (TS): no-op, stdout=(empty), exit_code=0
;
// (Empty stmt is the sole top-level item beyond decl-less context; mixed with explicit Empty
// means cell stays in A5a partition. Adding console.log() would shift to A6 mixed.)
function helper(): void { /* declaration only, A5a partition preserved */ }
