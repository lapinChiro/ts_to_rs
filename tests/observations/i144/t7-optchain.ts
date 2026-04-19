// T7: x?.v with x: {v: number} | null — does the non-null narrow propagate?
function f(x: { v: number } | null): number | undefined {
    return x?.v;
}
// Sequential narrow pattern
function g(x: { v: number } | null): number {
    if (x?.v !== undefined) {
        // Does TS narrow x to non-null inside this branch?
        return x.v * 2;
    }
    return -1;
}
console.log(f({ v: 10 }));
console.log(f(null));
console.log(g({ v: 10 }));
console.log(g(null));
