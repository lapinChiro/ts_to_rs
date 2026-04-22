// B9: `if (!x) <non-exit body>` on Option<number>.
// After the if, x is still Option<number> (no narrow materialization post-if).
function f(x: number | null): string {
    if (!x) console.log("falsy");  // non-exit body; narrow not materialized after
    // x remains `number | null` here
    return x === null ? "null" : `${x}`;
}
console.log(f(null));
console.log(f(0));
console.log(f(5));
