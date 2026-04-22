// I-171 Layer 1 Cell B-T5 (Option<F64> operand): `!x` on Option<number>.
// Current emission: `!x` on Option<f64> → E0600.
// Ideal: `!x.is_some_and(|v| *v != 0.0 && !v.is_nan())`.

function f(x: number | null): boolean {
    return !x;
}

function main(): void {
    console.log(f(null)); // true
    console.log(f(0));    // true
    console.log(f(NaN));  // true
    console.log(f(5));    // false
}
