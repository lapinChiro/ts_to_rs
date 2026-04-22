// I-171 Layer 1 Cell B.1.19 (double negation): `!!x` on Option<F64>.
// TS: `!!x` = truthy predicate of x.
// Current emission: `!(!x)` on Option<f64> → inner `!x` fails.
// Ideal: `x.is_some_and(|v| *v != 0.0 && !v.is_nan())` (truthy predicate directly).

function f(x: number | null): boolean {
    return !!x;
}

function main(): void {
    console.log(f(null)); // false
    console.log(f(0));    // false
    console.log(f(5));    // true
}
