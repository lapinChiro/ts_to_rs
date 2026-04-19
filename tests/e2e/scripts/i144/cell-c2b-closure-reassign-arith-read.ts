// I-144 Cell C-2b: closure reassigns narrowed outer var, later arithmetic read.
// Sub-matrix 5 cell RC1 × L1 stale — ideal emission is E2b
// `x.unwrap_or(0.0) + 1.0` (JS coerce_default: null → 0 in arithmetic).
// Current emission: shadow-let makes x an f64 local; closure body `x = null`
// does not compile (E0308). Even if compilation succeeded, the post-closure
// read `x + 1` must produce JS `null + 1 = 1` per the coerce_default table.
// TS runtime (observed cl3b): returns 1.

function f(): number {
    let x: number | null = 5;
    if (x === null) return -1;
    const reset = () => { x = null; };
    reset();
    return x + 1;
}

function main(): void {
    console.log(f());
}
