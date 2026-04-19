// I-144 Cell C-1: compound arithmetic preserves narrow after `??=` binding.
// Sub-matrix 2 cell L1 × R2a — scanner false-positive: `x += 1` treated as
// reset, emits `UnsupportedSyntaxError("nullish-assign with narrowing-reset")`.
// TS runtime: x ??= 10 leaves x=5 (non-null); x += 1 → 6.
// Ideal Rust (post-I-144): narrow state stays alive across R2a, emission keeps
// Option if needed + unwraps for arithmetic.

function f(): number {
    let x: number | null = 5;
    x ??= 10;
    x += 1;
    return x;
}

function main(): void {
    console.log(f());
}
