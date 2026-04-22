// B5: `!(x + 1)` on F64.
function f(x: number): string {
    if (!(x + 1)) return "falsy";
    return "truthy";
}
console.log(f(-1));  // x+1 = 0 → falsy
console.log(f(0));   // x+1 = 1 → truthy
console.log(f(5));   // x+1 = 6 → truthy
