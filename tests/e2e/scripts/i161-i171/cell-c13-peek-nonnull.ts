// I-171 Layer 2 Cell C-13 (TsNonNull peek-through): `!(x!)` in if-cond.
// Ideal: peek-through TsNonNull, use asserted non-null inner type.

function f(x: number | null): string {
    if (!(x!)) return "none";
    return `ok:${x + 1}`;
}

function main(): void {
    // null + 1 = 1 in JS; but `x!` asserts non-null, calling f(null) at runtime
    // produces `null` which `!null` = true → "none" (runtime tolerates null).
    console.log(f(null)); // none
    console.log(f(0));    // none
    console.log(f(5));    // ok:6
}
