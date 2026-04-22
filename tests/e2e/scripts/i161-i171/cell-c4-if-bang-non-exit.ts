// I-171 Layer 2 Cell C-4 (non-exit body): `if (!x) <non-exit>` on Option<F64>.
// Current emission: fall-through naive `if !x { ... }` → E0600.
// Ideal: predicate form `if x.map_or(true, |v| *v == 0.0 || v.is_nan()) { side_effect; }`.
// Narrow NOT materialized after if (body is non-exit).

function f(x: number | null): string {
    if (!x) console.log("falsy_side_effect");
    return x === null ? "null" : `${x}`;
}

function main(): void {
    console.log(f(null));
    console.log(f(0));
    console.log(f(5));
}
