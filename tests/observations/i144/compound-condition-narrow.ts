// Compound condition narrow
// Case 1: (x && x.v) — short-circuit narrow
function f(x: { v: number } | null): number {
    if (x && x.v > 0) {
        // TS: x narrowed to {v:number}
        return x.v * 2;
    }
    return -1;
}
// Case 2: || narrow — does TS narrow on ||?
function g(x: number | string | null): string {
    if (x === null || typeof x === "number") {
        return x === null ? "null" : "n:" + x;
    }
    // x: string (narrowed via || complement)
    return "s:" + x.toUpperCase();
}
// Case 3: Optional chain short-circuit with narrow
function h(x: { v?: number } | null): number {
    if (x?.v) {
        // TS: x narrowed to {v:number} (and x.v is truthy)
        return x.v + 1;
    }
    return -1;
}
console.log(f({v: 5}));
console.log(f({v: 0}));
console.log(f(null));
console.log(g(null));
console.log(g(3));
console.log(g("ab"));
console.log(h({v: 10}));
console.log(h({}));
