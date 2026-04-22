// B14: `if (!x) return <val>;` on Option<number> — early return pattern, T6-3 regression lock-in.
function f(x: number | null): string {
    if (!x) return "none";
    // x narrowed to number (non-falsy)
    return `ok:${x + 1}`;
}
console.log(f(null));
console.log(f(0));
console.log(f(5));
