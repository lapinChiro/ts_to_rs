// B7: `!(x && y)` - De Morgan expansion.
function f(x: number | null, y: string | null): string {
    if (!(x && y)) return "at_least_one_falsy";
    return `${x}:${y}`;
}
console.log(f(null, "a"));  // x falsy
console.log(f(1, null));    // y falsy (x && y = null since y is falsy last)
console.log(f(0, "a"));     // x falsy
console.log(f(1, ""));      // y falsy
console.log(f(1, "a"));     // both truthy
