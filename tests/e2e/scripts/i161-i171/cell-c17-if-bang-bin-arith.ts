// I-171 Layer 2 Cell C-17 (Bin arith in if-cond): `if (!(x + 1)) { ... }`.
// Ideal: Layer 1 feed-through with tmp bind on BinExpr.

function f(x: number): string {
    if (!(x + 1)) return "zero_at_plus_one";
    return `ok:${x + 1}`;
}

function main(): void {
    console.log(f(-1)); // zero_at_plus_one
    console.log(f(0));  // ok:1
    console.log(f(5));  // ok:6
}
