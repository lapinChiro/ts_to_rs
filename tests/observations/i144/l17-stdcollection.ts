// L17: StdCollection narrow — truthy on HashMap equivalent (Record)
function f(x: Record<string, number> | null): number {
    if (x) {
        // x narrowed to Record<string, number>
        return Object.keys(x).length;
    }
    return -1;
}
// Truthy on Map
function g(x: Map<string, number> | null): number {
    if (x) {
        return x.size;
    }
    return -1;
}
// Empty HashMap is truthy (JS: empty object is truthy)
console.log(f({}));
console.log(f({a: 1, b: 2}));
console.log(f(null));
console.log(g(new Map()));
console.log(g(new Map([["a", 1]])));
console.log(g(null));
