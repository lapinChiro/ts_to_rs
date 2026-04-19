// I-144 Cell C-2a: `??=` + closure capture reassigning outer var.
// Sub-matrix 5 cell RC3 × L1 stale — ideal emission is E2a
// `x.get_or_insert_with(|| d)` (Option preserved) so the closure can still
// reassign `x` to None afterwards.
// Current emission: shadow-let `let x = x.unwrap_or(0.0)` binds a local f64, the
// closure body then tries `x = None` on an f64 local → rustc E0308.
// TS runtime: x ??= 0 leaves 5; closure sets x = null; `x ?? -99` → -99.

function f(): number {
    let x: number | null = 5;
    x ??= 0;
    const reset = () => { x = null; };
    reset();
    return x ?? -99;
}

function main(): void {
    console.log(f());
}
