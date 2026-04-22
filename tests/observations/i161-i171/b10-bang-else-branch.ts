// B10: `if (!x) then else` on Option<number>.
// Else branch: x is narrowed to the truthy complement.
function f(x: number | null): string {
    if (!x) {
        return "falsy";
    } else {
        // x narrowed to number (and non-falsy)
        return `truthy:${x + 1}`;
    }
}
console.log(f(null));
console.log(f(0));
console.log(f(5));
