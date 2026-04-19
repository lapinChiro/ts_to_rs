// R6: pass-by-mutation — TS preserves narrow even if callee mutates (aliasing issue)
function mutate(a: number[]): void {
    a.push(99);
}
function f(x: number[] | null): number {
    if (x !== null) {
        mutate(x);
        // TS: x: number[] still (mutate's signature takes number[], not reassigns x)
        return x.length;
    }
    return -1;
}
function reset(x: { v: number | null }): void {
    x.v = null;
}
function g(o: { v: number | null }): number {
    if (o.v !== null) {
        // Narrow: o.v is number
        reset(o);
        // TS: does narrow on o.v persist? Intuitively NO (reset could set to null),
        // but TS sometimes preserves through assignment-via-property.
        return o.v ?? -99;  // use ?? to avoid type error
    }
    return -1;
}
console.log(f([1, 2, 3]));
console.log(f(null));
console.log(g({ v: 10 }));
