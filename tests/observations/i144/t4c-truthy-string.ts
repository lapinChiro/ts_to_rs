// T4c: if truthy on `string` — TS narrows to "non-empty string" via control-flow?
// TS 4.x: `if (x)` on `string` narrows to `string` (same type, no variant), but empty string is falsy at runtime.
function f(x: string): number {
    if (x) {
        // inside: x is still typed `string`, but at runtime non-empty
        return x.length;
    }
    return -1;
}
console.log(f("hello"));
console.log(f(""));
console.log(f("a"));
