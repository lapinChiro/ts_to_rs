// I-171 Layer 1 Cell B-T2 (F64 operand): `!x` on number return.
// Current emission: `!x` on f64 → E0600.
// Ideal: `x == 0.0 || x.is_nan()`.

function f(x: number): boolean {
    return !x;
}

function main(): void {
    console.log(f(0));   // true
    console.log(f(NaN)); // true
    console.log(f(5));   // false
    console.log(f(-1));  // false
}
