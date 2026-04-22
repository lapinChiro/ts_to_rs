// I-161 Cell A-5 (Option<F64> without narrow): `x &&= y` on Option LHS.
// JS semantics: Some(5) &&= 3 → Some(3); None &&= 3 → None; Some(0) &&= 3 → Some(0).
// Current emission: `x = x && 3.0;` (invalid on Option<f64>).
// Ideal: `if x.is_some_and(|v| *v != 0.0 && !v.is_nan()) { x = Some(3.0); }`.

function f(init: number | null): number | null {
    let x: number | null = init;
    x &&= 3;
    return x;
}

function show(v: number | null): string {
    return v === null ? "null" : `${v}`;
}

function main(): void {
    console.log(show(f(5)));    // 3
    console.log(show(f(null))); // null
    console.log(show(f(0)));    // 0
}
