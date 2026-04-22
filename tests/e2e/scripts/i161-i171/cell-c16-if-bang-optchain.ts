// I-171 Layer 2 Cell C-16 (OptChain `!x?.v` always-exit): Layer 1 only.
// Post-if narrow materialization of `x` (OptChain base non-null) would require
// extending guards.rs Bang arm with OptChain case. That is explicitly OUT OF
// SCOPE for this PRD (the scope note on C-16 in the PRD defers it to a future
// PRD; empirical verification 2026-04-22).
//
// To make this fixture Layer-1-only compatible (i.e., GREEN after T5 without
// requiring OptChain base narrow), post-if uses `x?.v` throughout so no narrow
// materialization is assumed. The Layer 1 fix targets the `!x?.v` compile
// error at the if-condition site only.

function f(x: { v: string | null } | null): string {
    if (!x?.v) return "none";
    return (x?.v ?? "") + ":ok";
}

function main(): void {
    console.log(f(null));          // none
    console.log(f({ v: null }));   // none
    console.log(f({ v: "" }));     // none
    console.log(f({ v: "hi" }));  // hi:ok
}
