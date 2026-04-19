// T4f: if truthy on `string[]` — empty array is truthy in JS/TS.
// TS narrows to `string[]` (same type, no sig change).
function f(x: string[]): number {
    if (x) {
        return x.length;
    }
    return -1;
}
console.log(f(["a", "b"]));
console.log(f([]));
