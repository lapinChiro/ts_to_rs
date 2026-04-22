// B.1.24 / C-23 observation: `!(x || y)` De Morgan semantics.
// JS: `x || y` returns first truthy or last falsy. `!(x || y)` = De Morgan `!x && !y`.
function f(x: number | null, y: string | null): string {
    if (!(x || y)) return "both_falsy";
    return `ok:${x ?? "null"}:${y ?? "null"}`;
}
console.log(f(null, null));  // both_falsy
console.log(f(0, ""));       // both_falsy
console.log(f(5, null));     // ok:5:null
console.log(f(null, "a"));   // ok:null:a
console.log(f(5, "a"));      // ok:5:a
