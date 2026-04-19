// I-144 Cell C-2c: closure reassigns narrowed outer var, later string concat.
// Sub-matrix 5 cell RC6 × L1 stale — ideal emission is E2b with String default
// `x.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string())` (or similar)
// to reproduce JS `"v=" + null = "v=null"`.
// Current emission: shadow-let binds x locally; closure cannot reassign + the
// concatenation path assumes narrow alive.
// TS runtime: returns "v=null".

function f(): string {
    let x: number | null = 5;
    if (x === null) return "no";
    const reset = () => { x = null; };
    reset();
    return "v=" + x;
}

function main(): void {
    console.log(f());
}
