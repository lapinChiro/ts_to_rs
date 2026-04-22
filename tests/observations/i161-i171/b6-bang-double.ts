// B6: `!!x` (double negation) on Option<number>.
function f(x: number | null): boolean {
    return !!x;
}
console.log(f(null));  // false
console.log(f(0));     // false
console.log(f(5));     // true

function g(x: number | null): string {
    if (!!x) return "truthy";
    return "falsy";
}
console.log(g(5));
console.log(g(0));
console.log(g(null));
