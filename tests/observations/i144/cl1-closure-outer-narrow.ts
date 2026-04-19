// Closure × Loop: closure captures outer-scoped variable narrowed in outer scope
// Pattern 1: closure inside narrowed if — TS narrow visible inside closure?
function f(x: number | null): number {
    if (x === null) return -1;
    // x: number (narrowed)
    const getter = () => x;  // does TS see `x: number` or `x: number | null`?
    return getter();
}
console.log(f(5));
console.log(f(null));
