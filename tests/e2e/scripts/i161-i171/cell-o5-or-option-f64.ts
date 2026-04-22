// I-161 Cell O-5 (Option<F64> without narrow): `x ||= y` on Option LHS.
// JS: None ||= 3 → Some(3); Some(5) ||= 3 → Some(5); Some(0) ||= 3 → Some(3).
// Ideal: `if x.map_or(true, |v| *v == 0.0 || v.is_nan()) { x = Some(3.0); }`.

function f(init: number | null): number | null {
    let x: number | null = init;
    x ||= 3;
    return x;
}

function show(v: number | null): string {
    return v === null ? "null" : `${v}`;
}

function main(): void {
    console.log(show(f(null))); // 3
    console.log(show(f(5)));    // 5
    console.log(show(f(0)));    // 3
}
