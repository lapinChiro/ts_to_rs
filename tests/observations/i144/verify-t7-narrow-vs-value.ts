// T7 verification: does TS actually narrow `x` itself when `x?.v !== undefined`,
// or only narrow the *result of* `x?.v`?
function g(x: { v: number } | null): number {
    if (x?.v !== undefined) {
        // If TS narrows x → non-null: `x.v` works without ?
        // If TS only narrows result: we'd need `x?.v` still
        const direct = x.v;  // does this compile?
        return direct * 2;
    }
    return -1;
}
// Variant where x.v might be undefined even when x is non-null
function h(x: { v?: number } | null): number {
    if (x?.v !== undefined) {
        // Now: x is non-null AND x.v is defined.
        // Both narrows should hold.
        return x.v * 2;  // compile?
    }
    return -1;
}
console.log(g({v: 10}));
console.log(g(null));
console.log(h({v: 10}));
console.log(h({}));
console.log(h(null));
