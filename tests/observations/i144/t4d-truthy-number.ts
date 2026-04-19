// T4d: if truthy on `number` — TS narrows to non-zero?
// TS 4.x: `if (x)` on `number` narrows to `number` (no literal type shift);
// runtime: 0 / NaN are falsy.
function f(x: number): string {
    if (x) {
        return "nonzero: " + x;
    }
    return "zero-or-nan";
}
console.log(f(5));
console.log(f(0));
console.log(f(-1));
console.log(f(NaN));
