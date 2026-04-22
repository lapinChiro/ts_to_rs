// I-171 Layer 1 Cell B.1.23 (LogicalAnd operand): `!(x && y)` on mixed types.
// TS: `x && y` = first falsy or last value. `!(x && y)` = De Morgan `!x || !y`.
// Current emission: `!(x && y)` with `x: Option<f64>, y: Option<String>` — invalid.
// Ideal: `<x falsy> || <y falsy>` → `!x.is_some_and(...) || !y.as_ref().is_some_and(...)`.

function f(x: number | null, y: string | null): string {
    if (!(x && y)) return "at_least_one_falsy";
    return `ok:${x}:${y}`;
}

function main(): void {
    console.log(f(null, "a")); // at_least_one_falsy
    console.log(f(5, null));   // at_least_one_falsy
    console.log(f(0, "a"));    // at_least_one_falsy
    console.log(f(5, ""));     // at_least_one_falsy
    console.log(f(5, "a"));    // ok:5:a
}
